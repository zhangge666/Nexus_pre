/** 本文件实现检索中心页面，支持混合/语义/关键词三种检索模式与来源筛选。 */
import type React from "react";
import { useState, useEffect, useMemo, FormEvent } from "react";
import { Search, Filter, Command } from "lucide-react";
import { searchMemory, listMemories, getMemory, updateMemory, listCollections, addMemoryToCollection } from "../core";
import type { MemorySummary, MemoryHit, MemoryCollection, SearchMode } from "../core";
import { Topbar } from "../components/Topbar";
import { MemoryRow } from "../components/MemoryRow";
import { MemoryDetail } from "../components/MemoryDetail";
import { QuickCapture } from "../components/QuickCapture";
import { EmptyState } from "../components/EmptyState";

const SOURCES = ["all", "orbit", "muse", "quill", "echo"] as const;
const MODES: { key: SearchMode; label: string }[] = [
  { key: "hybrid", label: "混合" },
  { key: "semantic", label: "语义" },
  { key: "keyword", label: "关键词" },
];

export default function SearchPage(): React.JSX.Element {
  const [query, setQuery] = useState("");
  const [mode, setMode] = useState<SearchMode>("hybrid");
  const [hits, setHits] = useState<MemoryHit[]>([]);
  const [memories, setMemories] = useState<MemorySummary[]>([]);
  const [selected, setSelected] = useState<MemorySummary | null>(null);
  const [source, setSource] = useState<string>("all");
  const [collections, setCollections] = useState<MemoryCollection[]>([]);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("加载中…");

  useEffect(() => {
    void refresh();
    void listCollections().then(setCollections);
  }, []);

  async function refresh(src?: string): Promise<void> {
    setBusy(true);
    try {
      const loaded = await listMemories(src ?? source);
      setMemories(loaded);
      setSelected((cur) => loaded.find((m) => m.id === cur?.id) ?? loaded[0] ?? null);
      setNotice(`已加载 ${loaded.length} 条记忆`);
    } catch (e) {
      setNotice(`加载失败：${String(e)}`);
    } finally {
      setBusy(false);
    }
  }

  async function handleSearch(e: FormEvent): Promise<void> {
    e.preventDefault();
    if (!query.trim()) { setHits([]); return; }
    setBusy(true);
    try {
      setHits(await searchMemory({ query: query.trim(), mode }));
      setNotice("检索完成");
    } catch (e) {
      setNotice(`检索失败：${String(e)}`);
    } finally {
      setBusy(false);
    }
  }

  async function handleSelect(mem: MemorySummary): Promise<void> {
    setSelected(mem);
    try { setSelected(await getMemory(mem.id)); } catch {}
  }

  async function handleSave(id: string, title: string | null, content: string): Promise<void> {
    const updated = await updateMemory(id, title, content);
    setSelected(updated);
    setMemories((ms) => ms.map((m) => m.id === id ? updated : m));
    setNotice("已保存");
  }

  function handleSourceChange(s: string): void {
    setSource(s);
    void refresh(s);
  }

  const visibleMemories = useMemo(
    () => memories.filter((m) => source === "all" || m.source === source),
    [memories, source],
  );

  const displayList: (MemorySummary & { score?: number; snippet?: string })[] = query
    ? hits.map((h) => {
        const mem = memories.find((m) => m.id === h.memoryId);
        return { ...(mem ?? { id: h.memoryId, source: "orbit", kind: "note" as const, title: null, content: h.snippet, contentFormat: "plain" as const, tags: [], pinned: false, archived: false, createdAt: 0, updatedAt: 0, capturedAt: null, links: [] }), score: h.score, snippet: h.snippet };
      })
    : visibleMemories;

  const resultLabel = query
    ? `${hits.length} 条检索结果`
    : `${visibleMemories.length} 条时间线记忆`;

  return (
    <div className="page-enter">
      <Topbar
        title="检索中心"
        subtitle={notice}
        actions={
          <button className="secondary-button"><Command size={14} />命令</button>
        }
      />

      {/* 搜索区 */}
      <section className="search-section" aria-labelledby="search-title">
        <h2 id="search-title">查找你的记忆</h2>
        <form className="search-box" onSubmit={handleSearch}>
          <Search size={17} aria-hidden="true" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="搜索内容、想法或来源…"
            aria-label="搜索记忆"
          />
          {/* 检索模式 */}
          <div className="mode-tabs" role="group" aria-label="检索模式">
            {MODES.map(({ key, label }) => (
              <button
                key={key}
                type="button"
                className={`mode-tab${mode === key ? " active" : ""}`}
                onClick={() => setMode(key)}
              >
                {label}
              </button>
            ))}
          </div>
          <button type="submit" disabled={busy || !query.trim()}>
            {busy ? "检索中" : "搜索"}
          </button>
        </form>
      </section>

      <div className="content-grid">
        {/* 结果列表 */}
        <section className="results" aria-labelledby="results-title">
          <div className="section-heading">
            <h2 id="results-title">{query ? "相关记忆" : "时间线"}</h2>
            <span>{resultLabel}</span>
          </div>

          {!query && (
            <div className="filter-row" role="group" aria-label="来源筛选">
              <Filter size={14} aria-hidden="true" />
              {SOURCES.map((s) => (
                <button
                  key={s}
                  className={`filter-button${source === s ? " active" : ""}`}
                  onClick={() => handleSourceChange(s)}
                >
                  {s === "all" ? "全部" : s.charAt(0).toUpperCase() + s.slice(1)}
                </button>
              ))}
            </div>
          )}

          <div className="result-list">
            {displayList.map((mem) => (
              <MemoryRow
                key={mem.id}
                memory={mem}
                selected={selected?.id === mem.id}
                snippet={mem.snippet}
                onClick={() => void handleSelect(mem)}
              />
            ))}
            {!busy && displayList.length === 0 && (
              <EmptyState
                icon={<Search size={32} />}
                title="没有匹配的记忆"
                description={query ? "试试换个关键词或切换检索模式" : "还没有任何记忆，开始记录吧"}
              />
            )}
          </div>
        </section>

        {/* 侧面板 */}
        <aside className="side-panels">
          <QuickCapture onCreated={() => void refresh()} />
          {selected && (
            <MemoryDetail
              memory={selected}
              collections={collections}
              onClose={() => setSelected(null)}
              onSave={handleSave}
              onAddToCollection={(colId) => addMemoryToCollection(colId, selected.id)}
            />
          )}
        </aside>
      </div>
    </div>
  );
}
