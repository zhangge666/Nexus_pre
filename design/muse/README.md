# Muse 界面与图标设计

本目录是 Muse 全面产品化阶段的第一版界面基线。设计目标不是把 Muse 做成另一个复杂工作台，而是让它保持为一个**通过快捷键直达、完成后立即离开的精致小助手**。

对应功能说明见 [`docs/apps/muse.md`](../../docs/apps/muse.md)。

## 1. 设计主张

- **小**：灵感和启动入口使用真正的小浮层；任务、会议、剪贴板只在需要更多上下文时展开。
- **快**：四类高频功能均预留独立快捷键，不要求先进入首页。
- **静**：默认暗色、低对比边框、一个主色动作，不使用大卡片、渐变背景或装饰性数据。
- **可追溯**：任务详情把原始要求、提出人、附件、进展和交付结果放在同一时间线。
- **本地感**：录音和剪贴板明确显示本地状态，临时剪贴板不会自动同步。

## 2. 文件结构

```text
design/muse/
├── index.html                  可交互界面原型
├── muse.css                   完整界面样式与响应式规则
├── muse.js                    页面切换和会议模式交互
├── icons/
│   ├── muse-app-icon.svg      Muse“星芒 M”应用图标源文件
│   ├── muse-app-icon-*.png    32–1024px PNG 导出
│   ├── idea.svg               灵感
│   ├── task-trace.svg         任务留痕
│   ├── meeting.svg            会议
│   ├── clipboard-compare.svg  剪贴板比较
│   └── hotkeys.svg            快捷键
└── screens/
    ├── muse-launcher.png
    ├── muse-idea.png
    ├── muse-task-trace.png
    ├── muse-meeting-live.png
    ├── muse-meeting-summary.png
    ├── muse-clipboard-compare.png
    └── muse-hotkeys.png
```

## 3. 界面预览

### 快捷启动条

![Muse 快捷启动条](./screens/muse-launcher.png)

四个入口同层级展示，字母键提示对应独立快捷功能；最右侧只保留快捷键设置。

### 灵感捕捉

![Muse 灵感捕捉](./screens/muse-idea.png)

只有标题、输入区、三个轻工具和一个“收好”动作。草稿状态与写入范围退到弱层级。

### 任务与工作留痕

![Muse 任务与工作留痕](./screens/muse-task-trace.png)

左侧是紧凑任务列表，右侧只呈现选中任务。原始要求独立成证据块，后续文件、进展、完成与复开进入同一时间线。

### 会议录音与摘要

![Muse 会议实时转写](./screens/muse-meeting-live.png)

![Muse 会后摘要](./screens/muse-meeting-summary.png)

录音页强调计时、状态、实时转写和重点标记；停止后切换为可回链时间戳的摘要与待确认行动项。

### 剪贴板比较

![Muse 剪贴板比较](./screens/muse-clipboard-compare.png)

左侧选择复制条目，右侧以等宽双栏突出逐词差异。底部明确提示“仅本机”，并提供绑定任务或主动保存。

### 快捷键设置

![Muse 快捷键设置](./screens/muse-hotkeys.png)

五个入口分别绑定，不把快捷键藏在多层设置中；冲突结果在页面底部原位反馈。

## 4. 本地预览

在本目录启动任意静态服务器：

```powershell
python -m http.server 4178 --bind 127.0.0.1
```

然后打开：

```text
http://127.0.0.1:4178/
```

也可以使用查询参数直达界面：

```text
?view=launcher
?view=idea
?view=tasks
?view=meeting
?view=clipboard
?view=settings
```

原型支持：

- 顶部标签切换；
- 启动条直接进入对应功能；
- 会议页切换“实时转写 / 会后摘要”；
- 非输入状态下使用 `I / T / R / V` 快速切换；
- `Esc` 返回快捷启动条。

## 5. 实现映射

原型进入 `apps/muse` 时建议按窗口而不是页面路由拆分：

| 原型 | 建议窗口/路由 | 首要能力 |
|---|---|---|
| 启动条 | `launcher` | 热键路由与焦点管理 |
| 灵感 | `capture/idea` | 草稿、本地队列、Memory 写入 |
| 任务 | `work/tasks` | 任务根记录、来源绑定、活动时间线 |
| 会议 | `meeting/live`、`meeting/summary` | 录音、转写、摘要与行动项 |
| 剪贴板 | `clipboard/compare` | 本地历史、置顶、差异比较 |
| 快捷键 | `settings/hotkeys` | 冲突检测与重新绑定 |

界面实现时继续复用 `@nexus/ui` token；原型中的业务文案与数据仅用于展示，不代表后端能力已经完成。
