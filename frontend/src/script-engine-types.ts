/**
 * 后端 JS 脚本引擎注入对象的 TypeScript 类型声明。
 * .hjs 使用 HTTP handler 类型，.djs 使用 DNS handler 类型。
 */
const commonScriptEngineTypes = `
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

declare const storage: Storage;
declare const cache: Cache;

declare function base64Encode(data: string | Uint8Array): string;
declare function base64Decode(data: string): Uint8Array;
declare function urlEncode(data: string): string;
declare function urlDecode(data: string): string;
`

export const httpScriptEngineTypes = `
${commonScriptEngineTypes}

interface MultiMap {
  get(key: string): string | undefined;
  [key: string]: string[] | ((key: string) => string | undefined);
}

interface UploadFile {
  readonly filename: string;
  readonly content: Uint8Array;
}

interface UploadFilesMap {
  get(name: string): UploadFile | undefined;
  [name: string]: UploadFile[] | ((name: string) => UploadFile | undefined);
}

interface HttpRequest {
  readonly method: string;
  readonly path: string;
  readonly clientAddr: string;
  readonly body: Uint8Array;
  readonly headers: MultiMap;
  readonly query: MultiMap;
  readonly json: any;
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
