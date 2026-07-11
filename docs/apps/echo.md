# Echo · 屏幕记忆 —— 功能说明 + 开发文档

> 意象：回声，过去的回响。
> 一句话定位：**一键抓取屏幕的主要信息，沉淀为可检索的记忆。**

Echo 是 Nexus 产品家族「捕获层」的成员之一。你在屏幕上看过的东西——文档、网页、幻灯片、聊天、报错信息——按一下快捷键就被收进中枢记忆库 Orbit，日后用关键词或语义随时找回。它让「我明明看过，但想不起在哪」成为过去。

- `source` = `echo`
- `kind` = `screen`
- 平台：**Windows + macOS 一期交付**；Linux 放缓；移动端暂不适用（屏幕抓取以桌面为主，见 [architecture.md](../architecture.md) §5.2 平台矩阵）。

---

## 1. 概述与定位

### 1.1 Echo 是什么

Echo 是一个「本地优先 + 端到端加密」的屏幕信息捕获工具，底层复用 `nexus-core`，产出的数据完全遵循 Nexus 统一记忆模型 Memory（见 [data-model.md](../data-model.md)）。它的核心信念是：

> **屏幕上流过的信息，价值不该随窗口关闭而消失。**

按下全局快捷键，Echo 截取当前屏幕，用 OCR 提取文字、用 AI 理解结构（标题、要点、来源应用、URL），然后走 `ingest` 管线切块、嵌入、入库。之后无论你记得的是原文的一个词、还是模糊的语义印象，都能在 Orbit 里找回。

### 1.2 它在家族中的位置

| 维度 | Echo 的职责 | 交给其他软件 |
|------|-------------|-------------|
| 屏幕素材 | 截屏、OCR、屏幕信息结构化理解、入库 | —— |
| 速记/灵感 | —— | **Muse** 负责极速文字/语音捕获 |
| 书写与理解 | 截图可被 Quill 引用进笔记 | **Quill** 负责 Markdown 笔记 |
| 卡片与复习 | 提供素材，可标记「送去复习」 | **Orbit** 负责知识卡片与间隔复习 |
| 检索与消费 | 仅负责写入 | **Orbit** 提供跨源全局检索 |

> **分工边界**：Echo 只负责「把屏幕信息变成高质量、可检索的记忆」。它不做笔记编辑（Quill）、不做速记（Muse）、不生成卡片或调度复习（Orbit）。

### 1.3 典型场景

- **回看看过却忘了出处的资料**：上周读到一段关于向量数据库的说明，只记得「HNSW」这个词——语义检索即可找回原始截图与来源应用。
- **开会时截取关键幻灯片**：一路按快捷键收录，会后 OCR 文本已可检索，无需手动整理。
- **留存易逝信息**：报错弹窗、验证页面、限时展示的内容，先抓下来再说。
- **研究素材归档**：浏览大量网页时随手抓取，Echo 记录来源 URL 与应用，形成可追溯的素材库。

---

## 2. 核心功能详解

### 2.1 全局快捷键抓取

- 注册系统级全局快捷键（经 `platform-desktop` 的 `Hotkey` 能力，见 [architecture.md](../architecture.md) §5.1），任意应用前台时都能触发。
- 触发后由 `platform-desktop` 的 `ScreenCapturer` 抓取当前帧，几乎无感。
- 可配置「抓取即静默入库」或「抓取后弹预览确认」两种节奏（见 §2.7、§7）。

### 2.2 抓取模式

| 模式 | 说明 | 实现要点 |
|------|------|---------|
| 全屏 | 抓取当前显示器整屏 | 多显示器时抓活动显示器（见 §6 多显示器） |
| 活动窗口 | 只抓当前聚焦窗口 | 由平台层拿到活动窗口边界后裁剪 |
| 区域框选 | 拉框选择任意区域 | 唤起轻量选区浮层，确认后截取 |

- 默认模式可在设置里指定；也可为不同快捷键绑定不同模式（如 `Ctrl+Shift+1` 全屏、`Ctrl+Shift+2` 框选）。

### 2.3 OCR 文字提取（本地优先）

Echo 的文字识别统一通过 `nexus-core` 的 `ai::Ocr` trait 调用（见 [nexus-core.md](../nexus-core.md) §8），默认走本地引擎，隐私敏感、可离线：

