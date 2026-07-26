# Nexus 统一记忆数据模型与隐私安全

本文档定义整个产品家族共用的「记忆」数据结构，以及本地优先 + 端到端加密的隐私安全设计。四款软件与所有外联应用都围绕这个模型工作。

---

## 1. 核心概念：一切皆 Memory

无论是 Echo 抓的屏幕、Muse 记的灵感、Quill 写的笔记，还是第三方喂进来的数据，最终都归一为统一的 **Memory（记忆项）**。这是整个系统能「内联+外联」的根基。

```
Memory  ← 记忆的原子单位（一段有意义的信息）
  ├── source        它从哪来（echo/muse/quill/外部应用）
  ├── content       正文（Markdown / 纯文本 / 结构化）
  ├── blocks        可选：切分后的语义块（用于精细检索）
  ├── media[]       关联的图片/音频/文件
  ├── embeddings[]  向量表示（供语义检索）
  ├── tags[]        标签
  ├── links[]       与其他 Memory 的关联（引用/派生/相关）
  ├── review        复习状态（供 Orbit 间隔重复）
  └── meta          时间、设备、地理、应用特有字段
```

### 为什么统一

- **检索一致**：在 Orbit 里一次检索能同时命中截图、速记、笔记。
- **关联自然**：一条速记可以 `link` 到一篇笔记，一张截图可以派生出一张知识卡片。
- **外联简单**：第三方只需理解一种数据结构。

---

## 2. 数据模型定义

### 2.1 Memory（记忆项）

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | UUID v7 | 全局唯一，时间可排序 |
| `source` | enum | `echo` / `muse` / `quill` / `orbit` / `external:<app_id>` |
| `kind` | enum | `screen` / `note` / `idea` / `voice` / `card` / `clip` / `file` |
| `title` | string? | 可选标题（无则由正文首行/AI 生成） |
| `content` | text | 正文，统一以 Markdown 存储 |
| `content_format` | enum | `markdown` / `plain` / `json` |
| `blocks` | Block[] | 语义切块（见 2.2），用于块级向量与引用 |
| `media` | MediaRef[] | 关联媒体（见 2.3） |
| `tags` | string[] | 标签 |
| `links` | Link[] | 关联关系（见 2.4） |
| `review` | ReviewState? | 复习状态（见 2.5），仅当被纳入复习 |
| `pinned` | bool | 置顶 |
| `archived` | bool | 归档（不删除但不常显示） |
| `created_at` | timestamp | 创建时间 |
| `updated_at` | timestamp | 更新时间 |
| `captured_at` | timestamp? | 信息实际发生时间（截屏/录音时刻） |
| `device_id` | string | 来源设备 |
| `meta` | JSON | 应用特有的扩展字段（见各 app 文档） |

### 2.2 Block（语义块）

正文会被切分为块，块是检索与引用的最小单位。

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | UUID | 块 ID |
| `memory_id` | UUID | 所属记忆 |
| `seq` | int | 顺序 |
| `type` | enum | `heading` / `paragraph` / `code` / `list` / `quote` / `ocr_line` / `transcript` |
| `text` | text | 块文本 |
| `embedding` | vector? | 该块的向量（sqlite-vec 存储） |

### 2.3 MediaRef（媒体引用）

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | UUID | |
| `kind` | enum | `image` / `audio` / `video` / `file` |
| `path` | string | 本地加密文件路径（内容加密，见 §5） |
| `mime` | string | |
| `width`/`height`/`duration` | number? | 尺寸/时长 |
| `ocr_text` | text? | 图片 OCR 结果（Echo） |
| `transcript` | text? | 音频转写（Muse） |
| `hash` | string | 内容哈希（去重/校验） |

### 2.4 Link（关联）

| 字段 | 类型 | 说明 |
|------|------|------|
| `from_id` | UUID | 源记忆 |
| `to_id` | UUID | 目标记忆 |
| `relation` | enum | `references`（引用）/ `derived_from`（派生自）/ `related`（相关）/ `duplicate` |
| `created_by` | enum | `user` / `ai` / `system` |

例：Orbit 由一篇 Quill 笔记生成知识卡片时，卡片 `derived_from` 该笔记；Echo 截图 OCR 后 Muse 引用它，则 `references`。

### 2.5 ReviewState（复习状态，Orbit 专用）

基于间隔重复算法（默认 FSRS，可选 SM-2）。

| 字段 | 类型 | 说明 |
|------|------|------|
| `memory_id` | UUID | |
| `card_front` / `card_back` | text | 卡片正反面（可由 AI 生成或手动） |
| `stability` / `difficulty` | float | FSRS 参数 |
| `due_at` | timestamp | 下次到期 |
| `last_reviewed_at` | timestamp? | |
| `reps` / `lapses` | int | 复习次数 / 遗忘次数 |
| `state` | enum | `new` / `learning` / `review` / `relearning` |

### 2.6 Collection（集合/知识库）

用于组织记忆的容器（类似 folder/notebook，可嵌套，一个 Memory 可属于多个集合）。

| 字段 | 类型 |
|------|------|
| `id` / `name` / `icon` / `parent_id` / `sort` |

---

## 3. 存储布局（SQLite）

