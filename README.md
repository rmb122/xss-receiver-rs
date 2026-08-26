# xss-receiver-rs

简体中文 | [English](./README.en.md)

一个使用 Rust 编写的高性能 XSS / 信息接收平台。它内置了 HTTP 与 DNS 两套可编程的服务端，可以灵活地捕获、记录并自定义响应来自外部的请求，适用于 XSS 数据接收、SSRF / OOB 探测、DNS Log 等渗透测试与安全研究场景。

## 功能特性

- **可编程的 HTTP 服务端**：通过路由规则（精确匹配 / 正则匹配）将请求映射到不同的处理器：
  - `STATIC`：直接返回存储中的静态文件。
  - `SCRIPT`：使用内置 JavaScript 引擎（[boa](https://github.com/boa-dev/boa)）动态生成响应。
  - `NONE`：仅记录请求，返回默认响应。
- **可编程的 DNS 服务端**：同样支持基于规则的路由，可静态返回应答或通过脚本动态构造 `A` / `AAAA` / `CNAME` / `TXT` 等记录，可用作 DNS Log。
- **完整的请求日志**：记录 HTTP / DNS 请求的来源、Header、Query、Body、上传文件等，并通过 [ip2region](https://github.com/lionsoul2014/ip2region) 进行 IP 归属地解析。
- **内置脚本引擎**：脚本以 ES 模块运行，支持顶层 `await`，可从 storage 导入其他模块、通过 `http` 主动发送 HTTP 请求，并可访问 `request`、`response`、`storage`、`cache` 等对象。
- **文件存储管理**：支持目录浏览、上传（含分片上传与合并）、下载、重命名、删除等，可供静态路由与脚本直接使用。
- **现代化 Web 管理后台**：基于 Vue 3 + Vuetify，提供 HTTP / DNS 路由与日志、文件、用户、系统日志的管理界面。
- **安全与运维友好**：JWT 鉴权、可自定义的后台前缀（`admin_prefix`）、反向代理真实 IP 解析、可选的 OpenAPI / Swagger 文档。

## 技术栈

- 后端：Rust 2024 edition、[axum](https://github.com/tokio-rs/axum)、[tokio](https://tokio.rs/)、[diesel](https://diesel.rs/) + [diesel-async](https://github.com/weiznich/diesel_async)（PostgreSQL）。
- 脚本引擎：[boa_engine](https://github.com/boa-dev/boa)。
- 前端：Vue 3、Vuetify、Vite、Monaco Editor（通过 `rust-embed` 内嵌进二进制）。
- 数据库：PostgreSQL。

## 快速开始（Docker Compose）

推荐使用 Docker Compose 部署，相关文件位于 `docker/` 目录。项目通过 GitHub Actions 自动构建镜像并推送到 GitHub Container Registry（GHCR），`docker-compose.yml` 已默认使用预构建镜像 `ghcr.io/rmb122/xss-receiver-rs:latest`，**无需在本地从源码构建**。

1. 获取部署文件：克隆仓库，或单独下载 `docker/` 目录下的 `docker-compose.yml` 与 `config.toml`。

2. 准备配置文件 `docker/config.toml`，将其中的占位符替换为真实值：
   - `jwt_secret`：JWT 签名密钥，留空则每次启动随机生成（会导致已签发 token 失效）。
   - `admin_prefix`：管理后台访问前缀，**不能为根路径 `/`**，建议设置为一个不易猜测的值，例如 `/a_secret_admin_path/`。

3. 拉取镜像并启动服务：

```bash
cd docker
docker compose up -d
```

4. 查看日志获取初始管理员账号密码（首次启动时自动创建）：

```bash
docker compose logs server | grep "admin user created"
```

5. 通过 `http://<your-host>:8000/<admin_prefix>/` 访问管理后台并登录。

> 如需启用 DNS 服务，请在 `docker/config.toml` 中设置 `dns_server.listen`（如 `0.0.0.0:53`），并在 `docker-compose.yml` 中放开对应的 UDP 端口映射。

> 若希望本地从源码构建镜像而非拉取，可执行 `docker compose build`（或 `docker compose up -d --build`），构建逻辑见 `docker/Dockerfile`。

## 配置说明

配置文件为 TOML 格式，可参考 `config_example.toml`。主要字段如下：

```toml
db_url = "postgres://postgres:postgres@database/postgres"  # PostgreSQL 连接串
storage_path = "/tmp/"                                     # 文件存储根目录

[http_server]
listen = "0.0.0.0:8000"   # HTTP 监听地址，留空则不启动 HTTP 服务
openapi = true            # 是否启用 OpenAPI / Swagger 文档
jwt_secret = "TEST_VALUE" # JWT 密钥，留空为随机
jwt_expire_time = 259200  # JWT 有效期（秒），默认 3 天
real_addr_header = ""     # 反代场景下获取真实地址的 Header 名，值需为 addr:port 格式（如 nginx 配置 proxy_set_header X-Real-Addr "$remote_addr:$remote_port"; 后填 X-Real-Addr）
admin_prefix = "/super_admin/"  # 管理后台前缀，不能为 /
max_body_size = 3145728   # 最大请求体大小（字节），默认 3MB

[dns_server]
listen = ""               # DNS 监听地址，留空则不启动 DNS 服务

[script.cache]
max_entries = 1024        # 脚本缓存最大条目数
max_entry_size = 65535    # 单条缓存最大字节数
max_ttl = 3600            # 缓存最大 TTL（秒）

[script.http]
allow_private_network = false # 是否允许脚本访问私有、回环与链路本地地址
timeout = 16000               # 单次出站请求的最长时间（毫秒）
max_response_size = 8388608   # 单次出站响应体上限（字节）
max_redirects = 8             # 最多自动跟随的重定向次数，0 表示不跟随

[ip2region]
ipv4_db = "docker/ip2region_v4.xdb"  # IPv4 归属地库路径
ipv6_db = "docker/ip2region_v6.xdb"  # IPv6 归属地库路径
```

运行方式：

```bash
xss-receiver-rs <config_file>
```

## 文件格式约定

路由的处理器（handler）指向存储中的一个文件。平台为不同用途约定了一组扩展名，管理后台的编辑器会据此自动提供语法高亮、类型提示与 Schema 校验：

| 扩展名   | 用途                                      | 编辑器支持                                                   |
| -------- | ----------------------------------------- | ------------------------------------------------------------ |
| `.hjs`   | HTTP `SCRIPT` 处理器或 HTTP 场景 ESM 模块 | JS 高亮 + HTTP 脚本引擎类型提示（`request` / `response` 等） |
| `.djs`   | DNS `SCRIPT` 处理器或 DNS 场景 ESM 模块   | JS 高亮 + DNS 脚本引擎类型提示（`request` / `response`）     |
| `.djson` | DNS `STATIC` 处理器的静态应答（JSON）     | JSON 高亮 + DNS 应答 Schema 校验                             |

`.djson` 静态应答文件的结构如下：

```json
{
  "rcode": "NOERROR",
  "ttl": 60,
  "answers": [{ "type": "A", "value": "1.2.3.4", "ttl": 60 }]
}
```

- `rcode`：`NOERROR` / `NXDOMAIN` / `SERVFAIL` / `REFUSED` / `FORMERR` / `NOTIMP`，默认 `NOERROR`。
- `ttl`：默认 60。
- `answers[].type`：`A` / `AAAA` / `CNAME` / `TXT`。

> 这些扩展名是面向编辑体验的约定。模块运行时使用哪套 HTTP / DNS 全局 API 由入口路由决定，而不是由被导入文件的扩展名决定；管理后台仅提供当前文件的类型提示，不解析跨文件导出。HTTP `STATIC` 处理器可指向任意类型的文件原样返回。

## 脚本引擎 API

`SCRIPT` 类型的路由会在请求到来时将对应 JavaScript 文件作为 **ES 模块**执行。HTTP 与 DNS 脚本均支持顶层 `await`、静态 `import` 和动态 `import()`；使用 `export default value` 将入口模块的结构化数据写入请求日志的 `extra_info`，未导出时写入 `null`。路由的 `timeout` 是包括异步操作在内的总执行时限。

脚本中均可使用 `request`、`response`、`storage`、`cache`、`http` 与全局工具函数；`request` / `response` 因场景不同而结构不同。

### 模块加载

脚本可以导入 user storage 中的其他 ESM 文件：

```js
import { normalize } from './lib/normalize.hjs'
const shared = await import('shared/utils.js')
```

- `./` 和 `../` 相对当前模块解析；不以 `./` / `../` 开头的路径相对 user storage 根目录解析。路径规范化后不得越过 storage 根目录。
- 路径必须包含准确文件名，不自动补 `.js`，也不自动解析 `index.js`。
- 加载器不限制扩展名，所有依赖都按 UTF-8 ESM JavaScript 解析；不支持 CommonJS `require`、JSON 模块、npm / Node 模块或 URL 导入。
- 所有依赖共享入口模块的 Context。HTTP 路由加载的依赖获得 HTTP 版 `request` / `response`，DNS 路由加载的依赖获得 DNS 版对象，即使文件扩展名不同也不会切换 Context。
- 同一路径在单次请求中只加载和执行一次；每个新请求都会创建新 Context 并重新读取入口及依赖文件。只有入口模块的 `export default` 会写入日志。

### `request`（HTTP）

| 属性 / 方法          | 说明                                                     |
| -------------------- | -------------------------------------------------------- |
| `request.method`     | 请求方法                                                 |
| `request.path`       | 请求路径                                                 |
| `request.clientAddr` | 客户端地址                                               |
| `request.body`       | 原始请求体（`Uint8Array`）                               |
| `request.headers`    | 请求头，支持 `headers.get(key)`                          |
| `request.query`      | 查询参数，支持 `query.get(key)`                          |
| `request.json`       | 解析后的 JSON body                                       |
| `request.forms`      | 表单字段，支持 `forms.get(key)`                          |
| `request.files`      | 上传文件，`files.get(name)` 返回 `{ filename, content }` |

### `response`（HTTP）

| 方法                              | 说明                                                    |
| --------------------------------- | ------------------------------------------------------- |
| `response.send(data)`             | 写入响应体（字符串或 `Uint8Array`），与 `sendFile` 互斥 |
| `response.sendFile(path)`         | 以存储中的文件作为响应体，仅可调用一次                  |
| `response.sendStatus(code)`       | 设置状态码                                              |
| `response.sendHeader(key, value)` | 设置响应头，`value` 可为字符串或字符串数组              |

### `request`（DNS）

| 属性                 | 说明                        |
| -------------------- | --------------------------- |
| `request.name`       | 查询的域名                  |
| `request.type`       | 查询类型（如 `A` / `AAAA`） |
| `request.class`      | 查询类（如 `IN`）           |
| `request.clientAddr` | 客户端地址                  |

### `response`（DNS）

| 方法                                 | 说明                                                           |
| ------------------------------------ | -------------------------------------------------------------- |
| `response.answer(type, value, ttl?)` | 追加一条应答记录，`type` 支持 `A` / `AAAA` / `CNAME` / `TXT`   |
| `response.rcode(code)`               | 设置响应码，如 `NOERROR` / `NXDOMAIN` / `SERVFAIL` / `REFUSED` |

### `storage`（通用）

`list(path)`、`listAll()`、`mkdir(path)`、`read(path)`、`write(path, content)`、`append(path, content)`、`remove(path)`、`rename(src, dst)`、`exists(path)`。

### `cache`（通用）

`cache.set(key, value, ttl?)`、`cache.get(key)`、`cache.delete(key)`、`cache.incr(key, delta?)`。

### `http`（通用）

`http` 是服务端出站 HTTP 客户端，提供 `request`、`get`、`post`、`put`、`patch`、`delete`、`head` 方法，并返回 Promise：

```js
const upstream = await http.post('https://example.com/api', {
  headers: { 'content-type': 'application/json' },
  body: JSON.stringify({ source: request.clientAddr }),
  timeout: 5000,
})

const data = upstream.json()
export default { status: upstream.statusCode, data }
```

请求选项包括 `method`、`headers`、`body`（字符串或 `Uint8Array`）、`timeout`、`maxResponseSize`、`maxRedirects` 与 `tlsVerify`。响应包含 `statusCode`、最终 `url`、多值 `headers`、`body`，以及可重复调用的 `text()` / `json()`。4xx/5xx 会正常返回；网络错误、超时、无效 JSON 或超过响应体上限时抛出异常。请求级限制只能收紧服务端配置；`tlsVerify` 默认为 `true`，请仅在确有需要时关闭。

默认禁止访问私有、回环、链路本地、CGNAT 与云元数据等非公网地址，并对 DNS 解析和每次重定向重新检查。只有显式设置 `script.http.allow_private_network = true` 才会开放这些地址；系统代理不会被使用。

### 全局工具函数（通用）

`base64Encode`、`base64Decode`、`urlEncode`、`urlDecode`。

## AI 技能（skills）

`skills/xss-receiver/` 提供了一份面向 AI 编程助手的技能（Agent Skill），让 AI 在**看不到本仓库源码**的情况下也能使用本平台的能力：

- 编写脚本引擎处理器与模块：`.hjs`（HTTP）/ `.djs`（DNS）脚本、可复用 ESM 模块与 `.djson` 静态应答。
- 通过后台 HTTP API 操作平台：上传脚本、创建 / 管理路由、拉取收到的请求日志。

包含的文件：

- `SKILL.md`：入口，能力概览与「上传脚本 → 新建路由 → 拉取最新日志」的端到端工作流。
- `script-engine.md`：脚本引擎 API（`request` / `response` / `storage` / `cache` / `http` 与工具函数）与示例。
- `admin-api.md`：后台 API（鉴权 / 文件 / 路由 / 日志）与 curl 端到端示例。

使用方式：让 AI 助手读取 `skills/xss-receiver/SKILL.md` 即可。由于 Base path（host、`admin_prefix`）因部署而异，技能要求 AI 在调用 API 前主动向人类索取。

## 本地开发

### 前置依赖

- Rust（nightly，使用 2024 edition）
- Node.js + [pnpm](https://pnpm.io/)
- PostgreSQL
- `libpq` 开发库（用于 diesel）

### 构建前端

```bash
cd frontend
pnpm install
pnpm build   # 产物输出到 frontend/dist，会被 rust-embed 内嵌
```

### 构建并运行后端

```bash
cp config_example.toml config.toml   # 按需修改
cargo run --release -- config.toml
```

## 目录结构

```
.
├── src/                # Rust 后端源码
│   ├── controllers/    # HTTP 接口与请求入口
│   ├── dispatcher/     # HTTP / DNS 路由分发与脚本引擎
│   ├── db/             # diesel 模型与查询
│   ├── storage/        # 文件存储
│   └── utils/          # DNS server、ip2region、JWT 等工具
├── frontend/           # Vue 3 + Vuetify 管理后台
├── skills/             # 面向 AI 编程助手的技能（Agent Skill）
├── docker/             # Docker / Compose 部署文件
├── migrations/         # 数据库迁移
└── thirdparty/         # 定制的第三方依赖（http / httparse）
```

## 免责声明

本项目仅用于授权范围内的安全测试、研究与学习。请勿用于任何非法用途，使用者需自行承担因不当使用而产生的一切后果。
