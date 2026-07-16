/** 本文件实现可拖拽的桌面标题栏与 Tauri 窗口控制按钮。 */
import type React from "react";
import { useEffect, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

/** 判断当前页面是否运行于可调用原生窗口 API 的 Tauri WebView。 */
function isTauriWindow(): boolean {
  return isTauri();
}

/** 渲染窗口拖拽区域以及最小化、最大化和关闭控制。 */
export function Titlebar(): React.JSX.Element {
  const [isMac, setIsMac] = useState(false);
  const [isMaximized, setIsMaximized] = useState(false);
  const canControlWindow = isTauriWindow();

  useEffect(() => {
    setIsMac(navigator.userAgent.includes("Mac"));
    if (!canControlWindow) return;

    const appWindow = getCurrentWindow();
    void appWindow.isMaximized().then(setIsMaximized).catch(() => setIsMaximized(false));
    const unlisten = appWindow.onResized(() => {
      void appWindow.isMaximized().then(setIsMaximized).catch(() => setIsMaximized(false));
    });
    return () => { void unlisten.then((remove) => remove()); };
  }, [canControlWindow]);

  /** 最小化当前原生窗口；浏览器预览中保持禁用而非抛出 IPC 错误。 */
  function handleMinimize(): void {
    if (canControlWindow) void getCurrentWindow().minimize();
  }

  /** 切换最大化状态，并在原生 API 返回后同步对应图标。 */
  async function handleMaximize(): Promise<void> {
    if (!canControlWindow) return;
    const appWindow = getCurrentWindow();
    await appWindow.toggleMaximize();
    setIsMaximized(await appWindow.isMaximized());
  }

  /** 关闭当前原生窗口；在浏览器预览中不执行页面级关闭。 */
  function handleClose(): void {
    if (canControlWindow) void getCurrentWindow().close();
  }

  /** 在可拖拽区域按下鼠标时显式调用原生拖拽，避免仅依赖 WebView 属性识别差异。 */
  function handleDragStart(event: React.MouseEvent<HTMLElement>): void {
    if (!canControlWindow || event.button !== 0) return;
    void getCurrentWindow().startDragging();
  }

  const controls = isMac ? (
    <div className="titlebar-controls mac" aria-label="窗口控制">
      <button className="control-btn close" onMouseDown={(event) => event.stopPropagation()} onClick={handleClose} title="关闭" aria-label="关闭" disabled={!canControlWindow} />
      <button className="control-btn minimize" onMouseDown={(event) => event.stopPropagation()} onClick={handleMinimize} title="最小化" aria-label="最小化" disabled={!canControlWindow} />
      <button className="control-btn maximize" onMouseDown={(event) => event.stopPropagation()} onClick={() => void handleMaximize()} title="最大化" aria-label="最大化" disabled={!canControlWindow} />
    </div>
  ) : (
    <div className="titlebar-controls win" aria-label="窗口控制">
      <button className="control-btn minimize" onMouseDown={(event) => event.stopPropagation()} onClick={handleMinimize} title="最小化" aria-label="最小化" disabled={!canControlWindow}>
        <svg width="10" height="1" viewBox="0 0 10 1" aria-hidden="true"><line x1="0" y1="0.5" x2="10" y2="0.5" /></svg>
      </button>
      <button className="control-btn maximize" onMouseDown={(event) => event.stopPropagation()} onClick={() => void handleMaximize()} title={isMaximized ? "还原" : "最大化"} aria-label={isMaximized ? "还原" : "最大化"} disabled={!canControlWindow}>
        {isMaximized ? (
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><rect x="1.5" y="3.5" width="5" height="5" fill="none" /><rect x="3.5" y="1.5" width="5" height="5" fill="none" /></svg>
        ) : (
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><rect x="1.5" y="1.5" width="7" height="7" fill="none" /></svg>
        )}
      </button>
      <button className="control-btn close" onMouseDown={(event) => event.stopPropagation()} onClick={handleClose} title="关闭" aria-label="关闭" disabled={!canControlWindow}>
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M1.5 1.5 L8.5 8.5 M8.5 1.5 L1.5 8.5" fill="none" /></svg>
      </button>
    </div>
  );

  return (
    <header className={`window-titlebar${isMac ? " is-mac" : ""}`} aria-label="Orbit 窗口标题栏">
      {isMac && controls}
      <div className="window-drag-region" data-tauri-drag-region onMouseDown={handleDragStart}>
        <span data-tauri-drag-region>Orbit</span>
      </div>
      {!isMac && controls}
    </header>
  );
}