```sql
-- 概念性 DDL（实际以迁移脚本管理）
CREATE TABLE memories (
  id TEXT PRIMARY KEY, source TEXT, kind TEXT, title TEXT,
  content TEXT, content_format TEXT,
  pinned INTEGER, archived INTEGER,
  created_at INTEGER, updated_at INTEGER, captured_at INTEGER,
  device_id TEXT, meta TEXT  -- JSON
);
CREATE TABLE blocks (
  id TEXT PRIMARY KEY, memory_id TEXT, seq INTEGER,
  type TEXT, text TEXT
);
-- 向量：sqlite-vec 虚拟表，块级向量
CREATE VIRTUAL TABLE block_vectors USING vec0(
  block_id TEXT, embedding FLOAT[768]
);
-- 全文检索：FTS5
CREATE VIRTUAL TABLE memories_fts USING fts5(
  title, content, content=memories
);
CREATE TABLE media  ( ... );
CREATE TABLE links  ( from_id, to_id, relation, created_by );
CREATE TABLE reviews( ... );
CREATE TABLE collections( ... );
CREATE TABLE collection_items( collection_id, memory_id );
```

- 关系、全文、向量**同库**，混合检索无需跨系统 join。
- 大媒体文件不入库，落盘为独立加密文件，库里存引用与哈希。

---

## 4. 混合存储模型（本地优先 + 可选云）

```
        设备 A                          云中继 (可选)                 设备 B
  ┌──────────────────┐            ┌────────────────────┐       ┌──────────────────┐
  │ 明文 SQLite (工作库)│            │  只存加密块 (blob)   │       │ 明文 SQLite (工作库)│
  │ 向量/FTS/检索      │  加密变更   │  永远无法解密        │ 加密变更│ 向量/FTS/检索      │
  │        │         │ ─────────► │  仅做存储与转发       │◄───── │        │         │
  │   本地密钥 🔑     │            └────────────────────┘       │   本地密钥 🔑     │
  └──────────────────┘                                          └──────────────────┘
```

三种运行档位，用户自选：

1. **纯本地**：只有明文本地库，不联网。最高隐私。
2. **本地 + E2E 云同步**：本地明文工作，变更加密后经云中继同步到其他设备。云端只见密文。
3. **本地 + 自托管**：云中继换成用户自己的服务器 / NAS。

> **重要边界**：语义检索、AI 处理都发生在**本地明文库**上。云端只承担「加密块的存储与转发」，不做检索、不持有密钥。这样既能 E2E 加密，又不牺牲检索能力。

---

## 5. 隐私与安全设计

### 5.1 端到端加密

- **算法**：内容与媒体用 XChaCha20-Poly1305（AEAD）；密钥交换/派生用现代方案（Argon2id 从主密码派生根密钥）。
- **主密钥**：由用户主密码 + 设备派生，永不上传。云端拿到的只有密文 blob 与不可逆的同步元数据。
- **媒体加密**：图片/音频等落盘即加密，按内容分块加密以支持增量同步。

### 5.2 密钥管理与多设备

- 首个设备生成根密钥（存于系统安全区：Windows DPAPI / macOS Keychain / iOS Keychain / Android Keystore）。
- 新设备加入：通过**配对码 / 二维码**在两台设备间安全传递密钥（不经服务器明文），或用恢复短语（BIP39 式）。
- 提供**恢复短语导出**，丢失所有设备时可恢复。丢失短语则数据不可恢复——这是 E2E 的代价，需在 UI 明确告知。
- 每台设备另生成 Ed25519 签名身份；中继只保存公钥，设备私钥封存在本机系统安全区。同步信封必须通过已登记且未撤销设备的签名验证。
- 配对二维码使用 256 位一次性秘密封装同步根密钥，中继只转发配对密文；界面显示的六位确认码仅供两台设备人工核对，不能替代二维码中的高熵秘密。
- 恢复短语固定为可还原 256 位根密钥的 24 词 BIP39 英文短语，并使用 BIP39 校验位拒绝误抄。
- 内容操作携带按设备全局逻辑时钟推进的版本向量。并发版本按稳定规则确定当前胜者，同时保留失败内容或删除墓碑；桌面与 Android 使用同一冲突契约预览、恢复或手工合并。
- 解决冲突时客户端必须提交打开检查器时的稳定版本 ID 集合。同步层重新核对集合后才生成观察全部旧版本的因果后继；版本 ID 为内容、设备与时间的 BLAKE3 哈希，不把明文放入同步元数据。

### 5.3 权限与本地服务鉴权

- 四款软件共享本地记忆库经由 Memory Protocol，本地服务对每个客户端签发**能力令牌**（capability token），限定其可读写的范围（见 memory-protocol.md）。
- 外部应用接入需用户显式授权，按**能力域**（scope）最小授权：如只读 / 只写指定 source / 仅特定 collection。

### 5.4 敏感数据处理

- Echo 屏幕抓取可能含密码、隐私窗口：提供**排除名单**（按应用/窗口标题）、**手动确认再入库**模式、以及入库前的**敏感信息检测**（本地模型识别疑似密钥/身份证号并提示打码）。
- 删除即**可证删除**：删除 Memory 时连带删除其块、向量、媒体文件与同步记录（云端下发 tombstone，中继删除对应 blob）。

### 5.5 云端 AI 的数据最小化

当用户选择云端大模型做总结/卡片时：
- 仅发送**完成该任务所必需的最小文本**，不发送整库。
- UI 明示「本次操作将把这段内容发送到 <Provider>」。
- 支持用户自带 Key、自定义端点（可指向自托管/私有部署模型）。

---

## 6. 版本化与迁移

- 数据库用递增迁移脚本（`nexus-core` 内置 migrator），跨端版本一致。
- Memory 结构变更向后兼容：`meta` 字段承载应用特有扩展，避免频繁改主表结构。
- 导入/导出：支持 Markdown + front-matter、JSON 全量导出（含关系），保证「数据可带走」。

---

## 7. 与各文档的关系

- 检索/加密/同步的**实现** → [nexus-core.md](nexus-core.md)
- 外部如何读写这些数据 → [memory-protocol.md](memory-protocol.md)
- 各 app 往 `meta` 里放什么 → `apps/*.md`
