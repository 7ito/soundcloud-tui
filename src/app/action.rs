#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Action {
    FocusNext,
    FocusPrevious,
    MoveUp,
    MoveDown,
    Select,
    Quit,
}
