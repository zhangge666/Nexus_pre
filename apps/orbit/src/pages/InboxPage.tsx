/** 本文件实现收件箱页面，展示新记忆、自动关联、去重提示等通知项。 */
import type React from "react";
import { useState, useEffect } from "react";
import { Inbox, CheckCheck } from "lucide-react";
import { listInboxItems, markInboxRead } from "../core";
import type { InboxItem, InboxItemType } from "../core";
import { Topbar } from "../components/Topbar";
import { EmptyState } from "../components/EmptyState";
import { useNavigate } from "react-router-dom";

const TYPE_CONFIG: Record<InboxItemType, { icon: string; color: string; label: string }> = {
  new_memory:           { icon: "🟢", color: "success",  label: "新记忆" },
  auto_link:            { icon: "🔗", color: "primary",  label: "自动关联" },
  duplicate_suggestion: { icon: "⚠️", color: "warning",  label: "疑似重复" },
  review_due:           { icon: "🃏", color: "muted",    label: "复习到期" },
};

function groupByDate(items: InboxItem[]): { label: string; items: InboxItem[] }[] {
  const now = Date.now();
  const groups: Map<string, InboxItem[]> = new Map();
  for (const item of items) {
    const diff = now - item.createdAt;
    const label = diff < 86_400_000 ? "今天" : diff < 172_800_000 ? "昨天" : "更早";
    if (!groups.has(label)) groups.set(label, []);
    groups.get(label)!.push(item);
  }
  return Array.from(groups.entries()).map(([label, items]) => ({ label, items }));
}

function InboxCard({ item, onMarkRead }: { item: InboxItem; onMarkRead: (id: string) => void }): React.JSX.Element {
  const navigate = useNavigate();
  const cfg = TYPE_CONFIG[item.type];

  function handlePrimary(): void {
    onMarkRead(item.id);
    if (item.type === "review_due") navigate("/review");
  }

  return (
    <div className={`inbox-card${item.read ? " read" : ""}`}>
      <div className="inbox-card-header">
        <span className={`inbox-type-badge type-${cfg.color}`}>
          {cfg.icon} {cfg.label}
        </span>
        {!item.read && <span className="unread-dot" aria-label="未读" />}
      </div>
      <div className="inbox-card-body">
        <strong>{item.memory.title ?? item.memory.kind}</strong>
        <p className="inbox-source">{item.memory.source} · {new Intl.DateTimeFormat("zh-CN", { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" }).format(item.createdAt)}</p>
        {item.suggestion && <p className="inbox-suggestion">{item.suggestion}</p>}
        {item.relatedMemoryTitle && (
          <p className="inbox-related">关联：<em>{item.relatedMemoryTitle}</em></p>
        )}
      </div>
      <div className="inbox-card-actions">
        {item.type === "new_memory" && (
          <>
            <button className="detail-action" onClick={handlePrimary}>查看</button>
            <button className="detail-action" onClick={() => onMarkRead(item.id)}>归档</button>
          </>
        )}
        {item.type === "auto_link" && (
          <>
            <button className="detail-action primary-action" onClick={handlePrimary}>确认关联</button>
            <button className="detail-action" onClick={() => onMarkRead(item.id)}>忽略</button>
          </>
        )}
        {item.type === "duplicate_suggestion" && (
          <>
            <button className="detail-action primary-action" onClick={handlePrimary}>合并</button>
            <button className="detail-action" onClick={() => onMarkRead(item.id)}>保留两条</button>
          </>
        )}
        {item.type === "review_due" && (
          <button className="detail-action primary-action" onClick={handlePrimary}>开始复习</button>
        )}
      </div>
    </div>
  );
}

export default function InboxPage(): React.JSX.Element {
  const [items, setItems] = useState<InboxItem[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    void listInboxItems().then((its) => { setItems(its); setLoading(false); });
  }, []);

  async function handleMarkRead(id: string): Promise<void> {
    await markInboxRead(id);
    setItems((is) => is.map((i) => i.id === id ? { ...i, read: true } : i));
  }

  async function handleMarkAllRead(): Promise<void> {
    await Promise.all(items.filter((i) => !i.read).map((i) => markInboxRead(i.id)));
    setItems((is) => is.map((i) => ({ ...i, read: true })));
  }

  const unreadCount = items.filter((i) => !i.read).length;
  const groups = groupByDate(items);

  return (
    <div className="page-enter inbox-page">
      <Topbar
        title="收件箱"
        subtitle={unreadCount > 0 ? `${unreadCount} 条未处理` : "全部已处理"}
        actions={
          unreadCount > 0 && (
            <button className="secondary-button" onClick={() => void handleMarkAllRead()}>
              <CheckCheck size={14} />全部已读
            </button>
          )
        }
      />

      <div className="inbox-content">
        {loading && <p className="loading-hint">加载中…</p>}
        {!loading && items.length === 0 && (
          <EmptyState
            icon={<Inbox size={40} />}
            title="收件箱为空"
            description="新到达的记忆和通知会出现在这里"
          />
        )}

        {groups.map(({ label, items: groupItems }) => (
          <div key={label} className="inbox-group">
            <div className="inbox-group-label">{label}</div>
            <div className="inbox-cards">
              {groupItems.map((item) => (
                <InboxCard
                  key={item.id}
                  item={item}
                  onMarkRead={(id) => void handleMarkRead(id)}
                />
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
