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
- **内置脚本引擎**：脚本中可访问 `request`、`response`、`storage`、`cache` 等对象与若干工具函数，自带脚本缓存（基于 [moka](https://github.com/moka-rs/moka)）。
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

[script_cache]
max_entries = 1024        # 脚本缓存最大条目数
max_entry_size = 65535    # 单条缓存最大字节数
max_ttl = 3600            # 缓存最大 TTL（秒）

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

| 扩展名   | 用途                                   | 编辑器支持                                                   |
| -------- | -------------------------------------- | ------------------------------------------------------------ |
| `.hjs`   | HTTP `SCRIPT` 处理器脚本（JavaScript） | JS 高亮 + HTTP 脚本引擎类型提示（`request` / `response` 等） |
| `.djs`   | DNS `SCRIPT` 处理器脚本（JavaScript）  | JS 高亮 + DNS 脚本引擎类型提示（`request` / `response`）     |
| `.djson` | DNS `STATIC` 处理器的静态应答（JSON）  | JSON 高亮 + DNS 应答 Schema 校验                             |

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

> 这些扩展名是面向编辑体验的约定，后端按 handler 配置读取对应文件并执行，HTTP `STATIC` 处理器则可指向任意类型的文件原样返回。

## 脚本引擎 API

`SCRIPT` 类型的路由会在请求到来时执行对应的 JavaScript 文件。脚本中均可使用 `request`、`response` 两个全局对象，其中 `storage`、`cache` 与全局工具函数为 HTTP 与 DNS 通用，而 `request` / `response` 因场景不同而结构不同。

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

### 全局工具函数（通用）

`base64Encode`、`base64Decode`、`urlEncode`、`urlDecode`。

## AI 技能（skills）

`skills/xss-receiver/` 提供了一份面向 AI 编程助手的技能（Agent Skill），让 AI 在**看不到本仓库源码**的情况下也能使用本平台的能力：

- 编写脚本引擎处理器：`.hjs`（HTTP）/ `.djs`（DNS）脚本与 `.djson` 静态应答。
- 通过后台 HTTP API 操作平台：上传脚本、创建 / 管理路由、拉取收到的请求日志。

包含的文件：

- `SKILL.md`：入口，能力概览与「上传脚本 → 新建路由 → 拉取最新日志」的端到端工作流。
- `script-engine.md`：脚本引擎 API（`request` / `response` / `storage` / `cache` 与工具函数）与示例。
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
