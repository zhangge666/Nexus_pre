# Nexus 开发路线图

本文档描述交付顺序、里程碑与风险。核心策略：**先把中枢和地基打牢，再让捕获层逐个接入**。

---

## 1. 交付哲学

- **地基先行**：`nexus-core` + Memory Protocol 是四款软件共同的地基，必须先稳定。
- **中枢先立**：Orbit 是记忆库本体，它（或独立 daemon）持有本地记忆服务。没有中枢，捕获层无处可写。
- **纵向切片**：每个里程碑交付一条可用的端到端链路，而非横向堆半成品。
- **本地优先落地**：先把纯本地体验做扎实，再叠加同步与云端 AI。

---

## 2. 里程碑总览

```
M0 地基      M1 中枢MVP     M2 捕获层      M3 智能化       M4 多端       M5 外联
─────────►  ─────────►    ─────────►    ─────────►     ─────────►    ─────────►
core/协议    Orbit+库      Echo/Muse/    卡片/复习/     移动端Orbit   MCP/SDK/
数据模型     检索+桌面     Quill 接入    RAG/云AI       +同步         第三方
```

| 里程碑 | 目标 | 产出 |
|--------|------|------|
| **M0 地基** | 核心与协议可用 | `nexus-core`（store/ingest/search/embed/crypto 基础）、Memory Protocol 本地服务、数据模型与迁移 |
| **M1 中枢 MVP** | 记忆库能用起来 | Orbit 桌面版：混合检索、集合、手动记忆、本地服务持有者；本地嵌入模型 |
| **M2 捕获层接入** | 三款捕获软件内联 | Echo（截图+OCR+入库）、Muse（热键速记+入库）、Quill（Markdown+入库）桌面 MVP |
| **M3 智能化** | 第二大脑成形 | 知识卡片生成、间隔复习(FSRS)、RAG 问答、云 AI Provider 接入、语音转写 |
| **M4 多端** | Orbit 上手机 | Orbit 移动端(iOS/Android)、E2E 云同步、多设备配对与恢复短语 |
| **M5 外联** | 开放生态 | MCP Server、TS/Python SDK、浏览器扩展剪藏、连接与隐私管理面板 |

> 里程碑之间可部分重叠推进，但依赖顺序（M0→M1→M2）应尊重。

---

## 3. 各里程碑详情

### M0 · 地基
- 搭建 monorepo（Cargo + pnpm workspace + 构建编排）。
- `nexus-core`：SQLite 存储 + 迁移、ingest 管线骨架、混合检索（向量+FTS+RRF）、本地嵌入（ONNX bge-small）、媒体加密。
- `nexus-protocol`：本地服务、鉴权/scope、`/v1/memories`、`/v1/search`、事件订阅。
- 平台适配 trait 定义（截屏/热键/录音/后台）。
- 完成标志：能通过协议写入一条记忆并检索到；核心有测试覆盖。

### M1 · 中枢 MVP（Orbit 桌面）
- `@nexus/ui` 设计系统雏形。
- Orbit 桌面：检索中心、集合管理、手动新建/编辑记忆、时间线。
- Orbit 作为本地服务持有者 + 单实例仲裁。
- 完成标志：用户能把 Orbit 当本地知识库用起来。

### M2 · 捕获层接入
- **Echo**：全局热键截图 → 本地 OCR → 入库 → 在 Orbit 检索到；桌面权限引导。
- **Muse**：热键秒唤起浮层 → 文字速记 → 入库；轻量冷启动优化。
- **Quill**：Markdown 编辑器（复用 packages/editor）→ 保存即切块嵌入入库 → 双链雏形。
- 完成标志：Echo 抓的、Muse 记的、Quill 写的，都能在 Orbit 里统一检索——「内联」跑通。

### M3 · 智能化
- 知识卡片生成（Memory → card，derived_from 关联）。
- 间隔复习：FSRS、复习队列、到期提醒（review.due 事件）。
- RAG 问答（`/v1/ask` + Completion Provider，带引用）。
- 云 AI Provider 接入（Claude/OpenAI/自定义端点，自带 Key，数据最小化护栏）。
- Muse 语音转写（Transcriber）、Echo AI 结构化理解与敏感检测。

### M4 · 多端
- Orbit 移动端（iOS/Android），优先验证 Tauri 移动关键路径。
- E2E 云同步（CRDT + 加密中继）、自托管中继选项。
- 多设备配对、恢复短语、可证删除。
- Muse 移动端（理想速记场景）。

### M5 · 外联
- MCP Server：把记忆库暴露给任意 AI 助手。
- `@nexus/sdk-ts`、`nexus-sdk`(Python)、`nexus` CLI。
- 浏览器扩展（native messaging 一键剪藏，source=external:clipper）。
- Orbit「连接与隐私」面板：授权应用、scope、令牌撤销、数据流向可视。

---

## 4. 依赖关系图

```
M0 core/protocol
   │
   ├──► M1 Orbit(桌面/本地服务) ──┬──► M3 智能化 ──► M4 多端
   │                              │
   └──► M2 Echo/Muse/Quill ───────┘
                                   └──► M5 外联(依赖协议成熟)
```

---

## 5. 团队与分工建议

| 方向 | 关注点 |
|------|--------|
| 核心/Rust | nexus-core、协议、加密、同步、平台适配 |
| AI | 本地推理集成、Provider 抽象、检索质量、卡片/RAG 质量 |
| 前端/设计系统 | @nexus/ui、四款 App 前端、编辑器 |
| 平台原生 | 桌面截屏/热键、移动端外壳与后台、权限 |

初期可用较小团队按里程碑串行推进（地基→中枢→捕获），M3 起并行铺开。

---

## 6. 主要风险与缓解（汇总）

| 风险 | 里程碑 | 缓解 |
|------|--------|------|
| Tauri 移动端成熟度 | M4 | 提前在 M1 做移动端可行性验证 spike；平台适配层隔离；必要时局部原生 |
| 本地共享库并发/可靠性 | M1–M2 | 单一持有者 + 协议中介写串行化；健康检查与仲裁移交 |
| 嵌入模型切换导致向量不兼容 | M3 | 后台重嵌入任务；版本标记向量空间 |
| E2E 加密与检索矛盾 | M4 | 检索恒在本地明文；云端零知识只做中继 |
| 屏幕/语音权限（各 OS 差异） | M2–M3 | 首启引导；无权限降级为手动模式 |
| 云 AI 成本与隐私 | M3 | 自带 Key、数据最小化、可纯本地 |

---

## 7. 建议的第一步（Sprint 0）

1. 初始化 monorepo 骨架（crates + apps + packages 目录）。
2. `nexus-core` 跑通「写入一条 Memory → 生成向量 → 混合检索命中」的最小闭环（可先命令行验证）。
3. Memory Protocol 本地服务 `/v1/memories` + `/v1/search` 打通。
4. 一个最小 Tauri 壳验证前端经 IPC 调 core。

完成这四步，就有了承载全部产品家族的地基。
