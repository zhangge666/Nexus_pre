/** 本文件定义 Muse 可选 Orbit 连接使用的 IPC 类型与调用。 */
import { invoke, isTauri } from "@tauri-apps/api/core";

/** 本地服务连接状态。 */
export interface ConnectionStatus {
  state: "connected" | "disconnected";
  endpoint: string | null;
  message: string | null;
}

/** 文字灵感创建成功后的最小结果。 */
export interface CreatedMemory {
  id: string;
  created_at: number;
}

/** 判断当前页面是否运行在 Muse Tauri WebView 中。 */
export function isTauriRuntime(): boolean {
  return isTauri();
}

/** 发现可选 Orbit 本地服务并登记 Muse 写入授权。 */
export function connectService(): Promise<ConnectionStatus> {
  return invoke<ConnectionStatus>("connect_service");
}

/** 读取当前进程保存的连接状态。 */
export function getConnectionStatus(): Promise<ConnectionStatus> {
  return invoke<ConnectionStatus>("get_connection_status");
}

/** 把已在本机保存的文字灵感额外同步到 Orbit。 */
export function submitIdea(content: string): Promise<CreatedMemory> {
  return invoke<CreatedMemory>("submit_idea", { content });
}
