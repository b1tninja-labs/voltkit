//! A guided shell for the profile capture.
//!
//! The questions choose a job the operator can name in plain language. The
//! result is the same sets and ranges `voltkit profile` would ask. Nothing is
//! sent until they confirm, and the confirmed plan still sends only reads.
//! `run` is the line shell. The `wizard` command uses the full-screen terminal
//! UI when stdout is a terminal, and this line shell when it is not.

use std::path::PathBuf;

use crate::select_rows;

/// One confirmed capture. The caller opens the adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// `HOST:PORT`, or a Windows `COMn` name.
    pub target: String,
    /// Serial baud. `None` for a TCP address, and for a COM port left at 115200.
    pub baud: Option<u32>,
    /// Profile set names, in the order they will be asked.
    pub sets: Vec<String>,
    /// Inclusive ranges, `(header, service, start, end)`.
    pub ranges: Vec<(u16, u8, u16, u16)>,
    /// Write a lab profile. The adapter dialogue stays separate from that text.
    pub clone: bool,
    /// Where the profile is written. `None` prints it.
    pub output: Option<PathBuf>,
}

/// One named capture the wizard can run without a range.
#[derive(Clone, Copy, Debug)]
pub struct Guide {
    /// The word the line shell accepts, such as `lab`.
    pub key: &'static str,
    /// The line shown in the menu.
    pub title: &'static str,
    /// What the capture asks, shown before the run.
    pub detail: &'static str,
    /// Profile set names, in the order they are asked.
    pub sets: &'static [&'static str],
}

const GUIDES: &[Guide] = &[
    Guide {
        key: "lab",
        title: "Copy this car into a lab profile",
        detail: "Asks the lab profile and mode 03 on 7DF. Parts, presence, the extra part identifiers, and hardware stay out.",
        sets: &["all"],
    },
    Guide {
        key: "programming",
        title: "Read parts and hardware for programming",
        detail: "Asks the part numbers, the extra part identifiers, the hardware id, and the VIN reads. It does not post a programming request.",
        sets: &["parts", "sps-extra", "hardware", "identity"],
    },
    Guide {
        key: "presence",
        title: "See which other modules answer",
        detail: "Asks $1A 90 once on nine headers the lab profile does not already ask.",
        sets: &["presence"],
    },
];

/// The named jobs, in menu order.
#[must_use]
pub fn guides() -> &'static [Guide] {
    GUIDES
}

/// One screen name and the line shown for it.
#[must_use]
pub fn screens() -> &'static [(&'static str, &'static str)] {
    SCREENS
}

/// What the plan asks, how many reads, the command, and where the profile goes.
///
/// # Errors
///
/// Returns the capture error when the sets or the range are refused.
pub fn describe(plan: &Plan) -> Result<String, String> {
    let rows = select_rows(&plan.sets, &plan.ranges)?;
    Ok(summary(plan, rows.len()))
}

const SCREENS: &[(&str, &str)] = &[
    ("charger", "Charger on 7E4"),
    ("climate", "Climate on 241"),
    ("cells", "Cell table on 7E7"),
    ("cluster", "Cluster on 244"),
    ("hpcm", "HPCM2 on 7E0"),
    ("powertrain", "Powertrain on 7E1"),
    ("becm", "BECM on 7E4"),
    ("body", "The other body reads on 241"),
    ("scan", "The 2026-11-14 trial"),
    ("identity", "The $1A identifiers already in the lab capture"),
];

/// The `voltkit profile` line a plan would run.
#[must_use]
pub fn command_line(plan: &Plan) -> String {
    let mut line = format!("voltkit profile {}", plan.target);
    if let Some(baud) = plan.baud {
        line.push_str(&format!(" --baud {baud}"));
    }
    for name in &plan.sets {
        line.push_str(&format!(" --set {name}"));
    }
    for (header, service, start, end) in &plan.ranges {
        line.push_str(&format!(
            " --range {header:03X} {service:02X} {start:X} {end:X}"
        ));
    }
    if plan.clone {
        line.push_str(" --clone");
    }
    line
}

