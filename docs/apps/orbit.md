# Orbit · 智能复习 / 中枢记忆库 —— 功能说明 + 开发文档

> 意象：轨道，知识不断环绕回来。
> 一句话定位：**中枢记忆库 + 知识卡片 + 间隔复习的第二大脑。**

Orbit 是 Nexus 产品家族的「中枢 + 消费层」。它不是一个普通的复习工具，而**就是中枢记忆库本体**——你的第二大脑。它把散落各处的记忆聚合起来，让你随时检索、问答、生成知识卡片、做间隔复习，让知识像行星一样不断环绕回归、不被遗忘。

- `source` = `orbit`（如 Orbit 生成的知识卡片）
- `kind` 主要生产 `card`（知识卡片）；同时**消费**所有 `kind` 的 Memory
- 平台：**Windows + macOS + iOS + Android**，**移动端为硬需求**（见 [architecture.md](../architecture.md) §5.2 平台矩阵）；Linux 放缓

> **贯穿全文的三个核心定位（务必牢记）**
> 1. **它是第二大脑本体**，不只是复习软件——检索、问答、卡片、复习都围绕「让知识环绕回来」。
> 2. **它是开放中枢，不是 Echo/Muse/Quill 的私有大脑**。前三款只是它的第一批「一等公民」客户端；任何有信息记录需求的应用都能通过 Memory Protocol 接入。
> 3. **移动端必不可少**。第二大脑必须随身，晨间地铁上的复习、随口一问「我之前记过什么」，都发生在手机上。

---

## 1. 概述与定位

### 1.1 Orbit 是什么

Orbit 是一个「本地优先 + 端到端加密」的中枢记忆库应用，底层深度复用 `nexus-core`，数据完全遵循 Nexus 统一记忆模型 Memory（见 [data-model.md](../data-model.md)）。它有三重身份：

| 身份 | 含义 |
|------|------|
| **中枢记忆库本体** | 它就是第二大脑——所有来源的 Memory 在此汇聚、被理解、被复习 |
| **智能整合检索中心** | 混合检索、语义搜索、时间线、主题聚类、记忆问答（RAG） |
| **连接与隐私管理中心** | 管理已授权应用、scope、令牌、同步档位、E2E 密钥与导出 |

与「一个记事本 + 一个 Anki」的组合不同，Orbit 的核心信念是：

> **记录只是开始，让知识环绕回来才是目的。** 记下的东西如果不能被检索、关联、复习，就等于遗忘。

### 1.2 开放中枢，而非私有大脑

这是 Orbit 最关键的定位，也是它区别于一切「笔记 + 复习」产品的地方：

> **Orbit 不是 Echo/Muse/Quill 三者的私有大脑，而是一个开放中枢。** Echo/Muse/Quill 只是它的第一批「一等公民」客户端。任何有信息记录需求的应用——第三方 App、脚本、浏览器扩展、AI 助手——都能通过 Memory Protocol 接入。

因此 Orbit 天生要同时做两件事：

```
                      ┌──────────────────────────────┐
                      │            Orbit             │
                      │   中枢记忆库 / 第二大脑       │
                      └───┬───────────────────────┬──┘
             内联 (inline) │                       │ 外联 (external)
        ┌──────────────────┘                       └───────────────────┐
        ▼                                                               ▼
  ┌───────────┐ ┌───────────┐ ┌───────────┐   ┌──────────────┐ ┌─────────────────┐
  │   Echo    │ │   Muse    │ │   Quill   │   │ 第三方 App/脚本│ │ AI 助手(MCP)/扩展 │
  │ 屏幕记忆  │ │ 灵感捕捉  │ │ 笔记系统  │   │ 剪藏/自动化   │ │ Claude / 浏览器   │
  └───────────┘ └───────────┘ └───────────┘   └──────────────┘ └─────────────────┘
      一等公民客户端（第一批）                       任意「有记录需求的大脑」
```

- **内联**：与 Echo/Muse/Quill 打通，抓的、记的、写的立刻在 Orbit 可检索、可复习（依赖本地服务与 [memory-protocol.md](../memory-protocol.md) §3 单实例仲裁）。
- **外联**：通过 Memory Protocol 对任意应用/AI 开放读写与检索能力（§6 详述架构）。

### 1.3 移动端为何必不可少

第二大脑的价值在于「随身」。Orbit 的许多核心场景天然发生在手机上：

