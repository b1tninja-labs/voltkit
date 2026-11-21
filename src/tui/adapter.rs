//! Adapter address, baud, and the profile path.

use std::path::PathBuf;

use crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Paragraph, Wrap};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ready {
    pub target: String,
    pub baud: Option<u32>,
    pub output: Option<PathBuf>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum AdapterEvent {
    Back,
    Ready(Ready),
}

pub struct Adapter {
    pub target: String,
    pub baud: String,
    pub output: String,
    pub field: usize,
    pub error: String,
}

impl Adapter {
    pub fn new() -> Self {
        Self {
            target: String::new(),
            baud: String::new(),
            output: String::new(),
            field: 0,
            error: String::new(),
        }
    }

    pub fn on_key(&mut self, code: KeyCode) -> Option<AdapterEvent> {
        match code {
            KeyCode::Esc => Some(AdapterEvent::Back),
            KeyCode::Backspace => {
                self.error.clear();
                self.current().pop();
                None
            }
            KeyCode::Char(ch) => {
                self.error.clear();
                self.current().push(ch);
                None
            }
            KeyCode::Tab | KeyCode::Down => {
                self.advance();
                None
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.retreat();
                None
            }
            KeyCode::Enter => self.enter(),
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
            Paragraph::new("voltkit").style(Style::new().add_modifier(Modifier::BOLD)),
            title,
        );
        let mut lines = vec![field_line(
            "Adapter",
            &self.target,
            "192.168.0.10:35000 or COM5",
            self.field == 0,
        )];
        if self.asks_baud() {
            lines.push(field_line("Baud", &self.baud, "115200", self.field == 1));
        }
        lines.push(field_line(
            "Profile file",
            &self.output,
            "enter prints it",
            self.field == 2,
        ));
        if !self.error.is_empty() {
            lines.push(
                Line::from(self.error.clone()).style(Style::new().fg(ratatui::style::Color::Red)),
            );
        }
        frame.render_widget(
            Paragraph::new(Text::from(lines))
                .block(Block::bordered().title("Where is the adapter?"))
                .wrap(Wrap { trim: false }),
            body,
        );
        frame.render_widget(
            Paragraph::new("Tab moves. Enter continues. Esc back.").style(Style::new().dim()),
            footer,
        );
    }

    fn asks_baud(&self) -> bool {
        self.target.to_ascii_uppercase().starts_with("COM")
    }

    fn current(&mut self) -> &mut String {
        match self.field {
            0 => &mut self.target,
            1 => &mut self.baud,
            _ => &mut self.output,
        }
    }

    fn advance(&mut self) {
        self.field = match self.field {
            0 if self.asks_baud() => 1,
            0 | 1 => 2,
            _ => 2,
        };
    }

    fn retreat(&mut self) {
        self.field = match self.field {
            2 if self.asks_baud() => 1,
            2 | 1 => 0,
            _ => 0,
        };
    }

    fn enter(&mut self) -> Option<AdapterEvent> {
        if self.field != 2 {
            if self.field == 0 {
                match normalize_target(&self.target) {
                    Ok(target) => self.target = target,
                    Err(err) => {
                        self.error = err;
                        return None;
                    }
                }
            }
            if self.field == 1 {
                if let Err(err) = parse_baud(&self.baud) {
                    self.error = err;
                    return None;
                }
            }
            self.error.clear();
            self.advance();
            return None;
        }
        self.submit()
    }

    fn submit(&mut self) -> Option<AdapterEvent> {
        let target = match normalize_target(&self.target) {
            Ok(target) => target,
            Err(err) => {
                self.error = err;
                self.field = 0;
                return None;
            }
        };
        let baud = if target.starts_with("COM") {
            match parse_baud(&self.baud) {
                Ok(baud) => baud,
                Err(err) => {
                    self.error = err;
                    self.field = 1;
                    return None;
                }
            }
        } else {
            None
        };
        let output = match parse_output(&self.output) {
            Ok(output) => output,
            Err(err) => {
                self.error = err;
                return None;
            }
        };
        self.target = target.clone();
        self.error.clear();
        Some(AdapterEvent::Ready(Ready {
            target,
            baud,
            output,
        }))
    }
}

fn field_line(label: &str, value: &str, hint: &str, active: bool) -> Line<'static> {
    let shown = if value.is_empty() {
        format!("({hint})")
    } else {
        value.to_string()
    };
    let row = format!("{label:<14} {shown}");
    if active {
        Line::from(row).style(Style::new().add_modifier(Modifier::REVERSED))
    } else {
        Line::from(row)
    }
}

fn normalize_target(text: &str) -> Result<String, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("Enter a host and port, or a COM port.".to_string());
    }
    let chosen = if trimmed.to_ascii_uppercase().starts_with("COM") {
        trimmed.to_ascii_uppercase()
    } else {
        trimmed.to_string()
    };
    if !chosen.contains(':') && !chosen.starts_with("COM") {
        return Err("Enter a host and port, or a COM port.".to_string());
    }
    #[cfg(not(windows))]
    if chosen.starts_with("COM") {
        return Err("A COM port is Windows only. Enter a host and port.".to_string());
    }
    Ok(chosen)
}

fn parse_baud(text: &str) -> Result<Option<u32>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    match text.parse::<u32>() {
        Ok(value) if value > 0 => Ok(if value == 115_200 { None } else { Some(value) }),
        _ => Err("Baud is a number, such as 115200.".to_string()),
    }
}

fn parse_output(text: &str) -> Result<Option<PathBuf>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let path = PathBuf::from(text);
    if path.is_dir() {
        return Err(format!("{} is a directory.", path.display()));
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            return Err(format!("Directory {} does not exist.", parent.display()));
        }
    }
    Ok(Some(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn type_text(form: &mut Adapter, text: &str) {
        for ch in text.chars() {
            assert!(form.on_key(KeyCode::Char(ch)).is_none());
        }
    }

    #[test]
    fn a_host_prints_the_profile() {
        let mut form = Adapter::new();
        type_text(&mut form, "192.168.0.10:35000");
        assert!(form.on_key(KeyCode::Enter).is_none());
        match form.on_key(KeyCode::Enter) {
            Some(AdapterEvent::Ready(ready)) => {
                assert_eq!(ready.target, "192.168.0.10:35000");
                assert_eq!(ready.baud, None);
                assert!(ready.output.is_none());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn escape_goes_back() {
        let mut form = Adapter::new();
        assert_eq!(form.on_key(KeyCode::Esc), Some(AdapterEvent::Back));
    }

    #[cfg(windows)]
    #[test]
    fn a_com_port_keeps_a_custom_baud() {
        let mut form = Adapter::new();
        type_text(&mut form, "com5");
        assert!(form.on_key(KeyCode::Enter).is_none());
        type_text(&mut form, "38400");
        assert!(form.on_key(KeyCode::Enter).is_none());
        match form.on_key(KeyCode::Enter) {
            Some(AdapterEvent::Ready(ready)) => {
                assert_eq!(ready.target, "COM5");
                assert_eq!(ready.baud, Some(38_400));
            }
            other => panic!("{other:?}"),
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn a_com_port_is_refused_off_windows() {
        let mut form = Adapter::new();
        type_text(&mut form, "COM5");
        assert!(form.on_key(KeyCode::Enter).is_none());
        assert!(form.error.contains("Windows only"));
    }
}
