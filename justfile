build:
	cargo build --features tracing

build-release:
	cargo build --release --features tracing

test:
	# cargo wasi test --all
	cargo watch -x "nextest run --tests"

lint:
	cargo clippy --all-targets --all-features
	cargo audit


