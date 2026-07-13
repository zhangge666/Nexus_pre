/** 本文件实现 Orbit 检索中心：聚焦搜索、结果列表与按需详情检查器。 */
import type React from "react";
import { useEffect, useMemo, useRef, useState } from "react";
import { Command, Filter, Plus, Search } from "lucide-react";
import { addMemoryToCollection, getMemory, listCollections, listMemories, searchMemory, updateMemory } from "../core";
import type { MemoryCollection, MemoryHit, MemorySummary, SearchMode } from "../core";
import { EmptyState } from "../components/EmptyState";
import { MemoryDetail } from "../components/MemoryDetail";
import { MemoryRow } from "../components/MemoryRow";
import { QuickCapture } from "../components/QuickCapture";
import { Topbar } from "../components/Topbar";

const SOURCES = ["all", "orbit", "muse", "quill", "echo"] as const;
const MODES: { key: SearchMode; label: string }[] = [
  { key: "hybrid", label: "混合" },
  { key: "semantic", label: "语义" },
  { key: "keyword", label: "关键词" },
];

/** 构造检索接口只返回命中片段时所需的最小列表行数据。 */
function toDisplayMemory(hit: MemoryHit, memories: MemorySummary[]): MemorySummary & { score: number; snippet: string } {
  const memory = memories.find((item) => item.id === hit.memoryId);
  return {
    ...(memory ?? {
      id: hit.memoryId,
      source: "orbit",
      kind: "note" as const,
      title: null,
      content: hit.snippet,
      contentFormat: "plain" as const,
      tags: [],
      pinned: false,
      archived: false,
      createdAt: 0,
      updatedAt: 0,
      capturedAt: null,
      links: [],
    }),
    score: hit.score,
    snippet: hit.snippet,
  };
}

