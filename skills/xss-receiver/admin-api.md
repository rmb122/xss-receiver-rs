# Admin API reference

All management features are exposed over the admin HTTP API (the web admin UI calls the
same endpoints). This document is self-contained; you do not need access to the platform
source to use it.

## Connection configuration

First read `XSS_RECEIVER_CONFIG` from the environment. It contains JSON:

```json
{
  "server_base": "https://example.com:8000",
  "admin_prefix": "/super_admin/",
  "token": "xrt_...",
  "dns_log_suffix": "dns.example.com"
}
```

`server_base` is the HTTP origin (scheme, host, optional port). `admin_prefix` is the
panel path starting with `/`, without `/api` or a frontend hash route, and must not be
`/`. Join them with `/api`, normalizing the joining slashes, to obtain `$BASE`.
`token` becomes `$TOKEN` and is sent as a Bearer token. Do not print either the token
or the complete configuration. See the end-to-end example for loading these fields in Bash.

Use `dns_log_suffix` for DNS Log domains, for example `<unique-label>.dns.example.com`.
It is required only for DNS tasks. It describes an existing DNS deployment; it does not
configure delegation or create routes automatically.

When configuration is missing, use details already supplied by the human and ask only
for missing fields needed by the task. Invalid JSON is a configuration error to resolve
before calling the API. All example addresses are placeholders, not deployment defaults.

## Conventions

- **Base path**: `<admin_prefix>/api`. `admin_prefix` is a platform config value
  (e.g. `/super_admin/`, never `/`). Example: the login endpoint is
  `http://host:8000/super_admin/api/user/login`. Below, `$BASE` denotes
  `<server_base><admin_prefix>/api` (from the configuration above).
- **Response envelope**: every endpoint returns HTTP 200 with body
  `{ "code": number, "msg": string|null, "payload": any }`.
  - Success: `code == 200`, data in `payload`.
  - Failure: `code == 500` (business error, reason in `msg`) or `code == 400` (auth failure).
  - **Decide success by `code`, not by the HTTP status.**
- **Pagination**: log list endpoints take `?page=&page_size=` (page starts at 1; page_size 1-500,
  default 20) and return `{ data, total, page, page_size }`.

## Authentication

With a configured token, send `Authorization: Bearer $TOKEN` directly. No login or
password is needed. A bare token is also accepted. The Authorization header takes
precedence over the `authorization` cookie, even if the header contains an invalid token.

```bash
curl -s -H "Authorization: Bearer $TOKEN" "$BASE/user/current"
```

Alternatively, when the human supplies username/password credentials:

1. `POST $BASE/user/login` with body `{"username":"...","password":"..."}`.
2. On success, `Set-Cookie: authorization=<jwt>; Path=<admin_prefix>; HttpOnly` contains
   the JWT; it is not returned in `payload`.
3. Send that cookie or use its JWT value as `$TOKEN` in the examples below.

```bash
# Optional password login: save and reuse the cookie.
curl -s -c cookies.txt -X POST "$BASE/user/login" \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"<password>"}'
curl -s -b cookies.txt "$BASE/user/current"
```

## File endpoints (`/file`)

| Method | Path             | body / params             | Notes                                 |
| ------ | ---------------- | ------------------------- | ------------------------------------- |
| POST   | `/file/upload`   | multipart: `path`, `file` | Upload/write a file (add a script)    |
| POST   | `/file/list`     | `{ path }`                | List a directory -> `{ entries }`     |
| POST   | `/file/list_all` | `{}`                      | Recursively list files -> `{ files }` |
| GET    | `/file/stat`     | `?path=`                  | Single entry info                     |
| POST   | `/file/mkdir`    | `{ path }`                | Create directory                      |
| POST   | `/file/remove`   | `{ path }`                | Delete                                |
| POST   | `/file/rename`   | `{ src, dst }`            | Rename / move                         |
| GET    | `/file/download` | `?path=`                  | Download a file                       |
| POST   | `/file/part`     | multipart: `file`         | Chunked upload -> `{ chunk_id }`      |
| POST   | `/file/merge`    | `{ chunk_ids, path }`     | Merge chunks into a file              |

```bash
# Create the parent directory before uploading into it.
curl -s -H "Authorization: Bearer $TOKEN" -X POST "$BASE/file/mkdir" \
  -H 'Content-Type: application/json' -d '{"path":"hooks"}'

# Upload a .hjs script to storage path hooks/collect.hjs
curl -s -H "Authorization: Bearer $TOKEN" -X POST "$BASE/file/upload" \
  -F "path=hooks/collect.hjs" \
  -F "file=@collect.hjs"
```

## HTTP route endpoints (`/http_route`)

| Method | Path          | body                           | Notes  |
| ------ | ------------- | ------------------------------ | ------ |
| GET    | `/http_route` | -                              | List   |
| POST   | `/http_route` | route object (below)           | Create |
| PATCH  | `/http_route` | route object + `http_route_id` | Update |
| DELETE | `/http_route` | `{ http_route_id }`            | Delete |

