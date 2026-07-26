# Orbit M5 Android 移动端开发注意

本文档记录 M5 Android 移动端的技术选型结论、可行性边界、界面适配策略与风险清单，供进入 M5 前对齐。Orbit iOS 保留为最终平台目标，但不在 M5–M7 开发，统一放到路线图最后的 M8 收口。

> 关联：里程碑定义见 [roadmap.md](roadmap.md#m5--android-多端)；跨端与平台适配层设计见 [architecture.md](architecture.md)。

## 0. 一句话结论

M5 用 **Tauri 2.0 + 现有 React/TS/Vite 前端** 先交付 Android，界面**复用页面与 `@nexus/ui`、只分叉布局外壳**。Android 的 E2E 云模式在本机维护由 Keystore 托管密钥保护的加密副本：记忆、集合和成员关系先本地落盘，再以 XChaCha20-Poly1305 签名信封上传零知识中继；列表、详情和关键词检索均在本机解密后完成。自托管 Memory Protocol 模式仍可使用远程 HTTPS 复习和问答能力。两种模式都不监听本地端口，也不把 Android 作为其他 Nexus 软件的服务端。Tauri 能交付“视觉精美 + 过渡丝滑”的效果，但受 WebView 天花板约束；真遇到原生手感瓶颈时，按架构预案把**个别页面**下沉到原生视图，不推翻整体。iOS 在 M8 复用已稳定的移动共享层，不与当前 Android 开发并行。

## 1. 技术选型（已锁定，非开放选择）

- **外壳：Tauri 2.0**。选它而非 Flutter/Electron 的核心理由是共享桌面与移动端的大部分代码；当前只交付 Android，iOS 延后到 M8。见 [architecture.md](architecture.md) §2.1、§5.4。
- **界面：React + TypeScript + Vite**，复用现有 `@nexus/ui` 设计系统与 `core/*`（api、events、types）。
- **渲染**：M5 Android 走系统 WebView（基于 Chromium），**不是原生渲染**——这是当前所有性能判断的前提；WKWebView 相关适配留到 M8。
- **Android 工程**：`apps/orbit/src-tauri/gen/android/`、移动图标、`nexus-platform-mobile` 安全存储与 WorkManager 后台同步已经落地；生成工程中的平台配置由 Tauri 配置、共享插件或独立原生模块维护。
- **本地边界**：移动端不启动 `nexus-protocol`、不监听回环 HTTP/TCP 端口、不参与桌面应用间仲裁；Tauri IPC 仅供同一 App 的 WebView 与 Rust 侧通信。
- **E2E 内容边界**：E2E 云模式的记忆写入、编辑、集合整理与删除均先进入本地加密副本和可靠待上传队列；零知识中继不提供明文 AI、语义向量或 FSRS 服务，因此该模式只显示本地副本已经完整承载的主导航。自托管 Memory Protocol 模式继续通过授权 HTTPS 使用复习、卡片和 AI 问答。

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
- **传输层分叉**：桌面继续使用本地 Memory Protocol；Android 的 `self_hosted` 模式使用远程 Memory Protocol，`e2e_cloud` 模式只使用 `/v1/sync/*` 密文中继并在本地完成内容读写、合并和关键词检索。移动端不能复用桌面本地端口发现或服务持有者逻辑。
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
| 浏览、集合、详情、离线查看 | E2E 云模式从 Keystore 密钥保护的本地加密副本读取；自托管模式使用加密 HTTP 响应缓存 |
| 写入、编辑、删除 | E2E 云模式先写本地副本和待上传 oplog，随后上传签名密文；删除使用 tombstone；自托管模式调用授权 HTTPS API |
| 检索 | E2E 云模式只在本机解密内容上执行关键词检索；自托管模式可使用服务端混合/语义检索 |
| 复习、卡片、AI 问答 | 仅自托管 Memory Protocol 模式提供；零知识中继不接收明文、向量或 AI 上下文，E2E 模式隐藏对应主导航 |
| 同步与配对 | 经独立 E2E 加密中继完成上传、拉取、合并、确认、配对、恢复与撤销，不暴露本机监听端口 |
| 本地协议服务、第三方接入、ONNX 嵌入 | 明确不做；仅桌面 Orbit 持有这些能力 |

## 5. 桌面端回归护栏

- Android 移动端合入后，桌面端 `verify:orbit-m4`（含前端类型检查、构建、Rust 测试与 Clippy）必须保持全绿。
- 任何对 `@nexus/ui` 共享组件的改动，需同时在桌面与移动两套外壳下自测。
- 严禁为移动端在共享组件内引入平台条件分支；平台差异一律走外壳层或 `platform-mobile`。

## 6. iOS 延后边界（路线图最后阶段）

- M5–M7 不执行 `tauri ios init/dev/build`，不创建或维护 Xcode 工程，不处理签名、Provisioning Profile、App Store、WKWebView、Keychain、iOS 本地通知或 iPhone/iPad 专属界面。
- 当前共享类型、页面和传输契约应保持平台中立，但不得为了尚未开始的 iOS 工作给 Android 主链路增加并行实现或条件分支。
- Android、外联能力和产品族协作稳定后，M8 再复用移动共享层完成 iOS 平台边界、真机验收与发布准备；M8 是现有路线图的最后阶段。

## 7. Android 当前交付状态（2026-07-26）

### 7.1 本阶段已完成

- 新增 Android 专用 `MobileShell`：采用全屏内容区、按连接能力显示三至五项底部导航和详情底部 Sheet，不复用桌面三栏拖拽外壳。
- 完成状态栏、底部手势区安全边距、触摸目标、键盘焦点和 `prefers-reduced-motion` 适配。
- Android 运行时不再启动本地 Memory Protocol、不参与桌面服务持有者仲裁、不打开桌面共享数据库，也不启动桌面常驻提醒扫描。
- Android 按连接模式分流：`self_hosted` 使用远程 Memory Protocol，`e2e_cloud` 只使用独立的 `/v1/sync/*` 零知识中继；发布构建强制 HTTPS，调试构建允许 HTTP。
- 新增“移动连接”设置，支持托管云、自托管地址和访问令牌；记忆详情可直接打开共享冲突检查器。
- `nexus-platform-mobile` 已实现 Tauri Android 原生安全存储插件：访问令牌由 Android Keystore 中不可导出的 AES-256-GCM 密钥保护，密文仅写入应用私有 `SharedPreferences`，不写入普通 JSON 设置文件。
- 新增 AES-256-GCM 加密离线响应缓存：缓存密钥由 Keystore 托管，缓存最多保留 256 条、30 天，并按远程服务来源隔离；网络错误或服务端 5xx 时可回退到只读缓存。
- 保存远程连接会先验证能力与令牌；切换端点、断开设备或凭据失效时会清理对应令牌和缓存，避免跨服务串用数据。
- Android 首次启动会按 Keystore 令牌状态进入工作台或移动连接设置；联网恢复、应用回到前台及每 60 秒会触发页面数据刷新。
- Android 已接入唯一 15 分钟周期 WorkManager 任务与一次性即时任务：仅在联网时运行，失败使用指数退避；应用启动、连接保存、身份初始化/恢复和配对完成后调度，断开或切回本地模式时取消。
- WorkManager 不启动 Activity 或 WebView。Kotlin 只从 Keystore 解封同步材料，协议、签名、拉取、合并和确认全部复用 Rust；JNI 密钥材料用后清零，前台与 Worker 通过同一缓存锁串行访问副本。
- Android 复习提醒已接入系统通知权限与每日调度；用户拒绝通知权限时，普通设置仍然保存并单独提示提醒未启用。
- Android 设置页隐藏无效的本地 RAG Provider，提供“断开并清除本机数据”，并明确展示当前远程问答、Keystore 与加密缓存边界。
- 将 `nexus-core`、`nexus-protocol`、SQLite 和本地嵌入相关依赖限制在桌面目标，Android 二进制不再链接本地数据库与协议服务实现。
- 桌面入口仍挂载原 `App`/`WorkspaceShell`，Android 构建通过平台常量挂载 `MobileApp`，两套外壳保持边界隔离。
- 新增轻量 `nexus-sync`：Android 只链接 XChaCha20-Poly1305、Ed25519、BIP39、配对封装和版本向量能力，不会因同步重新引入 SQLite 或 `nexus-core`。
- 新增独立 `nexus-relay`：只持久化签名密文、公钥、不可逆工作区/实体键和服务器游标，支持设备登记、恢复、配对、撤销、增量拉取、确认及 tombstone 清理。
- Orbit 桌面已接入与 Android 相同的 E2E 信封、设备身份、配对、增量同步和本地副本契约，可完成“桌面 → Relay → Android”与反向内容同步，不再只是中继或移动端单边实现。
- Android 已实现 24 词 BIP39 恢复、二维码 URI 与六位人工确认码、配对包幂等领取、设备清单和撤销；根密钥、设备标识、PKCS#8 私钥及待处理配对材料均由 Keystore 保护。
- E2E 云模式已实现加密本地内容副本和可靠待上传队列：记忆、集合、成员关系可离线创建或编辑，联网后按设备连续序号上传、按游标拉取、验证来源设备签名并确认应用进度。
- 版本向量按设备全局逻辑时钟推进；并发更新使用确定性规则收敛并保留失败版本。桌面与 Android 共用 `get_memory_conflicts` / `resolve_memory_conflict` 契约，可预览当前版本、并发内容与删除墓碑，恢复指定版本或手工合并；提交前会核对稳定版本 ID 集合，拒绝已经过期的检查器状态，成功后生成观察全部旧版本的因果后继版本。
- 删除入口已接入墓碑同步：本机立即隐藏记忆并清理已知集合成员关系，中继移除旧密文，全部有效设备确认后再删除墓碑本身。
- E2E 云模式的列表、详情、集合和关键词检索均读取本地解密副本；零知识中继不提供的复习、卡片、语义检索和 AI 问答不会再作为该模式的底部主导航。

### 7.2 已执行验证

- `cargo test -p orbit-app`：16 项通过，除原有设置、协议和桌面回归外，覆盖桌面双副本中继同步、冲突因果后继、过期状态拒绝及前端 camelCase IPC 契约。
- `cargo clippy -p orbit-app --all-targets -- -D warnings`：桌面全部目标严格检查通过。
- `cargo check -p nexus-platform-mobile`：平台移动插件 Rust 检查通过。
- `pnpm --filter @nexus/orbit check`：前端 TypeScript 检查通过。
- `pnpm --filter @nexus/orbit build`：桌面前端构建通过。
- `VITE_NEXUS_PLATFORM=android pnpm --filter @nexus/orbit build`：Android 外壳前端构建通过。
- `cargo test -p nexus-sync`：覆盖密钥/恢复短语、信封加解密、防篡改、配对封装、全局设备时钟、并发收敛和墓碑优先。
- `cargo test -p nexus-relay`：覆盖中继零明文、双设备并发收敛、全部有效设备确认后清除墓碑、恢复登记、配对与撤销。
- `cargo clippy -p orbit-app --target aarch64-linux-android --all-targets -- -D warnings`：当前冲突处理与 WorkManager 版本通过 Android arm64 严格交叉检查。
- `:app:compileUniversalDebugKotlin`：JDK 21、Android SDK/NDK 30 环境下 Kotlin 全应用编译通过。
- `pnpm --filter @nexus/orbit android:build -- --target aarch64 --apk --ci`：当前提交完整 release 构建通过，产物为 `apps/orbit/src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk`。
- 使用本机 Android 调试证书另行生成 `app-universal-release-preview-signed.apk`，`apksigner verify` 已确认 APK Signature Scheme v2/v3 有效；该包只用于本地安装预览，不能作为正式渠道签名。
- 冲突检查器已在 1280px 桌面、1024px 窄桌面和 390px Android 视口验收：无横向溢出，版本列表独立滚动，墓碑、恢复、手工合并、禁用、焦点和键盘页签状态可用。

### 7.3 M5 Android 剩余发布与真机验收

- 在真实手机和平板上完成双设备“创建 → 配对 → 并发编辑与解决 → 离线重连 → 删除 → 撤销 → 恢复短语重建”的完整数据验收，并抓取中继快照确认只含密文。
- 在 Doze、后台冻结、进程被系统回收和网络恢复场景中确认 WorkManager 周期任务、即时任务、指数退避与缓存互斥行为符合预期。
- E2E 云模式不向中继发送明文，因此复习、卡片、语义检索和 AI 问答只在自托管 Memory Protocol 模式开放；若未来要求零知识模式提供这些能力，必须新增本地索引/本地调度或由用户明确授权的端侧 Provider，不能回退为中继明文处理。
- 在真实手机与平板上完成返回手势、软键盘、长列表性能、弱网恢复、深色/亮色主题和无障碍验收。
- 当前同时保留 unsigned release 原件与调试证书预览签名包；正式交付仍需配置持久化签名密钥、版本号、AAB/渠道构建、Play 发布凭据与发布流水线。

以上后续项只针对 Android；iOS 仍按第 6 节延后到最终 M8。
