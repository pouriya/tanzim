TARGET_OPTION =

.PHONY: build test clippy style

build:
	cargo build -p tanzim-validate --no-default-features
	cargo build -p tanzim-validate
	cargo build -p tanzim-validate --no-default-features --features boolean
	cargo build -p tanzim-validate --no-default-features --features either
	cargo build -p tanzim-validate --no-default-features --features enumeration
	cargo build -p tanzim-validate --no-default-features --features float
	cargo build -p tanzim-validate --no-default-features --features integer
	cargo build -p tanzim-validate --no-default-features --features list
	cargo build -p tanzim-validate --no-default-features --features net
	cargo build -p tanzim-validate --no-default-features --features non_empty
	cargo build -p tanzim-validate --no-default-features --features number
	cargo build -p tanzim-validate --no-default-features --features path
	cargo build -p tanzim-validate --no-default-features --features percentage
	cargo build -p tanzim-validate --no-default-features --features static_map
	cargo build -p tanzim-validate --no-default-features --features string
	cargo build -p tanzim-validate --no-default-features --features dynamic_map
	cargo build -p tanzim-validate --no-default-features --features logging
	cargo build -p tanzim-validate --no-default-features --features tracing
	cargo build -p tanzim-validate --no-default-features --features regex
	cargo build -p tanzim-validate --no-default-features --features url
	cargo build -p tanzim-validate --no-default-features --features cidr
	cargo build -p tanzim-validate --no-default-features --features uuid
	cargo build -p tanzim-validate --no-default-features --features semver
	cargo build -p tanzim-validate --no-default-features --features encoding
	cargo build -p tanzim-validate --no-default-features --features duration
	cargo build -p tanzim-validate --no-default-features --features bytesize
	cargo build -p tanzim-validate --no-default-features --features datetime
	cargo build -p tanzim-validate --no-default-features --features schema
	cargo build -p tanzim-validate --no-default-features --features string,regex,schema
	cargo build -p tanzim-validate --no-default-features --features full
	cargo build -p tanzim-validate --all-features

test:
	cargo test $(TARGET_OPTION) -p tanzim-validate --all-features

clippy:
	cargo clippy -p tanzim-validate --all-features -- -D warnings

style:
	cargo fmt -p tanzim-validate -- --check
