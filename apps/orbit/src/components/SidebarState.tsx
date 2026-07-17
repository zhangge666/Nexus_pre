/** 本文件管理 Orbit 全局侧边栏的展开状态，供标题栏控制和导航区域同步使用。 */
import type React from "react";
import { createContext, useContext, useMemo, useState } from "react";

interface SidebarState {
  collapsed: boolean;
  toggle: () => void;
}

const SidebarContext = createContext<SidebarState | null>(null);

/** 为应用壳提供唯一的侧边栏状态源，避免各页面标题栏出现不同步的伸缩按钮。 */
export function SidebarProvider({ children }: { children: React.ReactNode }): React.JSX.Element {
  const [collapsed, setCollapsed] = useState(false);
  const value = useMemo(
    () => ({ collapsed, toggle: () => setCollapsed((current) => !current) }),
    [collapsed],
  );
  return <SidebarContext.Provider value={value}>{children}</SidebarContext.Provider>;
}

/** 读取全局侧边栏状态；仅允许在 `SidebarProvider` 管理的 Orbit 应用壳内调用。 */
export function useSidebar(): SidebarState {
  const value = useContext(SidebarContext);
  if (!value) throw new Error("useSidebar 必须在 SidebarProvider 内使用");
  return value;
}