- **晨间复习卡片**：地铁、排队、睡前的碎片时间，是间隔复习的黄金窗口。到期卡片（`review.due` 事件）在手机上推送并完成评分。
- **随时问「我之前记过什么」**：灵光一现或与人交谈时，掏出手机就能对第二大脑发问（`POST /v1/ask`）。
- **随手查阅与关联**：想不起某个概念，随时检索跨源记忆。

因此在平台矩阵中，Orbit 的 iOS/Android 与桌面**同为一期硬需求**，而非「后续跟进」。这也决定了工程上必须**优先验证移动端 Tauri 关键路径**（见 §7）。

### 1.4 它在家族中的位置

| 维度 | Orbit 的职责 | 交给其他软件 |
|------|-------------|-------------|
| 汇聚与消费 | 聚合所有来源的 Memory，统一检索/问答/复习 | Echo/Muse/Quill 负责各自场景的**捕获** |
| 知识卡片 | 从任意 Memory 生成卡片（`kind=card`，`derived_from` 关联） | Quill 提供「送去复习」入口标记素材 |
| 间隔复习 | 基于 ReviewState/FSRS 驱动复习队列、评分、到期提醒 | —— |
| 关联与图谱 | 相关推荐、去重合并、自动关联、知识图谱 | 捕获层只负责写入原子记忆 |
| 连接与隐私 | 授权/scope/令牌/同步/密钥的集中管理 | 由 Orbit 独家承担（`admin` scope） |
| 本地服务 | 可作为本地记忆服务**持有者**（单实例仲裁） | 任一 App 或 `nexus-daemon` 均可持有 |

> **分工边界**：捕获层（Echo/Muse/Quill）负责「把信息喂进来」，Orbit 负责「让信息环绕回来」。二者通过 Memory Protocol 与 `derived_from`/`related` 关联协作，功能不重叠。

### 1.5 典型场景

- **晨间复习卡片**：早上通勤，手机推送「今天有 24 张卡片到期」，逐张回忆、翻面、评分（Again/Hard/Good/Easy），FSRS 自动排下次到期。
- **随时问「我之前记过什么」**：「我上周关于定价的想法是什么？」Orbit 本地检索取材，调用 Completion 生成带引用的答案。
- **把散落各处的记忆聚合**：搜索「量子计算」，一次命中 Echo 抓的论文截图、Muse 的语音灵感、Quill 的读书笔记、以及浏览器扩展剪藏的网页——它们本是散落各处，如今在轨道上一并环绕回来。
- **一条笔记变成长期记忆**：在 Quill 写完一篇概念笔记，标记「送去复习」，Orbit 生成正反面卡片，从此纳入间隔复习。

---

## 2. 核心功能详解

Orbit 的能力全部构建在 `nexus-core` 之上。下表先给出「功能 → 依赖模块」的总览，随后逐项展开。

| 功能 | 主要依赖（nexus-core / protocol） |
|------|-----------------------------------|
| 统一检索中心 | `search`（Hybrid/Semantic）、`store` |
| 记忆问答（RAG） | `search` 取材 + `ai::Completion`，对外 `POST /v1/ask` |
| 知识卡片生成 | `ai::Completion` 生成正反面、`store.link`（`derived_from`） |
| 间隔复习 | `model::ReviewState` + FSRS、`events`（`review.due`） |
| 记忆整合与关联发现 | `search`（向量邻域）、`store.link`、去重（`ingest` 内容哈希） |
| 连接与隐私管理 | `nexus-protocol`（令牌/scope）、`crypto`、`sync` |
| 本地记忆服务持有者 | `nexus-protocol` 本地服务 + 单实例仲裁 |

### 2.1 统一检索中心

Orbit 的检索直接调用 `nexus-core` 的 `search` 模块（见 [nexus-core.md](../nexus-core.md) §4），对外亦即 Memory Protocol 的 `POST /v1/search`。

- **混合检索（Hybrid，默认）**：向量语义路（sqlite-vec 近邻）+ 关键词路（FTS5 BM25），用 RRF 融合，块级命中回溯到 Memory 去重聚合并高亮。
- **纯语义检索（Semantic）**：适合「意思对但词不对」的模糊回忆。
- **过滤（Filters）**：按 `source` / `kind` / `tags` / `collection` / 时间范围收窄，例如「只在 Quill 笔记里找」或「只看本月」。
- **时间线视图**：按 `created_at` / `captured_at` 排布，回放「我这段时间在关注什么」。
- **主题聚类**：基于向量邻域做无监督聚类，把一堆散记忆归拢为若干主题簇。

