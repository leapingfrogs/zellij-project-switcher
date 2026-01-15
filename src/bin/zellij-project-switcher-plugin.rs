use nu_ansi_term::{Color::Fixed, Style};
use zellij_tile::prelude::*;

use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use zellij_project_switcher_plugin::core;

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
    current_session: Option<String>,
    pending_events: Vec<Event>,
    got_permissions: bool,
    projects_loaded: bool,
}

impl State {
    pub fn refresh_projects(&mut self) {
        core::refresh_projects(&self.userspace_configuration, run_command);
    }

    pub fn handle_key(&mut self, key: &KeyWithModifier) -> bool {
        eprintln!("into handle_ley with: {key:?}");

        if let BareKey::Enter = key.bare_key {
            let default = "default".to_string();
            let layout = self
                .userspace_configuration
                .get("layout")
                .unwrap_or(&default);

            if let Some(cwd) = self.projects.get(&self.selected) {
                if self
                    .current_session
                    .clone()
                    .is_some_and(|cs| cs.eq(&self.selected))
                {
                    eprintln!("Refusing to launch current session");
                } else {
                    hide_self();
                    switch_session_with_layout(
                        Some(self.selected.as_str()),
                        LayoutInfo::BuiltIn(layout.into()),
                        Some(cwd.into()),
                    );
                }
            }
            return true;
        }
        if let BareKey::Backspace = key.bare_key {
            self.handle_backspace();
            return true;
        }
        if let BareKey::Esc = key.bare_key {
            close_self();
            return true;
        }
        if let BareKey::Down = key.bare_key {
            eprintln!("Down");
            self.update_selected(1, 0);
            return true;
        }
        if let BareKey::Up = key.bare_key {
            eprintln!("Up");
            self.update_selected(0, 1);
            return true;
        }
        if let BareKey::Char(char) = key.bare_key {
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
            if self.filtered_projects.is_empty() {
                0
            } else {
                self.filtered_projects.len() - 1
            }
        } else {
            self.sel_idx
        };
        if new_idx > 0 {
            new_idx -= up;
        }
        if new_idx < self.filtered_projects.len() {
            new_idx += down;
        }
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
                    format!("{acc}.*?{c}")
                } else {
                    format!("{acc}{c}")
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
        let init = State::default_projects();
        eprintln!("Default Projects: {:?}", self.projects,);
        lines.lines().fold(init, State::split)
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
        projects.insert("default".into(), "/Users/idavies".into());
        projects.insert(
            "zps-dev".into(),
            "/Users/idavies/Documents/GitHub/zellij-project-switcher".into(),
        );
        projects
    }

    fn handle_event(&mut self, event: Event) -> bool {
        let mut should_render = false;
        match event {
            Event::PermissionRequestResult(_status) => {
                // Permissions handled in update() method
            }
            Event::CustomMessage(message, payload) => {
                eprintln!("custom_message: {message:?} payload: {payload:?}");
                should_render = false;
            }
            Event::Key(key) => {
                should_render = self.handle_key(&key);
            }
            Event::ModeUpdate(mode_info) => {
                match mode_info.session_name {
                    Some(ref name) => eprintln!("mode_info: {name:?}"),
                    None => eprintln!("mode_info: missing"),
                }
                self.current_session.clone_from(&mode_info.session_name);
                should_render = true;
            }
            Event::RunCommandResult(Some(status), stdin, _stdout, _data) => {
                if status == 0 {
                    let mut v = match std::str::from_utf8(&stdin) {
                        Ok(val) => self.do_lines(val),

                        Err(_) => State::default_projects(),
                    };
                    self.projects.append(&mut v);
                    self.update_filtered();
                    self.update_selected(0, 0);
                    self.projects_loaded = true;
                }
                should_render = true;
            }
            _ => (),
        }
        should_render
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
        self.projects = State::default_projects();
        self.filtered_projects = self.projects.keys().fold(BTreeSet::new(), |mut c, v| {
            c.insert(v.to_owned());
            c
        });
        self.top_idx = 0;
        self.sel_idx = 0;
        self.selected = "default".to_string();
        self.search_term = String::new();
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
            EventType::ModeUpdate,
            EventType::Key,
            EventType::CustomMessage,
            EventType::RunCommandResult,
            EventType::PermissionRequestResult,
        ]);
    }
    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        eprintln!("pipe_message: {pipe_message:?}");
        true
    }
    fn update(&mut self, event: Event) -> bool {
        if let Event::PermissionRequestResult(PermissionStatus::Granted) = event {
            self.got_permissions = true;

            while !self.pending_events.is_empty() {
                let ev = self.pending_events.pop();
                self.handle_event(ev.unwrap());
            }

            // perform an initial load of projects...
            self.refresh_projects();
        }

        if !self.got_permissions {
            self.pending_events.push(event);
            return false;
        }

        self.handle_event(event)
    }

    fn render(&mut self, rows: usize, cols: usize) {
        const VERSION: &str = env!("CARGO_PKG_VERSION");

        // Reserve 3 rows for header (blank, status, blank) and 1 for footer (version)
        self.rows = rows.saturating_sub(4);
        self.cols = cols;
        self.update_selected(0, 0);
        println!();
        println!(
            "Current: [{}] :: Filter: [{}] :: Open project: [{}]?",
            color_bold(
                CYAN,
                &self
                    .current_session
                    .clone()
                    .unwrap_or("<unknown>".to_string())
            ),
            color_bold(ORANGE, &self.search_term.to_string()),
            color_bold(GREEN, &self.selected.to_string())
        );
        println!();

        if !self.projects_loaded {
            println!("{}", color_bold(CYAN, "Loading projects..."));
            return;
        }

        eprintln!(
            "Render {:?} projects... (sel: {:?}, top: {:?})",
            self.projects.len(),
            self.sel_idx,
            self.top_idx
        );
        eprintln!("Defaults {:?}", State::default_projects());

        let mut lines_printed = 0;
        for (i, p) in self.filtered_projects.iter().enumerate() {
            if i < self.rows {
                if i == self.sel_idx {
                    println!("{}", color_bold(GREEN, &format!("> {p}").to_string()));
                    lines_printed += 1;
                } else {
                    // we'll show self.rows items at a time - adjust for rows
                    if i >= self.top_idx && i < self.top_idx + self.rows {
                        println!("{}", color_bold(WHITE, &format!("  {p}").to_string()));
                        lines_printed += 1;
                    }
                }
            }
        }

        // Fill remaining lines to push version to bottom
        for _ in lines_printed..self.rows {
            println!();
        }

        // Print version right-aligned at the bottom
        let version_text = format!("v{}", VERSION);
        let padding = cols.saturating_sub(version_text.len());
        println!(
            "{}{}",
            " ".repeat(padding),
            color_bold(GRAY_LIGHT, &version_text)
        );
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
