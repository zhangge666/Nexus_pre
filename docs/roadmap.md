# Nexus 开发路线图

本文档描述交付顺序、里程碑与风险。核心策略：**先把中枢和地基打牢，完成 Orbit 的独立产品闭环；再用一个 Muse 最小来源验证跨应用接入，随后继续 Orbit 的智能化、Android 移动端与外联，再产品化 Echo、Muse、Quill，最后收口 Orbit iOS**。

---

## 1. 交付哲学

- **地基先行**：`nexus-core` + Memory Protocol 是四款软件共同的地基，必须先稳定。
- **中枢先立**：Orbit 是记忆库本体，它（或独立 daemon）持有本地记忆服务。没有中枢，捕获层无处可写。
- **Orbit 完整性门槛**：M2 专用于完成 Orbit；在 Orbit 的核心流程、真实 core/Protocol 对接、稳定性与验收未完成前，不启动 Echo、Muse、Quill 的产品开发或内联对接。
- **单源验证优先**：M3 不并行开发三款捕获软件，只把 Muse 当作 Orbit 之外的最小参考来源，验证一次真实的跨进程写入与回显闭环。
- **产品族后置**：Echo、Muse、Quill 的完整产品功能统一放到 M7；M3 的 Muse 代码是接入样例，不等同于 Muse MVP。
- **iOS 最后收口**：当前移动端开发只推进 Android；Orbit iOS 不在 M5–M7 初始化、开发或验收，统一放到 M8 作为现有路线图的最终平台收口。
- **纵向切片**：每个里程碑交付一条可用的端到端链路，而非横向堆半成品。
- **本地优先落地**：先把纯本地体验做扎实，再叠加同步与云端 AI。

---

## 2. 里程碑总览

```
M0 地基      M1 中枢MVP     M2 Orbit完善   M3 单源验证   M4 智能化      M5 Android    M6 外联       M7 产品族      M8 iOS
─────────►  ─────────►    ─────────►    ─────────►   ─────────►    ─────────►    ─────────►    ─────────►    ─────────►
core/协议    Orbit+库      完整闭环       Muse最小源    卡片/复习/    Android版     MCP/SDK/     Echo/Muse/     Orbit iOS
数据模型     检索+桌面     稳定验收       跨进程接入    RAG/云AI      +同步         第三方        Quill产品化   最终收口
```

| 里程碑 | 目标 | 产出 |
|--------|------|------|
| **M0 地基** | 核心与协议可用 | `nexus-core`（store/ingest/search/embed/crypto 基础）、Memory Protocol 本地服务、数据模型与迁移 |
| **M1 中枢 MVP** | 记忆库能用起来 | Orbit 桌面版：混合检索、集合、手动记忆、本地服务持有者；本地嵌入模型 |
| **M2 Orbit 完善** | 中枢达到独立交付标准 | Orbit 核心流程闭环、真实 core/Protocol 对接、桌面稳定性与验收；不开发其他产品 |
| **M3 单来源接入验证** | 证明 Orbit 能承接独立应用来源 | Muse 最小文字来源：连接本地服务、授权写入、事件刷新，并在 Orbit 检索/时间线/来源信息中可见 |
| **M4 智能化** | 第二大脑成形 | 知识卡片生成、间隔复习(FSRS)、RAG 问答、云 AI Provider 接入 |
| **M5 Android 多端** | Orbit 先在 Android 上随身可用 | Orbit Android 客户端、E2E 云同步、多设备配对与恢复短语 |
| **M6 外联** | 开放生态 | MCP Server、TS/Python SDK、浏览器扩展剪藏、连接与隐私管理面板 |
| **M7 产品族扩展** | 完整交付三款兄弟软件 | Echo、Muse、Quill 桌面产品化及其与 Orbit 的深度内联；移动端能力按各产品计划跟进 |
| **M8 Orbit iOS 收口** | 最后补齐 Orbit iOS | 复用已稳定的移动共享层，完成 iOS 外壳、平台能力、真机验收与发布准备 |

> M0→M1→M2→M3 为严格前置顺序。M3 只允许 Muse 最小来源适配器，不启动 Muse 完整产品，也不启动 Echo、Quill；完成单源验证后按 M4→M5→M6→M7 推进，Orbit iOS 统一在最后的 M8 开发与验收。

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

