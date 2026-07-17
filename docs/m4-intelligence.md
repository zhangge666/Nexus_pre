# Orbit M4 智能化实现与运行

本文档记录 M4 最小可用纵向切片的代码边界、运行方式和验收入口。

## 1. 已实现闭环

1. 任意 Memory 可生成或手动创建 `source=orbit`、`kind=card` 的知识卡片。
2. 卡片通过 `derived_from` 回链来源，并立即建立 `state=new` 的 ReviewState。
3. FSRS 调度支持 Again、Hard、Good、Easy，持久化稳定度、难度、到期时间和评分日志。
   Orbit 服务持有者每分钟扫描一次到期状态，并由核心去重发布 `review.due`。
4. `/v1/ask` 先在本地执行混合检索和 scope 过滤，再调用 Completion 并返回块级引用。
5. Completion 支持本地抽取式回退、Claude、OpenAI 和 OpenAI-compatible 自定义端点。
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
(RAG)」可以选择 Completion Provider；默认 `local` 无网络。云 Provider 的 API Key 不写入
磁盘，应用重启后需重新输入。

## 3. 使用路径

- **卡片**：进入「卡片 → 新建卡片」，选择手动创建或从一条来源记忆生成。
- **复习**：进入「复习」，翻面后用按钮或数字键 1–4 评分；失败时当前卡片不会前进。
- **问答**：进入「问答」，回答中的引用可跳回对应 Memory。云 Provider 启用发送前确认时，
  UI 会在调用前说明目标 Provider 和最小化边界。
- **Provider**：`local` 不外发；Claude/OpenAI/custom 只收到最多 6 条本地检索片段，
  无命中时不会发起云请求。

## 4. 自动验收

```powershell
pnpm verify:orbit-m4
```

该命令覆盖 `nexus-ai`、`nexus-core`、`nexus-protocol`、Orbit Rust IPC、前端类型检查、
前端构建和相关 Rust Clippy。发布前再执行 workspace 全量测试与构建。
