/** 本文件封装真实 Tauri IPC 调用，签名与 mock.ts 完全一致。 */

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AskStreamEvent,
  AskRequest,
  AskResponse,
  ConnectedApp,
  CreateCardRequest,
  GenerateCardsRequest,
  GradeResult,
  GraphEdge,
  GraphNode,
  InboxItem,
  MemoryCollection,
  MemoryHit,
  MemorySummary,
  OrbitSettings,
  Rating,
  ReviewCard,
  ReviewStats,
  SearchRequest,
  ServiceStatus,
} from "./types";

/** 调用真实 IPC 执行与用户所选模式一致的检索。 */
export async function searchMemory(req: SearchRequest): Promise<MemoryHit[]> {
  return invoke<MemoryHit[]>("search_memory", { query: req.query, mode: req.mode ?? "hybrid" });
}

/** 调用真实 IPC 获取记忆列表。 */
export async function listMemories(source?: string): Promise<MemorySummary[]> {
  // “全部”是界面筛选状态，不是 Memory Protocol 的真实来源标识；传空值才能请求全库。
  const normalizedSource = source && source !== "all" ? source : undefined;
  return invoke<MemorySummary[]>("list_memories", { source: normalizedSource });
}

/** 调用真实 IPC 获取指定集合中的记忆。 */
export async function listCollectionMemories(collectionId: string): Promise<MemorySummary[]> {
  return invoke<MemorySummary[]>("list_collection_memories", { collectionId });
}

/** 调用真实 IPC 获取指定记忆的完整内容。 */
export async function getMemory(id: string): Promise<MemorySummary> {
  return invoke<MemorySummary>("get_memory", { id });
}

/** 调用真实 IPC 新建一条 Orbit 手动记忆。 */
export async function createMemory(content: string): Promise<MemorySummary> {
  return invoke<MemorySummary>("create_memory", { content });
}

export async function updateMemory(
  id: string,
  title: string | null,
  content: string
): Promise<MemorySummary> {
  return invoke<MemorySummary>("update_memory", { id, title, content });
}

/** 读取本地 Memory Protocol 的可用性与当前服务持有角色。 */
export async function getServiceStatus(): Promise<ServiceStatus> {
  return invoke<ServiceStatus>("get_service_status");
}

export async function getReviewQueue(): Promise<ReviewCard[]> {
  return invoke<ReviewCard[]>("get_review_queue");
}

export async function getReviewStats(): Promise<ReviewStats> {
  return invoke<ReviewStats>("get_review_stats");
}

export async function gradeCard(memoryId: string, rating: Rating): Promise<GradeResult> {
  return invoke<GradeResult>("grade_card", { memoryId, rating });
}

/** 创建手动知识卡片并立即加入复习队列。 */
export async function createCard(request: CreateCardRequest): Promise<ReviewCard> {
  return invoke<ReviewCard>("create_card", { request });
}

/** 从用户选择的一条来源记忆生成并创建卡片。 */
export async function generateCards(request: GenerateCardsRequest): Promise<ReviewCard[]> {
  return invoke<ReviewCard[]>("generate_cards", {
    request: { ...request, maxCards: request.maxCards ?? 3 },
  });
}

export async function askMemory(req: AskRequest): Promise<AskResponse> {
  return invoke<AskResponse>("ask_memory", { question: req.question, scope: req.scope });
}

/** 订阅 Tauri 转发的服务端 SSE，并在每个真实文本增量到达时立即更新调用方。 */
export async function askMemoryStream(
  req: AskRequest,
  onDelta: (text: string) => void,
): Promise<AskResponse> {
  const requestId = crypto.randomUUID();
  const streamState: {
    answer: string;
    meta: Extract<AskStreamEvent, { type: "meta" }> | null;
    error: string | null;
  } = { answer: "", meta: null, error: null };
  const unlisten = await listen<AskStreamEvent>("ask-stream", ({ payload }) => {
    if (payload.requestId !== requestId) return;
    if (payload.type === "meta") {
      streamState.meta = payload;
    } else if (payload.type === "delta") {
      streamState.answer += payload.text;
      onDelta(payload.text);
    } else if (payload.type === "error") {
      streamState.error = payload.message;
    }
  });
  try {
    await invoke<void>("ask_memory_stream", {
      question: req.question,
      scope: req.scope,
      requestId,
    });
  } finally {
    unlisten();
  }
  if (streamState.error) throw new Error(streamState.error);
  if (!streamState.meta) throw new Error("流式问答未返回元数据");
  return {
    answer: streamState.answer,
    citations: streamState.meta.citations,
    provider: streamState.meta.provider,
    sentContextCount: streamState.meta.sentContextCount,
    sendsDataRemote: streamState.meta.sendsDataRemote,
  };
}

export async function listCollections(): Promise<MemoryCollection[]> {
  return invoke<MemoryCollection[]>("list_collections");
}

export async function createCollection(name: string): Promise<MemoryCollection> {
  return invoke<MemoryCollection>("create_collection", { name });
}

export async function addMemoryToCollection(
  collectionId: string,
  memoryId: string
): Promise<void> {
  return invoke<void>("add_memory_to_collection", { collectionId, memoryId });
}

export async function listInboxItems(): Promise<InboxItem[]> {
  return invoke<InboxItem[]>("list_inbox_items");
}

export async function markInboxRead(id: string): Promise<void> {
  return invoke<void>("mark_inbox_read", { id });
}

export async function listConnectedApps(): Promise<ConnectedApp[]> {
  return invoke<ConnectedApp[]>("list_connected_apps");
}

export async function revokeApp(tokenId: string): Promise<void> {
  return invoke<void>("revoke_app", { tokenId });
}

export async function getGraphData(): Promise<{ nodes: GraphNode[]; edges: GraphEdge[] }> {
  return invoke<{ nodes: GraphNode[]; edges: GraphEdge[] }>("get_graph_data");
}

export async function listReviewCards(): Promise<ReviewCard[]> {
  return invoke<ReviewCard[]>("list_review_cards");
}

export async function getSettings(): Promise<OrbitSettings> {
  return invoke<OrbitSettings>("get_settings");
}

export async function saveSettings(settings: Partial<OrbitSettings>): Promise<void> {
  return invoke<void>("save_settings", { settings });
}

/** 删除 Android 远程凭据和离线缓存，并恢复未连接状态。 */
export async function disconnectRemote(): Promise<void> {
  return invoke<void>("disconnect_remote");
}

/** 将复习提醒配置同步到 Android 系统通知调度器。 */
export async function configureReviewReminder(enabled: boolean, reminderTime: string): Promise<void> {
  return invoke<void>("configure_review_reminder", { enabled, reminderTime });
}
