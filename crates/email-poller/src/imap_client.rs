use anyhow::{Context, Result};
use async_imap::Session;
use async_native_tls::TlsStream;
use async_std::net::TcpStream;
use chrono::{DateTime, Utc};
use futures::TryStreamExt;

pub struct ImapClient {
    session: Session<TlsStream<TcpStream>>,
}

#[derive(Debug, Clone)]
pub struct EmailMessage {
    pub id: String,
    pub subject: String,
    pub from: String,
    pub snippet: String,
    pub body: Option<String>,
    pub received_at: Option<DateTime<Utc>>,
    pub unsubscribe: Option<String>,
}

impl ImapClient {
    pub async fn connect(server: &str, email: &str, password: &str) -> Result<Self> {
        let tcp = TcpStream::connect((server, 993))
            .await
            .context("Failed to connect to IMAP server")?;

        let tls = async_native_tls::TlsConnector::new();
        let tls_stream = tls
            .connect(server, tcp)
            .await
            .context("TLS handshake failed")?;

        let client = async_imap::Client::new(tls_stream);

        let session = client
            .login(email, password)
            .await
            .map_err(|e| anyhow::anyhow!("Login failed: {}", e.0))?;

        Ok(Self { session })
    }

    pub async fn fetch_recent_emails(&mut self, count: u32) -> Result<Vec<EmailMessage>> {
        let mailbox = self
            .session
            .select("INBOX")
            .await
            .context("Failed to select INBOX")?;

        let total = mailbox.exists;
        if total == 0 {
            return Ok(vec![]);
        }

        // Fetch the last N messages (newest first)
        let start = total.saturating_sub(count) + 1;
        let range = format!("{}:{}", start, total);

        let messages: Vec<_> = self
            .session
            .fetch(&range, "(UID RFC822)")
            .await
            .context("Failed to fetch messages")?
            .try_collect()
            .await?;

        let mut emails = Vec::new();

        for message in &messages {
            let uid = message.uid.unwrap_or(0);

            if let Some(body) = message.body() {
                match mailparse::parse_mail(body) {
                    Ok(parsed) => {
                        let email = Self::parse_email(uid, &parsed);
                        emails.push(email);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse email {}: {}", uid, e);
                    }
                }
            }
        }

        // Reverse so newest is first
        emails.reverse();
        Ok(emails)
    }

    pub async fn fetch_emails_since_uid(
        &mut self,
        since_uid: u32,
        max: u32,
    ) -> Result<Vec<EmailMessage>> {
        self.session
            .select("INBOX")
            .await
            .context("Failed to select INBOX")?;

        // Fetch messages with UID greater than since_uid
        let range = format!("{}:*", since_uid + 1);

        let messages: Vec<_> = self
            .session
            .uid_fetch(&range, "(UID RFC822)")
            .await
            .context("Failed to fetch messages")?
            .try_collect()
            .await?;

        let mut emails = Vec::new();

        for message in messages.iter().take(max as usize) {
            let uid = message.uid.unwrap_or(0);
            if uid <= since_uid {
                continue;
            }

            if let Some(body) = message.body() {
                match mailparse::parse_mail(body) {
                    Ok(parsed) => {
                        let email = Self::parse_email(uid, &parsed);
                        emails.push(email);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse email {}: {}", uid, e);
                    }
                }
            }
        }

        Ok(emails)
    }

    fn parse_email(uid: u32, parsed: &mailparse::ParsedMail) -> EmailMessage {
        let subject = parsed
            .headers
            .iter()
            .find(|h| h.get_key().eq_ignore_ascii_case("subject"))
            .map(|h| h.get_value())
            .unwrap_or_default();

        let from = parsed
            .headers
            .iter()
            .find(|h| h.get_key().eq_ignore_ascii_case("from"))
            .map(|h| h.get_value())
            .unwrap_or_default();

        let date = parsed
            .headers
            .iter()
            .find(|h| h.get_key().eq_ignore_ascii_case("date"))
            .and_then(|h| {
                let value = h.get_value();
                DateTime::parse_from_rfc2822(&value)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            });

        let unsubscribe = parsed
            .headers
            .iter()
            .find(|h| h.get_key().eq_ignore_ascii_case("list-unsubscribe"))
            .map(|h| h.get_value());

        let body = Self::extract_body(parsed);
        let snippet = body
            .as_ref()
            .map(|b| {
                b.chars()
                    .take(200)
                    .collect::<String>()
                    .replace('\n', " ")
                    .replace('\r', "")
            })
            .unwrap_or_default();

        EmailMessage {
            id: uid.to_string(),
            subject,
            from,
            snippet,
            body,
            received_at: date,
            unsubscribe,
        }
    }

