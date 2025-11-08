use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use google_gmail1::{
    api::Message,
    hyper::{client::HttpConnector, Client},
    hyper_rustls::{self, HttpsConnector},
    oauth2, Gmail,
};
use shared_types::EmailAccount;

pub struct GmailClient {
    hub: Gmail<HttpsConnector<HttpConnector>>,
}

#[derive(Debug, Clone)]
pub struct EmailMessage {
    pub id: String,
    pub subject: String,
    pub from: String,
    pub snippet: String,
    pub body: Option<String>,
    pub received_at: Option<DateTime<Utc>>,
}

impl GmailClient {
    pub async fn new(account: &EmailAccount) -> Result<Self> {
        let client_id =
            std::env::var("GMAIL_CLIENT_ID").context("GMAIL_CLIENT_ID not found in environment")?;
        let client_secret = std::env::var("GMAIL_CLIENT_SECRET")
            .context("GMAIL_CLIENT_SECRET not found in environment")?;

        // Verify refresh token exists (it will be loaded from persisted token file)
        let _ = account
            .oauth_refresh_token
            .as_ref()
            .context("No refresh token found for account")?;

        // Create OAuth2 application secret
        let secret = oauth2::ApplicationSecret {
            client_id: client_id.clone(),
            client_secret: client_secret.clone(),
            auth_uri: "https://accounts.google.com/o/oauth2/auth".to_string(),
            token_uri: "https://oauth2.googleapis.com/token".to_string(),
            redirect_uris: vec![
                "http://localhost:3000/api/email-accounts/oauth/callback".to_string()
            ],
            ..Default::default()
        };

        // Create authenticator with persistent token storage
        use yup_oauth2::InstalledFlowAuthenticator;

        let auth = InstalledFlowAuthenticator::builder(
            secret,
            yup_oauth2::InstalledFlowReturnMethod::HTTPRedirect,
        )
        .persist_tokens_to_disk(format!("/tmp/gmail_token_{}.json", account.id))
        .build()
        .await?;

        // The authenticator will handle token refresh automatically using the persisted refresh token

        let client = hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .context("Failed to load native roots")?
            .https_or_http()
            .enable_http1()
            .build();

        let hub = Gmail::new(Client::builder().build(client), auth);

        Ok(Self { hub })
    }

    pub async fn fetch_recent_emails(&self, max_results: u32) -> Result<Vec<EmailMessage>> {
        let result = self
            .hub
            .users()
            .messages_list("me")
            .max_results(max_results)
            .doit()
            .await?;

        let messages = result.1.messages.unwrap_or_default();
        let mut email_messages = Vec::new();

        for msg in messages {
            if let Some(msg_id) = msg.id {
                match self.fetch_email_details(&msg_id).await {
                    Ok(email) => email_messages.push(email),
                    Err(e) => {
                        tracing::warn!("Failed to fetch email {}: {}", msg_id, e);
                        continue;
                    }
                }
            }
        }

        Ok(email_messages)
    }

    pub async fn fetch_emails_since(
        &self,
        since_message_id: &str,
        max_results: u32,
    ) -> Result<Vec<EmailMessage>> {
        // Gmail doesn't directly support "since message ID", so we'll fetch recent
        // and filter. In production, you'd want to use history.list() API
        let all_messages = self.fetch_recent_emails(max_results).await?;

        // Filter messages that are newer than the since_message_id
        // This is a simplified approach
        let filtered: Vec<EmailMessage> = all_messages
            .into_iter()
            .take_while(|msg| msg.id != since_message_id)
            .collect();

        Ok(filtered)
    }

    async fn fetch_email_details(&self, message_id: &str) -> Result<EmailMessage> {
        let result = self
            .hub
            .users()
            .messages_get("me", message_id)
            .doit()
            .await?;

        let message = result.1;

        let subject = Self::get_header(&message, "Subject").unwrap_or_default();
        let from = Self::get_header(&message, "From").unwrap_or_default();
        let snippet = message.snippet.clone().unwrap_or_default();
        let body = Self::extract_body(&message);

        // Parse received time from internal date (milliseconds since epoch)
        let received_at = message
            .internal_date
            .and_then(|ts| DateTime::from_timestamp(ts / 1000, ((ts % 1000) * 1_000_000) as u32));

        Ok(EmailMessage {
            id: message_id.to_string(),
            subject,
            from,
            snippet,
            body,
            received_at,
        })
    }

    fn get_header(message: &Message, header_name: &str) -> Option<String> {
        message
            .payload
            .as_ref()?
            .headers
            .as_ref()?
            .iter()
            .find(|h| h.name.as_deref() == Some(header_name))
            .and_then(|h| h.value.clone())
    }

    fn extract_body(message: &Message) -> Option<String> {
        let payload = message.payload.as_ref()?;

        // Try to get plain text body
        // The Gmail API returns base64-encoded data as bytes
        if let Some(body) = &payload.body {
            if let Some(data_bytes) = &body.data {
                // data is already decoded by the gmail API, just convert to string
                if let Ok(text) = String::from_utf8(data_bytes.clone()) {
                    return Some(text);
                }
            }
        }

        // If no direct body, check parts
        if let Some(parts) = &payload.parts {
            for part in parts {
                if part.mime_type.as_deref() == Some("text/plain") {
                    if let Some(body) = &part.body {
                        if let Some(data_bytes) = &body.data {
                            if let Ok(text) = String::from_utf8(data_bytes.clone()) {
                                return Some(text);
                            }
                        }
                    }
                }
            }
        }

        None
    }
}
