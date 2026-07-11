# Quill · 笔记系统 —— 功能说明 + 开发文档

> 意象：羽毛笔，书写的象征。
> 一句话定位：**帮助学习、记忆、理解的智能 Markdown 编辑器。**

Quill 是 Nexus 产品家族的「捕获层」成员之一。它不只是让你记下东西，而是帮你把写下来的内容真正**学会、记住、理解**。你写的每一篇笔记都会被切块、嵌入、汇入中枢记忆库 Orbit，成为可检索、可关联、可复习的记忆。

- `source` = `quill`
- `kind` = `note`
- 平台：**Windows + macOS 一期交付**；移动端**以阅读为主**，后续跟进（见 [architecture.md](../architecture.md) §5.2 平台矩阵）。

---

## 1. 概述与定位

### 1.1 Quill 是什么

Quill 是一个「本地优先 + 端到端加密」的 Markdown 笔记应用，底层复用 `nexus-core`，数据完全遵循 Nexus 统一记忆模型 Memory（见 [data-model.md](../data-model.md)）。与普通笔记软件不同，Quill 的核心信念是：

> **写笔记的目的不是归档，而是学习、记忆与理解。**

因此 Quill 不停留在「所见即所得的编辑器」层面，而是通过双向链接、AI 辅助、块级引用、语义检索，把散落的笔记编织成一张会生长的知识网络；再借助与 Orbit 的协作，把「理解」沉淀为「长期记忆」。

### 1.2 它在家族中的位置

| 维度 | Quill 的职责 | 交给其他软件 |
|------|-------------|-------------|
| 写与理解 | Markdown 编辑、双链、块引用、AI 辅助理解 | —— |
| 速记/灵感 | 引用 Muse 的速记片段入笔记 | **Muse** 负责极速捕获 |
| 屏幕素材 | 引用 Echo 的截图/OCR 入笔记 | **Echo** 负责屏幕抓取 |
| 卡片与复习 | 提供「送去复习」入口，标记待生成卡片 | **Orbit** 负责知识卡片与间隔复习 |
| 检索与图谱 | 库内检索、关联推荐（读） | **Orbit** 提供跨源全局检索与消费视图 |

> **分工边界（贯穿全文）**：**Quill 负责「写与理解」，Orbit 负责「卡片与复习」。** Quill 把高质量、已切块的笔记喂给记忆库，Orbit 在此之上生成知识卡片、构建概念图谱、驱动间隔复习。二者通过 Memory Protocol 与 `derived_from` 关联协作，功能不重叠。

### 1.3 典型场景

- **整理课程笔记**：边听边记，课后用 AI 生成大纲、补全概念解释，标记重点送去 Orbit 复习。
- **写读书笔记**：摘录 + 批注，用双链把书中概念与既有笔记连起来，形成个人知识体系。
- **构建知识体系**：以「概念页」为节点，双向链接为边，逐步长出个人 wiki / Zettelkasten。
- **双链笔记（卡片盒笔记法）**：原子化笔记 + 密集互链，配合反向链接面板发现意外关联。

---

## 2. 核心功能详解

### 2.1 双模 Markdown 编辑（所见即所得 / 源码）

Quill 复用共享编辑器包 `@nexus/editor`（见 [architecture.md](../architecture.md) §3），提供两种可切换的书写模式：

| 模式 | 引擎 | 适用 |
|------|------|------|
| 所见即所得（WYSIWYG） | **TipTap**（ProseMirror） | 富文本手感、表格、拖拽块、斜杠命令 |
| 源码 / 混合 | **CodeMirror 6** | 纯 Markdown 手写、代码块高亮、Vim 键位、大文档性能 |

- 两种模式共享同一份 Markdown 源（`content_format = markdown`），切换不丢信息。
- 支持标准 Markdown + 常用扩展：表格、任务列表、脚注、数学公式（KaTeX）、代码高亮、图片粘贴。
- 斜杠命令（`/`）与块菜单用于快速插入标题、代码块、引用、双链、Echo/Muse 引用等。
- **底线：内容始终以 Markdown 文本为唯一真相（source of truth）**，块模型（Block）是它的派生视图，保证「数据可带走」（见 §4、§7.2）。

### 2.2 双向链接与反向链接

