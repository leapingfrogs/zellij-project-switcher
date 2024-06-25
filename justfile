build:
	cargo build --features tracing

run: build
	zellij -l ./plugin-dev-workspace.kdl -s zps-dev

test:
	cargo watch -x "nextest run --lib"

lint:
	cargo clippy --all-targets --all-features
	cargo audit


