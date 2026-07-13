use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Upper bound on remembered sessions; oldest entries fall off the bottom.
pub const MAX_ENTRIES: usize = 20;

/// A most-recently-used stack of session names. Index 0 is the most recent.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SessionStack {
    entries: Vec<String>,
}

impl SessionStack {
    /// Parse a newline-delimited stack file. Total: garbage lines and
    /// duplicates are dropped rather than reported, so a corrupt file
    /// degrades to a partial or empty stack and self-heals on the next write.
    #[must_use]
    pub fn parse(contents: &str) -> SessionStack {
        let mut stack = SessionStack::default();
        for line in contents.lines() {
            let name = line.trim();
            if name.is_empty() || stack.entries.iter().any(|e| e == name) {
                continue;
            }
            if stack.entries.len() >= MAX_ENTRIES {
                break;
            }
            stack.entries.push(name.to_string());
        }
        stack
    }

    #[must_use]
    pub fn serialize(&self) -> String {
        self.entries.iter().fold(String::new(), |mut acc, e| {
            acc.push_str(e);
            acc.push('\n');
            acc
        })
    }

    /// Insert `name` at the top, or move it there if already present.
    /// Returns false when the stack is unchanged (already on top, or empty name).
    pub fn push_top(&mut self, name: &str) -> bool {
        if name.is_empty() || self.entries.first().is_some_and(|top| top == name) {
            return false;
        }
        self.entries.retain(|e| e != name);
        self.entries.insert(0, name.to_string());
        self.entries.truncate(MAX_ENTRIES);
        true
    }

    /// Rename a session in place, preserving its position. If the new name
    /// already exists elsewhere in the stack, the higher entry wins and the
    /// lower duplicate is dropped. Returns whether anything changed.
    pub fn rename(&mut self, old: &str, new: &str) -> bool {
        if old == new || !self.entries.iter().any(|e| e == old) {
            return false;
        }
        let mut seen = false;
        for e in &mut self.entries {
            if e == old {
                new.clone_into(e);
            }
        }
        self.entries.retain(|e| {
            if e == new {
                if seen {
                    return false;
                }
                seen = true;
            }
            true
        });
        true
    }

    /// Drop entries that are not in the live session set. Returns whether
    /// anything was removed.
    pub fn prune(&mut self, live: &BTreeSet<String>) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| live.contains(e));
        before != self.entries.len()
    }

    /// The session to toggle to: the most recent live entry that is not the
    /// current session. None when there is nowhere to go.
    #[must_use]
    pub fn toggle_target(&self, current: &str, live: &BTreeSet<String>) -> Option<String> {
        self.entries
            .iter()
            .find(|e| e.as_str() != current && live.contains(*e))
            .cloned()
    }

    #[must_use]
    pub fn entries(&self) -> &[String] {
        &self.entries
    }
}

/// Read the stack from `path`. Any failure (missing file, bad UTF-8, IO
/// error) yields an empty stack — the stack is a convenience and must never
/// take the plugin down.
#[must_use]
pub fn read_stack(path: &Path) -> SessionStack {
    match std::fs::read_to_string(path) {
        Ok(contents) => SessionStack::parse(&contents),
        Err(_) => SessionStack::default(),
    }
}

/// Persist the stack atomically: write a temp file next to `path`, then
/// rename over it. Errors are logged and swallowed.
pub fn write_stack(path: &Path, stack: &SessionStack) {
    let tmp = tmp_path(path);
    let result =
        std::fs::write(&tmp, stack.serialize()).and_then(|()| std::fs::rename(&tmp, path));
    if let Err(e) = result {
        eprintln!("session-stack: failed to persist {}: {e}", path.display());
    }
}

/// Debounce window for the toggle command across plugin instances.
pub const TOGGLE_DEBOUNCE_MS: u128 = 500;

/// Claim the right to perform a toggle. Pipes can be delivered to instances
/// of this plugin in several sessions moments apart; whichever instance
/// claims first wins and the echoes are dropped. Returns true when the claim
/// succeeded. Errors count as a successful claim — a broken debounce file
/// must not disable the feature.
#[must_use]
pub fn claim_toggle_slot(path: &Path) -> bool {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    if let Ok(contents) = std::fs::read_to_string(path) {
        if let Ok(last_ms) = contents.trim().parse::<u128>() {
            if now_ms.saturating_sub(last_ms) < TOGGLE_DEBOUNCE_MS {
                return false;
            }
        }
    }
    if let Err(e) = std::fs::write(path, now_ms.to_string()) {
        eprintln!("session-stack: failed to write {}: {e}", path.display());
    }
    true
}

fn tmp_path(path: &Path) -> PathBuf {
    // Nanosecond suffix keeps concurrent writers (UI + tracker instance in
    // the same session) from interleaving writes into one temp file.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let mut name = path.as_os_str().to_owned();
    name.push(format!(".tmp.{nanos}"));
    PathBuf::from(name)
}

#[cfg(test)]
mod test {
    use super::*;