双链是 Quill 构建知识体系的核心。它直接建立在 Memory 的 `links` 之上（见 [data-model.md](../data-model.md) §2.4）。

- **正向链接**：编辑时输入 `[[` 触发笔记检索，选中目标笔记即插入一个链接。保存时，Quill 在两条 Memory 间写入一条 `Link`：

  | 字段 | 值 |
  |------|-----|
  | `from_id` | 当前笔记的 Memory id |
  | `to_id` | 目标笔记的 Memory id |
  | `relation` | `references` |
  | `created_by` | `user` |

- **反向链接（Backlinks）**：任意笔记打开时，Quill 反查所有 `to_id = 当前笔记` 且 `relation = references` 的 Link，在「反向链接」面板列出「谁引用了我」，并展示引用处的上下文块（block 级）。
- **未链接提及（Unlinked mentions）**：通过全文/语义检索发现正文里出现了某笔记标题却未建链的地方，一键补链。
- **块级链接**：双链可精确到块（`[[笔记标题#块]]`），落地为指向具体 `block_id` 的引用（见 §2.4）。

> 双链只承载「用户在 Quill 内主动建立的引用」。跨源的自动关联（如某截图与某笔记语义相近）由 `search` 的「相关记忆」推荐给出（`relation=related`，`created_by=ai/system`），不与用户双链混淆。

### 2.3 AI 辅助（调 `Completion` trait）

Quill 的 AI 能力统一通过 `nexus-core` 的 `ai::Completion` trait 调用（见 [nexus-core.md](../nexus-core.md) §8），本地/云端可切，均遵守**数据最小化**与**隐私提示**（见 [data-model.md](../data-model.md) §5.5）。

| 能力 | 说明 | 输入（最小化） | 产出去向 |
|------|------|---------------|---------|
| 总结 | 为长笔记/选区生成摘要 | 当前笔记或选中块的文本 | 插入为引用块，标注 AI 来源 |
| 追问 | 就选中内容提问、展开讨论 | 选区 + 用户问题 | 侧栏对话，可回填为笔记 |
| 生成大纲 | 从零散内容整理层级大纲 | 当前笔记正文 | 插入为标题层级骨架 |
| 解释概念 | 解释选中术语/段落 | 选区（必要时 + 库内相关块） | 侧栏或行内注解 |
| 辅助记忆 | 提炼要点、生成「问答对」候选 | 选中块 | **候选卡片**，送 Orbit 生成正式卡片（见 §6） |

关键约束：

- **数据最小化**：只发送完成任务所必需的最小文本（当前笔记 / 选区 / 检索到的相关块），**绝不发送整库**。
- **隐私提示**：使用云端 Provider 前，UI 明示「本次操作将把这段内容发送到 `<Provider>`」，用户可改用本地模型或取消。
- **引用可信**：涉及「基于我的笔记回答」的场景走库内 RAG（`search` 取材 → `Completion` 生成），产出附带 `block_id` 引用，可回跳原文（见 §3.2、§7.4）。

### 2.4 块级引用（Block Reference / Transclusion）

- 允许把某条笔记的**某个块**嵌入到另一条笔记里（内容嵌入 / transclusion），或仅做锚点引用。
- 基于 Block（见 [data-model.md](../data-model.md) §2.2）：每个块有稳定 `id`，引用即记录目标 `block_id`。
- 渲染时按 `block_id` 实时取源块内容展示，源块更新，引用处随之更新（单一真相）。
- 块引用同样落一条 `Link`（`relation = references`，`meta` 记录 `block_id`），保证可反查、可校验完整性（见 §7.3）。

### 2.5 标签与集合（Collection）

- **标签（tags）**：写在正文（`#tag`）或属性区，映射到 Memory 的 `tags[]`，用于轻量分类与检索过滤。
- **集合（Collection）**：对应 [data-model.md](../data-model.md) §2.6 的 `Collection`（可嵌套、一条 Memory 可属于多个集合）。Quill 用集合表达「笔记本 / 课程 / 项目 / 知识库」。
- 集合管理通过 Protocol 的 `GET /v1/collections` 与 store 的集合仓储完成（见 [memory-protocol.md](../memory-protocol.md) §5.3）。

### 2.6 全文 + 语义检索