/// Read answers from `lines`, print prompts through `write`, and call `act` for each confirmed plan.
///
/// End of input, `q`, or answering no to another capture returns. A refused range is shown and
/// the menu is offered again. `act` is not called until the operator answers yes.
///
/// # Errors
///
/// Returns the error from `act`, or an I/O error message from a caller that feeds one.
pub fn drive<I, W, F>(lines: &mut I, mut write: W, mut act: F) -> Result<(), String>
where
    I: Iterator<Item = String>,
    W: FnMut(&str),
    F: FnMut(&Plan) -> Result<(), String>,
{
    write(
        "This asks the adapter the same reads as voltkit profile.\n\
         It does not send security access or a write.\n",
    );
    let mut target: Option<String> = None;
    let mut baud: Option<u32> = None;
    loop {
        let Some(plan) = ask_plan(lines, &mut write, &mut target, &mut baud)? else {
            return Ok(());
        };
        let rows = select_rows(&plan.sets, &plan.ranges)?;
        write(&format!("{}\n", summary(&plan, rows.len())));
        write("Run this? [y/N]\n> ");
        match next_line(lines)?.as_deref().map(str::trim) {
            Some("y" | "yes") => act(&plan)?,
            Some(_) | None => {
                write("Nothing was sent.\n");
                if lines_ended_or_no(lines, &mut write, "Another capture? [y/N]\n> ")? {
                    return Ok(());
                }
                continue;
            }
        }
        if lines_ended_or_no(lines, &mut write, "Another capture? [y/N]\n> ")? {
            return Ok(());
        }
    }
}

/// Prompts on stderr and runs each confirmed plan.
///
/// # Errors
///
/// Returns a prompt error, a refused plan, or the capture error.
pub fn run<F>(mut act: F) -> Result<(), String>
where
    F: FnMut(&Plan) -> Result<(), String>,
{
    let stdin = std::io::stdin();
    let mut lines = stdin.lines().map_while(Result::ok);
    drive(&mut lines, |text| eprint!("{text}"), &mut act)
}

fn ask_plan<I, W>(
    lines: &mut I,
    write: &mut W,
    target: &mut Option<String>,
    baud: &mut Option<u32>,
) -> Result<Option<Plan>, String>
where
    I: Iterator<Item = String>,
    W: FnMut(&str),
{
    loop {
        write(&menu());
        write("> ");
        let Some(choice) = next_line(lines)? else {
            return Ok(None);
        };
        let choice = choice.trim();
        if choice == "q" || choice == "quit" {
            return Ok(None);
        }
        if choice == "?" || choice == "help" {
            continue;
        }
        let drafted = if let Some(job) = job_from(choice) {
            Some(draft_sets(job.sets))
        } else if choice == "4" || choice == "screen" {
            match ask_screen(lines, write)? {
                Some(name) => Some(draft_sets(&[name])),
                None => continue,
            }
        } else if choice == "5" || choice == "range" {
            match ask_range(lines, write)? {
                Some(range) => Some(Draft {
                    sets: Vec::new(),
                    ranges: vec![range],
                }),
                None => continue,
            }
        } else {
            write("Choose 1, 2, 3, 4, 5, or q.\n");
            continue;
        };
        let Some(draft) = drafted else {
            continue;
        };
        let Some(chosen) = ask_target(lines, write, target)? else {
            return Ok(None);
        };
        let chosen_baud = if chosen.to_ascii_uppercase().starts_with("COM") {
            match ask_baud(lines, write, *baud)? {
                Some(value) => value,
                None => return Ok(None),
            }
        } else {
            None
        };
        *baud = chosen_baud;
        let output = match ask_output(lines, write)? {
            Some(path) => path,
            None => return Ok(None),
        };
        return Ok(Some(Plan {
            target: chosen,
            baud: chosen_baud,
            sets: draft.sets,
            ranges: draft.ranges,
            clone: true,
            output,
        }));
    }
}

struct Draft {
    sets: Vec<String>,
    ranges: Vec<(u16, u8, u16, u16)>,
}

fn draft_sets(names: &[&str]) -> Draft {
    Draft {
        sets: names.iter().map(|name| (*name).to_string()).collect(),
        ranges: Vec::new(),
    }
}

