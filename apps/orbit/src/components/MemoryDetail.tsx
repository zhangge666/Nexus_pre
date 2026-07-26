/** 本文件实现记忆详情面板组件。 */
import type React from "react";
import { useEffect, useState } from "react";
import { X, Save, Pencil, Layers, BookmarkCheck, Trash2, TriangleAlert, GitMerge, RotateCcw } from "lucide-react";
import { getMemoryConflicts, resolveMemoryConflict } from "../core";
import type { MemorySummary, MemoryCollection, MemoryConflictSet, MemoryConflictVersion } from "../core";

interface MemoryDetailProps {
  memory: MemorySummary;
  collections: MemoryCollection[];
  onClose: () => void;
  onSave: (id: string, title: string | null, content: string) => Promise<void>;
  onAddToCollection: (collectionId: string) => Promise<void>;
  onDelete?: (id: string) => Promise<void>;
  onGenerateCard?: () => void;
  onConflictResolved?: (memory: MemorySummary) => void;
}

/** 将记忆创建时间格式化为检查器中稳定可追溯的完整日期。 */
function formatDate(ts: number): string {
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric", month: "long", day: "numeric",
    hour: "2-digit", minute: "2-digit",
  }).format(ts);
}

/** 渲染详情中允许的轻量 Markdown 标记。 */
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

/** 转换正文中的粗体、行内代码和强调标记。 */
function formatInline(text: string): string {
  return text
    .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
    .replace(/`(.+?)`/g, "<code>$1</code>")
    .replace(/_(.+?)_/g, "<em>$1</em>");
}

