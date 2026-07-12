/** 本文件实现 Orbit 记忆库主界面的检索、时间线筛选、详情阅读和快速记录交互。 */

import {
  Archive,
  BookOpenText,
  BrainCircuit,
  CircleHelp,
  Command,
  Database,
  FileText,
  Filter,
  FolderPlus,
  Inbox,
  Orbit as OrbitIcon,
  Plus,
  Search,
  Settings,
  Save,
  Sparkles,
  X,
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  createMemory,
  addMemoryToCollection,
  createCollection,
  getMemory,
  isTauriRuntime,
  listMemories,
  listCollections,
  MemoryCollection,
  MemoryHit,
  MemorySummary,
  searchMemory,
  updateMemory,
} from "./core";

const previewMemories: MemorySummary[] = [
  { id: "preview-1", source: "muse", kind: "idea", title: "统一记忆模型", content: "用统一的 Memory 模型连接捕获、检索与复习，让知识持续回到视野。", tags: ["产品", "想法"], pinned: true, archived: false, createdAt: Date.now() - 3_600_000 },
  { id: "preview-2", source: "quill", kind: "note", title: "本地协议边界", content: "Memory Protocol 通过能力令牌限制每个客户端的读写范围。", tags: ["架构"], pinned: false, archived: false, createdAt: Date.now() - 86_400_000 },
];

/** 把来源标识转换为界面显示名称。 */
function sourceLabel(source: string): string {
  return { orbit: "Orbit", muse: "Muse", quill: "Quill", echo: "Echo" }[source] ?? source;
}

