//! Phase 2: identify the VCX-NANO and run the live programming-data read.
//!
//! The operator chooses the backend: the **native** VCX read (our own
//! implementation, `gmdiag vcx-read` over the USB serial link) or the **vendor**
//! VXDIAG J2534 DLL (`VCXPT32.dll`, driven through our `vcikit-j2534` loader) as the
//! backup. Both do the same read-only job ($3E TesterPresent, then the $1A identity
//! DIDs); nothing here is gated or a skeleton. The native path is first; the vendor
//! path exists so the read works no matter what.

use crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Paragraph, Wrap};
use voltkit::Device;

/// Which backend runs the live read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// Our own native VCX read (`gmdiag vcx-read` over the serial link).
    Native,
    /// The vendor VXDIAG J2534 DLL (`VCXPT32.dll`) via our `vcikit-j2534` loader.
    Vendor,
}

impl Backend {
    /// The short name shown in the result header.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Backend::Native => "native VCX (our read)",
            Backend::Vendor => "vendor VXDIAG DLL (VCXPT32.dll)",
        }
    }
}

/// What to run on the chosen backend. Both are read-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// TesterPresent ($3E) probe only — brings the ISO 15765 channel up and
    /// checks for a positive $7E, without reading any identity. No VIT1 is written.
    Ping,
    /// The full identity read: $3E TesterPresent, then the $1A identity DIDs,
    /// writing the VIT1 saved-read.
    Read,
}

impl Action {
    /// The short name shown in the result header.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Action::Ping => "ping ($3E TesterPresent probe)",
            Action::Read => "identity read ($3E + $1A)",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum StageEvent {
    /// Run the chosen action on the chosen backend.
    Run(Backend, Action),
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
    /// The backend and action that produced [`result`](Self::result), for the header.
    pub ran: Option<(Backend, Action)>,
    /// `Ok(output)` from the read, or `Err(message)`. `None` until it is run.
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
            ran: None,
            result: None,
        }
    }

    fn done(&self) -> bool {
        self.result.is_some()
    }

    /// The "Backend / action" header line shown above the run output.
    fn ran_header(&self) -> String {
        match self.ran {
            Some((backend, action)) => {
                format!("Backend: {}   Action: {}", backend.label(), action.label())
            }
            None => String::new(),
        }
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
            // Adapter is already chosen in the detect screen; here the operator
            // picks what to run. Native is first-class; the vendor DLL is the backup.
            KeyCode::Enter | KeyCode::Char('n' | 'N') => {
                Some(StageEvent::Run(Backend::Native, Action::Read))
            }
            KeyCode::Char('p' | 'P') => Some(StageEvent::Run(Backend::Native, Action::Ping)),
            KeyCode::Char('v' | 'V') => Some(StageEvent::Run(Backend::Vendor, Action::Read)),
            KeyCode::Char('b' | 'B') => Some(StageEvent::Run(Backend::Vendor, Action::Ping)),
            KeyCode::Esc => Some(StageEvent::Back),
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
            Paragraph::new("Phase 2 — live programming-data read (VCX)")
                .style(Style::new().add_modifier(Modifier::BOLD)),
            title,
        );
        let block = Block::bordered().title("Read the VCX identity ($3E / $1A, read-only)");
        let inner = block.inner(body);
        frame.render_widget(block, body);
        let text = match &self.result {
            None => format!(
                "{}\n{}\n\nThe adapter is already selected. Choose what to run (all read-only: \
                 $3E TesterPresent, and for a full read the $1A identity DIDs):\n\n  native VCX  \
                 — our own read over the serial link (gmdiag vcx-read)\n    [N] identity read \
                 ($3E, then $1A — writes the VIT1)\n    [P] ping only ($3E TesterPresent probe, \
                 no VIT1)\n\n  vendor DLL  — VXDIAG VCXPT32.dll via our J2534 loader (the \
                 backup)\n    [V] identity read ($3E, then $1A — writes the VIT1)\n    [B] ping \
                 only ($3E TesterPresent probe, no VIT1)\n\nNative is first-class; the vendor \
                 path is there so the read works no matter what. A ping proves the channel and \
                 the ECU answer before a full read.",
                self.label, self.detail
            ),
            Some(Ok(output)) => format!("{}\n\n{output}", self.ran_header()),
            Some(Err(err)) => format!("{}\n\nCould not run it: {err}", self.ran_header()),
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
            "[N] native read  [P] native ping  [V] vendor read  [B] vendor ping  Esc back"
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
    fn keys_pick_backend_and_action_and_esc_goes_back() {
        let stage = Stage::new(&vcx());
        assert_eq!(stage.port, 4);
        // Enter and N both run the native full read (the default).
        assert_eq!(
            stage.on_key(KeyCode::Enter),
            Some(StageEvent::Run(Backend::Native, Action::Read))
        );
        assert_eq!(
            stage.on_key(KeyCode::Char('n')),
            Some(StageEvent::Run(Backend::Native, Action::Read))
        );
        // P is the native TesterPresent ping.
        assert_eq!(
            stage.on_key(KeyCode::Char('p')),
            Some(StageEvent::Run(Backend::Native, Action::Ping))
        );
        // V / B are the vendor read / ping.
        assert_eq!(
            stage.on_key(KeyCode::Char('v')),
            Some(StageEvent::Run(Backend::Vendor, Action::Read))
        );
        assert_eq!(
            stage.on_key(KeyCode::Char('b')),
            Some(StageEvent::Run(Backend::Vendor, Action::Ping))
        );
        assert_eq!(stage.on_key(KeyCode::Esc), Some(StageEvent::Back));
    }

    #[test]
    fn once_run_enter_returns_and_esc_repicks() {
        let mut stage = Stage::new(&vcx());
        stage.ran = Some((Backend::Native, Action::Ping));
        stage.result = Some(Ok("TesterPresent: positive $7E".to_string()));
        assert_eq!(stage.on_key(KeyCode::Enter), Some(StageEvent::Done));
        assert_eq!(stage.on_key(KeyCode::Esc), Some(StageEvent::Back));
    }

    #[test]
    fn backend_and_action_labels_name_the_source() {
        assert!(Backend::Native.label().contains("native"));
        assert!(Backend::Vendor.label().contains("VCXPT32"));
        assert!(Action::Ping.label().contains("TesterPresent"));
        assert!(Action::Read.label().contains("$1A"));
    }
}
