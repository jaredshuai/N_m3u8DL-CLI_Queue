#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DownloadDirectory {
    value: String,
}

impl DownloadDirectory {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.value
    }

    pub(crate) fn into_string(self) -> String {
        self.value
    }
}
