/** 本文件实现顶部栏组件。 */
import type React from "react";

interface TopbarProps {
  title: string;
  subtitle?: string;
  actions?: React.ReactNode;
}

export function Topbar({ title, subtitle, actions }: TopbarProps): React.JSX.Element {
  return (
    <header className="topbar">
      <div>
        <h1>{title}</h1>
        {subtitle && <p aria-live="polite">{subtitle}</p>}
      </div>
      {actions && <div className="topbar-actions">{actions}</div>}
    </header>
  );
}
