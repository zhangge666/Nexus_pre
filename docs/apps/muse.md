# Muse · 灵感捕捉 —— 功能说明 + 开发文档

> 意象：缪斯，灵感女神。
> 一句话定位：**呼之即来、挥之即去的极速速记与语音记录。**

Muse 是 Nexus 产品家族「捕获层」中最轻、最快的一员。一个念头冒出来，按下快捷键，浮层秒现，打字或说话，回车即存，浮层消失——全程不到两秒，想法直接落进中枢记忆库 Orbit。它的存在只为一件事：**让捕获灵感的成本趋近于零。**

- `source` = `muse`
- `kind` = `idea`（灵感）/ `voice`（语音）/ `note`（较长速记）
- 平台目标：完整产品覆盖 Windows + macOS，移动端后续跟进（见 [architecture.md](../architecture.md) §5.2 平台矩阵）。具体交付顺序以 [roadmap.md](../roadmap.md) 为准。

> **当前排期边界**：路线图 M3 只从 Muse 提取一个“文字输入 → Memory Protocol 写入 → Orbit 即时可见”的最小来源适配器，用于验证 Orbit 的跨应用接入能力。全局热键、托盘、语音、收件箱、冷启动优化和移动端均属于最后阶段 M7 的 Muse 产品化，不是 M3 的交付范围。

---

## 1. 概述与定位

### 1.1 Muse 是什么

Muse 是一个极致轻量的速记工具，底层复用 `nexus-core`，产出数据遵循 Nexus 统一记忆模型 Memory（见 [data-model.md](../data-model.md)）。它的核心信念是：

> **想法在脑中停留的时间越短越好。捕获的摩擦，就是灵感的敌人。**

Muse 不追求功能繁多，而追求「快」和「轻」：常驻托盘、后台静默、冷启动极快；全局快捷键唤起一个无边框浮层，支持文字与语音，写完即走。它既是速记本，也是灵感收件箱——先无脑接住，之后再由你或 Orbit 慢慢整理。

### 1.2 它在家族中的位置

| 维度 | Muse 的职责 | 交给其他软件 |
|------|-------------|-------------|
| 速记/灵感 | 极速文字/语音捕获、零摩擦入库 | —— |
| 屏幕素材 | —— | **Echo** 负责屏幕抓取 |
| 深度书写与理解 | 速记可「整理为」Quill 笔记 | **Quill** 负责 Markdown 笔记与知识网络 |
| 卡片与复习 | 提供素材，可被 Orbit 归纳 | **Orbit** 负责知识卡片与间隔复习 |
| 检索与消费 | 仅负责写入 + 就地速查 | **Orbit** 提供跨源全局检索 |

> **分工边界**：Muse 只做「秒级捕获」。它不做长文编辑（Quill）、不做屏幕抓取（Echo）、不做卡片与复习（Orbit）。一条速记若需深化，可一键「整理为 Quill 笔记」，或留在收件箱由 Orbit 归纳。

### 1.3 典型场景

- **走路时冒出的想法**：掏出手机（二期）或回到电脑，一句话按快捷键说出来，语音转写自动入库。
- **会议中一闪念**：不打断会议，快捷键唤起浮层敲两行，失焦即存。
- **睡前灵感**：黑暗中不想开电脑，语音记下，第二天在 Orbit 里已可检索。
- **临时暂存**：网址、一句话、一个待办，先扔进 Muse，之后整理或让 Orbit 归纳。

---

## 2. 核心功能详解

### 2.1 全局快捷键秒唤起（Quick Capture）

- 注册系统级全局快捷键（经 `platform-desktop` 的 `Hotkey` 能力，见 [architecture.md](../architecture.md) §5.1），任意场景下都能唤起。
- 唤起的是一个**无边框、居中/贴边的极简浮层**，只有一个输入框和麦克风按钮，无菜单、无干扰。
- **冷启动优化**是第一优先级：进程常驻托盘/后台，唤起只是显示已就绪的窗口，目标唤起延迟 < 200ms（见 §6）。

### 2.2 失焦即隐藏 / 自动保存

