# syntax=docker/dockerfile:1
#
# Static, musl-linked shellcheck-rust in a scratch image (~ a few MB).
#
#   docker build --memory=6g -t shellcheck-rust .
#   docker run --rm -i shellcheck-rust -f gcc - < script.sh
#
# The build stage runs on Alpine's Rust toolchain, whose host target is
# x86_64-unknown-linux-musl, so the binary is fully static with no libc
# dependency. Override the Rust version with --build-arg RUST_VERSION=...

ARG RUST_VERSION=1.93

FROM rust:${RUST_VERSION}-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY . .
# CARGO_BUILD_JOBS keeps peak memory bounded under `docker build --memory=6g`
# (the release profile uses fat LTO and a single codegen unit).
ENV CARGO_BUILD_JOBS=4
RUN cargo build --release --locked -p shellcheck-rust \
 && strip target/release/shellcheck-rust \
 && mkdir -p /out \
 && cp target/release/shellcheck-rust /out/shellcheck-rust

FROM scratch
LABEL org.opencontainers.image.title="shellcheck-rust" \
      org.opencontainers.image.description="A Rust port of ShellCheck" \
      org.opencontainers.image.licenses="GPL-3.0-or-later"
COPY --from=build /out/shellcheck-rust /shellcheck-rust
ENTRYPOINT ["/shellcheck-rust"]
CMD ["--help"]
