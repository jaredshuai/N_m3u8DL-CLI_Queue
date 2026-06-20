#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RetryPolicy {
    max_retries: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RetryDecision {
    Retry { next_retry_count: u8 },
    Terminal,
}

impl RetryPolicy {
    pub(crate) const fn new(max_retries: u8) -> Self {
        Self { max_retries }
    }

    pub(crate) fn decide(self, current_retry_count: u8) -> RetryDecision {
        if current_retry_count < self.max_retries {
            RetryDecision::Retry {
                next_retry_count: current_retry_count + 1,
            }
        } else {
            RetryDecision::Terminal
        }
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::new(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_until_max_retry_count_is_reached() {
        let policy = RetryPolicy::new(2);

        assert_eq!(
            policy.decide(0),
            RetryDecision::Retry {
                next_retry_count: 1
            }
        );
        assert_eq!(
            policy.decide(1),
            RetryDecision::Retry {
                next_retry_count: 2
            }
        );
        assert_eq!(policy.decide(2), RetryDecision::Terminal);
    }
}
