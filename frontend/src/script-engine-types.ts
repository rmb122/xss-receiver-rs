/**
 * 后端 JS 脚本引擎注入对象的 TypeScript 类型声明
 * 用于 Monaco Editor 的 IntelliSense 提示
 */
export const scriptEngineTypes = `
/**
 * MultiMap 类型：每个 key 对应一个 string 数组，
 * 可通过 get(key) 取首个值，也可直接 obj[key] 取整个数组。
 */
interface MultiMap {
  /** 获取指定 key 的第一个值，若不存在则返回 undefined */
  get(key: string): string | undefined;
  /** 通过索引签名访问某 key 下的所有值 */
  [key: string]: string[] | ((key: string) => string | undefined);
}

/**
 * 上传文件对象
 */
interface UploadFile {
  /** 上传时的原始文件名 */
  readonly filename: string;
  /** 文件二进制内容 */
  readonly content: Uint8Array;
}

/**
 * 上传文件 MultiMap：每个 field name 对应一个 UploadFile 数组
 */
interface UploadFilesMap {
  /** 获取指定 field name 的第一个文件对象 */
  get(name: string): UploadFile | undefined;
  /** 通过索引签名访问某 field name 下的所有文件 */
  [name: string]: UploadFile[] | ((name: string) => UploadFile | undefined);
}

/**
 * 目录项类型
 */
type EntryKind = 'file' | 'directory';

/**
 * 目录项信息（storage.list 返回）
 */
interface Entry {
  /** 文件或目录名（basename） */
  readonly name: string;
  /** 类型："file" 或 "directory" */
  readonly kind: EntryKind;
  /** 文件大小（字节），目录为 0 */
  readonly size: number;
  /** 最后修改时间（Unix 时间戳，秒） */
  readonly modifiedTime: number;
}

/**
 * HTTP 请求对象（只读）
 *
 * 包含当前 HTTP 请求的所有信息，包括方法、路径、请求头、
 * 查询参数、请求体及其解析结果。
 */
declare const request: {
  /** HTTP 请求方法，如 "GET"、"POST" */
  readonly method: string;
  /** 请求路径，如 "/api/data" */
  readonly path: string;
  /** 客户端地址，格式为 "ip:port" */
  readonly clientAddr: string;
  /** 原始请求体的二进制数据 */
  readonly body: Uint8Array;
  /** 请求头 MultiMap（key 格式为首字母大写，如 "Content-Type"） */
  readonly headers: MultiMap;
  /** URL 查询参数 MultiMap */
  readonly params: MultiMap;
  /**
   * JSON 解析后的请求体。
   * 当 Content-Type 为 JSON 类型时，包含解析后的 JSON 对象；
   * 否则为空对象 {}。
   */
  readonly json: any;
  /** 表单数据 MultiMap（application/x-www-form-urlencoded 或 multipart/form-data） */
  readonly forms: MultiMap;
  /** 上传文件 Map（multipart/form-data 中的文件字段） */
  readonly files: UploadFilesMap;
};

/**
 * HTTP 响应对象
 *
 * 用于构建 HTTP 响应。可多次调用 send() 追加响应体。
 * 默认状态码为 200。
 */
declare const response: {
  /**
   * 追加数据到响应体。可多次调用，数据会依次拼接。
   * 与 sendFile() 互斥：如果已调用过 sendFile()，再调用 send() 会抛出异常。
   * @param data 响应体数据，支持字符串或二进制数组
   */
  send(data: string | Uint8Array): void;

  /**
   * 直接使用文件作为响应体，后端会流式读取文件内容。
   * 只能调用一次，且与 send() 互斥：
   * - 如果已有 send() 写入的内容，调用 sendFile() 会抛出异常。
   * - 重复调用 sendFile() 会抛出异常。
   * @param path 用户存储中的文件路径（支持嵌套）
   */
  sendFile(path: string): void;

  /**
   * 设置 HTTP 响应状态码
   * @param code HTTP 状态码，如 200, 404, 500
   */
  sendStatus(code: number): void;

  /**
   * 设置响应头。同一个 key 多次调用会覆盖之前的值。
   * @param key 响应头名称
   * @param value 响应头值，支持单个字符串或字符串数组
   */
  sendHeader(key: string, value: string | string[]): void;
};

/**
 * 用户文件存储对象
 *
 * 提供对用户存储空间的文件操作能力，支持任意层级嵌套目录。
 * 所有路径参数使用 "/" 分隔层级，不允许包含路径穿越字符 (".", "..")
 * 或空字符串片段。空字符串表示 root 目录本身。
 */
declare const storage: {
  /**
   * 列出指定目录下的所有直接子项（文件和子目录）
   * @param path 目录路径；空字符串表示 root
   * @returns 目录项数组
   */
  list(path: string): Entry[];

  /**
   * 递归列出所有文件的完整路径
   * @returns 所有文件相对 root 的路径数组
   */
  listAll(): string[];

  /**
   * 创建目录（若父目录不存在会一并创建）
   * @param path 要创建的目录路径
   */
  mkdir(path: string): void;

  /**
   * 读取文件内容
   * @param path 文件完整路径
   * @returns 文件二进制内容
   */
  read(path: string): Uint8Array;

  /**
   * 写入文件（覆盖已有内容，父目录必须已存在）
   * @param path 文件完整路径
   * @param content 要写入的内容，支持字符串或二进制数组
   */
  write(path: string, content: string | Uint8Array): void;

  /**
   * 在文件末尾追加内容（文件不存在则创建）
   * @param path 文件完整路径
   * @param content 要追加的内容，支持字符串或二进制数组
   */
  append(path: string, content: string | Uint8Array): void;

  /**
   * 删除文件或目录（目录递归删除）
   * @param path 要删除的完整路径
   */
  remove(path: string): void;

  /**
   * 重命名或移动文件/目录
   * @param src 源路径
   * @param dst 目标路径
   */
  rename(src: string, dst: string): void;

  /**
   * 判断路径是否存在
   * @param path 要检查的路径
   * @returns 路径存在返回 true，否则返回 false
   */
  exists(path: string): boolean;
};

/**
 * 将字符串或二进制数据编码为 Base64 字符串
 * @param data 要编码的数据
 * @returns Base64 编码后的字符串
 */
declare function base64Encode(data: string | Uint8Array): string;

/**
 * 将 Base64 字符串解码为二进制数据
 * @param data Base64 编码的字符串
 * @returns 解码后的二进制数据
 */
declare function base64Decode(data: string): Uint8Array;

/**
 * 对字符串进行 URL 编码（percent-encoding）
 * @param data 要编码的字符串
 * @returns URL 编码后的字符串
 */
declare function urlEncode(data: string): string;

/**
 * 对 URL 编码的字符串进行解码
 * @param data URL 编码的字符串
 * @returns 解码后的字符串
 */
declare function urlDecode(data: string): string;
`
