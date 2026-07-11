# nexus-core · 共享核心引擎

`nexus-core` 是用 Rust 编写的共享核心库，承载所有与「记忆」有关的重活。四款软件与外联服务都建立在它之上，改一处四端受益。

---

## 1. 模块总览

```
nexus-core
├── store        存储引擎：SQLite 封装、迁移、CRUD、事务
├── search       检索：向量 + 全文 + 混合排序
├── embed        嵌入：文本切块与向量化（调用 nexus-ai）
├── crypto       加密：AEAD、密钥派生、媒体加密、安全区对接
├── sync         同步引擎：CRDT、变更日志、云中继客户端
├── ai           AI Provider 抽象（本地/云端统一接口，实体在 nexus-ai）
├── ingest       写入管线：清洗→切块→嵌入→去重→落库
├── model        数据模型（Memory/Block/Media/Link/Review…）
└── events       事件总线：记忆变更通知（供 UI/其他 App 订阅）
```

依赖关系：`ingest` 编排 `embed`+`store`+`sync`；`search` 读 `store`；`crypto` 服务于 `store`(媒体) 与 `sync`(传输)；`ai` 被 `embed` 与上层功能调用。

---

## 2. store — 存储引擎

- 基于 `rusqlite`（或 `sqlx` + SQLite），启用 `sqlite-vec` 与 FTS5 扩展。
- 提供类型安全的仓储 API：

```rust
pub struct MemoryStore { /* 连接池、迁移器 */ }

impl MemoryStore {
    pub async fn create(&self, m: NewMemory) -> Result<Memory>;
    pub async fn get(&self, id: &Id) -> Result<Option<Memory>>;
    pub async fn update(&self, id: &Id, patch: MemoryPatch) -> Result<Memory>;
    pub async fn delete(&self, id: &Id) -> Result<()>;   // 级联块/向量/媒体/tombstone
    pub async fn list(&self, q: ListQuery) -> Result<Page<Memory>>;
    pub async fn link(&self, l: Link) -> Result<()>;
    // 集合、复习等仓储...
}
```

- **迁移**：内置 migrator，版本号随库演进，启动时自动升级。
- **并发**：单写者模型（WAL 模式 + 写串行化），读并发。多 App 共享库时由 Protocol 层保证单一写入入口（见 architecture.md §5.3）。

---

## 3. ingest — 写入管线

所有记忆写入走统一管线，保证质量一致：

```
原始输入
  → 规范化 (转 Markdown / 提取纯文本)
  → 切块 (按标题/段落/代码/OCR行/转写句)
  → 去重 (内容哈希 + 近重复向量判断)
  → 嵌入 (embed: 每块生成向量)
  → 敏感检测 (可选: 本地模型识别疑似隐私)
  → 落库 (store: 事务写入 memory+blocks+vectors+fts)
  → 记账 (sync: 写变更日志)
  → 广播 (events: 通知订阅者，如 Orbit 实时刷新)
```

```rust
pub struct Ingestor { /* store, embedder, sync, events */ }
impl Ingestor {
    /// 各 App/协议的统一入口
    pub async fn ingest(&self, input: IngestInput) -> Result<Memory>;
}
```

---

## 4. search — 检索

### 4.1 混合检索

同时利用向量语义检索与关键词全文检索，融合排序：

```rust
pub struct SearchQuery {
    pub text: String,
    pub mode: SearchMode,        // Semantic / Keyword / Hybrid(默认)
    pub filters: Filters,        // source/kind/tags/collection/时间范围
    pub limit: usize,
}
pub async fn search(&self, q: SearchQuery) -> Result<Vec<SearchHit>>;
```

- **向量路**：查询文本嵌入后在 `block_vectors` 做近邻检索（sqlite-vec）。
- **关键词路**：FTS5 BM25。
- **融合**：RRF（Reciprocal Rank Fusion）合并两路结果，块级命中回溯到 Memory 并去重、聚合、高亮。
- **可选重排**：命中后用小型 rerank 模型或云端模型对 top-k 精排。

### 4.2 面向 Orbit 的高级检索

- 「相关记忆」推荐（基于向量邻域 + 关联图）。
- 时间线 / 主题聚类。
- 供问答（RAG）取材：检索 → 拼装上下文 → 交给 AI Provider。

