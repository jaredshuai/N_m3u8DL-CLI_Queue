#[derive(Debug, Clone)]
pub struct AddTaskPayload {
    pub url: String,
    pub save_name: Option<String>,
    pub headers: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_task_payload_carries_user_supplied_task_fields() {
        let payload = AddTaskPayload {
            url: "https://example.com/test.m3u8".to_string(),
            save_name: Some("episode-01".to_string()),
            headers: Some("User-Agent: test".to_string()),
        };

        assert_eq!(payload.url, "https://example.com/test.m3u8");
        assert_eq!(payload.save_name.as_deref(), Some("episode-01"));
        assert_eq!(payload.headers.as_deref(), Some("User-Agent: test"));
    }
}
