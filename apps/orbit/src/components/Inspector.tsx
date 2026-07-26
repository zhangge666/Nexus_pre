/** 本文件提供 Orbit 全局右侧检查器：页面在此展示详情，布局不再由页面各自分栏。 */
import type React from "react";
import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import { X } from "lucide-react";

interface InspectorState {
  open: boolean;
  title: string;
  content: React.ReactNode;
  show: (title: string, content: React.ReactNode) => void;
  present: (title: string, content: React.ReactNode) => void;
  close: () => void;
  toggle: () => void;
}

const InspectorContext = createContext<InspectorState | null>(null);

/** 为 Orbit 工作区提供单一、可控制的全局检查器状态。 */
export function InspectorProvider({ children }: { children: React.ReactNode }): React.JSX.Element {
  const [open, setOpen] = useState(() => typeof window === "undefined" || window.innerWidth > 1080);
  const [title, setTitle] = useState("检查器");
  const [content, setContent] = useState<React.ReactNode>(null);

  useEffect(() => {
    const compactViewport = window.matchMedia("(max-width: 1080px)");

    /** 进入双栏断点时自动收起检查器，避免从宽窗口缩小时遮挡主要工作区。 */
    function syncInspectorForViewport(event: MediaQueryListEvent | MediaQueryList): void {
      if (event.matches) setOpen(false);
    }

    syncInspectorForViewport(compactViewport);
    compactViewport.addEventListener("change", syncInspectorForViewport);
    return () => compactViewport.removeEventListener("change", syncInspectorForViewport);
  }, []);

  /** 打开检查器并替换当前页面的详情内容。 */
  const show = useCallback((nextTitle: string, nextContent: React.ReactNode): void => {
    setTitle(nextTitle);
    setContent(nextContent);
    setOpen(true);
  }, []);

  /** 更新检查器内容但不强制展开，供页面默认概览在窄窗口中保持低干扰。 */
  const present = useCallback((nextTitle: string, nextContent: React.ReactNode): void => {
    setTitle(nextTitle);
    setContent(nextContent);
  }, []);

  /** 收起检查器，但保留内容以便用户通过顶栏再次展开。 */
  const close = useCallback((): void => {
    setOpen(false);
  }, []);

  /** 切换检查器可见性，供顶栏的全局控制入口使用。 */
  const toggle = useCallback((): void => {
    setOpen((current) => !current);
  }, []);

  const value = useMemo(() => ({ open, title, content, show, present, close, toggle }), [open, title, content, present, show]);
  return <InspectorContext.Provider value={value}>{children}</InspectorContext.Provider>;
}

/** 读取全局检查器状态；必须在 InspectorProvider 内调用。 */
export function useInspector(): InspectorState {
  const context = useContext(InspectorContext);
  if (!context) throw new Error("useInspector 必须在 InspectorProvider 内使用");
  return context;
}

/** 渲染固定于应用布局右侧的可收起检查器容器。 */
export function InspectorPanel(): React.JSX.Element {
  const { open, title, content, close } = useInspector();
  return (
    <aside className={`global-inspector${open ? "" : " is-collapsed"}`} aria-label="全局检查器" aria-hidden={!open}>
      <div className="global-inspector-header">
        <h2>{title}</h2>
        <button type="button" className="icon-button inspector-close" aria-label="关闭检查器" onClick={close}>
          <X size={15} aria-hidden="true" />
        </button>
      </div>
      <div className="global-inspector-body">
        {content ?? (
          <div className="inspector-placeholder">
            <X size={18} aria-hidden="true" />
            <p>选择一条记忆或卡片以查看详情。</p>
          </div>
        )}
      </div>
    </aside>
  );
}
