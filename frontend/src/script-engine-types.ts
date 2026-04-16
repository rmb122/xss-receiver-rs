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
 * 文件信息对象（storage.list / storage.list_all 返回）
 */
interface FileInfo {
  /** 文件或目录名 */
  readonly name: string;
  /** 文件大小（字节） */
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
   * @param data 响应体数据，支持字符串或二进制数组
   */
  send(data: string | Uint8Array): void;

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
 * 提供对用户存储空间的文件操作能力。
 * 所有路径参数不允许包含路径穿越字符。
 */
declare const storage: {
  /**
   * 列出指定目录下的所有文件和子项
   * @param directory 目录名
   * @returns 文件信息数组
   */
  list(directory: string): FileInfo[];

  /**
   * 递归列出所有目录及其文件
   * @returns 以目录名为 key、文件信息数组为 value 的对象
   */
  list_all(): Record<string, FileInfo[]>;

  /**
   * 在根目录下创建新目录
   * @param directory 要创建的目录名
   */
  create_directory(directory: string): void;

  /**
   * 读取文件内容
   * @param directory 目录名
   * @param filename 文件名
   * @returns 文件二进制内容
   */
  read_file(directory: string, filename: string): Uint8Array;

  /**
   * 写入文件（覆盖已有内容）
   * @param directory 目录名
   * @param filename 文件名
   * @param content 要写入的内容，支持字符串或二进制数组
   */
  write_file(directory: string, filename: string, content: string | Uint8Array): void;

  /**
   * 在文件末尾追加内容（文件不存在则创建）
   * @param directory 目录名
   * @param filename 文件名
   * @param content 要追加的内容，支持字符串或二进制数组
   */
  append_file(directory: string, filename: string, content: string | Uint8Array): void;

  /**
   * 删除文件或目录
   * @param directory 目录名
   * @param filename 文件名；省略时删除整个目录
   */
  delete(directory: string, filename?: string): void;

  /**
   * 重命名或移动文件/目录
   * @param directory 源目录名
   * @param filename 源文件名；传 null 表示操作目录本身
   * @param newDirectory 目标目录名
   * @param newFilename 目标文件名；传 null 表示操作目录本身
   */
  rename(
    directory: string,
    filename: string | null,
    newDirectory: string,
    newFilename: string | null,
  ): void;
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
