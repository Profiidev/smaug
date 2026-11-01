ARG TARGETARCH
# If TARGETARCH is amd64, result is x86_64. If arm64, result is aarch64.
ARG RUST_ARCH=${TARGETARCH/amd64/x86_64}
ARG RUST_ARCH=${RUST_ARCH/arm64/aarch64}
ARG TARGET=${RUST_ARCH}-unknown-linux-gnu
ARG RUSTFLAGS="-C target-feature=+crt-static --cfg reqwest_unstable"

FROM ghcr.io/profiidev/images/rust-gnu-builder:main@sha256:ec481bc66b89eb0d7e489013b2b0396155c9f863032ac8be3e75362e6c86be91 AS agent-planner

ARG TARGET
ARG RUSTFLAGS

COPY agent/Cargo.toml agent/
COPY shared/Cargo.toml shared/
COPY ./Cargo.lock ./Cargo.toml ./

RUN sed -i -e '/^members = \[/{:a;/\]/!{N;ba}' -e 'c\members = ["agent", "shared"]' -e '}' Cargo.toml

RUN \
  --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/app/target \
  cargo chef prepare --recipe-path recipe.json --bin smaug-agent

FROM ghcr.io/profiidev/images/rust-gnu-builder:main@sha256:ec481bc66b89eb0d7e489013b2b0396155c9f863032ac8be3e75362e6c86be91 AS agent-builder

ARG TARGET
ARG RUSTFLAGS

COPY --from=agent-planner /app/recipe.json .

RUN \
  --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/app/target \
  cargo chef cook --release --target $TARGET

COPY agent/Cargo.toml agent/
COPY agent/src agent/src
COPY shared/Cargo.toml shared/
COPY shared/src shared/src
COPY ./Cargo.lock ./Cargo.toml ./

RUN sed -i -e '/^members = \[/{:a;/\]/!{N;ba}' -e 'c\members = ["agent", "shared"]' -e '}' Cargo.toml

RUN \
  --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/app/target \
  cd agent && cargo build --release --target $TARGET \
  && mv ../target/$TARGET/release/smaug-agent ../app

FROM scratch

WORKDIR /app
COPY --from=agent-builder /app/app /usr/local/bin/smaug-agent

EXPOSE 8000

ENTRYPOINT ["smaug-agent"]
