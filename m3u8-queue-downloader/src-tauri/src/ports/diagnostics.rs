pub(crate) trait Diagnostics: Send + Sync {
    fn warn(&self, message: &str);
}
