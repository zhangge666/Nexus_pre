/** 本文件实现 Orbit 根布局、可调整工作区侧栏与懒加载路由。 */

import type React from "react";
import { lazy, Suspense, useCallback, useRef, useState } from "react";
import { Routes, Route, Navigate } from "react-router-dom";
import { Sidebar } from "./components/Sidebar";
import { SidebarProvider, useSidebar } from "./components/SidebarState";
import { Titlebar } from "./components/Titlebar";
import { InspectorPanel, InspectorProvider, useInspector } from "./components/Inspector";

const TodayPage       = lazy(() => import("./pages/TodayPage"));
const SearchPage      = lazy(() => import("./pages/SearchPage"));
const TimelinePage    = lazy(() => import("./pages/TimelinePage"));
const InboxPage       = lazy(() => import("./pages/InboxPage"));
const ReviewPage      = lazy(() => import("./pages/ReviewPage"));
const CardsPage       = lazy(() => import("./pages/CardsPage"));
const AskPage         = lazy(() => import("./pages/AskPage"));
const GraphPage       = lazy(() => import("./pages/GraphPage"));
const ConnectionsPage = lazy(() => import("./pages/ConnectionsPage"));
const SettingsPage    = lazy(() => import("./pages/SettingsPage"));

const SIDEBAR_DEFAULT_WIDTH = 232;
const SIDEBAR_MIN_WIDTH = 120;
const SIDEBAR_COLLAPSE_THRESHOLD = 160;
const SIDEBAR_MAX_WIDTH = 360;
const INSPECTOR_DEFAULT_WIDTH = 340;
const INSPECTOR_MIN_WIDTH = 240;
const INSPECTOR_COLLAPSE_THRESHOLD = 280;
const INSPECTOR_MAX_WIDTH = 480;

/** 将拖拽得到的面板宽度限定在当前工作区的安全范围内。 */
function clampWidth(width: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(width, minimum), maximum);
}

interface ResizeHandleProps {
  side: "left" | "right";
  label: string;
  onResizeStart: () => void;
  onResize: (clientX: number) => void;
  onResizeEnd: () => void;
  onKeyboardResize: (key: "ArrowLeft" | "ArrowRight" | "Home") => void;
}

/** 渲染可拖拽且可键盘操作的工作区分隔条。 */
function ResizeHandle({ side, label, onResizeStart, onResize, onResizeEnd, onKeyboardResize }: ResizeHandleProps): React.JSX.Element {
  const draggingRef = useRef(false);
  const [dragging, setDragging] = useState(false);

  /** 捕获当前指针，确保拖拽离开细窄分隔条后仍能连续调整宽度。 */
  function handlePointerDown(event: React.PointerEvent<HTMLDivElement>): void {
    event.preventDefault();
    draggingRef.current = true;
    setDragging(true);
    event.currentTarget.setPointerCapture(event.pointerId);
    onResizeStart();
  }

  /** 根据指针横坐标实时更新对应侧栏的宽度。 */
  function handlePointerMove(event: React.PointerEvent<HTMLDivElement>): void {
    if (draggingRef.current) onResize(event.clientX);
  }

  /** 结束拖拽并让调用方根据阈值决定保留宽度或自动收起。 */
  function handlePointerEnd(event: React.PointerEvent<HTMLDivElement>): void {
    if (!draggingRef.current) return;
    draggingRef.current = false;
    setDragging(false);
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
    onResizeEnd();
  }

  /** 提供箭头键微调与 Home 键重置，避免宽度调整只能依赖鼠标。 */
  function handleKeyDown(event: React.KeyboardEvent<HTMLDivElement>): void {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight" && event.key !== "Home") return;
    event.preventDefault();
    onKeyboardResize(event.key);
  }

  return <div className={`resize-handle ${side}${dragging ? " is-dragging" : ""}`} role="separator" aria-label={label} aria-orientation="vertical" tabIndex={0} onPointerDown={handlePointerDown} onPointerMove={handlePointerMove} onPointerUp={handlePointerEnd} onPointerCancel={handlePointerEnd} onKeyDown={handleKeyDown} />;
}

