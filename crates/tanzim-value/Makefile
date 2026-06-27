TARGET_OPTION =

.PHONY: build test clippy style

build:
	cargo build -p tanzim-value
	cargo build -p tanzim-value --release

test:
	cargo test $(TARGET_OPTION) -p tanzim-value --all-features

clippy:
	cargo clippy -p tanzim-value --all-features -- -D warnings

style:
	cargo fmt -p tanzim-value -- --check