```rust
// 概念示意：Orbit 的检索页直接构造 SearchQuery 交给 nexus-core
let hits = core.search(SearchQuery {
    text: "量子计算的应用".into(),
    mode: SearchMode::Hybrid,
    filters: Filters { source: None, tags: vec!["reading".into()], ..Default::default() },
    limit: 20,
}).await?;
// hits: Vec<SearchHit> { memory_id, block_id, score, snippet }
```

> **隐私边界**：检索发生在**本地明文库**上（见 [data-model.md](../data-model.md) §4）。云端只做加密块中继，永远拿不到明文，也不参与检索。E2E 与「能搜到」并不矛盾。

### 2.2 记忆问答（RAG）

对第二大脑「发问」，而非「翻找」。对应 Memory Protocol 的 `POST /v1/ask`（见 [memory-protocol.md](../memory-protocol.md) §5.4）。

```http
POST /v1/ask
{ "question": "我上周关于定价的想法是什么？", "scope": { "collection": "ideas" } }
→ 200 {
    "answer": "你上周提出按用量分档定价……",
    "citations": [ { "memory_id": "01J...", "block_id": "b12" }, ... ]
  }
```

流程：`search` 本地检索取材 → 拼装上下文 → 按用户设置调用 `ai::Completion`（本地 LLM 或云端 Claude）→ 返回**带引用**的答案。要点：

- **必带引用**：每条结论回链到具体 Memory/Block，可点开溯源，杜绝「AI 瞎编」。
- **数据最小化**：走云端 Completion 时只发送检索命中的必要片段，不发整库，并在 UI 明示「本次将把这段内容发送到 <Provider>」（见 data-model.md §5.5）。
- **可流式**：长答案用 `Completion::stream` 逐 token 呈现。
- **scope 收窄**：可限定在某集合/来源内问答，提升相关性与隐私可控性。

### 2.3 知识卡片生成

Orbit 能从**任意 Memory** 生成知识卡片——无论它来自 Echo 截图、Muse 语音还是第三方剪藏。

- **生成的卡片本身也是一条 Memory**：`source=orbit`、`kind=card`。
- **溯源关联**：卡片通过 `Link { relation: derived_from }` 指向原始 Memory（见 data-model.md §2.4）。
- **AI 生成正反面**：调用 `ai::Completion` 从正文/选中块生成 `card_front` / `card_back`，用户可编辑确认；也支持完全手动创建。
- **批量生成**：对一篇长笔记可一次性抽取多个要点，生成多张卡片。
- **卡片即刻纳入复习**：生成后写入 `ReviewState`（初始 `state=new`），进入复习队列。

```
任意 Memory (note/screen/idea/clip…)
        │  Orbit「生成卡片」 (ai::Completion)
        ▼
新 Memory { source: orbit, kind: card, content: 正面/背面 }
        │  store.link
        ▼
Link { from: 卡片, to: 原记忆, relation: derived_from, created_by: ai }
        │
        ▼
ReviewState { state: new, due_at: now }   → 进入复习队列
```

### 2.4 间隔复习

复习引擎基于 `model::ReviewState`，默认算法 **FSRS**（可选 SM-2），见 data-model.md §2.5。

- **复习队列**：按 `due_at` 拉取到期卡片，`state` 在 `new / learning / review / relearning` 间流转。
- **评分**：每次复习给出评级（Again/Hard/Good/Easy），FSRS 据此更新 `stability` / `difficulty`，重算 `due_at`、累加 `reps` / `lapses`。
- **到期提醒**：`nexus-core` 的 `events` 模块在卡片到期时发出 `review.due` 事件，Orbit 订阅后在桌面/移动端提醒（移动端本地通知）。
- **跨设备一致**：`ReviewState` 也是 Memory 的一部分，经 `sync`（CRDT）在设备间收敛——手机上复习的进度，桌面立即可见。

```rust
// 概念示意：拉取到期队列并提交一次评分
let due = core.reviews_due(Utc::now(), 50).await?;      // 今日到期卡片
core.grade(card_id, Rating::Good).await?;                // FSRS 重算 due_at/stability
```

> 复习状态读写对应 Memory Protocol 的 `review` scope（见 memory-protocol.md §4.2），属「Orbit 生态」能力域。

### 2.5 记忆整合与关联发现

第二大脑不是记忆的堆积，而是记忆的**编织**。Orbit 主动发现记忆之间的联系：

