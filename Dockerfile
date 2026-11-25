# Multi-stage build for Aurexai backend (Rust + Axum + SeaORM + Postgres).

FROM rust:1-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Dependency cache layer
COPY Cargo.toml Cargo.lock ./
COPY migration/Cargo.toml ./migration/Cargo.toml
RUN mkdir -p src migration/src \
    && printf 'pub fn dummy() {}\n' > src/lib.rs \
    && printf 'fn main() {}\n' > src/main.rs \
    && printf 'pub fn dummy() {}\n' > migration/src/lib.rs \
    && printf 'fn main() {}\n' > migration/src/main.rs \
    && cargo build --release --locked \
    && rm -rf src migration/src \
    && find target/release -maxdepth 2 \( -name '*aurexai_backend*' -o -name '*aurexai-backend*' -o -name '*migration*' \) -exec rm -rf {} + \
    && find target/release -maxdepth 3 -path '*fingerprint/*aurexai*' -exec rm -rf {} + \
    && find target/release -maxdepth 3 -path '*fingerprint/*migration*' -exec rm -rf {} +

# Real build
COPY src ./src
COPY migration ./migration
COPY templates ./templates

RUN touch src/lib.rs src/main.rs migration/src/lib.rs migration/src/main.rs \
    && cargo build --release --locked \
    && test -x target/release/aurexai-backend \
    && test "$(stat -c%s target/release/aurexai-backend)" -gt 1000000

ARG GIT_SHA=unknown
RUN { echo "commit ${GIT_SHA}"; date -u +"built %Y-%m-%dT%H:%M:%SZ"; } > /app/BUILD_INFO

# Admin panel (React + Vite)
FROM node:24-slim AS frontend
WORKDIR /admin
COPY admin/package.json admin/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY admin/ ./
RUN npm run build

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 10001 appuser \
    && mkdir -p /app \
    && chown -R appuser:appuser /app

WORKDIR /app
COPY --from=builder --chown=appuser:appuser /app/target/release/aurexai-backend /app/aurexai-backend
COPY --from=builder --chown=appuser:appuser /app/BUILD_INFO /app/BUILD_INFO
COPY --from=frontend --chown=appuser:appuser /admin/dist /app/admin/dist

USER appuser
ENV APP_HOST=0.0.0.0 \
    APP_PORT=8080 \
    RUST_LOG=info

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
    CMD curl -fsS http://127.0.0.1:${APP_PORT:-8080}/api/v1/health/ || exit 1

CMD ["/app/aurexai-backend"]
