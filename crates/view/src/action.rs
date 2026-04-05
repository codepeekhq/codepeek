#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Quit,
    SelectFile(usize),
    Back,
    ToggleDiff,
    DismissPeek,
    Refresh,
    Noop,
}