- 浮层遵循「挥之即去」：**失焦、按下 Esc、或回车提交**都会隐藏浮层。
- 隐藏时若有内容则**自动保存入库**（可配置：回车提交 / 失焦也提交），绝不因为「忘了保存」而丢失灵感。
- 未写完就切走？可配置「保留草稿」，下次唤起接着写。

### 2.3 语音输入（本地转写）

Muse 的语音能力统一通过 `nexus-core` 的 `ai::Transcriber` trait 调用（见 [nexus-core.md](../nexus-core.md) §8），录音由 `platform` 的 `AudioRecorder` 提供：

```rust
pub trait Transcriber {
    async fn transcribe(&self, audio: AudioRef, opts: AsrOpts) -> Result<Transcript>;
}
```

- **默认本地**：whisper.cpp / Whisper via Candle，离线、隐私。
- **可选云端**：需更高准确率/多语种时切云端 ASR（遵守数据最小化与隐私提示，见 [data-model.md](../data-model.md) §5.5）。
- 录音文件作为 `media`（`kind=audio`）落盘**加密**保存，转写文本写入 `MediaRef.transcript`，并作为 `type=transcript` 的 Block 参与切块嵌入（见 §4）。
- 边说边转（流式）或说完再转，可配置；转写完成再入库，保证检索文本可用。

### 2.4 纯文本 / 轻 Markdown 速记

- 输入框支持纯文本与**轻量 Markdown**（换行、列表、`#tag`），不追求 Quill 的完整编辑能力——保持轻。
- 复用共享编辑器包 `@nexus/editor` 的**极简配置**（见 [architecture.md](../architecture.md) §3），去掉重型特性，只保留速记所需。
- 内容以 Markdown/纯文本存为 `content`，交给 `ingest` 切块（`kind=idea/note` 按段落切）。

### 2.5 自动时间地点标签

- 入库时自动记录 `captured_at`（灵感发生时刻）、`device_id`；移动端（二期）可选记录粗粒度位置到 `meta.location`。
- 可选按内容自动建议轻标签；但为保持「零摩擦」，默认不打断用户，标签事后在 Orbit 补。

### 2.6 收件箱与后续处理

Muse 是「先接住，再整理」的哲学，提供一个轻量**收件箱（Inbox）**视图：

- 列出最近速记（`search`，`filters.source=["muse"]`，按时间倒序），支持就地速查。
- 每条速记可：
  - **整理为 Quill 笔记**：把速记内容作为初稿交给 Quill 深化（跨 App，经记忆库；Quill 侧建立关联，见 [apps/quill.md](quill.md) §2.7）。
  - **送去 Orbit 归纳/复习**：标记候选，由 Orbit 生成卡片或纳入复习（见 [apps/orbit.md](orbit.md)）。
  - 打标签 / 归档 / 删除。
- 「整理/归纳」是**可选的下游动作**，不整理的速记依然是完整、可检索的记忆。

### 2.7 置顶速记板 / 剪贴板监听（可选）

- **置顶速记板**：一个可常驻桌面角落的迷你便签，随手涂写，同样入库。
- **剪贴板监听**（默认关闭、需显式开启）：可将复制的文本一键存为速记（`source=muse`），方便暂存网址/片段。开启时明确告知隐私影响。

---

## 3. 交互流程

Muse 前端不直接碰数据库；写入经 Tauri 命令进入 `nexus-core` 的 `ingest` 管线（见 [nexus-core.md](../nexus-core.md) §3、[architecture.md](../architecture.md) §4）。

### 3.1 主流程（快捷键 → 浮层 → 输入 → 即存 → 消失，全程 < 2s）

```
用户按下全局快捷键 (platform::Hotkey)
   ▼
显示已就绪的浮层 (常驻进程, <200ms)   ← 冷启动优化的关键
   ▼
┌─ 文字 ─────────────► 打字 (轻 Markdown)
└─ 语音 ─► AudioRecorder 录音 ─► ai::Transcriber 转写 ─► 文本
   ▼
提交 (回车 / 失焦, 视配置) 或 Esc 取消
   ▼
Tauri IPC 命令  quick_capture(text?, audio?, kind, tags?)
   ▼
nexus-core::ingest.ingest(IngestInput{ source: muse, kind: idea/voice/note, ... })
   │ (语音: 音频加密落盘 + transcript) → 切块 → 去重 → 嵌入 → 落库 → 记账(sync)
   ▼
events: MemoryCreated  →  浮层消失, Orbit 等订阅者实时收到
```

