/** 本文件提供四类 Muse 快捷工具窗共用的精简标题栏与内容骨架。 */

import React, { type ReactNode } from "react";
import { X } from "lucide-react";
import museIcon from "../assets/muse-app-icon.svg";
import { isMacOS } from "../core/platform";
import { hideToolWindow } from "./lifecycle";

interface ToolWindowFrameProps {
  title: string;
  subtitle: string;
  shortcut: string;
  icon: ReactNode;
  headerAccessory?: ReactNode;
  headerActions?: ReactNode;
  showShortcut?: boolean;
  tone?: "default" | "recording";
  children: ReactNode;
}

/** 渲染可拖动的无原生装饰工具窗，并将关闭动作转换为隐藏。 */
export function ToolWindowFrame({
  title,
  subtitle,
  shortcut,
  icon,
  headerAccessory,
  headerActions,
  showShortcut = true,
  tone = "default",
  children,
}: ToolWindowFrameProps): React.JSX.Element {
  const macOS = isMacOS();

  return (
    <div className={`tool-window tool-window--${tone}${headerAccessory ? " tool-window--with-accessory" : ""}`}>
      <header className="tool-titlebar" data-tauri-drag-region>
        {macOS ? (
          <button className="tool-close-control--macos" type="button" onClick={() => void hideToolWindow()} aria-label={`隐藏${title}窗口`} />
        ) : null}
        <img src={museIcon} alt="" data-tauri-drag-region />
        {icon ? <span className="tool-symbol" aria-hidden="true">{icon}</span> : null}
        <div className="tool-title-copy" data-tauri-drag-region>
          <strong data-tauri-drag-region>{title}</strong>
          {subtitle ? <span data-tauri-drag-region>{subtitle}</span> : null}
        </div>
        {headerAccessory ? <div className="tool-title-accessory">{headerAccessory}</div> : null}
        {headerActions ? <div className="tool-title-actions">{headerActions}</div> : null}
        {showShortcut ? <kbd data-tauri-drag-region>{shortcut}</kbd> : null}
        {macOS ? null : <button type="button" onClick={() => void hideToolWindow()} aria-label={`隐藏${title}窗口`}><X size={14} aria-hidden="true" /></button>}
      </header>
      <main className="tool-content">{children}</main>
    </div>
  );
}
