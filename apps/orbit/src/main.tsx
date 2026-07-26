/** 本文件挂载 Orbit React 应用，并包裹 HashRouter 用于 Tauri WebView 路由。 */

import "@nexus/ui/styles.css";
import React, { Suspense } from "react";
import ReactDOM from "react-dom/client";
import { HashRouter } from "react-router-dom";
import { App } from "./App";
import { MobileApp } from "./MobileApp";
import { isAndroidPlatform } from "./platform";
import "./orbit.css";
import "./mobile.css";

function mount(): void {
  const root = document.getElementById("root");
  if (!root) {
    throw new Error("缺少 Orbit 根节点");
  }
  const Application = isAndroidPlatform() ? MobileApp : App;
  ReactDOM.createRoot(root).render(
    <React.StrictMode>
      <HashRouter>
        <Application />
      </HashRouter>
    </React.StrictMode>,
  );
}

mount();
