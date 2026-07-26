/** 本文件实现 Muse 本地剪贴板条目选择、固定与双栏差异比较。 */

import React, { useMemo, useState } from "react";
import { ClipboardCopy, Pause, Pin, PinOff, ShieldCheck, Trash2 } from "lucide-react";
import { PageHeader } from "../components/PageHeader";
import { formatCompactTime } from "../core/format";
import type { MuseClipboardItem } from "../core/types";

interface ClipboardPageProps {
  items: MuseClipboardItem[];
  onTogglePin: (id: string) => void;
  onClearUnpinned: () => void;
}

/** 把剪贴板文本按行拆分为可比较的数据。 */
function compareLines(left: string, right: string) {
  const leftLines = left.split("\n");
  const rightLines = right.split("\n");
  const length = Math.max(leftLines.length, rightLines.length);
  return Array.from({ length }, (_, index) => ({
    left: leftLines[index] ?? "",
    right: rightLines[index] ?? "",
    changed: (leftLines[index] ?? "") !== (rightLines[index] ?? ""),
  }));
}

/** 呈现本地剪贴板历史与两项比较视图。 */
export function ClipboardPage({ items, onTogglePin, onClearUnpinned }: ClipboardPageProps): React.JSX.Element {
  const [selectedIds, setSelectedIds] = useState<string[]>(items.slice(0, 2).map((item) => item.id));
  const left = items.find((item) => item.id === selectedIds[0]);
  const right = items.find((item) => item.id === selectedIds[1]);
  const lines = useMemo(() => compareLines(left?.content ?? "", right?.content ?? ""), [left, right]);

  /** 最多保留两个选择，并按选择先后映射左右栏。 */
  function toggleSelection(id: string): void {
    setSelectedIds((current) => {
      if (current.includes(id)) return current.filter((item) => item !== id);
      return [...current.slice(-1), id];
    });
  }

  return (
    <div className="page page-clipboard">
      <PageHeader
        eyebrow="剪贴板"
        title="固定并比较多段内容"
        description="历史只在本机临时保存，不会因为连接 Orbit 而自动上传。"
        actions={(
          <>
            <span className="local-chip"><ShieldCheck size={12} /> 未同步</span>
            <button className="secondary-button" type="button"><Pause size={12} /> 暂停监听</button>
          </>
        )}
      />

      <section className="clipboard-workspace">
        <aside className="clipboard-list">
          <header><span>最近复制</span><button type="button" onClick={onClearUnpinned}><Trash2 size={12} /> 清理未固定</button></header>
          {items.map((item, index) => {
            const selectionIndex = selectedIds.indexOf(item.id);
            return (
              <div className={`clip-list-row ${selectionIndex >= 0 ? `is-selected side-${selectionIndex}` : ""}`} key={item.id}>
                <button className="clip-select" type="button" onClick={() => toggleSelection(item.id)}>
                  <span className="clip-letter">{selectionIndex >= 0 ? String.fromCharCode(65 + selectionIndex) : index + 1}</span>
                  <span>
                    <strong>{item.title}</strong>
                    <small>{item.source} · {formatCompactTime(item.copiedAt)}</small>
                  </span>
                </button>
                <button
                  className="pin-button"
                  type="button"
                  onClick={() => onTogglePin(item.id)}
                  aria-label={item.pinned ? "取消固定" : "固定条目"}
                >
                  {item.pinned ? <Pin size={12} /> : <PinOff size={12} />}
                </button>
              </div>
            );
          })}
          {items.length === 0 ? <div className="empty-state">复制内容后会显示在这里。</div> : null}
        </aside>

        <section className="compare-area">
          <header className="compare-toolbar">
            <div><button className="is-active" type="button">逐行</button><button type="button">仅差异</button></div>
            <span>{lines.filter((line) => line.changed).length} 处差异</span>
            <button type="button">交换 A / B</button>
          </header>
          <div className="compare-columns">
            {[left, right].map((item, side) => (
              <article key={item?.id ?? side}>
                <header>
                  <span className={`clip-letter side-${side}`}>{String.fromCharCode(65 + side)}</span>
                  <strong>{item?.title ?? "选择一个条目"}</strong>
                  <button type="button">复制</button>
                </header>
                <ol>
                  {lines.map((line, index) => {
                    const value = side === 0 ? line.left : line.right;
                    return (
                      <li className={line.changed ? `is-changed side-${side}` : ""} key={`${side}-${index}`}>
                        <span>{value || " "}</span>
                      </li>
                    );
                  })}
                </ol>
              </article>
            ))}
          </div>
          <footer>
            <span><ClipboardCopy size={12} /> 条目只保存在本机，除非主动绑定任务。</span>
            <button className="secondary-button" type="button">绑定到任务</button>
            <button className="primary-button" type="button">保存比较结果</button>
          </footer>
        </section>
      </section>
    </div>
  );
}
