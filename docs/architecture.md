# Nexus 技术架构与选型

本文档描述 Nexus 产品家族的整体技术架构、代码组织方式、跨端策略，以及为什么这样选型。

---

## 1. 设计目标与约束

| 目标 | 含义 | 对架构的影响 |
|------|------|-------------|
| 跨端 | Orbit 覆盖桌面+移动；Echo/Muse/Quill 至少 Win+Mac | 需要一套代码尽量复用的框架 |
| 高性能 | 屏幕抓取、向量检索、加密不能卡顿 | 核心逻辑用 Rust，避免 GC 抖动 |
| 极致轻量 | Muse「呼之即来挥之即去」，冷启动要快 | 包体小、内存占用低 |
| 本地优先 | 数据默认在本地，端到端加密 | 存储/加密/同步下沉到共享核心 |
| 可插拔 AI | 本地/云端模型可切换 | AI 能力抽象为 Provider 接口 |
| 开放外联 | 任意应用可接入记忆库 | 核心暴露稳定协议，而非藏在 App 内部 |

---

## 2. 技术选型

### 2.1 应用外壳：Tauri 2.0

选择 **Tauri 2.0** 而非 Electron / Flutter，核心理由：

- **真正的跨端**：Tauri 2.0 正式支持桌面（Win/macOS/Linux）**和移动端（iOS/Android）**，正好覆盖 Orbit「必须有手机端」的硬需求。
- **包体极小**：Tauri 应用通常 3–10 MB（复用系统 WebView），Electron 动辄 100+ MB。这对 Muse 的「轻量、秒开」定位是决定性的。
- **性能与安全**：业务核心跑在 Rust 侧（原生线程、无 GC），前端只负责渲染。屏幕抓取、向量检索、加密这类重活天然适合 Rust。
- **一套前端**：UI 用 Web 技术，四款软件共享同一套设计系统与组件库。

> **权衡说明**：Tauri 移动端相对年轻，某些原生能力（如 Android 无障碍服务、iOS 后台）需要写平台特定插件。我们通过 `nexus-core` 下的平台适配层隔离这些差异，见 §5。若后续遇到移动端原生瓶颈，Orbit 移动端保留「局部改用原生视图」的退路。

### 2.2 共享核心：Rust `nexus-core`

所有与「记忆」相关的核心逻辑写一次，四端复用：

- 存储引擎（SQLite）
- 向量检索（sqlite-vec）
- 加密与密钥管理
- 同步引擎（CRDT）
- AI Provider 抽象
- Memory Protocol 实现

前端（React）通过 Tauri 的 IPC 调用核心；外部应用通过 Memory Protocol（本地 HTTP/gRPC 或远程 API）调用核心。**同一套核心，两种入口。**

> **移动端边界**：Orbit 手机端不作为 Memory Protocol 持有者，也不监听本地 HTTP/TCP 端口。Android 的 E2E 云模式持有由 Keystore 托管密钥保护的本地内容副本和待上传 oplog，记忆/集合在本机读写、合并和关键词检索，只向独立中继发送签名密文；自托管模式才调用远程 Memory Protocol 提供复习、卡片、语义检索和 AI 问答。移动构建不链接 SQLite、协议服务或本地 ONNX Runtime，避免包体与 Android ABI 兼容性成本。

### 2.3 前端：React + TypeScript + Vite

- 生态成熟，招人容易，AI 辅助友好。
- 四款软件共享 `@nexus/ui`（设计系统）与 `@nexus/protocol-client`（协议客户端）。
- 状态管理：Zustand（轻量）；数据获取：TanStack Query。
- 编辑器（Quill 用）：CodeMirror 6 / TipTap，见 apps/quill.md。

### 2.4 本地存储：SQLite + sqlite-vec

- 单文件、事务安全、久经考验、跨端一致。
- `sqlite-vec` 扩展提供向量列与近邻检索，**关系数据与向量数据同库**，避免额外部署向量数据库。
- 全文检索用 SQLite FTS5，与向量检索做混合排序（见 nexus-core.md）。

### 2.5 AI 能力：分层可插拔

