use anyhow::Result;
use shared_types::Todo;

pub fn process_calendar_event_to_todo(_event: &str) -> Result<Option<Todo>> {
    Ok(None)
}
