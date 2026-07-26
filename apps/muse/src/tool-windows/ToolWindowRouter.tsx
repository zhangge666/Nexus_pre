/** 本文件根据 Tauri 窗口查询参数挂载对应的 Muse 专用工具界面。 */

import React from "react";
import { ClipboardToolWindow } from "./ClipboardToolWindow";
import { IdeaToolWindow } from "./IdeaToolWindow";
import { MeetingToolWindow } from "./MeetingToolWindow";
import { TaskToolWindow } from "./TaskToolWindow";
import "./tool-windows.css";

export type ToolWindowType = "idea" | "task" | "meeting" | "clipboard";

interface ToolWindowRouterProps {
  type: ToolWindowType;
}

/** 确保每个快捷键窗口只渲染自己的任务界面，而不是聚合主页。 */
export function ToolWindowRouter({ type }: ToolWindowRouterProps): React.JSX.Element {
  if (type === "idea") return <IdeaToolWindow />;
  if (type === "task") return <TaskToolWindow />;
  if (type === "meeting") return <MeetingToolWindow />;
  return <ClipboardToolWindow />;
}

/** 判断查询参数是否对应受支持的专用工具窗。 */
export function isToolWindowType(value: string | null): value is ToolWindowType {
  return value === "idea" || value === "task" || value === "meeting" || value === "clipboard";
}
