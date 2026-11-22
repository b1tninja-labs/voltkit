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

#[derive(Debug, PartialEq, Eq)]
pub enum StageEvent {
    /// Run the live read with the chosen backend.
    Run(Backend),
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
    /// The backend that produced [`result`](Self::result), for the header.
    pub ran: Option<Backend>,
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

    pub fn on_key(&self, code: KeyCode) -> Option<StageEvent> {
        if self.done() {
            return match code {
                KeyCode::Enter | KeyCode::Char('q') => Some(StageEvent::Done),
                KeyCode::Esc => Some(StageEvent::Back),
                _ => None,
            };
        }
        match code {
            // Native is the default (Enter / N); the vendor DLL is the backup (V).
            KeyCode::Enter | KeyCode::Char('n' | 'N') => Some(StageEvent::Run(Backend::Native)),
            KeyCode::Char('v' | 'V') => Some(StageEvent::Run(Backend::Vendor)),
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
                "{}\n{}\n\nChoose the backend for the live read (read-only: $3E TesterPresent, \
                 then the $1A identity DIDs):\n\n  [N] native VCX  — our own read over the serial \
                 link (gmdiag vcx-read)\n  [V] vendor DLL  — VXDIAG VCXPT32.dll via our J2534 \
                 loader (the backup)\n\nNative is first-class; the vendor path is there so the \
                 read works no matter what.",
                self.label, self.detail
            ),
            Some(Ok(output)) => format!(
                "Backend: {}\n\n{output}",
                self.ran.map_or("", Backend::label)
            ),
            Some(Err(err)) => format!(
                "Backend: {}\n\nCould not run the read: {err}",
                self.ran.map_or("", Backend::label)
            ),
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
            "[N] native read   [V] vendor-DLL read   Esc back"
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
    fn n_runs_native_v_runs_vendor_and_esc_goes_back() {
        let stage = Stage::new(&vcx());
        assert_eq!(stage.port, 4);
        assert_eq!(
            stage.on_key(KeyCode::Enter),
            Some(StageEvent::Run(Backend::Native))
        );
        assert_eq!(
            stage.on_key(KeyCode::Char('n')),
            Some(StageEvent::Run(Backend::Native))
        );
        assert_eq!(
            stage.on_key(KeyCode::Char('v')),
            Some(StageEvent::Run(Backend::Vendor))
        );
        assert_eq!(stage.on_key(KeyCode::Esc), Some(StageEvent::Back));
    }

    #[test]
    fn once_run_enter_returns_and_esc_repicks() {
        let mut stage = Stage::new(&vcx());
        stage.ran = Some(Backend::Native);
        stage.result = Some(Ok("TesterPresent: positive $7E".to_string()));
        assert_eq!(stage.on_key(KeyCode::Enter), Some(StageEvent::Done));
        assert_eq!(stage.on_key(KeyCode::Esc), Some(StageEvent::Back));
    }

    #[test]
    fn backend_labels_name_the_source() {
        assert!(Backend::Native.label().contains("native"));
        assert!(Backend::Vendor.label().contains("VCXPT32"));
    }
}
