ARG TARGET=x86_64-unknown-linux-gnu
ARG RUSTFLAGS="-C target-feature=+crt-static"

FROM ghcr.io/profiidev/images/rust-gnu-builder:main AS wings-planner

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

FROM ghcr.io/profiidev/images/rust-gnu-builder:main AS wings-builder

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

FROM node:24-alpine

ARG FRONTEND_DIR

COPY --from=wings-builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

WORKDIR /app
COPY --from=wings-builder /app/app /usr/local/bin/smaug-wings

ENTRYPOINT ["smaug-wings"]