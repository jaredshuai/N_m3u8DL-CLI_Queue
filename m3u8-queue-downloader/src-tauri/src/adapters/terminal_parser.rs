use encoding_rs::{Encoding, GB18030};

pub(crate) struct TerminalBuffer {
    committed_lines: Vec<String>,
    active_line: String,
    replace_on_next_char: bool,
    pending_bytes: Vec<u8>,
}

impl TerminalBuffer {
    pub(crate) fn new() -> Self {
        Self {
            committed_lines: Vec::new(),
            active_line: String::new(),
            replace_on_next_char: false,
            pending_bytes: Vec::new(),
        }
    }

    pub(crate) fn feed(&mut self, data: &[u8]) {
        self.pending_bytes.extend_from_slice(data);

        let text = decode_cli_bytes_lossy(&self.pending_bytes);
        self.pending_bytes.clear();

        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            match ch {
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                        self.commit_active_line();
                    } else {
                        self.replace_on_next_char = true;
                    }
                }
                '\n' => {
                    self.commit_active_line();
                }
                _ => {
                    if self.replace_on_next_char {
                        self.active_line.clear();
                        self.replace_on_next_char = false;
                    }
                    self.active_line.push(ch);
                }
            }
        }
    }

    fn commit_active_line(&mut self) {
        let line = self.active_line.trim().to_string();
        if !line.is_empty() {
            self.committed_lines.push(line);
        }
        self.active_line.clear();
        self.replace_on_next_char = false;
    }

    pub(crate) fn take_committed(&mut self) -> Vec<String> {
        std::mem::take(&mut self.committed_lines)
    }

    pub(crate) fn active_line(&self) -> String {
        self.active_line.clone()
    }

    pub(crate) fn finish(&mut self) {
        if !self.active_line.trim().is_empty() {
            self.commit_active_line();
        }
    }
}

pub(crate) fn decode_cli_bytes_lossy(bytes: &[u8]) -> String {
    if let Ok(decoded) = std::str::from_utf8(bytes) {
        return decoded.to_string();
    }

    cli_output_fallback_encoding().decode(bytes).0.into_owned()
}

fn cli_output_fallback_encoding() -> &'static Encoding {
    #[cfg(target_os = "windows")]
    {
        GB18030
    }

    #[cfg(not(target_os = "windows"))]
    {
        encoding_rs::UTF_8
    }
}

#[cfg(test)]
fn take_cli_segment(input: &[u8]) -> Option<(String, Vec<u8>)> {
    let split_at = input
        .iter()
        .position(|byte| *byte == b'\r' || *byte == b'\n')?;
    let mut rest_start = split_at + 1;
    if input.get(split_at) == Some(&b'\r') && input.get(split_at + 1) == Some(&b'\n') {
        rest_start = split_at + 2;
    }

    let segment = decode_cli_bytes_lossy(&input[..split_at])
        .trim()
        .to_string();
    let rest = input[rest_start..].to_vec();
    Some((segment, rest))
}

#[cfg(test)]
mod tests {
    use super::{decode_cli_bytes_lossy, take_cli_segment, TerminalBuffer};