- 直接使用 `nexus-core` 的 `search`（混合检索：向量 + FTS5 + RRF 融合，见 [nexus-core.md](../nexus-core.md) §4）。
- Quill 内检索默认 `filters.source = ["quill"]`，也可放开到全库（含 Echo/Muse）。
- 支持 `mode = hybrid / semantic / keyword`，命中回溯到块级，高亮片段。
- **关联推荐**：编辑时，基于当前笔记向量在库内找「相关记忆」，在侧栏推荐可能想链接的笔记/截图/速记。

### 2.7 与 Echo / Muse 内容互引

Quill 是「消费素材、产出理解」的一端。它可以把 Echo 的截图、Muse 的速记引入笔记：

- **插入引用**：在编辑器中通过「引用记忆」面板检索全库（`search`，可按 `source=echo/muse` 过滤），选中后插入。
- **关联落地**：插入 Echo 截图或 Muse 速记时，写一条 `Link`：`from = 当前 Quill 笔记`，`to = 被引记忆`，`relation = references`。
- **渲染**：Echo 引用显示缩略图 + OCR 文本片段；Muse 引用显示速记/转写片段；点击可跳转查看原记忆。

> 边界：Quill **不生产**截图或语音（那是 Echo/Muse 的职责），只**引用**它们，避免功能重叠。

---

## 3. 交互流程

### 3.1 编辑 → 自动保存 → 切块嵌入 → 入库

Quill 前端不直接碰数据库；所有写入经 Tauri 命令进入 `nexus-core` 的 `ingest` 管线（见 [nexus-core.md](../nexus-core.md) §3、[architecture.md](../architecture.md) §4）。

```
用户编辑 (WebView: TipTap/CodeMirror)
   │  本地即时渲染，无阻塞
   ▼
防抖自动保存 (debounce, 约 1–2s 空闲 / 失焦 / 手动 Ctrl+S)
   │  前端拿到 Markdown 文本
   ▼
Tauri IPC 命令  save_note(id?, markdown, tags, collection_ids)
   ▼
nexus-core::ingest.ingest(IngestInput { source: quill, kind: note, ... })
   │  规范化(已是 Markdown) → 切块(按标题/段落/代码) → 去重 → 嵌入(每块向量)
   │  → 落库(memory + blocks + block_vectors + fts) → 记账(sync oplog)
   ▼
events: MemoryCreated / MemoryUpdated
   │  广播
   ▼
Orbit 等订阅者实时收到 → 可立即检索、可评估是否生成卡片
```

要点：

- **切块策略**：笔记 `kind=note` 按「标题层级 / 段落 / 代码块」切分（见 [nexus-core.md](../nexus-core.md) §5 embed）。每块生成 768 维向量存入 `block_vectors`。
- **增量保存**：更新走 `store.update` + 局部重切块重嵌入（仅变更块），大文档避免全量重算（见 §7.1）。
- **首次保存**：若无 `title`，由正文首行或 AI 生成（见 [data-model.md](../data-model.md) §2.1）。

### 3.2 AI 辅助的调用与引用回填流程

以「解释概念 / 基于我的笔记追问」为例，展示 RAG + 引用回填：

```
用户选中文本 / 提问
   ▼
Quill 组装最小上下文
   ├─ 需要库内知识？→ search(SearchQuery{ text, filters.source=[quill], limit }) 取相关块
   └─ 仅就选区？   → 只带选区文本
   ▼
数据最小化 + 隐私提示 (若云端 Provider: 明示将发送的文本与目标 Provider)
   ▼
ai::Completion.complete/stream(CompletionReq{ prompt, context_blocks })
   ▼
流式返回答案 (stream) + citations[{ memory_id, block_id }]
   ▼
引用回填：
   ├─ 答案渲染时把 citation 显示为可点击角标 → 跳转对应 block
   └─ 用户「采纳」答案 → 作为引用块插入笔记，标注 created_by=ai
        并（可选）写 Link(from=本笔记, to=被引记忆, relation=references, created_by=ai)
```

- 与 Protocol 的 `POST /v1/ask` 语义一致（见 [memory-protocol.md](../memory-protocol.md) §5.4）：本地检索取材、按设置选模型、返回带引用的答案。
- AI 生成内容始终**可溯源、可标注**，避免「幻觉当事实」（见 §7.4）。

---

## 4. 数据模型映射

