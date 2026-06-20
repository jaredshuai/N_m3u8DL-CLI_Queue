#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TerminalActiveLine {
    Present(String),
    Missing,
}
