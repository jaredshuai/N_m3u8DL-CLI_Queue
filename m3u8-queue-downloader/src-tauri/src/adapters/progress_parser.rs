use regex::{Regex, RegexBuilder};
use std::sync::OnceLock;

static PROGRESS_PERCENT_RE: OnceLock<Regex> = OnceLock::new();
static PROGRESS_COUNT_RE: OnceLock<Regex> = OnceLock::new();
static SPEED_RE: OnceLock<Regex> = OnceLock::new();
static THREADS_RE: OnceLock<Regex> = OnceLock::new();

pub(crate) fn parse_progress(line: &str) -> Option<f32> {
    if let Some(caps) = regex_progress_percent().captures(line) {
        if let Some(m) = caps.get(1) {
            let normalized = m.as_str().replace(',', ".");
            if let Ok(pct) = normalized.parse::<f32>() {
                return Some((pct / 100.0).clamp(0.0, 1.0));
            }
        }
    }

    if let Some(caps) = regex_progress_count().captures(line) {
        let current = caps.get(1)?.as_str().parse::<f32>().ok()?;
        let total = caps.get(2)?.as_str().parse::<f32>().ok()?;
        if total > 0.0 {
            return Some((current / total).clamp(0.0, 1.0));
        }
    }
    None
}

pub(crate) fn parse_speed(line: &str) -> Option<String> {
    if let Some(caps) = regex_speed_lazy().captures(line) {
        if let Some(m) = caps.get(1) {
            return Some(m.as_str().trim().to_string());
        }
    }
    None
}

pub(crate) fn parse_threads(line: &str) -> Option<String> {
    if let Some(caps) = regex_threads_lazy().captures(line) {
        if let Some(m) = caps.get(1).or_else(|| caps.get(2)) {
            return Some(m.as_str().to_string());
        }
    }
    None
}

fn regex_progress_percent() -> &'static Regex {
    PROGRESS_PERCENT_RE.get_or_init(|| Regex::new(r"([0-9]+(?:[\.,][0-9]+)?)%").unwrap())
}

fn regex_progress_count() -> &'static Regex {
    PROGRESS_COUNT_RE.get_or_init(|| Regex::new(r"Progress:\s*(\d+)\s*/\s*(\d+)").unwrap())
}

fn regex_speed_lazy() -> &'static Regex {
    SPEED_RE.get_or_init(|| {
        RegexBuilder::new(r"([0-9]+(?:[\.,][0-9]+)?\s*(?:[KMGT]i?B|KB|MB|GB|TB|B)/(?:s|秒))")
            .case_insensitive(true)
            .build()
            .unwrap()
    })
}

fn regex_threads_lazy() -> &'static Regex {
    THREADS_RE.get_or_init(|| {
        RegexBuilder::new(r"(?:threads?|线程(?:数)?)[\s:=：]*(\d+)|([0-9]+)[\s]*(?:threads?|线程)")
            .case_insensitive(true)
            .build()
            .unwrap()
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_progress, parse_speed, parse_threads};

    #[test]
    fn parse_progress_reads_cli_percent_as_ratio() {
        assert_eq!(
            parse_progress("Progress: 10/40 (25.00%) -- 1.0MB/4.0MB"),
            Some(0.25)
        );
        assert_eq!(parse_progress("speed only 1.5 MB/s"), None);
    }

    #[test]
    fn parse_speed_reads_cli_speed_units() {
        assert_eq!(
            parse_speed("(1.5 MB/s @ 00:01:00)").as_deref(),
            Some("1.5 MB/s")
        );
    }

    #[test]
    fn parse_progress_reads_real_cli_progress_reporter_line() {
        let line = "12:34:56.789 Progress: 10/40 (25.00%) -- 1.00MB/4.00MB (512.5KB/s @ 00:00:06)";
        assert_eq!(parse_progress(line), Some(0.25));
        assert_eq!(parse_speed(line).as_deref(), Some("512.5KB/s"));
    }

    #[test]
    fn parse_progress_accepts_comma_decimal_and_count_fallback() {
        assert_eq!(
            parse_progress("Progress: 1/4 (25,00%) -- 1MB/4MB"),
            Some(0.25)
        );
        assert_eq!(parse_progress("Progress: 2/8 -- 1MB/4MB"), Some(0.25));
    }

    #[test]
    fn parse_threads_reads_common_cli_formats() {
        assert_eq!(parse_threads("Threads: 16").as_deref(), Some("16"));
        assert_eq!(parse_threads("16 threads active").as_deref(), Some("16"));
    }
}
