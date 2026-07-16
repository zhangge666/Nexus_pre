/** 本文件定义 Muse M3 最小来源前端使用的 IPC 类型与调用。 */
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

/** 发现 Orbit 本地服务并登记 Muse 最小写入授权。 */
export function connectService(): Promise<ConnectionStatus> {
  return invoke<ConnectionStatus>("connect_service");
}

/** 读取当前进程保存的连接状态。 */
export function getConnectionStatus(): Promise<ConnectionStatus> {
  return invoke<ConnectionStatus>("get_connection_status");
}

/** 以固定 Muse 来源提交一条文字灵感。 */
export function submitIdea(content: string): Promise<CreatedMemory> {
  return invoke<CreatedMemory>("submit_idea", { content });
}