### 3.2 「稍后处理」收件箱流

```
速记入库 (source=muse)
   ▼
Inbox 视图 (search filters.source=[muse], 时间倒序)
   ├─ 整理为 Quill 笔记 → 交 Quill 深化 (Quill 写 Link references, 见 quill.md §2.7)
   ├─ 送去 Orbit → 标记候选 → Orbit 生成 card(derived_from) / 纳入复习
   ├─ 打标签 / 归档
   └─ 不处理 → 仍是完整可检索的记忆
```

### 3.3 降级路径

```
无本地语音引擎 / 离线 → 语音按钮禁用或提示; 文字速记不受影响
云端 ASR 不可用       → 自动回落本地; 本地不可用则仅存音频, 后台就绪后补转写
全局热键冲突          → 引导用户改键 (见 §6)
```

> 优雅降级原则：任何能力缺失都不阻断「唤起 → 文字速记 → 入库」的核心闭环。

---

## 4. 数据模型映射

Muse 的每条速记就是一条 Memory，遵循 [data-model.md](../data-model.md)。字段约定：

| 字段 | Muse 取值 | 说明 |
|------|-----------|------|
| `source` | `muse` | 固定 |
| `kind` | `idea` / `voice` / `note` | 灵感 / 语音 / 较长速记 |
| `title` | 通常空（正文首行代替） | 保持轻 |
| `content` | 文本 / 转写文本（纯文本或轻 Markdown） | 检索正文 |
| `content_format` | `plain` / `markdown` | |
| `blocks` | 按段落/句切块（语音按转写句） | 块级向量 |
| `media` | 语音时含 `kind=audio`，`path` 指向**加密音频**，含 `transcript`、`duration`、`hash` | 见 [data-model.md](../data-model.md) §2.3 |
| `tags` | 可选自动/手动 | 默认不打断 |
| `links` | 通常空；被 Quill 引用时对方写 `references` | |
| `review` | 空 | 复习状态由 Orbit 管理 |
| `captured_at` | 灵感发生时刻 | |
| `meta` | Muse 特有扩展 | 见下 |

### 4.1 `meta` 约定（Muse 特有）

```jsonc
"meta": {
  "capture_method": "voice",       // text | voice | clipboard | scratchpad
  "asr_engine": "local:whisper",   // 语音引擎(本地/云端)
  "duration_ms": 8200,             // 语音时长
  "location": null,                // 移动端可选粗粒度位置
  "processed": false,              // 是否已"整理/归纳"(收件箱状态)
  "draft": false                   // 是否为未提交草稿
}
```

### 4.2 JSON 示例

一条语音速记：

```jsonc
{
  "id": "01J7MUSEIDEA0042",
  "source": "muse",
  "kind": "voice",
  "title": null,
  "content": "傅里叶变换可以用音叉类比——每个音叉是一个频率，一起响就还原了原始声音。",
  "content_format": "plain",
  "blocks": [
    { "id": "01J...b1", "memory_id": "01J7MUSEIDEA0042", "seq": 0, "type": "transcript",
      "text": "傅里叶变换可以用音叉类比——每个音叉是一个频率，一起响就还原了原始声音。" }
  ],
  "media": [
    { "id": "01J...a1", "kind": "audio", "path": "media/2026/07/01J7MUSEIDEA0042.enc",
      "mime": "audio/ogg", "duration": 8.2,
      "transcript": "傅里叶变换可以用音叉类比...", "hash": "sha256:..." }
  ],
  "tags": ["灵感"],
  "links": [],
  "pinned": false,
  "archived": false,
  "created_at": "2026-07-11T13:58:00Z",
  "updated_at": "2026-07-11T13:58:00Z",
  "captured_at": "2026-07-11T13:58:00Z",
  "device_id": "iphone-01",
  "meta": {
    "capture_method": "voice",
    "asr_engine": "local:whisper",
    "duration_ms": 8200,
    "location": null,
    "processed": false,
    "draft": false
  }
}
```

