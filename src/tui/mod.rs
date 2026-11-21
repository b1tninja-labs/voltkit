//! Full-screen wizard. The line shell in `wizard` stays for a pipe and for tests.

mod adapter;
mod confirm;
mod menu;
mod progress;

use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use voltkit::wizard::{self, Plan};

use adapter::AdapterEvent;
use confirm::ConfirmEvent;
use menu::MenuEvent;

enum Screen {
    Menu,
    Adapter,
    Confirm,
    Progress,
}

struct App {
    screen: Screen,
    menu: menu::Menu,
    adapter: adapter::Adapter,
    choice: Option<menu::MenuChoice>,
    plan: Option<Plan>,
    progress: progress::Progress,
    pending_stdout: Option<String>,
    quit: bool,
}

impl App {
    fn new() -> Self {
        Self {
            screen: Screen::Menu,
            menu: menu::Menu::new(),
            adapter: adapter::Adapter::new(),
            choice: None,
            plan: None,
            progress: progress::Progress::new(String::new()),
            pending_stdout: None,
            quit: false,
        }
    }

    fn loop_events(&mut self, terminal: &mut DefaultTerminal) -> Result<(), String> {
        loop {
            terminal
                .draw(|frame| self.draw(frame))
                .map_err(|err| err.to_string())?;
            if matches!(self.screen, Screen::Progress) && !self.progress.done {
                self.capture(terminal)?;
                continue;
            }
            if !event::poll(Duration::from_millis(200)).map_err(|err| err.to_string())? {
                if self.quit {
                    break;
                }
                continue;
            }
            let event = event::read().map_err(|err| err.to_string())?;
            if let Event::Key(key) = event {
                if key.kind == KeyEventKind::Release {
                    continue;
                }
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    self.quit = true;
                } else {
                    self.on_key(key.code);
                }
            }
            if self.quit {
                break;
            }
        }
        Ok(())
    }

    fn draw(&self, frame: &mut ratatui::Frame) {
        let area = frame.area();
        match self.screen {
            Screen::Menu => self.menu.draw(frame, area),
            Screen::Adapter => self.adapter.draw(frame, area),
            Screen::Confirm => {
                if let Some(plan) = &self.plan {
                    confirm::draw(frame, area, plan);
                }
            }
            Screen::Progress => self.progress.draw(frame, area),
        }
    }

    fn on_key(&mut self, code: KeyCode) {
        match self.screen {
            Screen::Menu => self.on_menu(code),
            Screen::Adapter => self.on_adapter(code),
            Screen::Confirm => self.on_confirm(code),
            Screen::Progress => self.on_progress(code),
        }
    }

    fn on_menu(&mut self, code: KeyCode) {
        match self.menu.on_key(code) {
            Some(MenuEvent::Quit) => self.quit = true,
            Some(MenuEvent::Chosen(choice)) => {
                self.choice = Some(choice);
                self.adapter.error.clear();
                self.screen = Screen::Adapter;
            }
            None => {}
        }
    }

    fn on_adapter(&mut self, code: KeyCode) {
        match self.adapter.on_key(code) {
            Some(AdapterEvent::Back) => self.screen = Screen::Menu,
            Some(AdapterEvent::Ready(ready)) => {
                let Some(choice) = self.choice.clone() else {
                    self.screen = Screen::Menu;
                    return;
                };
                match plan_from(&choice, &ready) {
                    Ok(plan) => match wizard::describe(&plan) {
                        Ok(_) => {
                            self.plan = Some(plan);
                            self.screen = Screen::Confirm;
                        }
                        Err(err) => self.adapter.error = err,
                    },
                    Err(err) => self.adapter.error = err,
                }
            }
            None => {}
        }
    }

    fn on_confirm(&mut self, code: KeyCode) {
        match confirm::on_key(code) {
            Some(ConfirmEvent::Run) => {
                let heading = self
                    .plan
                    .as_ref()
                    .map(wizard::command_line)
                    .unwrap_or_default();
                self.progress = progress::Progress::new(heading);
                self.screen = Screen::Progress;
            }
            Some(ConfirmEvent::Decline) => self.screen = Screen::Menu,
            Some(ConfirmEvent::Back) => self.screen = Screen::Adapter,
            None => {}
        }
    }

    fn on_progress(&mut self, code: KeyCode) {
        match self.progress.on_key(code) {
            Some(progress::ProgressEvent::Again) => {
                self.progress = progress::Progress::new(String::new());
                self.screen = Screen::Menu;
            }
            Some(progress::ProgressEvent::Quit) => self.quit = true,
            None => {}
        }
    }

    fn capture(&mut self, terminal: &mut DefaultTerminal) -> Result<(), String> {
        let Some(plan) = self.plan.clone() else {
            self.progress.error = Some("no plan".to_string());
            self.progress.done = true;
            return Ok(());
        };
        let args = super::profile_args(&plan);
        let result = {
            let progress = &mut self.progress;
            super::run_capture_logged(&args, |line| {
                progress.lines.push(line.to_string());
                let _ = terminal.draw(|frame| {
                    let area = frame.area();
                    progress.draw(frame, area);
                });
            })
        };
        match result {
            Ok(report) => {
                self.progress.answered = report.answered;
                self.progress.negative = report.negative;
                self.progress.silent = report.silent;
                self.progress.lines.push(format!(
                    "# answered {}, negative {}, silent {}",
                    report.answered, report.negative, report.silent
                ));
                if let Some(text) = report.profile {
                    self.keep_profile(&plan, text);
                }
            }
            Err(err) => self.progress.error = Some(err),
        }
        self.progress.done = true;
        Ok(())
    }

    fn keep_profile(&mut self, plan: &Plan, text: String) {
        if let Some(path) = &plan.output {
            match super::write_profile(path, &text) {
                Ok(()) => {
                    self.progress.wrote = Some(path.display().to_string());
                    self.pending_stdout = None;
                }
                Err(err) => {
                    self.progress.error = Some(err);
                    self.pending_stdout = Some(text);
                }
            }
        } else {
            self.pending_stdout = Some(text);
        }
    }
}

fn plan_from(choice: &menu::MenuChoice, ready: &adapter::Ready) -> Result<Plan, String> {
    let (sets, ranges) = match choice {
        menu::MenuChoice::Sets(names) => (
            names.iter().map(|name| (*name).to_string()).collect(),
            Vec::new(),
        ),
        menu::MenuChoice::Screen(name) => (vec![(*name).to_string()], Vec::new()),
        menu::MenuChoice::Range(range) => (Vec::new(), vec![*range]),
    };
    Ok(Plan {
        target: ready.target.clone(),
        baud: ready.baud,
        sets,
        ranges,
        clone: true,
        output: ready.output.clone(),
    })
}

pub fn run() -> Result<(), String> {
    let mut app = App::new();
    let outcome = ratatui::run(|terminal| app.loop_events(terminal));
    if let Some(text) = app.pending_stdout.take() {
        print!("{text}");
    }
    outcome
}
