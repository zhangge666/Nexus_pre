# Nexus 设计系统 · 暗色高级简约（Linear 风）

本文档是 Nexus 产品家族**统一的视觉规范**。四款软件（Echo/Muse/Quill/Orbit）与所有前端 agent 都以此为唯一依据。

> **给 agent 的第一原则：一切颜色、间距、圆角、动效都走本文件的 CSS 变量 token，禁止在组件里硬编码颜色值（如 `#fff`、`bg-black`）。** 需要新色时先在此登记 token，再引用。

风格一句话：**参照 Linear —— 暗色优先、低饱和、高信息密度、靠细边框而非阴影分层、单一强调色点睛、微交互快而克制。**

---

## 1. 技术选型

| 层 | 选型 | 说明 |
|----|------|------|
| 组件底座 | **shadcn/ui**（Radix UI + Tailwind） | 代码复制进项目、完全自有，暗色改造自由；落地为 `@nexus/ui`（见 [architecture.md](architecture.md) §3） |
| 无样式原语 | **Radix UI Primitives** | 可访问性（键盘/焦点/ARIA）满分，契合无障碍要求 |
| 样式引擎 | **Tailwind CSS** | 通过 CSS 变量承载全部 token（§2） |
| 字体 | **Inter**（备选 Geist） | 几何无衬线，Linear 同源气质 |
| 图标 | **Lucide** | shadcn 默认，线性风格统一 |
| 图表（Orbit 仪表盘） | **Tremor / Recharts** | 复习曲线、记忆增长等；暗色开箱即用 |
| 主题调参 | **tweakcn** | 可视化调 shadcn 暗色主题、导出变量（仅开发期辅助） |

> 特效点缀（发光边框、渐变光晕）可按需从 Aceternity/Magic UI 取单个组件，**不整套引入**，避免风格发散。

---

## 2. 设计 Token（唯一真相）

所有 token 以 CSS 变量定义在 `packages/ui` 的全局样式里，Tailwind 通过 `hsl(var(--x))` 引用。**暗色为默认主题**，亮色为可选（见 §2.5）。

### 2.1 颜色 · 背景与层级

Linear 的精髓：背景不是纯黑，而是带冷调的近黑灰，层级靠**极细微的明度差**拉开。

| Token | HSL | 近似 Hex | 用途 |
|-------|-----|---------|------|
| `--background` | `220 13% 4%` | `#08090A` | 应用最底层背景 |
| `--surface` | `220 13% 6%` | `#0D0E10` | 卡片/面板背景（比底层微亮） |
| `--surface-elevated` | `220 12% 9%` | `#141518` | 悬浮层：弹窗、下拉、tooltip |
| `--surface-hover` | `220 11% 12%` | `#1B1D21` | hover 态背景 |
| `--muted` | `220 10% 14%` | `#1F2126` | 次要区块、禁用背景 |

> 层级规则：**用背景明度差 + 1px 边框区分层级，不用大阴影**（见 §2.6）。

### 2.2 颜色 · 文字

| Token | HSL | 近似 Hex | 用途 |
|-------|-----|---------|------|
| `--foreground` | `220 15% 96%` | `#F2F3F5` | 主文字（不用纯白，降眩光） |
| `--foreground-secondary` | `220 9% 70%` | `#AEB1B8` | 次要文字、说明 |
| `--foreground-muted` | `220 8% 48%` | `#71757E` | 占位符、时间戳、弱提示 |
| `--foreground-disabled` | `220 7% 34%` | `#4E525A` | 禁用文字 |

### 2.3 颜色 · 边框与分隔

| Token | HSL / 值 | 用途 |
|-------|---------|------|
| `--border` | `220 10% 100% / 0.08` | 默认 1px 分隔（白色 8% 透明） |
| `--border-strong` | `220 10% 100% / 0.14` | 强调分隔、输入框聚焦前 |
| `--border-subtle` | `220 10% 100% / 0.05` | 极弱分隔（列表内分行） |
| `--ring` | `234 56% 60%` | 焦点环（用强调色） |

