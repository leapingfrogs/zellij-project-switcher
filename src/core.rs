use std::{
    cmp::min,
    collections::{BTreeMap, BTreeSet},
};

use regex::RegexBuilder;

pub fn refresh_projects<RC>(config: &BTreeMap<String, String>, mut f: RC)
where
    RC: FnMut(&[&str], BTreeMap<String, String>),
{
    let options = BTreeMap::from([(String::from("command"), String::from("refresh_projects"))]);

    let mut cmd: Vec<String> = Vec::from([
        String::from("fd"),
        String::from("-Htd"),
        String::from("--max-depth=2"),
        String::from("^\\.git$"),
    ]);

    // Everything after `--` is a search path, so a root can't be parsed as an
    // fd option such as --exec.
    cmd.push(String::from("--"));

    let default_root = &String::from("~");
    for root in config.get("roots").unwrap_or(default_root).split(':') {
        cmd.push(root.to_string());
    }

    let cmd: Vec<&str> = cmd.iter().map(String::as_ref).collect();
    f(&cmd, options);
}

/// Case-insensitive matcher for names containing the characters of `term` in
/// order. Each character is matched literally, so any typed input is safe.
///
/// # Panics
///
/// Only if the pattern exceeds the regex size limit, which takes a search term
/// far longer than anyone will type.
#[must_use]
pub fn search_regex(term: &str) -> regex::Regex {
    let pattern = term
        .chars()
        .map(|c| regex::escape(&c.to_string()))
        .collect::<Vec<_>>()
        .join(".*?");
    RegexBuilder::new(&pattern)
        .case_insensitive(true)
        .build()
        .expect("escaped search term is a valid regex")
}

/// One row of the picker list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerEntry {
    pub name: String,
    /// Whether a Zellij session with this name is currently running.
    pub active: bool,
}

/// Build the picker list from discovered projects and running sessions: every
/// name matching `term` except the current session, with active sessions
/// first and each group sorted alphabetically, ignoring case.
#[must_use]
pub fn picker_entries(
    projects: &BTreeMap<String, String>,
    live_sessions: &BTreeSet<String>,
    current_session: Option<&str>,
    term: &str,
) -> Vec<PickerEntry> {
    let regex = search_regex(term);
    let mut entries: Vec<PickerEntry> = projects
        .keys()
        .chain(live_sessions.iter())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|name| current_session != Some(name.as_str()) && regex.is_match(name))
        .map(|name| PickerEntry {
            name: name.clone(),
            active: live_sessions.contains(name),
        })
        .collect();
    entries.sort_by_cached_key(|e| (!e.active, e.name.to_lowercase(), e.name.clone()));
    entries
}

/// The first list index to draw so that `selected` stays within a window of
/// `rows` rows, moving as little as possible from the previous `top`.
#[must_use]
pub fn scroll_top(top: usize, selected: usize, rows: usize, len: usize) -> usize {
    let top = min(top, len.saturating_sub(rows));
    if selected < top {
        selected
    } else if rows > 0 && selected >= top + rows {
        selected + 1 - rows
    } else {
        top
    }
}

#[derive(Debug, Default)]
pub struct CoreState {
    pub projects: BTreeMap<String, String>,
    pub search_term: String,
    pub filtered_projects: BTreeSet<String>,
    pub current_session: String,
    pub selected_index: Option<usize>,
}

impl CoreState {
    #[must_use]
    pub fn init(projects: &BTreeMap<String, String>, current_session: String) -> CoreState {
        let mut filtered_projects = projects.keys().cloned().collect::<BTreeSet<String>>();
        filtered_projects.remove(&current_session.clone());
        CoreState {
            projects: projects.clone(),
            search_term: String::new(),
            filtered_projects,
            current_session,
            selected_index: if projects.is_empty() { None } else { Some(0) },
        }
    }

    pub fn update_search_term(&mut self, char: char) {
        self.search_term.push(char);
        self.update_filtered_projects();
    }

    pub fn update_search_term_backspace(&mut self) {
        if self.search_term.is_empty() {
            return;
        }
        self.search_term.pop();
        self.update_filtered_projects();
    }

    pub fn up(&mut self) {
        self.selected_index = match self.selected_index {
            Some(0) => Some(0),
            Some(index) => Some(index - 1),
            None => None,
        };
        self.update_filtered_projects();
    }

    pub fn down(&mut self) {
        self.selected_index = match self.selected_index {
            Some(index) => Some(min(self.filtered_projects.len() - 1, index + 1)),
            None => None,
        };
        self.update_filtered_projects();
    }

    #[must_use]
    pub fn selected_item(&self) -> Option<String> {
        match self.selected_index {
            Some(index) => self.filtered_projects.clone().into_iter().nth(index),
            None => None,
        }
    }

