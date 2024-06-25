use ansi_term::{Colour::Fixed, Style};
use zellij_tile::prelude::*;

use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use zellij_project_switcher_plugin::*;

#[derive(Default)]
struct State {
    userspace_configuration: BTreeMap<String, String>,
    projects: BTreeMap<String, String>,
    filtered_projects: BTreeSet<String>,
    top_idx: usize,
    sel_idx: usize,
    selected: String,
    search_term: String,
    rows: usize,
    cols: usize,
}

impl State {
    pub fn refresh_projects(&mut self) {
        core::refresh_projects(
            self.userspace_configuration.get("roots").cloned(),
            run_command,
        );
    }

    pub fn handle_key(&mut self, key: Key) -> bool {
        if let Key::Char('\n') = key {
            let default = "default".to_string();
            let layout = self
                .userspace_configuration
                .get("layout")
                .unwrap_or(&default);

            if let Some(cwd) = self.projects.get(&self.selected) {
                hide_self();
                switch_session_with_layout(
                    Some(self.selected.as_str()),
                    LayoutInfo::BuiltIn(layout.into()),
                    Some(cwd.into()),
                );
            }
            return true;
        }
        if let Key::Backspace = key {
            self.handle_backspace();
            return true;
        }
        if let Key::Esc = key {
            close_self();
            return true;
        }
        if let Key::Down = key {
            eprintln!("Down");
            self.update_selected(1, 0);
            return true;
        }
        if let Key::Up = key {
            eprintln!("Up");
            self.update_selected(0, 1);
            return true;
        }
        if let Key::Char(char) = key {
            self.update_search_term(char);
            return true;
        }

        false
    }

    pub fn update_search_term(&mut self, character: char) {
        self.search_term.push(character);
        self.update_filtered();
        self.update_selected(0, 0);
    }

    pub fn handle_backspace(&mut self) {
        if self.search_term.is_empty() {
            self.update_filtered();
            self.update_selected(0, 0);
        } else {
            self.search_term.pop();
            self.update_filtered();
            self.update_selected(0, 0);
        }
    }

    pub fn update_selected(&mut self, down: usize, up: usize) {
        let mut new_idx = if self.sel_idx >= self.filtered_projects.len() {
            if !self.filtered_projects.is_empty() {
                self.filtered_projects.len() - 1
            } else {
                0
            }
        } else {
            self.sel_idx
        };
        if new_idx > 0 {
            new_idx -= up
        };
        if new_idx < self.filtered_projects.len() {
            new_idx += down
        };
        self.sel_idx = new_idx;
        if self.sel_idx >= self.rows {
            self.top_idx = self.sel_idx - self.rows;
        } else {
            self.top_idx = 0;
        }
        if let Some(k) = self.filtered_projects.iter().nth(self.sel_idx) {
            self.selected.clone_from(k);
        }
    }

    pub fn update_filtered(&mut self) {
        let regex_str = self
            .search_term
            .chars()
            .enumerate()
            .fold(String::new(), |acc, (i, c)| {
                if i != 0 {
                    format!("{}.*?{}", acc, c)
                } else {
                    format!("{}{}", acc, c)
                }
            });
        let regex = RegexBuilder::new(&regex_str)
            .case_insensitive(true)
            .build()
            .unwrap();
        self.filtered_projects =
            self.projects
                .keys()
                .filter(|p| regex.is_match(p))
                .fold(BTreeSet::new(), |mut c, v| {
                    c.insert(v.to_owned());
                    c
                });
    }

    fn do_lines(&mut self, lines: &str) -> BTreeMap<String, String> {
        return lines
            .lines()
            .fold(self.default_projects(), |a, l| self.split(a, l));
    }

    fn split(&mut self, mut acc: BTreeMap<String, String>, line: &str) -> BTreeMap<String, String> {
        let re = Regex::new(r"^(?<path>.*\/(?<name>[^/]+))\/.git\/$").unwrap();

        match re.captures(line) {
            Some(caps) => {
                acc.insert(caps["name"].into(), caps["path"].into());
                acc
            }
            None => acc,
        }
    }

    fn default_projects(&mut self) -> BTreeMap<String, String> {
        let mut projects = BTreeMap::new();
        projects.insert("default".into(), "/Users/idavies".into());
        projects
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct ProjectWorker {}

#[cfg(not(test))]
register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.userspace_configuration = configuration;
        eprintln!("Config: {:?}", self.userspace_configuration);
        self.projects = self.default_projects();
        self.filtered_projects = self.projects.keys().fold(BTreeSet::new(), |mut c, v| {
            c.insert(v.to_owned());
            c
        });
        self.top_idx = 0;
        self.sel_idx = 0;
        self.selected = "default".to_string();
        self.search_term = "".to_string();
        self.rows = 10;
        self.cols = 40;

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
            EventType::PermissionRequestResult,
        ]);
    }
    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        eprintln!("pipe_message: {:?}", pipe_message);
        true
    }
    fn update(&mut self, event: Event) -> bool {
        let mut should_render = false;
        match event {
            Event::PermissionRequestResult(status) => {
                if status == PermissionStatus::Granted {
                    // perform an initial load of projects...
                    self.refresh_projects();
                }
            }
            Event::CustomMessage(message, payload) => {
                eprintln!("custom_message: {:?} payload: {:?}", message, payload);
                should_render = false;
            }
            Event::Key(key) => {
                should_render = self.handle_key(key);
            }
            Event::RunCommandResult(Some(status), stdin, _stdout, _data) => {
                if status == 0 {
                    let mut v = match std::str::from_utf8(&stdin) {
                        Ok(val) => self.do_lines(val),

                        Err(_) => self.default_projects(),
                    };
                    self.projects.append(&mut v);
                    self.update_filtered();
                    self.update_selected(0, 0);
                }
                should_render = true;
            }
            _ => (),
        };
        should_render
    }

    fn render(&mut self, rows: usize, cols: usize) {
        self.rows = rows - 3;
        self.cols = cols;
        self.update_selected(0, 0);
        println!();
        println!(
            "Filter: [{}] :: Open project: [{}]?",
            color_bold(ORANGE, &self.search_term.to_string()),
            color_bold(GREEN, &self.selected.to_string())
        );
        println!();
        eprintln!(
            "Render {:?} projects... (sel: {:?}, top: {:?})",
            self.projects.len(),
            self.sel_idx,
            self.top_idx
        );
        for (i, p) in self.filtered_projects.iter().enumerate() {
            if i < self.rows {
                if i == self.sel_idx {
                    println!("{}", color_bold(GREEN, &format!("> {}", p).to_string()));
                } else {
                    // we'll show self.rows items at a time - adjust for rows
                    if i >= self.top_idx && i < self.top_idx + self.rows {
                        println!("{}", color_bold(WHITE, &format!("  {}", p).to_string()));
                    }
                }
            }
        }
    }
}

// COLOR Helpers
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
