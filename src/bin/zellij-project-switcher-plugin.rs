// register_plugin! is cfg(not(test)), so in the bin's own test build nothing
// constructs State and everything below counts as dead code.
#![cfg_attr(test, allow(dead_code))]

use nu_ansi_term::{Color::Fixed, Style};
use zellij_tile::prelude::*;

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use zellij_project_switcher_plugin::core;
use zellij_project_switcher_plugin::stack;

// Lives in the plugin's /cache mount: keyed by plugin URL, shared across
// sessions, and persistent — one MRU stack for all instances.
const STACK_PATH: &str = "/cache/session-stack.v1";
const TOGGLE_DEBOUNCE_PATH: &str = "/cache/session-stack-toggle.claim";
const TOGGLE_MESSAGE: &str = "toggle_session";

#[derive(Default)]
struct State {
    userspace_configuration: BTreeMap<String, String>,
    projects: BTreeMap<String, String>,
    entries: Vec<core::PickerEntry>,
    top_idx: usize,
    sel_idx: usize,
    selected: String,
    // Set once the user moves the selection. Until then it stays on the first
    // row, however the list reorders as projects and sessions arrive.
    selection_pinned: bool,
    search_term: String,
    rows: usize,
    cols: usize,
    current_session: Option<String>,
    pending_events: Vec<Event>,
    got_permissions: bool,
    projects_loaded: bool,
    // Session-stack tracking (see src/stack.rs). own_session/own_connected
    // come from SessionUpdate's is_current_session entry, independent of the
    // ModeUpdate-driven current_session used by the UI.
    own_session: Option<String>,
    own_connected: Option<usize>,
    live_sessions: BTreeSet<String>,
    tracker_mode: bool,
    // A toggle_session pipe waiting for the ListClients reply that says
    // whether this instance's client is still attached (see pipe()).
    toggle_pending: bool,
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

