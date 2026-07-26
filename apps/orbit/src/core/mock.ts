/** 本文件提供所有页面的 Mock 数据与 Mock API 实现，用于浏览器预览和 M1 阶段开发。 */

import type {
  AskRequest,
  AskResponse,
  ChatMessage,
  ConnectedApp,
  E2eContentStatus,
  E2eDevice,
  E2ePairingJoin,
  E2ePairingOffer,
  E2ePairingStatus,
  E2eStatus,
  RegisteredConnection,
  CreateCardRequest,
  GradeResult,
  GenerateCardsRequest,
  GraphEdge,
  GraphNode,
  InboxItem,
  MemoryCollection,
  MemoryConflictResolution,
  MemoryConflictSet,
  MemoryConflictVersion,
  MemoryHit,
  MemorySummary,
  OrbitSettings,
  Rating,
  ReviewCard,
  ReviewStats,
  SearchRequest,
} from "./types";

// ---------------------------------------------------------------------------
// Mock 记忆库
// ---------------------------------------------------------------------------

export const mockMemories: MemorySummary[] = [
  {
    id: "m-001",
    source: "muse",
    kind: "idea",
    title: "统一记忆模型",
    content:
      "用统一的 Memory 模型连接捕获、检索与复习，让知识持续回到视野。\n\n核心是一切皆 Memory，无论来自 Echo 截图、Muse 速记还是 Quill 笔记，都能在 Orbit 中被统一检索与关联。",
    contentFormat: "markdown",
    tags: ["产品", "想法"],
    pinned: true,
    archived: false,
    createdAt: Date.now() - 3_600_000,
    updatedAt: Date.now() - 3_600_000,
    capturedAt: null,
    links: [{ fromId: "m-001", toId: "m-002", relation: "related", createdBy: "system" }],
    conflictCount: 2,
  },
  {
    id: "m-002",
    source: "quill",
    kind: "note",
    title: "本地协议边界",
    content:
      "## Memory Protocol Scope\n\nMemory Protocol 通过能力令牌限制每个客户端的读写范围，scope 分为五个等级：\n\n- **read** — 读取记忆\n- **write** — 写入记忆\n- **search** — 执行检索\n- **review** — 复习状态读写\n- **admin** — 连接管理（仅一等公民应用）\n\n每个令牌绑定到具体 source，外部写入统一标记为 `source=external:<app_id>`。",
    contentFormat: "markdown",
    tags: ["架构", "协议"],
    pinned: false,
    archived: false,
    createdAt: Date.now() - 86_400_000,
    updatedAt: Date.now() - 86_400_000,
    capturedAt: null,
    links: [{ fromId: "m-002", toId: "m-001", relation: "related", createdBy: "system" }],
  },
  {
    id: "m-003",
    source: "echo",
    kind: "screen",
    title: "API 设计截图",
    content:
      "截取的 REST API 设计文档页面，包含端点命名规范和版本策略。\n\nOCR 识别内容：Use nouns for resource names, avoid verbs. Version with /v1/ prefix. Use plural resource names.",
    contentFormat: "plain",
    tags: ["工程", "API"],
    pinned: false,
    archived: false,
    createdAt: Date.now() - 172_800_000,
    updatedAt: Date.now() - 172_800_000,
    capturedAt: Date.now() - 172_800_000,
    links: [],
  },
  {
    id: "m-004",
    source: "muse",
    kind: "voice",
    title: "语音备忘：产品定价",
    content:
      "按用量分档定价，基础版免费本地使用，高级版提供云同步和更大的存储空间。考虑年付折扣和团队协作版本。",
    contentFormat: "plain",
    tags: ["产品", "定价"],
    pinned: false,
    archived: false,
    createdAt: Date.now() - 604_800_000,
    updatedAt: Date.now() - 604_800_000,
    capturedAt: Date.now() - 604_800_000,
    links: [],
  },
  {
    id: "m-005",
    source: "quill",
    kind: "note",
    title: "FSRS 间隔重复算法笔记",
    content:
      "## 核心参数\n\nFSRS 用 **stability**（稳定性）和 **difficulty**（难度）两个参数刻画记忆。\n\n- **stability**：记忆的稳定程度，单位为天，越大越不容易忘\n- **difficulty**：学习难度，范围 1-10，越大越难掌握\n\n## 调度公式\n\n下次复习间隔 ≈ stability × f(difficulty, rating)\n\n## 评分级别\n\n- **Again**：完全忘记，重新学习\n- **Hard**：记得但很难，缩短间隔\n- **Good**：正常回忆，标准间隔\n- **Easy**：轻松记住，延长间隔\n\n## 与 SM-2 的区别\n\nFSRS 基于机器学习训练，参数可随个人复习历史个性化拟合，比 SM-2 更准确。",
    contentFormat: "markdown",
    tags: ["学习", "算法", "复习"],
    pinned: true,
    archived: false,
    createdAt: Date.now() - 259_200_000,
    updatedAt: Date.now() - 86_400_000,
    capturedAt: null,
    links: [
      { fromId: "card-001", toId: "m-005", relation: "derived_from", createdBy: "ai" },
      { fromId: "card-002", toId: "m-005", relation: "derived_from", createdBy: "ai" },
    ],
  },
  {
    id: "m-006",
    source: "quill",
    kind: "note",
    title: "Tauri 2.0 移动端调研",
    content:
      "## 结论\n\nTauri 2.0 的移动端已基本可用，但需要注意以下限制：\n\n1. 后台任务受系统限制，需使用本地通知调度复习提醒\n2. WebView 版本碎片化，需测试 Android 4.4+ 兼容性\n3. 插件生态尚不完善，截图/麦克风权限需手动处理\n\n推荐策略：先打通核心链路（连库/复习/问答/通知），再补齐边缘功能。",
    contentFormat: "markdown",
    tags: ["工程", "移动端", "Tauri"],
    pinned: false,
    archived: false,
    createdAt: Date.now() - 432_000_000,
    updatedAt: Date.now() - 432_000_000,
    capturedAt: null,
    links: [],
  },
  {
    id: "m-007",
    source: "muse",
    kind: "idea",
    title: "知识图谱可视化方案",
    content:
      "考虑用 Canvas 2D + 力导向布局实现知识图谱，避免引入重型 D3 依赖。节点按 source 着色，边按 relation 类型显示不同样式。主题聚类用半透明背景色块区分。",
    contentFormat: "plain",
    tags: ["产品", "可视化"],
    pinned: false,
    archived: false,
    createdAt: Date.now() - 518_400_000,
    updatedAt: Date.now() - 518_400_000,
    capturedAt: null,
    links: [],
  },
  {
    id: "m-008",
    source: "echo",
    kind: "screen",
    title: "Linear 设计风格参考",
    content:
      "截取的 Linear 应用界面，用于 Nexus 设计系统参考。特点：暗色优先、低饱和、高信息密度、靠细边框分层、单一紫靛强调色。",
    contentFormat: "plain",
    tags: ["设计", "参考"],
    pinned: false,
    archived: false,
    createdAt: Date.now() - 691_200_000,
    updatedAt: Date.now() - 691_200_000,
    capturedAt: Date.now() - 691_200_000,
    links: [],
  },
];

