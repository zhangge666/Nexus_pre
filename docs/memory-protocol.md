# Memory Protocol · 外联 API 协议

Memory Protocol 是 Nexus 记忆中枢的开放接口。它让四款软件「内联」互通，也让任意第三方应用、脚本、AI 助手「外联」接入。**这是 Orbit「不只是前三者的大脑」这一定位的技术兑现。**

---

## 1. 设计原则

- **本地优先**：默认走本机回环，无需联网、无延迟、数据不出设备。
- **一套语义，多种传输**：同一组能力，既有本地 HTTP/IPC，也有远程 HTTPS。
- **最小授权**：每个客户端按能力域（scope）授权，令牌可撤销。
- **数据模型即契约**：请求/响应围绕 [data-model.md](data-model.md) 的 Memory 结构。

---

## 2. 两种接入形态

```
┌────────────────────────── 本地 (Local) ──────────────────────────┐
│  同一设备上的应用 → 本机记忆服务 (loopback, 127.0.0.1 / IPC)       │
│  · Echo/Muse/Quill/Orbit 互通                                     │
│  · 本地脚本、浏览器扩展 native messaging、其他桌面 App             │
│  延迟极低，数据不出设备                                            │
└───────────────────────────────────────────────────────────────────┘
┌────────────────────────── 远程 (Remote) ─────────────────────────┐
│  跨设备/云 → 用户的中继或自托管服务 (HTTPS)                        │
│  · 移动端访问桌面记忆库、SaaS 集成、Webhook                        │
│  · 只传加密数据；服务端不解密（见 data-model §4）                  │
└───────────────────────────────────────────────────────────────────┘
```

---

## 3. 本地服务与单实例仲裁

同一设备上四款软件共享一个记忆库，需要一个「持有者」：

- 首个启动的 Nexus 应用（或独立的 `nexus-daemon` 轻服务）成为**持有者**，监听本地端口 / 命名管道。
- 其余应用发现已有持有者后，作为**客户端**连接，不再各自开库。
- 持有者退出时，仲裁移交给下一个存活实例（带库锁与健康检查）。
- 发现机制：约定端口 + 本地锁文件记录当前持有者的端点与公钥。

M1 桌面实现使用应用数据目录中的 `memory-service.lock` 作为排他租约，并通过原子替换的
`memory-service.json` 发布随机回环端口、进程 ID、实例标识、协议版本和本地短期令牌。
持有服务任务一旦退出，租约随任务释放并清理发现记录；其他存活实例再次竞争时即可接管。
客户端在接受发现记录前会校验端点格式、回环地址和 TCP 可达性，避免连接非本地或已经失效的端点。
Orbit 的 IPC 也统一通过该 HTTP 服务读写，不再绕过持有者直接打开 SQLite。

M3 起，桌面产品从各自 Tauri 应用目录的同级 `com.nexus.shared` 目录发现服务；Orbit 的
`nexus.db`、`memory-service.lock` 与 `memory-service.json` 也统一放在该共享目录，避免 Muse
因应用标识不同而误连到独立记忆库。

> 好处：Echo 抓的截图，Orbit 立刻能搜到，无需导入导出——这是「内联」的体验基础。

---

## 4. 鉴权与能力域

### 4.1 令牌

- 客户端首次接入需**用户授权**（本地弹窗确认 / Orbit 内的「已连接应用」管理页）。
- 授权后签发 **capability token**（本地用短期令牌，远程用可撤销的长期令牌）。
- 令牌绑定 scope，可随时在 Orbit 中查看与撤销。

### 4.2 能力域（scope）

| scope | 含义 |
|-------|------|
| `memory:read` | 读取记忆（可再按 source/collection 限定） |
| `memory:write` | 写入记忆（通常限定为该应用自己的 `source`） |
| `memory:delete` | 删除记忆 |
| `search` | 检索 |
| `subscribe` | 订阅变更事件 |
| `review` | 读写复习状态（Orbit 生态） |
| `admin` | 管理集合、连接、导出（仅一等公民应用/用户本人） |

最小授权示例：一个浏览器剪藏扩展只需 `memory:write`（限 `source=external:clipper`）。

### 4.3 第一方来源登记与第三方授权

M3 只开放 Muse 的第一方本地登记切片。Muse 读取当前用户专属的本地发现记录后，以持有者
登记凭据请求一个来源受限令牌：

