/** 本文件单独负责 Muse 主窗口的拖拽区域、原生窗口按钮与窗口级保存状态。 */

import React from "react";
import { Minus, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauriRuntime } from "../api";
import museIcon from "../assets/muse-app-icon.svg";
import { isMacOS } from "../core/platform";

type WindowAction = "minimize" | "maximize" | "close";

/** 在桌面壳中执行窗口动作；浏览器预览保持无副作用。 */
async function runWindowAction(action: WindowAction): Promise<void> {
  if (!isTauriRuntime()) return;
  const appWindow = getCurrentWindow();
  if (action === "minimize") await appWindow.minimize();
  if (action === "maximize") await appWindow.toggleMaximize();
  if (action === "close") await appWindow.close();
}

/** 渲染 Windows 约定的右侧窗口控制。 */
function WindowsControls(): React.JSX.Element {
  return (
    <div className="window-controls window-controls--windows" onDoubleClick={(event) => event.stopPropagation()}>
      <button type="button" onClick={() => void runWindowAction("minimize")} aria-label="最小化">
        <Minus size={13} aria-hidden="true" />
      </button>
      <button type="button" onClick={() => void runWindowAction("maximize")} aria-label="最大化或还原">
        <Square size={10} aria-hidden="true" />
      </button>
      <button className="window-control-close" type="button" onClick={() => void runWindowAction("close")} aria-label="关闭">
        <X size={13} aria-hidden="true" />
      </button>
    </div>
  );
}

/** 渲染 macOS 约定的左侧交通灯窗口控制。 */
function MacControls(): React.JSX.Element {
  return (
    <div className="window-controls window-controls--mac" onDoubleClick={(event) => event.stopPropagation()}>
      <button className="mac-control mac-control--close" type="button" onClick={() => void runWindowAction("close")} aria-label="关闭" />
      <button className="mac-control mac-control--minimize" type="button" onClick={() => void runWindowAction("minimize")} aria-label="最小化" />
      <button className="mac-control mac-control--maximize" type="button" onClick={() => void runWindowAction("maximize")} aria-label="全屏或还原" />
    </div>
  );
}

/** 渲染与业务页面解耦的窗口级标题栏。 */
export function MuseTitlebar(): React.JSX.Element {
  const macOS = isMacOS();

  return (
    <header
      className={`muse-titlebar ${macOS ? "is-macos" : "is-windows"}`}
      data-tauri-drag-region
      onDoubleClick={() => void runWindowAction("maximize")}
    >
      {macOS ? <MacControls /> : null}
      <div className="titlebar-brand" data-tauri-drag-region>
        <img src={museIcon} alt="" />
        <span data-tauri-drag-region>Muse</span>
      </div>
      <div className="titlebar-drag-space" data-tauri-drag-region />
      <div className="titlebar-save-state" data-tauri-drag-region>
        <span className="status-dot status-dot--success" />
        <span data-tauri-drag-region>已保存到本机</span>
      </div>
      {macOS ? null : <WindowsControls />}
    </header>
  );
}
