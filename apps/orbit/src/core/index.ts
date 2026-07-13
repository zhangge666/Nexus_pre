/**
 * Orbit 数据层统一入口。
 *
 * - 运行在 Tauri WebView 中时：调用真实 IPC（invoke）
 * - 运行在普通浏览器中时：使用 mock 数据
 *
 * 所有页面只从此文件导入，不直接接触 mock.ts 或 api.ts。
 */

// ===== 类型 =====
export * from "./types";

// ===== 运行时判断 =====
export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

// ===== Mock 实现（编译时始终可见）=====
import {
  searchMemory as _mockSearch,
  listMemories as _mockList,
  getMemory as _mockGet,
  createMemory as _mockCreate,
  updateMemory as _mockUpdate,
  getReviewQueue as _mockQueue,
  getReviewStats as _mockStats,
  gradeCard as _mockGrade,
  askMemory as _mockAsk,
  listCollections as _mockListCols,
  createCollection as _mockCreateCol,
  addMemoryToCollection as _mockAddToCol,
  listInboxItems as _mockListInbox,
  markInboxRead as _mockMarkRead,
  listConnectedApps as _mockListApps,
  revokeApp as _mockRevoke,
  getGraphData as _mockGraph,
  listReviewCards as _mockListCards,
  getSettings as _mockGetSettings,
  saveSettings as _mockSaveSettings,
} from "./mock";

// ===== 惰性路由包装 =====
// 不静态 import api.ts（它依赖 @tauri-apps/api，只在 Tauri 里有）。
// 只在 Tauri 运行时动态加载。

function makeFn<T extends (...args: Parameters<T>) => ReturnType<T>>(
  name: string,
  mockFn: T,
): T {
  return ((...args: Parameters<T>) => {
    if (!isTauriRuntime()) {
      return (mockFn as (...a: Parameters<T>) => ReturnType<T>)(...args);
    }
    return import("./api").then((api) => {
      const fn = (api as unknown as Record<string, T>)[name];
      return fn(...args);
    }) as ReturnType<T>;
  }) as T;
}

export const searchMemory      = makeFn("searchMemory",      _mockSearch);
export const listMemories       = makeFn("listMemories",       _mockList);
export const getMemory          = makeFn("getMemory",          _mockGet);
export const createMemory       = makeFn("createMemory",       _mockCreate);
export const updateMemory       = makeFn("updateMemory",       _mockUpdate);
export const getReviewQueue     = makeFn("getReviewQueue",     _mockQueue);
export const getReviewStats     = makeFn("getReviewStats",     _mockStats);
export const gradeCard          = makeFn("gradeCard",          _mockGrade);
export const askMemory          = makeFn("askMemory",          _mockAsk);
export const listCollections    = makeFn("listCollections",    _mockListCols);
export const createCollection   = makeFn("createCollection",   _mockCreateCol);
export const addMemoryToCollection = makeFn("addMemoryToCollection", _mockAddToCol);
export const listInboxItems     = makeFn("listInboxItems",     _mockListInbox);
export const markInboxRead      = makeFn("markInboxRead",      _mockMarkRead);
export const listConnectedApps  = makeFn("listConnectedApps",  _mockListApps);
export const revokeApp          = makeFn("revokeApp",          _mockRevoke);
export const getGraphData       = makeFn("getGraphData",       _mockGraph);
export const listReviewCards    = makeFn("listReviewCards",    _mockListCards);
export const getSettings        = makeFn("getSettings",        _mockGetSettings);
export const saveSettings       = makeFn("saveSettings",       _mockSaveSettings);
