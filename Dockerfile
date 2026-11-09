# syntax=docker/dockerfile:1
#
# Static, musl-linked shelllint in a scratch image (~ a few MB).
#
#   docker build --memory=6g -t shelllint .
#   docker run --rm -i shelllint -f gcc - < script.sh
#
# The build stage runs on Alpine's Rust toolchain, whose host target is
# x86_64-unknown-linux-musl, so the binary is fully static with no libc
# dependency. Override the Rust version with --build-arg RUST_VERSION=...
#
# Two final stages share the build stage. The last one, the default, is the
# scratch image. `--target alpine` builds a variant with a shell (for CI
# runners that exec `sh -c`, such as GitLab CI):
#
#   docker build --target alpine -t shelllint:alpine .
#
# GitLab CI also needs the entrypoint cleared:
#   image: { name: ghcr.io/OWNER/shelllint:alpine, entrypoint: [""] }

ARG RUST_VERSION=1.93
ARG ALPINE_VERSION=3.22

FROM rust:${RUST_VERSION}-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY . .
# CARGO_BUILD_JOBS keeps peak memory bounded under `docker build --memory=6g`
# (the release profile uses fat LTO and a single codegen unit).
ENV CARGO_BUILD_JOBS=4
RUN cargo build --release --locked -p shelllint \
 && strip target/release/shelllint \
 && mkdir -p /out \
 && cp target/release/shelllint /out/shelllint

FROM alpine:${ALPINE_VERSION} AS alpine
LABEL org.opencontainers.image.title="shelllint" \
      org.opencontainers.image.description="A Rust port of ShellCheck (Alpine variant with a shell)" \
      org.opencontainers.image.licenses="GPL-3.0-or-later"
COPY --from=build /out/shelllint /usr/local/bin/shelllint
ENTRYPOINT ["/usr/local/bin/shelllint"]
CMD ["--help"]

# Keep this stage last so a plain `docker build .` still yields the scratch image.
FROM scratch
LABEL org.opencontainers.image.title="shelllint" \
      org.opencontainers.image.description="A Rust port of ShellCheck" \
      org.opencontainers.image.licenses="GPL-3.0-or-later"
COPY --from=build /out/shelllint /shelllint
ENTRYPOINT ["/shelllint"]
CMD ["--help"]
