use ansi_term::{Colour::Fixed, Style};
use zellij_tile::prelude::*;

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, process};

#[derive(Default)]
struct State {
    userspace_configuration: BTreeMap<String, String>,
    projects: BTreeMap<String, String>,
    top_idx: usize,
    sel_idx: usize,
    selected: String,
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
        self.projects = default_projects();
        self.top_idx = 0;
        self.sel_idx = 0;
        self.selected = "default".to_string();

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

        // perform an initial load of projects...
        refresh_projects();
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
                should_render = false;
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
                    refresh_projects();
                }
                if let Key::Char('\n') = key {
                    eprintln!("Switch Project!");
                    match self.projects.get(&self.selected) {
                        Some(cwd) => switch_session_with_layout(
                            Some(self.selected.as_str()),
                            LayoutInfo::BuiltIn("default".into()),
                            Some(cwd.into()),
                        ),
                        None => (),
                    }
                }
                if let Key::Down = key {
                    eprintln!("Down");
                    let new_idx = self.sel_idx + 1;
                    match self.projects.keys().nth(new_idx) {
                        Some(k) => {
                            self.selected = k.clone();
                            self.sel_idx = new_idx;
                            if self.sel_idx > 5 {
                                self.top_idx = self.sel_idx - 5;
                            } else {
                                self.top_idx = 0;
                            }
                            ();
                        }
                        None => (),
                    }
                }
                if let Key::Up = key {
                    eprintln!("Up");
                    if self.sel_idx > 0 {
                        let new_idx = self.sel_idx - 1;
                        match self.projects.keys().nth(new_idx) {
                            Some(k) => {
                                self.selected = k.clone();
                                self.sel_idx = new_idx;
                                if self.sel_idx > 5 {
                                    self.top_idx = self.sel_idx - 5;
                                } else {
                                    self.top_idx = 0;
                                }
                                ();
                            }
                            None => (),
                        }
                    }
                }
                should_render = true;
            }
            Event::RunCommandResult(Some(status), stdin, _stdout, _data) => {
                if status == 0 {
                    let mut v = match std::str::from_utf8(&stdin) {
                        Ok(val) => do_lines(val),

                        Err(_) => default_projects(),
                    };
                    self.projects.append(&mut v);
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
        println!(" Open {}?", self.selected);
        eprintln!(
            "Render {:?} projects... (sel: {:?}, top: {:?})",
            self.projects.len(),
            self.sel_idx,
            self.top_idx
        );
        for (i, p) in self.projects.keys().enumerate() {
            if i == self.sel_idx {
                println!("{}", color_bold(GREEN, &format!("> {}", p).to_string()));
            } else {
                // we'll show 5 items at a time
                if i >= self.top_idx && i < self.top_idx + 5 {
                    println!("{}", color_bold(WHITE, &format!("  {}", p).to_string()));
                }
            }
        }
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

fn do_lines(lines: &str) -> BTreeMap<String, String> {
    return lines.lines().fold(default_projects(), |a, l| split(a, l));
}

fn refresh_projects() {
    let mut options = BTreeMap::new();
    options.insert("command".to_string(), "refresh_projects".to_string());
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

fn split(mut acc: BTreeMap<String, String>, line: &str) -> BTreeMap<String, String> {
    let re = Regex::new(r"^(?<path>.*\/(?<name>[^/]+))\/.git\/$").unwrap();

    match re.captures(line) {
        Some(caps) => {
            acc.insert(caps["name"].into(), caps["path"].into());
            acc
        }
        None => acc,
    }
}

fn default_projects() -> BTreeMap<String, String> {
    let mut projects = BTreeMap::new();
    projects.insert("default".into(), "~".into());
    return projects;
}
