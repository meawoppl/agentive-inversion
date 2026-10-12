use super::{
    auth::Principal,
    models::{ActionRow, ConnectionRow, IdRow},
    work::validate_reference,
};
use crate::{
    calendar_client::{CalendarClient, NewCalendarEvent},
    db,
    error::{ApiError, ApiResult},
    pollers::gmail_client::GmailClient,
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Extension, Json,
};
use chrono::Utc;
use diesel::{
    sql_query,
    sql_types::{Array, Nullable, Text, Uuid as SqlUuid},
    OptionalExtension,
};
use diesel_async::{AsyncConnection, RunQueryDsl};
use serde::Deserialize;
use shared_types::connector::*;
use uuid::Uuid;

pub async fn submit(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    headers: HeaderMap,
    Json(input): Json<ActionRequest>,
) -> ApiResult<Json<ActionInfo>> {
    p.scope("actions:propose")?;
    let cid = p.connection()?;
    validate_reference(&input.repository_commit, &input.repository_path)?;
    if input.reason.trim().is_empty() || input.reason.len() > 8000 {
        return Err(ApiError::bad_request("A bounded reason is required"));
    }
    if let Effect::CreateCalendarEvent {
        summary,
        start,
        end,
        ..
    } = &input.effect
    {
        if summary.trim().is_empty() || summary.len() > 500 || end <= start {
            return Err(ApiError::bad_request(
                "Event needs a summary and future start/end",
            ));
        }
    }
    let key = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty() && s.len() <= 200)
        .ok_or_else(|| ApiError::bad_request("Idempotency-Key header required (1–200 bytes)"))?
        .to_string();
    let json = serde_json::to_string(&input)?;
    let mut c = state.pool.get().await?;
    let row=c.transaction::<ActionRow,ApiError,_>(async move |c| {
  // Serialize a connection's submissions to make key replay and semantic
  // duplicate handling deterministic even under simultaneous requests.
  sql_query("SELECT id FROM connector_connections WHERE id=$1 AND revoked_at IS NULL AND expires_at>now() FOR UPDATE").bind::<SqlUuid,_>(cid).get_result::<IdRow>(&mut *c).await?;
  if let Some(row)=sql_query("SELECT * FROM connector_actions WHERE connection_id=$1 AND idempotency_key=$2").bind::<SqlUuid,_>(cid).bind::<Text,_>(&key).get_result::<ActionRow>(&mut *c).await.optional()?{
   if row.request_json!=json{return Err(ApiError::Conflict("Idempotency key reused for a different request".into()));}return Ok(row);
  }
  if matches!(&input.effect, Effect::CreateCalendarEvent { start, .. } if *start <= Utc::now()) {
      return Err(ApiError::bad_request("Event start must be in the future"));
  }
  let valid=sql_query("SELECT e.id FROM connector_claims w JOIN emails e ON e.id=w.email_id WHERE w.email_id=$1 AND w.claim_id=$2 AND w.connection_id=$3 AND w.expires_at>now() AND w.completed_at IS NULL AND e.account_id=ANY($4) FOR UPDATE OF w")
   .bind::<SqlUuid,_>(input.email_id).bind::<SqlUuid,_>(input.claim_id).bind::<SqlUuid,_>(cid).bind::<Array<SqlUuid>,_>(&p.account_ids).get_result::<IdRow>(&mut *c).await.optional()?;
  if valid.is_none(){return Err(ApiError::Conflict("Current, incomplete work claim required".into()));}
  let destination=if matches!(input.effect,Effect::ForwardReceipt){let v=crate::services::triage::forward_to_address();if v.contains(['\r','\n'])||!v.contains('@'){return Err(ApiError::Config("Invalid receipt destination".into()));}Some(v)}else{None};
  let inserted=sql_query("INSERT INTO connector_actions(id,connection_id,email_id,idempotency_key,request_json,kind,status,forward_to) VALUES($1,$2,$3,$4,$5,$6,'proposed',$7) ON CONFLICT DO NOTHING RETURNING *")
   .bind::<SqlUuid,_>(Uuid::new_v4()).bind::<SqlUuid,_>(cid).bind::<SqlUuid,_>(input.email_id).bind::<Text,_>(key).bind::<Text,_>(json).bind::<Text,_>(input.effect.kind()).bind::<Nullable<Text>,_>(destination).get_result::<ActionRow>(&mut *c).await.optional()?;
  inserted.ok_or_else(||ApiError::Conflict("An action of this kind already exists for this email; inspect its receipt".into()))
 }).await?;
    Ok(Json(row.try_into()?))
}
#[derive(Deserialize)]
pub struct ActionQuery {
    pub status: Option<String>,
    pub cursor: Option<Uuid>,
}
pub async fn list(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    Query(q): Query<ActionQuery>,
) -> ApiResult<Json<Vec<ActionInfo>>> {
    p.scope("actions:propose")?;
    let mut c = state.pool.get().await?;
    let rows=sql_query("SELECT x.* FROM connector_actions x JOIN emails e ON e.id=x.email_id WHERE e.account_id=ANY($1) AND ($2::uuid IS NULL OR x.connection_id=$2) AND ($3::text IS NULL OR x.status=$3) AND ($4::uuid IS NULL OR x.id>$4) ORDER BY x.id LIMIT 100")
 .bind::<Array<SqlUuid>,_>(&p.account_ids).bind::<Nullable<SqlUuid>,_>(if p.scopes.iter().any(|s|s=="actions:approve"){None}else{p.connection_id}).bind::<Nullable<Text>,_>(q.status).bind::<Nullable<SqlUuid>,_>(q.cursor).load::<ActionRow>(&mut c).await?;
    Ok(Json(
        rows.into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?,
    ))
}
pub async fn get(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ActionInfo>> {
    p.scope("actions:propose")?;
    let mut c = state.pool.get().await?;
    let row=sql_query("SELECT x.* FROM connector_actions x JOIN emails e ON e.id=x.email_id WHERE x.id=$1 AND e.account_id=ANY($2) AND ($3::uuid IS NULL OR x.connection_id=$3)").bind::<SqlUuid,_>(id).bind::<Array<SqlUuid>,_>(&p.account_ids).bind::<Nullable<SqlUuid>,_>(if p.scopes.iter().any(|s|s=="actions:approve"){None}else{p.connection_id}).get_result::<ActionRow>(&mut c).await?;
    Ok(Json(row.try_into()?))
}
pub async fn approve(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    p.scope("actions:approve")?;
    p.connection()?;
    let mut c = state.pool.get().await?;
    let n=sql_query("UPDATE connector_actions a SET status='queued',approved_by=$2,approved_connection_id=$3,updated_at=now() FROM connector_connections c WHERE a.id=$1 AND a.status='proposed' AND a.connection_id=c.id AND EXISTS (SELECT 1 FROM emails e WHERE e.id=a.email_id AND e.account_id=ANY($4)) AND c.revoked_at IS NULL AND c.expires_at>now()")
 .bind::<SqlUuid,_>(id).bind::<Text,_>(p.email).bind::<Nullable<SqlUuid>,_>(p.connection_id).bind::<Array<SqlUuid>,_>(&p.account_ids).execute(&mut c).await?;
    if n == 0 {
        return Err(ApiError::Conflict(
            "Action is not pending or its connection was revoked".into(),
        ));
    }
    Ok(StatusCode::ACCEPTED)
}
pub async fn reject(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    p.scope("actions:approve")?;
    p.connection()?;
    let mut c = state.pool.get().await?;
    let n=sql_query("UPDATE connector_actions SET status='rejected',approved_by=$2,updated_at=now() WHERE id=$1 AND status='proposed' AND email_id IN (SELECT id FROM emails WHERE account_id=ANY($3))").bind::<SqlUuid,_>(id).bind::<Text,_>(p.email).bind::<Array<SqlUuid>,_>(&p.account_ids).execute(&mut c).await?;
    if n == 0 {
        return Err(ApiError::Conflict(
            "Only pending proposals can be rejected".into(),
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}
pub async fn retry(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    p.scope("actions:approve")?;
    p.connection()?;
    let mut c = state.pool.get().await?;
    let n=sql_query("UPDATE connector_actions SET status='queued',approved_by=$2,approved_connection_id=$3,error=NULL,updated_at=now() WHERE id=$1 AND status='unknown' AND (kind IN ('archive','report_spam','create_calendar_event') OR (kind='forward_receipt' AND result_json IS NOT NULL AND (result_json::jsonb->>'forwarded')='true' AND result_json::jsonb->>'provider_id' IS NOT NULL)) AND email_id IN (SELECT id FROM emails WHERE account_id=ANY($4))").bind::<SqlUuid,_>(id).bind::<Text,_>(p.email).bind::<Nullable<SqlUuid>,_>(p.connection_id).bind::<Array<SqlUuid>,_>(&p.account_ids).execute(&mut c).await?;
    if n == 0 {
        return Err(ApiError::Conflict("Uncertain sends cannot be retried. Only archives, stable-ID calendar writes, and archiving an already-recorded forward are retryable".into()));
    }
    Ok(StatusCode::ACCEPTED)
}

pub async fn run(state: AppState) {
    loop {
        if let Err(e) = tick(&state).await {
            tracing::error!("Connector action worker: {e:#}");
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}
pub(super) async fn tick(state: &AppState) -> anyhow::Result<()> {
    let mut c = state.pool.get().await?;
    sql_query("UPDATE connector_actions a SET status='cancelled',error='Proposing connection expired or revoked',updated_at=now() WHERE status IN ('proposed','queued') AND NOT EXISTS (SELECT 1 FROM connector_connections c WHERE c.id=a.connection_id AND c.revoked_at IS NULL AND c.expires_at>now())").execute(&mut c).await?;
    sql_query("UPDATE connector_actions SET status='unknown',error='Worker interrupted; inspect provider before any retry',updated_at=now() WHERE status='running' AND updated_at<now()-interval '10 minutes'").execute(&mut c).await?;
    let row=sql_query("UPDATE connector_actions SET status='running',updated_at=now() WHERE id=(SELECT id FROM connector_actions WHERE status='queued' ORDER BY created_at LIMIT 1 FOR UPDATE SKIP LOCKED) RETURNING *").get_result::<ActionRow>(&mut c).await.optional()?;
    let Some(row) = row else { return Ok(()) };
    let grant=sql_query("SELECT * FROM connector_connections WHERE id=$1 AND revoked_at IS NULL AND expires_at>now()").bind::<SqlUuid,_>(row.connection_id).get_result::<ConnectionRow>(&mut c).await.optional()?;
    let approval_live=sql_query("SELECT a.id FROM connector_actions a LEFT JOIN connector_connections c ON c.id=a.approved_connection_id WHERE a.id=$1 AND c.revoked_at IS NULL AND c.expires_at>now() AND 'actions:approve'=ANY(c.scopes)").bind::<SqlUuid,_>(row.id).get_result::<IdRow>(&mut c).await.optional()?.is_some();
    let authorized = approval_live
        && grant
            .as_ref()
            .is_some_and(|g| state.auth_config.is_email_allowed(&g.owner_email))
        && row
            .approved_by
            .as_ref()
            .is_some_and(|e| state.auth_config.is_email_allowed(e));
    if !authorized {
        sql_query("UPDATE connector_actions SET status='cancelled',error='Execution authority revoked',updated_at=now() WHERE id=$1").bind::<SqlUuid,_>(row.id).execute(&mut c).await?;
        return Ok(());
    }
    drop(c);
    let result =
        tokio::time::timeout(std::time::Duration::from_secs(120), execute(state, &row)).await;
    let mut c = state.pool.get().await?;
    match result {
        Ok(Ok(receipt)) => {
            sql_query("UPDATE connector_actions SET status='succeeded',result_json=$2,error=NULL,updated_at=now() WHERE id=$1").bind::<SqlUuid,_>(row.id).bind::<Text,_>(serde_json::to_string(&receipt)?).execute(&mut c).await?;
        }
        error => {
            tracing::warn!(action_id=%row.id, "Provider outcome uncertain: {error:?}");
            sql_query("UPDATE connector_actions SET status='unknown',error='Provider outcome uncertain; inspect provider and recorded partial receipt. Do not repeat sends.',updated_at=now() WHERE id=$1").bind::<SqlUuid,_>(row.id).execute(&mut c).await?;
        }
    }
    Ok(())
}
async fn execute(state: &AppState, row: &ActionRow) -> anyhow::Result<Receipt> {
    let request: ActionRequest = serde_json::from_str(&row.request_json)?;
    let mut c = state.pool.get().await?;
    let email = db::emails::get_by_id(&mut c, row.email_id).await?;
    let account = db::google_accounts::get_by_id(&mut c, email.account_id).await?;
    drop(c);
    let mut receipt: Receipt = row
        .result_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()?
        .unwrap_or(Receipt {
            reused_existing: false,
            provider_id: None,
            calendar_id: None,
            forwarded: false,
            archived: false,
            reported_spam: false,
        });
    match request.effect {
        Effect::ReportSpam => {
            let gmail = GmailClient::from_account(&account).await?;
            gmail.report_spam(&email.gmail_id).await?;
            receipt.reported_spam = true;
            receipt.provider_id = Some(email.gmail_id.clone());
            let mut c = state.pool.get().await?;
            // This legacy field tracks removal from the inbox, including Spam.
            sql_query("UPDATE emails SET archived_in_gmail=true WHERE id=$1")
                .bind::<SqlUuid, _>(email.id)
                .execute(&mut c)
                .await?;
        }
        Effect::Archive | Effect::ForwardReceipt => {
            let gmail = GmailClient::from_account(&account).await?;
            if matches!(request.effect, Effect::ForwardReceipt) && !receipt.forwarded {
                let to = row
                    .forward_to
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("Missing approved destination"))?;
                let sent_id = gmail
                    .forward_with_receipt(&email.gmail_id, to, &account.email, &email.subject)
                    .await?;
                receipt.forwarded = true;
                receipt.provider_id = Some(sent_id);
                // Persist the irreversible step before attempting the independently
                // retryable archive. An uncertain send is never automatically repeated.
                let mut c = state.pool.get().await?;
                sql_query(
                    "UPDATE connector_actions SET result_json=$2,updated_at=now() WHERE id=$1",
                )
                .bind::<SqlUuid, _>(row.id)
                .bind::<Text, _>(serde_json::to_string(&receipt)?)
                .execute(&mut c)
                .await?;
            }
            let label = gmail
                .ensure_label(if receipt.forwarded {
                    "agent-forwarded"
                } else {
                    "agent-archived"
                })
                .await?;
            gmail.archive_with_label(&email.gmail_id, &label).await?;
            receipt.archived = true;
            let mut c = state.pool.get().await?;
            sql_query("UPDATE emails SET archived_in_gmail=true WHERE id=$1")
                .bind::<SqlUuid, _>(email.id)
                .execute(&mut c)
                .await?;
        }
        Effect::CreateCalendarEvent {
            summary,
            description,
            location,
            start,
            end,
        } => {
            let client = CalendarClient::from_account(&account).await?;
            let link = format!(
                "https://mail.google.com/mail/u/{}/#all/{}",
                urlencoding::encode(&account.email),
                email.gmail_id
            );
            let candidate = crate::services::calendar_dedupe::CandidateEvent {
                summary: summary.clone(),
                start,
                end,
                source_link: Some(link.clone()),
            };
            let from = start.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc()
                - chrono::Duration::hours(2);
            let to = from + chrono::Duration::hours(28);
            let mut existing = vec![];
            for calendar_id in client.list_calendar_ids().await? {
                existing.extend(client.list_events_between(&calendar_id, from, to).await?);
            }
            if let Some(found) =
                crate::services::calendar_dedupe::find_duplicate(&candidate, &existing)
            {
                receipt.provider_id = Some(found.existing.event_id);
                receipt.calendar_id = Some(found.existing.calendar_id);
                receipt.reused_existing = true;
                return Ok(receipt);
            }
            anyhow::ensure!(start > Utc::now(), "Event start has passed since approval");
            let calendar = client.ensure_calendar("Agent").await?;
            let created = client
                .create_event_with_id(
                    &calendar,
                    NewCalendarEvent {
                        summary,
                        description,
                        location,
                        start,
                        end,
                        email_link: Some(format!(
                            "https://mail.google.com/mail/u/{}/#all/{}",
                            urlencoding::encode(&account.email),
                            email.gmail_id
                        )),
                    },
                    Some(row.id.simple().to_string()),
                )
                .await?;
            receipt.provider_id = Some(created.google_event_id);
            receipt.calendar_id = Some(calendar);
        }
    }
    Ok(receipt)
}
