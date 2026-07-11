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

