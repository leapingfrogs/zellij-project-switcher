{ pkgs, ... }:

{
  # Native deps for host-target builds of transitive crates (openssl-sys via zellij-utils)
  packages = [
    pkgs.openssl
    pkgs.pkg-config
    pkgs.wasmtime # test runner for wasm32-wasip1 (see .cargo/config.toml)
    pkgs.just
    pkgs.cargo-watch
    pkgs.cargo-nextest
    pkgs.cargo-audit
    pkgs.cargo-outdated
  ];

  languages.rust = {
    enable = true;
    # nixpkgs channel can't cross-compile; stable channel comes from rust-overlay
    channel = "stable";
    targets = [ "wasm32-wasip1" ];
  };
}
