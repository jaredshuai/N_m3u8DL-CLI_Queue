use crate::ports::clock::Clock;
use chrono::{DateTime, Utc};

pub(crate) struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}
