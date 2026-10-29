//! Draft-only Gmail operations. This module never calls messages.send/drafts.send.
use super::gmail_client::GmailClient;
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use google_gmail1::api::{Draft, Message};
use mailparse::MailAddr;

pub fn header(message: &Message, name: &str) -> Result<String> {
    let values: Vec<_> = message
        .payload
        .as_ref()
        .and_then(|p| p.headers.as_ref())
        .into_iter()
        .flatten()
        .filter(|h| {
            h.name
                .as_deref()
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
        })
        .filter_map(|h| h.value.as_deref())
        .collect();
    if values.len() > 1 {
        bail!("Ambiguous duplicate {name} header");
    }
    let value = values.first().copied().unwrap_or_default();
    // Gmail normally unfolds these. Accept legitimate folds, never bare newlines.
    let value = value.replace("\r\n\t", " ").replace("\r\n ", " ");
    if value.chars().any(|c| c.is_control()) {
        bail!("Invalid {name} header");
    }
    Ok(value)
}

fn addresses(value: &str) -> Result<Vec<String>> {
    if value.trim().is_empty() {
        return Ok(vec![]);
    }
    let parsed = mailparse::addrparse(value).context("Malformed address header")?;
    let mut result = vec![];
    for entry in parsed.iter() {
        let singles = match entry {
            MailAddr::Single(a) => vec![a],
            MailAddr::Group(g) => g.addrs.iter().collect(),
        };
        for address in singles {
            let addr = &address.addr;
            if !addr.is_ascii()
                || !addr.contains('@')
                || addr
                    .chars()
                    .any(|c| c.is_whitespace() || "<>(),;:\"\\".contains(c))
            {
                bail!("Unsupported mailbox address");
            }
            result.push(addr.clone());
        }
    }
    Ok(result)
}

fn message_ids(value: &str) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    for item in value.split_whitespace() {
        if !item.starts_with('<')
            || !item.ends_with('>')
            || !item.contains('@')
            || item[1..item.len() - 1].contains(['<', '>'])
            || !item.is_ascii()
        {
            bail!("Invalid message threading header");
        }
        ids.push(item.to_string());
    }
    Ok(ids)
}

pub fn reply_mime(
    anchor: &Message,
    from: &str,
    own_addresses: &[String],
    body: &str,
    reply_all: bool,
    operation_id: uuid::Uuid,
) -> Result<Vec<u8>> {
    let own = |a: &str| own_addresses.iter().any(|s| s.eq_ignore_ascii_case(a));
    let from = addresses(from)?;
    if from.len() != 1 {
        bail!("Expected one sending account");
    }
    let sender = addresses(&header(anchor, "From")?)?;
    if sender.is_empty() {
        bail!("Reply target has no sender");
    }
    let sent_by_self = sender.iter().all(|a| own(a));
    let reply_to = header(anchor, "Reply-To")?;
    let mut to = if sent_by_self {
        addresses(&header(anchor, "To")?)?
    } else if reply_to.is_empty() {
        sender
    } else {
        addresses(&reply_to)?
    };
    if reply_all && !sent_by_self {
        to.extend(addresses(&header(anchor, "To")?)?);
    }
    let mut cc = if reply_all {
        addresses(&header(anchor, "Cc")?)?
    } else {
        vec![]
    };
    let mut seen = std::collections::HashSet::new();
    to.retain(|a| !own(a) && seen.insert(a.to_ascii_lowercase()));
    cc.retain(|a| !own(a) && seen.insert(a.to_ascii_lowercase()));
    if to.is_empty() && !cc.is_empty() {
        to.push(cc.remove(0));
    }
    if to.is_empty() {
        bail!("Reply has no external recipient");
    }
    let parent = message_ids(&header(anchor, "Message-ID")?)?;
    if parent.len() != 1 {
        bail!("Reply target needs one RFC Message-ID");
    }
    let mut refs = message_ids(&header(anchor, "References")?)?;
    if refs.is_empty() {
        refs = message_ids(&header(anchor, "In-Reply-To")?)?;
    }
    if refs.last() != parent.first() {
        refs.extend(parent.clone());
    }
    let subject_header = format!("Subject: {}", header(anchor, "Subject")?);
    let (decoded, _) = mailparse::parse_header(subject_header.as_bytes())?;
    let subject = decoded.get_value();
    // RFC 2047 chunks are UTF-8 boundary safe and stay below 75 characters.
    let mut chunks = Vec::new();
    let mut chunk = String::new();
    for ch in subject.chars() {
        if chunk.len() + ch.len_utf8() > 42 {
            chunks.push(std::mem::take(&mut chunk));
        }
        chunk.push(ch);
    }
    if !chunk.is_empty() {
        chunks.push(chunk);
    }
    let subject = chunks
        .iter()
        .map(|s| format!("=?UTF-8?B?{}?=", STANDARD.encode(s)))
        .collect::<Vec<_>>()
        .join("\r\n ");
    let encoded = STANDARD.encode(
        body.replace("\r\n", "\n")
            .replace('\r', "\n")
            .replace('\n', "\r\n"),
    );
    let encoded = encoded
        .as_bytes()
        .chunks(76)
        .map(|s| std::str::from_utf8(s).unwrap())
        .collect::<Vec<_>>()
        .join("\r\n");
    Ok(format!("From: {}\r\nTo: {}\r\nCc: {}\r\nSubject: {}\r\nMessage-ID: <agentive-{operation_id}@drafts.agentive.invalid>\r\nIn-Reply-To: {}\r\nReferences: {}\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\n{}\r\n", from[0], to.join(", "), cc.join(", "), subject, parent[0], refs.join("\r\n "), encoded).into_bytes())
}

