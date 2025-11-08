use anyhow::Result;
use shared_types::Todo;

pub fn process_email_to_todo(_email_content: &str) -> Result<Option<Todo>> {
    Ok(None)
}
