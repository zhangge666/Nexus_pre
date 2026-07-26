/** 本文件封装真实 Tauri IPC 调用，签名与 mock.ts 完全一致。 */

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AskStreamEvent,
  AskRequest,
  AskResponse,
  ConnectedApp,
  E2eDevice,
  E2eContentStatus,
  E2ePairingJoin,
  E2ePairingOffer,
  E2ePairingStatus,
  E2eStatus,
  RegisteredConnection,
  CreateCardRequest,
  GenerateCardsRequest,
  GradeResult,
  GraphEdge,
  GraphNode,
  InboxItem,
  MemoryCollection,
  MemoryConflictResolution,
  MemoryConflictSet,
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

/** 读取记忆当前胜出版本和全部 E2E 并发留痕。 */
export async function getMemoryConflicts(id: string): Promise<MemoryConflictSet> {
  return invoke<MemoryConflictSet>("get_memory_conflicts", { id });
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

/** 将恢复或手工合并结果提交为观察全部旧冲突的新因果版本。 */
export async function resolveMemoryConflict(
  id: string,
  resolution: MemoryConflictResolution,
): Promise<MemorySummary> {
  return invoke<MemorySummary>("resolve_memory_conflict", { id, resolution });
}

/** 删除记忆；E2E 模式由原生层生成并同步墓碑。 */
export async function deleteMemory(id: string): Promise<void> {
  return invoke<void>("delete_memory", { id });
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

/** 由 Orbit 持有者为用户确认的第三方应用签发来源受限令牌。 */
export async function registerExternalApp(
  appId: string,
  name: string,
  scopes: string[],
): Promise<RegisteredConnection> {
  return invoke<RegisteredConnection>("register_external_app", { appId, name, scopes });
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

/** 返回 Android Keystore 中 E2E 根密钥和设备身份状态。 */
export async function getE2eStatus(): Promise<E2eStatus> {
  return invoke<E2eStatus>("get_e2e_status");
}

/** 立即完成一轮 Android E2E 密文增量同步。 */
export async function syncE2eContent(): Promise<E2eContentStatus> {
  return invoke<E2eContentStatus>("sync_e2e_content");
}

/** 读取 Android E2E 加密副本的本地进度。 */
export async function getE2eContentStatus(): Promise<E2eContentStatus> {
  return invoke<E2eContentStatus>("get_e2e_content_status");
}

/** 创建首个 E2E 工作区和 Android 签名设备。 */
export async function initializeE2e(deviceName: string): Promise<E2eStatus> {
  return invoke<E2eStatus>("initialize_e2e", { deviceName });
}

/** 使用 24 词 BIP39 短语恢复 E2E 工作区。 */
export async function restoreE2e(recoveryPhrase: string, deviceName: string): Promise<E2eStatus> {
  return invoke<E2eStatus>("restore_e2e", { recoveryPhrase, deviceName });
}

/** 读取当前 E2E 根密钥对应的恢复短语。 */
export async function getRecoveryPhrase(): Promise<string> {
  return invoke<string>("get_recovery_phrase");
}

/** 创建含一次性秘密的 E2E 配对二维码。 */
export async function createE2ePairingOffer(): Promise<E2ePairingOffer> {
  return invoke<E2ePairingOffer>("create_e2e_pairing_offer");
}

/** 查询当前设备创建的配对会话状态。 */
export async function getE2ePairingStatus(): Promise<E2ePairingStatus> {
  return invoke<E2ePairingStatus>("get_e2e_pairing_status");
}

/** 使用二维码 URI 创建新设备加入申请。 */
export async function requestE2ePairing(
  pairingUri: string,
  deviceName: string,
): Promise<E2ePairingJoin> {
  return invoke<E2ePairingJoin>("request_e2e_pairing", { pairingUri, deviceName });
}

/** 批准当前配对会话中的新设备。 */
export async function approveE2ePairing(): Promise<E2eDevice> {
  return invoke<E2eDevice>("approve_e2e_pairing");
}

/** 新设备领取配对包并将根密钥写入 Android Keystore。 */
export async function completeE2ePairing(): Promise<E2eStatus> {
  return invoke<E2eStatus>("complete_e2e_pairing");
}

/** 列出当前 E2E 工作区登记的设备。 */
export async function listE2eDevices(): Promise<E2eDevice[]> {
  return invoke<E2eDevice[]>("list_e2e_devices");
}

/** 撤销指定 E2E 设备。 */
export async function revokeE2eDevice(deviceId: string): Promise<E2eDevice> {
  return invoke<E2eDevice>("revoke_e2e_device", { deviceId });
}