// ---------------------------------------------------------------------------
// Mock 复习卡片
// ---------------------------------------------------------------------------

export const mockReviewCards: ReviewCard[] = [
  {
    memoryId: "card-001",
    cardFront: "FSRS 用来刻画记忆的两个核心参数是什么？",
    cardBack:
      "**stability**（稳定性）与 **difficulty**（难度）。\n\n- stability：记忆稳定程度，单位为天\n- difficulty：学习难度，范围 1-10",
    state: "review",
    stability: 12.7,
    difficulty: 5.3,
    dueAt: Date.now() - 3_600_000,
    lastReviewedAt: Date.now() - 86_400_000 * 13,
    reps: 3,
    lapses: 0,
    sourceMemoryId: "m-005",
    sourceTitle: "FSRS 间隔重复算法笔记",
    deck: "学习/算法",
    tags: ["学习", "算法"],
  },
  {
    memoryId: "card-002",
    cardFront: "Memory Protocol 的五个 scope 等级是什么？",
    cardBack:
      "`read`、`write`、`search`、`review`、`admin`\n\n权限从低到高，**admin** scope 仅对一等公民应用与用户本人开放。",
    state: "review",
    stability: 8.2,
    difficulty: 4.1,
    dueAt: Date.now() - 7_200_000,
    lastReviewedAt: Date.now() - 86_400_000 * 8,
    reps: 5,
    lapses: 1,
    sourceMemoryId: "m-002",
    sourceTitle: "本地协议边界",
    deck: "架构",
    tags: ["架构", "协议"],
  },
  {
    memoryId: "card-003",
    cardFront: "Nexus 的「内联」与「外联」分别指什么？",
    cardBack:
      "**内联 (inline)**：Echo/Muse/Quill 等一等公民客户端通过 Memory Protocol 与 Orbit 内部打通。\n\n**外联 (external)**：第三方应用、AI 助手通过同一套 Memory Protocol 接入 Orbit。",
    state: "new",
    stability: 0,
    difficulty: 0,
    dueAt: Date.now(),
    lastReviewedAt: null,
    reps: 0,
    lapses: 0,
    sourceMemoryId: "m-001",
    sourceTitle: "统一记忆模型",
    deck: "产品",
    tags: ["产品", "架构"],
  },
  {
    memoryId: "card-004",
    cardFront: "为什么 Orbit 的检索全部在本地完成？",
    cardBack:
      "这是 E2E 加密下的**隐私红线**：云端只做加密块中继，永远拿不到明文。\n\n语义检索、嵌入、RAG 取材、复习调度全部在本地明文库完成，不为性能妥协。",
    state: "learning",
    stability: 1.2,
    difficulty: 6.8,
    dueAt: Date.now() - 600_000,
    lastReviewedAt: Date.now() - 3_600_000,
    reps: 1,
    lapses: 0,
    sourceMemoryId: null,
    sourceTitle: null,
    deck: "隐私与安全",
    tags: ["隐私", "架构"],
  },
  {
    memoryId: "card-005",
    cardFront: "FSRS 评分的四个等级分别是什么，各代表什么含义？",
    cardBack:
      "- **Again**：完全忘记，重新进入学习队列\n- **Hard**：想起但很难，缩短下次间隔\n- **Good**：正常回忆，按标准 FSRS 调度\n- **Easy**：轻松记住，延长下次间隔",
    state: "review",
    stability: 22.4,
    difficulty: 3.8,
    dueAt: Date.now() - 1_800_000,
    lastReviewedAt: Date.now() - 86_400_000 * 22,
    reps: 7,
    lapses: 0,
    sourceMemoryId: "m-005",
    sourceTitle: "FSRS 间隔重复算法笔记",
    deck: "学习/算法",
    tags: ["学习", "算法"],
  },
  {
    memoryId: "card-006",
    cardFront: "Tauri 2.0 移动端后台任务的推荐策略？",
    cardBack:
      "使用**系统本地通知**调度复习提醒，而非常驻后台进程。\n\n平台适配层（`platform-mobile`）隔离差异，优先验证 iOS/Android 关键路径：连库 → 复习 → 问答 → 通知。",
    state: "new",
    stability: 0,
    difficulty: 0,
    dueAt: Date.now(),
    lastReviewedAt: null,
    reps: 0,
    lapses: 0,
    sourceMemoryId: "m-006",
    sourceTitle: "Tauri 2.0 移动端调研",
    deck: "工程",
    tags: ["工程", "移动端"],
  },
];