fn job_from(choice: &str) -> Option<&'static Guide> {
    GUIDES.iter().enumerate().find_map(|(index, job)| {
        if choice == (index + 1).to_string() || choice == job.key {
            Some(job)
        } else {
            None
        }
    })
}

fn ask_screen<I, W>(lines: &mut I, write: &mut W) -> Result<Option<&'static str>, String>
where
    I: Iterator<Item = String>,
    W: FnMut(&str),
{
    loop {
        write(&screen_menu());
        write("> ");
        let Some(choice) = next_line(lines)? else {
            return Ok(None);
        };
        let choice = choice.trim();
        if choice == "q" || choice == "quit" {
            return Ok(None);
        }
        if let Some((name, _)) = SCREENS.iter().enumerate().find_map(|(index, screen)| {
            if choice == (index + 1).to_string() || choice == screen.0 {
                Some(*screen)
            } else {
                None
            }
        }) {
            return Ok(Some(name));
        }
        write("Choose a screen number, or q to go back.\n");
    }
}

fn ask_range<I, W>(lines: &mut I, write: &mut W) -> Result<Option<(u16, u8, u16, u16)>, String>
where
    I: Iterator<Item = String>,
    W: FnMut(&str),
{
    write("Header, in hex, such as 7E4.\n> ");
    let Some(header_text) = next_line(lines)? else {
        return Ok(None);
    };
    if header_text.trim() == "q" || header_text.trim() == "quit" {
        return Ok(None);
    }
    let Ok(header) = u16::from_str_radix(header_text.trim(), 16) else {
        write("The header is hex, such as 7E4.\n");
        return Ok(None);
    };
    write("Service, 22 or 1A.\n> ");
    let Some(service_text) = next_line(lines)? else {
        return Ok(None);
    };
    let service = match service_text.trim().to_ascii_uppercase().as_str() {
        "22" => 0x22,
        "1A" => 0x1A,
        "27" => {
            write("Security access is not sent.\n");
            return Ok(None);
        }
        _ => {
            write("The service is 22 or 1A.\n");
            return Ok(None);
        }
    };
    write("First identifier, in hex.\n> ");
    let Some(start_text) = next_line(lines)? else {
        return Ok(None);
    };
    let Ok(start) = u16::from_str_radix(start_text.trim(), 16) else {
        write("The identifier is hex.\n");
        return Ok(None);
    };
    write("Last identifier, in hex.\n> ");
    let Some(end_text) = next_line(lines)? else {
        return Ok(None);
    };
    let Ok(end) = u16::from_str_radix(end_text.trim(), 16) else {
        write("The identifier is hex.\n");
        return Ok(None);
    };
    if let Err(err) = crate::range_rows(header, service, start, end) {
        write(&format!("{err}\n"));
        return Ok(None);
    }
    Ok(Some((header, service, start, end)))
}

fn ask_target<I, W>(
    lines: &mut I,
    write: &mut W,
    previous: &mut Option<String>,
) -> Result<Option<String>, String>
where
    I: Iterator<Item = String>,
    W: FnMut(&str),
{
    loop {
        match previous {
            Some(value) => write(&format!(
                "Adapter address [{value}]. A host and port, or a COM port such as COM5.\n> "
            )),
            None => write(
                "Adapter address. A host and port, such as 192.168.0.10:35000, or a COM port such as COM5.\n> ",
            ),
        }
        let Some(text) = next_line(lines)? else {
            return Ok(None);
        };
        let text = text.trim();
        if text == "q" || text == "quit" {
            return Ok(None);
        }
        if text.is_empty() {
            if let Some(value) = previous.clone() {
                return Ok(Some(value));
            }
            write("Enter a host and port, or a COM port.\n");
            continue;
        }
        let chosen = if text.to_ascii_uppercase().starts_with("COM") {
            text.to_ascii_uppercase()
        } else {
            text.to_string()
        };
        if !chosen.contains(':') && !chosen.starts_with("COM") {
            write("Enter a host and port, or a COM port.\n");
            continue;
        }
        #[cfg(not(windows))]
        if chosen.starts_with("COM") {
            write("A COM port is Windows only. Enter a host and port.\n");
            continue;
        }
        *previous = Some(chosen.clone());
        return Ok(Some(chosen));
    }
}