Create/update body fields:

```jsonc
{
  "pattern_kind": "PLAIN", // PLAIN (exact) | REGEX (regular expression)
  "pattern": "/5cb4842a7587", // PLAIN = exact full path match; REGEX = a regex
  "priority": 0, // when multiple routes match, the highest priority wins
  "timeout": 1000, // script timeout in milliseconds
  "catalog": "default", // grouping label in the admin UI (any string)
  "handler_kind": "SCRIPT", // STATIC | SCRIPT | NONE
  "handler": "hooks/collect.hjs", // file path in storage (relative to root; no traversal)
  "write_log": true, // whether to log matching requests
  "comment": "collect xss",
}
```

- Create/update **install the route set immediately**. Upload the handler file first;
  script source is loaded when the route handles a request.
- **For a static file, set `handler_kind` to `STATIC`** and point `handler` at the file; it
  is returned as-is (any file type). Only use `SCRIPT` when the response is built dynamically.
- **Delete/update use the field `http_route_id`** (different from DNS routes).

```bash
RECEIVER_PATH="/$(python3 -c 'import secrets; print(secrets.token_hex(6))')"
curl -s -H "Authorization: Bearer $TOKEN" -X POST "$BASE/http_route" \
  -H 'Content-Type: application/json' --data-binary @- <<JSON
  {
    "pattern_kind":"PLAIN","pattern":"$RECEIVER_PATH","priority":0,"timeout":1000,
    "catalog":"default","handler_kind":"SCRIPT","handler":"hooks/collect.hjs",
    "write_log":true,"comment":"collect xss"
  }
JSON
```

## DNS route endpoints (`/dns_route`)

| Method | Path         | body                      | Notes  |
| ------ | ------------ | ------------------------- | ------ |
| GET    | `/dns_route` | -                         | List   |
| POST   | `/dns_route` | route object              | Create |
| PATCH  | `/dns_route` | route object + `route_id` | Update |
| DELETE | `/dns_route` | `{ route_id }`            | Delete |

- Create/update body fields are the same as HTTP routes
  (`pattern_kind / pattern / priority / timeout / catalog / handler_kind / handler / write_log / comment`).
- `pattern` is a domain name (PLAIN is normalized: trailing `.` removed, lowercased).
  `handler` points to a `.djs` file (SCRIPT) or a `.djson` file (STATIC).
- **Delete/update use the field `route_id`** (note: HTTP routes use `http_route_id` — easy to mix up).

## Log endpoints

| Method | Path                                           | Notes                                    |
| ------ | ---------------------------------------------- | ---------------------------------------- |
| GET    | `/http_log?page=&page_size=&filter=&timezone=` | Filtered HTTP request logs, newest first |
| GET    | `/http_log/{id}/raw_body`                      | Raw request body of one log (binary)     |
| GET    | `/dns_log?page=&page_size=&filter=&timezone=`  | Filtered DNS query logs, newest first    |
| GET    | `/system_log?page=&page_size=`                 | System logs (logins, etc.)               |

Main HTTP log fields: `id, client_ip, client_port, location, method, path, raw_query,
parsed_query, header, parsed_body_type, parsed_body, file, extra_info, error_log, create_time`.

- `extra_info`: the script module's `export default` value (JSON), or `null` when omitted.
- `error_log`: the script error message (null if none).

```bash
# Fetch the latest 20 HTTP requests (newest first)
curl -s -H "Authorization: Bearer $TOKEN" "$BASE/http_log?page=1&page_size=20"
```

### Filter expressions

`filter` is optional; omitted or whitespace-only filters match all logs. `total` counts all matching rows,
and `data` contains the requested page in descending ID order. `page` defaults to 1 and `page_size` to 20 (maximum 500).

| Logs      | Filterable fields                                                        |
| --------- | ------------------------------------------------------------------------ |
| Both      | `id`, `client_ip`, `client_port`, `location`, `create_time`, `error_log` |
| HTTP only | `method`, `path`, `raw_query`, `raw_body`, `parsed_body_type`            |
| DNS only  | `query_name`, `query_type`, `query_class`                                |

