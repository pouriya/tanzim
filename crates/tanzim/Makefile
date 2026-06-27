TARGET_OPTION =

.PHONY: build test clippy style

build:
	cargo build -p tanzim --no-default-features
	cargo build -p tanzim
	cargo build -p tanzim --no-default-features --features env
	cargo build -p tanzim --no-default-features --features file
	cargo build -p tanzim --no-default-features --features json
	cargo build -p tanzim --no-default-features --features yaml
	cargo build -p tanzim --no-default-features --features toml
	cargo build -p tanzim --no-default-features --features logging
	cargo build -p tanzim --no-default-features --features tracing
	cargo build -p tanzim --no-default-features --features env,file,json,yaml,toml
	cargo build -p tanzim --no-default-features --features env,file,json,yaml,toml,logging
	cargo build -p tanzim --no-default-features --features env,file,json,yaml,toml,tracing
	cargo build -p tanzim --all-features

test:
	cargo test $(TARGET_OPTION) -p tanzim --all-features

clippy:
	cargo clippy -p tanzim --all-features -- -D warnings

style:
	cargo fmt -p tanzim -- --check
