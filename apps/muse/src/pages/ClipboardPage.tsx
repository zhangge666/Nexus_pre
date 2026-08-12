/** 本文件实现 Linear 风格的小巧剪贴板推进主页面。 */

import React, { useState } from "react";
import { ArrowUpDown, ClipboardCopy, ClipboardPaste } from "lucide-react";
import { ClipboardProgressPanel, type ClipboardPanelView } from "../components/ClipboardProgressPanel";
import type { ClipboardSplitMode, MuseClipboardItem } from "../core/types";

interface ClipboardPageProps {
  items: MuseClipboardItem[];
  onAddClipboardItem: (content: string, source?: string) => MuseClipboardItem | null;
  onUpdateProgress: (id: string, progress: { splitMode?: ClipboardSplitMode; completedIndexes?: number[]; activeIndex?: number }) => void;
}

/** 渲染不带历史侧栏和多余模式工具条的紧凑推进页面。 */
export function ClipboardPage({ items, onAddClipboardItem, onUpdateProgress }: ClipboardPageProps): React.JSX.Element {
  const [activeId, setActiveId] = useState(items[0]?.id ?? "");
  const [compareId, setCompareId] = useState(items[1]?.id ?? "");
  const [view, setView] = useState<ClipboardPanelView>("progress");
  const [feedback, setFeedback] = useState("");
  const active = items.find((item) => item.id === activeId) ?? items[0];

  /** 主动读取当前系统剪贴板，并把新内容直接设为当前清单。 */
  async function captureClipboard(): Promise<void> {
    try {
      const item = onAddClipboardItem(await navigator.clipboard.readText());
      if (!item) return setFeedback("当前内容为空或已是最新一项");
      setActiveId(item.id);
      setView("progress");
      setFeedback("已读取当前剪贴板");
    } catch {
      setFeedback("无法读取剪贴板，请先复制文字后重试");
    }
  }

  return (
    <div className="page page-clipboard clipboard-focus-page">
      <header className="clipboard-focus-toolbar">
        <div className="clipboard-focus-title"><ClipboardCopy size={15} /><strong>剪贴板推进</strong></div>
        <label className="clipboard-source-select">
          <select value={active?.id ?? ""} onChange={(event) => { setActiveId(event.target.value); setView("progress"); }} aria-label="选择剪贴板清单">
            {items.map((item) => <option key={item.id} value={item.id}>{item.title}</option>)}
          </select>
        </label>
        <div className="clipboard-focus-actions">
          <button type="button" onClick={() => void captureClipboard()} aria-label="读取当前剪贴板" title="读取当前剪贴板"><ClipboardPaste size={14} /></button>
          <button className={view === "compare" ? "is-active" : ""} type="button" onClick={() => setView((current) => current === "compare" ? "progress" : "compare")} aria-label="切换对比视图" title="切换对比视图"><ArrowUpDown size={14} /></button>
        </div>
      </header>
      <ClipboardProgressPanel active={active} items={items} view={view} compareId={compareId} feedback={feedback} onCompareIdChange={setCompareId} onUpdateProgress={onUpdateProgress} />
    </div>
  );
}
