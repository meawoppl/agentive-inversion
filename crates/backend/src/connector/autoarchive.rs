//! Archive-only delegation. Classification is agent judgment; effect, grant,
//! ownership, freshness and unchanged-thread checks are enforced here.
use super::{auth::Principal, models::ActionRow};
use crate::{
    error::{ApiError, ApiResult},
    pollers::gmail_client::EmailMessage,
    AppState,
};
use axum::{
    extract::{Path, State},
    Extension, Json,
};
use chrono::{Duration, Utc};
use diesel::{
    sql_query,
    sql_types::{Array, Text, Uuid as SqlUuid},
};
use diesel_async::{AsyncConnection, RunQueryDsl};
use shared_types::connector::{ActionInfo, AutoArchiveCategory, AutoArchiveRequest};
use uuid::Uuid;

pub(super) fn validate(input: &AutoArchiveRequest) -> ApiResult<()> {
    let age = Utc::now() - input.reviewed_at;
    if input.reason.trim().is_empty()
        || input.reason.len() > 8000
        || input.thread_message_ids.is_empty()
        || input.thread_message_ids.len() > 1000
        || input
            .thread_message_ids
            .iter()
            .any(|id| id.is_empty() || id.len() > 128)
        || input.task_ids.len() > 100
        || age > Duration::minutes(15)
        || age < Duration::seconds(-30)
        || (matches!(input.category, AutoArchiveCategory::TaskCaptured)
            && input.task_ids.is_empty())
    {
        return Err(ApiError::bad_request("Auto-archive needs a bounded reason, a fresh full-thread snapshot, and task IDs for task-captured mail"));
    }
    let mut ids = input.thread_message_ids.clone();
    ids.sort();
    ids.dedup();
    if ids.len() != input.thread_message_ids.len() {
        return Err(ApiError::bad_request("Thread message IDs must be unique"));
    }
    Ok(())
}

pub async fn queue(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<Uuid>,
    Json(input): Json<AutoArchiveRequest>,
) -> ApiResult<Json<ActionInfo>> {
    p.scope("actions:autoarchive")?;
    let cid = p.connection()?;
    let mut c = state.pool.get().await?;
    let row = c.transaction::<ActionRow, ApiError, _>(async move |c| {
        // Lock grant before action, consistent with proposal submission.
        sql_query("SELECT id FROM connector_connections WHERE id=$1 AND revoked_at IS NULL AND expires_at>now() AND 'actions:autoarchive'=ANY(scopes) FOR UPDATE")
            .bind::<SqlUuid,_>(cid).get_result::<super::models::IdRow>(&mut *c).await?;
        let row = sql_query("SELECT a.* FROM connector_actions a JOIN emails e ON e.id=a.email_id WHERE a.id=$1 AND a.connection_id=$2 AND e.account_id=ANY($3) AND a.kind='archive' FOR UPDATE OF a")
            .bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(cid).bind::<Array<SqlUuid>,_>(&p.account_ids)
            .get_result::<ActionRow>(&mut *c).await?;
        let encoded = serde_json::to_string(&input)?;
        // Exact retries return the original ledger entry, even after completion.
        if let Some(previous) = &row.autoarchive_json {
            if previous == &encoded { return Ok(row); }
            return Err(ApiError::Conflict("Archive decision already recorded; inspect its receipt".into()));
        }
        if row.status != "proposed" {
            return Err(ApiError::Conflict("Only a pending archive can be automatically queued".into()));
        }
        validate(&input)?;
        sql_query("UPDATE connector_actions SET status='queued',autoarchive_json=$2,approved_by=$3,approved_connection_id=$4,updated_at=now() WHERE id=$1 RETURNING *")
            .bind::<SqlUuid,_>(id).bind::<Text,_>(encoded).bind::<Text,_>(p.email)
            .bind::<SqlUuid,_>(cid).get_result::<ActionRow>(&mut *c).await.map_err(Into::into)
    }).await?;
    Ok(Json(row.try_into()?))
}

pub(super) fn check_thread(
    input: &AutoArchiveRequest,
    source: &str,
    messages: &[EmailMessage],
) -> anyhow::Result<()> {
    validate(input).map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let mut expected = input.thread_message_ids.clone();
    expected.sort();
    let mut current: Vec<_> = messages.iter().map(|m| m.id.clone()).collect();
    current.sort();
    anyhow::ensure!(
        expected == current,
        "Thread changed after agent review; human review required"
    );
    let message = messages
        .iter()
        .find(|m| m.id == source)
        .ok_or_else(|| anyhow::anyhow!("Source no longer in thread"))?;
    anyhow::ensure!(
        !message
            .labels
            .iter()
            .any(|l| l == "TRASH" || l == "SPAM" || l == "DRAFT"),
        "Source moved to trash/spam or is a draft"
    );
    anyhow::ensure!(
        matches!(input.category, AutoArchiveCategory::ExplicitUserRequest)
            || !message.labels.iter().any(|l| l == "STARRED"),
        "Starred messages require explicit user instruction"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn decision() -> AutoArchiveRequest {
        AutoArchiveRequest {
            category: AutoArchiveCategory::BulkMail,
            reason: "Routine newsletter with no outstanding work".into(),
            reviewed_at: Utc::now(),
            thread_message_ids: vec!["one".into()],
            task_ids: vec![],
        }
    }
    #[test]
    fn evidence_is_fresh_bounded_and_task_backed() {
        let mut d = decision();
        assert!(validate(&d).is_ok());
        d.category = AutoArchiveCategory::TaskCaptured;
        assert!(validate(&d).is_err());
        d.task_ids.push(Uuid::new_v4());
        assert!(validate(&d).is_ok());
        d.reviewed_at = Utc::now() - Duration::minutes(16);
        assert!(validate(&d).is_err());
        d.reviewed_at = Utc::now() + Duration::minutes(1);
        assert!(validate(&d).is_err());
        d.reviewed_at = Utc::now();
        d.thread_message_ids.push("one".into());
        assert!(validate(&d).is_err());
    }
    #[test]
    fn changed_threads_and_starred_messages_are_protected() {
        let mut m = EmailMessage {
            id: "one".into(),
            thread_id: "thread".into(),
            subject: String::new(),
            from: String::new(),
            to: vec![],
            cc: vec![],
            snippet: String::new(),
            body_text: None,
            body_html: None,
            received_at: None,
            history_id: None,
            labels: vec!["INBOX".into()],
            has_attachments: false,
        };
        let mut d = decision();
        assert!(check_thread(&d, "one", &[m.clone()]).is_ok());
        d.thread_message_ids.push("new reply".into());
        assert!(check_thread(&d, "one", &[m.clone()]).is_err());
        d.thread_message_ids.pop();
        m.labels.push("STARRED".into());
        assert!(check_thread(&d, "one", &[m.clone()]).is_err());
        d.category = AutoArchiveCategory::ExplicitUserRequest;
        assert!(check_thread(&d, "one", &[m.clone()]).is_ok());
        m.labels.push("TRASH".into());
        assert!(check_thread(&d, "one", &[m]).is_err());
    }
}
