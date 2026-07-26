/** 本文件实现 Muse 页面统一使用的紧凑标题栏。 */

import React, { type ReactNode } from "react";

interface PageHeaderProps {
  eyebrow?: string;
  title: string;
  description: string;
  actions?: ReactNode;
}

/** 渲染页面位置、说明与少量上下文动作。 */
export function PageHeader({ eyebrow, title, description, actions }: PageHeaderProps): React.JSX.Element {
  return (
    <header className="page-header">
      <div>
        {eyebrow ? <span className="page-eyebrow">{eyebrow}</span> : null}
        <h1>{title}</h1>
        <p>{description}</p>
      </div>
      {actions ? <div className="page-actions">{actions}</div> : null}
    </header>
  );
}
