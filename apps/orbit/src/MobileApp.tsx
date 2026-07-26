/** 本文件实现 Orbit Android 专用外壳，复用内容页并提供底部主导航与详情 Sheet。 */

import type React from "react";
import { lazy, Suspense, useEffect, useState } from "react";
import { BrainCircuit, House, MessageCircle, Search, Settings } from "lucide-react";
import { NavLink, Navigate, Route, Routes } from "react-router-dom";
import { InspectorPanel, InspectorProvider } from "./components/Inspector";
import { getSettings } from "./core";

const TodayPage = lazy(() => import("./pages/TodayPage"));
const SearchPage = lazy(() => import("./pages/SearchPage"));
const TimelinePage = lazy(() => import("./pages/TimelinePage"));
const InboxPage = lazy(() => import("./pages/InboxPage"));
const ReviewPage = lazy(() => import("./pages/ReviewPage"));
const CardsPage = lazy(() => import("./pages/CardsPage"));
const AskPage = lazy(() => import("./pages/AskPage"));
const GraphPage = lazy(() => import("./pages/GraphPage"));
const ConnectionsPage = lazy(() => import("./pages/ConnectionsPage"));
const SettingsPage = lazy(() => import("./pages/SettingsPage"));

const MOBILE_TABS = [
  { to: "/today", label: "今日", icon: House },
  { to: "/search", label: "记忆", icon: Search },
  { to: "/review", label: "复习", icon: BrainCircuit },
  { to: "/ask", label: "问答", icon: MessageCircle },
  { to: "/settings", label: "设置", icon: Settings },
] as const;

/** 首次启动时根据 Keystore 中是否存在连接令牌选择工作台或连接设置。 */
function MobileLanding(): React.JSX.Element {
  const [target, setTarget] = useState<"/today" | "/settings" | null>(null);

  useEffect(() => {
    void getSettings()
      .then((settings) => setTarget(settings.sync.hasAccessToken ? "/today" : "/settings"))
      .catch(() => setTarget("/settings"));
  }, []);

  return target
    ? <Navigate to={target} replace />
    : <div className="page-loading"><span className="page-loading-spinner" />正在检查设备连接…</div>;
}

/** 渲染 Android 全屏工作区，并将常用入口固定在系统安全区上方。 */
function MobileShell(): React.JSX.Element {
  return (
    <div className="mobile-root">
      <main className="mobile-workspace">
        <Suspense fallback={<div className="page-loading"><span className="page-loading-spinner" />加载中…</div>}>
          <Routes>
            <Route path="/" element={<MobileLanding />} />
            <Route path="/today" element={<TodayPage />} />
            <Route path="/search" element={<SearchPage />} />
            <Route path="/timeline" element={<TimelinePage />} />
            <Route path="/inbox" element={<InboxPage />} />
            <Route path="/review" element={<ReviewPage />} />
            <Route path="/cards" element={<CardsPage />} />
            <Route path="/cards/:deck" element={<CardsPage />} />
            <Route path="/ask" element={<AskPage />} />
            <Route path="/graph" element={<GraphPage />} />
            <Route path="/connections" element={<ConnectionsPage />} />
            <Route path="/settings" element={<SettingsPage />} />
            <Route path="/memory/:id" element={<SearchPage />} />
            <Route path="*" element={<Navigate to="/today" replace />} />
          </Routes>
        </Suspense>
      </main>

      <InspectorPanel />

      <nav className="mobile-tabbar" aria-label="Android 主导航">
        {MOBILE_TABS.map(({ to, label, icon: Icon }) => (
          <NavLink key={to} to={to} className={({ isActive }) => `mobile-tab${isActive ? " active" : ""}`}>
            <Icon size={20} aria-hidden="true" />
            <span>{label}</span>
          </NavLink>
        ))}
      </nav>
    </div>
  );
}

/** 装配 Android 专用检查器状态和移动外壳。 */
export function MobileApp(): React.JSX.Element {
  return <InspectorProvider><MobileShell /></InspectorProvider>;
}
