/** 本文件定义 Muse 独立工作区使用的页面、内容与本地持久化类型。 */

export type MuseView = "today" | "ideas" | "tasks" | "meetings" | "clipboard" | "settings";
export type TaskStatus = "todo" | "doing" | "waiting" | "done";
export type SyncState = "local" | "syncing" | "synced" | "error";

/** 一条由 Muse 捕捉的灵感。 */
export interface MuseIdea {
  id: string;
  content: string;
  createdAt: number;
  syncState: SyncState;
}

/** 任务时间线中的证据或状态活动。 */
export interface TaskActivity {
  id: string;
  type: "source" | "file" | "note" | "status";
  title: string;
  detail: string;
  createdAt: number;
}

/** 可独立保存在 Muse 本机的工作任务。 */
export interface MuseTask {
  id: string;
  title: string;
  description: string;
  status: TaskStatus;
  dueLabel: string;
  requester: string;
  source: string;
  project: string;
  activities: TaskActivity[];
}

/** 已完成或正在整理的会议记录。 */
export interface MuseMeeting {
  id: string;
  title: string;
  summary: string;
  recordedAt: number;
  durationLabel: string;
  actionItems: string[];
}

/** Muse 本地剪贴板暂存条目。 */
export interface MuseClipboardItem {
  id: string;
  title: string;
  content: string;
  source: string;
  copiedAt: number;
  pinned: boolean;
}

/** Muse 单机模式持久化的完整工作区。 */
export interface MuseWorkspace {
  ideas: MuseIdea[];
  tasks: MuseTask[];
  meetings: MuseMeeting[];
  clipboard: MuseClipboardItem[];
}