impl GmailClient {
    pub async fn draft_reply_context(
        &self,
        thread_id: &str,
        target: Option<&str>,
    ) -> Result<(Message, Vec<String>)> {
        let (_, thread) = self
            .hub
            .users()
            .threads_get("me", thread_id)
            .add_scope(google_gmail1::api::Scope::Modify)
            .format("full")
            .doit()
            .await?;
        let mut messages = thread.messages.unwrap_or_default();
        messages.retain(|m| {
            !m.label_ids
                .as_ref()
                .is_some_and(|l| l.iter().any(|s| s == "DRAFT"))
        });
        messages.sort_by_key(|m| m.internal_date);
        let anchor = if let Some(id) = target {
            messages.into_iter().find(|m| m.id.as_deref() == Some(id))
        } else {
            messages.pop()
        }
        .context("Reply target is absent from live thread or is a draft")?;
        let (_, aliases) = self
            .hub
            .users()
            .settings_send_as_list("me")
            .add_scope(google_gmail1::api::Scope::Modify)
            .doit()
            .await?;
        let mut own = vec![self.email_address.clone()];
        own.extend(
            aliases
                .send_as
                .unwrap_or_default()
                .into_iter()
                .filter_map(|a| a.send_as_email),
        );
        Ok((anchor, own))
    }