| 任务 | 默认位置 | 备选 |
|------|---------|------|
| 文本嵌入 (embedding) | 本地小模型 (bge-small / gte via ONNX) | 云端 embedding API |
| OCR（Echo） | 本地 (系统 OCR / PaddleOCR-ONNX) | 云端视觉模型 |
| 语音转写（Muse） | 本地 (whisper.cpp / Whisper via Candle) | 云端 ASR |
| 总结/卡片/问答 | 云端大模型 (Claude 等) | 本地 LLM (Ollama) |

用户可在设置里逐项切换，或选择「完全离线」。详见 nexus-core.md §AI Provider。

---

## 3. 代码组织：Monorepo

```
nexus/
├── crates/                       # Rust 工作区
│   ├── nexus-core/               # 共享核心库（存储/检索/加密/同步/AI）
│   ├── nexus-protocol/           # Memory Protocol 定义与服务端实现
│   ├── nexus-ai/                 # AI Provider 抽象与本地推理
│   └── platform/                 # 平台适配（截屏/热键/无障碍/后台）
│       ├── platform-desktop/
│       └── platform-mobile/
│
├── apps/                         # 四款软件（Tauri 应用）
│   ├── echo/
│   │   ├── src-tauri/            # Rust 侧：命令、插件装配
│   │   └── src/                  # React 前端
│   ├── muse/
│   ├── quill/
│   └── orbit/
│
├── packages/                     # 共享前端包 (pnpm workspace)
│   ├── ui/                       # @nexus/ui 设计系统与组件
│   ├── protocol-client/          # @nexus/protocol-client 协议 TS 客户端
│   ├── sdk-ts/                   # @nexus/sdk-ts 第三方公开 SDK
│   ├── editor/                   # @nexus/editor 共享编辑器（Quill/Muse 复用）
│   └── shared/                   # 类型、工具、i18n
│
├── sdk/                          # 外联 SDK（供第三方使用）
│   ├── python/                   # nexus-sdk 与 nexus CLI
│   └── mcp-server/               # 面向 AI 助手的 MCP 服务
│
├── extensions/
│   └── clipper/                  # Manifest V3 + Native Messaging 剪藏器
│
└── docs/
```

- **Rust 侧**用 Cargo workspace；**JS 侧**用 pnpm workspace；顶层用 Turborepo/Nx 编排构建缓存。
- 四个 app 各自是独立可发布单元，但共享 crates 与 packages，改一处四端受益。

---

## 4. 运行时架构（单个 App 内部）

```
┌─────────────────────────────────────────────────────────┐
│  前端 (WebView)  ── React + @nexus/ui                     │
│    UI / 交互 / 本地状态                                    │
└───────────────┬───────────────────────────┬─────────────┘
                │ Tauri IPC (命令/事件)       │
┌───────────────▼───────────────────────────▼─────────────┐
│  App 外壳 (src-tauri, Rust)                               │
│    命令处理 · 窗口/托盘/热键 · 插件装配                    │
└───────────────┬───────────────────────────┬─────────────┘
                │                            │
┌───────────────▼──────────────┐  ┌──────────▼─────────────┐
│  nexus-core (Rust)            │  │  platform-* (Rust)     │
│   存储 · 检索 · 加密 · 同步    │  │   截屏 · 热键 · ASR/OCR │
│   AI Provider · Protocol      │  │   无障碍 · 后台服务     │
└───────────────┬──────────────┘  └────────────────────────┘
                │
┌───────────────▼──────────────────────────────────────────┐
│  本地存储: SQLite (+ sqlite-vec, FTS5) · 加密文件 · 密钥    │
└───────────────────────────────────────────────────────────┘
```

关键点：**前端不直接碰数据库或密钥**。所有敏感操作经由 Rust 命令，前端只拿到解密后的、经过授权的视图数据。

---

## 5. 跨端策略

### 5.1 复用与差异的边界

- **完全复用**：`nexus-core`、`nexus-protocol`、`nexus-ai`、所有 `packages/*`。
- **平台适配层**：`platform-desktop` 与 `platform-mobile` 实现同一组 trait（如 `ScreenCapturer`、`Hotkey`、`AudioRecorder`、`BackgroundTask`），App 只依赖 trait，不关心实现。

