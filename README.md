## About

This is an simple Zellij plugin that allows switching sessions between local github projects.

## Dependencies

This plugin requires Zellij 0.44 or newer (releases up to 0.2.x support Zellij 0.43).

This plugin requires the `fd` command, see the [fd github project](https://github.com/sharkdp/fd?tab=readme-ov-file#installation) for more
information and installation details.

## Configuration

To use this in your own Zellij setup add a section similar to the following to your zellij keybinds config:

```kdl
keybinds {
  bind "Ctrl s" {
    LaunchOrFocusPlugin "https://github.com/leapingfrogs/zellij-project-switcher/releases/latest/download/zellij-project-switcher-plugin.wasm" {
        floating true
        roots "/Users/xyzzy/<parent_of_git_repos>"
        width "20%"
        height "20%"
        x "40%"
        y "40%"
    }
  }
}
```

The `roots` attribute accepts multiple paths separated by a `:`, I recommend pointing to parent folders of your common roots into which
you checkout git repositories, as this helps keep the performance up. Internally the plugin passes these to the `fd` tool to find local 
repositories. You will need to first install that for the plugin to run, see [dependencies](#dependencies).


## Active sessions

Running Zellij sessions are listed first, sorted alphabetically and shown in
orange with a `●`, followed by the other projects. The session you are in is
left out of the list. Sessions that aren't in a project under `roots` are
included too, and selecting a running session attaches to it. Exited sessions
aren't treated as active.

## Session stack & toggle (Cmd-Tab for sessions)

The plugin maintains a most-recently-used stack of sessions in a file shared by
all of its instances (`/cache/session-stack.v1`, backed by
`~/.cache/zellij/<plugin-url>/plugin_cache/` on the host, so it survives
restarts). Whenever a session gains a client — via this plugin's menu, the
built-in session-manager, or `zellij attach` — that session moves to the top of
the stack.

The `toggle_session` command switches to the previous session in the stack, so
pressing it repeatedly bounces between your two most recent sessions, like
Cmd-Tab on macOS.

Two pieces of configuration are needed. First, load a background tracker
instance in every session so switches made outside the plugin are recorded:

```kdl
load_plugins {
    "https://github.com/leapingfrogs/zellij-project-switcher/releases/latest/download/zellij-project-switcher-plugin.wasm" {
        mode "tracker"
    }
}
```

Second, bind a key that pipes the toggle command to the tracker:

```kdl
keybinds {
    shared_except "locked" {
        bind "Ctrl y" {
            MessagePlugin "https://github.com/leapingfrogs/zellij-project-switcher/releases/latest/download/zellij-project-switcher-plugin.wasm" {
                name "toggle_session"
                mode "tracker"
            }
        }
    }
}
```

Notes:

- **The plugin URL and the `mode "tracker"` line must be byte-identical in
  both places.** Zellij identifies plugin instances by URL *plus*
  configuration; a mismatch makes the keybind launch a separate instance
  instead of messaging the tracker.
- On the first session after adding `load_plugins`, Zellij shows a one-time
  permission prompt for the tracker (it needs to read application state and
  switch sessions). The grant is cached per plugin URL.
- The toggle can also be invoked from a terminal (note the payload argument —
  without one the CLI only sends an end-of-pipe marker, which is ignored):

  ```bash
  zellij pipe --plugin <same-url> --plugin-configuration "mode=tracker" --name toggle_session -- go
  ```
- Zellij runs one tracker instance per client that has ever attached to a
  session (it never drops them on detach), and a plugin can only switch its
  own client. Each instance therefore checks `list_clients()` first and acts
  only if its own client is still attached.
- With multiple clients attached to different sessions simultaneously,
  toggling from two sessions at nearly the same moment can race; the plugin
  debounces toggles within 500ms machine-wide. Single-client use — the normal
  case — is unaffected.

## Development

*Note*: you will need to have `wasm32-wasi` added to rust as a target to build the plugin. This can be done with `rustup target add wasm32-wasi`.

You may also need wasmtime, try:

```bash
curl https://wasmtime.dev/install.sh -sSf | bash
```

## Inside Zellij
![img-2023-06-14-143355](https://github.com/zellij-org/rust-plugin-example/assets/795598/d9e563dc-5d71-4e10-af5b-190365bdca3b)

You can load the `./plugin-dev-workspace.kdl` file as a Zellij layout to get a terminal development environment:

Either when starting Zellij:
```
zellij --layout ./plugin-dev-workspace.kdl
```
*Note that in this case there's a small bug where the plugin is opened twice, it can be remedied by closing the oldest instance or loading with the new-tab action as secified below - this will be addressed in the near future*

Or from a running Zellij session:
```bash
zellij action new-tab --layout ./plugin-dev-workspace.kdl
```

## Otherwise

1. Build the project: `cargo build`
2. Load it inside a running Zellij session: `zellij action start-or-reload-plugin file:target/wasm32-wasi/debug/rust-plugin-example.wasm`
3. Repeat on changes (perhaps with a `watchexec` or similar command to run on fs changes).

## Releasing

To release a new version:

1. Update the version in `Cargo.toml` and run `cargo check` to update `Cargo.lock`
2. Commit both files and push to main. The release builds with `--locked`, so it fails if `Cargo.lock` is out of date
3. Once CI passes on main, create and push a git tag matching the version. Only repository admins can create `v*` tags:
   ```bash
   git tag -m "v0.x.x" v0.x.x
   git push origin v0.x.x
   ```
4. The GitHub Action will automatically run the tests, build the wasm plugin, attest its build provenance and create a release with the artifact attached

Releases are immutable once published, so a bad release needs a new version number. If the workflow fails before publishing, fix the problem, delete the tag and tag again.

## Verifying a release

Release builds from v0.3.5 onwards have a signed build provenance attestation. Zellij doesn't check it when loading a plugin, but you can check a downloaded file with the [GitHub CLI](https://cli.github.com/):

```bash
gh attestation verify zellij-project-switcher-plugin.wasm --repo leapingfrogs/zellij-project-switcher
```