export const mockReviewStats: ReviewStats = {
  dueToday: 6,
  newToday: 2,
  reviewedToday: 0,
  streak: 7,
  mature: 42,
  young: 18,
  totalCards: 65,
};

// ---------------------------------------------------------------------------
// Mock 集合
// ---------------------------------------------------------------------------

export const mockCollections: MemoryCollection[] = [
  { id: "col-001", name: "产品设计", icon: "💡", parentId: null, sort: 0, count: 12 },
  { id: "col-002", name: "技术架构", icon: "🔧", parentId: null, sort: 1, count: 8 },
  { id: "col-003", name: "学习笔记", icon: "📖", parentId: null, sort: 2, count: 24 },
  { id: "col-004", name: "算法", icon: null, parentId: "col-003", sort: 0, count: 6 },
  { id: "col-005", name: "设计参考", icon: "🎨", parentId: null, sort: 3, count: 5 },
];

// ---------------------------------------------------------------------------
// Mock 收件箱
// ---------------------------------------------------------------------------

export const mockInboxItems: InboxItem[] = [
  {
    id: "inbox-001",
    type: "new_memory",
    read: false,
    createdAt: Date.now() - 7_200_000,
    memory: mockMemories[2],
  },
  {
    id: "inbox-002",
    type: "auto_link",
    read: false,
    createdAt: Date.now() - 3_600_000,
    memory: mockMemories[0],
    suggestion: "「统一记忆模型」与「本地协议边界」语义相似度 0.87，建议建立 related 关联。",
    relatedMemoryId: "m-002",
    relatedMemoryTitle: "本地协议边界",
  },
  {
    id: "inbox-003",
    type: "duplicate_suggestion",
    read: false,
    createdAt: Date.now() - 1_800_000,
    memory: mockMemories[2],
    suggestion: "新写入的「API端点规范」与已有的「API设计截图」内容哈希相似度 92%，可能重复。",
    relatedMemoryId: "m-008",
    relatedMemoryTitle: "Linear 设计风格参考",
  },
  {
    id: "inbox-004",
    type: "review_due",
    read: true,
    createdAt: Date.now() - 86_400_000,
    memory: mockMemories[4],
    suggestion: "有 6 张卡片已到期，建议尽快复习。",
  },
  {
    id: "inbox-005",
    type: "new_memory",
    read: true,
    createdAt: Date.now() - 172_800_000,
    memory: mockMemories[5],
  },
];

