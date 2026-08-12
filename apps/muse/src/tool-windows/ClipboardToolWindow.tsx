/** 本文件实现 Linear 风格的小巧剪贴板推进快捷窗口。 */

import React, { useState } from "react";
import { ArrowUpDown, ClipboardPaste } from "lucide-react";
import { ClipboardProgressPanel, type ClipboardPanelView } from "../components/ClipboardProgressPanel";
import { useMuseWorkspace } from "../core/workspace";
import { useToolWindowLifecycle } from "./lifecycle";
import { ToolWindowFrame } from "./ToolWindowFrame";

/** 渲染保持置顶、只突出当前推进动作的剪贴板工具窗。 */
export function ClipboardToolWindow(): React.JSX.Element {
  const { workspace, addClipboardItem, updateClipboardProgress } = useMuseWorkspace();
  const [activeId, setActiveId] = useState(workspace.clipboard[0]?.id ?? "");
  const [compareId, setCompareId] = useState(workspace.clipboard[1]?.id ?? "");
  const [view, setView] = useState<ClipboardPanelView>("progress");
  const [feedback, setFeedback] = useState("");
  const active = workspace.clipboard.find((item) => item.id === activeId) ?? workspace.clipboard[0];
  useToolWindowLifecycle();

  /** 读取当前系统剪贴板，并直接进入新内容的推进状态。 */
  async function captureClipboard(): Promise<void> {
    try {
      const item = addClipboardItem(await navigator.clipboard.readText());
      if (!item) return setFeedback("当前内容为空或已是最新一项");
      setActiveId(item.id);
      setView("progress");
      setFeedback("已读取当前剪贴板");
    } catch {
      setFeedback("无法读取剪贴板，请先复制文字后重试");
    }
  }

  return (
    <ToolWindowFrame
      title="剪贴板推进"
      subtitle=""
      shortcut="Ctrl Shift V"
      icon={null}
      showShortcut={false}
      headerAccessory={(
        <label className="clipboard-source-select is-tool-window">
          <select value={active?.id ?? ""} onChange={(event) => { setActiveId(event.target.value); setView("progress"); }} aria-label="选择剪贴板清单">
            {workspace.clipboard.map((item) => <option key={item.id} value={item.id}>{item.title}</option>)}
          </select>
        </label>
      )}
      headerActions={(
        <>
          <button type="button" onClick={() => void captureClipboard()} aria-label="读取当前剪贴板" title="读取当前剪贴板"><ClipboardPaste size={14} /></button>
          <button className={view === "compare" ? "is-active" : ""} type="button" onClick={() => setView((current) => current === "compare" ? "progress" : "compare")} aria-label="切换对比视图" title="切换对比视图"><ArrowUpDown size={14} /></button>
        </>
      )}
    >
      <ClipboardProgressPanel active={active} items={workspace.clipboard} view={view} compareId={compareId} feedback={feedback} onCompareIdChange={setCompareId} onUpdateProgress={updateClipboardProgress} />
    </ToolWindowFrame>
  );
}
