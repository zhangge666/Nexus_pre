/** 本文件实现以逐项推进为主、双栏比较为辅的本地剪贴板工作区。 */

import React, { useMemo, useState } from "react";
import { Check, ClipboardCopy, Columns2, Pin, PinOff, Play, RotateCcw, ShieldCheck, Trash2 } from "lucide-react";
import { PageHeader } from "../components/PageHeader";
import { splitClipboardContent } from "../core/clipboard";
import { formatCompactTime } from "../core/format";
import type { ClipboardSplitMode, MuseClipboardItem } from "../core/types";

interface ClipboardPageProps {
  items: MuseClipboardItem[];
  onTogglePin: (id: string) => void;
  onClearUnpinned: () => void;
  onUpdateProgress: (id: string, progress: { splitMode?: ClipboardSplitMode; completedIndexes?: number[]; activeIndex?: number }) => void;
}

/** 生成保留顺序的逐行比较数据。 */
function compareLines(left: string, right: string): Array<{ left: string; right: string; changed: boolean }> {
  const leftLines = left.split(/\r?\n/);
  const rightLines = right.split(/\r?\n/);
  return Array.from({ length: Math.max(leftLines.length, rightLines.length) }, (_, index) => ({
    left: leftLines[index] ?? "",
    right: rightLines[index] ?? "",
    changed: (leftLines[index] ?? "") !== (rightLines[index] ?? ""),
  }));
}

