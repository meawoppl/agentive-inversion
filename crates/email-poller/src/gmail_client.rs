use anyhow::Result;

#[allow(dead_code)]
pub struct GmailClient {}

#[allow(dead_code)]
impl GmailClient {
    pub async fn new() -> Result<Self> {
        Ok(Self {})
    }

    pub async fn fetch_recent_emails(&self) -> Result<Vec<String>> {
        Ok(vec![])
    }
}
