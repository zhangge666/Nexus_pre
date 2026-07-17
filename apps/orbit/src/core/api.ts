/** 本文件封装真实 Tauri IPC 调用，签名与 mock.ts 完全一致。 */

import { invoke } from "@tauri-apps/api/core";
import type {
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
  return invoke<MemorySummary[]>("list_memories", { source });
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
