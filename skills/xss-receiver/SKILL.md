---
name: xss-receiver
description: >-
  Operate the xss-receiver-rs platform: write its programmable HTTP/DNS handler
  scripts (.hjs / .djs / .djson) and drive its admin HTTP API to upload script
  files, create routes, and fetch received HTTP/DNS request logs. Useful for CTF
  and security testing (XSS data collection, SSRF/OOB probing, DNS Log). Use when
  working with an xss-receiver-rs instance, writing its script-engine handlers
  (request/response/storage/cache), or automating it via curl/HTTP. Trigger
  terms: xss-receiver, .hjs, .djs, .djson, script engine, HTTP/DNS route,
  admin API, upload script, create route, fetch logs, CTF, security testing.
---

# xss-receiver

`xss-receiver-rs` is an XSS / information-receiver platform with programmable HTTP and
DNS servers. It captures, logs, and lets you customize responses to external requests
(useful for XSS data collection, SSRF/OOB probing, and DNS Log scenarios).

This skill is self-contained: it covers everything needed to use the platform without
reading its source code. It helps with two kinds of tasks:

1. **Write handler scripts** — use the built-in JavaScript engine to handle HTTP / DNS
   requests, customize responses, persist captured data, build a DNS Log, etc.
2. **Operate the platform** — use the admin HTTP API to upload script files, create and
   manage routes, and read received request logs.

## Platform capabilities

- **Programmable HTTP server**: routing rules (exact `PLAIN` / regex `REGEX`) map a
  request to a handler:
  - `STATIC`: return a stored file as-is.
  - `SCRIPT`: run a stored JavaScript file to build the response dynamically.
  - `NONE`: only log the request, return a default response (404).
- **Programmable DNS server**: same rule-based routing; answer statically or build
  `A` / `AAAA` / `CNAME` / `TXT` records from a script (good for DNS Log).
- **File storage**: a route's handler points to a file in storage; scripts can read/write
  storage via the `storage` object.
- **Script cache**: a lightweight shared key/value cache (`cache`) usable from both HTTP
  and DNS scripts.
- **Full request logging**: source address, headers, query, body, uploaded files, plus
  IP geolocation.
- **Admin API + web admin UI**: JWT-authenticated; everything is callable under
  `<admin_prefix>/api/...`.

## File extension conventions

A handler points to one file in storage. These extensions are conventions (the admin
editor uses them for highlighting / type hints); the server reads the file the handler
config points to:

| Extension | Purpose                            |
| --------- | ---------------------------------- |
| `.hjs`    | HTTP `SCRIPT` handler (JavaScript) |
| `.djs`    | DNS `SCRIPT` handler (JavaScript)  |
| `.djson`  | DNS `STATIC` answer (JSON)         |

A `STATIC` HTTP handler can point to any file type and returns it verbatim.

## End-to-end workflow: add script -> add route -> read latest logs

```mermaid
flowchart LR
  login["POST /user/login\n(get cookie / token)"] --> upload["POST /file/upload\n(upload .hjs script)"]
  upload --> route["POST /http_route\n(handler points to script)"]
  route --> logs["GET /http_log?page=1\n(newest request first)"]
```

1. **Log in**: `POST <prefix>/api/user/login` to obtain the `authorization` cookie / token.
2. **Upload a script**: `POST <prefix>/api/file/upload` (multipart; `path` = storage path
   such as `hooks/collect.hjs`, `file` = script content).
3. **Create a route**: `POST <prefix>/api/http_route` with `handler_kind=SCRIPT` and
   `handler` set to the uploaded path. Creating a route compiles it immediately; an
   invalid handler path fails the request.
4. **Read latest logs**: `GET <prefix>/api/http_log?page=1&page_size=20`; logs are ordered
   newest-first.

## Key things to remember

- **Serving a static file? Use a `STATIC` handler, not a `SCRIPT`.** If you only need to
  return a stored file as-is (HTML page, JS payload, image, etc.), point a `STATIC` handler
  at that file — don't write a `SCRIPT` that just calls `response.sendFile`. Reach for
  `SCRIPT` only when the response must be built dynamically.
- **Always ask the human for the base path first.** The full base path is
  `<host>:<port><admin_prefix>/api` (e.g. `https://example.com:8000/super_admin/api`).
  Both the host/port and `admin_prefix` are deployment-specific, secret-ish, and **not
  guessable** — they differ for every instance and the examples in this skill use
  placeholder values. Before making any API call, proactively ask the human for the
  instance's host/port, `admin_prefix`, and credentials. Do not assume `/super_admin/`
  or any value taken from an example.
- **API base path** is `<admin_prefix>/api` (e.g. `/super_admin/api`), not `/api`. The
  `admin_prefix` is set in the platform config and must not be `/`.
- **Auth**: login returns the token via `Set-Cookie: authorization=<jwt>` (the token is in
  the cookie, not the response body). Subsequent requests send that cookie, or set header
  `Authorization: <jwt>` (an optional `Bearer ` prefix is accepted).
- **Response envelope**: every endpoint returns HTTP 200 with body
  `{ "code": number, "msg": string|null, "payload": any }`. Success is `code == 200` —
  always check `code`, not the HTTP status.
- **Script `timeout` is in milliseconds** (route field); a timed-out script is aborted and
  an error is logged.
- **Script return value**: a script's last expression value is serialized to JSON and
  stored in that request log's `extra_info` field — handy for attaching structured data.
- **Field name gotcha**: HTTP route delete/update use `http_route_id`; DNS route delete/update
  use `route_id`.

## Reference docs

- Script-engine API (`request` / `response` / `storage` / `cache` / helpers) and script
  examples: see [script-engine.md](script-engine.md)
- Admin API (auth / files / routes / logs) and an end-to-end curl example: see
  [admin-api.md](admin-api.md)
