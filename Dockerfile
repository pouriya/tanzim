# Build the tanzim CLI as a static musl binary and ship it on a minimal Alpine image.
#
# The CLI crate (`tanzim/`) is standalone (excluded from the library workspace) and depends
# on the facade at `../crates/tanzim`, so the build stage needs both `tanzim/` and `crates/`.

FROM rust:1.96.1-alpine3.24 AS builder
RUN apk add --no-cache musl-dev make
WORKDIR /build
# Root manifest is needed so the `crates/*` can resolve their `workspace.package`
# inheritance (edition/license/repository) while building the excluded CLI crate.
COPY Cargo.toml Makefile ./
COPY crates ./crates
COPY tanzim ./tanzim
# The build goes through the project's Makefile, which compiles in release mode and
# stages the binary at bin/tanzim.
RUN make cli

FROM alpine:3.24.1
COPY --from=builder /build/bin/tanzim /usr/local/bin/tanzim
ENTRYPOINT ["tanzim"]