    fn update_filtered_projects(&mut self) {
        let prior_selection = &self.selected_item();

        let regex = search_regex(&self.search_term);
        self.filtered_projects = self
            .projects
            .keys()
            .filter(|p| !self.current_session.eq(*p))
            .filter(|p| regex.is_match(p))
            .fold(BTreeSet::new(), |mut c, v| {
                c.insert(v.to_owned());
                c
            });

        // now seek current selection, if present update index
        // otherwise update current selection
        if let Some(position) =
            self.filtered_projects
                .clone()
                .iter()
                .position(|i| match prior_selection {
                    Some(item) => i.eq(item),
                    None => false,
                })
        {
            self.selected_index = Some(position);
        }
    }
}

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn refresh_projects_returns_expected_options() {
        let mut opts: Option<BTreeMap<String, String>> = None;

        refresh_projects(&BTreeMap::new(), |_, context| {
            opts = Some(context);
        });
        assert_eq!(
            opts,
            Some(BTreeMap::from([(
                String::from("command"),
                String::from("refresh_projects")
            )]))
        );
    }

    #[test]
    fn refresh_projects_base_command() {
        let mut cmd: Vec<String> = Vec::new();

        refresh_projects(&BTreeMap::new(), |c, _| {
            for item in c {
                cmd.push((*item).to_string());
            }
        });
        assert_eq!(cmd[..4], vec!["fd", "-Htd", "--max-depth=2", "^\\.git$"]);
    }

    #[test]
    fn refresh_projects_with_default_root() {
        let mut cmd: Vec<String> = Vec::new();

        refresh_projects(&BTreeMap::new(), |c, _| {
            for item in c {
                cmd.push((*item).to_string());
            }
        });
        assert_eq!(cmd[4..], vec!["--", "~"]);
    }

    #[test]
    fn refresh_projects_with_configured_roots() {
        let mut cmd: Vec<String> = Vec::new();

        let config = BTreeMap::from([(
            String::from("roots"),
            String::from("~/personal_projects:~/work_projects"),
        )]);
        refresh_projects(&config, |c, _| {
            for item in c {
                cmd.push((*item).to_string());
            }
        });
        assert_eq!(
            cmd[4..],
            vec!["--", "~/personal_projects", "~/work_projects"]
        );
    }

    #[test]
    fn refresh_projects_keeps_option_like_roots_after_the_separator() {
        let mut cmd: Vec<String> = Vec::new();

        let config = BTreeMap::from([(String::from("roots"), String::from("--exec=evil"))]);
        refresh_projects(&config, |c, _| {
            for item in c {
                cmd.push((*item).to_string());
            }
        });
        assert_eq!(cmd[4..], vec!["--", "--exec=evil"]);
    }

    fn names(entries: &[PickerEntry]) -> Vec<(&str, bool)> {
        entries
            .iter()
            .map(|e| (e.name.as_str(), e.active))
            .collect()
    }

    fn project_map(items: &[&str]) -> BTreeMap<String, String> {
        items
            .iter()
            .map(|n| ((*n).to_string(), format!("/src/{n}")))
            .collect()
    }

    fn session_set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|n| (*n).to_string()).collect()
    }

    #[test]
    fn picker_lists_active_sessions_first_and_hides_the_current_one() {
        let entries = picker_entries(
            &project_map(&["alpha", "default", "prismapsis", "zps-dev"]),
            &session_set(&["nixos", "prismapsis", "default"]),
            Some("nixos"),
            "",
        );
        assert_eq!(
            names(&entries),
            vec![
                ("default", true),
                ("prismapsis", true),
                ("alpha", false),
                ("zps-dev", false),
            ]
        );
    }

    #[test]
    fn picker_includes_running_sessions_without_a_project() {
        let entries = picker_entries(
            &project_map(&["alpha"]),
            &session_set(&["scratch"]),
            None,
            "",
        );
        assert_eq!(names(&entries), vec![("scratch", true), ("alpha", false)]);
    }

    #[test]
    fn picker_sorts_each_group_ignoring_case() {
        let entries = picker_entries(
            &project_map(&["beta", "Alpha"]),
            &session_set(&["zeta", "Yak"]),
            None,
            "",
        );
        assert_eq!(
            names(&entries),
            vec![
                ("Yak", true),
                ("zeta", true),
                ("Alpha", false),
                ("beta", false),
            ]
        );
    }

    #[test]
    fn picker_applies_the_search_to_active_sessions_too() {
        let entries = picker_entries(
            &project_map(&["alpha", "prism-docs"]),
            &session_set(&["nixos", "prismapsis"]),
            None,
            "prs",
        );
        assert_eq!(
            names(&entries),
            vec![("prismapsis", true), ("prism-docs", false)]
        );
    }

    #[test]
    fn scroll_top_keeps_the_selection_in_view() {
        // Inside the window: no movement.
        assert_eq!(scroll_top(0, 2, 3, 10), 0);
        assert_eq!(scroll_top(4, 5, 3, 10), 4);
        // Moving past the bottom: the selection becomes the last visible row.
        assert_eq!(scroll_top(0, 3, 3, 10), 1);
        // Moving above the top: the selection becomes the first visible row.
        assert_eq!(scroll_top(4, 2, 3, 10), 2);
    }

    #[test]
    fn scroll_top_pulls_back_when_the_list_shrinks() {
        assert_eq!(scroll_top(7, 0, 3, 2), 0);
        assert_eq!(scroll_top(7, 4, 3, 5), 2);
    }

    #[test]
    fn scroll_top_copes_with_no_rows_or_entries() {
        assert_eq!(scroll_top(0, 0, 3, 0), 0);
        assert_eq!(scroll_top(0, 4, 0, 10), 0);
    }

    #[test]
    fn search_regex_matches_characters_in_order_ignoring_case() {
        let regex = search_regex("bt");
        assert!(regex.is_match("BETA"));
        assert!(!regex.is_match("tab"));
    }

    #[test]
    fn search_regex_treats_metacharacters_literally() {
        for term in ["(", "[", "*", "+", "?", "\\", "a(", "a|b", "."] {
            assert!(search_regex(term).is_match(term), "term: {:?}", term);
        }
        assert!(!search_regex(".").is_match("alpha"));
        assert!(!search_regex("a|b").is_match("beta"));
    }
}
