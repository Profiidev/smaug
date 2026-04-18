ARG TARGET=x86_64-unknown-linux-gnu
ARG RUSTFLAGS="-C target-feature=+crt-static"

FROM ghcr.io/profiidev/images/rust-gnu-builder:main@sha256:34cee96885e1080da4e0a9a8a86dd8db503796bfc140a13b4e1a0f72784644ab AS wings-planner

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

FROM ghcr.io/profiidev/images/rust-gnu-builder:main@sha256:34cee96885e1080da4e0a9a8a86dd8db503796bfc140a13b4e1a0f72784644ab AS wings-builder

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

FROM node:24-alpine@sha256:d1b3b4da11eefd5941e7f0b9cf17783fc99d9c6fc34884a665f40a06dbdfc94f

ARG FRONTEND_DIR

COPY --from=wings-builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

WORKDIR /app
COPY --from=wings-builder /app/app /usr/local/bin/smaug-wings

ENTRYPOINT ["smaug-wings"]