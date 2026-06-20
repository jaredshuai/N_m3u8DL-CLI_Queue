#[derive(Debug, Clone)]
pub(crate) struct TerminalOutputPage {
    pub lines: Vec<String>,
    pub offset: usize,
    pub total: usize,
    pub next_offset: usize,
    pub has_more_before: bool,
    pub has_more_after: bool,
}
