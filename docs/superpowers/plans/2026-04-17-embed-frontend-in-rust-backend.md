# Embed Frontend in Rust Backend - Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Embed the Vue 3 frontend into the Rust binary using `rust-embed`, eliminating the separate nginx container.

**Architecture:** Use `rust-embed` to embed `frontend/dist/` at compile time. Add a new axum handler that serves embedded static files under the configurable `admin_prefix`. Rebuild the Dockerfile as a three-stage build (node → rust → runtime).

**Tech Stack:** rust-embed 8, axum 0.8, mime_guess (already in project), node 25 + pnpm for frontend build.

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `Cargo.toml` | Modify | Add `rust-embed` dependency |
| `src/controllers/frontend.rs` | Create | Embedded frontend asset handler |
| `src/controllers/mod.rs` | Modify | Register frontend module, mount frontend routes |
| `docker/Dockerfile` | Modify | Three-stage build (node + rust + runtime) |
| `docker/docker-compose.yml` | Modify | Remove frontend service, expose backend port |
| `docker/build.sh` | Modify | Single image build |

---

### Task 1: Add `rust-embed` Dependency

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add rust-embed to Cargo.toml**

Add `rust-embed` to the `[dependencies]` section in `Cargo.toml`, after `mime_guess`:

```toml
rust-embed = "8"
```

- [ ] **Step 2: Verify dependency resolves**

Run: `cargo check 2>&1 | tail -5`
Expected: compilation succeeds (warnings OK, no errors)

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "feat: add rust-embed dependency for frontend embedding"
```

---

### Task 2: Create Frontend Asset Handler

**Files:**
- Create: `src/controllers/frontend.rs`

- [ ] **Step 1: Create the frontend handler module**

Create `src/controllers/frontend.rs` with the following content:

```rust
use axum::{
    extract::Path,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "frontend/dist/"]
struct FrontendAssets;

fn serve_file(path: &str) -> Response {
    match FrontendAssets::get(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime.as_ref())],
                file.data,
            )
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn index() -> Response {
    serve_file("index.html")
}

pub async fn serve(Path(path): Path<String>) -> Response {
    serve_file(&path)
}
```

- [ ] **Step 2: Verify it compiles**

Create a placeholder `frontend/dist/index.html` so rust-embed has something to embed during development:

```bash
mkdir -p frontend/dist && echo '<html><body>placeholder</body></html>' > frontend/dist/index.html
```

Run: `cargo check 2>&1 | tail -5`
Expected: compilation succeeds

- [ ] **Step 3: Commit**

```bash
git add src/controllers/frontend.rs
git commit -m "feat: add frontend asset handler using rust-embed"
```

---

### Task 3: Mount Frontend Routes in Router

**Files:**
- Modify: `src/controllers/mod.rs`

- [ ] **Step 1: Register the frontend module**

In `src/controllers/mod.rs`, add the `frontend` module declaration alongside existing ones. Find:

```rust
mod file;
mod http_log;
mod index;
mod route;
mod system_log;
mod user;
```

Add `mod frontend;` to the list:

```rust
mod file;
mod frontend;
mod http_log;
mod index;
mod route;
mod system_log;
mod user;
```

- [ ] **Step 2: Mount frontend routes under admin_prefix**

In the `get_app_router` function, the current code nests `admin_router` (containing API + swagger) under `prefix`, then sets a fallback to `index::index`. We need to add frontend static file routes into the `admin_router` so they sit alongside the API routes under the same prefix.

Find this block in `get_app_router`:

```rust
    // add open api
    admin_router = admin_router.merge(
        SwaggerUi::new("/swagger-ui")
            .url(OPEN_API_URL, api)
            .config(Config::from(format!("{}{}", prefix, OPEN_API_URL))),
    );

    let router = if prefix.is_empty() || prefix == "/" {
        Router::new().merge(admin_router)
    } else {
        Router::new().nest(&prefix, admin_router)
    };

    return router.fallback(index::index).with_state(context);
```

Replace with:

```rust
    // add open api
    admin_router = admin_router.merge(
        SwaggerUi::new("/swagger-ui")
            .url(OPEN_API_URL, api)
            .config(Config::from(format!("{}{}", prefix, OPEN_API_URL))),
    );

    // add frontend static file routes
    let admin_router = Router::from(admin_router)
        .route("/", axum::routing::get(frontend::index))
        .route("/{*path}", axum::routing::get(frontend::serve));

    let router = if prefix.is_empty() || prefix == "/" {
        Router::new().merge(admin_router)
    } else {
        Router::new().nest(&prefix, admin_router)
    };

    return router.fallback(index::index).with_state(context);
