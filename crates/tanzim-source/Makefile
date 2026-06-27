TARGET_OPTION =

.PHONY: build test clippy style

build:
	cargo build -p tanzim-source --no-default-features
	cargo build -p tanzim-source
	cargo build -p tanzim-source --features serde
	cargo build -p tanzim-source --all-features

test:
	cargo test $(TARGET_OPTION) -p tanzim-source --all-features

clippy:
	cargo clippy -p tanzim-source --all-features -- -D warnings

style:
	cargo fmt -p tanzim-source -- --check