// ---------------------------------------------------------------------------
// Mock 连接应用
// ---------------------------------------------------------------------------

export const mockConnectedApps: ConnectedApp[] = [
  {
    id: "app-echo",
    name: "Echo",
    source: "echo",
    scopes: ["memory:read", "memory:write", "search"],
    lastActiveAt: Date.now() - 7_200_000,
    createdAt: Date.now() - 30 * 86_400_000,
    memoriesCount: 142,
    readCount: 37,
    writeCount: 142,
    lastScope: "memory:write",
    sendsDataRemote: false,
    tokenId: "tok-001",
  },
  {
    id: "app-muse",
    name: "Muse",
    source: "muse",
    scopes: ["memory:write"],
    lastActiveAt: Date.now() - 1_800_000,
    createdAt: Date.now() - 20 * 86_400_000,
    memoriesCount: 89,
    readCount: 0,
    writeCount: 89,
    lastScope: "memory:write",
    sendsDataRemote: false,
    tokenId: "tok-002",
  },
  {
    id: "app-quill",
    name: "Quill",
    source: "quill",
    scopes: ["memory:read", "memory:write", "search"],
    lastActiveAt: Date.now() - 86_400_000,
    createdAt: Date.now() - 18 * 86_400_000,
    memoriesCount: 56,
    readCount: 24,
    writeCount: 56,
    lastScope: "search",
    sendsDataRemote: false,
    tokenId: "tok-003",
  },
  {
    id: "app-claude",
    name: "Claude (MCP)",
    source: "external:claude",
    scopes: ["memory:read", "search"],
    lastActiveAt: Date.now() - 259_200_000,
    createdAt: Date.now() - 14 * 86_400_000,
    memoriesCount: 12,
    readCount: 31,
    writeCount: 0,
    lastScope: "search",
    sendsDataRemote: false,
    tokenId: "tok-004",
  },
];

// ---------------------------------------------------------------------------
// Mock 知识图谱
// ---------------------------------------------------------------------------

export const mockGraphNodes: GraphNode[] = [
  { id: "m-001", title: "统一记忆模型", kind: "idea", source: "muse", cluster: 0 },
  { id: "m-002", title: "本地协议边界", kind: "note", source: "quill", cluster: 0 },
  { id: "m-003", title: "API 设计截图", kind: "screen", source: "echo", cluster: 1 },
  { id: "m-005", title: "FSRS 间隔重复算法笔记", kind: "note", source: "quill", cluster: 2 },
  { id: "m-006", title: "Tauri 2.0 移动端调研", kind: "note", source: "quill", cluster: 1 },
  { id: "m-007", title: "知识图谱可视化方案", kind: "idea", source: "muse", cluster: 0 },
  { id: "card-001", title: "FSRS 两个核心参数", kind: "card", source: "orbit", cluster: 2 },
  { id: "card-002", title: "Protocol 五个 scope", kind: "card", source: "orbit", cluster: 0 },
  { id: "card-003", title: "内联与外联的区别", kind: "card", source: "orbit", cluster: 0 },
];

export const mockGraphEdges: GraphEdge[] = [
  { from: "m-001", to: "m-002", relation: "related" },
  { from: "m-001", to: "m-007", relation: "related" },
  { from: "m-002", to: "m-003", relation: "references" },
  { from: "m-006", to: "m-003", relation: "references" },
  { from: "card-001", to: "m-005", relation: "derived_from" },
  { from: "card-002", to: "m-002", relation: "derived_from" },
  { from: "card-003", to: "m-001", relation: "derived_from" },
];

// ---------------------------------------------------------------------------
// Mock 对话历史
// ---------------------------------------------------------------------------

