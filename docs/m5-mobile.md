# Orbit M5 Android 移动端开发注意

本文档记录 M5 Android 移动端的技术选型结论、可行性边界、界面适配策略与风险清单，供进入 M5 前对齐。Orbit iOS 保留为最终平台目标，但不在 M5–M7 开发，统一放到路线图最后的 M8 收口。

> 关联：里程碑定义见 [roadmap.md](roadmap.md#m5--android-多端)；跨端与平台适配层设计见 [architecture.md](architecture.md)。

## 0. 一句话结论

M5 用 **Tauri 2.0 + 现有 React/TS/Vite 前端** 先交付 Android，界面**复用页面与 `@nexus/ui`、只分叉布局外壳**。Android 客户端本地负责加密缓存、离线查看和解密展示；写入、编辑、AI 问答均调用远程 HTTPS API，不监听本地端口，也不作为其他 Nexus 软件的服务端。Tauri 能交付"视觉精美 + 过渡丝滑"的效果，但受 WebView 天花板约束；真遇到原生手感瓶颈时，按架构预案把**个别页面**下沉到原生视图，不推翻整体。iOS 在 M8 复用已稳定的移动共享层，不与当前 Android 开发并行。

## 1. 技术选型（已锁定，非开放选择）

- **外壳：Tauri 2.0**。选它而非 Flutter/Electron 的核心理由是共享桌面与移动端的大部分代码；当前只交付 Android，iOS 延后到 M8。见 [architecture.md](architecture.md) §2.1、§2.6。
- **界面：React + TypeScript + Vite**，复用现有 `@nexus/ui` 设计系统与 `core/*`（api、events、types）。
- **渲染**：M5 Android 走系统 WebView（基于 Chromium），**不是原生渲染**——这是当前所有性能判断的前提；WKWebView 相关适配留到 M8。
- **已有脚手架**：`apps/orbit/src-tauri/icons/android/` 的 mipmap 资源、`crates/platform/platform-mobile` 空壳已建好，等 M5 填充。
- **本地边界**：移动端不启动 `nexus-protocol`、不监听回环 HTTP/TCP 端口、不参与桌面应用间仲裁；Tauri IPC 仅供同一 App 的 WebView 与 Rust 侧通信。
- **AI 与编辑**：移动端不链接 ONNX Runtime 或执行本地嵌入。记忆写入、编辑、AI 问答通过携带用户授权的远程 HTTPS API 完成；接口失败时保留输入并给出重试入口。

## 2. Tauri 能否做出"精美 + 丝滑"

能交付，但有天花板，需按下述边界规划。

**WebView 擅长、可放心做的：**
- CSS 合成器动画（`transform`、`opacity`、`cubic-bezier` 过渡）由 GPU 处理，能稳定 60fps，观感接近原生。现有桌面端 `cubic-bezier(0.16, 1, 0.3, 1)` 那套过渡在移动端同样丝滑。
- 精美视觉（圆角、阴影、模糊、渐变、字体）是浏览器强项，WebView 完全胜任。

**天花板与必须警惕的风险：**
- **冷启动**：要拉起 WebView 运行时，首帧比原生 Compose/SwiftUI 慢。用骨架屏遮盖。
- **长列表滚动**：时间线/卡片列表可能上千条，不做虚拟化（react-window / virtua）会掉帧。**最需优先处理的一点。**
- **系统手势**：橡皮筋、边缘返回、共享元素转场，WebView 需手动模拟，难 100% 还原原生手感。
- **JS 主线程**：交互动画与数据计算抢主线程会卡顿——重活儿丢给 Rust 核心层（架构本就如此设计）。

**结论**：追求"视觉精致 + 页面过渡顺滑"→ Tauri 完全够用；追求"逐帧媲美顶级原生手势/转场"→ WebView 会碰天花板。届时按架构 §2.1 预案，把**个别关键页面**下沉到 Jetpack Compose，不推翻整体。对 Orbit 这类以内容/复习为主、交互不重的应用，Tauri 是划算选择。

## 3. 界面适配策略：外壳分叉，页面复用

现有外壳 [App.tsx](../apps/orbit/src/App.tsx) 是彻底的桌面范式（5 列 grid：侧栏 + 拖拽条 + 工作区 + 拖拽条 + 检查器，含 `col-resize` 拖拽、`PointerCapture`、键盘调宽），手机无法直接用。适配的关键是**在哪一层分叉**。

**❌ 禁止：在现有组件里塞 `@media` 和 `if (isMobile)` 分支。**
把移动逻辑混进 `WorkspaceShell`/`Sidebar`/`Inspector`，会让桌面每次改动都可能误伤移动端，反之亦然——这正是"改移动端搞坏桌面端"的根源。

**✅ 采用：外壳分叉 + 页面/核心复用。**
- **共享**（不动）：`pages/*`（TodayPage、ReviewPage、CardsPage、AskPage 等内容页）、`@nexus/ui`、领域类型与缓存展示逻辑。
- **传输层分叉**：桌面继续使用本地 Memory Protocol；移动端使用远程 HTTPS 客户端。写入、编辑和 AI 页只调用远程接口，不能复用桌面本地端口发现或服务持有者逻辑。
- **分叉**（新增）：只有布局外壳分两套。桌面保留现有 `WorkspaceShell`（三栏 + 拖拽）；移动端新写 `MobileShell`（底部 Tab 栏 + 全屏页面 + 手势返回），检查器从"右侧常驻抽屉"改为"底部弹出 sheet"。
- 在入口 `main.tsx` 按 `platform` 判断挂载哪个外壳。**桌面代码路径完全不变，移动端是新增文件而非修改现有文件**，改坏桌面的概率趋近于零。呼应架构 §5：差异隔离在边界层，共享内核不动。

**可控的共享层小改（属增强，桌面同样受益）：**
- `@nexus/ui` 组件保证触摸目标 ≥ 44px、支持触摸事件——是增强不是替换，桌面只会更好用。
- 长列表页（时间线、卡片列表）引入虚拟化，桌面移动都受益。

## 4. 建议路径（进入 M5 的顺序）

1. **先做可行性 spike，别碰界面**：验证 Android 能以不链接 ONNX Runtime 的方式启动受保护的本地缓存，并能经 HTTPS 拉取、解密和展示同步数据；移动端不启动本地 HTTP 服务，也不参加桌面服务持有者仲裁。
2. 界面层采用"外壳分叉 + 页面/核心复用"，**桌面外壳只读不改**。
3. 动画走 CSS 合成器动画；长列表虚拟化；冷启动用骨架屏遮盖。
4. 个别页面 WebView 手感不够时，按架构预案单独下沉到 Jetpack Compose，不影响其余。

### 4.1 Android 开发环境与命令

Android 工程首次初始化前，开发机须按 Tauri 官方前置条件安装 Android Studio、Android SDK Platform、
Platform-Tools、NDK、Build-Tools 与 Command-line Tools，并设置 `ANDROID_HOME`、`NDK_HOME` 和指向
Android Studio JBR 的 `JAVA_HOME`。随后在仓库根目录执行：

```bash
pnpm --filter @nexus/orbit android:init
pnpm --filter @nexus/orbit android:dev
```

初始化生成的 `apps/orbit/src-tauri/gen/android/` 是 Orbit Android 壳的一部分，必须提交；禁止手工修改其
构建工具生成的内容，平台配置应优先放在 Tauri 配置、移动插件或独立原生模块中。移动真机调试时，
`vite.config.mjs` 会使用 Tauri 注入的 `TAURI_DEV_HOST` 配置开发服务器与 HMR；桌面调试仍只监听回环地址。

### 4.2 移动端能力边界

| 能力 | M5 移动端处理方式 |
|---|---|
| 浏览、集合、详情、离线查看 | 从受系统安全区保护的本地加密缓存读取并解密展示 |
| 写入、编辑 | 调用已授权的远程 HTTPS API；失败时保留草稿并可重试 |
| AI 问答 | 调用远程 HTTPS API，沿用桌面端的数据最小化与 Provider 提示规则 |
| 同步与配对 | 经 E2E 加密中继与远程 API 完成，不暴露本机监听端口 |
| 本地协议服务、第三方接入、ONNX 嵌入 | 明确不做；仅桌面 Orbit 持有这些能力 |

## 5. 桌面端回归护栏

- Android 移动端合入后，桌面端 `verify:orbit-m4`（含前端类型检查、构建、Rust 测试与 Clippy）必须保持全绿。
- 任何对 `@nexus/ui` 共享组件的改动，需同时在桌面与移动两套外壳下自测。
- 严禁为移动端在共享组件内引入平台条件分支；平台差异一律走外壳层或 `platform-mobile`。

## 6. iOS 延后边界（路线图最后阶段）

- M5–M7 不执行 `tauri ios init/dev/build`，不创建或维护 Xcode 工程，不处理签名、Provisioning Profile、App Store、WKWebView、Keychain、iOS 本地通知或 iPhone/iPad 专属界面。
- 当前共享类型、页面和传输契约应保持平台中立，但不得为了尚未开始的 iOS 工作给 Android 主链路增加并行实现或条件分支。
- Android、外联能力和产品族协作稳定后，M8 再复用移动共享层完成 iOS 平台边界、真机验收与发布准备；M8 是现有路线图的最后阶段。
