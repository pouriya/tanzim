TARGET_OPTION =

.PHONY: build test clippy style

build:
	cargo build -p tanzim-parse --no-default-features
	cargo build -p tanzim-parse
	cargo build -p tanzim-parse --no-default-features --features env
	cargo build -p tanzim-parse --no-default-features --features json
	cargo build -p tanzim-parse --no-default-features --features yaml
	cargo build -p tanzim-parse --no-default-features --features toml
	cargo build -p tanzim-parse --no-default-features --features logging
	cargo build -p tanzim-parse --no-default-features --features tracing
	cargo build -p tanzim-parse --no-default-features --features env,json
	cargo build -p tanzim-parse --no-default-features --features env,json,yaml,toml
	cargo build -p tanzim-parse --no-default-features --features env,json,yaml,toml,logging
	cargo build -p tanzim-parse --no-default-features --features env,json,yaml,toml,tracing
	cargo build -p tanzim-parse --all-features

test:
	cargo test $(TARGET_OPTION) -p tanzim-parse --all-features

clippy:
	cargo clippy -p tanzim-parse --all-features -- -D warnings

style:
	cargo fmt -p tanzim-parse -- --check
