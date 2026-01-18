use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use google_gmail1::api::Message;
use google_gmail1::hyper_rustls::HttpsConnector;
use google_gmail1::Gmail;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::path::Path;

use crate::config::GmailConfig;

/// Client for interacting with Gmail API
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
    pub snippet: String,
    pub body: Option<String>,
    pub received_at: Option<DateTime<Utc>>,
    pub history_id: Option<u64>,
}

impl GmailClient {
    /// Create a new Gmail client using InstalledFlow OAuth
    pub async fn new(config: &GmailConfig) -> Result<Self> {
        let secret = google_gmail1::yup_oauth2::read_application_secret(&config.credentials_path)
            .await
            .context("Failed to read OAuth credentials")?;

        let auth = google_gmail1::yup_oauth2::InstalledFlowAuthenticator::builder(
            secret,
            google_gmail1::yup_oauth2::InstalledFlowReturnMethod::HTTPRedirect,
        )
        .persist_tokens_to_disk(Path::new(&config.token_cache_path))
        .build()
        .await
        .context("Failed to build authenticator")?;

        let connector = google_gmail1::hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .context("Failed to load native TLS roots")?
            .https_or_http()
            .enable_http1()
            .build();

        let client = Client::builder(TokioExecutor::new()).build(connector);
        let hub = Gmail::new(client, auth);

        // Get email address from config
        let email_address = config.email.clone();

        Ok(Self { hub, email_address })
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

    /// Archive multiple messages
    pub async fn archive_many(&self, message_ids: &[String]) -> Result<()> {
        for id in message_ids {
            self.archive_message(id).await?;
        }
        Ok(())
    }

    fn parse_message(message: Message) -> EmailMessage {
        let id = message.id.clone().unwrap_or_default();
        let thread_id = message.thread_id.clone().unwrap_or_default();
        let snippet = message.snippet.clone().unwrap_or_default();
        let history_id = message.history_id;

        let mut subject = String::new();
        let mut from = String::new();
        let mut received_at = None;

        if let Some(payload) = &message.payload {
            if let Some(headers) = &payload.headers {
                for header in headers {
                    match header.name.as_deref() {
                        Some("Subject") => subject = header.value.clone().unwrap_or_default(),
                        Some("From") => from = header.value.clone().unwrap_or_default(),
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

        let body = Self::extract_body(&message);

        EmailMessage {
            id,
            thread_id,
            subject,
            from,
            snippet,
            body,
            received_at,
            history_id,
        }
    }

    fn parse_date(date_str: &str) -> Option<DateTime<Utc>> {
        if let Ok(dt) = DateTime::parse_from_rfc2822(date_str) {
            return Some(dt.with_timezone(&Utc));
        }
        None
    }

    fn extract_body(message: &Message) -> Option<String> {
        let payload = message.payload.as_ref()?;

        // Check if body data is directly in payload
        if let Some(body) = &payload.body {
            if let Some(data) = &body.data {
                if let Ok(decoded) =
                    base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE, data)
                {
                    if let Ok(text) = String::from_utf8(decoded) {
                        return Some(text);
                    }
                }
            }
        }

        // Check parts for text/plain
        if let Some(parts) = &payload.parts {
            for part in parts {
                if part.mime_type.as_deref() == Some("text/plain") {
                    if let Some(body) = &part.body {
                        if let Some(data) = &body.data {
                            if let Ok(decoded) = base64::Engine::decode(
                                &base64::engine::general_purpose::URL_SAFE,
                                data,
                            ) {
                                if let Ok(text) = String::from_utf8(decoded) {
                                    return Some(text);
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }
}
