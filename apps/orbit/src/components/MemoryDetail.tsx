/** 本文件实现记忆详情面板组件。 */
import type React from "react";
import { useState } from "react";
import { X, Save, Pencil, Layers, BookmarkCheck } from "lucide-react";
import type { MemorySummary, MemoryCollection } from "../core";

interface MemoryDetailProps {
  memory: MemorySummary;
  collections: MemoryCollection[];
  onClose: () => void;
  onSave: (id: string, title: string | null, content: string) => Promise<void>;
  onAddToCollection: (collectionId: string) => Promise<void>;
  onGenerateCard?: () => void;
}

function formatDate(ts: number): string {
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric", month: "long", day: "numeric",
    hour: "2-digit", minute: "2-digit",
  }).format(ts);
}

function renderMarkdown(text: string): React.JSX.Element {
  // 简单 markdown 渲染（粗体/代码/换行）
  const lines = text.split("\n");
  return (
    <div className="markdown-body">
      {lines.map((line, i) => {
        if (line.startsWith("## ")) return <h2 key={i}>{line.slice(3)}</h2>;
        if (line.startsWith("# ")) return <h1 key={i}>{line.slice(2)}</h1>;
        if (line.startsWith("### ")) return <h3 key={i}>{line.slice(4)}</h3>;
        if (line.startsWith("- ") || line.startsWith("* ")) {
          return <li key={i} dangerouslySetInnerHTML={{ __html: formatInline(line.slice(2)) }} />;
        }
        if (line === "") return <br key={i} />;
        return <p key={i} dangerouslySetInnerHTML={{ __html: formatInline(line) }} />;
      })}
    </div>
  );
}

function formatInline(text: string): string {
  return text
    .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
    .replace(/`(.+?)`/g, "<code>$1</code>")
    .replace(/_(.+?)_/g, "<em>$1</em>");
}

export function MemoryDetail({
  memory,
  collections,
  onClose,
  onSave,
  onAddToCollection,
  onGenerateCard,
}: MemoryDetailProps): React.JSX.Element {
  const [editing, setEditing] = useState(false);
  const [editTitle, setEditTitle] = useState(memory.title ?? "");
  const [editContent, setEditContent] = useState(memory.content);
  const [saving, setSaving] = useState(false);

  async function handleSave(): Promise<void> {
    setSaving(true);
    try {
      await onSave(memory.id, editTitle.trim() || null, editContent.trim());
      setEditing(false);
    } finally {
      setSaving(false);
    }
  }

  const sourceLabel = memory.source === "orbit" ? "Orbit"
    : memory.source.charAt(0).toUpperCase() + memory.source.slice(1);

  return (
    <section className="memory-detail" aria-labelledby="detail-title">
      <div className="section-heading">
        <h2 id="detail-title">记忆详情</h2>
        <span className="detail-actions">
          {!editing && (
            <>
              {onGenerateCard && (
                <button className="detail-action" onClick={onGenerateCard} title="生成卡片">
                  <Layers size={13} />生成卡片
                </button>
              )}
              <button className="detail-action" onClick={() => setEditing(true)}>
                <Pencil size={13} />编辑
              </button>
            </>
          )}
          <button className="icon-button" onClick={onClose} aria-label="关闭详情">
            <X size={15} />
          </button>
        </span>
      </div>

      <div className="detail-body">
        {editing ? (
          <>
            <input
              className="detail-input"
              value={editTitle}
              onChange={(e) => setEditTitle(e.target.value)}
              placeholder="标题"
              aria-label="记忆标题"
            />
            <textarea
              className="detail-editor"
              value={editContent}
              onChange={(e) => setEditContent(e.target.value)}
              aria-label="记忆正文"
            />
            <div className="detail-controls">
              <button className="secondary-button" onClick={() => setEditing(false)}>取消</button>
              <button className="primary-small" onClick={handleSave} disabled={saving}>
                <Save size={13} />{saving ? "保存中…" : "保存"}
              </button>
            </div>
          </>
        ) : (
          <>
            <div className="detail-meta">
              <span className={`source-badge src-${memory.source.split(":")[0]}`}>{sourceLabel}</span>
              <span className="detail-time">{formatDate(memory.createdAt)}</span>
              {memory.pinned && <span className="pin-badge"><BookmarkCheck size={11} />置顶</span>}
            </div>
            <h3 className="detail-title-text">{memory.title ?? memory.kind}</h3>
            <div className="detail-content">
              {memory.contentFormat === "markdown"
                ? renderMarkdown(memory.content)
                : <p>{memory.content}</p>}
            </div>
            {memory.tags.length > 0 && (
              <div className="tag-list detail-tags">
                {memory.tags.map((tag) => <em key={tag}>{tag}</em>)}
              </div>
            )}
            {collections.length > 0 && (
              <select
                className="collection-select"
                defaultValue=""
                onChange={(e) => {
                  if (e.target.value) void onAddToCollection(e.target.value);
                  e.currentTarget.value = "";
                }}
                aria-label="加入集合"
              >
                <option value="">加入集合…</option>
                {collections.map((col) => (
                  <option key={col.id} value={col.id}>{col.icon} {col.name}</option>
                ))}
              </select>
            )}
            <code className="detail-id">{memory.id}</code>
          </>
        )}
      </div>
    </section>
  );
}
