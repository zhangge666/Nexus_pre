# M6 · 外联实现与验收

本文档说明 M6 外联能力的实现边界、接入方式、安全模型和可重复验收命令。外联仍以 Orbit 桌面持有的本地 Memory Protocol 为唯一明文服务端；第三方应用不直接打开数据库。

## 1. 交付范围

| 能力 | 代码位置 | 当前交付 |
|---|---|---|
| 第三方授权与审计 | `crates/nexus-protocol`、`apps/orbit` | 支持显式 scope、`external:<app_id>` 来源绑定、令牌摘要持久化、活动计数、立即撤销 |
| TypeScript SDK | `packages/sdk-ts` | `@nexus/sdk-ts`，提供固定来源写入、CRUD、检索和问答 |
| Python SDK / CLI | `sdk/python` | `nexus-sdk` 与 `nexus` 命令，零运行时第三方依赖 |
| MCP Server | `sdk/mcp-server` | stdio MCP，提供检索、写入、读取和带引用问答 |
| 浏览器剪藏 | `extensions/clipper` | Manifest V3 + Native Messaging，一键写入 `external:clipper` |
| 连接与隐私 UI | `apps/orbit/src/pages/ConnectionsPage.tsx` | 创建授权、令牌一次性展示、scope、数据流向、访问计数与撤销 |

## 2. 授权模型

### 2.1 第三方登记

第三方应用不能自行匿名登记。用户必须在 Orbit「连接与隐私」中输入应用名称、稳定标识并选择最小 scope；Orbit 使用持有者 `admin` 令牌代为调用：

```http
POST /v1/connections
Authorization: Bearer <orbit-holder-token>
Content-Type: application/json

{
  "app_id": "my-app",
  "name": "My App",
  "source": "external:my-app",
  "scopes": ["memory:read", "memory:write", "search"]
}
```

服务端约束：

- `app_id` 只能包含小写字母、数字、点、短横线和下划线，最长 80 字节。
- `source` 必须严格等于 `external:<app_id>`。
- 第三方只能申请 `memory:read`、`memory:write`、`memory:delete`、`search`、`subscribe`；拒绝 `review` 与 `admin`。
- `memory:write`、`memory:delete` 始终限制在令牌自己的来源；读和检索按用户授予的 scope 访问记忆库。
- 令牌正文只在创建响应中展示一次。连接文件只保存 SHA-256 摘要、scope 和审计元数据。

M3 Muse 保持第一方兼容切片：只能申请 `source=muse` 与 `memory:write`，不会借由 M6 获得更高权限。

### 2.2 撤销与持久化

Orbit 调用 `DELETE /v1/connections/{tokenId}` 后，旧令牌立即返回 `401`。第三方连接存储在共享数据目录的 `connections.json`，其中不包含令牌正文；Orbit 重启后令牌仍可使用和撤销。重复登记不会再次泄露已有令牌，遗失时必须先撤销再创建。

### 2.3 数据流向

连接列表显示：

- 应用 → Orbit：`memory:write` / `memory:delete`；
- Orbit → 应用：`memory:read` / `search` / `subscribe`；
- 最近使用的 scope、累计通过能力校验的读写请求数；
- Memory Protocol 自身是否把数据发送到远程网络。

本地协议只监听回环地址，当前 `sendsDataRemote=false`。RAG Completion Provider 是否外发数据仍由 M4 Provider 设置和问答响应中的 `sends_data_remote` 单独披露。

## 3. SDK 与 CLI

### 3.1 TypeScript

```ts
import { NexusClient } from "@nexus/sdk-ts";

const nexus = new NexusClient({
  endpoint: "http://127.0.0.1:4111",
  token: process.env.NEXUS_TOKEN!,
  source: "external:my-app",
});

await nexus.addMemory({ content: "# 决策记录", tags: ["decision"] });
const hits = await nexus.searchMemory({ text: "决策", mode: "hybrid" });
```

高层客户端固定 `source`，同时通过 `client.protocol` 保留完整 Memory Protocol v1 客户端。

### 3.2 Python 与 `nexus` CLI

```powershell
$env:NEXUS_ENDPOINT = 'http://127.0.0.1:4111'
$env:NEXUS_TOKEN = '<Orbit 生成的令牌>'
$env:NEXUS_SOURCE = 'external:cli'
nexus add '# 一条记忆' --tag example
nexus search '一条记忆' --limit 10
nexus ask '我记录了什么？'
```

Python API 和 CLI 只依赖标准库，适合脚本、ETL 和本地自动化。

## 4. MCP Server

为应用标识 `mcp` 授予 `memory:read`、`memory:write` 和 `search`，然后在 MCP 客户端中配置：

```json
{
  "mcpServers": {
    "nexus": {
      "command": "npx",
      "args": ["-y", "@nexus/mcp-server"],
      "env": {
        "NEXUS_ENDPOINT": "http://127.0.0.1:4111",
        "NEXUS_TOKEN": "<token>",
        "NEXUS_SOURCE": "external:mcp"
      }
    }
  }
}
```

工具：

- `search_memory(query, limit?, source?)`
- `add_memory(content, title?, tags?)`
- `get_memory(id)`
- `ask_memory(question)`

服务使用 stdio 逐行 JSON-RPC，标准输出不混入日志；参数错误以 MCP 工具错误返回。

## 5. 浏览器剪藏

1. 在 Orbit 创建应用标识 `clipper`，只授予 `memory:write`。
2. 在 Chrome/Edge 开发者模式加载 `extensions/clipper`。
3. 用扩展 ID 运行 `extensions/clipper/native-host/install-windows.ps1`。
4. 在安装脚本的安全输入提示中粘贴一次性令牌。

扩展只把当前页面标题、URL、选中文本或有界正文交给本机宿主；令牌保存在宿主配置，不进入网页上下文或扩展存储。宿主只接受 HTTP/HTTPS 页面，并固定写入：

```json
{
  "source": "external:clipper",
  "kind": "clip",
  "meta": {
    "capture_method": "browser_native_messaging",
    "url": "https://example.com"
  }
}
```

## 6. 验收

执行统一脚本：

```powershell
pnpm verify:orbit-m6
```

验收覆盖：

- Rust 协议契约、外部 scope 拒绝、来源越权拒绝、摘要持久化和重启恢复；
- Orbit Rust 编译与前端 TypeScript 检查；
- `@nexus/sdk-ts`、MCP Server、Python SDK/CLI 单元测试；
- Native Messaging 宿主消息帧、来源固定和 URL 边界；
- 浏览器扩展 JavaScript 与 Manifest 语法。

## 7. 后续发布工作

M6 代码与本地验收闭环已完成。正式对外发布仍需在发布流水线中完成 npm/PyPI 包签名、版本发布、浏览器商店审核和 Native Host 安装器签名；这些是发布渠道工作，不改变当前协议与本地产品能力。
