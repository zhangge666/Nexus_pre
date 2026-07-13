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
  CircleHelp,
  ChevronRight,
} from "lucide-react";
import { listCollections, createCollection, getReviewStats, listInboxItems } from "../core";
import type { MemoryCollection } from "../core";

function navCls({ isActive }: { isActive: boolean }): string {
  return `nav-item${isActive ? " active" : ""}`;
}

export function Sidebar(): React.JSX.Element {
  const navigate = useNavigate();
  const [collections, setCollections] = useState<MemoryCollection[]>([]);
  const [collectionName, setCollectionName] = useState("");
  const [dueCount, setDueCount] = useState(0);
  const [inboxCount, setInboxCount] = useState(0);
  const [collectionsOpen, setCollectionsOpen] = useState(true);

  useEffect(() => {
    void listCollections().then(setCollections);
    void getReviewStats().then((s) => setDueCount(s.dueToday));
    void listInboxItems().then((items) => setInboxCount(items.filter((i) => !i.read).length));
  }, []);

  async function handleCreateCollection(e: FormEvent): Promise<void> {
    e.preventDefault();
    if (!collectionName.trim()) return;
    const col = await createCollection(collectionName.trim());
    setCollections((prev) => [...prev, col]);
    setCollectionName("");
  }

  const roots = collections.filter((c: import("../core").MemoryCollection) => !c.parentId);
  const childrenOf = (id: string) => collections.filter((c: import("../core").MemoryCollection) => c.parentId === id);

  return (
    <aside className="sidebar">
      {/* 品牌 */}
      <div className="brand">
        <OrbitIcon size={18} aria-hidden="true" />
        <strong>Orbit</strong>
      </div>

      {/* 主导航 */}
      <nav aria-label="主导航">
        <NavLink className={navCls} to="/search" end>
          <Search size={15} />检索中心
        </NavLink>
        <NavLink className={navCls} to="/timeline">
          <BookOpenText size={15} />时间线
        </NavLink>
        <NavLink className={navCls} to="/inbox">
          <Inbox size={15} />收件箱
          {inboxCount > 0 && <span className="count">{inboxCount}</span>}
        </NavLink>
        <NavLink className={navCls} to="/review">
          <BrainCircuit size={15} />今日复习
          {dueCount > 0 && <span className="count primary">{dueCount}</span>}
        </NavLink>
        <NavLink className={navCls} to="/cards">
          <Layers size={15} />知识卡片
        </NavLink>
        <NavLink className={navCls} to="/ask">
          <MessageCircle size={15} />记忆问答
        </NavLink>
        <NavLink className={navCls} to="/graph">
          <Network size={15} />知识图谱
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
                <span className="collection-icon">{col.icon ?? "📁"}</span>
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
                  <span className="collection-icon">{child.icon ?? "📄"}</span>
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
        </nav>
      )}

      {/* 底部 */}
      <div className="sidebar-footer">
        <NavLink className={navCls} to="/connections">
          <Link2 size={15} />连接管理
        </NavLink>
        <NavLink className={navCls} to="/settings">
          <Settings size={15} />设置
        </NavLink>
        <button className="icon-button" title="帮助" aria-label="帮助" style={{ marginTop: "auto" }}>
          <CircleHelp size={16} />
        </button>
      </div>
    </aside>
  );
}
