use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

#[derive(Debug, Default)]
pub struct Task {
    pub title: String,
    pub description: String,
}

pub fn generate_tasks() -> Vec<Task> {
    (0..5)
        .into_iter()
        .map(|_| Task {
            title: lipsum::lipsum_title(),
            description: lipsum::lipsum_words(30),
        })
        .collect::<Vec<Task>>()
}

impl Widget for &Task {
    fn render(self, area: Rect, buffer: &mut Buffer) {}
}
