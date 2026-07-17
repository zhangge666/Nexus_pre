/** 本文件实现顶部栏组件。 */
import type React from "react";
import { PanelLeft, PanelLeftClose, PanelRight, PanelRightClose } from "lucide-react";
import { useInspector } from "./Inspector";
import { useSidebar } from "./SidebarState";

interface TopbarProps {
  title: string;
  subtitle?: string;
  actions?: React.ReactNode;
}

/** 渲染工作区位置、低干扰状态反馈与当前页面主要操作。 */
export function Topbar({ title, subtitle, actions }: TopbarProps): React.JSX.Element {
  const { open, toggle } = useInspector();
  const { collapsed, toggle: toggleSidebar } = useSidebar();
  return (
    <header className="topbar">
      <div>
        <h1>{title}</h1>
        {subtitle && <p aria-live="polite">{subtitle}</p>}
      </div>
      <div className="topbar-actions">
        {actions}
        <button
          className="icon-button sidebar-toggle-button"
          onClick={toggleSidebar}
          aria-label={collapsed ? "展开侧边栏" : "收起侧边栏"}
          title={collapsed ? "展开侧边栏" : "收起侧边栏"}
        >
          {collapsed ? <PanelLeft size={16} /> : <PanelLeftClose size={16} />}
        </button>
        <button
          className="icon-button"
          onClick={toggle}
          aria-label={open ? "收起右侧检查器" : "展开右侧检查器"}
          title={open ? "收起检查器" : "展开检查器"}
        >
          {open ? <PanelRightClose size={16} /> : <PanelRight size={16} />}
        </button>
      </div>
    </header>
  );
}
