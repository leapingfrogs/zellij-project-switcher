use std::collections::BTreeSet;

use zellij_project_switcher_plugin::stack::SessionStack;

fn live(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(ToString::to_string).collect()
}

/// Simulate what a session's instance does when it observes its own attach.
fn attach(stack: &mut SessionStack, session: &str) {
    stack.push_top(session);
}

#[test]
fn it_builds_the_stack_in_attach_order() {
    let mut stack = SessionStack::default();
    attach(&mut stack, "alpha");
    attach(&mut stack, "beta");
    attach(&mut stack, "gamma");
    assert_eq!(stack.entries(), ["gamma", "beta", "alpha"]);
}

#[test]
fn it_ping_pongs_between_the_two_most_recent_sessions() {
    let mut stack = SessionStack::default();
    let sessions = live(&["alpha", "beta", "gamma"]);
    attach(&mut stack, "alpha");
    attach(&mut stack, "beta");
    attach(&mut stack, "gamma");

    // Toggle from gamma -> beta; the switch attaches beta, pushing it on top.
    let target = stack.toggle_target("gamma", &sessions).unwrap();
    assert_eq!(target, "beta");
    attach(&mut stack, &target);

    // Toggle again returns to gamma, not alpha (Cmd-Tab semantics).
    let target = stack.toggle_target("beta", &sessions).unwrap();
    assert_eq!(target, "gamma");
    attach(&mut stack, &target);

    let target = stack.toggle_target("gamma", &sessions).unwrap();
    assert_eq!(target, "beta");
}

#[test]
fn it_tracks_switches_made_outside_the_plugin() {
    let mut stack = SessionStack::default();
    let sessions = live(&["alpha", "beta", "gamma"]);
    attach(&mut stack, "alpha");
    attach(&mut stack, "beta");
    attach(&mut stack, "gamma");

    // User jumps gamma -> alpha via the built-in session-manager; alpha's
    // instance observes the attach and pushes itself.
    attach(&mut stack, "alpha");

    // Toggle now goes alpha <-> gamma.
    assert_eq!(
        stack.toggle_target("alpha", &sessions),
        Some("gamma".to_string())
    );
}

#[test]
fn it_skips_dead_sessions_when_toggling() {
    let mut stack = SessionStack::default();
    attach(&mut stack, "alpha");
    attach(&mut stack, "beta");
    attach(&mut stack, "gamma");

    // beta was killed: falls through to alpha.
    let sessions = live(&["alpha", "gamma"]);
    assert_eq!(
        stack.toggle_target("gamma", &sessions),
        Some("alpha".to_string())
    );

    // Pruning removes the dead entry permanently.
    assert!(stack.prune(&sessions));
    assert_eq!(stack.entries(), ["gamma", "alpha"]);
}

#[test]
fn it_follows_a_session_rename() {
    let mut stack = SessionStack::default();
    attach(&mut stack, "alpha");
    attach(&mut stack, "beta");
    attach(&mut stack, "gamma");

    assert!(stack.rename("beta", "renamed"));
    assert_eq!(stack.entries(), ["gamma", "renamed", "alpha"]);
    assert_eq!(
        stack.toggle_target("gamma", &live(&["alpha", "renamed", "gamma"])),
        Some("renamed".to_string())
    );
}

#[test]
fn it_does_nothing_with_a_single_session() {
    let mut stack = SessionStack::default();
    attach(&mut stack, "alpha");
    assert_eq!(stack.toggle_target("alpha", &live(&["alpha"])), None);
}

#[test]
fn it_recovers_from_a_corrupt_stack_file() {
    // parse is total: junk in, empty stack out; the next attach rebuilds it.
    let mut stack = SessionStack::parse("\u{0}\u{1}garbage\n\n\n");
    attach(&mut stack, "alpha");
    assert!(stack.entries().contains(&"alpha".to_string()));
}
