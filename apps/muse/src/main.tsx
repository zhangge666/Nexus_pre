/** 本文件挂载 Muse 可独立运行的多页面 React 桌面界面。 */
import React from "react";
import ReactDOM from "react-dom/client";
import "@nexus/ui/styles.css";
import { App } from "./App";
import { isToolWindowType, ToolWindowRouter } from "./tool-windows/ToolWindowRouter";

/** 将根组件挂载到 Tauri WebView 页面。 */
function mount(): void {
  const root = document.getElementById("root");
  if (root === null) throw new Error("缺少 Muse 根挂载节点");
  const windowType = new URLSearchParams(window.location.search).get("window");
  const content = isToolWindowType(windowType) ? <ToolWindowRouter type={windowType} /> : <App />;
  ReactDOM.createRoot(root).render(
    <React.StrictMode>
      {content}
    </React.StrictMode>,
  );
}

mount();