```http
POST /v1/connections
Authorization: Bearer <holder-registration-token>
Content-Type: application/json

{
  "app_id": "com.nexus.muse",
  "name": "Muse",
  "source": "muse",
  "scopes": ["memory:write"]
}
→ 201 { "tokenId": "...", "token": "...", "source": "muse", "scopes": ["memory:write"] }
```

服务端严格拒绝 Muse 申请其他来源或 scope。M6 第三方授权由用户在 Orbit「连接与隐私」
面板中创建，第三方来源必须严格等于 `external:<app_id>`，且不能申请 `review` 或 `admin`：

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

第三方令牌正文只在创建响应中展示一次；服务端仅把 SHA-256 摘要、scope 与活动审计元数据
持久化到共享数据目录。Orbit 重启后令牌继续有效，重复登记不会再次返回正文，遗失时必须
撤销后重新创建。`memory:write` 与 `memory:delete` 只能操作令牌自己的来源。

Orbit 使用 `admin` 令牌调用
`GET /v1/connections` 查看连接、最近活动与来源记忆数量，调用
`DELETE /v1/connections/{tokenId}` 撤销授权；撤销后旧令牌立即返回 `401`，Muse 保留输入并
要求用户重新连接。M3 Muse 继续使用随本地服务实例存在的短期令牌；M6 第三方令牌使用上述
摘要持久化流程。

---

## 5. 核心 API

以本地 REST 表述（远程同形，换 HTTPS + 长期令牌；另提供 gRPC 与进程内绑定）。

### 5.1 写入

```http
POST /v1/memories
Authorization: Bearer <token>
Content-Type: application/json

{
  "source": "external:my-app",
  "kind": "note",
  "title": "会议要点",
  "content": "## 结论\n- 下周上线\n- 负责人：A",
  "content_format": "markdown",
  "tags": ["meeting"],
  "media": [ { "kind": "image", "data_base64": "...", "mime": "image/png" } ],
  "meta": { "app_specific": "..." }
}
→ 201 { "id": "01J...", "created_at": "..." }
```

写入即走 `ingest` 管线（切块/嵌入/去重/落库/广播）。

### 5.2 检索

```http
POST /v1/search
{
  "text": "上线负责人是谁",
  "mode": "hybrid",
  "filters": { "source": ["quill","external:my-app"], "tags": ["meeting"] },
  "limit": 10
}
→ 200 { "hits": [ { "memory_id":"...", "score":0.83, "snippet":"负责人：A", "block_id":"..." } ] }
```

### 5.3 读取 / 更新 / 删除 / 关联

```http
GET    /v1/memories/{id}
PATCH  /v1/memories/{id}          # 局部更新
DELETE /v1/memories/{id}          # 级联删除 + tombstone
POST   /v1/links                  # { from_id, to_id, relation }
GET    /v1/collections            # 集合列表/管理
```

M1 的关联与集合管理端点使用 `admin` scope：

```http
GET    /v1/links?memory_id={id}
DELETE /v1/links/{from_id}/{to_id}/{relation}

POST   /v1/collections
GET    /v1/collections/{id}
PATCH  /v1/collections/{id}
DELETE /v1/collections/{id}
PUT    /v1/collections/{collection_id}/memories/{memory_id}
DELETE /v1/collections/{collection_id}/memories/{memory_id}
GET    /v1/collections/{collection_id}/memories
```

删除集合会移除其成员关系，并把直接子集合移动到根级；删除 Memory 会级联清理其关联和集合成员关系。

### 5.4 问答 / RAG（可选，需 Completion Provider）

```http
POST /v1/ask
{ "question": "我上周关于定价的想法是什么？", "scope": { "collection": "ideas" } }
→ 200 {
  "answer": "... [1]",
  "citations": [{
    "memory_id":"...", "block_id":"...", "snippet":"...",
    "source_title":"产品定价", "source_kind":"idea", "created_at":1710000000000
  }],
  "provider":"local", "sent_context_count":1, "sends_data_remote":false
}
```

服务端本地检索取材，先应用 collection/source scope，再把最多 6 条、每条最多 1200
字符的必要片段交给 Completion。无命中时不会调用远程 Provider。回答始终返回可回跳的
Memory/Block 引用及实际 Provider、发送片段数和远程数据流向状态。

需要实时呈现时使用同一请求体调用流式端点：

