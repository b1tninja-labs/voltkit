//! Detect the adapters present and let the operator pick one, instead of typing
//! a COM port.
//!
//! The scan runs on a background thread so the list stays responsive: the picker
//! shows "scanning" until the devices arrive, with "Enter an address manually"
//! and "Rescan" always available. Choosing a capturable device goes straight to
//! the confirm screen with the profile path already chosen; a VCX-NANO is shown
//! for identification only and does not start a capture.

use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph, Wrap};
use voltkit::detect::{self, Device};

/// What the operator chose on the picker.
#[derive(Debug, PartialEq, Eq)]
pub enum DetectEvent {
    /// Esc: go back to the job menu.
    Back,
    /// Type an address by hand instead.
    Manual,
    /// A capturable device was selected.
    Picked(Device),
}

/// The BLE scan bound. Short, so the picker is not slow; a rescan can be run.
const SCAN_TIMEOUT: Duration = Duration::from_secs(4);

pub struct Picker {
    devices: Vec<Device>,
    index: usize,
    rx: Option<Receiver<Vec<Device>>>,
    note: String,
    remembered: Option<String>,
}

impl Picker {
    pub fn new(remembered: Option<String>) -> Self {
        let mut picker = Self {
            devices: Vec::new(),
            index: 0,
            rx: None,
            note: String::new(),
            remembered,
        };
        picker.rescan();
        picker
    }

    /// Start a background scan. The list shows what is known meanwhile.
    pub fn rescan(&mut self) {
        let (tx, rx) = mpsc::channel();
        let remembered = self.remembered.clone();
        std::thread::spawn(move || {
            let found = detect::devices(SCAN_TIMEOUT, remembered.as_deref());
            let _ = tx.send(found);
        });
        self.rx = Some(rx);
        self.note = "Scanning for adapters…".to_string();
    }

    /// Collect a finished scan, if one is ready. Called once per UI tick.
    pub fn tick(&mut self) {
        if let Some(rx) = &self.rx {
            if let Ok(found) = rx.try_recv() {
                self.devices = found;
                self.index = self.index.min(self.last());
                self.note = if self.devices.is_empty() {
                    "No adapters detected. Enter an address, or rescan.".to_string()
                } else {
                    format!("{} adapter(s) detected.", self.devices.len())
                };
                self.rx = None;
            }
        }
    }

    fn scanning(&self) -> bool {
        self.rx.is_some()
    }

    /// Replace the footer note, for example to show why a plan was refused.
    pub fn set_note(&mut self, note: String) {
        self.note = note;
    }

    /// The index of the last selectable row: devices, then manual, then rescan.
    fn last(&self) -> usize {
        self.devices.len() + 1
    }

    fn manual_index(&self) -> usize {
        self.devices.len()
    }

    fn rescan_index(&self) -> usize {
        self.devices.len() + 1
    }

    pub fn on_key(&mut self, code: KeyCode) -> Option<DetectEvent> {
        match code {
            KeyCode::Esc => Some(DetectEvent::Back),
            KeyCode::Up => {
                self.index = self.index.saturating_sub(1);
                None
            }
            KeyCode::Down => {
                if self.index < self.last() {
                    self.index += 1;
                }
                None
            }
            KeyCode::Enter => self.activate(),
            _ => None,
        }
    }

    fn activate(&mut self) -> Option<DetectEvent> {
        if self.index == self.manual_index() {
            return Some(DetectEvent::Manual);
        }
        if self.index == self.rescan_index() {
            self.rescan();
            return None;
        }
        let device = self.devices.get(self.index)?.clone();
        if device.link.capturable() {
            Some(DetectEvent::Picked(device))
        } else {
            // A VCX-NANO: identified, but its live read is gated. Show its detail
            // and stay on the picker.
            self.note = format!("{} — {}", device.label, device.detail);
            None
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
            Paragraph::new("voltkit").style(Style::new().add_modifier(Modifier::BOLD)),
            title,
        );
        let [left, right] =
            Layout::horizontal([Constraint::Percentage(55), Constraint::Fill(1)]).areas(body);
        let mut items: Vec<ListItem> = self
            .devices
            .iter()
            .map(|device| ListItem::new(format!("  {}", device.label)))
            .collect();
        items.push(ListItem::new("  Enter an address manually"));
        let rescan = if self.scanning() {
            "  Scanning…"
        } else {
            "  Rescan for adapters"
        };
        items.push(ListItem::new(rescan));
        let list = List::new(items)
            .block(Block::bordered().title("Which adapter?"))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol(">");
        let mut state = ListState::default().with_selected(Some(self.index));
        frame.render_stateful_widget(list, left, &mut state);

        let detail = self
            .devices
            .get(self.index)
            .map(|device| format!("{}\n\n{}", device.label, device.detail))
            .unwrap_or_else(|| {
                if self.index == self.manual_index() {
                    "Type a host and port, or a COM port, on the next screen.".to_string()
                } else {
                    "Run the scan again, for example after plugging the adapter in.".to_string()
                }
            });
        frame.render_widget(
            Paragraph::new(detail)
                .block(Block::bordered().title("Detail"))
                .wrap(Wrap { trim: false }),
            right,
        );
        let footer_line = if self.note.is_empty() {
            "Up and Down move. Enter selects. Esc back.".to_string()
        } else {
            format!("{}  —  Enter selects. Esc back.", self.note)
        };
        frame.render_widget(
            Paragraph::new(footer_line).style(Style::new().fg(Color::Cyan)),
            footer,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltkit::detect::Link;

    fn device(label: &str, link: Link) -> Device {
        Device {
            label: label.to_string(),
            detail: String::new(),
            link,
        }
    }

    fn ready_picker(devices: Vec<Device>) -> Picker {
        Picker {
            devices,
            index: 0,
            rx: None,
            note: String::new(),
            remembered: None,
        }
    }

    #[test]
    fn manual_and_rescan_sit_after_the_devices() {
        let mut picker = ready_picker(vec![device(
            "COM5  OBDLink",
            Link::Serial {
                port: "COM5".to_string(),
                baud: None,
            },
        )]);
        // device(0), manual(1), rescan(2)
        assert_eq!(picker.manual_index(), 1);
        assert_eq!(picker.rescan_index(), 2);
        picker.index = 1;
        assert_eq!(picker.on_key(KeyCode::Enter), Some(DetectEvent::Manual));
    }

    #[test]
    fn a_capturable_device_is_picked_and_a_vcx_only_informs() {
        let mut picker = ready_picker(vec![
            device(
                "COM5  OBDLink",
                Link::Serial {
                    port: "COM5".to_string(),
                    baud: None,
                },
            ),
            device("VCX-NANO on COM4", Link::Vcx { port: 4 }),
        ]);
        picker.index = 0;
        match picker.on_key(KeyCode::Enter) {
            Some(DetectEvent::Picked(d)) => assert!(d.link.capturable()),
            other => panic!("{other:?}"),
        }
        picker.index = 1;
        assert_eq!(picker.on_key(KeyCode::Enter), None);
        assert!(picker.note.contains("VCX-NANO"));
    }

    #[test]
    fn escape_goes_back_and_the_cursor_stops_at_the_ends() {
        let mut picker = ready_picker(Vec::new());
        // No devices: manual(0), rescan(1).
        assert_eq!(picker.on_key(KeyCode::Up), None);
        assert_eq!(picker.index, 0);
        picker.on_key(KeyCode::Down);
        picker.on_key(KeyCode::Down);
        assert_eq!(picker.index, 1, "cannot move past rescan");
        assert_eq!(picker.on_key(KeyCode::Esc), Some(DetectEvent::Back));
    }
}
