# ---- Stage 1: Frontend Builder ----
FROM node:25-slim AS frontend-builder

RUN npm install -g pnpm

WORKDIR /app/frontend

# Copy package manifests first for layer caching
COPY frontend/package.json frontend/pnpm-lock.yaml frontend/pnpm-workspace.yaml ./

RUN pnpm install --frozen-lockfile

# Copy frontend source and build
COPY frontend/ ./

# Raise V8 heap limit to avoid OOM (SIGABRT / exit 134) when type-check and
# vite build run in parallel for the monaco-editor / vuetify heavy bundle.
ENV NODE_OPTIONS=--max-old-space-size=4096

RUN pnpm build

# ---- Stage 2: Rust Builder ----
FROM rustlang/rust:nightly AS builder

RUN apt-get update && apt-get install -y \
    libpq-dev \
    pkg-config \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy ip2region xdb data files
COPY docker/ip2region_v4.xdb /app/data/
COPY docker/ip2region_v6.xdb /app/data/

# Copy manifests and lock file first (for better layer caching)
COPY Cargo.toml Cargo.lock ./

# Copy thirdparty local dependencies (referenced in [patch.crates-io])
COPY thirdparty/ thirdparty/

# Create dummy source to trigger dependency compilation
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs

# Create dummy frontend/dist so rust-embed compiles during dep caching
RUN mkdir -p frontend/dist && echo '<html></html>' > frontend/dist/index.html

# Build dependencies only (this layer is cached as long as Cargo.toml/Cargo.lock don't change)
RUN cargo build --release && rm -rf src target/release/deps/xss_receiver_rs* target/release/xss-receiver-rs

# Copy built frontend from Stage 1
COPY --from=frontend-builder /app/frontend/dist/ frontend/dist/

# Copy the real source code
COPY src/ src/
COPY migrations/ migrations/
COPY diesel.toml ./

# Build the actual project (only recompiles project source, not dependencies)
RUN cargo build --release

# ---- Stage 3: Runtime ----
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libpq5 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy compiled binary from builder
COPY --from=builder /app/target/release/xss-receiver-rs /app/xss-receiver-rs

# Copy ip2region xdb data files from builder
COPY --from=builder /app/data /app/data

# Create storage directory
RUN mkdir -p /app/storage

EXPOSE 8000

ENTRYPOINT ["/app/xss-receiver-rs"]
CMD ["/app/config.toml"]
