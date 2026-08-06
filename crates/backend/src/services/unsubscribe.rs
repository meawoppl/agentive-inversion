//! Standardized unsubscribe (RFC 2369 List-Unsubscribe, RFC 8058 one-click).
//!
//! Headers are fetched live from Gmail per attempt, so this works for every
//! stored email with no schema support. Preference order: one-click HTTPS
//! POST (built for automation) > mailto (sent from the owning account) >
//! plain HTTPS link (returned for a human to open). Anything else is "none".

use anyhow::{Context, Result};
use shared_types::UnsubscribeResponse;
use uuid::Uuid;

use crate::db::{self, DbPool};
use crate::pollers::gmail_client::GmailClient;

/// Entries inside a List-Unsubscribe header: `<mailto:...>, <https://...>`
fn parse_entries(header: &str) -> Vec<String> {
    header
        .split(',')
        .filter_map(|part| {
            let part = part.trim();
            let start = part.find('<')? + 1;
            let end = part.find('>')?;
            (start < end).then(|| part[start..end].trim().to_string())
        })
        .collect()
}

fn https_entry(entries: &[String]) -> Option<&String> {
    entries.iter().find(|e| e.starts_with("https://"))
}

fn mailto_entry(entries: &[String]) -> Option<&String> {
    entries.iter().find(|e| e.starts_with("mailto:"))
}

/// RFC 8058: the header must contain exactly `List-Unsubscribe=One-Click`
fn is_one_click(post_header: Option<&str>) -> bool {
    post_header
        .map(|v| v.trim().eq_ignore_ascii_case("List-Unsubscribe=One-Click"))
        .unwrap_or(false)
}

/// Refuse obviously non-public targets before the server POSTs anywhere
/// (light SSRF guard: https only, named host, no userinfo, no port games)
fn url_is_sane(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    !host.is_empty()
        && !host.contains('@')
        && !host.contains(':')
        && host.contains('.')
        && !host.chars().all(|c| c.is_ascii_digit() || c == '.')
}

pub struct UnsubscribeService;

impl UnsubscribeService {
    pub async fn unsubscribe(pool: &DbPool, email_id: Uuid) -> Result<UnsubscribeResponse> {
        let mut conn = pool.get().await.context("Failed to get DB connection")?;
        let email = db::emails::get_by_id(&mut conn, email_id)
            .await
            .context("Email not found")?;
        let account = db::google_accounts::get_by_id(&mut conn, email.account_id).await?;
        drop(conn);

        let client = GmailClient::from_account(&account).await?;
        let (list_unsub, list_unsub_post) = client.get_unsubscribe_headers(&email.gmail_id).await?;

        let Some(header) = list_unsub else {
            return Ok(UnsubscribeResponse {
                method: "none".to_string(),
                url: None,
            });
        };
        let entries = parse_entries(&header);

        // One-click: a plain POST with the fixed form body completes the
        // unsubscribe with no human involved — that is its entire contract
        if is_one_click(list_unsub_post.as_deref()) {
            if let Some(url) = https_entry(&entries).filter(|u| url_is_sane(u)) {
                let resp = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(15))
                    .build()?
                    .post(url)
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .body("List-Unsubscribe=One-Click")
                    .send()
                    .await
                    .context("One-click unsubscribe POST failed")?;
                if resp.status().is_success() || resp.status().is_redirection() {
                    tracing::info!(
                        "One-click unsubscribed {} from {}",
                        account.email,
                        email.from_address
                    );
                    return Ok(UnsubscribeResponse {
                        method: "one_click".to_string(),
                        url: None,
                    });
                }
                tracing::warn!(
                    "One-click unsubscribe returned {} for {}; falling back",
                    resp.status(),
                    email.from_address
                );
            }
        }

        // Mailto: send the request from the account the mail landed in
        if let Some(mailto) = mailto_entry(&entries) {
            let rest = &mailto["mailto:".len()..];
            let (to, query) = rest.split_once('?').unwrap_or((rest, ""));
            let subject = query
                .split('&')
                .find_map(|kv| kv.strip_prefix("subject="))
                .map(|s| s.replace('+', " "))
                .unwrap_or_else(|| "unsubscribe".to_string());
            client
                .send_plain_message(to, &account.email, &subject, "unsubscribe")
                .await
                .context("Unsubscribe mail failed to send")?;
            tracing::info!("Sent unsubscribe mail for {} to {}", email.from_address, to);
            return Ok(UnsubscribeResponse {
                method: "mailto".to_string(),
                url: None,
            });
        }

        // A page that needs a human: hand the link back
        if let Some(url) = https_entry(&entries) {
            return Ok(UnsubscribeResponse {
                method: "link".to_string(),
                url: Some(url.clone()),
            });
        }

        Ok(UnsubscribeResponse {
            method: "none".to_string(),
            url: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multi_entry_headers() {
        let entries = parse_entries(
            "<mailto:unsub@lists.example.com?subject=stop>, <https://example.com/u/1>",
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], "mailto:unsub@lists.example.com?subject=stop");
        assert_eq!(entries[1], "https://example.com/u/1");
        assert_eq!(mailto_entry(&entries), Some(&entries[0]));
        assert_eq!(https_entry(&entries), Some(&entries[1]));
    }

    #[test]
    fn one_click_requires_exact_token() {
        assert!(is_one_click(Some("List-Unsubscribe=One-Click")));
        assert!(is_one_click(Some(" list-unsubscribe=one-click ")));
        assert!(!is_one_click(Some("something-else")));
        assert!(!is_one_click(None));
    }

    #[test]
    fn url_sanity_rejects_non_public_shapes() {
        assert!(url_is_sane("https://example.com/unsub?u=1"));
        assert!(!url_is_sane("http://example.com/unsub"));
        assert!(!url_is_sane("https://10.0.0.1/unsub"));
        assert!(!url_is_sane("https://evil.com:8080/unsub"));
        assert!(!url_is_sane("https://user@evil.com/unsub"));
        assert!(!url_is_sane("https://localhost/unsub"));
    }
}