fn ask_baud<I, W>(
    lines: &mut I,
    write: &mut W,
    previous: Option<u32>,
) -> Result<Option<Option<u32>>, String>
where
    I: Iterator<Item = String>,
    W: FnMut(&str),
{
    let shown = previous.unwrap_or(115_200);
    loop {
        write(&format!("Baud [{shown}]. Press enter to keep it.\n> "));
        let Some(text) = next_line(lines)? else {
            return Ok(None);
        };
        let text = text.trim();
        if text == "q" || text == "quit" {
            return Ok(None);
        }
        if text.is_empty() {
            return Ok(Some(previous.filter(|value| *value != 115_200)));
        }
        match text.parse::<u32>() {
            Ok(value) if value > 0 => {
                return Ok(Some(if value == 115_200 { None } else { Some(value) }));
            }
            _ => write("Baud is a number, such as 115200.\n"),
        }
    }
}

fn ask_output<I, W>(lines: &mut I, write: &mut W) -> Result<Option<Option<PathBuf>>, String>
where
    I: Iterator<Item = String>,
    W: FnMut(&str),
{
    loop {
        write(
            "Profile file. Press enter to print it here. Otherwise type a path outside this repository.\n> ",
        );
        let Some(text) = next_line(lines)? else {
            return Ok(None);
        };
        let text = text.trim();
        if text == "q" || text == "quit" {
            return Ok(None);
        }
        if text.is_empty() {
            return Ok(Some(None));
        }
        let path = PathBuf::from(text);
        if path.is_dir() {
            write(&format!("{} is a directory.\n", path.display()));
            continue;
        }
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                write(&format!("Directory {} does not exist.\n", parent.display()));
                continue;
            }
        }
        return Ok(Some(Some(path)));
    }
}

fn lines_ended_or_no<I, W>(lines: &mut I, write: &mut W, prompt: &str) -> Result<bool, String>
where
    I: Iterator<Item = String>,
    W: FnMut(&str),
{
    write(prompt);
    match next_line(lines)?.as_deref().map(str::trim) {
        Some("y" | "yes") => Ok(false),
        Some(_) | None => Ok(true),
    }
}

fn next_line<I>(lines: &mut I) -> Result<Option<String>, String>
where
    I: Iterator<Item = String>,
{
    Ok(lines.next())
}

fn menu() -> String {
    let mut text = String::from("\nWhat do you want to do?\n");
    for (index, job) in GUIDES.iter().enumerate() {
        text.push_str(&format!("  {}  {}\n", index + 1, job.title));
    }
    text.push_str("  4  Read one screen\n");
    text.push_str("  5  Walk a range of identifiers\n");
    text.push_str("  q  Quit\n");
    text
}

fn screen_menu() -> String {
    let mut text = String::from("\nWhich screen?\n");
    for (index, (_, title)) in SCREENS.iter().enumerate() {
        text.push_str(&format!("  {}  {title}\n", index + 1));
    }
    text.push_str("  q  Back\n");
    text
}

