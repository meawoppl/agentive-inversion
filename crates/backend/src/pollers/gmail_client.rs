//! Gmail API client for fetching and managing emails.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use google_gmail1::api::Message;
use google_gmail1::hyper_rustls::HttpsConnector;
use google_gmail1::Gmail;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use shared_types::GoogleAccount;

/// Client for interacting with Gmail API
#[allow(dead_code)]
pub struct GmailClient {
    hub: Gmail<HttpsConnector<HttpConnector>>,
    pub email_address: String,
}

/// Email message fetched from Gmail
#[derive(Debug, Clone)]
pub struct EmailMessage {
    pub id: String,
    pub thread_id: String,
    pub subject: String,
    pub from: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub snippet: String,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub received_at: Option<DateTime<Utc>>,
    pub history_id: Option<u64>,
    pub labels: Vec<String>,
    pub has_attachments: bool,
}

impl GmailClient {
    /// Create a new Gmail client from a GoogleAccount with stored OAuth tokens
    pub async fn from_account(account: &GoogleAccount) -> Result<Self> {
        let refresh_token = &account.refresh_token;

        let client_id = std::env::var("GOOGLE_CLIENT_ID")
            .context("GOOGLE_CLIENT_ID environment variable must be set")?;
        let client_secret = std::env::var("GOOGLE_CLIENT_SECRET")
            .context("GOOGLE_CLIENT_SECRET environment variable must be set")?;

        // Build AuthorizedUserSecret with our stored refresh token
        // Use the yup_oauth2 re-exported by google_gmail1 to avoid version mismatch
        let secret = google_gmail1::yup_oauth2::authorized_user::AuthorizedUserSecret {
            client_id,
            client_secret,
            refresh_token: refresh_token.clone(),
            key_type: "authorized_user".to_string(),
        };

        // Create authenticator using authorized user flow
        let auth = google_gmail1::yup_oauth2::AuthorizedUserAuthenticator::builder(secret)
            .build()
            .await
            .context("Failed to build authenticator from refresh token")?;

        let connector = google_gmail1::hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .context("Failed to load native TLS roots")?
            .https_or_http()
            .enable_http1()
            .build();

        let client = Client::builder(TokioExecutor::new()).build(connector);
        let hub = Gmail::new(client, auth);

        Ok(Self {
            hub,
            email_address: account.email.clone(),
        })
    }

    /// Fetch recent messages from inbox
    pub async fn fetch_messages(&self, max_results: u32) -> Result<Vec<EmailMessage>> {
        let (_, list_response) = self
            .hub
            .users()
            .messages_list("me")
            .add_label_ids("INBOX")
            .max_results(max_results)
            .doit()
            .await
            .context("Failed to list messages")?;

        let messages = list_response.messages.unwrap_or_default();
        let mut emails = Vec::new();

        for msg in messages {
            if let Some(id) = msg.id {
                match self.get_message(&id).await {
                    Ok(email) => emails.push(email),
                    Err(e) => {
                        tracing::warn!("Failed to fetch message {}: {}", id, e);
                    }
                }
            }
        }

        Ok(emails)
    }

