use anyhow::Result;

pub struct GmailClient {}

impl GmailClient {
    pub async fn new() -> Result<Self> {
        Ok(Self {})
    }

    pub async fn fetch_recent_emails(&self) -> Result<Vec<String>> {
        Ok(vec![])
    }
}