| 能力 | 实现要点 |
|------|---------|
| **相关记忆推荐** | 基于向量邻域 + 关联图（`search` §4.2），在查看一条记忆时推荐「你可能还想看」 |
| **去重与合并** | 写入管线的内容哈希 + 近重复向量判断（`ingest`）识别重复，Orbit 提示合并，标 `relation=duplicate` |
| **自动关联** | 语义相近的记忆自动建 `related` 链（`created_by=system`），用户可确认或忽略 |
| **知识图谱** | 以 Memory 为节点、`Link` 为边，渲染可交互的概念网络；主题聚类为图着色 |

这些关联全部落在统一的 `Link` 表上（`references` / `derived_from` / `related` / `duplicate`），既服务图谱可视化，也回哺检索的「相关推荐」。

### 2.6 连接与隐私管理中心

Orbit 独家承担整个记忆库的「连接与隐私」控制台（依赖 `nexus-protocol` + `crypto` + `sync`），这是它作为中枢的治理职责：

- **已连接应用管理**：查看所有已授权的一等公民客户端与外部应用，逐个查看其持有的 scope、最近活跃、写入的 `source`。
- **scope 与令牌**：审批新接入请求（本地弹窗确认），**随时撤销**任意令牌（见 memory-protocol.md §4）。外部写入统一标 `source=external:*`，便于按来源审计与批量清理。
- **同步档位**：在「纯本地 / 本地+E2E 云同步 / 本地+自托管」三档间切换（见 data-model.md §4）。
- **导出**：Markdown + front-matter、JSON 全量导出（含关系），保证「数据可带走」。
- **E2E 密钥与恢复短语**：查看设备密钥状态、生成/展示恢复短语（BIP39 式）、新设备配对码/二维码。UI 明确告知「丢失短语则数据不可恢复」。

> 授权、撤销、数据流向都在 Orbit 的「连接与隐私」面板集中可见可控（memory-protocol.md §8）。此面板需要 `admin` scope，仅对一等公民应用与用户本人开放。

### 2.7 作为本地记忆服务的持有者

同一设备上四款软件共享**同一个本地记忆库**，需要一个「持有者」承载 Memory Protocol 本地服务（见 architecture.md §5.3、memory-protocol.md §3）。

- Orbit 常驻、且是天然的「消费中心」，非常适合担任**持有者**：启动时监听本地回环端口 / 命名管道，其余 App 作为客户端连接。
- **单实例仲裁**：通过「约定端口 + 本地锁文件（记录当前持有者端点与公钥）」发现已有持有者；持有者退出时仲裁移交下一存活实例，带库锁与健康检查。
- 若 Orbit 未运行，则由首个启动的 App 或独立的 `nexus-daemon` 轻服务持有——Orbit 再启动时作为客户端接入。**任何一方都不各自开库**，写操作串行化经单一入口（`store` 单写者模型 + CRDT 兜底）。

> 这正是「内联」体验的地基：Echo 刚抓的截图，Orbit 通过 `events`（`memory.created`）立刻收到通知并刷新，无需导入导出。

---

## 3. 交互流程

### 3.1 间隔复习流程（时序）

```
用户            Orbit 前端         src-tauri         nexus-core
 │                 │                  │            (reviews/events/sync)
 │  打开「今日复习」 │                  │                  │
 │────────────────►│  invoke          │                  │
 │                 │─────────────────►│  reviews_due()   │
 │                 │                  │─────────────────►│  按 due_at 查询
 │                 │                  │◄─────────────────│  Vec<Memory(card)>+ReviewState
 │                 │◄─────────────────│                  │
 │  逐张回忆/翻面   │                  │                  │
 │  评分 Good      │                  │                  │
 │────────────────►│  invoke grade    │                  │
 │                 │─────────────────►│  grade(id,Good)  │
 │                 │                  │─────────────────►│  FSRS 重算 stability/due_at
 │                 │                  │                  │  reps++；写 ReviewState
 │                 │                  │                  │  sync: 记账(CRDT oplog)
 │                 │◄─────────────────│◄─────────────────│  下一张 / 队列空
 │  队列清空 🎉     │                  │                  │
 │                 │                  │   (稍后到期)      │
 │                 │◄──── event: review.due ────────────│  events 广播 → 本地通知
```

### 3.2 记忆问答检索流程（RAG，时序）

