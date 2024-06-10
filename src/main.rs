use ansi_term::{Colour::Fixed, Style};
use zellij_tile::prelude::*;

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, process};

#[derive(Default)]
struct State {
    userspace_configuration: BTreeMap<String, String>,
}

#[derive(Default, Serialize, Deserialize)]
pub struct ProjectWorker {}

register_plugin!(State);

register_worker!(ProjectWorker, project_worker, PROJECT_WORKER);

impl ZellijWorker<'_> for ProjectWorker {
    fn on_message(&mut self, message: String, payload: String) {
        eprintln!("worker message: {:?} payload: {:?}", message, payload);
        if message == "list_projects" {
            let output = process::Command::new("fd")
                .args([
                    "-Htd",
                    "^\\.git$",
                    "/Users/idavies",
                    "/Users/idavies/Documents",
                    "/Users/idavies/Documents/GitHub",
                    "/Users/idavies/Documents/GitHub/Github",
                    "--max-depth=2",
                ])
                .output()
                .expect("failed to execute process");
            let result = match std::str::from_utf8(&output.stdout) {
                Ok(val) => val,
                Err(_) => "No Projects found",
            };
            eprintln!("Got projects: {}", result);
            post_message_to_plugin(PluginMessage {
                name: "projects".into(),
                payload: result.into(),
                worker_name: None,
            })
        }
    }
}

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.userspace_configuration = configuration;
        // we need the ReadApplicationState permission to receive the ModeUpdate and TabUpdate
        // events
        // we need the ChangeApplicationState permission to open sessions
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::RunCommands,
        ]);
        subscribe(&[
            EventType::Key,
            EventType::CustomMessage,
            EventType::RunCommandResult,
        ]);
    }
    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        eprintln!("pipe_message: {:?}", pipe_message);
        true
    }
    fn update(&mut self, event: Event) -> bool {
        let mut should_render = true;
        match event {
            Event::CustomMessage(message, payload) => {
                eprintln!("custom_message: {:?} payload: {:?}", message, payload);
                should_render = true;
            }
            Event::Key(key) => {
                if let Key::Char('N') = key {
                    switch_session_with_layout(
                        Some("Downloads".into()),
                        LayoutInfo::BuiltIn("default".into()),
                        Some("/Users/idavies/Downloads".into()),
                    );
                }
                if let Key::Char('w') = key {
                    post_message_to(PluginMessage {
                        name: String::from("list_projects"),
                        payload: String::from("sample_payload"),
                        worker_name: Some(String::from("project")),
                    });
                }
                if let Key::Char('g') = key {
                    let mut options = BTreeMap::new();
                    options.insert("sample".into(), "42".into());
                    run_command(
                        &[
                            "fd",
                            "-Htd",
                            "^\\.git$",
                            "/Users/idavies",
                            "/Users/idavies/Documents",
                            "/Users/idavies/Documents/GitHub",
                            "/Users/idavies/Documents/GitHub/Github",
                            "--max-depth=2",
                        ],
                        options,
                    );
                }
                should_render = true;
            }
            Event::RunCommandResult(Some(status), stdin, stdout, data) => {
                eprintln!(
                    "Command Result: status: {:?} in: {:?} out: {:?} data: {:?}",
                    status,
                    match std::str::from_utf8(&stdin) {
                        Ok(val) => {
                            split_lines(val).into()
                        }
                        Err(_) => {
                            "None".to_string()
                        }
                    },
                    match std::str::from_utf8(&stdout) {
                        Ok(val) => val,
                        Err(_) => "No Projects found",
                    },
                    data
                );
                if status == 0 {
                    let result = match std::str::from_utf8(&stdin) {
                        Ok(val) => do_lines(val),
                        Err(_) => {
                            vec![]
                        }
                    };
                    match result.first() {
                        Some(l) => {
                            eprintln!("Would open: {}", l);
                            match l.split_once("::") {
                                Some((name, cwd)) => switch_session_with_layout(
                                    Some(name.into()),
                                    LayoutInfo::BuiltIn("default".into()),
                                    Some(cwd.into()),
                                ),
                                None => {}
                            }
                        }
                        None => {}
                    }
                }
                should_render = true;
            }
            _ => (),
        };
        should_render
    }

    fn render(&mut self, rows: usize, cols: usize) {
        let colored_rows = color_bold(CYAN, &rows.to_string());
        let colored_cols = color_bold(CYAN, &cols.to_string());
        println!("");
        println!("Size: {} rows and {} columns", colored_rows, colored_cols);
        println!("");
        println!(
            "{} {:#?}",
            color_bold(GREEN, "Started with the following user configuration:"),
            self.userspace_configuration
        );
        println!("");
    }
}

pub const CYAN: u8 = 51;
pub const GRAY_LIGHT: u8 = 238;
pub const GRAY_DARK: u8 = 245;
pub const WHITE: u8 = 15;
pub const BLACK: u8 = 16;
pub const RED: u8 = 124;
pub const GREEN: u8 = 154;
pub const ORANGE: u8 = 166;

fn color_bold(color: u8, text: &str) -> String {
    format!("{}", Style::new().fg(Fixed(color)).bold().paint(text))
}

fn do_lines(lines: &str) -> Vec<String> {
    return lines.lines().map(|l| split(l)).collect();
}

fn split_lines(lines: &str) -> String {
    return lines
        .lines()
        .fold("".to_string(), |acc, l| {
            format!("{},{}", acc, split(l).to_owned())
        })
        .into();
}

fn split(line: &str) -> String {
    let re = Regex::new(r"^(?<path>.*\/(?<name>[^/]+))\/.git\/$").unwrap();

    match re.captures(line) {
        Some(caps) => format!("{}::{}", &caps["name"], &caps["path"]),
        None => "default".into(),
    }
}
