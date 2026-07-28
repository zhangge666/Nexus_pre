/** 本文件组合 Muse 独立标题栏、全局侧栏和可滚动业务工作区。 */

import React, { type ReactNode } from "react";
import type { MuseView } from "../core/types";
import { MuseSidebar } from "./MuseSidebar";
import { MuseTitlebar } from "./MuseTitlebar";

interface MuseShellProps {
  activeView: MuseView;
  taskCount: number;
  onNavigate: (view: MuseView) => void;
  onOpenCommand: () => void;
  children: ReactNode;
}

/** 渲染窗口级框架，业务页面只负责工作区内部内容。 */
export function MuseShell({
  activeView,
  taskCount,
  onNavigate,
  onOpenCommand,
  children,
}: MuseShellProps): React.JSX.Element {
  return (
    <div className="muse-app">
      <MuseTitlebar />
      <div className="muse-shell">
        <MuseSidebar
          activeView={activeView}
          taskCount={taskCount}
          onNavigate={onNavigate}
          onOpenCommand={onOpenCommand}
        />
        <main className="app-workspace">{children}</main>
      </div>
    </div>
  );
}