> 这条速记正是 [apps/quill.md](quill.md) §4.2 示例中被 Quill 笔记以 `references` 引用的那条（`to_id=01J7MUSEIDEA0042`），展示了家族内联。

---

## 5. 技术实现

### 5.1 应用形态（极致轻量）

Muse 是一个 Tauri 2.0 应用（`apps/muse/`，见 [architecture.md](../architecture.md) §3），设计目标是**小、快、静默**：

- **前端**（`apps/muse/src`）：React + TypeScript + Vite，UI 用 `@nexus/ui`，编辑器用 `@nexus/editor` 的极简配置；界面只有浮层、收件箱、设置三块。
- **外壳**（`apps/muse/src-tauri`）：Rust，常驻托盘/后台，管理全局快捷键、无边框浮层窗口、录音；暴露 `quick_capture`、`start_recording`、`search` 等命令。
- **核心**：`src-tauri` 直接依赖 `nexus-core`，或作为 Protocol 客户端连本机记忆服务（见 [architecture.md](../architecture.md) §5.3）。

```
┌───────────────────────────────────────────┐
│ Muse 前端 (WebView) —— 极简                 │
│  无边框浮层(输入+麦克风) · 收件箱 · 设置     │
└──────────────┬──────────────────────────────┘
               │ Tauri IPC
┌──────────────▼──────────────────────────────┐
│ apps/muse/src-tauri (Rust, 常驻托盘)          │
│  命令: quick_capture / start_recording / search│
└───────┬───────────────────────────┬──────────┘
        │                           │
┌───────▼────────────┐   ┌───────────▼──────────────┐
│ platform            │   │ nexus-core                │
│  Hotkey             │   │  ingest(切块/嵌入/加密媒体)│
│  AudioRecorder      │   │  ai::Transcriber(语音转写) │
│  (常驻/后台)         │   │  search · events · crypto  │
└─────────────────────┘   └───────────────────────────┘
```

### 5.2 平台适配：`Hotkey` 与 `AudioRecorder`

Muse 依赖 `platform` 层的两组能力（trait 抽象，App 不关心 OS 差异，见 [architecture.md](../architecture.md) §5.1）：

```rust
pub trait Hotkey {
    fn register(&self, combo: KeyCombo, id: HotkeyId) -> Result<()>;
}
pub trait AudioRecorder {
    fn start(&self) -> Result<RecordingHandle>;
    fn stop(&self, h: RecordingHandle) -> Result<AudioRef>;
}
```

| 平台 | 热键 | 录音 | 要点 |
|------|------|------|------|
| Windows | RegisterHotKey | WASAPI | 常驻托盘，冷启动优化 |
| macOS | Carbon/CGEventTap | AVAudioEngine | 需麦克风权限，首启引导 |
| 移动端（二期） | 系统快捷方式/小组件 | 系统录音 API | **后台限制**是关键挑战，见 §6 |

### 5.3 依赖 `nexus-core` 的具体模块

| 能力 | 依赖模块 | 说明 |
|------|---------|------|
| 速记入库 | `ingest`（编排 `embed`+`store`+`sync`+`events`） | 文本切块嵌入；语音加密落盘 |
| 语音转写 | `ai::Transcriber` | 本地优先，可切云端 |
| 收件箱/速查 | `search`（Hybrid，`filters.source=["muse"]`） | 时间倒序 |
| 媒体加密 | `crypto`（音频分块加密 + hash） | 见 [nexus-core.md](../nexus-core.md) §6 |
| 实时联动 | `events`（`MemoryCreated`） | Orbit/Quill 实时可见 |

### 5.4 通过 Memory Protocol 写入

- 同机多 App 共享一个记忆库：Muse 或作为**持有者**、或作为**客户端**连本机记忆服务（loopback / IPC，见 [memory-protocol.md](../memory-protocol.md) §3）。
- Muse 的能力域限定为 `memory:write`（`source=muse`），另需 `search`、`memory:read`（见 [memory-protocol.md](../memory-protocol.md) §4.2）。
- 前端不直连数据库/密钥；录音、转写、加密全在 Rust 侧，前端只拿授权视图（见 [architecture.md](../architecture.md) §4）。

