# Orbit M4 智能化实现与运行

本文档记录 M4 最小可用纵向切片的代码边界、运行方式和验收入口。

## 1. 已实现闭环

1. 任意 Memory 可生成或手动创建 `source=orbit`、`kind=card` 的知识卡片。
2. 卡片通过 `derived_from` 回链来源，并立即建立 `state=new` 的 ReviewState。
3. FSRS 调度使用 FSRS-4.5 官方默认权重和 90% 目标可回忆率，支持 Again、Hard、Good、Easy，
   并持久化稳定度、难度、到期时间和评分日志。Orbit 服务持有者每分钟扫描一次到期状态，
   由核心去重发布 `review.due`，且每天在设置的提醒时间通过系统桌面通知汇总到期卡片。
4. `/v1/ask` 先在本地执行混合检索和 scope 过滤，再调用 Completion 并返回块级引用；
   `/v1/ask/stream` 以 SSE 依次返回 `meta`、`delta`、`done`（或 `error`）事件。
5. Completion 支持本地抽取式回退、Ollama 本地 LLM、Claude、OpenAI 和 OpenAI-compatible
   自定义端点。Ollama 与回环自定义端点不外发数据，云 Provider 仍遵守最小化上下文边界。
6. Orbit 卡片、复习、问答和设置页面均通过 Tauri IPC 调真实 Memory Protocol。

## 2. 本地运行

安装依赖后启动 Orbit 前端预览：

```powershell
pnpm install
pnpm --filter @nexus/orbit dev
```

浏览器打开 `http://127.0.0.1:1420/`。浏览器预览使用 mock 数据，仅用于 UI 与响应式检查。

运行包含 SQLite、Memory Protocol 和 Tauri IPC 的真实桌面应用：

```powershell
pnpm --filter @nexus/orbit tauri dev
```

首次启动由 Orbit 持有本地服务，数据库位于产品族共享数据目录。进入「设置 → 问答
(RAG)」可以选择 Completion Provider；默认 `local` 无网络，选择 `ollama` 后默认访问
`http://127.0.0.1:11434/api/chat`。Claude、OpenAI 与 custom 的 API Key 写入操作系统凭据库，
不会写入 SQLite、设置文件、日志或协议响应，应用重启后会自动恢复；本机 custom 端点可不填 Key。

## 3. 使用路径

- **卡片**：进入「卡片 → 新建卡片」，选择手动创建或从一条来源记忆生成。
- **复习**：进入「复习」，翻面后用按钮或数字键 1–4 评分；失败时当前卡片不会前进。
- **问答**：进入「问答」，开启「实时流式输出」后会显示服务端真实的增量文本，回答中的引用
  可跳回对应 Memory。云 Provider 启用发送前确认时，UI 会在调用前说明目标 Provider 和最小化边界。
- **Provider**：`local`、`ollama` 和回环 custom 不外发；Claude/OpenAI/远程 custom 只收到最多
  6 条本地检索片段，无命中时不会发起云请求。
- **桌面提醒**：进入「设置 → 复习调度」启用提醒并设置时间；Orbit 持有者每天仅发送一次包含
  到期数量的系统桌面通知，避免为同一批卡片重复弹窗。

## 4. 自动验收

```powershell
pnpm verify:orbit-m4
```

该命令覆盖 `nexus-ai`、`nexus-core`、`nexus-protocol`、Orbit Rust IPC、前端类型检查、
前端构建和相关 Rust Clippy。发布前再执行 workspace 全量测试与构建。