### M2 · Orbit 完善与独立交付
- 完成 Orbit 桌面核心流程：检索、集合、记忆创建与编辑、时间线及状态反馈均以真实 `nexus-core` / Memory Protocol 数据运行。
- 完成 Orbit 作为本地服务持有者的可靠性闭环：单实例仲裁、启动与退出恢复、事件刷新、失败提示与基础诊断。
- 完成 Orbit 工作台、详情检查器、快速记录、键盘可达性与响应式等产品体验收敛，并通过桌面验收。
- 完成 Orbit 核心工作流的自动化测试、构建检查与发布前回归；已知阻断问题必须清零或明确降级方案。
- **范围约束**：本里程碑不新增 Echo、Muse、Quill 的产品功能，不进行三款产品的内联对接；仅可保留其既有协议契约与测试桩。
- 完成标志：用户无需安装其他 Nexus 产品，即可稳定地将 Orbit 作为本地知识库与中枢使用；Orbit 已具备承接后续捕获层写入的稳定接口。

### M3 · Muse 最小来源接入（M2 完成后）
- 在 `apps/muse` 只实现一个最小文字来源：单一输入框、提交状态和连接状态；不在本阶段追求完整 Muse UI。
- Muse 通过 Memory Protocol 连接 Orbit 持有的本地服务，完成应用登记/授权，并以 `memory:write` scope 写入 `source=muse`、`kind=idea` 的 Memory。
- Orbit 必须通过事件订阅即时收到写入，并在检索、时间线、详情来源与连接管理中正确展示该记忆。
- 覆盖服务未启动、授权失败、写入失败和重试等最小失败路径；保留一条自动化端到端测试验证“从 Muse 写入 → Orbit 可检索”。
- **明确不做**：全局热键、托盘常驻、失焦保存、语音转写、收件箱、冷启动专项、移动端，以及任何 Echo/Quill 产品功能。
- `source=muse` 表示 Nexus 一方来源；本阶段不伪装为 `external:muse`。M6 的第三方应用仍使用 `source=external:<app_id>`。
- 完成标志：无需手工导入，Muse 最小来源写入的一条文字记忆能即时出现在 Orbit，并可按来源追踪、撤销授权和再次连接。

### M4 · 智能化
- 知识卡片生成（Memory → card，derived_from 关联）。
- 间隔复习：FSRS、复习队列、到期提醒（review.due 事件）。
- RAG 问答（`/v1/ask` + Completion Provider，带引用）。
- 云 AI Provider 接入（Claude/OpenAI/自定义端点，自带 Key，数据最小化护栏）。
- **当前实现（2026-07-17）**：已打通 Orbit 桌面端卡片创建/生成、持久化 ReviewState、
  基于 FSRS-4.5 官方默认权重的四档评分、到期队列与定时系统桌面通知、带块级引用的 RAG
  及 `/v1/ask/stream` 服务端流式输出；Completion 支持 local/Ollama/Claude/OpenAI/custom。
  云端 Key 保存于系统凭据库而非设置文件，Ollama 与回环 custom 不外发数据，远程 Provider
  只接收本地筛选后的必要片段。

### M5 · Android 多端
- 本阶段只开发 Orbit Android 客户端；不初始化、不开发、不构建或验收 iOS 工程。
- Orbit Android 作为随身客户端：本地仅保留受系统安全区保护的加密缓存与离线浏览能力；不持有对外 Memory Protocol 服务、不监听本地端口，也不加载本地 ONNX 嵌入模型。
- E2E 云模式的记忆写入、编辑、集合整理与删除先进入本机加密副本和 oplog，再经 HTTPS 上传签名密文；列表、详情和关键词检索在本机解密完成。复习、卡片、语义检索与 AI 问答仅由自托管 Memory Protocol 模式提供。
- E2E 云同步（CRDT + 加密中继）、自托管中继选项。
- 多设备配对、恢复短语、可证删除。

