use chrono::{DateTime, Utc};

pub(crate) trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}
