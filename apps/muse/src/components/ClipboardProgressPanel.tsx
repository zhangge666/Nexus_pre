/** 本文件提供主页面与快捷工具窗共用的紧凑剪贴板推进核心界面。 */

import React, { useMemo } from "react";
import { Check, RotateCcw } from "lucide-react";
import { splitClipboardContent } from "../core/clipboard";
import type { ClipboardSplitMode, MuseClipboardItem } from "../core/types";

export type ClipboardPanelView = "progress" | "compare";

interface ClipboardProgressPanelProps {
  active?: MuseClipboardItem;
  items: MuseClipboardItem[];
  view: ClipboardPanelView;
  compareId: string;
  feedback?: string;
  onCompareIdChange: (id: string) => void;
  onUpdateProgress: (
    id: string,
    progress: { splitMode?: ClipboardSplitMode; completedIndexes?: number[]; activeIndex?: number },
  ) => void;
}

/** 返回有效且不重复的完成索引，兼容旧版或已改变拆分结果的本地数据。 */
function normalizeCompleted(indexes: number[] | undefined, length: number): number[] {
  return [...new Set(indexes ?? [])].filter((index) => index >= 0 && index < length).sort((a, b) => a - b);
}

/** 选择当前应该推进的未完成项；全部完成时返回 -1。 */
function resolveCurrentIndex(preferred: number | undefined, completed: number[], length: number): number {
  if (length === 0 || completed.length >= length) return -1;
  const index = Math.min(Math.max(preferred ?? 0, 0), length - 1);
  if (!completed.includes(index)) return index;
  const after = Array.from({ length }, (_, itemIndex) => itemIndex).find((itemIndex) => itemIndex > index && !completed.includes(itemIndex));
  return after ?? Array.from({ length }, (_, itemIndex) => itemIndex).find((itemIndex) => !completed.includes(itemIndex)) ?? -1;
}

/** 渲染以“当前项 → 完成并继续”为唯一主动作的剪贴板推进面板。 */
export function ClipboardProgressPanel({
  active,
  items,
  view,
  compareId,
  feedback,
  onCompareIdChange,
  onUpdateProgress,
}: ClipboardProgressPanelProps): React.JSX.Element {
  const units = useMemo(
    () => splitClipboardContent(active?.content ?? "", active?.splitMode ?? "smart"),
    [active?.content, active?.splitMode],
  );
  const completed = normalizeCompleted(active?.completedIndexes, units.length);
  const currentIndex = resolveCurrentIndex(active?.activeIndex, completed, units.length);
  const progress = units.length ? Math.round((completed.length / units.length) * 100) : 0;
  const compareItem = items.find((item) => item.id === compareId) ?? items.find((item) => item.id !== active?.id);
  const compareUnits = useMemo(
    () => splitClipboardContent(compareItem?.content ?? "", compareItem?.splitMode ?? "smart"),
    [compareItem?.content, compareItem?.splitMode],
  );
  const comparison = useMemo(
    () => Array.from({ length: Math.max(units.length, compareUnits.length) }, (_, index) => ({
      left: units[index] ?? "",
      right: compareUnits[index] ?? "",
      changed: (units[index] ?? "") !== (compareUnits[index] ?? ""),
    })),
    [compareUnits, units],
  );

  /** 切换指定条目的完成状态，并把指针移动到下一个未完成项。 */
  function toggleUnit(index: number): void {
    if (!active) return;
    const nextCompleted = completed.includes(index)
      ? completed.filter((item) => item !== index)
      : [...completed, index].sort((a, b) => a - b);
    const nextIndex = resolveCurrentIndex(index, nextCompleted, units.length);
    onUpdateProgress(active.id, {
      completedIndexes: nextCompleted,
      activeIndex: nextIndex >= 0 ? nextIndex : index,
    });
  }

  if (!active) {
    return <div className="clipboard-focus-empty">复制一段文字后，Muse 会把它拆成可逐项完成的清单。</div>;
  }

  if (view === "compare") {
    return (
      <section className="clipboard-focus-compare">
        <header>
          <div><span>当前</span><strong>{active.title}</strong></div>
          <span>与</span>
          <label>
            <select value={compareItem?.id ?? ""} onChange={(event) => onCompareIdChange(event.target.value)} aria-label="选择对比清单">
              {items.filter((item) => item.id !== active.id).map((item) => <option key={item.id} value={item.id}>{item.title}</option>)}
            </select>
          </label>
          <small>{comparison.filter((row) => row.changed).length} 处不同</small>
        </header>
        <div className="clipboard-focus-compare-grid">
          <div className="compare-column-title"><strong>{active.title}</strong><strong>{compareItem?.title ?? "未选择"}</strong></div>
          {comparison.map((row, index) => (
            <div className={row.changed ? "is-changed" : ""} key={`${index}-${row.left}-${row.right}`}>
              <span>{row.left || "—"}</span><span>{row.right || "—"}</span>
            </div>
          ))}
        </div>
        <footer><span>对比仅用于辅助核对 · 内容仅保存在本机</span></footer>
      </section>
    );
  }

  return (
    <section className="clipboard-focus-progress">
      <header className="clipboard-progress-overview">
        <span><strong>{completed.length} / {units.length}</strong> 已完成</span>
        <small>{progress}%</small>
        <div aria-label={`完成进度 ${progress}%`}><span style={{ width: `${progress}%` }} /></div>
      </header>

      {currentIndex >= 0 ? (
        <div className="clipboard-current-row">
          <span>当前</span>
          <strong>{units[currentIndex]}</strong>
          <button type="button" onClick={() => toggleUnit(currentIndex)}>完成并继续</button>
        </div>
      ) : (
        <div className="clipboard-current-row is-complete">
          <span><Check size={13} />完成</span><strong>当前清单已经全部处理完毕</strong>
        </div>
      )}

      <div className="clipboard-unit-rows">
        {units.map((unit, index) => {
          const done = completed.includes(index);
          const current = index === currentIndex;
          return (
            <button className={`${done ? "is-done" : ""} ${current ? "is-current" : ""}`} key={`${unit}-${index}`} type="button" onClick={() => toggleUnit(index)}>
              <span className="clipboard-unit-state">{done ? <Check size={12} /> : current ? <i /> : null}</span>
              <strong>{unit}</strong>
              <small>{done ? "已完成" : current ? "进行中" : "待处理"}</small>
            </button>
          );
        })}
      </div>

      <footer className="clipboard-focus-footer">
        <span>{feedback || "智能识别换行与空格"} · 仅本机</span>
        <button type="button" onClick={() => onUpdateProgress(active.id, { completedIndexes: [], activeIndex: 0 })}><RotateCcw size={12} />重置</button>
      </footer>
    </section>
  );
}
