/** 本文件实现按需打开的快速记录浮层，避免其持续占用检索工作区。 */
import type React from "react";
import { useEffect, useRef, useState } from "react";
import { X } from "lucide-react";
import { createMemory } from "../core";
import type { MemorySummary } from "../core";

interface QuickCaptureProps {
  open: boolean;
  onClose: () => void;
  onCreated?: (memory: MemorySummary) => void;
}

/** 渲染可由顶栏主操作唤起的快速记录浮层。 */
export function QuickCapture({ open, onClose, onCreated }: QuickCaptureProps): React.JSX.Element | null {
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("");
  const inputRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (!open) return;
    inputRef.current?.focus();

    /** 使用 Escape 让浮层具备明确且可预期的键盘退出方式。 */
    function handleEscape(event: KeyboardEvent): void {
      if (event.key === "Escape" && !busy) onClose();
    }

    window.addEventListener("keydown", handleEscape);
    return () => window.removeEventListener("keydown", handleEscape);
  }, [busy, onClose, open]);

  /** 写入草稿并将新建记忆回传给工作区，以便立即在列表中反馈结果。 */
  async function handleCreate(): Promise<void> {
    if (!draft.trim()) return;
    setBusy(true);
    try {
      const memory = await createMemory(draft.trim());
      setDraft("");
      setNotice("记忆已写入");
      onCreated?.(memory);
      window.setTimeout(() => {
        setNotice("");
        onClose();
      }, 450);
    } catch (error) {
      setNotice(`写入失败：${String(error)}`);
    } finally {
      setBusy(false);
    }
  }

  /** 支持常见的主修饰键加回车提交，保留换行输入能力。 */
  function handleKeyDown(event: React.KeyboardEvent<HTMLTextAreaElement>): void {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) void handleCreate();
  }

  if (!open) return null;

  return (
    <div className="capture-overlay" role="presentation" onMouseDown={onClose}>
      <section
        className="quick-capture"
        role="dialog"
        aria-modal="true"
        aria-labelledby="capture-title"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="capture-header">
          <div>
            <p>快速记录</p>
            <h2 id="capture-title">留下一条记忆</h2>
          </div>
          <button className="icon-button" onClick={onClose} aria-label="关闭快速记录" disabled={busy}>
            <X size={16} />
          </button>
        </div>
        <textarea
          ref={inputRef}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="记录想法、上下文或待办…"
          aria-label="记忆内容"
        />
        <div className="capture-footer">
          <span aria-live="polite">{notice || "支持 Markdown · ⌘↵ 写入"}</span>
          <button onClick={() => void handleCreate()} disabled={busy || !draft.trim()}>
            {busy ? "写入中…" : "写入记忆"}
          </button>
        </div>
      </section>
    </div>
  );
}