> **Android 实施状态（2026-07-26）**：移动外壳、Android Keystore、AES-GCM 本地副本、独立
> `nexus-sync` 密码学核心与 `nexus-relay` 零知识中继已经完成。E2E 云模式已接通记忆/集合的离线写入、
> 签名密文上传、增量拉取、版本向量合并、冲突留痕、游标确认和墓碑删除；24 词恢复短语、二维码配对、
> 配对包领取与设备撤销也已接入 Android 设置。Rust arm64 交叉检查、前端 Android 构建和双设备中继契约
> 测试通过。M5 剩余项是 WorkManager 后台时效、真机/平板完整交互与弱网验收，以及最终 APK 重建、签名
> 和发布流水线。详细状态见 [m5-mobile.md](m5-mobile.md#7-android-当前交付状态2026-07-26)。

### M6 · 外联
- MCP Server：把记忆库暴露给任意 AI 助手。
- `@nexus/sdk-ts`、`nexus-sdk`(Python)、`nexus` CLI。
- 浏览器扩展（native messaging 一键剪藏，source=external:clipper）。
- Orbit「连接与隐私」面板：授权应用、scope、令牌撤销、数据流向可视。

> **实施状态（2026-07-26）**：MCP stdio Server、TypeScript/Python SDK、`nexus` CLI、
> Chrome/Edge Native Messaging 剪藏器及 Orbit「连接与隐私」面板均已完成。第三方令牌
> 强制绑定 `external:<app_id>`，正文只展示一次并以 SHA-256 摘要持久化；scope、读写活动、
> 数据流向与撤销均可审计。统一验收与发布边界见 [m6-outreach.md](m6-outreach.md)。

### M7 · 产品族扩展
- **Muse 产品化**：全局热键秒唤起、失焦/回车保存、托盘常驻、轻量冷启动、收件箱与本地语音转写；M3 最小来源升级而不是另起协议。
- **Echo 产品化**：全局热键截图、本地 OCR、预览确认、敏感信息保护与入库，并在 Orbit 中检索和治理来源。
- **Quill 产品化**：Markdown 编辑器、保存即切块入库、双链/反向链接，以及“送去 Orbit 复习”的协作入口。
- Echo、Muse、Quill 均复用已经在 M3/M6 验证成熟的 Memory Protocol、授权、事件与来源治理能力，不复制 Orbit 的检索、卡片或复习功能。
- 完成标志：三款兄弟软件各自达到可独立使用的桌面 MVP，并都能稳定写入 Orbit；高级增强和移动端按各产品文档继续演进。

### M8 · Orbit iOS 收口（最后阶段）
- 在 Android 移动链路、外联能力和产品族协作稳定后，再初始化 Orbit iOS 工程。
- 复用 M5 已稳定的页面、移动外壳、远程 HTTPS 传输、加密缓存、E2E 同步、设备配对与恢复能力，仅在平台边界实现 WKWebView、Keychain、本地通知、安全区和系统返回等 iOS 差异。
- 完成 iPhone/iPad 真机关键路径、签名、权限、后台限制、性能与发布前验收；此前阶段不为 iOS 单独开发条件分支或维护并行实现。
- 完成标志：Orbit iOS 在不回退 Android 和桌面能力的前提下，通过“同步 → 离线查看 → 写入/编辑 → 复习 → 问答 → 通知 → 恢复”的真机闭环。

---

## 4. 依赖关系图

```
M0 core/protocol
   └──► M1 Orbit(桌面/本地服务)
          └──► M2 Orbit完善与验收
                 └──► M3 Muse最小来源
                        └──► M4 智能化 ──► M5 Android ──► M6 外联 ──► M7 Echo/Muse/Quill产品化 ──► M8 Orbit iOS
```

---

## 5. 团队与分工建议

| 方向 | 关注点 |
|------|--------|
| 核心/Rust | nexus-core、协议、加密、同步、平台适配 |
| AI | 本地推理集成、Provider 抽象、检索质量、卡片/RAG 质量 |
| 前端/设计系统 | @nexus/ui、四款 App 前端、编辑器 |
| 平台原生 | 桌面截屏/热键、移动端外壳与后台、权限 |

初期可用较小团队按里程碑串行推进（地基→中枢 MVP→Orbit 完善→Muse 单源验证）。M3 通过后继续集中完成 Orbit 的 M4–M6；不要因为接入样例已经存在，就提前扩张为三款兄弟软件的并行开发。Echo、Muse、Quill 的产品团队工作统一在 M7 展开，Orbit iOS 则在 M8 最后收口。

---

## 6. 主要风险与缓解（汇总）

| 风险 | 里程碑 | 缓解 |
|------|--------|------|
| Tauri 移动端成熟度 | M5、M8 | M5 先验证并稳定 Android；平台适配层隔离；M8 再处理 iOS 差异，必要时局部原生 |
| 本地共享库并发/可靠性 | M1–M3 | 单一持有者 + 协议中介写串行化；健康检查与仲裁移交 |
| 嵌入模型切换导致向量不兼容 | M4 | 后台重嵌入任务；版本标记向量空间 |
| E2E 加密与检索矛盾 | M5 | 检索恒在本地明文；云端零知识只做中继 |
| M3 样例膨胀为完整 Muse | M3 | 以端到端验收清单锁定范围；热键、语音、托盘、移动端全部留到 M7 |
| 屏幕/语音权限（各 OS 差异） | M7 | 首启引导；无权限降级为手动模式 |
| 云 AI 成本与隐私 | M4 | 自带 Key、数据最小化、可纯本地 |

---

## 7. 建议的第一步（Sprint 0）

1. 初始化 monorepo 骨架（crates + apps + packages 目录）。
2. `nexus-core` 跑通「写入一条 Memory → 生成向量 → 混合检索命中」的最小闭环（可先命令行验证）。
3. Memory Protocol 本地服务 `/v1/memories` + `/v1/search` 打通。
4. 一个最小 Tauri 壳验证前端经 IPC 调 core。

完成这四步，就有了承载全部产品家族的地基。