---

## 5. embed — 嵌入

- 切块策略随 `kind` 不同（笔记按标题层级，屏幕按 OCR 行聚合，语音按句）。
- 向量化调用 `ai::Embedder`，默认本地小模型（bge-small/gte，768 维，ONNX），可切云端。
- **维度一致性**：切换嵌入模型会导致向量空间不兼容 → 提供后台**重新嵌入**任务，切换模型时对存量数据重建向量。

---

## 6. crypto — 加密

- **AEAD**：XChaCha20-Poly1305 加密媒体与同步载荷。
- **KDF**：Argon2id 从主密码派生根密钥。
- **安全区**：根密钥封存于系统安全存储（DPAPI/Keychain/Keystore），运行时仅在内存解封。
- **媒体**：分块加密，块哈希用于去重与增量同步。
- 详细威胁模型与密钥流转见 [data-model.md](data-model.md) §5。

---

## 7. sync — 同步引擎

### 7.1 模型

- **CRDT** 保证多设备无中心也能收敛（记忆的字段级合并；正文用文本 CRDT 或 last-writer-wins + 冲突留痕，视字段而定）。
- **变更日志**（oplog）：每次写产生加密操作记录，同步即交换 oplog。
- **传输**：与云中继（或自托管）走加密通道；中继只存转发密文，不解密、不检索。

### 7.2 冲突与删除

- 结构化字段字段级合并；正文冲突保留双版本并标记，交由用户/Orbit 处理。
- 删除用 **tombstone** 传播，确保各端与中继都清除对应密文。

### 7.3 档位

对应 data-model.md §4 的三档（纯本地 / E2E 云 / 自托管），`sync` 通过配置切换中继端点或整体关闭。

---

## 8. ai — Provider 抽象

统一接口，屏蔽本地/云端差异；实体实现放在 `nexus-ai` crate。

```rust
pub trait Embedder {
    fn dim(&self) -> usize;
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}
pub trait Transcriber {                 // 语音转写 (Muse)
    async fn transcribe(&self, audio: AudioRef, opts: AsrOpts) -> Result<Transcript>;
}
pub trait Ocr {                         // 图像文字识别 (Echo)
    async fn recognize(&self, image: ImageRef) -> Result<OcrResult>;
}
pub trait Completion {                  // 总结/卡片/问答
    async fn complete(&self, req: CompletionReq) -> Result<CompletionResp>;
    async fn stream(&self, req: CompletionReq) -> Result<TokenStream>;
}

pub enum ProviderKind {
    Local(LocalModelCfg),               // ONNX/Candle/whisper.cpp/Ollama
    Remote(RemoteCfg),                  // Claude / OpenAI / 自定义端点，自带 Key
}
```

- **路由策略**：按任务与用户设置选择 Provider；离线时自动回落本地；云端不可用时降级。
- **成本与隐私护栏**：云端调用前做数据最小化与用户提示（见 data-model.md §5.5）。
- **默认矩阵**：见 architecture.md §2.5。

---

## 9. events — 事件总线

- 记忆的增删改产生事件（`MemoryCreated/Updated/Deleted`、`ReviewDue` 等）。
- 前端订阅以实时刷新；跨 App 场景下，Echo 写入后 Orbit 立即收到通知。
- 也是 Memory Protocol 的 `subscribe` 能力的底层（见 memory-protocol.md）。

---

## 10. 对外暴露

`nexus-core` 有两种被调用方式，共享同一套逻辑：

1. **进程内**：Tauri App 的 `src-tauri` 直接依赖 `nexus-core`，前端经 IPC 调用命令。
2. **跨进程**：`nexus-protocol` 在 `nexus-core` 之上包一层本地/远程服务，供其他 App 与第三方接入。

```
       前端(React) ──IPC──► src-tauri ──►┐
                                          ├──► nexus-core
  第三方/其他App ──Protocol──► nexus-protocol ──►┘
```

---

## 11. 测试与质量

- 检索、加密、同步（CRDT 收敛、冲突、tombstone）有充分单元与属性测试。
- `ingest` 管线有针对各 `kind` 的快照测试。
- 迁移脚本有前后兼容测试。
- 性能基准：万级/十万级记忆下的检索延迟、嵌入吞吐、同步收敛时间。