    /// Fetch messages since a history ID (incremental sync)
    pub async fn fetch_messages_since(
        &self,
        history_id: u64,
        max_results: u32,
    ) -> Result<Vec<EmailMessage>> {
        let (_, history_response) = self
            .hub
            .users()
            .history_list("me")
            .start_history_id(history_id)
            .label_id("INBOX")
            .add_history_types("messageAdded")
            .max_results(max_results)
            .doit()
            .await
            .context("Failed to list history")?;

        let mut emails = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        if let Some(history) = history_response.history {
            for h in history {
                if let Some(messages_added) = h.messages_added {
                    for msg_added in messages_added {
                        if let Some(message) = msg_added.message {
                            if let Some(id) = message.id {
                                if seen_ids.insert(id.clone()) {
                                    match self.get_message(&id).await {
                                        Ok(email) => emails.push(email),
                                        Err(e) => {
                                            tracing::warn!("Failed to fetch message {}: {}", id, e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(emails)
    }

    /// Get the current history ID for the mailbox
    pub async fn get_history_id(&self) -> Result<u64> {
        let (_, profile) = self
            .hub
            .users()
            .get_profile("me")
            .doit()
            .await
            .context("Failed to get profile")?;

        profile.history_id.context("No history ID in profile")
    }

    /// Get full message details
    pub async fn get_message(&self, message_id: &str) -> Result<EmailMessage> {
        let (_, message) = self
            .hub
            .users()
            .messages_get("me", message_id)
            .format("full")
            .doit()
            .await
            .context("Failed to get message")?;

        Ok(Self::parse_message(message))
    }

    /// Archive a message (remove INBOX label)
    #[allow(dead_code)]
    pub async fn archive_message(&self, message_id: &str) -> Result<()> {
        let modify_request = google_gmail1::api::ModifyMessageRequest {
            remove_label_ids: Some(vec!["INBOX".to_string()]),
            add_label_ids: None,
        };

        self.hub
            .users()
            .messages_modify(modify_request, "me", message_id)
            .doit()
            .await
            .context("Failed to archive message")?;

        tracing::info!("Archived message: {}", message_id);
        Ok(())
    }

    fn parse_message(message: Message) -> EmailMessage {
        let id = message.id.clone().unwrap_or_default();
        let thread_id = message.thread_id.clone().unwrap_or_default();
        let snippet = message.snippet.clone().unwrap_or_default();
        let history_id = message.history_id;
        let labels = message.label_ids.clone().unwrap_or_default();

        let mut subject = String::new();
        let mut from = String::new();
        let mut to = Vec::new();
        let mut cc = Vec::new();
        let mut received_at = None;

        if let Some(payload) = &message.payload {
            if let Some(headers) = &payload.headers {
                for header in headers {
                    match header.name.as_deref() {
                        Some("Subject") => subject = header.value.clone().unwrap_or_default(),
                        Some("From") => from = header.value.clone().unwrap_or_default(),
                        Some("To") => {
                            if let Some(val) = &header.value {
                                to = Self::parse_address_list(val);
                            }
                        }
                        Some("Cc") => {
                            if let Some(val) = &header.value {
                                cc = Self::parse_address_list(val);
                            }
                        }
                        Some("Date") => {
                            if let Some(date_str) = &header.value {
                                received_at = Self::parse_date(date_str);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        let (body_text, body_html) = Self::extract_bodies(&message);
        let has_attachments = Self::detect_attachments(&message);

        EmailMessage {
            id,
            thread_id,
            subject,
            from,
            to,
            cc,
            snippet,
            body_text,
            body_html,
            received_at,
            history_id,
            labels,
            has_attachments,
        }
    }

    fn parse_date(date_str: &str) -> Option<DateTime<Utc>> {
        if let Ok(dt) = DateTime::parse_from_rfc2822(date_str) {
            return Some(dt.with_timezone(&Utc));
        }
        None
    }

    fn parse_address_list(header_value: &str) -> Vec<String> {
        header_value
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    fn extract_bodies(message: &Message) -> (Option<String>, Option<String>) {
        let payload = match message.payload.as_ref() {
            Some(p) => p,
            None => return (None, None),
        };

        let mut text_body = None;
        let mut html_body = None;

        if let Some(body) = &payload.body {
            if let Some(data) = &body.data {
                if let Some(decoded) = Self::bytes_to_string(data) {
                    match payload.mime_type.as_deref() {
                        Some("text/plain") => text_body = Some(decoded),
                        Some("text/html") => html_body = Some(decoded),
                        _ => text_body = Some(decoded),
                    }
                }
            }
        }

        if let Some(parts) = &payload.parts {
            Self::extract_bodies_from_parts(parts, &mut text_body, &mut html_body);
        }

        (text_body, html_body)
    }

    fn extract_bodies_from_parts(
        parts: &[google_gmail1::api::MessagePart],
        text_body: &mut Option<String>,
        html_body: &mut Option<String>,
    ) {
        for part in parts {
            match part.mime_type.as_deref() {
                Some("text/plain") if text_body.is_none() => {
                    if let Some(body) = &part.body {
                        if let Some(data) = &body.data {
                            if let Some(decoded) = Self::bytes_to_string(data) {
                                *text_body = Some(decoded);
                            }
                        }
                    }
                }
                Some("text/html") if html_body.is_none() => {
                    if let Some(body) = &part.body {
                        if let Some(data) = &body.data {
                            if let Some(decoded) = Self::bytes_to_string(data) {
                                *html_body = Some(decoded);
                            }
                        }
                    }
                }
                Some(mime) if mime.starts_with("multipart/") => {
                    if let Some(nested_parts) = &part.parts {
                        Self::extract_bodies_from_parts(nested_parts, text_body, html_body);
                    }
                }
                _ => {}
            }
        }
    }

    fn bytes_to_string(data: &[u8]) -> Option<String> {
        String::from_utf8(data.to_vec()).ok()
    }

    fn detect_attachments(message: &Message) -> bool {
        let payload = match message.payload.as_ref() {
            Some(p) => p,
            None => return false,
        };

        if let Some(parts) = &payload.parts {
            return Self::has_attachments_in_parts(parts);
        }

        false
    }

    fn has_attachments_in_parts(parts: &[google_gmail1::api::MessagePart]) -> bool {
        for part in parts {
            if let Some(filename) = &part.filename {
                if !filename.is_empty() {
                    return true;
                }
            }

            if let Some(headers) = &part.headers {
                for header in headers {
                    if header.name.as_deref() == Some("Content-Disposition") {
                        if let Some(value) = &header.value {
                            if value.starts_with("attachment") {
                                return true;
                            }
                        }
                    }
                }
            }

            if let Some(nested_parts) = &part.parts {
                if Self::has_attachments_in_parts(nested_parts) {
                    return true;
                }
            }
        }

        false
    }
}