> **边框优先原则**：分隔、卡片轮廓、输入框一律用 1px 低对比边框。

### 2.4 颜色 · 强调色与语义色

整体灰阶，**只有一个紫靛强调色**点睛（Linear 招牌）。

| Token | HSL | 近似 Hex | 用途 |
|-------|-----|---------|------|
| `--primary` | `234 56% 60%` | `#5E6AD2` | 主强调：主按钮、链接、选中、焦点 |
| `--primary-hover` | `234 56% 65%` | `#727DDA` | 强调 hover |
| `--primary-foreground` | `0 0% 100%` | `#FFFFFF` | 强调色上的文字 |
| `--success` | `142 44% 52%` | `#4CAF6E` | 成功/已同步 |
| `--warning` | `38 92% 58%` | `#F5A623` | 警告/敏感提示 |
| `--danger` | `0 65% 58%` | `#DC4C4C` | 危险/删除 |
| `--info` | `234 56% 60%` | `#5E6AD2` | 信息（复用 primary） |

> 语义色只在必要时出现（状态、提示），**大面积区域保持灰阶**，靠强调色制造焦点。

### 2.5 亮色主题（可选）

亮色为 opt-in。同名 token 在 `:root:not(.dark)` 下给亮色值（背景 `#FFFFFF`→`#F7F8F9` 层级、文字近黑、边框黑色 8%、强调色 `#5E6AD2` 不变）。组件只引用 token，**切换主题无需改组件**。

### 2.6 阴影（克制）

| Token | 值 | 用途 |
|-------|-----|------|
| `--shadow-sm` | `0 1px 2px 0 rgb(0 0 0 / 0.3)` | 悬浮层轻微抬升 |
| `--shadow-md` | `0 4px 12px -2px rgb(0 0 0 / 0.4)` | 弹窗/下拉 |
| `--shadow-glow` | `0 0 0 1px var(--border-strong), 0 8px 24px -4px rgb(0 0 0 / 0.5)` | 模态最高层 |

> 暗色下阴影感弱，**优先用边框和背景明度差表达层级**，阴影仅用于真正悬浮的元素。

### 2.7 圆角

| Token | 值 | 用途 |
|-------|-----|------|
| `--radius-sm` | `4px` | 标签、徽标、输入内小元素 |
| `--radius` | `6px` | 按钮、输入框、下拉项（默认） |
| `--radius-md` | `8px` | 卡片、面板 |
| `--radius-lg` | `12px` | 弹窗、大容器 |

> 整体偏小圆角，克制而精确，避免圆润卡通感。

---

## 3. 排版

| 用途 | 字号 | 行高 | 字重 | 说明 |
|------|------|------|------|------|
| 正文 | **13–14px** | 1.5 | 400 | Linear 式偏小、高密度 |
| 次要/说明 | 12px | 1.5 | 400 | 用 `--foreground-secondary` |
| 小字/时间戳 | 11px | 1.4 | 400 | 用 `--foreground-muted` |
| H3 / 卡片标题 | 15px | 1.4 | 600 | |
| H2 / 区块标题 | 18px | 1.3 | 600 | |
| H1 / 页面标题 | 22–24px | 1.2 | 600 | 少用 |

- **字体族**：`Inter, -apple-system, "Segoe UI", system-ui, sans-serif`；代码用 `"Geist Mono", "JetBrains Mono", monospace`。
- **字间距**：标题略收紧 `letter-spacing: -0.01em`；正文默认。
- 开启 `font-feature-settings: "cv11", "ss01"`（Inter）让数字/字形更精致。

---

## 4. 间距与栅格

采用 4px 基准刻度（Tailwind 默认对齐）：`2 / 4 / 8 / 12 / 16 / 24 / 32 / 48`（px）。

- **紧凑但透气**：控件内边距偏小（按钮 `padding: 6px 12px`），但模块间留白充足。
- 列表行高约 `32–36px`；侧边栏项 `28–32px`。
- 内容最大宽度：阅读态（Quill/记忆详情）约 `720px`，避免长行。