```rust
pub trait Ocr {
    async fn recognize(&self, image: ImageRef) -> Result<OcrResult>;
}
```

- **默认本地**：系统 OCR（Windows OCR API / macOS Vision）或 PaddleOCR-ONNX，离线可用。
- **可选云端**：识别质量要求高、或需版面理解时，可切云端视觉模型（遵守数据最小化与隐私提示，见 [data-model.md](../data-model.md) §5.5）。
- OCR 结果按行落为 `MediaRef.ocr_text`，并作为 `type = ocr_line` 的 Block 参与切块与嵌入（见 §4）。

### 2.4 AI 结构化理解

在原始 OCR 之上，Echo 可选调用 `ai::Completion`（见 [nexus-core.md](../nexus-core.md) §8）把杂乱的屏幕文字理解为结构化信息：

| 产出 | 说明 |
|------|------|
| 标题 | 为这条屏幕记忆生成简洁标题（写入 `title`） |
| 要点 | 提炼 3–5 条关键信息，作为正文摘要 |
| 来源识别 | 结合系统信息 + 画面内容判断来源应用 / 网页标题 / URL |
| 内容类型 | 判定是代码、文章、聊天、幻灯片等，便于后续过滤 |

- 结构化理解为**可选增强**：无 AI 或离线时，Echo 仍能仅凭 OCR + 元数据完成入库与检索，功能优雅降级。
- 云端调用前统一走数据最小化 + 隐私提示。

### 2.5 自动打标签

- 基于 AI 理解结果与来源应用，自动建议标签（如 `代码`、`会议`、来源应用名），写入 Memory 的 `tags[]`。
- 用户可在预览确认时增删标签；纯静默模式下用自动标签，事后可在 Orbit 调整。

### 2.6 入库到 Memory

所有抓取结果经 `nexus-core` 的 `ingest` 管线统一入库（见 [nexus-core.md](../nexus-core.md) §3）：截图落盘为**加密媒体文件**，OCR 文本切块嵌入，元数据写入 `meta`。写入即广播 `MemoryCreated` 事件，Orbit 等订阅者实时可见（见 §3）。

### 2.7 检索回看

- Echo 自身提供一个轻量「历史」视图，直接用 `nexus-core` 的 `search`（Hybrid：向量 + FTS5 + RRF，见 [nexus-core.md](../nexus-core.md) §4），默认 `filters.source=["echo"]`。
- 结果以时间线 + 缩略图 + OCR 片段高亮呈现；点击查看原图与结构化信息。
- 全局跨源检索的主场在 Orbit；Echo 内检索只为「就地快速找刚抓的东西」。

### 2.8 敏感信息处理

屏幕抓取天然可能含密码、隐私窗口、身份信息。Echo 内置多重防护（对应 [data-model.md](../data-model.md) §5.4）：

| 手段 | 说明 |
|------|------|
| 排除名单 | 按应用 / 窗口标题配置「永不抓取」（如密码管理器、银行 App） |
| 入库前确认 | 预览模式下，用户可先审阅再决定是否入库、是否打码 |
| 敏感检测 | 可选本地模型识别疑似密钥 / 身份证号 / 卡号，提示打码 |
| 手动打码 | 预览时可框选涂抹敏感区域，涂抹作用于入库前的图像与 OCR 文本 |

---

## 3. 用户交互流程

Echo 前端不直接碰数据库；抓取与写入经 Tauri 命令进入 `nexus-core` 的 `ingest` 管线（见 [nexus-core.md](../nexus-core.md) §3、[architecture.md](../architecture.md) §4）。

### 3.1 主流程（按快捷键 → 捕获 → 预览确认 → 入库）