```rust
// 概念示意：平台能力以 trait 抽象，编译期按目标平台选实现
pub trait ScreenCapturer {
    fn capture_active(&self) -> Result<CapturedFrame>;
}
// desktop: 用系统截屏 API；mobile: 用 MediaProjection / ReplayKit
```

### 5.2 各软件的平台矩阵

| 软件 | Windows | macOS | Linux | iOS | Android | 说明 |
|------|:---:|:---:|:---:|:---:|:---:|------|
| Echo | ✅ | ✅ | ⏳ | ➖ | ➖ | 屏幕抓取以桌面为主 |
| Muse | ✅ | ✅ | ⏳ | ⭐ | ⭐ | 移动端为理想速记场景（二期） |
| Quill | ✅ | ✅ | ⏳ | ⏳ | ⏳ | 桌面优先，移动端阅读为主 |
| Orbit | ✅ | ✅ | ⏳ | ⏳ | ✅ | Android 在 M5 交付；iOS 保留为硬需求并在 M8 最后收口 |

✅=当前路线正式交付　⭐=高优先级目标　⏳=后续　➖=暂不适用。Orbit 当前只推进 Android，iOS 不在 M5–M7 开发，统一放到路线图最后的 M8；具体排期以 [roadmap.md](roadmap.md) 为准。

### 5.3 一个记忆库，多个客户端

同一台设备上，四款软件**共享同一个本地记忆库**（默认路径下的 SQLite）。它们不各自存一份，而是都作为 Memory Protocol 的客户端连到本机的记忆服务：

> **交付顺序说明**：上述架构是最终形态。M3 只用 Muse 最小文字来源验证“独立客户端 → Protocol → Orbit”的共享库链路；Echo、Muse、Quill 的完整产品壳、平台能力和深度内联统一在 M7 开发。

- 方案：由**首个启动的 App 或一个独立的轻量后台服务**持有记忆库，其余 App 通过本地回环（loopback）连接。
- 好处：Echo 抓的、Muse 记的、Quill 写的，**立刻都能在 Orbit 里检索到**，无需导入导出。
- 详见 memory-protocol.md（本地服务发现与单实例仲裁）。

---

## 6. 关键技术风险与对策

| 风险 | 影响 | 对策 |
|------|------|------|
| Tauri 移动端成熟度 | Orbit 移动端受阻 | 平台适配层隔离；M5 优先验证 Android 关键路径，M8 再处理 iOS；必要时局部原生 |
| 本地共享记忆库并发 | 多 App 同时写冲突 | 单一持有者 + Protocol 中介，写操作串行化，CRDT 兜底 |
| 屏幕抓取权限（Echo） | macOS/移动端权限复杂 | 首启引导授权；无权限时降级为手动截图 |
| 本地 AI 体积/性能 | 影响轻量目标 | 模型按需下载；低配设备默认走云端；量化模型 |
| 端到端加密 vs 语义检索 | 服务端无法检索密文 | 检索在**本地明文**上做；云端只做加密块中继与可选的密文备份 |

---

## 7. 构建、质量与发布

- **CI**：Rust `cargo test/clippy` + 前端 `vitest/eslint`；跨平台矩阵构建先覆盖 Win/mac 与 Android，iOS runner 和签名验证延后到 M8。
- **测试策略**：`nexus-core` 单元测试全覆盖核心算法（检索、加密、同步）；Protocol 有契约测试；App 层做关键流程 E2E。
- **发布**：桌面走 Tauri updater（增量更新，签名）；M5 Android 走 Google Play，App Store 发布准备延后到 M8；SDK 走 npm / PyPI。
- **可观测性**：本地优先前提下，遥测默认关闭、可选开启且匿名。

---

## 8. 下一步

- 核心引擎细节 → [nexus-core.md](nexus-core.md)
- 数据模型与隐私 → [data-model.md](data-model.md)
- 外联协议 → [memory-protocol.md](memory-protocol.md)
- 各软件开发文档 → [apps/](apps/)
- 里程碑 → [roadmap.md](roadmap.md)
