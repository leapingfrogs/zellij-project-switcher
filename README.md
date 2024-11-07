## About

This is an simple Zellij plugin that allows switching sessions between local github projects.

## Dependencies

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
