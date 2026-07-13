/** 本文件实现时间线页面，按日期分组显示记忆，带时间轴线视觉效果。 */
import type React from "react";
import { useState, useEffect, useCallback } from "react";
import { BookOpenText, Filter } from "lucide-react";
import { listMemories, getMemory, updateMemory, listCollections, addMemoryToCollection } from "../core";
import type { MemorySummary, MemoryCollection } from "../core";
import { Topbar } from "../components/Topbar";
import { MemoryDetail } from "../components/MemoryDetail";
import { EmptyState } from "../components/EmptyState";

const SOURCES = ["all", "orbit", "muse", "quill", "echo"] as const;

function groupByDate(memories: MemorySummary[]): { label: string; items: MemorySummary[] }[] {
  const groups: Map<string, MemorySummary[]> = new Map();
  const now = Date.now();

  for (const mem of memories) {
    const diff = now - mem.createdAt;
    let label: string;
    if (diff < 86_400_000) {
      label = "今天";
    } else if (diff < 172_800_000) {
      label = "昨天";
    } else {
      label = new Intl.DateTimeFormat("zh-CN", { month: "long", day: "numeric" }).format(mem.createdAt);
    }
    if (!groups.has(label)) groups.set(label, []);
    groups.get(label)!.push(mem);
  }
  return Array.from(groups.entries()).map(([label, items]) => ({ label, items }));
}

function formatTimeShort(ts: number): string {
  return new Intl.DateTimeFormat("zh-CN", { hour: "2-digit", minute: "2-digit" }).format(ts);
}

const KIND_ICON: Record<string, string> = {
  note: "📝", idea: "💡", screen: "🖥", voice: "🎤", card: "🃏", clip: "📎", file: "📄",
};

export default function TimelinePage(): React.JSX.Element {
  const [memories, setMemories] = useState<MemorySummary[]>([]);
  const [selected, setSelected] = useState<MemorySummary | null>(null);
  const [source, setSource] = useState("all");
  const [collections, setCollections] = useState<MemoryCollection[]>([]);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async (src?: string) => {
    setBusy(true);
    try {
      const loaded = await listMemories(src ?? source);
      setMemories(loaded);
    } finally {
      setBusy(false);
    }
  }, [source]);

  useEffect(() => {
    void refresh();
    void listCollections().then(setCollections);
  }, []);

  async function handleSelect(mem: MemorySummary): Promise<void> {
    setSelected(mem);
    try { setSelected(await getMemory(mem.id)); } catch {}
  }

  function handleSourceChange(s: string): void {
    setSource(s);
    void refresh(s);
  }

  const displayed = source === "all" ? memories : memories.filter((m) => m.source === source);
  const groups = groupByDate(displayed);

  return (
    <div className="page-enter timeline-page">
      <Topbar title="时间线" subtitle={`${displayed.length} 条记忆`} />

      {/* 过滤器 */}
      <div className="timeline-filters">
        <Filter size={14} aria-hidden="true" />
        <div className="filter-row" role="group" aria-label="来源筛选">
          {SOURCES.map((s) => (
            <button
              key={s}
              className={`filter-button${source === s ? " active" : ""}`}
              onClick={() => handleSourceChange(s)}
            >
              {s === "all" ? "全部" : s.charAt(0).toUpperCase() + s.slice(1)}
            </button>
          ))}
        </div>
      </div>

      <div className="timeline-layout">
        {/* 时间线列表 */}
        <div className="timeline-list">
          {busy && <p className="loading-hint">加载中…</p>}

          {!busy && groups.length === 0 && (
            <EmptyState
              icon={<BookOpenText size={36} />}
              title="还没有记忆"
              description="通过 Echo、Muse 或 Quill 开始记录"
            />
          )}

          {groups.map(({ label, items }) => (
            <div key={label} className="timeline-group">
              <div className="timeline-date">
                {label} <span className="timeline-count">({items.length})</span>
              </div>
              {items.map((mem) => (
                <div
                  key={mem.id}
                  className={`timeline-item${selected?.id === mem.id ? " selected" : ""}`}
                  onClick={() => void handleSelect(mem)}
                  role="button"
                  tabIndex={0}
                  onKeyDown={(e) => e.key === "Enter" && void handleSelect(mem)}
                >
                  <span className="timeline-dot" />
                  <span className="timeline-time">{mem.createdAt ? formatTimeShort(mem.createdAt) : "--:--"}</span>
                  <span className="timeline-kind">{KIND_ICON[mem.kind] ?? "📄"}</span>
                  <div className="timeline-body">
                    <strong>{mem.title ?? mem.kind}</strong>
                    <p>{mem.content.slice(0, 80)}{mem.content.length > 80 ? "…" : ""}</p>
                    {mem.tags.length > 0 && (
                      <span className="tag-list">
                        {mem.tags.slice(0, 2).map((t) => <em key={t}>{t}</em>)}
                      </span>
                    )}
                  </div>
                </div>
              ))}
            </div>
          ))}
        </div>

        {/* 详情面板 */}
        {selected && (
          <aside className="timeline-detail">
            <MemoryDetail
              memory={selected}
              collections={collections}
              onClose={() => setSelected(null)}
              onSave={async (id, title, content) => {
                const updated = await updateMemory(id, title, content);
                setMemories((ms) => ms.map((m) => m.id === id ? updated : m));
                setSelected(updated);
              }}
              onAddToCollection={(colId) => addMemoryToCollection(colId, selected.id)}
            />
          </aside>
        )}
      </div>
    </div>
  );
}
