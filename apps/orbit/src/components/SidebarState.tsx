/** 本文件管理 Orbit 全局侧边栏的展开状态，供标题栏控制和导航区域同步使用。 */
import type React from "react";
import { createContext, useContext, useMemo, useState } from "react";

interface SidebarState {
  collapsed: boolean;
  hidden: boolean;
  toggle: () => void;
  setCollapsed: (collapsed: boolean) => void;
  setHidden: (hidden: boolean) => void;
}

const SidebarContext = createContext<SidebarState | null>(null);

/** 为应用壳提供唯一的侧边栏状态源，避免各页面标题栏出现不同步的伸缩按钮。 */
export function SidebarProvider({ children }: { children: React.ReactNode }): React.JSX.Element {
  const [collapsed, setCollapsed] = useState(false);
  const [hidden, setHidden] = useState(false);
  const value = useMemo(
    () => ({
      collapsed,
      hidden,
      /** 完全隐藏时优先恢复完整侧栏，避免标题栏按钮切换到不可见的图标栏状态。 */
      toggle: () => {
        if (hidden) {
          setHidden(false);
          setCollapsed(false);
          return;
        }
        setCollapsed((current) => !current);
      },
      /** 拖拽重新展开侧栏时同步退出完全隐藏状态。 */
      setCollapsed: (nextCollapsed: boolean) => {
        if (!nextCollapsed) setHidden(false);
        setCollapsed(nextCollapsed);
      },
      /** 完全隐藏使用独立状态，保留原有的图标栏收缩行为。 */
      setHidden: (nextHidden: boolean) => {
        setHidden(nextHidden);
        if (nextHidden) setCollapsed(true);
      },
    }),
    [collapsed, hidden],
  );
  return <SidebarContext.Provider value={value}>{children}</SidebarContext.Provider>;
}

/** 读取全局侧边栏状态；仅允许在 `SidebarProvider` 管理的 Orbit 应用壳内调用。 */
export function useSidebar(): SidebarState {
  const value = useContext(SidebarContext);
  if (!value) throw new Error("useSidebar 必须在 SidebarProvider 内使用");
  return value;
}
