/** 本文件实现 Muse 无原生装饰窗口使用的自定义标题栏与窗口控制。 */

import React from "react";
import { Minus, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauriRuntime } from "../api";
import museIcon from "../assets/muse-app-icon.svg";

/** 执行 Tauri 窗口动作；浏览器预览中保持静默。 */
async function runWindowAction(action: "minimize" | "maximize" | "close"): Promise<void> {
  if (!isTauriRuntime()) return;
  const window = getCurrentWindow();
  if (action === "minimize") await window.minimize();
  if (action === "maximize") await window.toggleMaximize();
  if (action === "close") await window.close();
}

/** 渲染可拖动标题栏，并使用正式 Muse 图标。 */
export function Titlebar(): React.JSX.Element {
  return (
    <header className="app-titlebar" data-tauri-drag-region onDoubleClick={() => void runWindowAction("maximize")}>
      <div className="titlebar-brand" data-tauri-drag-region>
        <img src={museIcon} alt="" />
        <strong data-tauri-drag-region>Muse</strong>
        <span data-tauri-drag-region>随叫随到的工作助手</span>
      </div>
      <div className="titlebar-status" data-tauri-drag-region>
        <span className="status-dot" />
        本地模式
      </div>
      <div className="window-controls" onDoubleClick={(event) => event.stopPropagation()}>
        <button type="button" onClick={() => void runWindowAction("minimize")} aria-label="最小化">
          <Minus size={14} aria-hidden="true" />
        </button>
        <button type="button" onClick={() => void runWindowAction("maximize")} aria-label="最大化或还原">
          <Square size={11} aria-hidden="true" />
        </button>
        <button className="close-control" type="button" onClick={() => void runWindowAction("close")} aria-label="关闭">
          <X size={14} aria-hidden="true" />
        </button>
      </div>
    </header>
  );
}
