/** 本文件实现剪贴板历史读取、固定选择与双栏逐行比较工具窗。 */

import React, { useMemo, useState } from "react";
import { Check, ClipboardCopy, Copy, Pin, PinOff, Rows3 } from "lucide-react";
import { formatCompactTime } from "../core/format";
import type { MuseClipboardItem } from "../core/types";
import { useMuseWorkspace } from "../core/workspace";
import { useToolWindowLifecycle } from "./lifecycle";
import { ToolWindowFrame } from "./ToolWindowFrame";

/** 将两段文本转换为按行对齐的比较行。 */
function buildComparison(left: string, right: string): Array<{ left: string; right: string; equal: boolean }> {
  const leftLines = left.split(/\r?\n/);
  const rightLines = right.split(/\r?\n/);
  return Array.from({ length: Math.max(leftLines.length, rightLines.length) }, (_, index) => ({
    left: leftLines[index] ?? "",
    right: rightLines[index] ?? "",
    equal: leftLines[index] === rightLines[index],
  }));
}

/** 渲染保持置顶的剪贴板专用窗口，并支持最多两项对照。 */
export function ClipboardToolWindow(): React.JSX.Element {
  const { workspace, addClipboardItem, toggleClipboardPin } = useMuseWorkspace();
  const [selectedIds, setSelectedIds] = useState<string[]>(() => workspace.clipboard.slice(0, 2).map((item) => item.id));
  const [feedback, setFeedback] = useState("选择两项即可逐行比较");

  useToolWindowLifecycle();

  const selectedItems = useMemo(
    () =>
      selectedIds
        .map((id) => workspace.clipboard.find((item) => item.id === id))
        .filter((item): item is MuseClipboardItem => Boolean(item)),
    [selectedIds, workspace.clipboard],
  );
  const comparison = useMemo(
    () => buildComparison(selectedItems[0]?.content ?? "", selectedItems[1]?.content ?? ""),
    [selectedItems],
  );

  /** 读取当前系统剪贴板并追加到本机历史。 */
  async function captureClipboard(): Promise<void> {
    try {
      const value = await navigator.clipboard.readText();
      const item = addClipboardItem(value);
      if (!item) {
        setFeedback(value.trim() ? "该内容已经是最新一项" : "当前剪贴板没有文字");
        return;
      }
      setSelectedIds((current) => [item.id, ...current].slice(0, 2));
      setFeedback("已读取当前剪贴板");
    } catch {
      setFeedback("未获得剪贴板读取权限，可先在目标应用中复制");
    }
  }

  /** 维护最多两项的比较选择，新选择会替换最早选择。 */
  function toggleSelection(id: string): void {
    setSelectedIds((current) => {
      if (current.includes(id)) return current.filter((selectedId) => selectedId !== id);
      return [...current.slice(-1), id];
    });
  }

  /** 将指定条目重新写入系统剪贴板。 */
  async function copyItem(item: MuseClipboardItem): Promise<void> {
    try {
      await navigator.clipboard.writeText(item.content);
      setFeedback(`已复制「${item.title}」`);
    } catch {
      setFeedback("当前环境无法写入系统剪贴板");
    }
  }

  return (
    <ToolWindowFrame
      title="剪贴板比较"
      subtitle="集中保留，减少来回切换"
      shortcut="Ctrl Shift V"
      icon={<ClipboardCopy size={14} />}
    >
      <div className="clipboard-tool">
        <aside className="clipboard-history">
          <div className="clipboard-toolbar">
            <span>{workspace.clipboard.length} 项 · 仅本机</span>
            <button className="tool-secondary-button" type="button" onClick={() => void captureClipboard()}>
              <ClipboardCopy size={13} />
              读取当前
            </button>
          </div>
          <div className="clipboard-list" role="listbox" aria-label="剪贴板历史" aria-multiselectable="true">
            {workspace.clipboard.map((item) => {
              const selected = selectedIds.includes(item.id);
              return (
                <article className={selected ? "is-selected" : ""} key={item.id}>
                  <button
                    className="clip-select"
                    type="button"
                    onClick={() => toggleSelection(item.id)}
                    aria-pressed={selected}
                  >
                    <span className="clip-check">{selected && <Check size={10} />}</span>
                    <span>
                      <strong>{item.title}</strong>
                      <small>{item.source} · {formatCompactTime(item.copiedAt)}</small>
                    </span>
                  </button>
                  <div className="clip-row-actions">
                    <button type="button" onClick={() => toggleClipboardPin(item.id)} aria-label={item.pinned ? "取消固定" : "固定"}>
                      {item.pinned ? <PinOff size={12} /> : <Pin size={12} />}
                    </button>
                    <button type="button" onClick={() => void copyItem(item)} aria-label="复制此项">
                      <Copy size={12} />
                    </button>
                  </div>
                </article>
              );
            })}
          </div>
        </aside>
        <section className="clipboard-compare">
          <header>
            <div>
              <Rows3 size={14} />
              <strong>逐行比较</strong>
            </div>
            <span>{selectedItems.length === 2 ? `${comparison.filter((row) => !row.equal).length} 处不同` : "请选择两项"}</span>
          </header>
          <div className="compare-headings">
            <strong>{selectedItems[0]?.title ?? "左侧条目"}</strong>
            <strong>{selectedItems[1]?.title ?? "右侧条目"}</strong>
          </div>
          <div className="compare-grid">
            {selectedItems.length === 2 ? comparison.map((row, index) => (
              <React.Fragment key={`${index}-${row.left}-${row.right}`}>
                <pre className={row.equal ? "" : "is-removed"}>{row.left || " "}</pre>
                <pre className={row.equal ? "" : "is-added"}>{row.right || " "}</pre>
              </React.Fragment>
            )) : (
              <div className="compare-empty">从左侧选择两个条目，差异会在这里并排显示。</div>
            )}
          </div>
          <footer>
            <span role="status" aria-live="polite">{feedback}</span>
            <span>窗口保持置顶 · Esc 隐藏</span>
          </footer>
        </section>
      </div>
    </ToolWindowFrame>
  );
}
