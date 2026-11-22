//! Phase 2: identify the VCX-NANO and stage the programming-data collect.
//!
//! This screen runs the SAFE GetInfo doctor and writes a gated skeleton of the
//! SPS read. It sends no vehicle read; the live SPS collect stays gated.

use crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Paragraph, Wrap};
use voltkit::Device;

#[derive(Debug, PartialEq, Eq)]
pub enum StageEvent {
    /// Run the doctor and write the skeleton.
    Run,
    /// Back to the adapter picker.
    Back,
    /// Finished: return to the job menu.
    Done,
}

pub struct Stage {
    label: String,
    detail: String,
    /// The COM port the VCX answered on.
    pub port: u32,
    /// `Ok(path)` written, or `Err(message)`. `None` until it is run.
    pub result: Option<Result<String, String>>,
}

impl Stage {
    pub fn new(device: &Device) -> Self {
        let port = match device.link {
            voltkit::Link::Vcx { port } => port,
            _ => 0,
        };
        Self {
            label: device.label.clone(),
            detail: device.detail.clone(),
            port,
            result: None,
        }
    }

    fn done(&self) -> bool {
        self.result.is_some()
    }

    pub fn on_key(&self, code: KeyCode) -> Option<StageEvent> {
        if self.done() {
            return match code {
                KeyCode::Enter | KeyCode::Char('q') => Some(StageEvent::Done),
                KeyCode::Esc => Some(StageEvent::Back),
                _ => None,
            };
        }
        match code {
            KeyCode::Enter | KeyCode::Char('y' | 'Y') => Some(StageEvent::Run),
            KeyCode::Esc | KeyCode::Char('n' | 'N') => Some(StageEvent::Back),
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
        frame.render_widget(
            Paragraph::new("Phase 2 — programming data (VCX)")
                .style(Style::new().add_modifier(Modifier::BOLD)),
            title,
        );
        let block = Block::bordered().title("Stage the SPS collect");
        let inner = block.inner(body);
        frame.render_widget(block, body);
        let text = match &self.result {
            None => format!(
                "{}\n{}\n\nThis runs the GetInfo doctor and writes a gated skeleton of the \
                 programming-data read. It sends no vehicle read — the live SPS collect stays \
                 gated for owner sign-off.",
                self.label, self.detail
            ),
            Some(Ok(path)) => format!(
                "Staged the gated skeleton.\n\nWrote {path}\n\nNo vehicle read was sent. Fill the \
                 null values once the live SPS collect is ungated, then hand to spskit."
            ),
            Some(Err(err)) => format!("Could not stage: {err}"),
        };
        let style = match &self.result {
            Some(Err(_)) => Style::new().fg(Color::Red),
            _ => Style::new(),
        };
        frame.render_widget(
            Paragraph::new(text).style(style).wrap(Wrap { trim: false }),
            inner,
        );
        let help = if self.done() {
            "Enter returns to the menu. Esc re-picks the adapter."
        } else {
            "Enter stages the skeleton (no vehicle read). Esc back."
        };
        frame.render_widget(Paragraph::new(help).style(Style::new().dim()), footer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltkit::Link;

    fn vcx() -> Device {
        Device {
            label: "VCX-NANO on COM4".to_string(),
            detail: "Phase 2".to_string(),
            link: Link::Vcx { port: 4 },
        }
    }

    #[test]
    fn enter_runs_then_returns_and_esc_goes_back() {
        let mut stage = Stage::new(&vcx());
        assert_eq!(stage.port, 4);
        assert_eq!(stage.on_key(KeyCode::Enter), Some(StageEvent::Run));
        assert_eq!(stage.on_key(KeyCode::Esc), Some(StageEvent::Back));
        stage.result = Some(Ok("C:/captures/sps-collect-x.json".to_string()));
        assert_eq!(stage.on_key(KeyCode::Enter), Some(StageEvent::Done));
        assert_eq!(stage.on_key(KeyCode::Esc), Some(StageEvent::Back));
    }
}
