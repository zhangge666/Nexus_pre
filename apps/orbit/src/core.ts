/** 本文件封装 Orbit 前端通过 Tauri IPC 调用 nexus-core 的类型安全接口。 */

import { invoke } from "@tauri-apps/api/core";

/** 表示 IPC 创建成功后的记忆摘要。 */
export interface CreatedMemory {
  id: string;
  createdAt: number;
}

/** 表示 IPC 混合检索返回的块级结果。 */
export interface MemoryHit {
  memoryId: string;
  blockId: string;
  score: number;
  snippet: string;
}

/** 表示 Orbit 时间线与详情面板使用的记忆摘要。 */
export interface MemorySummary {
  id: string;
  source: string;
  kind: string;
  title: string | null;
  content: string;
  tags: string[];
  pinned: boolean;
  archived: boolean;
  createdAt: number;
}

/** 表示 Orbit 集合树使用的集合数据。 */
export interface MemoryCollection {
  id: string;
  name: string;
  icon: string | null;
  parentId: string | null;
  sort: number;
}

/** 判断当前页面是否运行在 Tauri WebView 中。 */
export function isTauriRuntime(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

/** 通过 IPC 创建一条来源为 Orbit 的 Markdown 记忆。 */
export async function createMemory(content: string): Promise<CreatedMemory> {
  return invoke<CreatedMemory>("create_memory", { content });
}

/** 通过 IPC 对本地记忆库执行混合检索。 */
export async function searchMemory(query: string): Promise<MemoryHit[]> {
  return invoke<MemoryHit[]>("search_memory", { query });
}

/** 通过 IPC 按来源读取时间线记忆。 */
export async function listMemories(source?: string): Promise<MemorySummary[]> {
  return invoke<MemorySummary[]>("list_memories", { source });
}

/** 通过 IPC 读取指定记忆的完整详情。 */
export async function getMemory(id: string): Promise<MemorySummary> {
  return invoke<MemorySummary>("get_memory", { id });
}

/** 通过 IPC 保存记忆标题和正文。 */
export async function updateMemory(id: string, title: string | null, content: string): Promise<MemorySummary> {
  return invoke<MemorySummary>("update_memory", { id, title, content });
}

/** 通过 IPC 读取集合列表。 */
export async function listCollections(): Promise<MemoryCollection[]> {
  return invoke<MemoryCollection[]>("list_collections");
}

/** 通过 IPC 创建集合。 */
export async function createCollection(name: string): Promise<MemoryCollection> {
  return invoke<MemoryCollection>("create_collection", { name });
}

/** 通过 IPC 将记忆加入集合。 */
export async function addMemoryToCollection(collectionId: string, memoryId: string): Promise<void> {
  return invoke<void>("add_memory_to_collection", { collectionId, memoryId });
}
