/** 本文件负责把剪贴板文本转换为可逐项推进的稳定条目列表。 */

import type { ClipboardSplitMode } from "./types";

/** 根据用户选择的模式拆分文本；智能模式优先尊重换行，否则按空白分隔。 */
export function splitClipboardContent(content: string, mode: ClipboardSplitMode): string[] {
  const value = content.trim();
  if (!value) return [];
  if (mode === "lines") return value.split(/\r?\n+/).map((item) => item.trim()).filter(Boolean);
  if (mode === "spaces") return value.split(/\s+/).map((item) => item.trim()).filter(Boolean);

  const lines = value.split(/\r?\n+/).map((item) => item.trim()).filter(Boolean);
  return lines.length > 1 ? lines : value.split(/\s+/).map((item) => item.trim()).filter(Boolean);
}
