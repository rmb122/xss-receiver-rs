# ---- Builder Stage ----
FROM rustlang/rust:nightly AS builder

RUN sed -i 's/deb.debian.org/mirrors.ustc.edu.cn/g' /etc/apt/sources.list.d/debian.sources && \
    apt-get update && apt-get install -y \
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

# Configure crates.io mirror for faster dependency download
RUN mkdir -p /usr/local/cargo && \
    printf '[source.crates-io]\nreplace-with = "mirror"\n\n[source.mirror]\nregistry = "sparse+https://mirrors.bfsu.edu.cn/crates.io-index/"\n' \
    > /usr/local/cargo/config.toml

# Create dummy source to trigger dependency compilation
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs

# Build dependencies only (this layer is cached as long as Cargo.toml/Cargo.lock don't change)
RUN cargo build --release && rm -rf src target/release/deps/xss_receiver_rs* target/release/xss-receiver-rs

# Copy the real source code
COPY src/ src/
COPY migrations/ migrations/
COPY diesel.toml ./

# Build the actual project (only recompiles project source, not dependencies)
RUN cargo build --release

# ---- Runtime Stage ----
FROM debian:bookworm-slim

RUN sed -i 's/deb.debian.org/mirrors.ustc.edu.cn/g' /etc/apt/sources.list.d/debian.sources && \
    apt-get update && apt-get install -y \
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