export const mockChatHistory: ChatMessage[] = [
  {
    id: "msg-001",
    role: "user",
    content: "我上周关于定价的想法是什么？",
    createdAt: Date.now() - 3_600_000,
  },
  {
    id: "msg-002",
    role: "ai",
    content:
      "你上周提出了按用量分档定价的方案：\n\n基础版免费本地使用，高级版提供云同步和更大的存储空间。你还提到考虑年付折扣和团队协作版本。",
    citations: [
      {
        memoryId: "m-004",
        blockId: "b-004-1",
        snippet: "按用量分档定价，基础版免费本地使用，高级版提供云同步和更大的存储空间。",
        sourceTitle: "语音备忘：产品定价",
        sourceKind: "voice",
        createdAt: Date.now() - 604_800_000,
      },
    ],
    createdAt: Date.now() - 3_599_000,
  },
];

// ---------------------------------------------------------------------------
// Mock 设置
// ---------------------------------------------------------------------------

export const mockSettings: OrbitSettings = {
  search: { defaultMode: "hybrid", enableRerank: true, defaultScope: "all" },
  rag: {
    provider: "local",
    apiKey: "",
    hasApiKey: false,
    model: "",
    customEndpoint: "",
    streamEnabled: true,
    confirmBeforeSend: true,
  },
  cards: { generationMode: "ai", provider: "local", maxCardsPerNote: 10, defaultDeck: "默认" },
  review: {
    algorithm: "fsrs",
    dailyNewLimit: 20,
    dailyReviewLimit: 100,
    reminderTime: "08:00",
    reminderEnabled: true,
  },
  links: { autoLink: true, dedupeThreshold: 0.85, graphDensity: 0.6 },
  sync: {
    mode: "local",
    relayEndpoint: "",
    accessToken: "",
    hasAccessToken: false,
    conflictStrategy: "auto",
  },
  appearance: { theme: "dark", language: "zh-CN" },
};

// ---------------------------------------------------------------------------
// Mock API 函数（与 api.ts 签名一致）
// ---------------------------------------------------------------------------

function delay(ms = 400): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

export async function searchMemory(req: SearchRequest): Promise<MemoryHit[]> {
  await delay();
  const q = req.query.toLowerCase();
  return mockMemories
    .filter((m) => m.title?.toLowerCase().includes(q) || m.content.toLowerCase().includes(q))
    .slice(0, 8)
    .map((m) => ({
      memoryId: m.id,
      blockId: m.id + "-b0",
      score: 0.9 - Math.random() * 0.2,
      snippet: m.content.slice(0, 120),
    }));
}

export async function listMemories(source?: string): Promise<MemorySummary[]> {
  await delay(200);
  return source && source !== "all"
    ? mockMemories.filter((m) => m.source === source)
    : mockMemories;
}

export async function getMemory(id: string): Promise<MemorySummary> {
  await delay(100);
  const mem = mockMemories.find((m) => m.id === id);
  if (!mem) throw new Error(`Memory ${id} not found`);
  return mem;
}

/** 浏览器预览中构造一个当前版本和一个来自 Android 的并发版本。 */
export async function getMemoryConflicts(id: string): Promise<MemoryConflictSet> {
  await delay(120);
  const memory = await getMemory(id);
  const versions: MemoryConflictVersion[] = [{
    versionId: `${id}:current`,
    isCurrent: true,
    deviceId: "desktop-preview",
    modifiedAt: memory.updatedAt,
    memory: { ...memory },
  }];
  if (memory.conflictCount) {
    versions.push({
      versionId: `${id}:android`,
      isCurrent: false,
      deviceId: "android-preview",
      modifiedAt: memory.updatedAt - 60_000,
      memory: {
        ...memory,
        title: memory.title ? `${memory.title}（Android 版本）` : "Android 版本",
        content: `${memory.content}\n\n来自 Android 并发编辑的补充内容。`,
        conflictCount: 0,
      },
    });
    versions.push({
      versionId: `${id}:tombstone`,
      isCurrent: false,
      deviceId: "android-offline-preview",
      modifiedAt: memory.updatedAt - 120_000,
      memory: null,
    });
  }
  return { memoryId: id, versions };
}

export async function createMemory(content: string): Promise<MemorySummary> {
  await delay(300);
  const newMem: MemorySummary = {
    id: "m-" + Date.now(),
    source: "orbit",
    kind: "note",
    title: null,
    content,
    contentFormat: "markdown",
    tags: [],
    pinned: false,
    archived: false,
    createdAt: Date.now(),
    updatedAt: Date.now(),
    capturedAt: null,
    links: [],
  };
  mockMemories.unshift(newMem);
  return newMem;
}

