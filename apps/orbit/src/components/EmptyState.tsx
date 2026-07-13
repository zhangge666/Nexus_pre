/** 本文件实现空状态占位组件。 */
import type React from "react";

interface EmptyStateProps {
  icon: React.ReactNode;
  title: string;
  description?: string;
  action?: { label: string; onClick: () => void };
}

export function EmptyState({ icon, title, description, action }: EmptyStateProps): React.JSX.Element {
  return (
    <div className="empty-state">
      <span className="empty-icon">{icon}</span>
      <h3>{title}</h3>
      {description && <p>{description}</p>}
      {action && (
        <button className="primary-small" onClick={action.onClick}>
          {action.label}
        </button>
      )}
    </div>
  );
}
