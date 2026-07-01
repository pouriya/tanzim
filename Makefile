LOG_FEATURE = ,tracing
TARGET_OPTION =

.PHONY: all build test clippy check-style docs examples example-full

all: build clippy test check-style

build:
	$(MAKE) -C crates/tanzim-value build
	$(MAKE) -C crates/tanzim-source build
	$(MAKE) -C crates/tanzim-load build
	$(MAKE) -C crates/tanzim-parse build
	$(MAKE) -C crates/tanzim-merge build
	$(MAKE) -C crates/tanzim-validate build
	$(MAKE) -C crates/tanzim build

test:
	$(MAKE) -C crates/tanzim-value test
	$(MAKE) -C crates/tanzim-source test
	$(MAKE) -C crates/tanzim-load test
	$(MAKE) -C crates/tanzim-parse test
	$(MAKE) -C crates/tanzim-merge test
	$(MAKE) -C crates/tanzim-validate test
	$(MAKE) -C crates/tanzim test

clippy:
	cargo clippy --workspace --all-features --all-targets --no-deps -- -D warnings

check-style:
	cargo fmt --check --verbose

docs:
	cargo doc --workspace --all-features

examples: example-full

example-full:
	env \
	'APP_NAME.FOO.SERVER.ADDRESS=127.0.0.1' \
	'APP_NAME.BAR.SQLITE.FILE=/path/to/app.db' \
	'APP_NAME.BAZ.LOGGING.LEVEL=debug' \
	'APP_NAME.QUX.HTTPS.INSECURE=false' \
	RUST_BACKTRACE=1 cargo run $(TARGET_OPTION) -p tanzim --no-default-features \
		--features="full$(LOG_FEATURE)" --example full -- \
		--trace 'env(prefix=APP_NAME,separator=".")' 'file:examples/full/etc'
