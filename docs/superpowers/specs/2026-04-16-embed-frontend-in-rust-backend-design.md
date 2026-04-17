# Embed Frontend in Rust Backend

## Background

The frontend was previously served by a separate nginx container, resulting in redundancy:
- Two separate Docker images (backend + frontend/nginx)
- Three services in docker-compose (db, backend, frontend)
- nginx config with proxy pass, gzip, caching

Goal: embed the Vue 3 frontend into the Rust binary (like Go's `embed.FS`), producing a single binary that serves both the API and the frontend.

## Design

### Embedding Mechanism

Use [`rust-embed`](https://crates.io/crates/rust-embed) crate to embed `frontend/dist/` at compile time.

```rust
#[derive(RustEmbed)]
#[folder = "frontend/dist/"]
struct FrontendAssets;
```

In debug mode, `rust-embed` reads from the filesystem (convenient for development). In release mode, files are embedded in the binary.

### Routing

Given `admin_prefix` from config (e.g. `/super_admin/`):

| Path | Handler |
|------|---------|
| `{admin_prefix}/api/...` | Existing API routes (unchanged) |
| `{admin_prefix}/swagger-ui/...` | Existing Swagger UI (unchanged) |
| `{admin_prefix}/` | Serve `index.html` from embedded assets |
| `{admin_prefix}/{path}` | Serve matching file from embedded assets, 404 if not found |
| `/*` (everything else) | Existing fallback dispatcher (XSS receiver logic, unchanged) |

No history-mode fallback needed - the frontend uses hash routing.

Implementation in `src/controllers/mod.rs::get_app_router`:
- Add a new handler function (e.g. `frontend::serve`) that reads from `FrontendAssets`
- Mount it as a nested router under `admin_prefix`, after the `/api/` routes
- The handler extracts the path, looks up the file in `FrontendAssets::get(path)`, returns the content with proper `Content-Type` (using `mime_guess`)
- For the root path `/`, serve `index.html`

### No Compression or Caching Headers

No gzip/brotli compression or cache headers needed. External reverse proxy handles this if required.

### Dockerfile: Three-Stage Build

```
Stage 1: Frontend Builder (node:25-slim)
  - Install pnpm
  - Copy frontend/, install deps, build
  - Output: frontend/dist/

Stage 2: Rust Builder (rustlang/rust:nightly)
  - Copy frontend/dist/ from Stage 1
  - Copy Cargo.toml, Cargo.lock, thirdparty/, src/, migrations/, diesel.toml
  - Build release binary (rust-embed embeds frontend/dist/ at compile time)

Stage 3: Runtime (debian:bookworm-slim)
  - Copy binary + ip2region data
  - Single binary serves everything
```

Mirror configurations (mirrors.ustc.edu.cn for apt, mirrors.bfsu.edu.cn for crates.io) remain as-is. NPM mirror will use the same pattern if needed.

### docker-compose.yml Changes

- Remove `frontend` service entirely
- `backend` service exposes port `8002:8000` directly (replacing nginx's `8002:80`)
- Remove `ADMIN_PREFIX` and `BACKEND_URL` environment variables (no longer needed)

### build.sh Changes

- Single image build only, remove any frontend image references

### Frontend .env.production

Remains `VITE_API_BASE_URL=./api/` - this works because the frontend and API are served under the same prefix.

## Files to Change

1. `Cargo.toml` - add `rust-embed` dependency
2. `src/controllers/mod.rs` - mount frontend static file routes under admin_prefix
3. `src/controllers/frontend.rs` (new) - handler to serve embedded frontend assets
4. `docker/Dockerfile` - three-stage build (node + rust + runtime)
5. `docker/docker-compose.yml` - remove frontend service, expose backend port
6. `docker/build.sh` - single image build