```
用户          Orbit          nexus-core.search      ai::Completion
 │             │                    │                    │
 │ 提问        │                    │                    │
 │────────────►│ POST /v1/ask       │                    │
 │             │───────────────────►│ 检索取材(Hybrid)   │
 │             │                    │  向量+FTS→RRF融合  │
 │             │◄───────────────────│ top-k SearchHit    │
 │             │  拼装上下文(命中片段)                     │
 │             │  [数据最小化 + UI 提示 Provider]          │
 │             │─────────────────────────────────────────►│ complete/stream
 │             │◄─────────────────────────────────────────│ 答案(流式 token)
 │             │  回填 citations(memory_id/block_id)       │
 │◄────────────│ 带引用的答案                              │
 │ 点击引用溯源 │                                          │
 │────────────►│ GET /v1/memories/{id} → 展开原始记忆      │
```

### 3.3 一条 Quill 笔记如何变成卡片（时序）

```
Quill              本地服务(Protocol)        Orbit               nexus-core
 │                       │                     │             (ingest/store/ai/events)
 │ 写完笔记，标记「送去复习」                    │                     │
 │ POST /v1/memories(note)                     │                     │
 │──────────────────────►│  ingest 管线        │                     │
 │                       │────────────────────────────────────────►│ 切块/嵌入/落库
 │                       │  event: memory.created                    │
 │                       │─────────────────────►│ (订阅) 待生成卡片列表 +1
 │                       │                       │                    │
 │                       │        用户在 Orbit 点「生成卡片」          │
 │                       │                       │ ai::Completion 生成正反面
 │                       │                       │───────────────────►│ complete()
 │                       │                       │◄───────────────────│ card_front/back
 │                       │  新建 Memory(source=orbit,kind=card)        │
 │                       │                       │───────────────────►│ store.create
 │                       │  关联 derived_from → 原笔记                 │
 │                       │                       │───────────────────►│ store.link
 │                       │  初始化 ReviewState(state=new)              │
 │                       │                       │───────────────────►│ reviews.upsert
 │                       │                       │ 卡片进入复习队列 ✅ │
```

---

## 4. 数据模型映射

Orbit 的数据角色可概括为一句话：**消费所有 Memory，生产 `kind=card` 的 Memory 与 ReviewState。**

| 数据 | Orbit 的关系 | 说明 |
|------|-------------|------|
| `Memory`（全部 source/kind） | **消费** | 检索、问答、关联、生成卡片的素材来源 |
| `Memory`（`source=orbit`, `kind=card`） | **生产** | 从任意记忆派生出的知识卡片 |
| `ReviewState` | **生产 + 维护** | 卡片的间隔复习状态，FSRS 参数随评分更新 |
| `Link`（`derived_from` / `related` / `duplicate`） | **生产** | 卡片溯源、自动关联、去重合并 |
| `Collection` | 读写 | 组织复习集与主题库 |
| `meta`（Orbit 扩展字段） | 生产 | 见下方约定 |

### 4.1 知识卡片 Memory 示例（`kind=card`）

```jsonc
{
  "id": "01J9ZT7K3M8Q0RVB2CE4WX7N6H",
  "source": "orbit",                 // Orbit 生成
  "kind": "card",
  "title": "FSRS 的两个核心参数",
  "content": "## 正面\nFSRS 用来刻画记忆的两个核心参数是什么？\n\n## 背面\nstability（稳定性）与 difficulty（难度）。",
  "content_format": "markdown",
  "tags": ["spaced-repetition", "memory"],
  "links": [
    {
      "from_id": "01J9ZT7K3M8Q0RVB2CE4WX7N6H",  // 本卡片
      "to_id":   "01J9ZQF0AA11BB22CC33DD44EE",  // 原始 Quill 笔记
      "relation": "derived_from",
      "created_by": "ai"
    }
  ],
  "review": { "...": "见下方 ReviewState 示例" },
  "created_at": "2026-07-11T07:20:00Z",
  "updated_at": "2026-07-11T07:20:00Z",
  "device_id": "mac-studio-01",
  "meta": {
    "orbit": {
      "generator": "ai",             // ai | manual
      "provider": "claude",          // 生成所用 Completion Provider（可空=本地）
      "source_block_id": "b12",      // 派生自原记忆的哪个块
      "deck": "reading/2026"         // 所属复习集（映射 Collection）
    }
  }
}
```

### 4.2 ReviewState 示例

对应 data-model.md §2.5，随每次评分由 FSRS 更新：