Quill 的每篇笔记就是一条 Memory，完全遵循 [data-model.md](../data-model.md)。字段约定：

| 字段 | Quill 取值 | 说明 |
|------|-----------|------|
| `source` | `quill` | 固定 |
| `kind` | `note` | 固定 |
| `title` | 用户标题 / 正文首行 / AI 生成 | 可选 |
| `content` | Markdown 文本 | **唯一真相** |
| `content_format` | `markdown` | 固定 |
| `blocks` | 按标题/段落/代码切分 | 由 ingest 生成，块级向量与引用的最小单位 |
| `tags` | 正文 `#tag` / 属性区 | 映射到 `tags[]` |
| `links` | 双链 + 块引用 + 跨源引用 | `relation` 见下 |
| `review` | 通常为空 | **复习状态由 Orbit 写入/管理**，Quill 不主动生成 |
| `pinned` / `archived` | 置顶 / 归档 | |
| `meta` | Quill 特有扩展 | 见下 |

Link 的 `relation` 在 Quill 中的用法：

| relation | 场景 | created_by |
|----------|------|-----------|
| `references` | 双链、块引用、引用 Echo/Muse | `user`（手动）/ `ai`（AI 采纳时） |
| `related` | 语义相关推荐（非用户主动建链） | `ai` / `system` |
| `derived_from` | **Orbit 由本笔记派生卡片时写入**（卡片 `derived_from` 笔记） | `ai` / `system` |

> `derived_from` 由 Orbit 侧在生成卡片时写入，方向为「卡片 → 笔记」；Quill 只读它来展示「这篇笔记衍生了哪些卡片」。

### 4.1 `meta` 约定（Quill 特有）

```jsonc
"meta": {
  "editor_mode": "wysiwyg",        // wysiwyg | source
  "outline_generated_by": "ai",    // 大纲是否 AI 生成
  "word_count": 862,
  "block_ref_targets": ["01J...blockA"],  // 本笔记引用到的块（冗余索引，便于完整性校验）
  "cursor": { "block_seq": 12, "offset": 34 }  // 上次编辑位置，便于恢复
}
```

### 4.2 JSON 示例

一篇引用了另一篇笔记、并插入了一段 Muse 速记的课程笔记：

```jsonc
{
  "id": "01J8QUILLNOTE0001",
  "source": "quill",
  "kind": "note",
  "title": "傅里叶变换 · 直觉理解",
  "content": "# 傅里叶变换\n\n把信号从时域拆成不同频率的正弦叠加。\n\n## 核心直觉\n任何周期信号都能表示为 [[正弦基]] 的加权和。\n\n> 引用速记：昨天想到的类比——像用不同音叉还原一段声音。\n\n## 代码验证\n```python\nnp.fft.fft(signal)\n```\n",
  "content_format": "markdown",
  "blocks": [
    { "id": "01J...b1", "memory_id": "01J8QUILLNOTE0001", "seq": 0, "type": "heading",   "text": "傅里叶变换" },
    { "id": "01J...b2", "memory_id": "01J8QUILLNOTE0001", "seq": 1, "type": "paragraph", "text": "把信号从时域拆成不同频率的正弦叠加。" },
    { "id": "01J...b3", "memory_id": "01J8QUILLNOTE0001", "seq": 2, "type": "heading",   "text": "核心直觉" },
    { "id": "01J...b4", "memory_id": "01J8QUILLNOTE0001", "seq": 3, "type": "paragraph", "text": "任何周期信号都能表示为 正弦基 的加权和。" },
    { "id": "01J...b5", "memory_id": "01J8QUILLNOTE0001", "seq": 4, "type": "quote",     "text": "引用速记：昨天想到的类比——像用不同音叉还原一段声音。" },
    { "id": "01J...b6", "memory_id": "01J8QUILLNOTE0001", "seq": 5, "type": "heading",   "text": "代码验证" },
    { "id": "01J...b7", "memory_id": "01J8QUILLNOTE0001", "seq": 6, "type": "code",      "text": "np.fft.fft(signal)" }
  ],
  "tags": ["数学", "信号处理", "课程"],
  "links": [
    { "from_id": "01J8QUILLNOTE0001", "to_id": "01J8QUILLNOTE0009", "relation": "references", "created_by": "user" },
    { "from_id": "01J8QUILLNOTE0001", "to_id": "01J7MUSEIDEA0042", "relation": "references", "created_by": "user" }
  ],
  "pinned": false,
  "archived": false,
  "created_at": "2026-07-10T09:12:00Z",
  "updated_at": "2026-07-11T14:03:00Z",
  "device_id": "mac-studio-01",
  "meta": {
    "editor_mode": "wysiwyg",
    "word_count": 128,
    "block_ref_targets": [],
    "cursor": { "block_seq": 6, "offset": 12 }
  }
}
```

