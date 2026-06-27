TARGET_OPTION =

.PHONY: build test clippy style

build:
	cargo build -p tanzim-merge --no-default-features
	cargo build -p tanzim-merge

test:
	cargo test $(TARGET_OPTION) -p tanzim-merge --all-features

clippy:
	cargo clippy -p tanzim-merge --all-features -- -D warnings

style:
	cargo fmt -p tanzim-merge -- --check