```jsonc
{
  "memory_id": "01J9ZT7K3M8Q0RVB2CE4WX7N6H",
  "card_front": "FSRS 用来刻画记忆的两个核心参数是什么？",
  "card_back":  "stability（稳定性）与 difficulty（难度）。",
  "stability": 12.7,        // FSRS：记忆稳定度（天）
  "difficulty": 5.3,        // FSRS：难度
  "due_at": "2026-07-24T00:00:00Z",
  "last_reviewed_at": "2026-07-11T07:22:10Z",
  "reps": 3,
  "lapses": 0,
  "state": "review"         // new | learning | review | relearning
}
```

> 约定：Orbit 的私有扩展字段一律放在 `meta.orbit.*` 下（见 data-model.md §6，避免频繁改主表结构）。`card_front` / `card_back` 属于 `ReviewState` 标准字段，不重复放进 `meta`。

---

## 5. 技术实现

### 5.1 应用形态：桌面 + 移动 Tauri 应用

Orbit 是一个 Tauri 2.0 应用（`apps/orbit/`），同时面向桌面（Win/macOS）与移动端（iOS/Android）。前端 React + `@nexus/ui`，Rust 侧 `src-tauri` 装配命令与插件。

```
┌──────────────────────────────────────────────────────────┐
│ 前端 (React + @nexus/ui)                                   │
│   检索页 · 问答页 · 复习界面 · 图谱 · 连接与隐私控制台      │
└───────────────┬────────────────────────────┬─────────────┘
                │ Tauri IPC                    │
┌───────────────▼────────────────────────────▼─────────────┐
│ apps/orbit/src-tauri (Rust)                                │
│   命令处理 · 复习提醒调度 · 托盘/通知 · 本地服务装配        │
└───────────────┬────────────────────────────┬─────────────┘
   ┌────────────▼───────────┐   ┌─────────────▼────────────┐
   │ nexus-core             │   │ nexus-protocol           │
   │ store/search/embed     │   │ 本地服务(loopback/IPC)   │
   │ crypto/sync/ai/events  │   │ 令牌/scope/单实例仲裁     │
   └────────────┬───────────┘   └─────────────┬────────────┘
   ┌────────────▼───────────┐   ┌─────────────▼────────────┐
   │ platform-desktop/mobile │   │ sdk/mcp-server           │
   │ 后台任务·本地通知        │   │ 让 AI 助手接入(MCP)      │
   └─────────────────────────┘   └──────────────────────────┘
```

> **移动端关键路径优先验证**（呼应 architecture.md §6 的首要风险）：Orbit 移动端不是「桌面的缩小版」，而是硬需求。工程排期上，先打通「移动端能连到记忆库、能复习、能问答、能收本地通知」这条主干，再补桌面独有的重功能（如大图谱）。平台差异（后台任务、本地通知）通过 `platform-mobile` 的 trait 隔离。

### 5.2 深度依赖 nexus-core

Orbit 是四款软件里对 `nexus-core` 依赖最深、最全的一个——它几乎用到每一个模块：

| 模块 | Orbit 的用法 |
|------|-------------|
| `search` | 统一检索中心的引擎；RAG 取材；相关推荐、主题聚类 |
| `events` | 订阅 `memory.created`（内联实时刷新）、`review.due`（到期提醒） |
| `ai`（`Completion`） | 问答生成、卡片正反面生成；按用户设置路由本地/云端 |
| `ai`（`Embedder`） | 间接：检索/聚类依赖块级向量（由 `embed` 在写入时产出） |
| `store` | 卡片 CRUD、`Link` 关联、`ReviewState` 与 `Collection` 仓储 |
| `sync` | 复习进度、卡片、关联的跨设备 CRDT 收敛 |
| `crypto` | E2E 密钥状态、恢复短语、媒体解密（在本地明文层检索） |

Orbit **不重复实现** 检索/嵌入/加密/同步，全部下沉到核心，改一处四端受益。

### 5.3 承载 nexus-protocol 服务

Orbit 内置 `nexus-protocol` 本地服务能力（见 memory-protocol.md），作为潜在的**持有者**：

- 监听本机回环 / IPC，暴露 `POST /v1/memories`、`/v1/search`、`/v1/ask`、`GET /v1/events`、`/v1/collections`、`POST /v1/links` 等。
- 负责令牌签发与校验、scope 检查、单实例仲裁（§2.7）。
- 远程形态（供移动端跨设备访问桌面库、或自托管中继）同形，换 HTTPS + 长期令牌。

### 5.4 MCP Server：让 AI 助手接入

外联的重点接入方式（见 memory-protocol.md §6）。`sdk/mcp-server` 把记忆库暴露为 Model Context Protocol 工具，Orbit 负责在「连接与隐私」面板管理其授权：