对应的「Orbit 由这篇笔记派生的卡片」（**由 Orbit 写入，此处仅示意关联方向**）：

```jsonc
{
  "id": "01J9ORBITCARD0007",
  "source": "orbit",
  "kind": "card",
  "links": [
    { "from_id": "01J9ORBITCARD0007", "to_id": "01J8QUILLNOTE0001",
      "relation": "derived_from", "created_by": "ai" }
  ],
  "review": { "state": "new", "due_at": "2026-07-12T00:00:00Z" }
}
```

---

## 5. 技术实现

### 5.1 应用形态

Quill 是一个 Tauri 2.0 应用（`apps/quill/`，见 [architecture.md](../architecture.md) §3）：

- **前端**（`apps/quill/src`）：React + TypeScript + Vite，UI 用 `@nexus/ui`，编辑器复用 `@nexus/editor`（TipTap + CodeMirror 6）。
- **外壳**（`apps/quill/src-tauri`）：Rust，装配窗口/菜单/快捷键，暴露 `save_note`、`open_note`、`link`、`ask` 等 Tauri 命令。
- **核心**：`src-tauri` 直接依赖 `nexus-core`（进程内调用），或作为 Protocol 客户端连本机记忆服务（见 [architecture.md](../architecture.md) §5.3 单实例仲裁）。

```
┌───────────────────────────────────────────┐
│ Quill 前端 (WebView)                        │
│  @nexus/editor (TipTap / CodeMirror 6)     │
│  双链面板 · 反向链接 · AI 侧栏 · 检索        │
└──────────────┬──────────────────────────────┘
               │ Tauri IPC
┌──────────────▼──────────────────────────────┐
│ apps/quill/src-tauri (Rust)                  │
│  命令: save_note / open_note / link / ask    │
└──────────────┬──────────────────────────────┘
               │
┌──────────────▼──────────────────────────────┐
│ nexus-core                                   │
│  ingest(保存即切块嵌入) · search(检索/推荐)  │
│  ai::Completion(总结/追问/大纲/解释/记忆)     │
│  store · events · crypto · sync              │
└──────────────────────────────────────────────┘
```

### 5.2 依赖 `nexus-core` 的具体模块

| 能力 | 依赖模块 | 说明 |
|------|---------|------|
| 保存即切块嵌入 | `ingest`（编排 `embed`+`store`+`sync`+`events`） | 见 [nexus-core.md](../nexus-core.md) §3 |
| 库内检索 / 关联推荐 | `search`（Hybrid，`filters.source` 控制范围） | 见 §4 |
| AI 辅助 | `ai::Completion`（`complete` / `stream`） | 云/本地可切，数据最小化 |
| 双链 / 块引用落地 | `store.link` / Protocol `POST /v1/links` | `relation=references` |
| 集合管理 | store 集合仓储 / `GET /v1/collections` | Collection |
| 实时刷新 | `events`（`MemoryCreated/Updated`） | 与 Orbit 联动 |

### 5.3 通过 Memory Protocol 与记忆库交互

- 同机多 App 共享一个记忆库：Quill 或作为**持有者**，或作为**客户端**连接本机记忆服务（loopback / IPC），见 [memory-protocol.md](../memory-protocol.md) §3。
- Quill 的写入能力域限定为 `memory:write`（`source=quill`），另需 `search`、`memory:read`、`subscribe`（见 [memory-protocol.md](../memory-protocol.md) §4.2）。
- 前端**不直连数据库/密钥**，敏感操作全部经 Rust 命令，前端只拿解密后的授权视图（见 [architecture.md](../architecture.md) §4）。

### 5.4 AI Provider 的云/本地切换

