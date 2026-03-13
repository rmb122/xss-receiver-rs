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

# Copy the rest of the source code
COPY src/ src/
COPY migrations/ migrations/
COPY diesel.toml ./

# Configure crates.io mirror for faster dependency download
RUN mkdir -p /usr/local/cargo && \
    printf '[source.crates-io]\nreplace-with = "mirror"\n\n[source.mirror]\nregistry = "sparse+https://mirrors.bfsu.edu.cn/crates.io-index/"\n' \
    > /usr/local/cargo/config.toml

# Build release binary
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