/** 把 Unix 毫秒转换为紧凑的本地日期时间。 */
function formatTime(timestamp: number): string {
  return new Intl.DateTimeFormat("zh-CN", { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" }).format(timestamp);
}

/** 渲染 Orbit 本地记忆工作台。 */
export function App(): React.JSX.Element {
  const [query, setQuery] = useState("");
  const [draft, setDraft] = useState("");
  const [hits, setHits] = useState<MemoryHit[]>([]);
  const [memories, setMemories] = useState<MemorySummary[]>(previewMemories);
  const [selected, setSelected] = useState<MemorySummary | null>(previewMemories[0]);
  const [source, setSource] = useState<string>("all");
  const [busy, setBusy] = useState(false);
  const [collections, setCollections] = useState<MemoryCollection[]>([]);
  const [collectionName, setCollectionName] = useState("");
  const [editing, setEditing] = useState(false);
  const [editTitle, setEditTitle] = useState("");
  const [editContent, setEditContent] = useState("");
  const [notice, setNotice] = useState(isTauriRuntime() ? "正在连接本地记忆服务" : "浏览器预览模式");

  const visibleMemories = useMemo(
    () => memories.filter((memory) => source === "all" || memory.source === source),
    [memories, source],
  );
  const resultLabel = query ? `${hits.length} 条检索结果` : `${visibleMemories.length} 条时间线记忆`;

  /** 在桌面运行时加载时间线，浏览器中保留可交互预览数据。 */
  useEffect(() => {
    if (!isTauriRuntime()) return;
    void refreshTimeline();
    void refreshCollections();
  }, []);

  /** 订阅提交后事件，让其他本地应用写入的记忆自动出现在当前时间线。 */
  useEffect(() => {
    if (!isTauriRuntime()) return;
    let unlisten: (() => void) | undefined;
    void listen("memory-changed", () => {
      void refreshTimeline(source);
      void refreshCollections();
    }).then((dispose) => { unlisten = dispose; });
    return () => unlisten?.();
  }, [source]);

  /** 从本地服务刷新时间线并保留有效的详情选中项。 */
  async function refreshTimeline(nextSource?: string): Promise<void> {
    if (!isTauriRuntime()) return;
    setBusy(true);
    try {
      const loaded = await listMemories(nextSource === "all" ? undefined : nextSource);
      setMemories(loaded);
      setSelected((current) => loaded.find((memory) => memory.id === current?.id) ?? loaded[0] ?? null);
      setNotice(`已加载 ${loaded.length} 条记忆`);
    } catch (error) {
      setNotice(`加载失败：${String(error)}`);
    } finally {
      setBusy(false);
    }
  }

  /** 从本地服务刷新可用于归档的集合列表。 */
  async function refreshCollections(): Promise<void> {
    if (!isTauriRuntime()) return;
    try {
      setCollections(await listCollections());
    } catch (error) {
      setNotice(`集合加载失败：${String(error)}`);
    }
  }

  /** 创建集合并刷新侧边栏树。 */
  async function handleCreateCollection(event: FormEvent): Promise<void> {
    event.preventDefault();
    if (!collectionName.trim() || !isTauriRuntime()) return;
    try {
      await createCollection(collectionName.trim());
      setCollectionName("");
      await refreshCollections();
      setNotice("集合已创建");
    } catch (error) {
      setNotice(`创建集合失败：${String(error)}`);
    }
  }

  /** 打开详情编辑态并复制当前值，避免取消操作污染已保存内容。 */
  function beginEditing(): void {
    if (!selected) return;
    setEditTitle(selected.title ?? "");
    setEditContent(selected.content);
    setEditing(true);
  }

  /** 保存编辑后的记忆并同步刷新列表和详情。 */
  async function handleSaveMemory(): Promise<void> {
    if (!selected || !editContent.trim() || !isTauriRuntime()) return;
    try {
      const updated = await updateMemory(selected.id, editTitle.trim() || null, editContent.trim());
      setSelected(updated);
      setMemories((items) => items.map((item) => item.id === updated.id ? updated : item));
      setEditing(false);
      setNotice("记忆已保存");
    } catch (error) {
      setNotice(`保存失败：${String(error)}`);
    }
  }

  /** 将当前记忆加入选择的集合。 */
  async function handleAddToCollection(collectionId: string): Promise<void> {
    if (!selected || !isTauriRuntime()) return;
    try {
      await addMemoryToCollection(collectionId, selected.id);
      setNotice("记忆已加入集合");
    } catch (error) {
      setNotice(`归档失败：${String(error)}`);
    }
  }

  /** 提交检索词并通过 IPC 刷新结果列表。 */
  async function handleSearch(event: FormEvent): Promise<void> {
    event.preventDefault();
    if (!query.trim()) {
      setHits([]);
      return;
    }
    if (!isTauriRuntime()) {
      setHits(previewMemories.map((memory, index) => ({ memoryId: memory.id, blockId: memory.id, score: 0.92 - index * 0.08, snippet: memory.content })));
      setNotice("浏览器预览显示示例检索结果");
      return;
    }
    setBusy(true);
    try {
      setHits(await searchMemory(query.trim()));
      setNotice("检索完成");
    } catch (error) {
      setNotice(`检索失败：${String(error)}`);
    } finally {
      setBusy(false);
    }
  }

  /** 将快速记录内容通过 IPC 写入统一记忆库并刷新时间线。 */
  async function handleCreate(): Promise<void> {
    if (!draft.trim()) return;
    if (!isTauriRuntime()) {
      setNotice("浏览器预览不写入本地记忆库");
      return;
    }
    setBusy(true);
    try {
      await createMemory(draft.trim());
      setDraft("");
      await refreshTimeline(source);
      setNotice("记忆已写入并刷新时间线");
    } catch (error) {
      setNotice(`写入失败：${String(error)}`);
    } finally {
      setBusy(false);
    }
  }

  /** 切换来源筛选并同步刷新本地时间线。 */
  function handleSourceChange(nextSource: string): void {
    setSource(nextSource);
    void refreshTimeline(nextSource);
  }

  /** 打开记忆详情；本地运行时额外读取最新完整内容。 */
  async function handleSelect(memory: MemorySummary): Promise<void> {
    setSelected(memory);
    if (!isTauriRuntime()) return;
    try {
      setSelected(await getMemory(memory.id));
    } catch (error) {
      setNotice(`读取详情失败：${String(error)}`);
    }
  }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand"><OrbitIcon size={18} aria-hidden="true" /><strong>Orbit</strong></div>
        <nav aria-label="主导航">
          <a className="nav-item active" href="#search"><Search size={15} />检索中心</a>
          <a className="nav-item" href="#timeline"><BookOpenText size={15} />时间线</a>
          <a className="nav-item" href="#inbox"><Inbox size={15} />收件箱<span className="count">8</span></a>
          <a className="nav-item" href="#review"><BrainCircuit size={15} />今日复习<span className="count primary">12</span></a>
        </nav>
        <div className="nav-section-label">集合</div>
        <nav aria-label="记忆集合">
          {collections.map((collection) => <a className="nav-item" href="#timeline" key={collection.id}><FileText size={15} />{collection.name}</a>)}
          {collections.length === 0 && <a className="nav-item" href="#notes"><Sparkles size={15} />尚未创建集合</a>}
          <a className="nav-item" href="#all"><Database size={15} />全部记忆</a>
        </nav>
        <form className="collection-form" onSubmit={handleCreateCollection}><input value={collectionName} onChange={(event) => setCollectionName(event.target.value)} placeholder="新建集合" aria-label="新建集合名称" /><button type="submit" aria-label="创建集合" disabled={!collectionName.trim()}><FolderPlus size={14} /></button></form>
        <div className="sidebar-footer"><button className="icon-button" title="帮助" aria-label="帮助"><CircleHelp size={16} /></button><button className="icon-button" title="设置" aria-label="设置"><Settings size={16} /></button></div>
      </aside>

      <main className="workspace" id="search">
        <header className="topbar"><div><h1>检索中心</h1><p aria-live="polite">{notice}</p></div><button className="secondary-button"><Command size={14} />命令</button></header>
        <section className="search-section" aria-labelledby="search-title"><h2 id="search-title">查找你的记忆</h2><form className="search-box" onSubmit={handleSearch}><Search size={17} aria-hidden="true" /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索内容、想法或来源" aria-label="搜索记忆" /><button type="submit" disabled={busy || !query.trim()}>{busy ? "检索中" : "搜索"}</button></form></section>

        <div className="content-grid">
          <section className="results" id="timeline" aria-labelledby="results-title">
            <div className="section-heading"><h2 id="results-title">{query ? "相关记忆" : "时间线"}</h2><span>{resultLabel}</span></div>
            {!query && <div className="filter-row" role="group" aria-label="来源筛选"><Filter size={14} aria-hidden="true" />{["all", "orbit", "muse", "quill", "echo"].map((value) => <button className={source === value ? "filter-button active" : "filter-button"} key={value} onClick={() => handleSourceChange(value)}>{value === "all" ? "全部" : sourceLabel(value)}</button>)}</div>}
            <div className="result-list">
              {(query ? hits.map((hit) => ({ id: hit.memoryId, source: "orbit", kind: "note", title: null, content: hit.snippet, tags: [], pinned: false, archived: false, createdAt: 0, score: hit.score })) : visibleMemories).map((memory) => (
                <button className={selected?.id === memory.id ? "memory-row selected" : "memory-row"} key={memory.id} onClick={() => void handleSelect(memory)}>
                  <span className="source-mark">{sourceLabel(memory.source).slice(0, 1)}</span><span className="memory-content"><span className="memory-meta"><span>{sourceLabel(memory.source)}</span><span>{memory.createdAt ? formatTime(memory.createdAt) : "检索命中"}</span></span><strong>{memory.title ?? memory.kind}</strong><span>{memory.content}</span>{memory.tags.length > 0 && <span className="tag-list">{memory.tags.map((tag) => <em key={tag}>{tag}</em>)}</span>}</span>
                </button>
              ))}
              {!busy && (query ? hits.length === 0 : visibleMemories.length === 0) && <div className="empty-state"><Archive size={18} /><p>没有匹配的记忆</p></div>}
            </div>
          </section>

          <aside className="side-panels"><section className="quick-capture" aria-labelledby="capture-title"><div className="section-heading"><h2 id="capture-title">快速记录</h2><Plus size={15} /></div><textarea value={draft} onChange={(event) => setDraft(event.target.value)} placeholder="记录此刻值得保留的内容…" aria-label="记忆内容" /><div className="capture-footer"><span>Markdown</span><button onClick={() => void handleCreate()} disabled={busy || !draft.trim()}>写入记忆</button></div></section>
          {selected && <section className="memory-detail" aria-labelledby="detail-title"><div className="section-heading"><h2 id="detail-title">记忆详情</h2><span><button className="detail-action" onClick={beginEditing}>编辑</button><button className="icon-button" onClick={() => setSelected(null)} aria-label="关闭详情"><X size={15} /></button></span></div><div className="detail-body">{editing ? <><input className="detail-input" value={editTitle} onChange={(event) => setEditTitle(event.target.value)} placeholder="标题" aria-label="记忆标题" /><textarea className="detail-editor" value={editContent} onChange={(event) => setEditContent(event.target.value)} aria-label="记忆正文" /><div className="detail-controls"><button className="secondary-button" onClick={() => setEditing(false)}>取消</button><button className="primary-small" onClick={() => void handleSaveMemory()}><Save size={14} />保存</button></div></> : <><div className="memory-meta"><span>{sourceLabel(selected.source)}</span><span>{formatTime(selected.createdAt)}</span></div><h3>{selected.title ?? selected.kind}</h3><p>{selected.content}</p>{selected.tags.length > 0 && <div className="tag-list">{selected.tags.map((tag) => <em key={tag}>{tag}</em>)}</div>}{collections.length > 0 && <select className="collection-select" defaultValue="" onChange={(event) => { if (event.target.value) void handleAddToCollection(event.target.value); event.currentTarget.value = ""; }} aria-label="加入集合"><option value="">加入集合…</option>{collections.map((collection) => <option key={collection.id} value={collection.id}>{collection.name}</option>)}</select>}<code>{selected.id}</code></>}</div></section>}</aside>
        </div>
      </main>
    </div>
  );
}
