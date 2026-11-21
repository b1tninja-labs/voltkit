//! The last question before the adapter is opened.

use crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Paragraph, Wrap};
use voltkit::wizard::{self, Plan};

#[derive(Debug, PartialEq, Eq)]
pub enum ConfirmEvent {
    Run,
    Decline,
    Back,
}

pub fn on_key(code: KeyCode) -> Option<ConfirmEvent> {
    match code {
        KeyCode::Char('y' | 'Y') | KeyCode::Enter => Some(ConfirmEvent::Run),
        KeyCode::Char('n' | 'N') => Some(ConfirmEvent::Decline),
        KeyCode::Esc => Some(ConfirmEvent::Back),
        KeyCode::Char('q') => Some(ConfirmEvent::Decline),
        _ => None,
    }
}

pub fn draw(frame: &mut ratatui::Frame, area: Rect, plan: &Plan) {
    let [title, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);
    frame.render_widget(
        Paragraph::new("voltkit").style(Style::new().add_modifier(Modifier::BOLD)),
        title,
    );
    let block = Block::bordered().title("Run this?");
    let inner = block.inner(body);
    frame.render_widget(block, body);
    match wizard::describe(plan) {
        Ok(text) => frame.render_widget(
            Paragraph::new(labeled_rows(&text)).wrap(Wrap { trim: false }),
            inner,
        ),
        Err(err) => frame.render_widget(
            Paragraph::new(err)
                .style(Style::new().fg(Color::Red))
                .wrap(Wrap { trim: false }),
            inner,
        ),
    }
    frame.render_widget(
        Paragraph::new("y runs the reads. n goes back to the menu. Esc returns to the adapter.")
            .style(Style::new().dim()),
        footer,
    );
}

fn labeled_rows(text: &str) -> String {
    const LABELS: [&str; 4] = ["What", "Reads", "Command", "Profile"];
    let width = LABELS
        .iter()
        .map(|label| label.len())
        .max()
        .unwrap_or_default()
        + 2;
    let mut blocks = text.splitn(LABELS.len(), '\n');
    LABELS
        .into_iter()
        .map_while(|label| blocks.next().map(|value| format!("{label:<width$}{value}")))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn y_runs_and_n_declines() {
        assert_eq!(on_key(KeyCode::Char('y')), Some(ConfirmEvent::Run));
        assert_eq!(on_key(KeyCode::Char('n')), Some(ConfirmEvent::Decline));
        assert_eq!(on_key(KeyCode::Esc), Some(ConfirmEvent::Back));
        assert_eq!(on_key(KeyCode::Enter), Some(ConfirmEvent::Run));
    }
}