```
用户按下全局快捷键 (platform-desktop::Hotkey)
   ▼
排除名单检查 (当前应用/窗口是否禁抓)
   ├─ 命中 → 静默忽略并轻提示
   └─ 通过 ▼
ScreenCapturer.capture_active() 抓帧 (全屏/活动窗口/区域)
   ▼
本地 OCR (ai::Ocr) → OCR 文本 + 行框
   ▼
(可选) 敏感检测 → 疑似敏感区域提示
   ▼
┌─ 静默模式 ──────────────► 直接入库
└─ 预览模式 ─► 预览浮层
      · 显示截图 + OCR + 建议标题/标签
      · 可打码 / 改标签 / 取消
      · (可选) AI 结构化理解
      确认 ▼
Tauri IPC 命令 capture_commit(image, ocr, meta, tags)
   ▼
nexus-core::ingest.ingest(IngestInput{ source: echo, kind: screen, ... })
   │ 媒体加密落盘 → 切块(ocr_line) → 去重 → 嵌入 → 落库 → 记账(sync)
   ▼
events: MemoryCreated  →  Orbit 等订阅者实时收到，可立即检索
```

### 3.2 降级路径（无权限 / 无 OCR / 无 AI）

```
无屏幕录制/截屏权限 (常见于 macOS)
   → 首启引导授权; 未授权时降级为「手动截图导入」(用户用系统截图, Echo 接收图片入库)

无本地 OCR 可用
   → 仅存图片 + 元数据; OCR 标记为待处理, 权限/引擎就绪后后台补跑

无 AI / 离线
   → 跳过结构化理解与自动标签; 仅凭 OCR + 元数据入库, 检索仍可用
```

> 优雅降级原则：任何单项能力缺失都不阻断「抓取 → 入库 → 检索」的核心闭环。

---

## 4. 数据模型映射

Echo 的每次抓取就是一条 Memory，遵循 [data-model.md](../data-model.md)。字段约定：

| 字段 | Echo 取值 | 说明 |
|------|-----------|------|
| `source` | `echo` | 固定 |
| `kind` | `screen` | 固定 |
| `title` | AI 生成 / 来源窗口标题 | 可选 |
| `content` | OCR 文本汇总（Markdown/纯文本）+ 可选要点 | 检索正文 |
| `content_format` | `plain`（纯 OCR）/ `markdown`（含结构化要点） | |
| `blocks` | OCR 按行/区域聚合切块，`type=ocr_line` | 块级向量与检索单位 |
| `media` | 截图，`kind=image`，`path` 指向**加密文件**，含 `ocr_text`、`hash`、宽高 | 见 [data-model.md](../data-model.md) §2.3 |
| `tags` | 自动 + 手动标签 | |
| `links` | 通常为空；被 Quill 引用时对方写 `references` | |
| `review` | 空 | 复习状态由 Orbit 管理 |
| `captured_at` | 抓取时刻 | 信息实际发生时间 |
| `meta` | Echo 特有扩展 | 见下 |

### 4.1 `meta` 约定（Echo 特有）

```jsonc
"meta": {
  "app_name": "Google Chrome",       // 来源应用
  "window_title": "向量检索 - 维基百科",
  "url": "https://...",               // 若能获取
  "monitor": { "index": 1, "w": 3840, "h": 2160 },  // 来源显示器
  "capture_mode": "active_window",    // fullscreen | active_window | region
  "ocr_engine": "local:vision",       // 本地/云端与引擎
  "ai_understood": true,              // 是否做了结构化理解
  "redacted": false                   // 是否打码
}
```

### 4.2 JSON 示例

一次对浏览器网页的活动窗口抓取：

```jsonc
{
  "id": "01J8ECHOSCRN0001",
  "source": "echo",
  "kind": "screen",
  "title": "向量检索 · HNSW 概述",
  "content": "## 要点\n- HNSW 是一种近似最近邻图索引\n- 分层可导航小世界，查询复杂度近似对数\n\n### 原文摘录\nHierarchical Navigable Small World...",
  "content_format": "markdown",
  "blocks": [
    { "id": "01J...b1", "memory_id": "01J8ECHOSCRN0001", "seq": 0, "type": "ocr_line", "text": "Hierarchical Navigable Small World (HNSW)" },
    { "id": "01J...b2", "memory_id": "01J8ECHOSCRN0001", "seq": 1, "type": "ocr_line", "text": "一种用于近似最近邻搜索的图索引结构" }
  ],
  "media": [
    { "id": "01J...m1", "kind": "image", "path": "media/2026/07/01J8ECHOSCRN0001.enc",
      "mime": "image/webp", "width": 1920, "height": 1080,
      "ocr_text": "Hierarchical Navigable Small World (HNSW) ...", "hash": "sha256:..." }
  ],
  "tags": ["技术", "检索", "来源:Chrome"],
  "links": [],
  "pinned": false,
  "archived": false,
  "created_at": "2026-07-11T10:22:00Z",
  "updated_at": "2026-07-11T10:22:00Z",
  "captured_at": "2026-07-11T10:22:00Z",
  "device_id": "win-desktop-01",
  "meta": {
    "app_name": "Google Chrome",
    "window_title": "HNSW - 维基百科",
    "url": "https://zh.wikipedia.org/wiki/HNSW",
    "monitor": { "index": 0, "w": 1920, "h": 1080 },
    "capture_mode": "active_window",
    "ocr_engine": "local:windows-ocr",
    "ai_understood": true,
    "redacted": false
  }
}
```

