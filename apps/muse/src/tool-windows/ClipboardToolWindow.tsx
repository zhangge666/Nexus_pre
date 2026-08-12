/** 本文件实现可被快捷键唤起的剪贴板逐项推进窗口，并保留次级比较能力。 */

import React, { useMemo, useState } from "react";
import { Check, ClipboardCopy, Columns2, Copy, Pin, PinOff, Play, RotateCcw } from "lucide-react";
import { splitClipboardContent } from "../core/clipboard";
import { formatCompactTime } from "../core/format";
import type { ClipboardSplitMode, MuseClipboardItem } from "../core/types";
import { useMuseWorkspace } from "../core/workspace";
import { useToolWindowLifecycle } from "./lifecycle";
import { ToolWindowFrame } from "./ToolWindowFrame";

/** 渲染保持置顶的剪贴板推进窗口。 */
export function ClipboardToolWindow(): React.JSX.Element {
  const { workspace, addClipboardItem, toggleClipboardPin, updateClipboardProgress } = useMuseWorkspace();
  const [activeId, setActiveId] = useState(workspace.clipboard[0]?.id ?? "");
  const [view, setView] = useState<"progress" | "compare">("progress");
  const [compareId, setCompareId] = useState(workspace.clipboard[1]?.id ?? "");
  const [feedback, setFeedback] = useState("点击当前项即可完成并继续");
  useToolWindowLifecycle();

  const active = workspace.clipboard.find((item) => item.id === activeId) ?? workspace.clipboard[0];
  const compareItem = workspace.clipboard.find((item) => item.id === compareId);
  const mode = active?.splitMode ?? "smart";
  const units = useMemo(() => splitClipboardContent(active?.content ?? "", mode), [active?.content, mode]);
  const completed = active?.completedIndexes ?? [];
  const activeIndex = Math.min(active?.activeIndex ?? 0, Math.max(0, units.length - 1));

  /** 读取系统剪贴板并将新内容作为当前推进清单。 */
  async function captureClipboard(): Promise<void> {
    try {
      const item = addClipboardItem(await navigator.clipboard.readText());
      if (!item) return setFeedback("当前内容为空或已经是最新一项");
      setActiveId(item.id);
      setFeedback("已读取，可选择拆分方式后开始推进");
    } catch {
      setFeedback("未获得剪贴板读取权限，可先在目标应用中复制");
    }
  }

  /** 动态切换完成状态并定位下一项未完成内容。 */
  function toggleUnit(index: number): void {
    if (!active) return;
    const nextCompleted = completed.includes(index) ? completed.filter((item) => item !== index) : [...completed, index];
    const nextIndex = units.findIndex((_, unitIndex) => unitIndex > index && !nextCompleted.includes(unitIndex));
    updateClipboardProgress(active.id, { completedIndexes: nextCompleted, activeIndex: nextIndex >= 0 ? nextIndex : index });
  }

  /** 改变拆分模式并重置与旧结构不再对应的索引。 */
  function changeMode(splitMode: ClipboardSplitMode): void {
    if (active) updateClipboardProgress(active.id, { splitMode, completedIndexes: [], activeIndex: 0 });
  }

  /** 把指定条目重新写回系统剪贴板。 */
  async function copyItem(item: MuseClipboardItem): Promise<void> {
    try { await navigator.clipboard.writeText(item.content); setFeedback(`已复制「${item.title}」`); }
    catch { setFeedback("当前环境无法写入系统剪贴板"); }
  }

  return (
    <ToolWindowFrame title="剪贴板推进" subtitle="逐项完成，减少记忆负担" shortcut="Ctrl Shift V" icon={<ClipboardCopy size={14} />}>
      <div className="clipboard-tool progress-tool">
        <aside className="clipboard-history">
          <div className="clipboard-toolbar"><span>{workspace.clipboard.length} 项 · 仅本机</span><button className="tool-secondary-button" type="button" onClick={() => void captureClipboard()}><ClipboardCopy size={13} />读取当前</button></div>
          <div className="clipboard-list">
            {workspace.clipboard.map((item) => (
              <article className={active?.id === item.id ? "is-selected" : ""} key={item.id}>
                <button className="clip-select" type="button" onClick={() => setActiveId(item.id)}><span className="clip-check">{active?.id === item.id && <Check size={10} />}</span><span><strong>{item.title}</strong><small>{item.source} · {formatCompactTime(item.copiedAt)}</small></span></button>
                <div className="clip-row-actions"><button type="button" onClick={() => toggleClipboardPin(item.id)} aria-label={item.pinned ? "取消固定" : "固定"}>{item.pinned ? <PinOff size={12} /> : <Pin size={12} />}</button><button type="button" onClick={() => void copyItem(item)} aria-label="复制此项"><Copy size={12} /></button></div>
              </article>
            ))}
          </div>
        </aside>
        <section className="tool-progress-panel">
          <header className="clip-mode-toolbar"><div className="segmented-control"><button className={view === "progress" ? "is-active" : ""} type="button" onClick={() => setView("progress")}><Play size={12} />推进</button><button className={view === "compare" ? "is-active" : ""} type="button" onClick={() => setView("compare")}><Columns2 size={12} />对比</button></div>{view === "progress" ? <div className="split-modes">{(["smart", "lines", "spaces"] as const).map((value) => <button className={mode === value ? "is-active" : ""} key={value} type="button" onClick={() => changeMode(value)}>{value === "smart" ? "智能" : value === "lines" ? "换行" : "空格"}</button>)}</div> : null}</header>
          {view === "progress" ? <div className="tool-progress-body"><div className="progress-summary"><div><span>{completed.length} / {units.length}</span><small>已完成</small></div><strong>{units.length ? Math.round(completed.length / units.length * 100) : 0}%</strong></div><div className="progress-track"><span style={{ width: `${units.length ? completed.length / units.length * 100 : 0}%` }} /></div>{units[activeIndex] ? <button className="current-clip-unit" type="button" onClick={() => toggleUnit(activeIndex)}><span>当前</span><strong>{units[activeIndex]}</strong><em>完成并继续 <Check size={13} /></em></button> : null}<div className="clip-unit-list">{units.map((unit, index) => <button className={`${completed.includes(index) ? "is-done" : ""} ${activeIndex === index ? "is-current" : ""}`} key={`${unit}-${index}`} type="button" onClick={() => toggleUnit(index)}><span className="unit-check">{completed.includes(index) ? <Check size={11} /> : index + 1}</span><strong>{unit}</strong><small>{completed.includes(index) ? "已完成" : activeIndex === index ? "当前项" : "未完成"}</small></button>)}</div><footer><span role="status">{feedback}</span><button type="button" onClick={() => active && updateClipboardProgress(active.id, { completedIndexes: [], activeIndex: 0 })}><RotateCcw size={12} />重置</button></footer></div> : <div className="tool-compare-secondary"><div className="compare-selector"><span>与</span><select value={compareId} onChange={(event) => setCompareId(event.target.value)}>{workspace.clipboard.filter((item) => item.id !== active?.id).map((item) => <option key={item.id} value={item.id}>{item.title}</option>)}</select></div><div className="tool-compare-columns"><pre>{active?.content}</pre><pre>{compareItem?.content}</pre></div><footer><span>{feedback}</span><span>对比是次级辅助视图</span></footer></div>}
        </section>
      </div>
    </ToolWindowFrame>
  );
}
