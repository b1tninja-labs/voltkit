//! Full-screen wizard. The line shell in `wizard` stays for a pipe and for tests.

mod adapter;
mod art;
mod confirm;
mod detect;
mod menu;
mod progress;
mod stage;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::DefaultTerminal;
use voltkit::wizard::{self, Plan};

use adapter::AdapterEvent;
use confirm::ConfirmEvent;
use detect::DetectEvent;
use menu::MenuEvent;

enum Screen {
    Menu,
    Detect,
    Adapter,
    Confirm,
    Progress,
    Stage,
}

struct App {
    screen: Screen,
    menu: menu::Menu,
    detect: Option<detect::Picker>,
    adapter: adapter::Adapter,
    choice: Option<menu::MenuChoice>,
    plan: Option<Plan>,
    managed_bridge: bool,
    remembered: Option<String>,
    progress: progress::Progress,
    stage: Option<stage::Stage>,
    pending_stdout: Option<String>,
    quit: bool,
}

impl App {
    fn new() -> Self {
        Self {
            screen: Screen::Menu,
            menu: menu::Menu::new(),
            detect: None,
            adapter: adapter::Adapter::new(),
            choice: None,
            plan: None,
            managed_bridge: false,
            remembered: None,
            progress: progress::Progress::new(String::new()),
            stage: None,
            pending_stdout: None,
            quit: false,
        }
    }

