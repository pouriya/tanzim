TARGET_OPTION =

.PHONY: build test clippy style

build:
	cargo build -p tanzim-testing
	cargo build -p tanzim-testing --no-default-features --features logging
	cargo build -p tanzim-testing --no-default-features --features tracing
	cargo build -p tanzim-testing --all-features

test:
	cargo test $(TARGET_OPTION) -p tanzim-testing --all-features

clippy:
	cargo clippy -p tanzim-testing --all-features -- -D warnings

style:
	cargo fmt -p tanzim-testing -- --check