fn summary(plan: &Plan, reads: usize) -> String {
    let job = plan.sets.first().and_then(|name| {
        GUIDES
            .iter()
            .find(|job| job.sets.len() == 1 && job.sets[0] == name)
            .map(|job| job.detail)
    });
    let detail = if plan.sets.len() > 1 {
        GUIDES
            .iter()
            .find(|job| job.key == "programming")
            .map(|job| job.detail)
            .unwrap_or("")
    } else if let Some(detail) = job {
        detail
    } else if let Some(name) = plan.sets.first() {
        SCREENS
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, title)| *title)
            .unwrap_or(name)
    } else {
        "Walks one header across the identifiers you named."
    };
    let destination = match &plan.output {
        Some(path) => format!("The profile is written to {}.", path.display()),
        None => "The profile is printed here. The adapter dialogue is kept beside it.".to_string(),
    };
    format!(
        "{detail}\nThis asks {reads} reads.\n{}\n{destination}",
        command_line(plan)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drive_script(answers: &[&str]) -> (String, Vec<Plan>) {
        let mut lines = answers.iter().map(|line| (*line).to_string());
        let mut shown = String::new();
        let mut plans = Vec::new();
        drive(
            &mut lines,
            |text| shown.push_str(text),
            |plan| {
                plans.push(plan.clone());
                Ok(())
            },
        )
        .unwrap();
        (shown, plans)
    }

    #[test]
    fn the_lab_job_is_the_emulation_pass() {
        let (shown, plans) = drive_script(&["1", "192.168.0.10:35000", "", "y", "n"]);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].sets, vec!["all".to_string()]);
        assert!(plans[0].clone);
        assert!(plans[0].ranges.is_empty());
        assert!(plans[0].output.is_none());
        assert_eq!(plans[0].baud, None);
        assert_eq!(
            command_line(&plans[0]),
            "voltkit profile 192.168.0.10:35000 --set all --clone"
        );
        assert!(shown.contains("mode 03 on 7DF"));
        assert!(!shown.contains("Nothing was sent."));
    }

    #[test]
    fn programming_names_the_four_sets() {
        let (_, plans) = drive_script(&["programming", "10.0.0.2:35000", "", "yes", "q"]);
        assert_eq!(
            plans[0].sets,
            vec![
                "parts".to_string(),
                "sps-extra".to_string(),
                "hardware".to_string(),
                "identity".to_string()
            ]
        );
        assert!(command_line(&plans[0])
            .contains("--set parts --set sps-extra --set hardware --set identity --clone"));
        assert!(select_rows(&plans[0].sets, &plans[0].ranges).unwrap().len() > 4);
    }

    #[test]
    fn a_screen_can_be_declined_and_security_access_is_refused() {
        let (shown, plans) = drive_script(&[
            "4",
            "charger",
            "192.168.0.10:35000",
            "",
            "n",
            "y",
            "5",
            "7E4",
            "27",
            "q",
        ]);
        assert!(plans.is_empty());
        assert!(shown.contains("Nothing was sent."));
        assert!(shown.contains("Security access is not sent."));
    }

    #[test]
    fn a_range_keeps_the_span() {
        let (_, plans) = drive_script(&[
            "range",
            "7E7",
            "22",
            "4181",
            "4200",
            "192.168.0.10:35000",
            "",
            "y",
            "n",
        ]);
        assert_eq!(plans[0].ranges, vec![(0x7E7, 0x22, 0x4181, 0x4200)]);
        assert_eq!(plans[0].baud, None);
        assert_eq!(
            command_line(&plans[0]),
            "voltkit profile 192.168.0.10:35000 --range 7E7 22 4181 4200 --clone"
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_com_port_keeps_a_custom_baud() {
        let (_, plans) = drive_script(&["presence", "com5", "38400", "", "y", "n"]);
        assert_eq!(plans[0].target, "COM5");
        assert_eq!(plans[0].baud, Some(38_400));
        assert!(command_line(&plans[0]).contains("voltkit profile COM5 --baud 38400"));
    }

    #[cfg(not(windows))]
    #[test]
    fn a_com_port_is_refused_off_windows() {
        let (shown, plans) = drive_script(&["3", "com5", "q"]);
        assert!(plans.is_empty());
        assert!(shown.contains("A COM port is Windows only."));
    }

    #[test]
    fn enter_keeps_the_adapter_and_a_wide_range_is_refused() {
        let (shown, plans) = drive_script(&[
            "3",
            "192.168.0.10:35000",
            "",
            "y",
            "y",
            "5",
            "7E7",
            "22",
            "4181",
            "4281",
            "",
            "q",
        ]);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].sets, vec!["presence".to_string()]);
        assert!(shown.contains("a range is at most 256 identifiers"));
    }

    #[test]
    fn the_catalog_matches_the_line_menu() {
        assert_eq!(guides().len(), 3);
        assert_eq!(screens().len(), 10);
        let plan = Plan {
            target: "192.168.0.10:35000".to_string(),
            baud: None,
            sets: vec!["presence".to_string()],
            ranges: Vec::new(),
            clone: true,
            output: None,
        };
        let text = describe(&plan).unwrap();
        assert!(text.contains("This asks 9 reads."));
        assert!(text.contains("voltkit profile 192.168.0.10:35000 --set presence --clone"));
    }
}