---

## 5. 组件基线规范

| 组件 | 关键规范 |
|------|---------|
| 按钮 | 主按钮 `--primary` 实心 + 白字；次按钮 `--surface` + `--border`；幽灵按钮透明 + hover 显 `--surface-hover`。高度 32px，圆角 `--radius` |
| 输入框 | `--surface` 背景 + `--border`，聚焦时 `--border` → `--ring` + 2px 焦点环；高度 32–36px |
| 卡片/面板 | `--surface` + 1px `--border` + `--radius-md`，**默认无阴影** |
| 弹窗/下拉 | `--surface-elevated` + `--shadow-md` + `--radius-lg`；Radix 驱动焦点陷阱与键盘 |
| 分隔线 | 1px `--border-subtle`，绝不用粗线 |
| 徽标/标签 | `--muted` 背景 + `--foreground-secondary`，`--radius-sm`，11–12px |
| 选中态 | 背景 `--primary` 10–15% 透明叠加，左侧可加 2px `--primary` 指示条 |
| 滚动条 | 细（6–8px）、半透明、hover 才明显（自定义 `::-webkit-scrollbar`） |

---

## 6. 动效与交互

Linear 的手感：**快、轻、克制**。

| 场景 | 时长 | 缓动 |
|------|------|------|
| hover / 颜色过渡 | 100–150ms | `ease-out` |
| 下拉/弹窗进出 | 150–200ms | `cubic-bezier(0.16, 1, 0.3, 1)` |
| 位移/展开 | 200ms | 同上，位移幅度小（≤8px） |

- 禁止夸张弹跳、长时长动画。
- 遵守 `prefers-reduced-motion`：用户开启后关闭非必要动效（无障碍要求）。
- 焦点可见：键盘导航必须有清晰 `--ring` 焦点环，不为美观移除 outline。

---

## 7. 落地方式（`packages/ui`）

1. `@nexus/ui` 用 shadcn/ui 模式初始化，把上述 token 写入全局 CSS（`:root.dark { --background: ...; }`），Tailwind `theme.extend.colors` 映射到 `hsl(var(--token))`。
2. 四款 app 的前端只从 `@nexus/ui` 引组件、只用 token 类名（如 `bg-surface text-foreground border-border`），**不写死颜色**。
3. 暗色为默认（`<html class="dark">`），亮色 opt-in。
4. 新增视觉元素前，先确认能否用现有 token 组合；确需新 token 则登记到本文件再用。

```css
/* packages/ui/src/styles/tokens.css —— 摘要示意 */
:root.dark {
  --background: 220 13% 4%;
  --surface: 220 13% 6%;
  --surface-elevated: 220 12% 9%;
  --foreground: 220 15% 96%;
  --foreground-secondary: 220 9% 70%;
  --border: 220 10% 100% / 0.08;
  --primary: 234 56% 60%;
  --ring: 234 56% 60%;
  --radius: 6px;
  /* …其余见 §2 */
}
```

---

## 8. 给 agent 的风格描述模板（可直接复制到任务里）

> UI 遵循 `docs/design-system.md`。视觉风格参照 Linear：暗色优先、高级简约、低饱和。背景用带冷调的近黑灰做多层级（非纯黑），分隔靠 1px 低对比边框而非阴影，单一紫靛强调色（`--primary` #5E6AD2）点睛。字体 Inter，正文 13–14px，信息密度高但间距讲究。微交互 100–150ms、位移克制，遵守 `prefers-reduced-motion`。技术上用 shadcn/ui + Radix + Tailwind，所有颜色/间距/圆角走 CSS 变量 token，**禁止硬编码颜色值**。键盘可达、焦点环清晰。

---

## 9. 与各文档的关系

- 组件库选型与代码组织（`@nexus/ui`、`packages/`）→ [architecture.md](architecture.md) §2.3 / §3
- 各 app 前端章节实现 UI 时引用本规范 → `apps/*.md` §5
- 无障碍要求贯穿：可访问组件（Radix）、焦点可见、reduced-motion
