# Script engine API reference

A `SCRIPT` route runs a JavaScript file when a request arrives (`.hjs` = HTTP, `.djs` = DNS).
Scripts run in an embedded JavaScript engine; a fresh context is created per request.
`request` / `response` differ by scenario; `storage` / `cache` and the global helper
functions are shared by both HTTP and DNS scripts.

## Execution model

- `async` / `await` are supported (the script is evaluated asynchronously).
- The route's `timeout` (milliseconds) aborts the script when reached and logs an error.
- The script's **last expression value** is serialized to JSON and written to that request
  log's `extra_info` field — use it to attach structured info.
- A thrown exception is recorded in the log's `error_log` field.
- The engine is a standalone JS runtime: there is **no** Node.js, no `require`/`import`, no
  `fetch`, no filesystem access except through the `storage` object below.

## Shared: `storage`

Operates on the platform file store (path-checked; traversal outside the store is rejected).
Paths are relative to the storage root and use `/` separators.

```ts
type EntryKind = 'file' | 'directory'
interface Entry {
  readonly name: string
  readonly kind: EntryKind
  readonly size: number
  readonly modifiedTime: number
}

storage.list(path: string): Entry[]            // list a directory's direct children
storage.listAll(): string[]                    // recursively list all file paths
storage.mkdir(path: string): void              // create directory (recursive)
storage.read(path: string): Uint8Array
storage.write(path: string, content: string | Uint8Array): void   // create/overwrite
storage.append(path: string, content: string | Uint8Array): void
storage.remove(path: string): void
storage.rename(src: string, dst: string): void
storage.exists(path: string): boolean
```

## Shared: `cache`

Process-wide shared key/value cache (HTTP and DNS scripts share the same cache).
Value types: `string | boolean | number | Uint8Array`.

```ts
type CacheValue = string | boolean | number | Uint8Array

cache.set(key: string, value: CacheValue, ttl?: number): void  // ttl in seconds
cache.get(key: string): CacheValue | undefined
cache.delete(key: string): boolean                             // true if key existed
cache.incr(key: string, delta?: number, ttl?: number): number  // atomic; returns new value, delta defaults to 1
```

Constraints:

- `ttl` must be > 0 and not greater than the server's configured max TTL (default 3600s);
  if omitted, the max TTL is used.
- `key length + value size` must not exceed the server's configured max entry size.
- `incr` errors if the existing cached value is not a number.

## Shared: global helper functions

```ts
base64Encode(data: string | Uint8Array): string
base64Decode(data: string): Uint8Array
urlEncode(data: string): string
urlDecode(data: string): string
```

## HTTP scripts (`.hjs`)

### `request`

```ts
// MultiMap: one key may map to multiple values.
//   request.headers["x-foo"]      -> string[]   (all values)
//   request.headers.get("x-foo")  -> string | undefined  (first value)
interface MultiMap {
  get(key: string): string | undefined
  [key: string]: string[] | ((key: string) => string | undefined)
}

interface UploadFile {
  readonly filename: string
  readonly content: Uint8Array
}
interface UploadFilesMap {
  get(name: string): UploadFile | undefined
  [name: string]: UploadFile[] | ((name: string) => UploadFile | undefined)
}

request.method: string        // request method
request.path: string          // request path
request.clientAddr: string    // client address (ip:port)
request.body: Uint8Array      // raw request body
request.headers: MultiMap     // request headers; headers.get(key) for first value
request.query: MultiMap       // query params; query.get(key)
request.json: any             // parsed object when the body is JSON (otherwise an empty object)
request.forms: MultiMap       // form fields (multipart / urlencoded); forms.get(key)
request.files: UploadFilesMap // uploaded files; files.get(name) -> { filename, content }
```

### `response`

```ts
response.send(data: string | Uint8Array): void  // write response body (can be called repeatedly to append); mutually exclusive with sendFile
response.sendFile(path: string): void           // stream a stored file as the body; call at most once; mutually exclusive with send
response.sendStatus(code: number): void         // set status code (default 200)
response.sendHeader(key: string, value: string | string[]): void
```

## DNS scripts (`.djs`)

### `request`

```ts
request.name: string        // queried domain name
request.type: string        // query type, e.g. A / AAAA / CNAME / TXT
request.class: string       // query class, e.g. IN
request.clientAddr: string  // client address (ip:port)
```

### `response`

```ts
type DnsAnswerType = 'A' | 'AAAA' | 'CNAME' | 'TXT'
type DnsResponseCode = 'NOERROR' | 'NXDOMAIN' | 'SERVFAIL' | 'REFUSED' | 'FORMERR' | 'NOTIMP'

response.answer(type: DnsAnswerType, value: string, ttl?: number): void  // append one answer record
response.rcode(code: DnsResponseCode): void                              // set the response code
```

- The engine filters answers by the actual query type (a query for `A` returns only `A`
  records; `ANY` returns all appended records).

## DNS static answer (`.djson`)

A DNS `STATIC` handler points to a JSON file:

```json
{
  "rcode": "NOERROR",
  "ttl": 60,
  "answers": [{ "type": "A", "value": "1.2.3.4", "ttl": 60 }]
}
```

- `rcode`: `NOERROR` / `NXDOMAIN` / `SERVFAIL` / `REFUSED` / `FORMERR` / `NOTIMP`, default `NOERROR`.
- `ttl`: default 60.
- `answers[].type`: `A` / `AAAA` / `CNAME` / `TXT`; `answers[].ttl` is optional.

## Examples

### 1. Collect XSS data and persist it (`.hjs`)

```js
// Append received data to a per-day file in storage, then return a 1x1 gif.
const data = {
  time: new Date().toISOString(),
  from: request.clientAddr,
  ua: request.headers.get("user-agent"),
  cookie: request.query.get("c") || request.forms.get("c") || "",
};

const day = data.time.slice(0, 10);
storage.append(`loot/${day}.jsonl`, JSON.stringify(data) + "\n");

response.sendHeader("Content-Type", "image/gif");
response.send(base64Decode("R0lGODlhAQABAAAAACwAAAAAAQABAAA="));

// last expression -> stored in this log's extra_info
({ collected: true, who: data.from });
```

### 2. Dynamic DNS Log answer (`.djs`)

```js
// Count queries per name, log the query, always answer 1.2.3.4.
const hits = cache.incr(`dns:${request.name}`, 1, 3600);
storage.append(
  "dns/queries.log",
  `${request.clientAddr} ${request.type} ${request.name}\n`,
);

response.answer("A", "1.2.3.4", 60);
({ name: request.name, hits });
```

### 3. Simple rate-limit counter with cache (`.hjs`)

```js
const key = `rl:${request.clientAddr}`;
const count = cache.incr(key, 1, 60); // 60s window

if (count > 100) {
  response.sendStatus(429);
  response.send("rate limited");
} else {
  response.send("ok");
}
({ count });
```