```http
POST /v1/ask/stream
Accept: text/event-stream
```

服务端按顺序发送 `event: meta`（`provider`、`citations`、`sentContextCount`、
`sendsDataRemote`）、零到多条 `event: delta`（`text`）、最终 `event: done`；生成失败时发送
`event: error`（`message`）。检索、scope 过滤、上下文上限与无命中不调用远程 Provider 的规则
与 `/v1/ask` 完全一致。

M4 卡片与复习端点统一使用 `review` scope（`admin` 隐含该能力）：

```http
POST /v1/cards                         # 手动创建 kind=card，并建立 ReviewState
POST /v1/cards/generate                # 从一条明确来源记忆生成卡片
GET  /v1/reviews                       # 全部卡片和来源摘要
GET  /v1/reviews/due                   # 当前到期队列
POST /v1/reviews/{id}/grade            # Again / Hard / Good / Easy
GET  /v1/reviews/stats                 # 到期、新卡、成熟度、streak
POST /v1/reviews/notify-due            # 去重发布 review.due
```

Orbit 持有者可用 `admin` scope 在进程内配置 Completion；响应永不返回 Key：

```http
GET  /v1/completion
POST /v1/completion
{ "provider":"openai", "api_key":"...", "model":"gpt-4.1-mini", "endpoint":null }
```

Provider 可选值为 `local`（抽取式离线回退）、`ollama`（仅回环地址的本地 LLM）、`claude`、
`openai` 与 `custom`。`ollama` 无需 Key，`custom` 指向回环 OpenAI-compatible 服务时可不填 Key；
其他远程端点必须使用 HTTPS。所有配置请求仅在进程内生效，Orbit 桌面端负责将云 Key 保存到系统
凭据库，协议状态和响应均不持久化、不回显 Key。

### 5.5 订阅事件

```http
GET /v1/events?types=memory.created,review.due   (SSE / WebSocket)
→ event: memory.created  data: { "id": "...", "source": "echo" }
```

---

## 6. SDK 与集成方式

| 集成方 | 方式 |
|--------|------|
| 前端/Node 应用 | `@nexus/sdk-ts`（公开 SDK）或 `@nexus/protocol-client`（低层协议客户端） |
| Python 脚本/数据管线 | `nexus-sdk`（PyPI） |
| 桌面其他 App | 本地 REST / gRPC |
| 浏览器扩展 | native messaging → 本地服务（一键剪藏进记忆库） |
| **AI 助手（重点）** | **MCP Server**：把记忆库暴露为 Model Context Protocol 工具，Claude 等助手可直接 `search_memory` / `add_memory`，让你的第二大脑成为任意 AI 的长期记忆 |
| 自动化 | Webhook / CLI (`nexus` 命令行) |

```ts
import { NexusClient } from "@nexus/sdk-ts";
const nexus = new NexusClient({ endpoint, token, source: "external:my-app" });
await nexus.addMemory({ content: "...", tags: ["example"] });
const hits = await nexus.searchMemory({ text: "量子计算笔记", mode: "hybrid" });
```

低层客户端包含 Memory CRUD、混合检索、问答、关联、集合成员管理、服务端错误映射，以及
会自动重连的 `subscribeEvents()` SSE 异步迭代器；公开 SDK 额外固定第三方写入来源。

### MCP 示例（让 AI 助手拥有你的记忆）

```jsonc
// 提供的工具
"add_memory(content, tags?, source?)"
"search_memory(query, filters?)"
"get_memory(id)"
"ask_memory(question)"
```

这样任意支持 MCP 的 AI 客户端都能把 Nexus 当作可检索的长期记忆——真正做到「对接任何有信息记录需求的大脑」。

---

## 7. 版本化与兼容

- 路径版本前缀 `/v1`；字段增量演进，未知字段进 `meta`，保证向后兼容。
- 能力发现：`GET /v1/capabilities` 返回服务端支持的功能、模型档位、scope 列表。
- 契约测试保证 SDK 与服务端一致。

---

## 8. 安全要点

- 本地服务仅监听回环，令牌 + 来源校验，防止本机恶意进程越权。
- 远程仅传密文，服务端零知识（见 data-model.md §4/§5）。
- 所有外部写入标注 `source=external:*`，便于审计、按来源撤销与批量清理。
- 授权、撤销、数据流向在 Orbit 的「连接与隐私」面板集中可见可控。