export async function updateMemory(
  id: string,
  title: string | null,
  content: string
): Promise<MemorySummary> {
  await delay(200);
  const idx = mockMemories.findIndex((m) => m.id === id);
  if (idx === -1) throw new Error(`Memory ${id} not found`);
  mockMemories[idx] = { ...mockMemories[idx], title, content, updatedAt: Date.now() };
  return mockMemories[idx];
}

/** 浏览器预览中应用冲突恢复或手工合并，并清除冲突计数。 */
export async function resolveMemoryConflict(
  id: string,
  resolution: MemoryConflictResolution,
): Promise<MemorySummary> {
  const conflicts = await getMemoryConflicts(id);
  const actualVersionIds = conflicts.versions.map((version) => version.versionId).sort();
  const expectedVersionIds = [...resolution.expectedVersionIds].sort();
  if (
    actualVersionIds.length !== expectedVersionIds.length
    || actualVersionIds.some((versionId, index) => versionId !== expectedVersionIds[index])
  ) {
    throw new Error("冲突版本已经变化，请刷新后重新确认");
  }
  const selected = resolution.strategy === "restore"
    ? conflicts.versions.find((version) => version.versionId === resolution.versionId)?.memory
    : conflicts.versions.find((version) => version.isCurrent)?.memory;
  if (!selected) throw new Error("所选冲突版本不可用");
  return updateMemory(
    id,
    resolution.strategy === "merge" ? resolution.title : selected.title,
    resolution.strategy === "merge" ? resolution.content : selected.content,
  ).then((memory) => {
    memory.conflictCount = 0;
    return memory;
  });
}

/** 浏览器预览中删除一条 mock 记忆。 */
export async function deleteMemory(id: string): Promise<void> {
  await delay(150);
  const index = mockMemories.findIndex((memory) => memory.id === id);
  if (index === -1) throw new Error(`Memory ${id} not found`);
  mockMemories.splice(index, 1);
}

export async function getReviewQueue(): Promise<ReviewCard[]> {
  await delay(300);
  return mockReviewCards.filter((c) => c.dueAt <= Date.now());
}

export async function getReviewStats(): Promise<ReviewStats> {
  await delay(100);
  return { ...mockReviewStats, dueToday: mockReviewCards.filter((c) => c.dueAt <= Date.now()).length };
}

export async function gradeCard(memoryId: string, rating: Rating): Promise<GradeResult> {
  await delay(200);
  const intervals: Record<Rating, number> = {
    again: 60_000,
    hard: 360_000,
    good: 86_400_000 * 13,
    easy: 86_400_000 * 32,
  };
  return {
    nextDueAt: Date.now() + intervals[rating],
    newStability: 15.2,
    newDifficulty: 5.1,
    newState: rating === "again" ? "relearning" : "review",
  };
}

export async function createCard(request: CreateCardRequest): Promise<ReviewCard> {
  await delay(200);
  const card: ReviewCard = {
    memoryId: `card-${Date.now()}`,
    cardFront: request.cardFront,
    cardBack: request.cardBack,
    state: "new",
    stability: 0,
    difficulty: 5,
    dueAt: Date.now(),
    lastReviewedAt: null,
    reps: 0,
    lapses: 0,
    sourceMemoryId: request.sourceMemoryId ?? null,
    sourceTitle: request.sourceMemoryId
      ? mockMemories.find((memory) => memory.id === request.sourceMemoryId)?.title ?? null
      : null,
    deck: request.deck ?? null,
    tags: request.tags ?? [],
  };
  mockReviewCards.push(card);
  return card;
}

export async function generateCards(request: GenerateCardsRequest): Promise<ReviewCard[]> {
  const source = mockMemories.find((memory) => memory.id === request.sourceMemoryId);
  if (!source) throw new Error("来源记忆不存在");
  return [await createCard({
    cardFront: `「${source.title ?? "这条记忆"}」的核心要点是什么？`,
    cardBack: source.content.slice(0, 320),
    sourceMemoryId: source.id,
    deck: request.deck,
    tags: request.tags,
  })];
}

export async function askMemory(req: AskRequest): Promise<AskResponse> {
  await delay(800);
  return {
    answer: `根据你的记忆库，关于「${req.question}」：\n\n你在之前的记录中提到过相关内容。本地检索命中了以下记忆片段，综合来看核心要点是：记忆优先存储在本地，通过统一的 Memory Protocol 对外开放访问，同时保持端到端加密确保隐私安全。`,
    citations: [
      {
        memoryId: "m-001",
        blockId: "m-001-b0",
        snippet: "用统一的 Memory 模型连接捕获、检索与复习，让知识持续回到视野。",
        sourceTitle: "统一记忆模型",
        sourceKind: "idea",
        createdAt: Date.now() - 3_600_000,
      },
    ],
    provider: "local",
    sentContextCount: 1,
    sendsDataRemote: false,
  };
}

