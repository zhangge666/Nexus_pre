/** 本文件实现顶部栏组件。 */
import type React from "react";

interface TopbarProps {
  title: string;
  subtitle?: string;
  actions?: React.ReactNode;
}

/** 渲染工作区位置、低干扰状态反馈与当前页面主要操作。 */
export function Topbar({ title, subtitle, actions }: TopbarProps): React.JSX.Element {
  return (
    <header className="topbar">
      <div>
        <h1>{title}</h1>
        {subtitle && <p aria-live="polite">{subtitle}</p>}
      </div>
      <div className="topbar-actions">
        {actions}
      </div>
    </header>
  );
}
