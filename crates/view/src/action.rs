/// Actions that can be dispatched by components or the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Quit,
    SelectFile(usize),
    Back,
    ToggleDiff,
    ScrollUp(u16),
    ScrollDown(u16),
    PeekDeleted(usize),
    DismissPeek,
    Refresh,
    Noop,
}
