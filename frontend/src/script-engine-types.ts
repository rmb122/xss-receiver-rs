/** TypeScript declarations for the HTTP (.hjs) and DNS (.djs) script runtimes. */
const commonScriptEngineTypes = `
/// <reference no-default-lib="true"/>
/// <reference lib="es2024" />
/// <reference lib="esnext.array" />
/// <reference lib="esnext.collection" />
/// <reference lib="esnext.error" />
/// <reference lib="esnext.disposable" />
/// <reference lib="esnext.iterator" />
/// <reference lib="esnext.float16" />
/// <reference lib="esnext.promise" />
/// <reference lib="esnext.sharedmemory" />

/** Additional globals provided by QuickJS-NG. */
declare const InternalError: ErrorConstructor;

declare class DOMException extends Error {
  constructor(message?: string, name?: string);
  readonly code: number;
}

declare const performance: {
  readonly timeOrigin: number;
  now(): number;
};

declare function queueMicrotask(callback: () => void): void;
declare function atob(data: string): string;
declare function btoa(data: string): string;
declare function escape(value: string): string;
declare function unescape(value: string): string;

type EntryKind = 'file' | 'directory';

interface Entry {
  readonly name: string;
  readonly kind: EntryKind;
  readonly size: number;
  readonly modifiedTime: number;
}

interface Storage {
  list(path: string): Entry[];
  listAll(): string[];
  mkdir(path: string): void;
  read(path: string): Uint8Array;
  write(path: string, content: string | Uint8Array): void;
  append(path: string, content: string | Uint8Array): void;
  remove(path: string): void;
  rename(src: string, dst: string): void;
  exists(path: string): boolean;
}

type CacheValue = string | boolean | number | Uint8Array;

interface Cache {
  set(key: string, value: CacheValue, ttl?: number): void;
  get(key: string): CacheValue | undefined;
  delete(key: string): boolean;
  incr(key: string, delta?: number, ttl?: number): number;
}

interface HttpRequestOptions {
  method?: string;
  headers?: Record<string, string | string[]>;
  body?: string | Uint8Array;
  /** Milliseconds; may only reduce the server limit. */
  timeout?: number;
  /** Bytes; may only reduce the server limit. */
  maxResponseSize?: number;
  /** May only reduce the server limit; zero disables redirects. */
  maxRedirects?: number;
  /** Defaults to true. */
  tlsVerify?: boolean;
}

type HttpMethodOptions = Omit<HttpRequestOptions, 'method'>;

interface HttpClientResponse {
  readonly statusCode: number;
  readonly url: string;
  readonly headers: Readonly<Record<string, readonly string[]>>;
  readonly body: Uint8Array;
  text(): string;
  json(): any;
}

interface HttpClient {
  request(url: string, options?: HttpRequestOptions): Promise<HttpClientResponse>;
  get(url: string, options?: HttpMethodOptions): Promise<HttpClientResponse>;
  post(url: string, options?: HttpMethodOptions): Promise<HttpClientResponse>;
  put(url: string, options?: HttpMethodOptions): Promise<HttpClientResponse>;
  patch(url: string, options?: HttpMethodOptions): Promise<HttpClientResponse>;
  delete(url: string, options?: HttpMethodOptions): Promise<HttpClientResponse>;
  head(url: string, options?: HttpMethodOptions): Promise<HttpClientResponse>;
}

declare const storage: Storage;
declare const cache: Cache;
declare const http: HttpClient;

declare function base64Encode(data: string | Uint8Array): string;
declare function base64Decode(data: string): Uint8Array;
declare function urlEncode(data: string): string;
declare function urlDecode(data: string): string;
`

export const httpScriptEngineTypes = `
${commonScriptEngineTypes}

interface MultiMap {
  readonly get: (key: string) => string | undefined;
  readonly [key: string]: readonly string[] | ((key: string) => string | undefined);
}

type ReadonlyJsonValue = null | boolean | number | string
  | readonly ReadonlyJsonValue[]
  | { readonly [key: string]: ReadonlyJsonValue };

interface UploadFile {
  readonly filename: string;
  /** The content property is fixed; the Uint8Array bytes remain writable. */
  readonly content: Uint8Array;
}

interface UploadFilesMap {
  readonly get: (name: string) => UploadFile | undefined;
  readonly [name: string]: readonly UploadFile[] | ((name: string) => UploadFile | undefined);
}

interface HttpRequest {
  readonly method: string;
  readonly path: string;
  readonly clientAddr: string;
  /** The body property is fixed; the Uint8Array bytes remain writable. */
  readonly body: Uint8Array;
  /** Read-only header map and value arrays. ASCII case-insensitive lookup via get(key), bracket, or dot access. The exact property "get" is reserved for the method; use get('get') for that header. */
  readonly headers: MultiMap;
  readonly query: MultiMap;
  readonly json: ReadonlyJsonValue;
  readonly forms: MultiMap;
  readonly files: UploadFilesMap;
}

interface HttpResponse {
  send(data: string | Uint8Array): void;
  sendFile(path: string): void;
  sendStatus(code: number): void;
  sendHeader(key: string, value: string | string[]): void;
}

declare const request: HttpRequest;
declare const response: HttpResponse;
`

export const dnsScriptEngineTypes = `
${commonScriptEngineTypes}

type DnsAnswerType = 'A' | 'AAAA' | 'CNAME' | 'TXT';
type DnsResponseCode = 'NOERROR' | 'NXDOMAIN' | 'SERVFAIL' | 'REFUSED' | 'FORMERR' | 'NOTIMP';

interface DnsRequest {
  readonly name: string;
  readonly type: string;
  readonly class: string;
  readonly clientAddr: string;
}

interface DnsResponse {
  answer(type: DnsAnswerType, value: string, ttl?: number): void;
  rcode(code: DnsResponseCode): void;
}

declare const request: DnsRequest;
declare const response: DnsResponse;
`