```

Note: The `route("/", ...)` and `route("/{*path}", ...)` are added after the API routes, so `/api/...` and `/swagger-ui/...` take priority. The wildcard `{*path}` catches remaining paths for static file serving.

- [ ] **Step 3: Add axum::routing import if needed**

Check that `axum::routing` is accessible. The current file imports `axum::Router` — the `axum::routing::get` is accessed via full path in the code above, so no additional import is needed.

- [ ] **Step 4: Verify it compiles**

Run: `cargo check 2>&1 | tail -5`
Expected: compilation succeeds

- [ ] **Step 5: Commit**

```bash
git add src/controllers/mod.rs
git commit -m "feat: mount embedded frontend routes under admin_prefix"
```

---

### Task 4: Rebuild Dockerfile as Three-Stage Build

**Files:**
- Modify: `docker/Dockerfile`

- [ ] **Step 1: Rewrite the Dockerfile**

Replace the entire content of `docker/Dockerfile` with:

```dockerfile
# ---- Stage 1: Frontend Builder ----
FROM node:25-slim AS frontend-builder

RUN npm install -g pnpm

WORKDIR /app/frontend

# Copy package manifests first for layer caching
COPY frontend/package.json frontend/pnpm-lock.yaml ./

RUN pnpm install --frozen-lockfile

# Copy frontend source and build
COPY frontend/ ./

RUN pnpm build

# ---- Stage 2: Rust Builder ----
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
```

Key changes from original:
- Added Stage 1 (node:25-slim) for frontend build
- Stage 2 copies `frontend/dist/` from Stage 1 before Rust compile
- Dummy `frontend/dist/index.html` created during dep-caching layer so `rust-embed` doesn't fail
- Stage 3 is unchanged — single binary, no node/nginx runtime needed

- [ ] **Step 2: Commit**

```bash
git add docker/Dockerfile
git commit -m "feat: three-stage Dockerfile embedding frontend in Rust binary"
```

---

### Task 5: Update docker-compose.yml

**Files:**
- Modify: `docker/docker-compose.yml`

- [ ] **Step 1: Rewrite docker-compose.yml**

Replace the entire content of `docker/docker-compose.yml` with:

```yaml
services:
  db:
    image: postgres:17-alpine
    environment:
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: postgres
      POSTGRES_DB: postgres
    volumes:
      - ./db_data:/var/lib/postgresql/data
      - ./init.sql:/docker-entrypoint-initdb.d/init.sql
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 5s
      timeout: 5s
      retries: 5
    restart: unless-stopped

  backend:
    image: ccr.ccs.tencentyun.com/rmb122/xss-receiver-backend:latest
    ports:
      - 8002:8000
    volumes:
      - ./config.toml:/app/config.toml:ro
      - ./app_data:/app/storage
    depends_on:
      db:
        condition: service_healthy
    restart: unless-stopped
```

Changes:
- Removed `frontend` service entirely
- Added `ports: - 8002:8000` to `backend` (replacing nginx's `8002:80`)

- [ ] **Step 2: Commit**

```bash
git add docker/docker-compose.yml
git commit -m "feat: remove frontend service from docker-compose, expose backend port"
```

---

### Task 6: Update build.sh

**Files:**
- Modify: `docker/build.sh`

- [ ] **Step 1: Rewrite build.sh**

Replace the entire content of `docker/build.sh` with:

```bash
#!/bin/sh

## TIPS: 必须从仓库根目录运行

curl https://raw.githubusercontent.com/lionsoul2014/ip2region/refs/heads/master/data/ip2region_v4.xdb -o docker/ip2region_v4.xdb
curl https://raw.githubusercontent.com/lionsoul2014/ip2region/refs/heads/master/data/ip2region_v6.xdb -o docker/ip2region_v6.xdb

sudo docker build -f docker/Dockerfile -t ccr.ccs.tencentyun.com/rmb122/xss-receiver-backend:latest .
sudo docker push ccr.ccs.tencentyun.com/rmb122/xss-receiver-backend:latest
```

This is effectively the same as before (no frontend image to build/push anymore). Kept as-is since it already only builds the backend image.

- [ ] **Step 2: Commit**

```bash
git add docker/build.sh
git commit -m "chore: confirm build.sh only builds single backend image"
```

---

### Task 7: Clean Up and Final Verification

- [ ] **Step 1: Remove placeholder frontend/dist if present**

If `frontend/dist/index.html` was created as a placeholder in Task 2, remove it (it should be gitignored, but verify):

```bash
rm -rf frontend/dist
```

Verify `frontend/dist` is in `.gitignore` or `.dockerignore`. If not, add `frontend/dist/` to `.gitignore`.

- [ ] **Step 2: Final cargo check**

Run: `cargo check 2>&1 | tail -10`
Expected: compilation succeeds (note: in dev mode without `frontend/dist/`, `rust-embed` reads from filesystem — it will still compile but the directory can be empty/missing in debug mode)

- [ ] **Step 3: Commit all remaining changes**

```bash
git add -A
git commit -m "chore: clean up placeholder files and verify build"
```
