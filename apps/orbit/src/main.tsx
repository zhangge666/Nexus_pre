/** 本文件挂载 Orbit React 应用并加载 Nexus 统一设计 token。 */

import "@nexus/ui/styles.css";
import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import "./orbit.css";

/** 将 Orbit 工作台挂载到 HTML 根节点。 */
function mount(): void {
  const root = document.getElementById("root");
  if (!root) {
    throw new Error("缺少 Orbit 根节点");
  }
  ReactDOM.createRoot(root).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

mount();