    fn live(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn parse_round_trips_serialize() {
        let stack = SessionStack::parse("alpha\nbeta\ngamma\n");
        assert_eq!(stack.entries(), ["alpha", "beta", "gamma"]);
        assert_eq!(stack.serialize(), "alpha\nbeta\ngamma\n");
    }

    #[test]
    fn parse_drops_blank_lines_whitespace_and_duplicates() {
        let stack = SessionStack::parse("alpha\n\n  beta  \nalpha\n");
        assert_eq!(stack.entries(), ["alpha", "beta"]);
    }

    #[test]
    fn parse_of_garbage_yields_empty_stack() {
        assert_eq!(SessionStack::parse("").entries().len(), 0);
        assert_eq!(SessionStack::parse("\n\n\n").entries().len(), 0);
    }

    #[test]
    fn parse_caps_entries() {
        let contents = (0..40).fold(String::new(), |acc, i| format!("{acc}session-{i}\n"));
        assert_eq!(SessionStack::parse(&contents).entries().len(), MAX_ENTRIES);
    }

    #[test]
    fn push_top_inserts_and_moves_to_top() {
        let mut stack = SessionStack::default();
        assert!(stack.push_top("alpha"));
        assert!(stack.push_top("beta"));
        assert_eq!(stack.entries(), ["beta", "alpha"]);
        assert!(stack.push_top("alpha"));
        assert_eq!(stack.entries(), ["alpha", "beta"]);
    }

    #[test]
    fn push_top_is_noop_when_already_on_top_or_empty() {
        let mut stack = SessionStack::parse("alpha\nbeta\n");
        assert!(!stack.push_top("alpha"));
        assert!(!stack.push_top(""));
        assert_eq!(stack.entries(), ["alpha", "beta"]);
    }

    #[test]
    fn push_top_caps_entries() {
        let mut stack = SessionStack::default();
        for i in 0..MAX_ENTRIES + 5 {
            stack.push_top(&format!("session-{i}"));
        }
        assert_eq!(stack.entries().len(), MAX_ENTRIES);
        assert_eq!(stack.entries()[0], format!("session-{}", MAX_ENTRIES + 4));
    }

    #[test]
    fn rename_preserves_position() {
        let mut stack = SessionStack::parse("alpha\nbeta\ngamma\n");
        assert!(stack.rename("beta", "delta"));
        assert_eq!(stack.entries(), ["alpha", "delta", "gamma"]);
    }

    #[test]
    fn rename_missing_or_identity_is_noop() {
        let mut stack = SessionStack::parse("alpha\nbeta\n");
        assert!(!stack.rename("gamma", "delta"));
        assert!(!stack.rename("alpha", "alpha"));
        assert_eq!(stack.entries(), ["alpha", "beta"]);
    }

    #[test]
    fn rename_onto_existing_name_keeps_higher_entry() {
        let mut stack = SessionStack::parse("alpha\nbeta\ngamma\n");
        assert!(stack.rename("gamma", "alpha"));
        assert_eq!(stack.entries(), ["alpha", "beta"]);
    }

    #[test]
    fn prune_removes_dead_sessions() {
        let mut stack = SessionStack::parse("alpha\nbeta\ngamma\n");
        assert!(stack.prune(&live(&["alpha", "gamma"])));
        assert_eq!(stack.entries(), ["alpha", "gamma"]);
        assert!(!stack.prune(&live(&["alpha", "gamma"])));
    }

    #[test]
    fn toggle_target_returns_most_recent_other_live_session() {
        let stack = SessionStack::parse("current\nrecent\nolder\n");
        assert_eq!(
            stack.toggle_target("current", &live(&["current", "recent", "older"])),
            Some("recent".to_string())
        );
    }

    #[test]
    fn toggle_target_skips_dead_sessions() {
        let stack = SessionStack::parse("current\ndead\nolder\n");
        assert_eq!(
            stack.toggle_target("current", &live(&["current", "older"])),
            Some("older".to_string())
        );
    }

    #[test]
    fn toggle_target_handles_stale_stack_where_current_is_not_on_top() {
        // The pipe can race ahead of the attach snapshot that would put
        // `current` on top; the target must still not be `current` itself.
        let stack = SessionStack::parse("recent\ncurrent\n");
        assert_eq!(
            stack.toggle_target("current", &live(&["current", "recent"])),
            Some("recent".to_string())
        );
    }

    #[test]
    fn toggle_target_none_when_alone_or_empty() {
        let stack = SessionStack::parse("current\n");
        assert_eq!(stack.toggle_target("current", &live(&["current"])), None);
        assert_eq!(
            SessionStack::default().toggle_target("current", &live(&["current"])),
            None
        );
    }

    #[test]
    fn claim_toggle_slot_debounces_rapid_claims() {
        let dir = PathBuf::from("target/zps-claim-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("toggle.claim");
        std::fs::remove_file(&path).ok();
        assert!(claim_toggle_slot(&path));
        assert!(!claim_toggle_slot(&path));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn claim_toggle_slot_ignores_corrupt_claim_file() {
        let dir = PathBuf::from("target/zps-claim-corrupt-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("toggle.claim");
        std::fs::write(&path, "not-a-number").unwrap();
        assert!(claim_toggle_slot(&path));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn read_stack_of_missing_file_is_empty() {
        let stack = read_stack(Path::new("/nonexistent/dir/stack.v1"));
        assert_eq!(stack.entries().len(), 0);
    }

    #[test]
    fn write_then_read_round_trips() {
        // The wasm test runner maps only the project dir (see
        // .cargo/config.toml), so scratch files must live under it.
        let dir = PathBuf::from("target/zps-stack-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("session-stack.v1");
        let mut stack = SessionStack::default();
        stack.push_top("alpha");
        stack.push_top("beta");
        write_stack(&path, &stack);
        assert_eq!(read_stack(&path), stack);
        std::fs::remove_dir_all(&dir).ok();
    }
}