/** 渲染按需打开的记忆检查器，并提供查看、编辑与归档入口。 */
export function MemoryDetail({
  memory,
  collections,
  onClose,
  onSave,
  onAddToCollection,
  onDelete,
  onGenerateCard,
  onConflictResolved,
}: MemoryDetailProps): React.JSX.Element {
  const [editing, setEditing] = useState(false);
  const [editTitle, setEditTitle] = useState(memory.title ?? "");
  const [editContent, setEditContent] = useState(memory.content);
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [saveError, setSaveError] = useState("");
  const [conflictOpen, setConflictOpen] = useState(false);
  const [conflicts, setConflicts] = useState<MemoryConflictSet | null>(null);
  const [conflictMode, setConflictMode] = useState<"versions" | "merge">("versions");
  const [conflictBusy, setConflictBusy] = useState(false);
  const [conflictError, setConflictError] = useState("");
  const [mergeTitle, setMergeTitle] = useState(memory.title ?? "");
  const [mergeContent, setMergeContent] = useState(memory.content);

  useEffect(() => {
    setEditTitle(memory.title ?? "");
    setEditContent(memory.content);
    setMergeTitle(memory.title ?? "");
    setMergeContent(memory.content);
    if (!memory.conflictCount) {
      setConflictOpen(false);
      setConflicts(null);
    }
  }, [memory.content, memory.conflictCount, memory.id, memory.title, memory.updatedAt]);

  /** 保存编辑内容并在成功后退出编辑态，避免阅读与编辑状态混杂。 */
  async function handleSave(): Promise<void> {
    setSaving(true);
    setSaveError("");
    try {
      await onSave(memory.id, editTitle.trim() || null, editContent.trim());
      setEditing(false);
    } catch (error) {
      // 保留用户草稿并在编辑区域原位展示错误，使用户可以直接修正或重试。
      setSaveError(`保存失败：${String(error)}`);
    } finally {
      setSaving(false);
    }
  }

  /** 二次确认后删除当前记忆；E2E 模式会由原生层写入墓碑。 */
  async function handleDelete(): Promise<void> {
    if (!onDelete || !window.confirm("删除后会同步到其他设备，是否继续？")) return;
    setDeleting(true);
    setSaveError("");
    try {
      await onDelete(memory.id);
      onClose();
    } catch (error) {
      setSaveError(`删除失败：${String(error)}`);
    } finally {
      setDeleting(false);
    }
  }

  /** 打开冲突检查器并读取最新副本，避免依据列表中的过期计数解决冲突。 */
  async function handleOpenConflicts(): Promise<void> {
    if (conflictOpen) {
      setConflictOpen(false);
      return;
    }
    setConflictOpen(true);
    setConflictBusy(true);
    setConflictError("");
    try {
      const loaded = await getMemoryConflicts(memory.id);
      setConflicts(loaded);
      const current = loaded.versions.find((version) => version.isCurrent)?.memory ?? memory;
      setMergeTitle(current.title ?? "");
      setMergeContent(current.content);
    } catch (error) {
      setConflictError(`读取冲突版本失败：${String(error)}`);
    } finally {
      setConflictBusy(false);
    }
  }

  /** 采用检查器中的一个内容版本，并由原生同步层生成新的因果后继版本。 */
  async function handleRestoreConflict(version: MemoryConflictVersion): Promise<void> {
    if (!version.memory) return;
    setConflictBusy(true);
    setConflictError("");
    try {
      const updated = await resolveMemoryConflict(memory.id, {
        strategy: "restore",
        versionId: version.versionId,
        expectedVersionIds: conflicts?.versions.map((candidate) => candidate.versionId) ?? [],
      });
      setConflicts(null);
      setConflictOpen(false);
      onConflictResolved?.(updated);
    } catch (error) {
      setConflictError(`恢复版本失败：${String(error)}`);
    } finally {
      setConflictBusy(false);
    }
  }

  /** 提交用户整理后的标题和正文，清除已观察到的旧冲突留痕。 */
  async function handleMergeConflict(): Promise<void> {
    if (!mergeContent.trim()) {
      setConflictError("合并后的正文不能为空");
      return;
    }
    setConflictBusy(true);
    setConflictError("");
    try {
      const updated = await resolveMemoryConflict(memory.id, {
        strategy: "merge",
        title: mergeTitle.trim() || null,
        content: mergeContent.trim(),
        expectedVersionIds: conflicts?.versions.map((version) => version.versionId) ?? [],
      });
      setConflicts(null);
      setConflictOpen(false);
      onConflictResolved?.(updated);
    } catch (error) {
      setConflictError(`保存合并版本失败：${String(error)}`);
    } finally {
      setConflictBusy(false);
    }
  }

  /** 在冲突处理页签间提供标准左右方向键导航，并同步移动键盘焦点。 */
  function handleConflictTabKeyDown(event: React.KeyboardEvent<HTMLButtonElement>): void {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    const nextMode = event.key === "ArrowLeft" ? "versions" : "merge";
    setConflictMode(nextMode);
    event.currentTarget.parentElement
      ?.querySelector<HTMLButtonElement>(`[data-conflict-tab="${nextMode}"]`)
      ?.focus();
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
              {onDelete && (
                <button className="detail-action danger-text" onClick={() => void handleDelete()} disabled={deleting}>
                  <Trash2 size={13} />{deleting ? "删除中…" : "删除"}
                </button>
              )}
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
            {saveError && <p className="detail-save-error" role="alert">{saveError}</p>}
          </>
        ) : (
          <>
            <div className="detail-meta">
              <span className={`source-badge src-${memory.source.split(":")[0]}`}>{sourceLabel}</span>
              <span className="detail-time">{formatDate(memory.createdAt)}</span>
              {memory.pinned && <span className="pin-badge"><BookmarkCheck size={11} />置顶</span>}
            </div>
            {!!memory.conflictCount && (
              <>
                <div className="detail-conflict-notice" role="status">
                  <TriangleAlert size={13} aria-hidden="true" />
                  <span>已保留 {memory.conflictCount} 个并发版本，当前内容已按确定性规则收敛。</span>
                  <button type="button" onClick={() => void handleOpenConflicts()} aria-expanded={conflictOpen}>
                    {conflictOpen ? "收起" : "查看版本"}
                  </button>
                </div>
                {conflictOpen && (
                  <section className="conflict-inspector" aria-label="并发版本检查器" aria-busy={conflictBusy}>
                    <div className="conflict-tabs" role="tablist" aria-label="冲突处理方式">
                      <button type="button" role="tab" id="conflict-tab-versions" data-conflict-tab="versions" aria-controls="conflict-panel-versions" aria-selected={conflictMode === "versions"} tabIndex={conflictMode === "versions" ? 0 : -1} className={conflictMode === "versions" ? "active" : ""} onClick={() => setConflictMode("versions")} onKeyDown={handleConflictTabKeyDown}>
                        版本预览
                      </button>
                      <button type="button" role="tab" id="conflict-tab-merge" data-conflict-tab="merge" aria-controls="conflict-panel-merge" aria-selected={conflictMode === "merge"} tabIndex={conflictMode === "merge" ? 0 : -1} className={conflictMode === "merge" ? "active" : ""} onClick={() => setConflictMode("merge")} onKeyDown={handleConflictTabKeyDown}>
                        <GitMerge size={12} aria-hidden="true" />手工合并
                      </button>
                    </div>
                    {conflictBusy && !conflicts && <p className="conflict-loading">正在读取加密副本…</p>}
                    {conflictMode === "versions" && conflicts && (
                      <div className="conflict-version-list" role="tabpanel" id="conflict-panel-versions" aria-labelledby="conflict-tab-versions">
                        {conflicts.versions.map((version) => (
                          <article key={version.versionId} className={`conflict-version${version.isCurrent ? " current" : ""}`}>
                            <header>
                              <span>{version.isCurrent ? "当前版本" : "并发版本"}</span>
                              <time dateTime={new Date(version.modifiedAt).toISOString()}>{formatDate(version.modifiedAt)}</time>
                            </header>
                            <code title={version.deviceId}>{version.deviceId}</code>
                            {version.memory ? (
                              <>
                                <h4>{version.memory.title ?? version.memory.kind}</h4>
                                <pre>{version.memory.content}</pre>
                                <button type="button" className="secondary-button conflict-restore" disabled={conflictBusy} onClick={() => void handleRestoreConflict(version)}>
                                  <RotateCcw size={12} aria-hidden="true" />{version.isCurrent ? "保留当前版本" : "恢复此版本"}
                                </button>
                              </>
                            ) : (
                              <p className="conflict-tombstone">此并发版本已删除记忆，保留为墓碑记录。</p>
                            )}
                          </article>
                        ))}
                      </div>
                    )}
                    {conflictMode === "merge" && (
                      <div className="conflict-merge-editor" role="tabpanel" id="conflict-panel-merge" aria-labelledby="conflict-tab-merge">
                        <label>
                          合并标题
                          <input value={mergeTitle} onChange={(event) => setMergeTitle(event.target.value)} placeholder="可选标题" />
                        </label>
                        <label>
                          合并正文
                          <textarea value={mergeContent} onChange={(event) => setMergeContent(event.target.value)} />
                        </label>
                        <button type="button" className="primary-small" disabled={conflictBusy || !mergeContent.trim()} onClick={() => void handleMergeConflict()}>
                          <GitMerge size={13} aria-hidden="true" />{conflictBusy ? "保存中…" : "保存合并版本"}
                        </button>
                      </div>
                    )}
                    {conflictError && <p className="conflict-error" role="alert">{conflictError}</p>}
                  </section>
                )}
              </>
            )}
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
                  <option key={col.id} value={col.id}>{col.name}</option>
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
