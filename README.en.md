# xss-receiver-rs

[简体中文](./README.md) | English

A high-performance XSS / data receiver platform written in Rust. It ships with two programmable servers (HTTP and DNS) that let you flexibly capture, log, and craft custom responses to incoming requests. It is well suited for XSS data collection, SSRF / OOB probing, DNS Log, and other penetration-testing and security-research scenarios.

## Features

- **Programmable HTTP server**: route rules (exact or regex matching) map requests to different handlers:
  - `STATIC`: serve a static file from storage directly.
  - `SCRIPT`: build the response dynamically with the embedded JavaScript engine ([boa](https://github.com/boa-dev/boa)).
  - `NONE`: only log the request and return a default response.
- **Programmable DNS server**: rule-based routing as well, returning static answers or dynamically constructing `A` / `AAAA` / `CNAME` / `TXT` records via scripts. Great as a DNS Log.
- **Full request logging**: records the source, headers, query, body, uploaded files, etc. of HTTP / DNS requests, with IP geolocation powered by [ip2region](https://github.com/lionsoul2014/ip2region).
- **Built-in script engine**: scripts run as ES modules with top-level `await`, can import other modules from storage, make outbound HTTP requests through `http`, and access `request`, `response`, `storage`, and `cache`.
- **File storage management**: browse directories, upload (including chunked upload and merge), download, rename, and delete files, usable directly by static routes and scripts.
- **Modern web admin panel**: built with Vue 3 + Vuetify, providing management for HTTP / DNS routes and logs, files, users, and system logs.
- **Security & ops friendly**: JWT authentication, customizable admin prefix (`admin_prefix`), real client IP resolution behind reverse proxies, and optional OpenAPI / Swagger docs.

## Tech Stack

- Backend: Rust 2024 edition, [axum](https://github.com/tokio-rs/axum), [tokio](https://tokio.rs/), [diesel](https://diesel.rs/) + [diesel-async](https://github.com/weiznich/diesel_async) (PostgreSQL).
- Script engine: [boa_engine](https://github.com/boa-dev/boa).
- Frontend: Vue 3, Vuetify, Vite, Monaco Editor (embedded into the binary via `rust-embed`).
- Database: PostgreSQL.

## Quick Start (Docker Compose)

Deploying with Docker Compose is recommended. The related files live in the `docker/` directory. Images are automatically built by GitHub Actions and pushed to the GitHub Container Registry (GHCR). `docker-compose.yml` already uses the prebuilt image `ghcr.io/rmb122/xss-receiver-rs:latest` by default, so **there is no need to build from source locally**.

1. Get the deployment files: clone the repository, or download just `docker-compose.yml` and `config.toml` from the `docker/` directory.

2. Prepare the config file `docker/config.toml` and replace the placeholders with real values:
   - `jwt_secret`: the JWT signing key. Leave empty to generate a random one on each start (which invalidates already-issued tokens).
   - `admin_prefix`: the access prefix for the admin panel. It **must not be the root path `/`**; pick something hard to guess, e.g. `/a_secret_admin_path/`.

3. Pull the image and start the services:

```bash
cd docker
docker compose up -d
```

4. Read the logs to get the initial admin credentials (created automatically on first start):

```bash
docker compose logs server | grep "admin user created"
```

5. Open `http://<your-host>:8000/<admin_prefix>/` to access the admin panel and log in.

> To enable the DNS server, set `dns_server.listen` (e.g. `0.0.0.0:53`) in `docker/config.toml` and expose the corresponding UDP port in `docker-compose.yml`.

> If you prefer to build the image from source instead of pulling, run `docker compose build` (or `docker compose up -d --build`); the build logic lives in `docker/Dockerfile`.

## Configuration

The config file is in TOML format; see `config_example.toml`. Key fields:

```toml
db_url = "postgres://postgres:postgres@database/postgres"  # PostgreSQL connection string
storage_path = "/tmp/"                                     # root directory for file storage

[http_server]
listen = "0.0.0.0:8000"   # HTTP listen address; empty disables the HTTP server
openapi = true            # enable OpenAPI / Swagger docs
jwt_secret = "TEST_VALUE" # JWT key; empty means random
jwt_expire_time = 259200  # JWT lifetime in seconds, default 3 days
real_addr_header = ""     # header carrying the real client address behind a proxy; value must be addr:port (e.g. set X-Real-Addr in nginx via proxy_set_header X-Real-Addr "$remote_addr:$remote_port"; then use X-Real-Addr)
admin_prefix = "/super_admin/"  # admin panel prefix, must not be /
max_body_size = 3145728   # max request body size in bytes, default 3MB

[dns_server]
listen = ""               # DNS listen address; empty disables the DNS server

[script.cache]
max_entries = 1024        # max number of cache entries
max_entry_size = 65535    # max bytes per entry
max_ttl = 3600            # max cache TTL in seconds

[script.http]
allow_private_network = false # whether scripts may access private/loopback/link-local destinations
timeout = 16000               # maximum time per outbound request in milliseconds
max_response_size = 8388608   # maximum outbound response body size in bytes
max_redirects = 8             # maximum redirects to follow; 0 disables redirects

[ip2region]
ipv4_db = "docker/ip2region_v4.xdb"  # path to the IPv4 geolocation database
ipv6_db = "docker/ip2region_v6.xdb"  # path to the IPv6 geolocation database
```

Run with:

```bash
xss-receiver-rs <config_file>
```

## File Format Conventions

A route's handler points to a file in storage. The platform defines a set of extensions for different purposes, and the admin panel editor uses them to provide syntax highlighting, type hints, and schema validation:

| Extension | Purpose                                           | Editor support                                                                 |
| --------- | ------------------------------------------------- | ------------------------------------------------------------------------------ |
| `.hjs`    | HTTP `SCRIPT` handler or HTTP-specific ESM module | JS highlighting + HTTP script-engine type hints (`request` / `response`, etc.) |
| `.djs`    | DNS `SCRIPT` handler or DNS-specific ESM module   | JS highlighting + DNS script-engine type hints (`request` / `response`)        |
| `.djson`  | static answer for a DNS `STATIC` handler (JSON)   | JSON highlighting + DNS answer schema validation                               |

A `.djson` static answer file has the following structure:

```json
{
  "rcode": "NOERROR",
  "ttl": 60,
  "answers": [{ "type": "A", "value": "1.2.3.4", "ttl": 60 }]
}
```

- `rcode`: `NOERROR` / `NXDOMAIN` / `SERVFAIL` / `REFUSED` / `FORMERR` / `NOTIMP`, default `NOERROR`.
- `ttl`: default 60.
- `answers[].type`: `A` / `AAAA` / `CNAME` / `TXT`.

> These extensions are editing conventions. The entry route, not an imported file's extension, determines which HTTP / DNS globals a module receives. The admin editor provides hints for the current file only and does not analyze exports across files. An HTTP `STATIC` handler can point to any file type and return it verbatim.

## Script Engine API

`SCRIPT` routes execute the corresponding JavaScript file as an **ES module** when a request arrives. HTTP and DNS scripts support top-level `await`, static `import`, and dynamic `import()`. Use `export default value` in the entry module to write structured data to the request log's `extra_info`; omitting it stores `null`. The route's `timeout` is the overall execution limit, including asynchronous work.

The `request`, `response`, `storage`, `cache`, `http`, and global helper functions are available to scripts; `request` / `response` differ in shape per scenario.

### Module loading

Scripts can import other ESM files from user storage:

```js
import { normalize } from './lib/normalize.hjs'
const shared = await import('shared/utils.js')
```

- `./` and `../` resolve relative to the importing module. Specifiers without either prefix resolve from the user-storage root. Normalized paths may not escape that root.
- Specifiers must contain the exact filename; the loader does not append `.js` or resolve `index.js` automatically.
- Extensions are unrestricted and every dependency is parsed as UTF-8 ESM JavaScript. CommonJS `require`, JSON modules, npm / Node modules, and URL imports are unsupported.
- Dependencies share the entry module's context. An HTTP route gives every dependency HTTP `request` / `response` globals, while a DNS route gives every dependency the DNS versions, regardless of file extension.
- A resolved path is loaded and evaluated once per request. Each new request creates a fresh context and rereads the entry and its dependencies. Only the entry module's `export default` is written to the request log.

### `request` (HTTP)

| Property / Method    | Description                                                       |
| -------------------- | ----------------------------------------------------------------- |
| `request.method`     | request method                                                    |
| `request.path`       | request path                                                      |
| `request.clientAddr` | client address                                                    |
| `request.body`       | raw request body (`Uint8Array`)                                   |
| `request.headers`    | request headers, supports `headers.get(key)`                      |
| `request.query`      | query parameters, supports `query.get(key)`                       |
| `request.json`       | parsed JSON body                                                  |
| `request.forms`      | form fields, supports `forms.get(key)`                            |
| `request.files`      | uploaded files; `files.get(name)` returns `{ filename, content }` |

### `response` (HTTP)

| Method                            | Description                                                                          |
| --------------------------------- | ------------------------------------------------------------------------------------ |
| `response.send(data)`             | write the response body (string or `Uint8Array`); mutually exclusive with `sendFile` |
| `response.sendFile(path)`         | use a file from storage as the response body; callable only once                     |
| `response.sendStatus(code)`       | set the status code                                                                  |
| `response.sendHeader(key, value)` | set a response header; `value` may be a string or array of strings                   |

### `request` (DNS)

| Property             | Description                    |
| -------------------- | ------------------------------ |
| `request.name`       | queried domain name            |
| `request.type`       | query type (e.g. `A` / `AAAA`) |
| `request.class`      | query class (e.g. `IN`)        |
| `request.clientAddr` | client address                 |

### `response` (DNS)

| Method                               | Description                                                                 |
| ------------------------------------ | --------------------------------------------------------------------------- |
| `response.answer(type, value, ttl?)` | append an answer record; `type` supports `A` / `AAAA` / `CNAME` / `TXT`     |
| `response.rcode(code)`               | set the response code, e.g. `NOERROR` / `NXDOMAIN` / `SERVFAIL` / `REFUSED` |

### `storage` (shared)

`list(path)`, `listAll()`, `mkdir(path)`, `read(path)`, `write(path, content)`, `append(path, content)`, `remove(path)`, `rename(src, dst)`, `exists(path)`.

### `cache` (shared)

`cache.set(key, value, ttl?)`, `cache.get(key)`, `cache.delete(key)`, `cache.incr(key, delta?)`.

### `http` (shared)

`http` is the server-side outbound HTTP client. It provides `request`, `get`, `post`, `put`, `patch`, `delete`, and `head`, all returning a Promise:

```js
const upstream = await http.post('https://example.com/api', {
  headers: { 'content-type': 'application/json' },
  body: JSON.stringify({ source: request.clientAddr }),
  timeout: 8000,
})

const data = upstream.json()
export default { status: upstream.statusCode, data }
```

Request options are `method`, `headers`, `body` (a string or `Uint8Array`), `timeout`, `maxResponseSize`, `maxRedirects`, and `tlsVerify`. Responses expose `statusCode`, final `url`, multi-value `headers`, `body`, and repeatable `text()` / `json()` methods. HTTP 4xx/5xx responses resolve normally; network errors, timeouts, invalid JSON, and oversized bodies throw. Per-request limits may only tighten the server configuration. `tlsVerify` defaults to `true` and should only be disabled when necessary.

Private, loopback, link-local, CGNAT, cloud-metadata, and other non-public destinations are blocked by default, with DNS results and every redirect checked again. Set `script.http.allow_private_network = true` explicitly to permit them. System proxies are not used.

### Global helper functions (shared)

`base64Encode`, `base64Decode`, `urlEncode`, `urlDecode`.

## AI Skill (skills)

`skills/xss-receiver/` provides an Agent Skill for AI coding assistants, so an AI can use this platform's capabilities **even without access to this repository's source**:

- Write script-engine handlers and modules: `.hjs` (HTTP) / `.djs` (DNS) scripts, reusable ESM modules, and `.djson` static answers.
- Operate the platform via the admin HTTP API: upload scripts, create/manage routes, and fetch received request logs.

Files:

- `SKILL.md`: entry point, with a capability overview and the "upload script → create route → fetch latest logs" end-to-end workflow.
- `script-engine.md`: script-engine API (`request` / `response` / `storage` / `cache` / `http` and helpers) with examples.
- `admin-api.md`: admin API (auth / files / routes / logs) with an end-to-end curl example.

Usage: point your AI assistant at `skills/xss-receiver/SKILL.md`. Because the base path (host, `admin_prefix`) is deployment-specific, the skill instructs the AI to ask a human for it before making API calls.

## Local Development

### Prerequisites

- Rust (nightly, 2024 edition)
- Node.js + [pnpm](https://pnpm.io/)
- PostgreSQL
- `libpq` development library (for diesel)

### Build the frontend

```bash
cd frontend
pnpm install
pnpm build   # output goes to frontend/dist and is embedded via rust-embed
```

### Build and run the backend

```bash
cp config_example.toml config.toml   # edit as needed
cargo run --release -- config.toml
```

## Project Structure

```
.
├── src/                # Rust backend source
│   ├── controllers/    # HTTP endpoints and request entry points
│   ├── dispatcher/     # HTTP / DNS routing and the script engine
│   ├── db/             # diesel models and queries
│   ├── storage/        # file storage
│   └── utils/          # DNS server, ip2region, JWT, and other helpers
├── frontend/           # Vue 3 + Vuetify admin panel
├── skills/             # Agent Skill for AI coding assistants
├── docker/             # Docker / Compose deployment files
├── migrations/         # database migrations
└── thirdparty/         # customized third-party deps (http / httparse)
```

## Disclaimer

This project is intended only for authorized security testing, research, and learning. Do not use it for any illegal purpose. Users are solely responsible for any consequences arising from improper use.