/** 将侧栏状态、检查器状态与可调宽度组合为统一的桌面工作区。 */
function WorkspaceShell(): React.JSX.Element {
  const { collapsed, toggle: toggleSidebar } = useSidebar();
  const { open: inspectorOpen, close: closeInspector } = useInspector();
  const shellRef = useRef<HTMLDivElement>(null);
  const sidebarDragWidthRef = useRef(SIDEBAR_DEFAULT_WIDTH);
  const inspectorDragWidthRef = useRef(INSPECTOR_DEFAULT_WIDTH);
  const [sidebarWidth, setSidebarWidth] = useState(SIDEBAR_DEFAULT_WIDTH);
  const [inspectorWidth, setInspectorWidth] = useState(INSPECTOR_DEFAULT_WIDTH);
  const [resizing, setResizing] = useState(false);

  /** 基于工作区左边界更新左侧栏宽度，并记录是否已越过自动收起阈值。 */
  const resizeSidebar = useCallback((clientX: number): void => {
    const shell = shellRef.current;
    if (!shell) return;
    const width = clampWidth(clientX - shell.getBoundingClientRect().left, SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH);
    sidebarDragWidthRef.current = width;
    setSidebarWidth(width);
  }, []);

  /** 基于工作区右边界更新检查器宽度，并记录是否已越过自动关闭阈值。 */
  const resizeInspector = useCallback((clientX: number): void => {
    const shell = shellRef.current;
    if (!shell) return;
    const width = clampWidth(shell.getBoundingClientRect().right - clientX, INSPECTOR_MIN_WIDTH, INSPECTOR_MAX_WIDTH);
    inspectorDragWidthRef.current = width;
    setInspectorWidth(width);
  }, []);

  /** 左栏拖拽结束时，低于阈值则自动收起；否则保留本次设定的宽度。 */
  function finishSidebarResize(): void {
    setResizing(false);
    if (sidebarDragWidthRef.current < SIDEBAR_COLLAPSE_THRESHOLD && !collapsed) toggleSidebar();
  }

  /** 右栏拖拽结束时，低于阈值则自动关闭；否则保留本次设定的宽度。 */
  function finishInspectorResize(): void {
    setResizing(false);
    if (inspectorDragWidthRef.current < INSPECTOR_COLLAPSE_THRESHOLD) closeInspector();
  }

  /** 处理左栏分隔条键盘调整，并在越过阈值时保持与拖拽一致的自动收起行为。 */
  function resizeSidebarByKeyboard(key: "ArrowLeft" | "ArrowRight" | "Home"): void {
    const next = key === "Home" ? SIDEBAR_DEFAULT_WIDTH : clampWidth(sidebarWidth + (key === "ArrowLeft" ? -16 : 16), SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH);
    sidebarDragWidthRef.current = next;
    setSidebarWidth(next);
    if (next < SIDEBAR_COLLAPSE_THRESHOLD && !collapsed) toggleSidebar();
  }

  /** 处理右栏分隔条键盘调整，并在越过阈值时保持与拖拽一致的自动关闭行为。 */
  function resizeInspectorByKeyboard(key: "ArrowLeft" | "ArrowRight" | "Home"): void {
    const next = key === "Home" ? INSPECTOR_DEFAULT_WIDTH : clampWidth(inspectorWidth + (key === "ArrowLeft" ? 16 : -16), INSPECTOR_MIN_WIDTH, INSPECTOR_MAX_WIDTH);
    inspectorDragWidthRef.current = next;
    setInspectorWidth(next);
    if (next < INSPECTOR_COLLAPSE_THRESHOLD) closeInspector();
  }

  const shellStyle = {
    "--sidebar-width": `${sidebarWidth}px`,
    "--inspector-width": `${inspectorWidth}px`,
  } as React.CSSProperties;

  return (
    <div className="orbit-root">
      <Titlebar />
      <div ref={shellRef} className={`app-shell${collapsed ? " sidebar-collapsed" : ""}${inspectorOpen ? "" : " inspector-collapsed"}${resizing ? " is-resizing" : ""}`} style={shellStyle}>
        <Sidebar />
        <ResizeHandle side="left" label="调整左侧栏宽度" onResizeStart={() => setResizing(true)} onResize={resizeSidebar} onResizeEnd={finishSidebarResize} onKeyboardResize={resizeSidebarByKeyboard} />
        <main className="workspace">
          <Suspense fallback={<div className="page-loading"><span className="page-loading-spinner" />加载中…</div>}>
            <Routes>
              <Route path="/"            element={<Navigate to="/today" replace />} />
              <Route path="/today"       element={<TodayPage />} />
              <Route path="/search"      element={<SearchPage />} />
              <Route path="/timeline"    element={<TimelinePage />} />
              <Route path="/inbox"       element={<InboxPage />} />
              <Route path="/review"      element={<ReviewPage />} />
              <Route path="/cards"       element={<CardsPage />} />
              <Route path="/cards/:deck" element={<CardsPage />} />
              <Route path="/ask"         element={<AskPage />} />
              <Route path="/graph"       element={<GraphPage />} />
              <Route path="/connections" element={<ConnectionsPage />} />
              <Route path="/settings"    element={<SettingsPage />} />
              <Route path="/memory/:id"  element={<SearchPage />} />
              <Route path="*"            element={<Navigate to="/today" replace />} />
            </Routes>
          </Suspense>
        </main>
        <ResizeHandle side="right" label="调整右侧检查器宽度" onResizeStart={() => setResizing(true)} onResize={resizeInspector} onResizeEnd={finishInspectorResize} onKeyboardResize={resizeInspectorByKeyboard} />
        <InspectorPanel />
      </div>
    </div>
  );
}

/** 提供检查器上下文后渲染可调整宽度的工作区。 */
function AppShell(): React.JSX.Element {
  return <InspectorProvider><WorkspaceShell /></InspectorProvider>;
}

/** 装配 Orbit 应用的全局布局状态。 */
export function App(): React.JSX.Element {
  return <SidebarProvider><AppShell /></SidebarProvider>;
}