    fn loop_events(&mut self, terminal: &mut DefaultTerminal) -> Result<(), String> {
        loop {
            if let (Screen::Detect, Some(picker)) = (&self.screen, self.detect.as_mut()) {
                picker.tick();
            }
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
        let body = banner_body(frame);
        match self.screen {
            Screen::Menu => self.menu.draw(frame, body),
            Screen::Detect => {
                if let Some(picker) = &self.detect {
                    picker.draw(frame, body);
                }
            }
            Screen::Adapter => self.adapter.draw(frame, body),
            Screen::Confirm => {
                if let Some(plan) = &self.plan {
                    confirm::draw(frame, body, plan);
                }
            }
            Screen::Progress => self.progress.draw(frame, body),
            Screen::Stage => {
                if let Some(stage) = &self.stage {
                    stage.draw(frame, body);
                }
            }
        }
    }

    fn on_key(&mut self, code: KeyCode) {
        match self.screen {
            Screen::Menu => self.on_menu(code),
            Screen::Detect => self.on_detect(code),
            Screen::Adapter => self.on_adapter(code),
            Screen::Confirm => self.on_confirm(code),
            Screen::Progress => self.on_progress(code),
            Screen::Stage => self.on_stage(code),
        }
    }

    fn on_menu(&mut self, code: KeyCode) {
        match self.menu.on_key(code) {
            Some(MenuEvent::Quit) => self.quit = true,
            Some(MenuEvent::Chosen(choice)) => {
                self.choice = Some(choice);
                self.adapter.error.clear();
                self.detect = Some(detect::Picker::new(self.remembered.clone()));
                self.screen = Screen::Detect;
            }
            None => {}
        }
    }

    fn on_detect(&mut self, code: KeyCode) {
        let Some(picker) = self.detect.as_mut() else {
            self.screen = Screen::Menu;
            return;
        };
        match picker.on_key(code) {
            Some(DetectEvent::Back) => self.screen = Screen::Menu,
            Some(DetectEvent::Manual) => {
                self.adapter.error.clear();
                self.screen = Screen::Adapter;
            }
            Some(DetectEvent::Picked(device)) => self.pick_device(&device),
            Some(DetectEvent::Vcx(device)) => {
                self.stage = Some(stage::Stage::new(&device));
                self.screen = Screen::Stage;
            }
            None => {}
        }
    }

    fn on_stage(&mut self, code: KeyCode) {
        let Some(stage) = self.stage.as_mut() else {
            self.screen = Screen::Detect;
            return;
        };
        match stage.on_key(code) {
            Some(stage::StageEvent::Run) => {
                let dir = voltkit::detect::default_capture_dir();
                let result =
                    run_stage(stage.port, &dir, unix_secs()).map(|path| path.display().to_string());
                stage.result = Some(result);
            }
            Some(stage::StageEvent::Back) => self.screen = Screen::Detect,
            Some(stage::StageEvent::Done) => {
                self.stage = None;
                self.screen = Screen::Menu;
            }
            None => {}
        }
    }

    fn pick_device(&mut self, device: &voltkit::Device) {
        let Some(choice) = self.choice.clone() else {
            self.screen = Screen::Menu;
            return;
        };
        let Some((target, baud)) = device.link.target() else {
            return;
        };
        if let voltkit::Link::Tcp(addr) = &device.link {
            self.remembered = Some(addr.clone());
        }
        let dir = voltkit::detect::default_capture_dir();
        let _ = std::fs::create_dir_all(&dir);
        let output = Some(voltkit::detect::auto_profile_path(
            &dir,
            &job_label(&choice),
            unix_secs(),
        ));
        let (sets, ranges) = sets_ranges(&choice);
        let plan = Plan {
            target,
            baud,
            sets,
            ranges,
            clone: true,
            output,
        };
        match wizard::describe(&plan) {
            Ok(_) => {
                self.managed_bridge = device.link.managed_bridge();
                self.plan = Some(plan);
                self.screen = Screen::Confirm;
            }
            Err(err) => {
                if let Some(picker) = self.detect.as_mut() {
                    picker.set_note(err);
                }
            }
        }
    }

    fn on_adapter(&mut self, code: KeyCode) {
        match self.adapter.on_key(code) {
            Some(AdapterEvent::Back) => self.screen = Screen::Detect,
            Some(AdapterEvent::Ready(ready)) => {
                let Some(choice) = self.choice.clone() else {
                    self.screen = Screen::Menu;
                    return;
                };
                if ready.target.contains(':') {
                    self.remembered = Some(ready.target.clone());
                }
                match plan_from(&choice, &ready) {
                    Ok(plan) => match wizard::describe(&plan) {
                        Ok(_) => {
                            self.managed_bridge = false;
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
            Some(ConfirmEvent::Back) => self.screen = Screen::Detect,
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
        // An OBDLink MX capture is managed: voltkit spawns obd-bridge, waits for
        // its listen line, and keeps it alive for the capture, which is the one
        // TCP client. voltkit does not open the radio.
        let _bridge = if self.managed_bridge {
            match start_bridge() {
                Ok(peer) => Some(peer),
                Err(err) => {
                    self.progress.error = Some(err);
                    self.progress.done = true;
                    return Ok(());
                }
            }
        } else {
            None
        };
        let args = super::profile_args(&plan);
        let result = {
            let progress = &mut self.progress;
            super::run_capture_logged(&args, |line| {
                progress.lines.push(line.to_string());
                let _ = terminal.draw(|frame| {
                    let body = banner_body(frame);
                    progress.draw(frame, body);
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

fn banner_body(frame: &mut ratatui::Frame) -> ratatui::layout::Rect {
    let area = frame.area();
    if area.height < art::HEIGHT + 8 {
        return area;
    }
    let [banner, body] =
        Layout::vertical([Constraint::Length(art::HEIGHT), Constraint::Min(0)]).areas(area);
    art::draw(frame, banner);
    body
}

/// The capture set names and inclusive ranges a job asks.
type SetsRanges = (Vec<String>, Vec<(u16, u8, u16, u16)>);

fn sets_ranges(choice: &menu::MenuChoice) -> SetsRanges {
    match choice {
        menu::MenuChoice::Sets(names) => (
            names.iter().map(|name| (*name).to_string()).collect(),
            Vec::new(),
        ),
        menu::MenuChoice::Screen(name) => (vec![(*name).to_string()], Vec::new()),
        menu::MenuChoice::Range(range) => (Vec::new(), vec![*range]),
    }
}

fn plan_from(choice: &menu::MenuChoice, ready: &adapter::Ready) -> Result<Plan, String> {
    let (sets, ranges) = sets_ranges(choice);
    Ok(Plan {
        target: ready.target.clone(),
        baud: ready.baud,
        sets,
        ranges,
        clone: true,
        output: ready.output.clone(),
    })
}

/// A short word for the capture file name, from the chosen job.
fn job_label(choice: &menu::MenuChoice) -> String {
    match choice {
        menu::MenuChoice::Sets(names) => wizard::guides()
            .iter()
            .find(|guide| guide.sets == *names)
            .map(|guide| guide.key.to_string())
            .unwrap_or_else(|| names.join("-")),
        menu::MenuChoice::Screen(name) => (*name).to_string(),
        menu::MenuChoice::Range((header, _, _, _)) => format!("range-{header:03X}"),
    }
}

fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Run the Phase 2 VCX doctor and write the gated SPS skeleton.
///
/// The real staging is Windows + `vcx` only (where a VCX device can be detected).
/// Off that build the call is never reachable from a real device, but it still
/// has to compile, so it returns a clear message.
#[cfg(all(windows, feature = "vcx"))]
fn run_stage(port: u32, dir: &std::path::Path, secs: u64) -> Result<std::path::PathBuf, String> {
    voltkit::stage::stage(port, dir, secs)
}

#[cfg(not(all(windows, feature = "vcx")))]
fn run_stage(_port: u32, _dir: &std::path::Path, _secs: u64) -> Result<std::path::PathBuf, String> {
    Err("VCX staging needs the vcx feature on Windows".to_string())
}

/// Spawn the managed obd-bridge and wait until it is listening.
fn start_bridge() -> Result<voltkit::bench::BenchPeer, String> {
    let program = voltkit::bench::program(std::env::var_os(voltkit::bench::PROGRAM_ENV));
    let mut peer = voltkit::bench::BenchPeer::start(&program).map_err(|e| e.to_string())?;
    peer.wait_listening().map_err(|e| e.to_string())?;
    Ok(peer)
}

pub fn run() -> Result<(), String> {
    let mut app = App::new();
    let outcome = ratatui::run(|terminal| app.loop_events(terminal));
    if let Some(text) = app.pending_stdout.take() {
        print!("{text}");
    }
    outcome
}
