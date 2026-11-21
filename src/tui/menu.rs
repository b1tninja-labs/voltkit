//! Job list, screen list, and the range form.

use crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph, Wrap};
use voltkit::wizard;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuChoice {
    Sets(&'static [&'static str]),
    Screen(&'static str),
    Range((u16, u8, u16, u16)),
}

#[derive(Debug, PartialEq, Eq)]
pub enum MenuEvent {
    Quit,
    Chosen(MenuChoice),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Jobs,
    Screens,
    Range,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RangeForm {
    pub header: String,
    pub service: String,
    pub start: String,
    pub end: String,
    pub field: usize,
    pub error: String,
}

impl RangeForm {
    fn new() -> Self {
        Self {
            header: String::new(),
            service: String::new(),
            start: String::new(),
            end: String::new(),
            field: 0,
            error: String::new(),
        }
    }
}

pub struct Menu {
    pub jobs: usize,
    pub screens: usize,
    mode: Mode,
    range: RangeForm,
}

impl Menu {
    pub fn new() -> Self {
        Self {
            jobs: 0,
            screens: 0,
            mode: Mode::Jobs,
            range: RangeForm::new(),
        }
    }

    pub fn on_key(&mut self, code: KeyCode) -> Option<MenuEvent> {
        match self.mode {
            Mode::Jobs => self.on_jobs(code),
            Mode::Screens => self.on_screens(code),
            Mode::Range => self.on_range(code),
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
        match self.mode {
            Mode::Jobs => self.draw_jobs(frame, body),
            Mode::Screens => self.draw_screens(frame, body),
            Mode::Range => self.draw_range(frame, body),
        }
        let help = match self.mode {
            Mode::Jobs => "1-5 or Enter selects. q quits.",
            Mode::Screens => "Up and Down move. Enter selects. Esc back. q quits.",
            Mode::Range => "Hex in the field. Enter next. Esc back. q quits.",
        };
        frame.render_widget(Paragraph::new(help).style(Style::new().dim()), footer);
    }

    fn on_jobs(&mut self, code: KeyCode) -> Option<MenuEvent> {
        let last = wizard::guides().len() + 1;
        match code {
            KeyCode::Up => self.jobs = self.jobs.saturating_sub(1),
            KeyCode::Down if self.jobs < last => self.jobs += 1,
            KeyCode::Enter => return self.activate_job(),
            KeyCode::Char(ch @ '1'..='5') => {
                let index = usize::from(ch as u8 - b'1');
                if index <= last {
                    self.jobs = index;
                    return self.activate_job();
                }
            }
            KeyCode::Char('q') | KeyCode::Esc => return Some(MenuEvent::Quit),
            _ => {}
        }
        None
    }

    fn activate_job(&mut self) -> Option<MenuEvent> {
        let guides = wizard::guides();
        if self.jobs < guides.len() {
            return Some(MenuEvent::Chosen(MenuChoice::Sets(guides[self.jobs].sets)));
        }
        if self.jobs == guides.len() {
            self.mode = Mode::Screens;
            return None;
        }
        self.mode = Mode::Range;
        self.range = RangeForm::new();
        None
    }

    fn on_screens(&mut self, code: KeyCode) -> Option<MenuEvent> {
        let last = wizard::screens().len().saturating_sub(1);
        match code {
            KeyCode::Up => self.screens = self.screens.saturating_sub(1),
            KeyCode::Down if self.screens < last => self.screens += 1,
            KeyCode::Enter => {
                let name = wizard::screens().get(self.screens)?.0;
                return Some(MenuEvent::Chosen(MenuChoice::Screen(name)));
            }
            KeyCode::Char(ch @ '1'..='9') => {
                let index = usize::from(ch as u8 - b'1');
                if index < wizard::screens().len() {
                    self.screens = index;
                    let name = wizard::screens()[index].0;
                    return Some(MenuEvent::Chosen(MenuChoice::Screen(name)));
                }
            }
            KeyCode::Esc => self.mode = Mode::Jobs,
            KeyCode::Char('q') => return Some(MenuEvent::Quit),
            _ => {}
        }
        None
    }

    fn on_range(&mut self, code: KeyCode) -> Option<MenuEvent> {
        match code {
            KeyCode::Esc => self.mode = Mode::Jobs,
            KeyCode::Char('q') => return Some(MenuEvent::Quit),
            KeyCode::Backspace => {
                self.range.error.clear();
                self.range_field().pop();
            }
            KeyCode::Char(ch) if ch.is_ascii_hexdigit() => {
                self.range.error.clear();
                let max = if self.range.field == 1 { 2 } else { 4 };
                let field = self.range_field();
                if field.len() < max {
                    field.push(ch.to_ascii_uppercase());
                }
            }
            KeyCode::Enter | KeyCode::Tab => return self.commit_range_field(),
            KeyCode::BackTab | KeyCode::Up if self.range.field > 0 => {
                self.range.field -= 1;
            }
            _ => {}
        }
        None
    }

    fn range_field(&mut self) -> &mut String {
        match self.range.field {
            0 => &mut self.range.header,
            1 => &mut self.range.service,
            2 => &mut self.range.start,
            _ => &mut self.range.end,
        }
    }

    fn commit_range_field(&mut self) -> Option<MenuEvent> {
        self.range.error.clear();
        match self.range.field {
            0 => match parse_header(&self.range.header) {
                Ok(_) => self.range.field = 1,
                Err(err) => self.range.error = err,
            },
            1 => match parse_service(&self.range.service) {
                Ok(_) => self.range.field = 2,
                Err(err) => self.range.error = err,
            },
            2 => match u16::from_str_radix(self.range.start.trim(), 16) {
                Ok(_) => self.range.field = 3,
                Err(_) => self.range.error = "The identifier is hex.".to_string(),
            },
            _ => match finish_range(&self.range) {
                Ok(range) => {
                    return Some(MenuEvent::Chosen(MenuChoice::Range(range)));
                }
                Err(err) => self.range.error = err,
            },
        }
        None
    }

    fn draw_jobs(&self, frame: &mut ratatui::Frame, area: Rect) {
        let [left, right] =
            Layout::horizontal([Constraint::Percentage(46), Constraint::Fill(1)]).areas(area);
        let mut items: Vec<ListItem> = wizard::guides()
            .iter()
            .enumerate()
            .map(|(index, guide)| ListItem::new(format!("  {}  {}", index + 1, guide.title)))
            .collect();
        let screen = wizard::guides().len() + 1;
        items.push(ListItem::new(format!("  {screen}  Read one screen")));
        items.push(ListItem::new(format!(
            "  {}  Walk a range of identifiers",
            screen + 1
        )));
        let list = List::new(items)
            .block(Block::bordered().title("What do you want to do?"))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol(">");
        let mut state = ListState::default().with_selected(Some(self.jobs));
        frame.render_stateful_widget(list, left, &mut state);
        let pane = match self.job_command() {
            Some(command) => format!("{}\n\n{command}", self.job_detail()),
            None => self.job_detail(),
        };
        frame.render_widget(
            Paragraph::new(pane)
                .block(Block::bordered().title("This asks"))
                .wrap(Wrap { trim: false }),
            right,
        );
    }

    fn job_detail(&self) -> String {
        let guides = wizard::guides();
        if self.jobs < guides.len() {
            return guides[self.jobs].detail.to_string();
        }
        if self.jobs == guides.len() {
            return "Asks one named screen from the lab profile.".to_string();
        }
        "Walks one header from the first identifier through the last. At most 256. The service is 22 or 1A.".to_string()
    }

    fn job_command(&self) -> Option<String> {
        let guide = wizard::guides().get(self.jobs)?;
        let plan = wizard::Plan {
            target: "ADAPTER".to_string(),
            baud: None,
            sets: guide.sets.iter().map(|name| (*name).to_string()).collect(),
            ranges: Vec::new(),
            clone: true,
            output: None,
        };
        Some(wizard::command_line(&plan))
    }

    fn draw_screens(&self, frame: &mut ratatui::Frame, area: Rect) {
        let items: Vec<ListItem> = wizard::screens()
            .iter()
            .enumerate()
            .map(|(index, (_, title))| ListItem::new(format!("  {}  {title}", index + 1)))
            .collect();
        let list = List::new(items)
            .block(Block::bordered().title("Which screen?"))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol(">");
        let mut state = ListState::default().with_selected(Some(self.screens));
        frame.render_stateful_widget(list, area, &mut state);
    }

    fn draw_range(&self, frame: &mut ratatui::Frame, area: Rect) {
        let labels = [
            ("Header", self.range.header.as_str(), "7E4"),
            ("Service, 22 or 1A", self.range.service.as_str(), "22"),
            ("First identifier", self.range.start.as_str(), "4181"),
            ("Last identifier", self.range.end.as_str(), "4200"),
        ];
        let mut lines = Vec::new();
        for (index, (label, value, example)) in labels.iter().enumerate() {
            let shown = if value.is_empty() {
                format!("({example})")
            } else {
                (*value).to_string()
            };
            let row = format!("{label:<20} {shown}");
            if index == self.range.field {
                lines.push(
                    ratatui::text::Line::from(row)
                        .style(Style::new().add_modifier(Modifier::REVERSED)),
                );
            } else {
                lines.push(ratatui::text::Line::from(row));
            }
        }
        if !self.range.error.is_empty() {
            lines.push(
                ratatui::text::Line::from(self.range.error.clone())
                    .style(Style::new().fg(ratatui::style::Color::Red)),
            );
        }
        frame.render_widget(
            Paragraph::new(ratatui::text::Text::from(lines))
                .block(Block::bordered().title("Walk a range"))
                .wrap(Wrap { trim: false }),
            area,
        );
    }
}

fn parse_header(text: &str) -> Result<u16, String> {
    let header = u16::from_str_radix(text.trim(), 16)
        .map_err(|_| "The header is hex, such as 7E4.".to_string())?;
    if header == 0 || header > 0x7FF {
        return Err("The header is an 11-bit id.".to_string());
    }
    Ok(header)
}

fn parse_service(text: &str) -> Result<u8, String> {
    match text.trim().to_ascii_uppercase().as_str() {
        "22" => Ok(0x22),
        "1A" => Ok(0x1A),
        "27" => Err("Security access is not sent.".to_string()),
        _ => Err("The service is 22 or 1A.".to_string()),
    }
}

fn finish_range(form: &RangeForm) -> Result<(u16, u8, u16, u16), String> {
    let header = parse_header(&form.header)?;
    let service = parse_service(&form.service)?;
    let start = u16::from_str_radix(form.start.trim(), 16)
        .map_err(|_| "The identifier is hex.".to_string())?;
    let end = u16::from_str_radix(form.end.trim(), 16)
        .map_err(|_| "The identifier is hex.".to_string())?;
    voltkit::range_rows(header, service, start, end)?;
    Ok((header, service, start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn type_hex(menu: &mut Menu, text: &str) {
        for ch in text.chars() {
            assert!(menu.on_key(KeyCode::Char(ch)).is_none());
        }
    }

    #[test]
    fn the_first_job_is_the_lab_profile() {
        let mut menu = Menu::new();
        match menu.on_key(KeyCode::Enter) {
            Some(MenuEvent::Chosen(MenuChoice::Sets(sets))) => assert_eq!(sets, &["all"]),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn programming_is_the_second_job() {
        let mut menu = Menu::new();
        match menu.on_key(KeyCode::Char('2')) {
            Some(MenuEvent::Chosen(MenuChoice::Sets(sets))) => {
                assert_eq!(sets, &["parts", "sps-extra", "hardware", "identity"]);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_screen_choice_names_the_charger() {
        let mut menu = Menu::new();
        assert!(menu.on_key(KeyCode::Char('4')).is_none());
        match menu.on_key(KeyCode::Enter) {
            Some(MenuEvent::Chosen(MenuChoice::Screen(name))) => assert_eq!(name, "charger"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn security_access_stays_on_the_form() {
        let mut menu = Menu::new();
        assert!(menu.on_key(KeyCode::Char('5')).is_none());
        type_hex(&mut menu, "7E4");
        assert!(menu.on_key(KeyCode::Enter).is_none());
        type_hex(&mut menu, "27");
        let event = menu.on_key(KeyCode::Enter);
        assert!(event.is_none());
        assert!(menu.range.error.contains("Security access is not sent."));
    }

    #[test]
    fn a_range_returns_the_span() {
        let mut menu = Menu::new();
        assert!(menu.on_key(KeyCode::Char('5')).is_none());
        type_hex(&mut menu, "7E7");
        assert!(menu.on_key(KeyCode::Enter).is_none());
        type_hex(&mut menu, "22");
        assert!(menu.on_key(KeyCode::Enter).is_none());
        type_hex(&mut menu, "4181");
        assert!(menu.on_key(KeyCode::Enter).is_none());
        type_hex(&mut menu, "4200");
        match menu.on_key(KeyCode::Enter) {
            Some(MenuEvent::Chosen(MenuChoice::Range(range))) => {
                assert_eq!(range, (0x7E7, 0x22, 0x4181, 0x4200));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn escape_leaves_the_range_and_q_quits() {
        let mut menu = Menu::new();
        assert!(menu.on_key(KeyCode::Char('5')).is_none());
        assert!(menu.on_key(KeyCode::Esc).is_none());
        assert_eq!(menu.on_key(KeyCode::Char('q')), Some(MenuEvent::Quit));
    }
}
