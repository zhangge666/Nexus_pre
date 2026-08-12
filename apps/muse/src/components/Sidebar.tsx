/** 本文件实现 Muse 正式应用的紧凑页面导航与本地状态入口。 */

import React from "react";
import {
  CalendarDays,
  CheckSquare2,
  ClipboardCopy,
  Lightbulb,
  Mic2,
  Settings2,
} from "lucide-react";
import type { MuseView } from "../core/types";

interface SidebarProps {
  activeView: MuseView;
  taskCount: number;
  onNavigate: (view: MuseView) => void;
}

const primaryItems = [
  { id: "today" as const, label: "工作流", icon: CalendarDays },
  { id: "ideas" as const, label: "灵感", icon: Lightbulb },
  { id: "tasks" as const, label: "任务留痕", icon: CheckSquare2 },
  { id: "meetings" as const, label: "会议", icon: Mic2 },
  { id: "clipboard" as const, label: "剪贴板", icon: ClipboardCopy },
];

/** 渲染低噪声侧栏，并把设置固定在底部。 */
export function Sidebar({ activeView, taskCount, onNavigate }: SidebarProps): React.JSX.Element {
  return (
    <aside className="app-sidebar">
      <nav className="sidebar-nav" aria-label="Muse 页面">
        <span className="sidebar-label">工作台</span>
        {primaryItems.map(({ id, label, icon: Icon }) => (
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
      <div className="sidebar-shortcuts" aria-label="全局快捷键">
        <span className="sidebar-label">快速唤起</span>
        <p><span>灵感</span><kbd>Ctrl ⇧ I</kbd></p>
        <p><span>任务</span><kbd>Ctrl ⇧ T</kbd></p>
        <p><span>会议</span><kbd>Ctrl ⇧ R</kbd></p>
        <p><span>剪贴板</span><kbd>Ctrl ⇧ V</kbd></p>
      </div>
      <div className="local-mode-note">
        <span className="status-dot" />
        <div>
          <strong>独立运行中</strong>
          <span>Orbit 是可选连接</span>
        </div>
      </div>
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