/** 浏览器预览使用单次 mock 回答模拟真实流式接口，避免依赖 Tauri 事件总线。 */
export async function askMemoryStream(
  req: AskRequest,
  onDelta: (text: string) => void,
): Promise<AskResponse> {
  const response = await askMemory(req);
  onDelta(response.answer);
  return response;
}

export async function listCollections(): Promise<MemoryCollection[]> {
  await delay(100);
  return mockCollections;
}

export async function createCollection(name: string): Promise<MemoryCollection> {
  await delay(200);
  const col: MemoryCollection = {
    id: "col-" + Date.now(),
    name,
    icon: null,
    parentId: null,
    sort: mockCollections.length,
    count: 0,
  };
  mockCollections.push(col);
  return col;
}

/** 浏览器预览中模拟清除移动连接。 */
export async function disconnectRemote(): Promise<void> {
  await delay(100);
}

/** 浏览器预览中模拟保存系统复习提醒。 */
export async function configureReviewReminder(_enabled: boolean, _reminderTime: string): Promise<void> {
  await delay(100);
}

let mockE2eStatus: E2eStatus = {
  configured: false,
  workspaceId: null,
  deviceId: null,
  pendingJoin: false,
  outgoingPairing: false,
};

const mockE2eDevices: E2eDevice[] = [];

/** 浏览器预览中返回 E2E 设备身份状态。 */
export async function getE2eStatus(): Promise<E2eStatus> {
  await delay(100);
  return { ...mockE2eStatus };
}

/** 浏览器预览中返回稳定的本地副本同步状态。 */
export async function getE2eContentStatus(): Promise<E2eContentStatus> {
  await delay(80);
  return {
    cursor: mockMemories.length,
    pendingChanges: 0,
    conflictCount: mockMemories.reduce((total, memory) => total + (memory.conflictCount ?? 0), 0),
    lastSyncAt: Date.now(),
  };
}

/** 浏览器预览中模拟立即完成一次密文增量同步。 */
export async function syncE2eContent(): Promise<E2eContentStatus> {
  await delay(180);
  return getE2eContentStatus();
}

/** 浏览器预览中模拟创建首个 E2E 工作区。 */
export async function initializeE2e(deviceName: string): Promise<E2eStatus> {
  await delay(250);
  const deviceId = `android-${crypto.randomUUID().replaceAll("-", "")}`;
  mockE2eStatus = {
    configured: true,
    workspaceId: crypto.randomUUID().replaceAll("-", ""),
    deviceId,
    pendingJoin: false,
    outgoingPairing: false,
  };
  mockE2eDevices.push({
    workspaceId: mockE2eStatus.workspaceId ?? "preview",
    deviceId,
    name: deviceName,
    publicKey: "preview-ed25519-public-key",
    createdAt: Date.now(),
    lastSeenAt: Date.now(),
    revokedAt: null,
    lastSequence: 0,
    acknowledgedCursor: 0,
  });
  return { ...mockE2eStatus };
}

/** 浏览器预览中模拟使用恢复短语登记设备。 */
export async function restoreE2e(_recoveryPhrase: string, deviceName: string): Promise<E2eStatus> {
  return initializeE2e(deviceName);
}

/** 浏览器预览中返回不可用于真实恢复的示例短语。 */
export async function getRecoveryPhrase(): Promise<string> {
  await delay(100);
  return "abandon ".repeat(23) + "art";
}

/** 浏览器预览中模拟创建配对二维码。 */
export async function createE2ePairingOffer(): Promise<E2ePairingOffer> {
  await delay(200);
  mockE2eStatus = { ...mockE2eStatus, outgoingPairing: true };
  return {
    sessionId: crypto.randomUUID(),
    pairingUri: "nexus://pair?version=1&session=preview&workspace=preview&secret=preview",
    qrDataUrl: "",
    verificationCode: "482731",
    expiresAt: Date.now() + 600_000,
  };
}

/** 浏览器预览中返回待批准配对状态。 */
export async function getE2ePairingStatus(): Promise<E2ePairingStatus> {
  await delay(100);
  return {
    sessionId: "preview",
    expiresAt: Date.now() + 600_000,
    pendingDevice: null,
    approved: false,
    consumed: false,
  };
}

