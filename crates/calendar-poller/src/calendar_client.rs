use anyhow::Result;

#[allow(dead_code)]
pub struct CalendarClient {}

#[allow(dead_code)]
impl CalendarClient {
    pub async fn new() -> Result<Self> {
        Ok(Self {})
    }

    pub async fn fetch_upcoming_events(&self) -> Result<Vec<String>> {
        Ok(vec![])
    }
}