---

## 6. 关键难点与对策

| 难点 | 说明 | 对策 |
|------|------|------|
| 冷启动速度 | 「秒开」是核心气质，启动慢即失败 | 进程**常驻托盘/后台**，唤起=显示已就绪窗口；WebView 预热；浮层 UI 极简、无重依赖；目标 <200ms |
| 后台常驻内存 | 常驻不能占太多内存 | Tauri 本身轻；浮层按需创建/复用单窗口；空闲释放非必要资源；监控内存占用基线 |
| 全局热键冲突 | 与系统/其他软件撞键 | 注册失败时提示并引导改键；提供多套预设；冲突检测 |
| 语音本地转写延迟 | 转写慢影响「即走」体验 | 支持「先入库音频、后台补转写」；小模型/流式转写；转写完成再补 `transcript` 与向量 |
| 移动端后台限制 | iOS/Android 后台唤起与录音受限 | 二期用系统小组件/快捷方式/分享面板作为唤起入口；录音走系统 API；接受平台约束，见 [architecture.md](../architecture.md) §6 |
| 零摩擦 vs 结构化 | 记得太随意，事后难整理 | 「先接住，再整理」：收件箱 + 一键整理为 Quill / 送 Orbit 归纳，把结构化后移 |

---

## 7. 设置项

| 分类 | 设置 | 默认 |
|------|------|------|
| 快捷键 | 唤起快捷键、置顶速记板快捷键 | `Ctrl/Cmd+Shift+Space` |
| 提交行为 | 失焦是否自动提交、保留草稿、Esc 行为 | 失焦提交开，保留草稿开 |
| 语音 | 引擎（本地/云端）、语言、流式转写、先入库后转写 | 本地，先入库后转写 |
| 默认 | 默认 `kind`（idea/note）、默认标签 | idea |
| 收件箱 | 显示条数、整理提醒 | 最近 50 条 |
| 剪贴板 | 剪贴板监听（默认关闭） | 关 |
| 隐私 | 云端数据最小化提示、位置记录（移动端） | 提示开，位置关 |
| 同步 | 档位（纯本地 / E2E 云 / 自托管），见 [data-model.md](../data-model.md) §4 | 纯本地 |

---

## 8. 分阶段开发计划

| 阶段 | 范围 | 说明 |
|------|------|------|
| **M3 接入验证** | 单一文字输入 + 连接/授权状态 + Memory Protocol 写入 `source=muse` + Orbit 检索/事件回显 | 这是来源适配器和端到端测试样例，不作为 Muse MVP 发布；不做热键、托盘、语音、收件箱或移动端 |
| **M7 产品化一期** | 全局热键唤起浮层 + 文字速记 + 失焦/回车即存 + 入库（ingest） | 将 M3 样例升级为可独立使用的 Muse 桌面 MVP，覆盖 Win/Mac，并完成冷启动优化 |
| **M7 产品化二期** | 语音输入（本地 `Transcriber`）+ 收件箱 + 就地速查 | 补齐语音与“先接住，再整理”体验 |
| **M7 产品化三期** | 整理为 Quill 笔记 + 送 Orbit 归纳/复习 + 置顶速记板 | 家族内联联动；依赖 Quill 同期产品化能力 |
| **M7 后续增强** | 云端 ASR 可切 + 剪贴板监听 + 自动标签 + 移动端 | 落实隐私护栏，并按平台约束逐步交付 |

> M3 与 M7 之间不扩展 Muse 产品功能。里程碑与其他软件的排期见 [roadmap.md](../roadmap.md)。

---

## 9. 与各文档的关系

- 数据结构与隐私 → [data-model.md](../data-model.md)
- 核心引擎（ingest / ai::Transcriber / search / crypto）→ [nexus-core.md](../nexus-core.md)
- 热键/录音平台适配与代码组织 → [architecture.md](../architecture.md)
- 与记忆库交互的接口 → [memory-protocol.md](../memory-protocol.md)
- 速记的深化端 → [apps/quill.md](quill.md)；归纳与复习端 → [apps/orbit.md](orbit.md)
