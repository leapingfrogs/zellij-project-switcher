use std::collections::{BTreeMap, BTreeSet};

use zellij_project_switcher_plugin::core::CoreState;

#[test]
fn it_inits_the_state() {
    let projects = BTreeMap::from([
        (String::from("alpha"), String::from("alphabet")),
        (String::from("beta"), String::from("betabet")),
        (String::from("default"), String::from("~")),
    ]);
    let current_session = String::from("default");

    let state = CoreState::init(projects.clone(), current_session.clone());

    assert_eq!(state.projects, projects);
    assert_eq!(
        state.filtered_projects,
        BTreeSet::from([String::from("alpha"), String::from("beta"),])
    );

    assert_eq!(state.search_term, String::new());
    assert_eq!(state.current_session, current_session);
    assert_eq!(state.selected_index, Some(0))
}

#[test]
fn it_inits_the_state_when_no_projects() {
    let projects = BTreeMap::new();
    let current_session = String::from("default");

    let state = CoreState::init(projects.clone(), current_session.clone());

    assert_eq!(state.projects, projects);

    assert!(state.filtered_projects.is_empty());

    assert_eq!(state.search_term, String::new());
    assert_eq!(state.current_session, current_session);
    assert_eq!(state.selected_index, None)
}

#[test]
fn it_updates_search_term_on_addition() {
    let mut state = CoreState::init(
        BTreeMap::from([
            (String::from("alphab"), String::from("alphabet")),
            (String::from("beta"), String::from("betabet")),
            (String::from("default"), String::from("~")),
        ]),
        String::from("default"),
    );

    state.update_search_term('b');

    assert_eq!(state.search_term, String::from("b"));
    assert_eq!(
        state.filtered_projects,
        BTreeSet::from([String::from("alphab"), String::from("beta")])
    );
    assert_eq!(state.selected_index, Some(0));
    assert_eq!(state.selected_item(), Some(String::from("alphab")));

    state.update_search_term('t');

    assert_eq!(state.search_term, String::from("bt"));
    assert_eq!(
        state.filtered_projects,
        BTreeSet::from([String::from("beta")])
    );
    assert_eq!(state.selected_index, Some(0));
    assert_eq!(state.selected_item(), Some(String::from("beta")));
}

#[test]
fn it_updates_search_term_on_removal() {
    let mut state = CoreState {
        projects: BTreeMap::from([
            (String::from("alpha"), String::from("alphabet")),
            (String::from("beta"), String::from("betabet")),
            (String::from("default"), String::from("~")),
        ]),
        current_session: String::from("default"),
        search_term: String::from("t"),
        filtered_projects: BTreeSet::from([String::from("beta")]),
        selected_index: Some(0),
    };

    state.update_search_term_backspace();
    assert_eq!(state.search_term, String::from(""));
    assert_eq!(
        state.filtered_projects,
        BTreeSet::from([String::from("alpha"), String::from("beta")])
    );
    assert_eq!(state.selected_index, Some(1));
    assert_eq!(state.selected_item(), Some(String::from("beta")));
}

#[test]
fn it_allows_scrolling_results() {
    let mut state = CoreState::init(
        BTreeMap::from([
            (String::from("alpha"), String::from("alphabet")),
            (String::from("beta"), String::from("betabet")),
            (String::from("default"), String::from("~")),
            (String::from("gamma"), String::from("gammabet")),
        ]),
        String::from("default"),
    );

    state.up();
    assert_eq!(state.selected_index, Some(0));
    assert_eq!(state.selected_item(), Some("alpha".to_string()));

    state.down();
    assert_eq!(state.selected_index, Some(1));
    assert_eq!(state.selected_item(), Some("beta".to_string()));

    state.down();
    assert_eq!(state.selected_index, Some(2));
    assert_eq!(state.selected_item(), Some("gamma".to_string()));

    state.down();
    assert_eq!(state.selected_index, Some(2));
    assert_eq!(state.selected_item(), Some("gamma".to_string()));

    state.up();
    assert_eq!(state.selected_index, Some(1));
    assert_eq!(state.selected_item(), Some("beta".to_string()));

    state.up();
    state.up();
    state.up();
    assert_eq!(state.selected_index, Some(0));
    assert_eq!(state.selected_item(), Some("alpha".to_string()));
}

#[test]
fn it_updates_indexes_when_filtering() {
    let projects = BTreeMap::from([
        (String::from("alpha"), String::from("alphabet")),
        (String::from("other"), String::from("other")),
        (String::from("beta"), String::from("betabet")),
        (String::from("default"), String::from("~")),
    ]);
    let current_session = String::from("default");

    let mut state = CoreState::init(projects.clone(), current_session.clone());
    state.down();
    state.down();

    assert_eq!(state.selected_index, Some(2));
    assert_eq!(state.selected_item(), Some("other".to_string()));

    state.update_search_term('t');

    assert_eq!(state.selected_index, Some(1));
    assert_eq!(state.selected_item(), Some("other".to_string()));
}

// fn trace<T>(state: &mut T, f: impl Fn(&mut T))
// where
//     T: std::fmt::Debug,
// {
//     println!("\n  Before: {:#?}", state);
//     f(state);
//     println!("\n  After: {:#?}", state);
// }