/** 渲染剪贴板历史、动态完成进度和次级比较视图。 */
export function ClipboardPage({ items, onTogglePin, onClearUnpinned, onUpdateProgress }: ClipboardPageProps): React.JSX.Element {
  const [activeId, setActiveId] = useState(items[0]?.id ?? "");
  const [view, setView] = useState<"progress" | "compare">("progress");
  const [compareId, setCompareId] = useState(items[1]?.id ?? "");
  const active = items.find((item) => item.id === activeId) ?? items[0];
  const compareItem = items.find((item) => item.id === compareId);
  const mode = active?.splitMode ?? "smart";
  const units = useMemo(() => splitClipboardContent(active?.content ?? "", mode), [active?.content, mode]);
  const completed = active?.completedIndexes ?? [];
  const activeIndex = Math.min(active?.activeIndex ?? 0, Math.max(0, units.length - 1));
  const comparison = useMemo(() => compareLines(active?.content ?? "", compareItem?.content ?? ""), [active, compareItem]);
  const progress = units.length ? Math.round((completed.length / units.length) * 100) : 0;

  /** 切换单项完成状态，并把当前指针移动到下一个未完成项。 */
  function toggleUnit(index: number): void {
    if (!active) return;
    const nextCompleted = completed.includes(index) ? completed.filter((item) => item !== index) : [...completed, index];
    const nextActive = units.findIndex((_, unitIndex) => unitIndex > index && !nextCompleted.includes(unitIndex));
    onUpdateProgress(active.id, { completedIndexes: nextCompleted, activeIndex: nextActive >= 0 ? nextActive : index });
  }

  /** 更新拆分方式时清空索引进度，避免旧索引错误映射到新条目。 */
  function setSplitMode(splitMode: ClipboardSplitMode): void {
    if (!active) return;
    onUpdateProgress(active.id, { splitMode, completedIndexes: [], activeIndex: 0 });
  }

  return (
    <div className="page page-clipboard">
      <PageHeader
        eyebrow="剪贴板"
        title="把复制内容变成可推进的清单"
        description="按换行或空格拆分，随工作动态标记完成；需要时再切换到双栏比较。"
        actions={<span className="local-chip"><ShieldCheck size={12} /> 仅本机</span>}
      />

      <section className="clipboard-workspace progress-first">
        <aside className="clipboard-list">
          <header><span>最近复制</span><button type="button" onClick={onClearUnpinned}><Trash2 size={12} /> 清理</button></header>
          {items.map((item, index) => (
            <div className={`clip-list-row ${active?.id === item.id ? "is-selected" : ""}`} key={item.id}>
              <button className="clip-select" type="button" onClick={() => setActiveId(item.id)}>
                <span className="clip-letter">{index + 1}</span>
                <span><strong>{item.title}</strong><small>{item.source} · {formatCompactTime(item.copiedAt)}</small></span>
              </button>
              <button className="pin-button" type="button" onClick={() => onTogglePin(item.id)} aria-label={item.pinned ? "取消固定" : "固定条目"}>
                {item.pinned ? <Pin size={12} /> : <PinOff size={12} />}
              </button>
            </div>
          ))}
        </aside>

        <section className="clip-progress-area">
          <header className="clip-mode-toolbar">
            <div className="segmented-control">
              <button className={view === "progress" ? "is-active" : ""} type="button" onClick={() => setView("progress")}><Play size={12} />推进</button>
              <button className={view === "compare" ? "is-active" : ""} type="button" onClick={() => setView("compare")}><Columns2 size={12} />对比</button>
            </div>
            {view === "progress" ? (
              <div className="split-modes" aria-label="拆分方式">
                {(["smart", "lines", "spaces"] as const).map((value) => (
                  <button className={mode === value ? "is-active" : ""} key={value} type="button" onClick={() => setSplitMode(value)}>
                    {value === "smart" ? "智能" : value === "lines" ? "换行" : "空格"}
                  </button>
                ))}
              </div>
            ) : null}
          </header>

          {view === "progress" ? (
            <div className="progress-view">
              <div className="progress-summary">
                <div><span>{completed.length} / {units.length}</span><small>已完成</small></div>
                <strong>{progress}%</strong>
              </div>
              <div className="progress-track"><span style={{ width: `${progress}%` }} /></div>
              {units[activeIndex] ? (
                <button className="current-clip-unit" type="button" onClick={() => toggleUnit(activeIndex)}>
                  <span>当前</span><strong>{units[activeIndex]}</strong><em>完成并继续 <Check size={13} /></em>
                </button>
              ) : null}
              <div className="clip-unit-list">
                {units.map((unit, index) => (
                  <button className={`${completed.includes(index) ? "is-done" : ""} ${activeIndex === index ? "is-current" : ""}`} key={`${unit}-${index}`} type="button" onClick={() => toggleUnit(index)}>
                    <span className="unit-check">{completed.includes(index) ? <Check size={11} /> : index + 1}</span>
                    <strong>{unit}</strong>
                    <small>{completed.includes(index) ? "已完成" : activeIndex === index ? "当前项" : "未完成"}</small>
                  </button>
                ))}
              </div>
              <footer><button type="button" onClick={() => active && onUpdateProgress(active.id, { completedIndexes: [], activeIndex: 0 })}><RotateCcw size={12} />重置进度</button><span>点击任意条目可动态切换完成状态</span></footer>
            </div>
          ) : (
            <div className="secondary-compare-view">
              <div className="compare-selector">
                <span>与</span>
                <select value={compareId} onChange={(event) => setCompareId(event.target.value)} aria-label="选择对比条目">
                  {items.filter((item) => item.id !== active?.id).map((item) => <option key={item.id} value={item.id}>{item.title}</option>)}
                </select>
                <small>{comparison.filter((row) => row.changed).length} 处不同</small>
              </div>
              <div className="compare-columns compact">
                {[active, compareItem].map((item, side) => (
                  <article key={item?.id ?? side}><header><strong>{item?.title ?? "未选择"}</strong></header><ol>{comparison.map((line, index) => <li className={line.changed ? `is-changed side-${side}` : ""} key={`${side}-${index}`}><span>{side === 0 ? line.left || " " : line.right || " "}</span></li>)}</ol></article>
                ))}
              </div>
            </div>
          )}
        </section>
      </section>
    </div>
  );
}
