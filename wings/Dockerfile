ARG TARGETARCH
# If TARGETARCH is amd64, result is x86_64. If arm64, result is aarch64.
ARG RUST_ARCH=${TARGETARCH/amd64/x86_64}
ARG RUST_ARCH=${RUST_ARCH/arm64/aarch64}
ARG TARGET=${RUST_ARCH}-unknown-linux-gnu
ARG RUSTFLAGS="-C target-feature=+crt-static --cfg reqwest_unstable"

FROM ghcr.io/profiidev/images/rust-gnu-builder:main@sha256:ff57874d1ac77b2bde727c89e113ac21eb6b001f77c859fed6b31f008a1acfd8 AS wings-planner

ARG TARGET
ARG RUSTFLAGS

COPY wings/Cargo.toml wings/
COPY shared/Cargo.toml shared/
COPY ./Cargo.lock ./Cargo.toml ./

RUN sed -i '/^members = /c\members = ["wings", "shared"]' Cargo.toml

RUN \
  --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/app/target \
  cargo chef prepare --recipe-path recipe.json --bin wings

FROM ghcr.io/profiidev/images/rust-gnu-builder:main@sha256:ff57874d1ac77b2bde727c89e113ac21eb6b001f77c859fed6b31f008a1acfd8 AS wings-builder

ARG TARGET
ARG RUSTFLAGS

COPY --from=wings-planner /app/recipe.json .

RUN \
  --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/app/target \
  cargo chef cook --release --target $TARGET

COPY wings/Cargo.toml wings/
COPY wings/src wings/src
COPY shared/Cargo.toml shared/
COPY shared/src shared/src
COPY ./Cargo.lock ./Cargo.toml ./

RUN sed -i '/^members = /c\members = ["wings", "shared"]' Cargo.toml

RUN \
  --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/app/target \
  cd wings && cargo build --release --target $TARGET \
  && mv ../target/$TARGET/release/wings ../app

RUN groupadd -r smaug && useradd -r -g smaug smaug

FROM scratch

COPY --from=wings-builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=wings-builder /etc/passwd /etc/passwd
COPY --from=wings-builder /etc/group /etc/group

WORKDIR /app
COPY --from=wings-builder /app/app /usr/local/bin/smaug-wings

EXPOSE 8000

USER smaug

ENTRYPOINT ["smaug-wings"]
