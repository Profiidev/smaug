ARG TARGET=x86_64-unknown-linux-gnu
ARG RUSTFLAGS="-C target-feature=+crt-static"

FROM ghcr.io/profiidev/images/rust-gnu-builder:main AS wing-planner

ARG TARGET
ARG RUSTFLAGS

COPY wing/Cargo.toml wing/
COPY ./Cargo.lock ./Cargo.toml ./

RUN sed -i '/^members = /c\members = ["wing"]' Cargo.toml

RUN \
  --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/app/target \
  cargo chef prepare --recipe-path recipe.json --bin wing

FROM ghcr.io/profiidev/images/rust-gnu-builder:main AS wing-builder

ARG TARGET
ARG RUSTFLAGS

COPY --from=wing-planner /app/recipe.json .

RUN \
  --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/app/target \
  cargo chef cook --release --target $TARGET

COPY wing/Cargo.toml wing/
COPY wing/src wing/src
COPY ./Cargo.lock ./Cargo.toml ./

RUN sed -i '/^members = /c\members = ["wing"]' Cargo.toml

RUN \
  --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/app/target \
  cd wing && cargo build --release --target $TARGET \
  && mv ../target/$TARGET/release/wing ../app

FROM node:24-alpine

ARG FRONTEND_DIR

COPY --from=wing-builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

WORKDIR /app
COPY --from=wing-builder /app/app /usr/local/bin/smaug-wing

ENTRYPOINT ["smaug-wing"]