---

## 5. 技术实现

### 5.1 应用形态

Echo 是一个 Tauri 2.0 应用（`apps/echo/`，见 [architecture.md](../architecture.md) §3）：

- **前端**（`apps/echo/src`）：React + TypeScript + Vite，UI 用 `@nexus/ui`；主要界面是预览确认浮层、历史/检索视图、设置。
- **外壳**（`apps/echo/src-tauri`）：Rust，装配托盘、全局快捷键、抓取命令；暴露 `capture_now`、`capture_commit`、`search` 等 Tauri 命令。
- **核心**：`src-tauri` 直接依赖 `nexus-core`，或作为 Protocol 客户端连本机记忆服务（见 [architecture.md](../architecture.md) §5.3）。

```
┌───────────────────────────────────────────┐
│ Echo 前端 (WebView)                         │
│  预览确认浮层 · 打码 · 历史/检索 · 设置      │
└──────────────┬──────────────────────────────┘
               │ Tauri IPC
┌──────────────▼──────────────────────────────┐
│ apps/echo/src-tauri (Rust)                   │
│  命令: capture_now / capture_commit / search │
└───────┬───────────────────────────┬──────────┘
        │                           │
┌───────▼────────────┐   ┌───────────▼──────────────┐
│ platform-desktop    │   │ nexus-core                │
│  ScreenCapturer     │   │  ingest(切块/嵌入/加密媒体)│
│  Hotkey             │   │  ai::Ocr / ai::Completion  │
│  (活动窗口/多显示器) │   │  search · events · crypto  │
└─────────────────────┘   └───────────────────────────┘
```

### 5.2 平台适配：`ScreenCapturer`

Echo 依赖 `platform-desktop` 实现的 `ScreenCapturer` trait（见 [architecture.md](../architecture.md) §5.1），App 只依赖 trait，不关心各 OS 差异：

```rust
pub trait ScreenCapturer {
    fn capture_active(&self) -> Result<CapturedFrame>;   // 活动显示器/窗口
    fn capture_region(&self, rect: Rect) -> Result<CapturedFrame>;
    fn list_monitors(&self) -> Vec<MonitorInfo>;
}
```

| 平台 | 截屏实现 | 权限要点 |
|------|---------|---------|
| Windows | Windows.Graphics.Capture / DXGI 复制 | 一般无需特殊授权 |
| macOS | ScreenCaptureKit / CGWindowList | **需「屏幕录制」权限**，首启引导，未授权降级手动 |
| Linux（放缓） | PipeWire / X11 抓帧 | Wayland 需 portal 授权 |

活动窗口边界、窗口标题、来源应用等由平台层一并提供，写入 `meta`。

### 5.3 依赖 `nexus-core` 的具体模块

| 能力 | 依赖模块 | 说明 |
|------|---------|------|
| 抓取入库 | `ingest`（编排 `embed`+`store`+`sync`+`events`） | 媒体加密落盘、OCR 切块嵌入 |
| OCR | `ai::Ocr` | 本地优先，可切云端 |
| 结构化理解 / 标题 / 标签 | `ai::Completion` | 可选增强，离线降级 |
| 就地检索 | `search`（Hybrid，`filters.source=["echo"]`） | 历史视图 |
| 媒体加密 | `crypto`（分块加密 + hash 去重） | 见 [nexus-core.md](../nexus-core.md) §6 |
| 实时联动 | `events`（`MemoryCreated`） | Orbit 实时可见 |

### 5.4 通过 Memory Protocol 写入