- `id` and `client_port` take 32-bit integers, without quotes. Integers and timestamps support `=`, `!=`, `<`, `<=`, `>`, `>=`.
- Text fields take single- or double-quoted strings and support case-sensitive `=`, `!=`, and `contains(field, "text")`. Strings support JSON escapes (including `\uXXXX`) and `\'`; for example, `'\u4e2d\u6587'` is equivalent to `"中文"`. The substring is literal, including `%`, `_`, and escaped backslashes.
- `parsed_body_type` supports only `=` and `!=` with `"NONE"`, `"FAILED"`, `"FORM"`, or `"JSON"`.
- `raw_body` supports `=`, `!=`, and `contains` with single- or double-quoted strings. The decoded string is encoded as UTF-8 and bound as `bytea`: equality compares the entire stored body, and `contains` searches for a literal byte substring. For example, `raw_body = "asd"`, `raw_body != ""`, or `contains(raw_body, "\u0000")`. Invalid UTF-8 bytes in the body are allowed; the body is not decoded or parsed. Query encoding does not depend on Content-Type. Ordinary text fields reject NUL characters.
- `error_log = null` and `error_log != null` check for absent/present errors. Other text comparisons, including their negations, do not match null values.
- Combine conditions with `&&`, `||`, `!`, and parentheses. Logical precedence is `!`, then `&&`, then `||`.
- `create_time` accepts RFC3339, `YYYY-MM-DD`, or `YYYY-MM-DD HH:mm:ss`. `T` can replace the space and fractional seconds are accepted. Dates mean midnight, not a whole-day range.
- Pass `timezone` as an IANA name, e.g. `Asia/Shanghai`, whenever a timestamp has no explicit offset. The UI sends the browser's timezone. Explicit timestamp offsets take precedence; ambiguous or nonexistent local times require an explicit offset.
- Unknown fields, unsupported operations, invalid types/times, and malformed expressions return the existing error envelope: HTTP 200 with `code: 500`, a descriptive `msg`, and `payload: null`. Filter errors include a 1-based character position. This is not an authentication error.
- Limits: 256 conditions and logical operators, and 64 levels of parentheses/negation. Header filtering and nested Body, Query, and extra_info lookups are unsupported.

Use `--data-urlencode` so quotes, `&&`, and timezone offsets are encoded correctly:

```bash
curl -s -G -H "Authorization: Bearer $TOKEN" "$BASE/http_log" \
  --data-urlencode 'filter=client_ip = "192.0.2.1" && create_time < "2026-09-19 12:00:00"' \
  --data-urlencode 'timezone=Asia/Shanghai' \
  --data-urlencode 'page=1' --data-urlencode 'page_size=20'

curl -s -G -H "Authorization: Bearer $TOKEN" "$BASE/dns_log" \
  --data-urlencode 'filter=query_type = "A" && !contains(query_name, "example.com")'

curl -s -G -H "Authorization: Bearer $TOKEN" "$BASE/http_log" \
  --data-urlencode 'filter=contains(raw_body, "asd") && method = "POST"'
```

## End-to-end example: add script -> add route -> read latest logs

```bash
#!/usr/bin/env bash
set -euo pipefail

# 1) Read the supplied JSON configuration. Do not enable shell tracing.
read_config() {
  python3 -c 'import json, os, sys; print(json.loads(os.environ["XSS_RECEIVER_CONFIG"])[sys.argv[1]])' "$1"
}
SERVER_BASE="$(read_config server_base)"
ADMIN_PREFIX="$(read_config admin_prefix)"
TOKEN="$(read_config token)"
BASE="${SERVER_BASE%/}/${ADMIN_PREFIX#/}"
BASE="${BASE%/}/api"

# HTTP 200 can still contain an API error.
check_response() {
  python3 -c '
import json, sys
result = json.load(sys.stdin)
if result["code"] != 200:
    raise SystemExit(result.get("msg") or "API request failed")
print(json.dumps(result, ensure_ascii=False))'
}

# 2) Create the parent directory, write a script, and upload it.
curl -s -H "Authorization: Bearer $TOKEN" -X POST "$BASE/file/mkdir" \
  -H 'Content-Type: application/json' -d '{"path":"hooks"}' | check_response > /dev/null

cat > collect.hjs <<'JS'
storage.append('hits.jsonl', JSON.stringify({
  t: new Date().toISOString(), from: request.clientAddr, q: request.path
}) + '\n')
response.send('ok')
export default { ok: true }
JS
curl -s -H "Authorization: Bearer $TOKEN" -X POST "$BASE/file/upload" \
  -F "path=hooks/collect.hjs" -F "file=@collect.hjs" | check_response > /dev/null

# 3) Generate a random public path and point its route to the script.
RECEIVER_PATH="/$(python3 -c 'import secrets; print(secrets.token_hex(6))')"
curl -s -H "Authorization: Bearer $TOKEN" -X POST "$BASE/http_route" \
  -H 'Content-Type: application/json' --data-binary @- <<JSON | check_response > /dev/null
  {
    "pattern_kind":"PLAIN","pattern":"$RECEIVER_PATH","priority":0,"timeout":1000,
    "catalog":"default","handler_kind":"SCRIPT","handler":"hooks/collect.hjs",
    "write_log":true,"comment":"collect"
  }
JSON

# 4) Trigger it once, then read the latest logs
curl -s "${SERVER_BASE%/}${RECEIVER_PATH}" > /dev/null
curl -s -H "Authorization: Bearer $TOKEN" "$BASE/http_log?page=1&page_size=5" | check_response
```