- 嵌入默认走本地小模型（bge-small/gte，768 维 ONNX）；总结/问答默认云端大模型，用户可自带 Key 或切本地 LLM（见 [architecture.md](../architecture.md) §2.5、[nexus-core.md](../nexus-core.md) §8）。
- 离线自动回落本地；云端不可用降级；云端调用前统一走**数据最小化 + 隐私提示**护栏。

---

## 6. 「学习 · 记忆 · 理解」的智能设计

这是 Quill 区别于普通编辑器的核心。它的策略是：**Quill 把「理解」做深，把「记忆」交给 Orbit**，二者通过记忆库与 `derived_from` 关联协作。

### 6.1 分工边界（再次明确）

| 环节 | 归属 | 产物 |
|------|------|------|
| 写、组织、理解笔记 | **Quill** | 高质量、已切块、已互链的 `note` |
| 从笔记生成知识卡片 | **Orbit** | `kind=card`，`derived_from` 笔记 |
| 概念图谱构建与展示 | **Orbit**（数据源含 Quill 的 links/blocks） | 图谱视图 |
| 间隔复习调度与执行 | **Orbit** | `ReviewState`（FSRS/SM-2） |
| **在笔记里的复习入口** | **Quill**（仅入口，动作交 Orbit） | 「送去复习」标记 |

> Quill **不实现**间隔重复算法，也**不写** `ReviewState`；它只提供「把这段送去 Orbit 生成卡片/复习」的入口，实际调度归 Orbit。

### 6.2 把笔记喂给 Orbit 生成知识卡片

流程（跨 App，经记忆库与事件，无需导入导出）：

```
Quill: 用户选中要点 → 点「辅助记忆 / 送去复习」
   ├─(可选) 调 Completion 生成「问答对」候选(card_front/card_back)
   └─ 标记该块/笔记为「候选卡片」(写入 meta 或产生一个待处理事件)
        ▼
记忆库: 笔记 Memory 已在库中(blocks 已切好)
        ▼
Orbit: 订阅/扫描到候选 → 生成 kind=card 的 Memory
        └─ 写 Link(from=card, to=note, relation=derived_from, created_by=ai)
        └─ 初始化 ReviewState(state=new, due_at=...)
        ▼
Quill: 反查 to_id=本笔记 & relation=derived_from → 展示「本笔记衍生的卡片」
```

- **`derived_from` 是协作契约**：卡片始终能溯源回原笔记；笔记更新时 Orbit 可据此提示卡片是否需要重生成。
- Quill 侧只需产出「候选」并保证笔记块质量；卡片文本的最终生成、去重、排期归 Orbit。

### 6.3 概念图谱

- 图谱的**边**来自 Quill 的 `links`（双链 `references`、块引用）与 `search` 的语义近邻（`related`）。
- 图谱的**节点**是 Memory（尤其是「概念页」笔记）。
- **构建与可视化在 Orbit**（跨源全局视角）；Quill 侧提供「局部关系」小图（当前笔记的一跳邻居），作为写作时的导航辅助，不与 Orbit 的全局图谱重复。

### 6.4 间隔复习入口

- Quill 在笔记/块的上下文菜单提供「送去复习」；点击后仅产生候选（见 6.2），不涉及调度。
- 到期提醒、复习卡片界面、评分回写 `ReviewState` 全部在 Orbit（订阅 `review.due` 事件，见 [memory-protocol.md](../memory-protocol.md) §5.5）。
- Quill 可选展示一个只读的「本笔记相关复习状态」小徽标（读 `review`），方便用户回看，但不提供复习操作。

---

## 7. 关键难点与对策

### 7.1 大文档编辑性能

| 问题 | 对策 |
|------|------|
| 万字长文实时渲染卡顿 | 大文档默认走 CodeMirror 6（虚拟滚动、按视口渲染）；WYSIWYG 模式对超长文档提示切源码模式 |
| 每次保存全量重切块/重嵌入昂贵 | **增量切块**：diff 出变更块，仅对变更块重嵌入；未变块复用旧向量 |
| 自动保存与输入争抢 | 防抖 + 后台异步 ingest；嵌入在 Rust 侧线程池，不阻塞 UI |
| 大量图片 | 媒体不入库，落盘为加密文件，正文只存引用（见 [data-model.md](../data-model.md) §3） |

### 7.2 Markdown 与 Block 模型的双向映射

