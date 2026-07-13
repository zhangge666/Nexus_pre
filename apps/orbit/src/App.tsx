/** 本文件实现 Orbit 根布局：侧边栏 + 懒加载路由 Outlet。 */

import { lazy, Suspense } from "react";
import { Routes, Route, Navigate } from "react-router-dom";
import { Sidebar } from "./components/Sidebar";

const SearchPage      = lazy(() => import("./pages/SearchPage"));
const TimelinePage    = lazy(() => import("./pages/TimelinePage"));
const InboxPage       = lazy(() => import("./pages/InboxPage"));
const ReviewPage      = lazy(() => import("./pages/ReviewPage"));
const CardsPage       = lazy(() => import("./pages/CardsPage"));
const AskPage         = lazy(() => import("./pages/AskPage"));
const GraphPage       = lazy(() => import("./pages/GraphPage"));
const ConnectionsPage = lazy(() => import("./pages/ConnectionsPage"));
const SettingsPage    = lazy(() => import("./pages/SettingsPage"));

export function App(): React.JSX.Element {
  return (
    <div className="app-shell">
      <Sidebar />
      <main className="workspace">
        <Suspense fallback={<div className="page-loading"><span className="page-loading-spinner" />加载中…</div>}>
          <Routes>
            <Route path="/"            element={<Navigate to="/search" replace />} />
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
            <Route path="*"            element={<Navigate to="/search" replace />} />
          </Routes>
        </Suspense>
      </main>
    </div>
  );
}