    #[test]
    fn take_cli_segment_splits_on_carriage_return() {
        let (segment, rest) = take_cli_segment(b"Progress: 1/2 (50.00%)\rnext").expect("segment");
        assert_eq!(segment, "Progress: 1/2 (50.00%)");
        assert_eq!(rest, b"next".to_vec());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn decode_cli_bytes_lossy_prefers_utf8_when_valid() {
        let bytes = "文件名称：测试".as_bytes();
        assert_eq!(decode_cli_bytes_lossy(bytes), "文件名称：测试");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn decode_cli_bytes_lossy_falls_back_for_non_utf8_bytes() {
        let (encoded, _, _) = encoding_rs::GB18030.encode("文件名称：测试");
        assert_eq!(decode_cli_bytes_lossy(&encoded), "文件名称：测试");
    }

    #[test]
    fn terminal_buffer_cr_overwrites_active_line() {
        let mut buf = TerminalBuffer::new();
        buf.feed(b"Progress: 1/10\rProgress: 2/10\rProgress: 3/10");
        assert_eq!(buf.active_line(), "Progress: 3/10");
        assert!(buf.take_committed().is_empty());
    }

    #[test]
    fn terminal_buffer_lf_commits_to_history() {
        let mut buf = TerminalBuffer::new();
        buf.feed(b"line one\nline two\n");
        let committed = buf.take_committed();
        assert_eq!(committed, vec!["line one", "line two"]);
        assert_eq!(buf.active_line(), "");
    }

    #[test]
    fn terminal_buffer_mixed_cr_and_lf() {
        let mut buf = TerminalBuffer::new();
        buf.feed(b"Starting download\nProgress: 1/10\rProgress: 2/10\rProgress: 3/10\nDone\n");
        let committed = buf.take_committed();
        assert_eq!(
            committed,
            vec!["Starting download", "Progress: 3/10", "Done"]
        );
        assert_eq!(buf.active_line(), "");
    }

    #[test]
    fn terminal_buffer_keeps_progress_reporter_line_active_after_trailing_cr() {
        let mut buf = TerminalBuffer::new();
        buf.feed(b"\rProgress: 1/10 (10.00%) -- 1MB/10MB\r");
        assert_eq!(buf.active_line(), "Progress: 1/10 (10.00%) -- 1MB/10MB");
        assert!(buf.take_committed().is_empty());
    }

    #[test]
    fn terminal_buffer_overwrites_progress_reporter_line_without_committing_history() {
        let mut buf = TerminalBuffer::new();
        buf.feed(b"\rProgress: 1/10 (10.00%) -- 1MB/10MB\r\rProgress: 2/10 (20.00%) -- 2MB/10MB\r");
        assert_eq!(buf.active_line(), "Progress: 2/10 (20.00%) -- 2MB/10MB");
        assert!(buf.take_committed().is_empty());
    }

    #[test]
    fn terminal_buffer_replaces_longer_progress_tail_with_shorter_status_line() {
        let mut buf = TerminalBuffer::new();
        buf.feed(
            b"\rProgress: 1650/1657 (99.58%) -- 1.14 GB/1.15 GB (793.51 KB/s @ 00m06s18s))\r\r11:07:37.504 \xe5\xb7\xb2\xe4\xb8\x8b\xe8\xbd\xbd\xe5\xae\x8c\xe6\x88\x90\r",
        );
        assert_eq!(buf.active_line(), "11:07:37.504 已下载完成");
        assert!(buf.take_committed().is_empty());
    }

    #[test]
    fn terminal_buffer_does_not_commit_progress_line_when_logger_prints_next_line() {
        let mut buf = TerminalBuffer::new();
        buf.feed(
            b"\rProgress: 3/10 (30.00%) -- 3MB/10MB\r\r                                        \r11:00:00.000 \xe7\xad\x89\xe5\xbe\x85\xe4\xb8\x8b\xe8\xbd\xbd\xe5\xae\x8c\xe6\x88\x90...\n",
        );
        let committed = buf.take_committed();
        assert_eq!(committed, vec!["11:00:00.000 等待下载完成..."]);
        assert_eq!(buf.active_line(), "");
    }

    #[test]
    fn terminal_buffer_crlf_treated_as_newline() {
        let mut buf = TerminalBuffer::new();
        buf.feed(b"hello\r\nworld\r\n");
        let committed = buf.take_committed();
        assert_eq!(committed, vec!["hello", "world"]);
    }

    #[test]
    fn terminal_buffer_finish_commits_trailing_active_line() {
        let mut buf = TerminalBuffer::new();
        buf.feed(b"trailing text");
        assert_eq!(buf.active_line(), "trailing text");
        buf.finish();
        let committed = buf.take_committed();
        assert_eq!(committed, vec!["trailing text"]);
    }
}