- **单一真相是 Markdown 文本**；Block 是 ingest 的派生产物（切块规则见 [nexus-core.md](../nexus-core.md) §5）。
- 编辑器编辑的是 Markdown（或其 AST），保存时交给 `ingest` 重新切块，**不要求前端与后端切块逻辑各写一份**——切块只以 `nexus-core` 为准，避免两套规则漂移。
- 块 `id` 稳定性：增量更新时通过内容/位置匹配尽量复用既有块 `id`，以维持块引用有效（见 7.3）。
- 导出即回写 Markdown + front-matter，保证数据可带走（见 [data-model.md](../data-model.md) §6）。

### 7.3 双链与块引用完整性

| 风险 | 对策 |
|------|------|
| 目标笔记被删 | 删除走 tombstone 级联；反查引用方，将失效链接标记为「悬空」并提示修复 |
| 目标块被编辑掉/合并 | 块 `id` 尽量复用；确实消失时，块引用降级为指向所属 Memory 并提示 |
| 双链不一致 | `meta.block_ref_targets` 冗余索引 + 后台校验任务，定期核对 links 与实际块 |
| 重命名笔记标题 | 双链存 `to_id`（而非标题字符串），重命名不断链；显示名实时取目标 title |

### 7.4 AI 生成内容的引用可信

- **可溯源**：库内问答走 RAG，答案附 `citations[{memory_id, block_id}]`，可回跳原文（见 §3.2、[memory-protocol.md](../memory-protocol.md) §5.4）。
- **可标注**：AI 采纳进笔记的内容标记 `created_by=ai`，视觉上与用户原文区分。
- **可校验**：鼓励「基于我的笔记」而非「凭空生成」；无引用来源的生成内容给出明确提示，降低幻觉被当作事实的风险。
- **最小化**：所有云端调用遵守数据最小化与显式提示（见 [data-model.md](../data-model.md) §5.5）。

---

## 8. 设置项与分阶段计划

### 8.1 设置项

| 分类 | 设置 | 默认 |
|------|------|------|
| 编辑器 | 默认模式（WYSIWYG / 源码）、字体、Vim 键位、自动保存间隔 | WYSIWYG，1.5s |
| AI | Provider（本地/云端）、模型、自带 Key、云端调用前确认 | 云端总结 + 确认开 |
| 隐私 | 云端数据最小化提示、离线模式、可发送范围 | 提示开 |
| 检索 | 默认检索范围（仅 quill / 全库）、默认模式（hybrid） | 仅 quill，hybrid |
| 双链 | 未链接提及提示、悬空链接检查频率 | 开 |
| 复习 | 「送去复习」默认集合、是否显示复习徽标 | 开 |
| 同步 | 档位（纯本地 / E2E 云 / 自托管），见 [data-model.md](../data-model.md) §4 | 纯本地 |

### 8.2 分阶段计划

| 阶段 | 范围 | 说明 |
|------|------|------|
| **MVP** | Markdown 双模编辑 + 保存即切块入库（ingest）+ 全文/语义检索 + 标签/集合 | 打通「写 → 入库 → 检索」核心闭环，Win/Mac |
| **二期** | 双向链接 + 反向链接 + 块级引用 + 未链接提及 | 知识网络成形 |
| **三期** | AI 辅助（总结/追问/大纲/解释/辅助记忆）+ RAG 引用回填 + Echo/Muse 互引 | 接入 `Completion`，落实数据最小化 |
| **四期** | 与 Orbit 协作：候选卡片（`derived_from`）、局部关系小图、复习入口 | 分工协作，卡片/复习在 Orbit |
| **五期** | 移动端**阅读为主**（浏览、检索、反链查看，轻量编辑跟进） | 见平台矩阵 |

> 里程碑与其他软件的排期见 [roadmap.md](../roadmap.md)。

---

## 9. 与各文档的关系

- 数据结构与隐私 → [data-model.md](../data-model.md)
- 核心引擎（ingest / search / ai::Completion / embed）→ [nexus-core.md](../nexus-core.md)
- 编辑器选型与代码组织 → [architecture.md](../architecture.md)
- 与记忆库交互的接口 → [memory-protocol.md](../memory-protocol.md)
- 卡片与复习的另一端 → `apps/orbit.md`

