/** 本文件实现侧边栏导航组件，使用 react-router-dom NavLink 自动高亮。 */
import type React from "react";
import { useEffect, useState, FormEvent } from "react";
import { NavLink, useNavigate } from "react-router-dom";
import {
  Orbit as OrbitIcon,
  Search,
  BookOpenText,
  Inbox,
  BrainCircuit,
  Layers,
  MessageCircle,
  Network,
  Link2,
  Settings,
  Database,
  FolderPlus,
  ChevronRight,
  Sun,
  Circle,
} from "lucide-react";
import { listCollections, createCollection } from "../core";
import type { MemoryCollection } from "../core";
import { useMemoryChanges } from "../core/events";
import { ServiceStatus } from "./ServiceStatus";
import { useSidebar } from "./SidebarState";

/** 将路由激活状态映射为设计系统定义的导航样式。 */
function navCls({ isActive }: { isActive: boolean }): string {
  return `nav-item${isActive ? " active" : ""}`;
}

/** 渲染 Orbit 全局导航与可折叠的记忆集合树。 */
export function Sidebar(): React.JSX.Element {
  const navigate = useNavigate();
  const { collapsed } = useSidebar();
  const [collections, setCollections] = useState<MemoryCollection[]>([]);
  const [collectionName, setCollectionName] = useState("");
  const [collectionsOpen, setCollectionsOpen] = useState(true);
  const [collectionNotice, setCollectionNotice] = useState("");

  useEffect(() => {
    void listCollections().then(setCollections).catch((error) => setCollectionNotice(`集合加载失败：${String(error)}`));
  }, []);

  /** 收到其他本地客户端的写入事件后，刷新集合树及其计数。 */
  useMemoryChanges(() => {
    void listCollections().then(setCollections).catch((error) => setCollectionNotice(`集合刷新失败：${String(error)}`));
  }, (error) => setCollectionNotice(`实时更新不可用：${String(error)}`));

  /** 创建集合后原地更新树，避免为这项轻量操作重载整个侧栏。 */
  async function handleCreateCollection(e: FormEvent): Promise<void> {
    e.preventDefault();
    if (!collectionName.trim()) return;
    try {
      const col = await createCollection(collectionName.trim());
      setCollections((prev) => [...prev, col]);
      setCollectionName("");
      setCollectionNotice("集合已创建");
    } catch (error) {
      // 保留用户输入的集合名称，方便修正服务状态后直接重试。
      setCollectionNotice(`创建集合失败：${String(error)}`);
    }
  }

  const roots = collections.filter((c: import("../core").MemoryCollection) => !c.parentId);
  /** 依据父级标识取出子集合，以缩进而非拟物图标表现层级。 */
  const childrenOf = (id: string) => collections.filter((c: import("../core").MemoryCollection) => c.parentId === id);

  return (
    <aside className={`sidebar${collapsed ? " collapsed" : ""}`}>
      {/* 品牌 */}
      <div className="brand" data-tauri-drag-region>
        <OrbitIcon size={18} aria-hidden="true" data-tauri-drag-region />
        <strong data-tauri-drag-region>Orbit</strong>
      </div>

      {/* 主导航 */}
      <nav aria-label="主导航">
        <NavLink className={navCls} to="/today" end title="今日" aria-label="今日">
          <Sun size={15} />今日
        </NavLink>
        <NavLink className={navCls} to="/search" title="记忆" aria-label="记忆">
          <Search size={15} />记忆
        </NavLink>
        <NavLink className={navCls} to="/timeline" title="时间线" aria-label="时间线">
          <BookOpenText size={15} />时间线
        </NavLink>
        <NavLink className={navCls} to="/inbox" title="收件箱" aria-label="收件箱">
          <Inbox size={15} />收件箱
        </NavLink>
        <NavLink className={navCls} to="/review" title="复习" aria-label="复习">
          <BrainCircuit size={15} />复习
        </NavLink>
        <NavLink className={navCls} to="/cards" title="卡片" aria-label="卡片">
          <Layers size={15} />卡片
        </NavLink>
        <NavLink className={navCls} to="/ask" title="问答" aria-label="问答">
          <MessageCircle size={15} />问答
        </NavLink>
        <NavLink className={navCls} to="/graph" title="知识图谱" aria-label="知识图谱">
          <Network size={15} />图谱
        </NavLink>
        <NavLink className={navCls} to="/connections" title="连接" aria-label="连接">
          <Link2 size={15} />连接
        </NavLink>
        <NavLink className={navCls} to="/settings" title="设置" aria-label="设置">
          <Settings size={15} />设置
        </NavLink>
      </nav>

      {/* 集合区块 */}
      <button
        className="nav-section-label collapsible"
        onClick={() => setCollectionsOpen((v) => !v)}
        aria-expanded={collectionsOpen}
      >
        集合
        <ChevronRight size={12} className={collectionsOpen ? "chevron open" : "chevron"} />
      </button>

      {collectionsOpen && (
        <nav aria-label="记忆集合">
          {roots.map((col) => (
            <div key={col.id}>
              <button
                className="nav-item collection-item"
                onClick={() => navigate(`/timeline?collection=${col.id}`)}
              >
                <Circle className="collection-icon" size={8} aria-hidden="true" />
                {col.name}
                {((col.count ?? 0) > 0) && (
                  <span className="count">{col.count}</span>
                )}
              </button>
              {childrenOf(col.id).map((child) => (
                <button
                  key={child.id}
                  className="nav-item collection-item nested"
                  onClick={() => navigate(`/timeline?collection=${child.id}`)}
                >
                  <Circle className="collection-icon" size={8} aria-hidden="true" />
                  {child.name}
                  {((child.count ?? 0) > 0) && (
                    <span className="count">{child.count}</span>
                  )}
                </button>
              ))}
            </div>
          ))}
          <NavLink className={navCls} to="/timeline">
            <Database size={15} />全部记忆
          </NavLink>

          {/* 新建集合 */}
          <form className="collection-form" onSubmit={handleCreateCollection}>
            <input
              value={collectionName}
              onChange={(e) => setCollectionName(e.target.value)}
              placeholder="新建集合"
              aria-label="新建集合名称"
            />
            <button type="submit" aria-label="创建集合" disabled={!collectionName.trim()}>
              <FolderPlus size={14} />
            </button>
          </form>
          {collectionNotice && <p className="collection-notice" role="status">{collectionNotice}</p>}
        </nav>
      )}

      {/* 底部 */}
      <div className="sidebar-footer">
        <ServiceStatus />
        {/* 用户个人卡片 */}
        <div className="sidebar-user-card">
          <div className="user-avatar">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
              <circle cx="12" cy="7" r="4" />
            </svg>
          </div>
          <div className="user-info">
            <span className="user-name">Alex Chen</span>
            <span className="user-desc">Personal Orbit</span>
          </div>
          <ChevronRight size={14} className="user-card-arrow" />
        </div>
      </div>
    </aside>
  );
}
