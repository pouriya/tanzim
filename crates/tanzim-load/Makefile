TARGET_OPTION =

.PHONY: build test clippy style

build:
	cargo build -p tanzim-load --no-default-features
	cargo build -p tanzim-load
	cargo build -p tanzim-load --no-default-features --features env
	cargo build -p tanzim-load --no-default-features --features file
	cargo build -p tanzim-load --no-default-features --features http
	cargo build -p tanzim-load --no-default-features --features logging
	cargo build -p tanzim-load --no-default-features --features tracing
	cargo build -p tanzim-load --no-default-features --features env,file
	cargo build -p tanzim-load --no-default-features --features env,file,http
	cargo build -p tanzim-load --no-default-features --features env,file,http,logging
	cargo build -p tanzim-load --no-default-features --features env,file,http,tracing
	cargo build -p tanzim-load --no-default-features --features full
	cargo build -p tanzim-load --all-features

test:
	cargo test $(TARGET_OPTION) -p tanzim-load --all-features

clippy:
	cargo clippy -p tanzim-load --all-features -- -D warnings

style:
	cargo fmt -p tanzim-load -- --check
