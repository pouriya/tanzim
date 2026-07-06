LOG_FEATURE = ,tracing
TARGET_OPTION =

.PHONY: all build test clippy check-style version-check docs open-docs examples example-full cli cli-docker

all: build clippy test check-style

cli:
	cargo build --release --manifest-path tanzim/Cargo.toml
	mkdir -p bin
	cp tanzim/target/release/tanzim bin/tanzim

# Optionally set PLATFORM to cross-build (e.g. PLATFORM=linux/arm64). Login and push are
# intentionally NOT here -- the release workflow does the multi-arch build & push.
cli-docker:
	docker build $(if $(PLATFORM),--platform $(PLATFORM),) -t tanzim -f Dockerfile .

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

version-check:
	./crates/versioning.sh --check

docs:
	cargo doc --workspace --all-features

open-docs:
	cargo doc --workspace --all-features --open

# Build every workspace example in release mode and copy each resulting binary
# into bin/ under a sanitised snake-case name (hyphens become underscores; the
# words "example" and "tanzim" are stripped out). This covers the root
# examples/<name>/main.rs as well as every crates/*/examples/*.rs.
examples:
	mkdir -p bin
	cargo build $(TARGET_OPTION) --release --workspace --all-features --examples
	@for src in examples/*/ crates/*/examples/*.rs; do \
		[ -e "$$src" ] || continue; \
		if [ -d "$$src" ]; then name=$$(basename $$src); \
		else name=$$(basename $$src .rs); fi; \
		out=$$(echo $$name | tr 'A-Z-' 'a-z_' \
			| sed -e 's/example//g' -e 's/tanzim//g' \
			      -e 's/__*/_/g' -e 's/^_//' -e 's/_$$//'); \
		echo ">>> $$name -> bin/$$out"; \
		cp target/release/examples/$$name bin/$$out; \
	done

# Demo run of the "full" example against sample sources under examples/full/etc.
example-full:
	env \
	'APP_NAME.FOO.SERVER.ADDRESS=127.0.0.1' \
	'APP_NAME.BAR.SQLITE.FILE=/path/to/app.db' \
	'APP_NAME.BAZ.LOGGING.LEVEL=debug' \
	'APP_NAME.QUX.HTTPS.INSECURE=false' \
	RUST_BACKTRACE=1 cargo run $(TARGET_OPTION) --release -p tanzim --no-default-features \
		--features="full$(LOG_FEATURE)" --example full -- \
		--trace 'env(prefix=APP_NAME,separator=".")' 'file:examples/full/etc'