/** 浏览器预览中模拟提交新设备配对申请。 */
export async function requestE2ePairing(
  _pairingUri: string,
  _deviceName: string,
): Promise<E2ePairingJoin> {
  await delay(200);
  mockE2eStatus = { ...mockE2eStatus, pendingJoin: true };
  return {
    deviceId: `android-${crypto.randomUUID().replaceAll("-", "")}`,
    verificationCode: "482731",
    waitingForApproval: true,
  };
}

/** 浏览器预览中模拟批准配对设备。 */
export async function approveE2ePairing(): Promise<E2eDevice> {
  await delay(200);
  const device: E2eDevice = {
    workspaceId: mockE2eStatus.workspaceId ?? "preview",
    deviceId: `android-${crypto.randomUUID().replaceAll("-", "")}`,
    name: "新 Android 设备",
    publicKey: "preview-ed25519-public-key",
    createdAt: Date.now(),
    lastSeenAt: Date.now(),
    revokedAt: null,
    lastSequence: 0,
    acknowledgedCursor: 0,
  };
  mockE2eDevices.push(device);
  mockE2eStatus = { ...mockE2eStatus, outgoingPairing: false };
  return device;
}

/** 浏览器预览中模拟领取配对包。 */
export async function completeE2ePairing(): Promise<E2eStatus> {
  await delay(200);
  mockE2eStatus = {
    ...mockE2eStatus,
    configured: true,
    pendingJoin: false,
    workspaceId: mockE2eStatus.workspaceId ?? "preview",
    deviceId: mockE2eStatus.deviceId ?? "android-preview",
  };
  return { ...mockE2eStatus };
}

/** 浏览器预览中列出 E2E 设备。 */
export async function listE2eDevices(): Promise<E2eDevice[]> {
  await delay(100);
  return mockE2eDevices.map((device) => ({ ...device }));
}

/** 浏览器预览中模拟撤销 E2E 设备。 */
export async function revokeE2eDevice(deviceId: string): Promise<E2eDevice> {
  await delay(150);
  const device = mockE2eDevices.find((item) => item.deviceId === deviceId);
  if (!device) throw new Error("设备不存在");
  device.revokedAt = Date.now();
  return { ...device };
}

export async function addMemoryToCollection(collectionId: string, memoryId: string): Promise<void> {
  await delay(100);
  void collectionId; void memoryId; // mock: no-op
}

export async function listInboxItems(): Promise<InboxItem[]> {
  await delay(200);
  return mockInboxItems;
}

export async function markInboxRead(id: string): Promise<void> {
  await delay(50);
  const item = mockInboxItems.find((i) => i.id === id);
  if (item) item.read = true;
}

export async function listConnectedApps(): Promise<ConnectedApp[]> {
  await delay(200);
  return mockConnectedApps;
}

/** 浏览器预览中模拟签发一条来源受限的第三方授权。 */
export async function registerExternalApp(
  appId: string,
  name: string,
  scopes: string[],
): Promise<RegisteredConnection> {
  await delay(300);
  const tokenId = `tok-${crypto.randomUUID()}`;
  const source = `external:${appId}` as const;
  mockConnectedApps.unshift({
    id: appId,
    name,
    source,
    scopes,
    lastActiveAt: Date.now(),
    createdAt: Date.now(),
    memoriesCount: 0,
    readCount: 0,
    writeCount: 0,
    lastScope: null,
    sendsDataRemote: false,
    tokenId,
  });
  return {
    tokenId,
    token: `nx_${crypto.randomUUID().replaceAll("-", "")}`,
    scopes,
    source,
  };
}

export async function revokeApp(tokenId: string): Promise<void> {
  await delay(300);
  const idx = mockConnectedApps.findIndex((a) => a.tokenId === tokenId);
  if (idx !== -1) mockConnectedApps.splice(idx, 1);
}

export async function getGraphData(): Promise<{ nodes: GraphNode[]; edges: GraphEdge[] }> {
  await delay(400);
  return { nodes: mockGraphNodes, edges: mockGraphEdges };
}

export async function listReviewCards(): Promise<ReviewCard[]> {
  await delay(200);
  return mockReviewCards;
}

export async function getSettings(): Promise<OrbitSettings> {
  await delay(100);
  return { ...mockSettings };
}

export async function saveSettings(settings: Partial<OrbitSettings>): Promise<void> {
  await delay(200);
  Object.assign(mockSettings, settings);
}
