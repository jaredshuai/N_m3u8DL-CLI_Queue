pub(crate) trait TaskIdGenerator: Send + Sync {
    fn next_task_id(&self) -> String;
}
