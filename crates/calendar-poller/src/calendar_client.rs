use anyhow::Result;

pub struct CalendarClient {}

impl CalendarClient {
    pub async fn new() -> Result<Self> {
        Ok(Self {})
    }

    pub async fn fetch_upcoming_events(&self) -> Result<Vec<String>> {
        Ok(vec![])
    }
}
