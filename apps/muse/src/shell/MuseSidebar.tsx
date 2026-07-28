/** 本文件负责 Muse 主窗口的全局导航、命令入口与可选 Orbit 状态。 */

import React from "react";
import type { LucideIcon } from "lucide-react";
import {
  CalendarDays,
  CheckSquare2,
  ClipboardCopy,
  Lightbulb,
  Mic2,
  Search,
  Settings2,
} from "lucide-react";
import museIcon from "../assets/muse-app-icon.svg";
import { isMacOS } from "../core/platform";
import type { MuseView } from "../core/types";

interface MuseSidebarProps {
  activeView: MuseView;
  taskCount: number;
  onNavigate: (view: MuseView) => void;
  onOpenCommand: () => void;
}

interface SidebarItem {
  id: Exclude<MuseView, "settings">;
  label: string;
  icon: LucideIcon;
}

const sidebarItems: SidebarItem[] = [
  { id: "today", label: "今天", icon: CalendarDays },
  { id: "ideas", label: "灵感", icon: Lightbulb },
  { id: "tasks", label: "任务", icon: CheckSquare2 },
  { id: "meetings", label: "会议", icon: Mic2 },
  { id: "clipboard", label: "剪贴板", icon: ClipboardCopy },
];

/** 渲染紧凑的工作区导航，并把低频设置固定到底部。 */
export function MuseSidebar({
  activeView,
  taskCount,
  onNavigate,
  onOpenCommand,
}: MuseSidebarProps): React.JSX.Element {
  return (
    <aside className="muse-sidebar">
      <div className="sidebar-product">
        <img src={museIcon} alt="" />
        <strong>Muse</strong>
      </div>

      <button className="sidebar-command" type="button" onClick={onOpenCommand}>
        <Search size={14} aria-hidden="true" />
        <span>搜索或快速创建</span>
        <kbd>{isMacOS() ? "⌘ K" : "Ctrl K"}</kbd>
      </button>

      <nav className="sidebar-nav" aria-label="Muse 工作区">
        <span className="sidebar-label">工作区</span>
        {sidebarItems.map(({ id, label, icon: Icon }) => (
          <button
            className={activeView === id ? "is-active" : ""}
            key={id}
            type="button"
            onClick={() => onNavigate(id)}
          >
            <Icon size={15} aria-hidden="true" />
            <span>{label}</span>
            {id === "tasks" && taskCount > 0 ? <small>{taskCount}</small> : null}
          </button>
        ))}
      </nav>

      <div className="sidebar-spacer" />
      <button className="sidebar-orbit" type="button" onClick={() => onNavigate("settings")}>
        <span className="orbit-symbol">O</span>
        <span>
          <strong>Orbit · 未连接</strong>
          <small>Muse 正在独立运行</small>
        </span>
      </button>
      <button
        className={`sidebar-settings ${activeView === "settings" ? "is-active" : ""}`}
        type="button"
        onClick={() => onNavigate("settings")}
      >
        <Settings2 size={15} aria-hidden="true" />
        <span>设置</span>
      </button>
    </aside>
  );
}
