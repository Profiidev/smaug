ARG TARGET=x86_64-unknown-linux-gnu
ARG RUSTFLAGS="-C target-feature=+crt-static --cfg reqwest_unstable"
ARG FRONTEND_DIR=/app/frontend
ARG FRONTEND_URL="http://localhost:3000"
ARG BACKEND_URL="http://localhost:8000"

FROM node:24-alpine@sha256:d1b3b4da11eefd5941e7f0b9cf17783fc99d9c6fc34884a665f40a06dbdfc94f AS frontend-builder

WORKDIR /app/frontend

COPY frontend/package.json ./
COPY package-lock.json package.json ../

RUN npm ci

ARG FRONTEND_URL
ARG BACKEND_URL

COPY frontend/svelte.config.js frontend/tsconfig.json frontend/vite.config.ts ./
COPY frontend/src ./src
COPY frontend/static ./static

RUN npm run build

FROM ghcr.io/profiidev/images/rust-gnu-builder:main@sha256:34cee96885e1080da4e0a9a8a86dd8db503796bfc140a13b4e1a0f72784644ab AS backend-planner

ARG TARGET
ARG RUSTFLAGS

COPY backend/Cargo.toml backend/
COPY backend/entity/Cargo.toml backend/entity/
COPY backend/migration/Cargo.toml backend/migration/
COPY shared/Cargo.toml shared/
COPY ./Cargo.lock ./Cargo.toml ./

RUN sed -i '/^members = /c\members = ["backend", "backend/entity", "backend/migration", "shared"]' Cargo.toml

RUN \
  --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/app/target \
  cargo chef prepare --recipe-path recipe.json --bin backend

FROM ghcr.io/profiidev/images/rust-gnu-builder:main@sha256:34cee96885e1080da4e0a9a8a86dd8db503796bfc140a13b4e1a0f72784644ab AS backend-builder

ARG TARGET
ARG RUSTFLAGS
ARG FRONTEND_DIR

COPY --from=backend-planner /app/recipe.json .

RUN \
  --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/app/target \
  cargo chef cook --release --target $TARGET

COPY backend/Cargo.toml backend/
COPY backend/src backend/src
COPY backend/entity/Cargo.toml backend/entity/
COPY backend/entity/src backend/entity/src
COPY backend/migration/Cargo.toml backend/migration/
COPY backend/migration/src backend/migration/src
COPY shared/Cargo.toml shared/
COPY shared/src shared/src
COPY ./Cargo.lock ./Cargo.toml ./

RUN sed -i '/^members = /c\members = ["backend", "backend/entity", "backend/migration", "shared"]' Cargo.toml

RUN \
  --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/app/target \
  cd backend && cargo build --release --target $TARGET \
  && mv ../target/$TARGET/release/backend ../app

FROM node:24-alpine@sha256:d1b3b4da11eefd5941e7f0b9cf17783fc99d9c6fc34884a665f40a06dbdfc94f

ARG FRONTEND_DIR

ENV DB_URL="sqlite:/data/smaug.db?mode=rwc"
RUN mkdir /data

COPY --from=backend-builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

WORKDIR /app
COPY --from=frontend-builder /app/frontend/build /app/frontend
COPY --from=frontend-builder /app/frontend/package.json /app/frontend/package.json
COPY --from=frontend-builder /app/package-lock.json /app/package-lock.json
COPY --from=backend-builder /app/app /usr/local/bin/smaug

ENTRYPOINT ["smaug"]