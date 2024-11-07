build:
	cargo build --features tracing

build-release:
	cargo build --release --features tracing

run: build
	zellij -l ./plugin-dev-workspace.kdl -s zps-dev

test:
	cargo watch -x "nextest run --tests"

lint:
	cargo clippy --all-targets --all-features
	cargo audit


