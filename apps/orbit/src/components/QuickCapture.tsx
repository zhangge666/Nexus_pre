/** 本文件实现快速记录组件。 */
import type React from "react";
import { useState } from "react";
import { Plus } from "lucide-react";
import { createMemory } from "../core";

interface QuickCaptureProps {
  onCreated?: () => void;
}

export function QuickCapture({ onCreated }: QuickCaptureProps): React.JSX.Element {
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("");

  async function handleCreate(): Promise<void> {
    if (!draft.trim()) return;
    setBusy(true);
    try {
      await createMemory(draft.trim());
      setDraft("");
      setNotice("记忆已写入");
      onCreated?.();
      setTimeout(() => setNotice(""), 2000);
    } catch (e) {
      setNotice(`写入失败：${String(e)}`);
    } finally {
      setBusy(false);
    }
  }

  function handleKeyDown(e: React.KeyboardEvent): void {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) void handleCreate();
  }

  return (
    <section className="quick-capture" aria-labelledby="capture-title">
      <div className="section-heading">
        <h2 id="capture-title">快速记录</h2>
        <Plus size={15} aria-hidden="true" />
      </div>
      <textarea
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="记录此刻值得保留的内容…&#10;⌘↵ 快速写入"
        aria-label="记忆内容"
      />
      <div className="capture-footer">
        <span>{notice || "Markdown"}</span>
        <button
          onClick={() => void handleCreate()}
          disabled={busy || !draft.trim()}
        >
          {busy ? "写入中…" : "写入记忆"}
        </button>
      </div>
    </section>
  );
}
