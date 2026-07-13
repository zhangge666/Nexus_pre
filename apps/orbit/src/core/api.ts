/** 本文件封装真实 Tauri IPC 调用，签名与 mock.ts 完全一致。 */

import { invoke } from "@tauri-apps/api/core";
import type {
  AskRequest,
  AskResponse,
  ConnectedApp,
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
} from "./types";

export async function searchMemory(req: SearchRequest): Promise<MemoryHit[]> {
  return invoke<MemoryHit[]>("search_memory", { query: req.query, mode: req.mode ?? "hybrid" });
}

export async function listMemories(source?: string): Promise<MemorySummary[]> {
  return invoke<MemorySummary[]>("list_memories", { source });
}

export async function getMemory(id: string): Promise<MemorySummary> {
  return invoke<MemorySummary>("get_memory", { id });
}

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

export async function getReviewQueue(): Promise<ReviewCard[]> {
  return invoke<ReviewCard[]>("get_review_queue");
}

export async function getReviewStats(): Promise<ReviewStats> {
  return invoke<ReviewStats>("get_review_stats");
}

export async function gradeCard(memoryId: string, rating: Rating): Promise<GradeResult> {
  return invoke<GradeResult>("grade_card", { memoryId, rating });
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
