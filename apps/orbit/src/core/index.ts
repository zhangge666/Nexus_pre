/**
 * Orbit 数据层统一入口。
 *
 * - 运行在 Tauri WebView 中时：调用真实 IPC（invoke）
 * - 运行在普通浏览器中时：使用 mock 数据
 *
 * 所有页面只从此文件导入，不直接接触 mock.ts 或 api.ts。
 */

import { isTauri } from "@tauri-apps/api/core";

// ===== 类型 =====
export * from "./types";

// ===== 运行时判断 =====
/** 使用 Tauri 官方运行时判定，避免桌面端因内部对象变更而错误回退至 mock。 */
export function isTauriRuntime(): boolean {
  return isTauri();
}

// ===== Mock 实现（编译时始终可见）=====
import {
  searchMemory as _mockSearch,
  listMemories as _mockList,
  getMemory as _mockGet,
  createMemory as _mockCreate,
  updateMemory as _mockUpdate,
  deleteMemory as _mockDelete,
  getReviewQueue as _mockQueue,
  getReviewStats as _mockStats,
  gradeCard as _mockGrade,
  askMemory as _mockAsk,
  askMemoryStream as _mockAskStream,
  listCollections as _mockListCols,
  createCollection as _mockCreateCol,
  addMemoryToCollection as _mockAddToCol,
  listInboxItems as _mockListInbox,
  markInboxRead as _mockMarkRead,
  listConnectedApps as _mockListApps,
  registerExternalApp as _mockRegisterExternalApp,
  revokeApp as _mockRevoke,
  getGraphData as _mockGraph,
  listReviewCards as _mockListCards,
  getSettings as _mockGetSettings,
  saveSettings as _mockSaveSettings,
  createCard as _mockCreateCard,
  generateCards as _mockGenerateCards,
  disconnectRemote as _mockDisconnectRemote,
  configureReviewReminder as _mockConfigureReviewReminder,
  getE2eStatus as _mockGetE2eStatus,
  getE2eContentStatus as _mockGetE2eContentStatus,
  syncE2eContent as _mockSyncE2eContent,
  initializeE2e as _mockInitializeE2e,
  restoreE2e as _mockRestoreE2e,
  getRecoveryPhrase as _mockGetRecoveryPhrase,
  createE2ePairingOffer as _mockCreateE2ePairingOffer,
  getE2ePairingStatus as _mockGetE2ePairingStatus,
  requestE2ePairing as _mockRequestE2ePairing,
  approveE2ePairing as _mockApproveE2ePairing,
  completeE2ePairing as _mockCompleteE2ePairing,
  listE2eDevices as _mockListE2eDevices,
  revokeE2eDevice as _mockRevokeE2eDevice,
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
export const deleteMemory       = makeFn("deleteMemory",       _mockDelete);
export const getReviewQueue     = makeFn("getReviewQueue",     _mockQueue);
export const getReviewStats     = makeFn("getReviewStats",     _mockStats);
export const gradeCard          = makeFn("gradeCard",          _mockGrade);
export const askMemory          = makeFn("askMemory",          _mockAsk);
export const askMemoryStream    = makeFn("askMemoryStream",    _mockAskStream);
export const listCollections    = makeFn("listCollections",    _mockListCols);
export const createCollection   = makeFn("createCollection",   _mockCreateCol);
export const addMemoryToCollection = makeFn("addMemoryToCollection", _mockAddToCol);
export const listInboxItems     = makeFn("listInboxItems",     _mockListInbox);
export const markInboxRead      = makeFn("markInboxRead",      _mockMarkRead);
export const listConnectedApps  = makeFn("listConnectedApps",  _mockListApps);
export const registerExternalApp = makeFn("registerExternalApp", _mockRegisterExternalApp);
export const revokeApp          = makeFn("revokeApp",          _mockRevoke);
export const getGraphData       = makeFn("getGraphData",       _mockGraph);
export const listReviewCards    = makeFn("listReviewCards",    _mockListCards);
export const getSettings        = makeFn("getSettings",        _mockGetSettings);
export const saveSettings       = makeFn("saveSettings",       _mockSaveSettings);
export const createCard         = makeFn("createCard",         _mockCreateCard);
export const generateCards      = makeFn("generateCards",      _mockGenerateCards);
export const disconnectRemote   = makeFn("disconnectRemote",   _mockDisconnectRemote);
export const configureReviewReminder = makeFn("configureReviewReminder", _mockConfigureReviewReminder);
export const getE2eStatus = makeFn("getE2eStatus", _mockGetE2eStatus);
export const getE2eContentStatus = makeFn("getE2eContentStatus", _mockGetE2eContentStatus);
export const syncE2eContent = makeFn("syncE2eContent", _mockSyncE2eContent);
export const initializeE2e = makeFn("initializeE2e", _mockInitializeE2e);
export const restoreE2e = makeFn("restoreE2e", _mockRestoreE2e);
export const getRecoveryPhrase = makeFn("getRecoveryPhrase", _mockGetRecoveryPhrase);
export const createE2ePairingOffer = makeFn("createE2ePairingOffer", _mockCreateE2ePairingOffer);
export const getE2ePairingStatus = makeFn("getE2ePairingStatus", _mockGetE2ePairingStatus);
export const requestE2ePairing = makeFn("requestE2ePairing", _mockRequestE2ePairing);
export const approveE2ePairing = makeFn("approveE2ePairing", _mockApproveE2ePairing);
export const completeE2ePairing = makeFn("completeE2ePairing", _mockCompleteE2ePairing);
export const listE2eDevices = makeFn("listE2eDevices", _mockListE2eDevices);
export const revokeE2eDevice = makeFn("revokeE2eDevice", _mockRevokeE2eDevice);

/** 获取集合成员；浏览器预览将按现有 mock 列表返回，便于页面结构预览。 */
export async function listCollectionMemories(collectionId: string): Promise<import("./types").MemorySummary[]> {
  if (!isTauriRuntime()) return _mockList();
  const api = await import("./api");
  return api.listCollectionMemories(collectionId);
}

/** 返回桌面端本地服务诊断；浏览器预览始终视为可用的 mock 数据源。 */
export async function getServiceStatus(): Promise<import("./types").ServiceStatus> {
  if (!isTauriRuntime()) {
    return { role: "holder", endpoint: "浏览器预览", available: true, message: null };
  }
  const api = await import("./api");
  return api.getServiceStatus();
}
