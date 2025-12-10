#[derive(Debug, Default)]
pub struct Notification {
    pub message: String,
    pub kind: NotificationKind,
}

#[derive(Debug, Default)]
pub enum NotificationKind {
    Success,
    Error,
    #[default]
    Info,
}
