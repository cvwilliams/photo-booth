# syntax=docker/dockerfile:1

FROM rust:1-trixie AS base
ARG TRUNK_VERSION=0.22.1
ARG TARGETARCH
WORKDIR /app
ENV CARGO_TERM_COLOR=always \
    CARGO_TARGET_DIR=/app/target
RUN rustup target add wasm32-unknown-unknown \
 && rustup component add clippy rustfmt \
 && case "$TARGETARCH" in \
      arm64) TRIPLE=aarch64-unknown-linux-musl ;; \
      *)     TRIPLE=x86_64-unknown-linux-musl ;; \
    esac \
 && curl -fsSL "https://github.com/trunk-rs/trunk/releases/download/v${TRUNK_VERSION}/trunk-${TRIPLE}.tar.gz" \
    | tar -xz -C /usr/local/bin trunk \
 && trunk --version

FROM base AS dev
EXPOSE 8080
CMD ["trunk", "serve"]

FROM base AS build
COPY . .
RUN cargo fmt --all --check \
 && cargo clippy --workspace --all-targets --target wasm32-unknown-unknown -- -D warnings \
 && cargo test -p booth-core \
 && trunk build --release

FROM node:24-bookworm-slim AS firebase
ENV NPM_CONFIG_UPDATE_NOTIFIER=false \
    NPM_CONFIG_FUND=false
RUN npm install -g firebase-tools
WORKDIR /app
ENTRYPOINT ["firebase"]
