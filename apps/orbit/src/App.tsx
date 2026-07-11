/** 本文件实现 Orbit 的 IPC 验证工作台、记忆写入和本地混合检索交互。 */

import {
  BookOpenText,
  BrainCircuit,
  CircleHelp,
  Command,
  Database,
  FileText,
  Inbox,
  Orbit as OrbitIcon,
  Plus,
  Search,
  Settings,
  Sparkles,
} from "lucide-react";
import { FormEvent, useMemo, useState } from "react";
import { createMemory, isTauriRuntime, MemoryHit, searchMemory } from "./core";

const sampleMemories: MemoryHit[] = [
  {
    memoryId: "preview-1",
    blockId: "preview-block-1",
    score: 0.91,
    snippet: "用统一的 Memory 模型连接捕获、检索与复习，让知识持续回到视野。",
  },
  {
    memoryId: "preview-2",
    blockId: "preview-block-2",
    score: 0.84,
    snippet: "Memory Protocol 通过能力令牌限制每个客户端的读写范围。",
  },
];

/** 渲染 Orbit 本地记忆工作台。 */
export function App(): React.JSX.Element {
  const [query, setQuery] = useState("");
  const [draft, setDraft] = useState("");
  const [hits, setHits] = useState<MemoryHit[]>(sampleMemories);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState(
    isTauriRuntime() ? "本地核心已连接" : "浏览器预览模式",
  );

  const resultLabel = useMemo(() => `${hits.length} 条记忆`, [hits.length]);

  /** 提交检索词并通过 IPC 刷新结果列表。 */
  async function handleSearch(event: FormEvent): Promise<void> {
    event.preventDefault();
    if (!query.trim()) return;
    if (!isTauriRuntime()) {
      setNotice("浏览器预览不连接本地记忆库");
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

  /** 将快速记录内容通过 IPC 写入统一记忆库。 */
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
      setNotice("记忆已写入");
    } catch (error) {
      setNotice(`写入失败：${String(error)}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand"><OrbitIcon size={18} aria-hidden="true" /><strong>Orbit</strong></div>
        <nav aria-label="主导航">
          <a className="nav-item active" href="#search"><Search size={15} />检索中心</a>
          <a className="nav-item" href="#inbox"><Inbox size={15} />收件箱<span className="count">8</span></a>
          <a className="nav-item" href="#review"><BrainCircuit size={15} />今日复习<span className="count primary">12</span></a>
          <a className="nav-item" href="#timeline"><BookOpenText size={15} />时间线</a>
        </nav>
        <div className="nav-section-label">集合</div>
        <nav aria-label="记忆集合">
          <a className="nav-item" href="#notes"><FileText size={15} />产品笔记</a>
          <a className="nav-item" href="#research"><Sparkles size={15} />研究资料</a>
          <a className="nav-item" href="#all"><Database size={15} />全部记忆</a>
        </nav>
        <div className="sidebar-footer">
          <button className="icon-button" title="帮助" aria-label="帮助"><CircleHelp size={16} /></button>
          <button className="icon-button" title="设置" aria-label="设置"><Settings size={16} /></button>
        </div>
      </aside>

      <main className="workspace" id="search">
        <header className="topbar">
          <div>
            <h1>检索中心</h1>
            <p>{notice}</p>
          </div>
          <button className="secondary-button"><Command size={14} />命令</button>
        </header>

        <section className="search-section" aria-labelledby="search-title">
          <h2 id="search-title">查找你的记忆</h2>
          <form className="search-box" onSubmit={handleSearch}>
            <Search size={17} aria-hidden="true" />
            <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索内容、想法或来源" aria-label="搜索记忆" />
            <button type="submit" disabled={busy || !query.trim()}>{busy ? "检索中" : "搜索"}</button>
          </form>
        </section>

        <div className="content-grid">
          <section className="results" aria-labelledby="results-title">
            <div className="section-heading"><h2 id="results-title">相关记忆</h2><span>{resultLabel}</span></div>
            <div className="result-list">
              {hits.map((hit, index) => (
                <article className="memory-row" key={hit.blockId}>
                  <div className="source-mark">{index === 0 ? "M" : "Q"}</div>
                  <div className="memory-content">
                    <div className="memory-meta"><span>{index === 0 ? "Muse" : "Quill"}</span><span>相关度 {Math.round(hit.score * 100)}%</span></div>
                    <p>{hit.snippet}</p>
                    <code>{hit.memoryId}</code>
                  </div>
                </article>
              ))}
            </div>
          </section>

          <aside className="quick-capture" aria-labelledby="capture-title">
            <div className="section-heading"><h2 id="capture-title">快速记录</h2><Plus size={15} /></div>
            <textarea value={draft} onChange={(event) => setDraft(event.target.value)} placeholder="记录此刻值得保留的内容…" aria-label="记忆内容" />
            <div className="capture-footer"><span>Markdown</span><button onClick={handleCreate} disabled={busy || !draft.trim()}>写入记忆</button></div>
          </aside>
        </div>
      </main>
    </div>
  );
}

