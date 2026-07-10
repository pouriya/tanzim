TARGET_OPTION =

.PHONY: build test clippy style

build:
	cargo build -p tanzim --no-default-features
	cargo build -p tanzim
	cargo build -p tanzim --no-default-features --features load-env
	cargo build -p tanzim --no-default-features --features load-file
	cargo build -p tanzim --no-default-features --features load-http-closure
	cargo build -p tanzim --no-default-features --features parse-env
	cargo build -p tanzim --no-default-features --features parse-json
	cargo build -p tanzim --no-default-features --features parse-yaml
	cargo build -p tanzim --no-default-features --features parse-toml
	cargo build -p tanzim --no-default-features --features validate-default
	cargo build -p tanzim --no-default-features --features validate-schema
	cargo build -p tanzim --no-default-features --features validate-integer
	cargo build -p tanzim --no-default-features --features validate-url
	cargo build -p tanzim --no-default-features --features validate-full
	cargo build -p tanzim --no-default-features --features logging
	cargo build -p tanzim --no-default-features --features tracing
	cargo build -p tanzim --no-default-features --features load-env,load-file,load-http-closure,parse-env,parse-json,parse-yaml,parse-toml
	cargo build -p tanzim --no-default-features --features full
	cargo build -p tanzim --no-default-features --features full,logging
	cargo build -p tanzim --no-default-features --features full,tracing
	cargo build -p tanzim --all-features

test:
	cargo test $(TARGET_OPTION) -p tanzim --all-features

clippy:
	cargo clippy -p tanzim --all-features -- -D warnings

style:
	cargo fmt -p tanzim -- --check