/** 渲染检索中心的三栏工作台，详情与记录功能均按需展示。 */
export default function SearchPage(): React.JSX.Element {
  const [query, setQuery] = useState("");
  const [mode, setMode] = useState<SearchMode>("hybrid");
  const [hits, setHits] = useState<MemoryHit[]>([]);
  const [memories, setMemories] = useState<MemorySummary[]>([]);
  const [selected, setSelected] = useState<MemorySummary | null>(null);
  const [source, setSource] = useState<string>("all");
  const [collections, setCollections] = useState<MemoryCollection[]>([]);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("正在载入记忆");
  const [captureOpen, setCaptureOpen] = useState(false);
  const searchInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    void refresh();
    void listCollections().then(setCollections);
  }, []);

  useEffect(() => {
    /** 将 Ctrl/⌘ K 统一为聚焦检索入口，不与浏览器页面跳转冲突。 */
    function focusSearch(event: KeyboardEvent): void {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        searchInputRef.current?.focus();
      }
    }

    window.addEventListener("keydown", focusSearch);
    return () => window.removeEventListener("keydown", focusSearch);
  }, []);

  /** 根据来源重新加载列表，同时仅在已选项被删除时关闭检查器。 */
  async function refresh(nextSource?: string): Promise<void> {
    setBusy(true);
    try {
      const loaded = await listMemories(nextSource ?? source);
      setMemories(loaded);
      setSelected((current) => current && loaded.find((item) => item.id === current.id) ? current : null);
      setNotice(`已载入 ${loaded.length} 条记忆`);
    } catch (error) {
      setNotice(`加载失败：${String(error)}`);
    } finally {
      setBusy(false);
    }
  }

  /** 执行当前模式的检索；空查询恢复为时间线列表而非发起空请求。 */
  async function handleSearch(event: React.FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    if (!query.trim()) {
      setHits([]);
      setNotice("正在显示全部记忆");
      return;
    }

    setBusy(true);
    try {
      const loadedHits = await searchMemory({ query: query.trim(), mode });
      setHits(loadedHits);
      setNotice(`找到 ${loadedHits.length} 条相关记忆`);
    } catch (error) {
      setNotice(`检索失败：${String(error)}`);
    } finally {
      setBusy(false);
    }
  }

  /** 打开检查器前读取完整内容，保证编辑和详情拥有最新数据。 */
  async function handleSelect(memory: MemorySummary): Promise<void> {
    setSelected(memory);
    try {
      setSelected(await getMemory(memory.id));
    } catch {
      setNotice("无法读取完整记忆，正在显示列表摘要");
    }
  }

  /** 保存详情编辑，并同步更新主列表中同一条记忆的摘要。 */
  async function handleSave(id: string, title: string | null, content: string): Promise<void> {
    const updated = await updateMemory(id, title, content);
    setSelected(updated);
    setMemories((current) => current.map((memory) => memory.id === id ? updated : memory));
    setNotice("更改已保存");
  }

  /** 切换来源后清除旧检索命中，避免不同数据集混合显示。 */
  function handleSourceChange(nextSource: string): void {
    setSource(nextSource);
    setQuery("");
    setHits([]);
    void refresh(nextSource);
  }

  /** 新建记忆完成后把它插入列表并直接打开检查器，提供即时确认。 */
  function handleCreated(memory: MemorySummary): void {
    setMemories((current) => [memory, ...current]);
    setSelected(memory);
    setNotice("已写入一条新记忆");
  }

  const visibleMemories = useMemo(
    () => memories.filter((memory) => source === "all" || memory.source === source),
    [memories, source],
  );
  const displayList: Array<MemorySummary & { score?: number; snippet?: string }> = query
    ? hits.map((hit) => toDisplayMemory(hit, memories))
    : visibleMemories;
  const resultLabel = query ? `${hits.length} 条结果` : `${visibleMemories.length} 条记忆`;

  return (
    <div className={`search-workbench page-enter${selected ? " inspector-open" : ""}`}>
      <Topbar
        title="检索中心"
        subtitle={notice}
        actions={
          <>
            <button className="secondary-button command-entry" onClick={() => searchInputRef.current?.focus()}>
              <Command size={14} />搜索 <kbd>⌘ K</kbd>
            </button>
            <button className="primary-small" onClick={() => setCaptureOpen(true)}>
              <Plus size={15} />新建记忆
            </button>
          </>
        }
      />

      <section className="search-main" aria-labelledby="search-title">
        <div className="search-intro">
          <p>你的个人知识库</p>
          <h2 id="search-title">从所有上下文中找到答案</h2>
        </div>
        <form className="search-box" onSubmit={handleSearch}>
          <Search size={17} aria-hidden="true" />
          <input
            ref={searchInputRef}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="搜索内容、想法或来源"
            aria-label="搜索记忆"
          />
          <select value={mode} onChange={(event) => setMode(event.target.value as SearchMode)} aria-label="搜索模式">
            {MODES.map(({ key, label }) => <option key={key} value={key}>{label}</option>)}
          </select>
          <button type="submit" disabled={busy || !query.trim()}>{busy ? "搜索中" : "搜索"}</button>
        </form>
      </section>

      <div className="search-layout">
        <section className="results" aria-labelledby="results-title">
          <div className="results-toolbar">
            <div>
              <h2 id="results-title">{query ? "搜索结果" : "最近记忆"}</h2>
              <span>{resultLabel}</span>
            </div>
            <div className="filter-row" role="group" aria-label="来源筛选">
              <Filter size={14} aria-hidden="true" />
              {SOURCES.map((item) => (
                <button
                  key={item}
                  className={`filter-button${source === item ? " active" : ""}`}
                  onClick={() => handleSourceChange(item)}
                >
                  {item === "all" ? "全部" : item.charAt(0).toUpperCase() + item.slice(1)}
                </button>
              ))}
            </div>
          </div>

          <div className="result-list" aria-busy={busy}>
            {displayList.map((memory) => (
              <MemoryRow
                key={memory.id}
                memory={memory}
                selected={selected?.id === memory.id}
                snippet={memory.snippet}
                onClick={() => void handleSelect(memory)}
              />
            ))}
            {!busy && displayList.length === 0 && (
              <EmptyState
                icon={<Search size={32} />}
                title="没有匹配的记忆"
                description={query ? "尝试更换关键词或切换搜索模式。" : "当前来源还没有记忆，可先创建一条。"}
                action={!query ? { label: "新建记忆", onClick: () => setCaptureOpen(true) } : undefined}
              />
            )}
          </div>
        </section>

        {selected && (
          <aside className="detail-inspector" aria-label="记忆检查器">
            <MemoryDetail
              memory={selected}
              collections={collections}
              onClose={() => setSelected(null)}
              onSave={handleSave}
              onAddToCollection={(collectionId) => addMemoryToCollection(collectionId, selected.id)}
            />
          </aside>
        )}
      </div>

      <QuickCapture open={captureOpen} onClose={() => setCaptureOpen(false)} onCreated={handleCreated} />
    </div>
  );
}