- 同机多 App 共享一个记忆库：Echo 或作为**持有者**、或作为**客户端**连本机记忆服务（loopback / IPC，见 [memory-protocol.md](../memory-protocol.md) §3）。
- Echo 的能力域限定为 `memory:write`（`source=echo`），另需 `search`、`memory:read`（见 [memory-protocol.md](../memory-protocol.md) §4.2）。
- 前端不直连数据库/密钥；截图与 OCR 全在 Rust 侧处理，前端只拿授权后的视图（见 [architecture.md](../architecture.md) §4）。

---

## 6. 关键技术难点与对策

| 难点 | 说明 | 对策 |
|------|------|------|
| macOS 屏幕录制权限 | 未授权无法截屏 | 首启引导授权；未授权降级为「手动截图导入」；清晰的权限状态提示 |
| 多显示器 | 抓哪块屏、坐标混乱 | `list_monitors` 识别活动显示器；`meta.monitor` 记录来源；框选跨屏坐标归一化 |
| 高频截图性能 | 连续抓取占 CPU/内存/磁盘 | 抓取用高效编码（WebP）；OCR/嵌入异步进 Rust 线程池；相同画面 `hash` 去重（见 [data-model.md](../data-model.md) §2.3） |
| 隐私窗口 | 误抓密码/隐私 | 排除名单（应用/窗口标题）；入库前确认；敏感检测 + 打码（§2.8、[data-model.md](../data-model.md) §5.4） |
| OCR 准确率 | 小字/复杂版面/多语种识别差 | 本地引擎兜底 + 可选云端视觉模型；保留原图，可随时重跑 OCR；多语种引擎选择 |
| 存储膨胀 | 截图累积占空间 | WebP 压缩 + 加密；`hash` 去重；可设「仅存 OCR 文本不存图」的省空间模式 |

---

## 7. 设置项

| 分类 | 设置 | 默认 |
|------|------|------|
| 快捷键 | 抓取快捷键（可多组，绑定不同模式） | `Ctrl/Cmd+Shift+E` |
| 抓取 | 默认模式（全屏 / 活动窗口 / 区域） | 活动窗口 |
| 确认 | 入库前是否需预览确认 | 预览确认开 |
| OCR | 引擎（本地 / 云端）、语言 | 本地 |
| AI | 结构化理解开关、Provider、自带 Key、云端调用前确认 | 理解开 + 确认开 |
| 隐私 | 排除名单（应用/窗口标题）、敏感检测、默认打码 | 排除名单空，检测开 |
| 存储 | 图片格式/质量、是否仅存 OCR、去重 | WebP，去重开 |
| 检索 | 默认范围（仅 echo / 全库）、默认模式 | 仅 echo，hybrid |
| 同步 | 档位（纯本地 / E2E 云 / 自托管），见 [data-model.md](../data-model.md) §4 | 纯本地 |

---

## 8. 分阶段开发计划

| 阶段 | 范围 | 说明 |
|------|------|------|
| **MVP** | 全局快捷键 + 全屏/活动窗口抓取 + 本地 OCR + 入库（ingest）+ 就地检索 | 打通「抓 → OCR → 入库 → 检索」核心闭环，Win/Mac |
| **二期** | 预览确认 + 区域框选 + 自动标签 + 历史时间线视图 | 交互与组织完善 |
| **三期** | AI 结构化理解（标题/要点/来源/类型）+ 云端 OCR 可切 | 接入 `Completion`，数据最小化护栏 |
| **四期** | 敏感检测 + 打码 + 排除名单 + 省空间模式 | 隐私与存储优化 |
| **五期** | 与家族协作：被 Quill 引用、可标记「送去 Orbit 复习」 | 内联联动 |

> 里程碑与其他软件的排期见 [roadmap.md](../roadmap.md)。

---

## 9. 与各文档的关系

- 数据结构与隐私 → [data-model.md](../data-model.md)
- 核心引擎（ingest / ai::Ocr / ai::Completion / search / crypto）→ [nexus-core.md](../nexus-core.md)
- 截屏/热键平台适配与代码组织 → [architecture.md](../architecture.md)
- 与记忆库交互的接口 → [memory-protocol.md](../memory-protocol.md)
- 检索与复习的消费端 → `apps/orbit.md`
