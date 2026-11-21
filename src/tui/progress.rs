//! Adapter dialogue while a capture is in flight, then the result.

use crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Paragraph, Wrap};

#[derive(Debug, PartialEq, Eq)]
pub enum ProgressEvent {
    Again,
    Quit,
}

pub struct Progress {
    pub lines: Vec<String>,
    pub answered: u32,
    pub negative: u32,
    pub silent: u32,
    pub done: bool,
    pub error: Option<String>,
    pub wrote: Option<String>,
    pub heading: String,
}

impl Progress {
    pub fn new(heading: String) -> Self {
        Self {
            lines: Vec::new(),
            answered: 0,
            negative: 0,
            silent: 0,
            done: false,
            error: None,
            wrote: None,
            heading,
        }
    }

    pub fn on_key(&self, code: KeyCode) -> Option<ProgressEvent> {
        if !self.done {
            return None;
        }
        match code {
            KeyCode::Enter | KeyCode::Char('y' | 'Y') => Some(ProgressEvent::Again),
            KeyCode::Char('q') | KeyCode::Esc => Some(ProgressEvent::Quit),
            _ => None,
        }
    }

    pub fn draw(&self, frame: &mut ratatui::Frame, area: Rect) {
        let [title, body, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(area);
        let title_text = if self.done {
            "Finished"
        } else {
            "Reading the adapter"
        };
        frame.render_widget(
            Paragraph::new(title_text).style(Style::new().add_modifier(Modifier::BOLD)),
            title,
        );
        let block = Block::bordered().title(self.counts());
        let inner = block.inner(body);
        frame.render_widget(&block, body);
        let height = inner.height as usize;
        let start = self.lines.len().saturating_sub(height.max(1));
        let shown: Vec<Line> = self.lines[start..]
            .iter()
            .map(|line| log_line(line))
            .collect();
        frame.render_widget(
            Paragraph::new(Text::from(shown)).wrap(Wrap { trim: false }),
            inner,
        );
        let help = if let Some(err) = &self.error {
            err.clone()
        } else if !self.done {
            self.heading.clone()
        } else if let Some(path) = &self.wrote {
            format!("Wrote {path}. Enter for another capture. q quits.")
        } else {
            "The profile is printed when you quit. Enter for another capture. q quits.".to_string()
        };
        frame.render_widget(Paragraph::new(help).style(Style::new().dim()), footer);
    }

    fn counts(&self) -> String {
        format!(
            "answered {}  negative {}  silent {}",
            self.answered, self.negative, self.silent
        )
    }
}

fn log_line(text: &str) -> Line<'_> {
    let style = if text.starts_with("> ") {
        Style::new().add_modifier(Modifier::DIM)
    } else if text.starts_with("= ") {
        Style::new().fg(Color::Green)
    } else if text.starts_with("# ") {
        Style::new().fg(Color::Cyan)
    } else if text.starts_with("< ")
        && (text.contains("NO DATA") || text.contains("ERROR") || text.contains("UNABLE"))
    {
        Style::new().fg(Color::Yellow)
    } else {
        Style::new()
    };
    Line::from(text).style(style)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_wait_until_the_capture_finishes() {
        let mut progress =
            Progress::new("voltkit profile 192.168.0.10:35000 --set all --clone".to_string());
        assert!(progress.on_key(KeyCode::Char('q')).is_none());
        progress.done = true;
        assert_eq!(progress.on_key(KeyCode::Enter), Some(ProgressEvent::Again));
        assert_eq!(
            progress.on_key(KeyCode::Char('q')),
            Some(ProgressEvent::Quit)
        );
    }
}