    fn extract_body(parsed: &mailparse::ParsedMail) -> Option<String> {
        // If this part is text/plain, return it
        if parsed.ctype.mimetype == "text/plain" {
            return parsed.get_body().ok();
        }

        // Check subparts for text/plain
        for part in &parsed.subparts {
            if part.ctype.mimetype == "text/plain" {
                if let Ok(body) = part.get_body() {
                    return Some(body);
                }
            }
        }

        // Fallback: try to get any body
        for part in &parsed.subparts {
            if let Ok(body) = part.get_body() {
                return Some(body);
            }
        }

        parsed.get_body().ok()
    }

    /// Archive a message by UID (Gmail: removes from INBOX, keeps in All Mail)
    pub async fn archive(&mut self, uid: u32) -> Result<()> {
        self.session
            .select("INBOX")
            .await
            .context("Failed to select INBOX")?;

        // For Gmail, just mark as deleted from INBOX - it stays in All Mail
        self.session
            .uid_store(format!("{}", uid), "+FLAGS (\\Deleted)")
            .await
            .context("Failed to mark message as deleted")?
            .try_collect::<Vec<_>>()
            .await?;

        // Expunge to actually remove from INBOX
        self.session
            .expunge()
            .await
            .context("Failed to expunge")?
            .try_collect::<Vec<_>>()
            .await?;

        Ok(())
    }

    /// Archive multiple messages by UID
    pub async fn archive_many(&mut self, uids: &[u32]) -> Result<()> {
        if uids.is_empty() {
            return Ok(());
        }

        self.session
            .select("INBOX")
            .await
            .context("Failed to select INBOX")?;

        // Build UID sequence (e.g., "123,456,789")
        let uid_seq: String = uids
            .iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(",");

        self.session
            .uid_store(&uid_seq, "+FLAGS (\\Deleted)")
            .await
            .context("Failed to mark messages as deleted")?
            .try_collect::<Vec<_>>()
            .await?;

        self.session
            .expunge()
            .await
            .context("Failed to expunge")?
            .try_collect::<Vec<_>>()
            .await?;

        Ok(())
    }

    /// Fetch emails before a given UID (going backwards in time)
    pub async fn fetch_emails_before_uid(
        &mut self,
        before_uid: u32,
        max: u32,
    ) -> Result<Vec<EmailMessage>> {
        self.session
            .select("INBOX")
            .await
            .context("Failed to select INBOX")?;

        if before_uid <= 1 {
            return Ok(vec![]);
        }

        // Calculate the range: from (before_uid - max) to (before_uid - 1)
        let end_uid = before_uid - 1;
        let start_uid = before_uid.saturating_sub(max).max(1);
        let range = format!("{}:{}", start_uid, end_uid);

        let messages: Vec<_> = self
            .session
            .uid_fetch(&range, "(UID RFC822)")
            .await
            .context("Failed to fetch messages")?
            .try_collect()
            .await?;

        let mut emails = Vec::new();

        for message in messages.iter() {
            let uid = message.uid.unwrap_or(0);
            if uid >= before_uid || uid == 0 {
                continue;
            }

            if let Some(body) = message.body() {
                match mailparse::parse_mail(body) {
                    Ok(parsed) => {
                        let email = Self::parse_email(uid, &parsed);
                        emails.push(email);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse email {}: {}", uid, e);
                    }
                }
            }
        }

        // Sort by UID descending (newest first)
        emails.sort_by(|a, b| {
            let uid_a: u32 = a.id.parse().unwrap_or(0);
            let uid_b: u32 = b.id.parse().unwrap_or(0);
            uid_b.cmp(&uid_a)
        });

        Ok(emails)
    }

    pub async fn logout(mut self) -> Result<()> {
        self.session.logout().await.context("Failed to logout")?;
        Ok(())
    }
}
