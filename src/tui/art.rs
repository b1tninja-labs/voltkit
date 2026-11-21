//! Side-view car, lightning bolt, and the name, drawn above the wizard.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// Rows the banner occupies.
pub const HEIGHT: u16 = 5;

const CAR: [&str; HEIGHT as usize] = [
    r"           ______________",
    r"      ____/              \___",
    r"    _/     [____]  [____]    \_",
    r"  __/       \_________/       \__",
    r"       (O)               (O)",
];

const BOLT: [&str; HEIGHT as usize] = [r"  /\", r" /  \", r"<   /", r" \  \", r"  \/"];

const WORD_ROW: usize = 2;
const WORD: &str = "VOLTKIT";

/// Draw the mark into `area`.
pub fn draw(frame: &mut ratatui::Frame, area: Rect) {
    let outline = Style::new().fg(Color::Gray).add_modifier(Modifier::DIM);
    let bolt = Style::new()
        .fg(Color::LightGreen)
        .add_modifier(Modifier::BOLD);
    let word = Style::new()
        .fg(Color::LightCyan)
        .add_modifier(Modifier::BOLD);
    let car_width = CAR.iter().map(|line| line.len()).max().unwrap_or(0);
    let bolt_width = BOLT.iter().map(|line| line.len()).max().unwrap_or(0);
    let mut lines = Vec::with_capacity(HEIGHT as usize);
    for (index, (car, flash)) in CAR.iter().zip(BOLT).enumerate() {
        let mut spans = vec![
            Span::styled(format!("{car:<car_width$}   "), outline),
            Span::styled(format!("{flash:<bolt_width$}"), bolt),
        ];
        if index == WORD_ROW {
            spans.push(Span::styled(format!("  {WORD}"), word));
        }
        lines.push(Line::from(spans));
    }
    frame.render_widget(Paragraph::new(lines), area);
}