    pub async fn create_reply_draft(&self, thread_id: &str, mime: Vec<u8>) -> Result<Draft> {
        let request = Draft {
            message: Some(Message {
                thread_id: Some(thread_id.into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (_, draft) = self
            .hub
            .users()
            .drafts_create(request, "me")
            .add_scope(google_gmail1::api::Scope::Modify)
            .upload(
                std::io::Cursor::new(mime),
                "message/rfc822".parse().unwrap(),
            )
            .await?;
        Ok(draft)
    }

    pub async fn read_draft(&self, id: &str) -> Result<Draft> {
        let (_, draft) = self
            .hub
            .users()
            .drafts_get("me", id)
            .add_scope(google_gmail1::api::Scope::Modify)
            .format("full")
            .doit()
            .await?;
        Ok(draft)
    }

    /// Read-only recovery; an inconclusive search must never trigger another create.
    pub async fn find_reply_draft(
        &self,
        operation_id: uuid::Uuid,
        thread_id: &str,
    ) -> Result<Option<Draft>> {
        let marker = format!("<agentive-{operation_id}@drafts.agentive.invalid>");
        let (_, found) = self
            .hub
            .users()
            .drafts_list("me")
            .add_scope(google_gmail1::api::Scope::Modify)
            .q(&format!("rfc822msgid:{marker}"))
            .max_results(10)
            .doit()
            .await?;
        for draft in found.drafts.unwrap_or_default() {
            if let Some(id) = draft.id {
                let live = self.read_draft(&id).await?;
                if let Some(m) = live.message.as_ref() {
                    if m.thread_id.as_deref() == Some(thread_id)
                        && header(m, "Message-ID")? == marker
                    {
                        return Ok(Some(live));
                    }
                }
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use google_gmail1::api::{MessagePart, MessagePartHeader};
    use mailparse::MailHeaderMap;
    fn message(headers: &[(&str, &str)]) -> Message {
        Message {
            payload: Some(MessagePart {
                headers: Some(
                    headers
                        .iter()
                        .map(|(n, v)| MessagePartHeader {
                            name: Some(n.to_string()),
                            value: Some(v.to_string()),
                        })
                        .collect(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        }
    }
    #[tokio::test]
    async fn provider_upload_contains_thread_metadata_and_never_sends() {
        use axum::{
            body::Bytes,
            http::{Method, Uri},
            routing::any,
            Json, Router,
        };
        use hyper_util::{client::legacy::Client, rt::TokioExecutor};
        use std::sync::{Arc, Mutex};
        let captured = Arc::new(Mutex::new(Vec::new()));
        let calls = captured.clone();
        let app = Router::new().fallback(any(move |method: Method, uri: Uri, body: Bytes| {
            let calls = calls.clone();
            async move {
                calls.lock().unwrap().push((method.to_string(), uri.path().to_owned(), String::from_utf8_lossy(&body).to_string()));
                Json(serde_json::json!({"id":"draft1","message":{"id":"msg1","threadId":"thread123","labelIds":["DRAFT"]}}))
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}/", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let connector = google_gmail1::hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .unwrap()
            .https_or_http()
            .enable_http1()
            .build();
        let mut hub = google_gmail1::Gmail::new(
            Client::builder(TokioExecutor::new()).build(connector),
            google_gmail1::common::NoToken,
        );
        hub.root_url(origin.clone());
        hub.base_url(origin);
        let client = GmailClient {
            hub,
            email_address: "me@example.com".into(),
        };
        let mime = b"From: me@example.com\r\nTo: peer@example.com\r\nIn-Reply-To: <p@example.com>\r\n\r\nhello".to_vec();
        let created = client.create_reply_draft("thread123", mime).await.unwrap();
        assert_eq!(created.id.as_deref(), Some("draft1"));
        client.read_draft("draft1").await.unwrap();
        let calls = captured.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "POST");
        assert!(calls[0].1.ends_with("/users/me/drafts"));
        assert!(
            calls[0].2.contains("\"threadId\":\"thread123\""),
            "{}",
            calls[0].2
        );
        assert!(calls[0].2.contains("In-Reply-To: <p@example.com>"));
        assert_eq!(calls[1].0, "GET");
        assert!(!calls.iter().any(|(_, path, _)| path.contains("send")));
        server.abort();
    }
    #[test]
    fn reply_all_honors_reply_to_aliases_and_quoted_names() {
        let m = message(&[
            ("From", "Vendor <notify@example.com>"),
            ("Reply-To", "\"Doe, Jane\" <jane@example.com>"),
            ("To", "Me <me@example.com>, peer@example.com"),
            ("Cc", "alias@example.com, PEER@example.com, cc@example.com"),
            ("Message-ID", "<parent@example.com>"),
            ("References", "<first@example.com>"),
            ("Subject", "Résumé 🛰"),
        ]);
        let raw = reply_mime(
            &m,
            "me@example.com",
            &["me@example.com".into(), "alias@example.com".into()],
            "Works!\nThanks",
            true,
            uuid::Uuid::nil(),
        )
        .unwrap();
        let parsed = mailparse::parse_mail(&raw).unwrap();
        assert_eq!(
            parsed.headers.get_first_value("To").unwrap(),
            "jane@example.com, peer@example.com"
        );
        assert_eq!(
            parsed.headers.get_first_value("Cc").unwrap(),
            "cc@example.com"
        );
        assert_eq!(
            parsed.headers.get_first_value("Subject").unwrap(),
            "Résumé 🛰"
        );
        assert_eq!(
            parsed.headers.get_first_value("In-Reply-To").unwrap(),
            "<parent@example.com>"
        );
        assert!(parsed
            .headers
            .get_first_value("References")
            .unwrap()
            .contains("<first@example.com>"));
        assert_eq!(parsed.get_body().unwrap(), "Works!\r\nThanks");
        let single = reply_mime(
            &m,
            "me@example.com",
            &["me@example.com".into()],
            "ok",
            false,
            uuid::Uuid::nil(),
        )
        .unwrap();
        assert_eq!(
            mailparse::parse_mail(&single)
                .unwrap()
                .headers
                .get_first_value("To")
                .unwrap(),
            "jane@example.com"
        );
    }
    #[test]
    fn follow_up_to_own_sent_message_targets_original_recipients() {
        let m = message(&[
            ("From", "me@example.com"),
            ("To", "peer@example.com"),
            ("Message-ID", "<sent@example.com>"),
        ]);
        let raw = reply_mime(
            &m,
            "me@example.com",
            &["me@example.com".into()],
            "follow up",
            false,
            uuid::Uuid::nil(),
        )
        .unwrap();
        assert_eq!(
            mailparse::parse_mail(&raw)
                .unwrap()
                .headers
                .get_first_value("To")
                .unwrap(),
            "peer@example.com"
        );
    }
    #[test]
    fn malformed_and_injected_headers_fail_closed() {
        for h in [
            vec![
                ("From", "peer@example.com"),
                ("Subject", "Hi\r\nBcc: victim@example.com"),
                ("Message-ID", "<p@example.com>"),
            ],
            vec![
                ("From", "peer@example.com"),
                ("Message-ID", "missing-brackets"),
            ],
            vec![("From", "peer@example.com")],
        ] {
            assert!(reply_mime(
                &message(&h),
                "me@example.com",
                &["me@example.com".into()],
                "test",
                false,
                uuid::Uuid::nil()
            )
            .is_err());
        }
    }
}
