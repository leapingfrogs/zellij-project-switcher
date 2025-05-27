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

    let default_root = &String::from("~");
    for root in config.get("roots").unwrap_or(default_root).split(':') {
        cmd.push(root.to_string());
    }

    let cmd: Vec<&str> = cmd.iter().map(String::as_ref).collect();
    f(&cmd, options);
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

        let regex = self.build_search_regex();
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

    fn build_search_regex(&mut self) -> regex::Regex {
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
        RegexBuilder::new(&regex_str)
            .case_insensitive(true)
            .build()
            .unwrap()
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
        assert_eq!(cmd[4..], vec!["~"]);
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
        assert_eq!(cmd[4..], vec!["~/personal_projects", "~/work_projects"]);
    }
}