```jsonc
// MCP 提供的工具（AI 助手可直接调用）
"add_memory(content, tags?, source?)"      // 写入（source=external:<ai>）
"search_memory(query, filters?)"           // 混合检索
"get_memory(id)"                           // 读取
"ask_memory(question)"                     // RAG 问答，带引用
```

任意支持 MCP 的 AI 客户端（如 Claude）都能把 Nexus 当作可检索的长期记忆——这正是「对接任何有信息记录需求的大脑」的兑现。

---

## 6. 「内联 + 外联」架构说明

Orbit 作为开放中枢，一侧连接家族内的一等公民客户端（内联），另一侧连接任意第三方/AI/扩展（外联）。二者走**同一套** Memory Protocol、落到**同一个** `nexus-core`。

```
        ┌───────────── 内联 (inline) ─────────────┐
        │      家族一等公民客户端（第一批）         │
        │  ┌────────┐  ┌────────┐  ┌────────┐      │
        │  │  Echo  │  │  Muse  │  │ Quill  │      │
        │  │ screen │  │ idea/  │  │  note  │      │
        │  │        │  │ voice  │  │        │      │
        │  └───┬────┘  └───┬────┘  └───┬────┘      │
        └──────┼───────────┼───────────┼───────────┘
               │  写入/检索/订阅 (Memory Protocol)
               ▼           ▼           ▼
        ╔══════════════════════════════════════════╗
        ║                 Orbit                    ║
        ║        中枢记忆库 · 第二大脑本体          ║
        ║  检索中心 · RAG问答 · 卡片 · 复习 · 图谱  ║
        ║  连接与隐私管理 · (可作)本地服务持有者     ║
        ╠══════════════════════════════════════════╣
        ║  nexus-core  (store/search/ai/sync/…)    ║
        ╚══════════════════════════════════════════╝
               ▲           ▲           ▲
               │  写入/检索/问答 (Memory Protocol)
        ┌──────┼───────────┼───────────┼───────────┐
        │  ┌───┴────┐ ┌────┴─────┐ ┌───┴──────────┐│
        │  │第三方App│ │AI助手(MCP)│ │浏览器扩展     ││
        │  │/脚本   │ │Claude 等  │ │(native msg)  ││
        │  └────────┘ └──────────┘ └──────────────┘│
        └───────────── 外联 (external) ─────────────┘
                  对接「任何有记录需求的大脑」
```

要点：

- **对称性**：内联与外联在协议层是同一套语义。Echo/Muse/Quill 只是「授权范围更宽、体验更深」的一等公民，本质上和第三方走一样的 Memory 契约。这保证了 Orbit 天生开放，而非事后开放。
- **来源可辨、可治理**：内联写入用 `source=echo/muse/quill`，外联写入统一 `source=external:<app_id>`，便于审计、按来源撤销与批量清理。
- **对接任何大脑**：无论信息来自截屏、语音、笔记、网页剪藏，还是 AI 助手的对话记录，最终都归一为 Memory，在 Orbit 中一并被检索、关联、复习。这就是「不只是前三者的私有大脑」的架构含义。

---

## 7. 关键难点与对策

| 难点 | 影响 | 对策 |
|------|------|------|
| **Tauri 移动端成熟度与后台** | 移动端是硬需求，但 Tauri 移动端年轻，后台任务/本地通知/长连接受系统限制 | 平台适配层（`platform-mobile`）隔离差异；**优先验证 iOS/Android 关键路径**（连库/复习/问答/通知）；到期提醒用系统本地通知调度而非常驻进程；必要时局部改用原生视图（architecture.md §2.1 的退路） |
| **承载本地服务的可靠性与单实例仲裁** | Orbit 作持有者时崩溃/退出会影响全家族 | 约定端口 + 锁文件（记端点与公钥）；健康检查 + 仲裁移交下一存活实例；持有者退出走优雅移交；库锁 + `store` 单写者串行化 + CRDT 兜底（memory-protocol.md §3、nexus-core.md §2） |
| **大规模记忆的检索性能** | 十万级 Memory 下检索/聚类可能变慢 | sqlite-vec 近邻 + FTS5 同库，避免跨系统 join；RRF 融合限定 top-k；重排仅对 top-k；主题聚类离线/增量计算并缓存；对齐 nexus-core.md §11 的性能基准（万级/十万级检索延迟） |
| **复习算法调参** | FSRS 默认参数未必贴合个人 | 默认 FSRS，可切 SM-2；随复习历史（`reps`/`lapses`/评分序列）在本地做参数个性化拟合；参数与调度全在本地明文层完成，不上云 |
| **跨设备同步一致性** | 手机与桌面的复习进度/卡片可能冲突 | `ReviewState` 走字段级 CRDT 合并（如 `due_at` 取最新一次评分结果，`reps` 单调）；正文冲突留痕交用户处理；tombstone 传播删除（nexus-core.md §7） |
| **E2E 下检索都在本地** | 云端零知识，无法借云端算力检索 | 语义检索/嵌入/RAG 取材/复习调度全部在**本地明文库**完成；云端只做加密块中继与可选密文备份（data-model.md §4）；这是隐私红线，不为性能妥协 |
| **云端 Completion 的隐私与成本** | 问答/卡片走云端会外发文本、产生费用 | 数据最小化：只发检索命中的必要片段；UI 明示将发送给哪个 Provider；支持自带 Key、自定义端点、纯本地 LLM 回落（data-model.md §5.5、nexus-core.md §8） |

