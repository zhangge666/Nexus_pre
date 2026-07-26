/** 本文件集中处理 Muse 列表中的本地时间与任务状态文案。 */

import type { TaskStatus } from "./types";

/** 把时间戳格式化为适合紧凑列表的中文时间。 */
export function formatCompactTime(timestamp: number): string {
  const date = new Date(timestamp);
  const today = new Date();
  if (date.toDateString() === today.toDateString()) {
    return date.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", hour12: false });
  }
  return date.toLocaleDateString("zh-CN", { month: "numeric", day: "numeric" });
}

/** 返回用户可读的任务状态。 */
export function taskStatusLabel(status: TaskStatus): string {
  return { todo: "待处理", doing: "进行中", waiting: "等待", done: "已完成" }[status];
}
