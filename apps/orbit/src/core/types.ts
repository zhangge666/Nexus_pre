/** 本文件定义 Orbit 前端所有共享的 TypeScript 类型，对应 data-model.md。 */

export type MemorySource = "orbit" | "muse" | "quill" | "echo" | string;
export type MemoryKind = "note" | "idea" | "screen" | "voice" | "card" | "clip" | "file";
export type ContentFormat = "markdown" | "plain" | "json";
export type LinkRelation = "references" | "derived_from" | "related" | "duplicate";
export type CreatedBy = "user" | "ai" | "system";
export type ReviewState = "new" | "learning" | "review" | "relearning";
export type Rating = "again" | "hard" | "good" | "easy";
export type SyncMode = "local" | "e2e_cloud" | "self_hosted";

/** 关联关系 */
export interface Link {
  fromId: string;
  toId: string;
  relation: LinkRelation;
  createdBy: CreatedBy;
}

/** 记忆摘要（时间线、检索结果通用） */
export interface MemorySummary {
  id: string;
  source: MemorySource;
  kind: MemoryKind;
  title: string | null;
  content: string;
  contentFormat: ContentFormat;
  tags: string[];
  pinned: boolean;
  archived: boolean;
  createdAt: number;
  updatedAt: number;
  capturedAt: number | null;
  links: Link[];
}

/** 检索命中（块级） */
export interface MemoryHit {
  memoryId: string;
  blockId: string;
  score: number;
  snippet: string;
  highlightRanges?: [number, number][];
}

/** 检索模式 */
export type SearchMode = "hybrid" | "semantic" | "keyword";

/** 检索请求 */
export interface SearchRequest {
  query: string;
  mode?: SearchMode;
  filters?: {
    source?: MemorySource;
    tags?: string[];
    collection?: string;
    since?: number;
    until?: number;
  };
}

/** 知识卡片（含复习状态） */
export interface ReviewCard {
  memoryId: string;
  cardFront: string;
  cardBack: string;
  state: ReviewState;
  stability: number;
  difficulty: number;
  dueAt: number;
  lastReviewedAt: number | null;
  reps: number;
  lapses: number;
  sourceMemoryId: string | null;
  sourceTitle: string | null;
  deck: string | null;
  tags: string[];
}

/** 评分结果 */
export interface GradeResult {
  nextDueAt: number;
  newStability: number;
  newState: ReviewState;
}

/** 复习统计 */
export interface ReviewStats {
  dueToday: number;
  newToday: number;
  reviewedToday: number;
  streak: number;
  mature: number;
  young: number;
  totalCards: number;
}

/** RAG 问答请求 */
export interface AskRequest {
  question: string;
  scope?: { collection?: string; source?: MemorySource };
}

/** 问答引用 */
export interface Citation {
  memoryId: string;
  blockId: string;
  snippet: string;
  sourceTitle?: string;
  sourceKind?: MemoryKind;
  createdAt?: number;
}

/** RAG 问答响应 */
export interface AskResponse {
  answer: string;
  citations: Citation[];
}

/** 对话消息 */
export interface ChatMessage {
  id: string;
  role: "user" | "ai";
  content: string;
  citations?: Citation[];
  createdAt: number;
}

/** 收件箱项类型 */
export type InboxItemType = "new_memory" | "duplicate_suggestion" | "auto_link" | "review_due";

/** 收件箱项 */
export interface InboxItem {
  id: string;
  type: InboxItemType;
  memory: MemorySummary;
  suggestion?: string;
  relatedMemoryId?: string;
  relatedMemoryTitle?: string;
  createdAt: number;
  read: boolean;
}

/** 已连接应用 */
export interface ConnectedApp {
  id: string;
  name: string;
  source: string;
  scopes: string[];
  lastActiveAt: number;
  memoriesCount: number;
  tokenId: string;
}

/** 集合 */
export interface MemoryCollection {
  id: string;
  name: string;
  icon: string | null;
  parentId: string | null;
  sort: number;
  count?: number;
}

/** 知识图谱节点 */
export interface GraphNode {
  id: string;
  title: string;
  kind: MemoryKind;
  source: MemorySource;
  cluster?: number;
  x?: number;
  y?: number;
  vx?: number;
  vy?: number;
}

/** 知识图谱边 */
export interface GraphEdge {
  from: string;
  to: string;
  relation: LinkRelation;
}

/** 设置项 */
export interface OrbitSettings {
  search: {
    defaultMode: SearchMode;
    enableRerank: boolean;
    defaultScope: string;
  };
  rag: {
    provider: "local" | "claude" | "openai" | "custom";
    apiKey: string;
    customEndpoint: string;
    streamEnabled: boolean;
    confirmBeforeSend: boolean;
  };
  cards: {
    generationMode: "ai" | "manual";
    provider: string;
    maxCardsPerNote: number;
    defaultDeck: string;
  };
  review: {
    algorithm: "fsrs" | "sm2";
    dailyNewLimit: number;
    dailyReviewLimit: number;
    reminderTime: string;
    reminderEnabled: boolean;
  };
  links: {
    autoLink: boolean;
    dedupeThreshold: number;
    graphDensity: number;
  };
  sync: {
    mode: SyncMode;
    relayEndpoint: string;
    conflictStrategy: "auto" | "manual";
  };
  appearance: {
    theme: "dark" | "light" | "system";
    language: string;
  };
}

/** 本地 Memory Protocol 服务的可展示诊断状态。 */
export interface ServiceStatus {
  role: "holder" | "client";
  endpoint: string;
  available: boolean;
  message: string | null;
}

/** 由 core 事务提交后广播给 Orbit 前端的记忆变更事件。 */
export interface MemoryChangedEvent {
  type: "memory_created" | "memory_updated" | "memory_deleted";
  id: string;
  source: MemorySource;
}