---

## 8. 设置项与分阶段计划

### 8.1 主要设置项

| 分类 | 设置项 |
|------|--------|
| 检索 | 默认检索模式（Hybrid/Semantic/Keyword）、是否启用重排、默认过滤范围 |
| 问答（RAG） | Completion Provider（本地 LLM / 云端 Claude 等 / 自定义端点）、是否流式、默认 scope、发送前确认提示 |
| 卡片 | 生成方式（AI/手动）、生成所用 Provider、每篇最多抽取卡片数、默认复习集（deck） |
| 复习 | 算法（FSRS/SM-2）、每日新卡上限、每日复习上限、到期提醒时间与渠道（桌面/移动本地通知） |
| 关联 | 是否开启自动关联（`related`）、去重提示阈值、图谱显示密度 |
| 同步 | 档位（纯本地 / 本地+E2E 云 / 本地+自托管）、中继端点、冲突处理偏好 |
| 连接与隐私 | 已连接应用与 scope、令牌撤销、外部写入审计、导出（MD/JSON） |
| 安全 | 恢复短语查看/导出、新设备配对、密钥状态 |
| 服务 | 是否允许 Orbit 作为本地服务持有者、监听端点、MCP Server 开关与授权 |

### 8.2 分阶段计划

对齐 architecture.md §5.2 平台矩阵与 roadmap 的节奏，**移动端是一期硬需求，不后置**。

| 阶段 | 目标 | 交付内容 |
|------|------|---------|
| **MVP（一期）** | 让第二大脑先转起来 | 统一检索中心（Hybrid）· 知识卡片生成（AI + 手动）· 间隔复习（FSRS，桌面）· 桌面端（Win/macOS）· 作为本地服务持有者 + 单实例仲裁 · 内联打通 Echo/Muse/Quill |
| **二期** | 随身 + 问答 | **移动端（iOS/Android）——关键路径优先验证**（连库/复习/本地通知/问答）· 记忆问答（`POST /v1/ask`，带引用）· 到期提醒（`review.due` → 移动本地通知）· 跨设备 E2E 同步复习进度 |
| **三期** | 全面外联 | MCP Server（AI 助手接入）· 浏览器扩展剪藏（native messaging）· SDK 外联（`@nexus/sdk-ts` / `nexus-sdk`）· 「连接与隐私」控制台完善（scope/令牌/审计/导出） |
| **四期** | 编织与深化 | 记忆整合与关联发现（去重合并、自动关联）· 知识图谱可视化 · 主题聚类与时间线 · FSRS 个性化调参 · 复习算法与检索性能在十万级规模上的打磨 |

> 里程碑与全家族节奏详见 [roadmap.md](../roadmap.md)。

---

## 9. 与各文档的关系

- 整体架构与平台矩阵（移动端硬需求、Linux 放缓、本地服务持有者）→ [architecture.md](../architecture.md)
- Memory / ReviewState / FSRS / Link / Collection 的字段定义 → [data-model.md](../data-model.md)
- `search` / `events` / `ai::Completion` / `Embedder` / `sync` / `crypto` 的实现 → [nexus-core.md](../nexus-core.md)
- `POST /v1/ask`、`/v1/search`、scope、令牌、MCP、单实例仲裁 → [memory-protocol.md](../memory-protocol.md)
- 兄弟应用（捕获层）→ [echo.md](echo.md) · [muse.md](muse.md) · [quill.md](quill.md)