            if let Some(entry) = self.entries.get(self.sel_idx) {
                // Eager push so the stack is correct even if the target
                // session's attach snapshot lags behind the switch.
                let mut stack = stack::read_stack(Path::new(STACK_PATH));
                if stack.push_top(&entry.name) {
                    stack::write_stack(Path::new(STACK_PATH), &stack);
                    eprintln!(
                        "stack: menu-enter pushed {:?}; stack now {:?}",
                        entry.name,
                        stack.entries()
                    );
                }
                hide_self();
                // A running session is simply attached to, and may have no
                // project. Anything else is a project, so it gets a new session.
                match self.projects.get(&entry.name).filter(|_| !entry.active) {
                    Some(cwd) => switch_session_with_layout(
                        Some(entry.name.as_str()),
                        LayoutInfo::BuiltIn(layout.into()),
                        Some(cwd.into()),
                    ),
                    None => switch_session(Some(entry.name.as_str())),
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
            self.move_selection(true);
            return true;
        }
        if let BareKey::Up = key.bare_key {
            eprintln!("Up");
            self.move_selection(false);
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
    }

    pub fn handle_backspace(&mut self) {
        self.search_term.pop();
        self.update_filtered();
    }

    fn move_selection(&mut self, down: bool) {
        let last = self.entries.len().saturating_sub(1);
        self.sel_idx = if down {
            (self.sel_idx + 1).min(last)
        } else {
            self.sel_idx.saturating_sub(1)
        };
        self.selection_pinned = !self.entries.is_empty();
        self.sync_selected();
    }

    /// Rebuild the list. A row the user moved to stays selected wherever it
    /// lands; otherwise the selection goes to the first row.
    pub fn update_filtered(&mut self) {
        self.entries = core::picker_entries(
            &self.projects,
            &self.live_sessions,
            self.current_session
                .as_deref()
                .or(self.own_session.as_deref()),
            &self.search_term,
        );
        let kept = if self.selection_pinned {
            self.entries.iter().position(|e| e.name == self.selected)
        } else {
            None
        };
        self.selection_pinned = kept.is_some();
        self.sel_idx = kept.unwrap_or(0);
        self.sync_selected();
    }

    fn sync_selected(&mut self) {
        self.selected = self
            .entries
            .get(self.sel_idx)
            .map_or_else(String::new, |e| e.name.clone());
    }

    /// Fetch running sessions now: the `SessionUpdate` event only carries other
    /// sessions after something has called `get_session_list`.
    fn refresh_sessions(&mut self) {
        match get_session_list() {
            Ok(snapshot) => {
                self.live_sessions = snapshot
                    .live_sessions
                    .iter()
                    .map(|s| s.name.clone())
                    .collect();
                self.update_filtered();
            }
            Err(e) => eprintln!("picker: get_session_list failed: {e}"),
        }
    }

    fn do_lines(&mut self, lines: &str) -> BTreeMap<String, String> {
        let init = State::default_projects();
        eprintln!("Default Projects: {:?}", self.projects);
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

    /// Cmd-Tab-style toggle: switch to the most recent live session that
    /// isn't the current one. Silent no-op when there is nowhere to go.
    ///
    /// Deliberately stateless: everything is derived from a fresh
    /// `get_session_list()` snapshot so the handler is correct even in a
    /// freshly-launched instance that has not yet seen a `SessionUpdate`.
    /// (The `SessionUpdate` event only carries other sessions after
    /// something has called `get_session_list` — zellij 0.44 has no
    /// periodic machine-wide scan.)
    fn toggle_session(&mut self) {
        let snapshot = match get_session_list() {
            Ok(snapshot) => snapshot,
            Err(e) => {
                eprintln!("toggle: get_session_list failed: {e}");
                return;
            }
        };
        let seen: Vec<String> = snapshot
            .live_sessions
            .iter()
            .map(|s| {
                format!(
                    "{}(clients:{}{})",
                    s.name,
                    s.connected_clients,
                    if s.is_current_session { ",own" } else { "" }
                )
            })
            .collect();
        eprintln!("toggle: snapshot {seen:?}");
        let Some(own) = snapshot.live_sessions.iter().find(|s| s.is_current_session) else {
            eprintln!("toggle: no-op — own session missing from snapshot");
            return;
        };
        // Guard: pipes can reach instances of this plugin in every session
        // (CLI pipes are machine-wide); only an instance in a session the
        // user is attached to may act.
        if own.connected_clients == 0 && self.own_connected.unwrap_or(0) == 0 {
            eprintln!(
                "toggle: no-op — no client attached here (snapshot clients: 0, tracked: {:?})",
                self.own_connected
            );
            return;
        }
        // Debounce across instances: a broadcast pipe reaches several
        // trackers moments apart, and the target session's copy could
        // otherwise observe the first copy's switch and bounce the client
        // straight back.
        if !stack::claim_toggle_slot(Path::new(TOGGLE_DEBOUNCE_PATH)) {
            eprintln!(
                "toggle: no-op — debounced (another toggle ran within {}ms)",
                stack::TOGGLE_DEBOUNCE_MS
            );
            return;
        }
        let current = own.name.clone();
        let live: BTreeSet<String> = snapshot
            .live_sessions
            .iter()
            .map(|s| s.name.clone())
            .collect();
        let mut stack = stack::read_stack(Path::new(STACK_PATH));
        eprintln!(
            "toggle: current {current:?}, stack {:?}",
            stack.entries()
        );
        if stack.prune(&live) {
            stack::write_stack(Path::new(STACK_PATH), &stack);
            eprintln!("toggle: pruned dead entries; stack now {:?}", stack.entries());
        }
        if let Some(target) = stack.toggle_target(&current, &live) {
            stack.push_top(&target);
            stack::write_stack(Path::new(STACK_PATH), &stack);
            eprintln!(
                "toggle: switching to {target:?}; stack now {:?}",
                stack.entries()
            );
            switch_session(Some(&target));
        } else {
            eprintln!("toggle: no-op — no live toggle target in stack");
        }
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
                self.update_filtered();
                should_render = true;
            }
            Event::Visible(true) => {
                // The picker stays loaded while hidden, so sessions may have
                // started or stopped since it was last shown.
                self.refresh_sessions();
                should_render = true;
            }
            Event::ListClients(clients) => {
                // is_current_client is true only for this instance's own
                // client, and only while it is attached (detached clients
                // are dropped from the panes the list is built from).
                if self.toggle_pending {
                    self.toggle_pending = false;
                    if clients.iter().any(|c| c.is_current_client) {
                        self.toggle_session();
                    } else {
                        eprintln!("toggle: no-op — this instance's client is not attached");
                    }
                }
            }
            Event::SessionUpdate(infos, _resurrectable) => {
                self.live_sessions = infos.iter().map(|s| s.name.clone()).collect();
                if let Some(own) = infos.iter().find(|s| s.is_current_session) {
                    // Session renamed: keep the stack entry's position.
                    if let Some(prev) = &self.own_session {
                        if prev != &own.name {
                            let mut stack = stack::read_stack(Path::new(STACK_PATH));
                            if stack.rename(prev, &own.name) {
                                stack::write_stack(Path::new(STACK_PATH), &stack);
                                eprintln!(
                                    "stack: renamed {prev:?} -> {:?}; stack now {:?}",
                                    own.name,
                                    stack.entries()
                                );
                            }
                        }
                    }
                    // Any increase in our own client count is an attach —
                    // including the first snapshot while already attached —
                    // and moves this session to the top of the stack.
                    let attached = match self.own_connected {
                        Some(prev) => own.connected_clients > prev,
                        None => own.connected_clients > 0,
                    };
                    if attached {
                        let mut stack = stack::read_stack(Path::new(STACK_PATH));
                        if stack.push_top(&own.name) {
                            stack::write_stack(Path::new(STACK_PATH), &stack);
                            eprintln!(
                                "stack: attach pushed {:?} (clients {:?} -> {}); stack now {:?}",
                                own.name,
                                self.own_connected,
                                own.connected_clients,
                                stack.entries()
                            );
                        }
                    }
                    self.own_connected = Some(own.connected_clients);
                    self.own_session = Some(own.name.clone());
                }
                if !self.tracker_mode {
                    self.update_filtered();
                    should_render = true;
                }
            }
            Event::RunCommandResult(Some(status), stdin, _stdout, _data) => {
                if status == 0 {
                    let mut v = match std::str::from_utf8(&stdin) {
                        Ok(val) => self.do_lines(val),

                        Err(_) => State::default_projects(),
                    };
                    self.projects.append(&mut v);
                    self.update_filtered();
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
        self.tracker_mode = self
            .userspace_configuration
            .get("mode")
            .is_some_and(|m| m == "tracker");
        if self.tracker_mode {
            // Headless instance (loaded via load_plugins): only track the
            // session stack — no UI, no project discovery. Subscribe before
            // requesting permissions: a background instance has no pane to
            // show the permission dialog in (zellij #4982), so it relies on
            // the cached grant from the visible instance's one-time approval
            // and must not block event delivery on a dialog round-trip.
            subscribe(&[
                EventType::SessionUpdate,
                EventType::ListClients,
                EventType::PermissionRequestResult,
            ]);
            // Must request the SAME permission set as the UI mode below, even
            // though the tracker never runs commands: zellij rewrites the
            // cached grant with exactly the requested set on every load
            // (including silent cached grants), so requesting a subset here
            // would strip RunCommands from the cache each time a session
            // starts and re-prompt on the next UI open, forever.
            request_permission(&[
                PermissionType::ReadApplicationState,
                PermissionType::ChangeApplicationState,
                PermissionType::RunCommands,
            ]);
            return;
        }
        self.projects = State::default_projects();
        self.top_idx = 0;
        self.search_term = String::new();
        self.update_filtered();
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
            EventType::SessionUpdate,
            EventType::ListClients,
            EventType::PermissionRequestResult,
            EventType::Visible,
        ]);
    }
    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        eprintln!("pipe_message: {pipe_message:?}");
        // CLI pipes deliver the payload message and then a payload-less
        // end-of-pipe marker; only the first may trigger the toggle.
        let actionable = match pipe_message.source {
            PipeSource::Keybind => true,
            PipeSource::Cli(_) => pipe_message.payload.is_some(),
            PipeSource::Plugin(_) => false,
        };
        // Zellij creates an instance of every running plugin for each client
        // that attaches and never drops it when the client leaves, so a pipe
        // reaches one instance per client that has ever been here, and
        // switch_session() moves only the instance's own client. Ask which
        // clients are attached and toggle from the ListClients reply, so an
        // instance whose client has gone neither switches nobody nor claims
        // the debounce slot ahead of the live client's instance.
        if actionable && pipe_message.name == TOGGLE_MESSAGE {
            self.toggle_pending = true;
            list_clients();
        }
        false
    }
    fn update(&mut self, event: Event) -> bool {
        if self.tracker_mode {
            // No permission gate here: events are only delivered when the
            // cached grant is in place, and the tracker runs no commands.
            return self.handle_event(event);
        }

        if let Event::PermissionRequestResult(PermissionStatus::Granted) = event {
            self.got_permissions = true;

            while !self.pending_events.is_empty() {
                let ev = self.pending_events.pop();
                self.handle_event(ev.unwrap());
            }

            // perform an initial load of projects...
            if !self.tracker_mode {
                self.refresh_projects();
                self.refresh_sessions();
            }
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
        self.top_idx = core::scroll_top(self.top_idx, self.sel_idx, self.rows, self.entries.len());
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
            color_bold(ORANGE, &self.search_term),
            color_bold(GREEN, &self.selected)
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
        for (i, entry) in self
            .entries
            .iter()
            .enumerate()
            .skip(self.top_idx)
            .take(self.rows)
        {
            let selected = i == self.sel_idx;
            let color = if selected {
                GREEN
            } else if entry.active {
                ORANGE
            } else {
                WHITE
            };
            let prefix = if selected { ">" } else { " " };
            let marker = if entry.active { "● " } else { "" };
            println!(
                "{}",
                color_bold(color, &format!("{prefix} {marker}{}", entry.name))
            );
            lines_printed += 1;
        }

        // Fill remaining lines to push version to bottom
        for _ in lines_printed..self.rows {
            println!();
        }

        // Print version right-aligned at the bottom
        let version_text = format!("v{VERSION}");
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

#[cfg(test)]
mod test {
    use super::*;

    fn picker(projects: &[&str], live: &[&str], current: &str) -> State {
        let mut state = State {
            projects: projects
                .iter()
                .map(|n| ((*n).to_string(), format!("/src/{n}")))
                .collect(),
            live_sessions: live.iter().map(|n| (*n).to_string()).collect(),
            current_session: Some(current.to_string()),
            ..State::default()
        };
        state.update_filtered();
        state
    }

    #[test]
    fn it_selects_the_first_row_until_the_user_moves() {
        let mut state = picker(&["alpha", "beta"], &[], "main");
        assert_eq!(state.selected, "alpha");

        state.live_sessions.insert("zeta".to_string());
        state.update_filtered();

        assert_eq!(state.sel_idx, 0);
        assert_eq!(state.selected, "zeta");
    }

    #[test]
    fn it_keeps_a_chosen_row_selected_when_the_list_reorders() {
        let mut state = picker(&["alpha", "beta"], &[], "main");
        state.move_selection(true);
        assert_eq!(state.selected, "beta");

        state.live_sessions.insert("zeta".to_string());
        state.update_filtered();

        assert_eq!(state.selected, "beta");
        assert_eq!(state.sel_idx, 2);
    }

    #[test]
    fn it_returns_to_the_first_row_when_the_chosen_row_is_filtered_out() {
        let mut state = picker(&["alpha", "beta"], &[], "main");
        state.move_selection(true);

        state.update_search_term('a');
        state.update_search_term('l');

        assert_eq!(state.selected, "alpha");
        assert!(!state.selection_pinned);
    }

    #[test]
    fn it_clears_the_selection_when_nothing_matches() {
        let mut state = picker(&["alpha"], &[], "main");

        state.update_search_term('q');

        assert_eq!(state.entries, []);
        assert_eq!(state.selected, "");
    }

    #[test]
    fn it_stops_moving_at_either_end_of_the_list() {
        let mut state = picker(&["alpha", "beta"], &[], "main");

        state.move_selection(false);
        assert_eq!(state.sel_idx, 0);

        state.move_selection(true);
        state.move_selection(true);
        assert_eq!(state.sel_idx, 1);
    }

    #[test]
    fn it_drops_the_current_session_once_it_is_known() {
        let mut state = picker(&["alpha", "nixos"], &["nixos"], "other");
        assert_eq!(state.selected, "nixos");

        state.current_session = Some("nixos".to_string());
        state.update_filtered();

        assert_eq!(state.selected, "alpha");
        assert_eq!(state.entries.len(), 1);
    }
}
