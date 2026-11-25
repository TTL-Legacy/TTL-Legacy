# ── Stage 1: Builder ─────────────────────────────────────────────────────────
FROM rust:1.78-slim AS builder

# Install build-time dependencies (OpenSSL headers + pkg-config for crates that
# link against libssl, e.g. reqwest with native-tls)
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy workspace-level manifests first so Docker can cache the dependency layer
COPY Cargo.toml Cargo.lock ./

# Copy the backend package manifest
COPY backend/Cargo.toml ./backend/Cargo.toml

# Copy the full source tree (contracts + backend)
COPY . .

# Build only the backend binary in release mode
RUN cargo build --release --package ttl-legacy-backend

# ── Stage 2: Runtime ─────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies:
#   - ca-certificates: needed by reqwest / TLS calls
#   - curl: used by the HEALTHCHECK instruction below
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the compiled binary from the builder stage
COPY --from=builder /app/target/release/ttl-legacy-backend ./ttl-legacy-backend

EXPOSE 3000

# Docker-native healthcheck — hits the /health endpoint every 30 s.
# The container is marked unhealthy after 3 consecutive failures.
HEALTHCHECK --interval=30s --timeout=10s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

CMD ["./ttl-legacy-backend"]

# ── Stage 3: Development (extends runtime) ───────────────────────────────────
# docker-compose.override.yml targets this stage so the dev container gets
# richer error output without rebuilding the full binary.
FROM runtime AS development

ENV RUST_ENV=development
ENV RUST_BACKTRACE=1
