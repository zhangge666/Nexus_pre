/** 本文件实现 Orbit 检索中心，并将详情与 AI 摘要统一交给全局右侧检查器展示。 */
import type React from "react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Command, Filter, Plus, Search, Sparkles } from "lucide-react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { addMemoryToCollection, deleteMemory, getMemory, getSettings, listCollections, listMemories, searchMemory, updateMemory } from "../core";
import type { MemoryCollection, MemoryHit, MemorySummary, SearchMode } from "../core";
import { EmptyState } from "../components/EmptyState";
import { MemoryDetail } from "../components/MemoryDetail";
import { MemoryRow } from "../components/MemoryRow";
import { QuickCapture } from "../components/QuickCapture";
import { Topbar } from "../components/Topbar";
import { useInspector } from "../components/Inspector";
import { PageLayout } from "../components/PageLayout";
import { useMemoryChanges } from "../core/events";
import { isAndroidPlatform } from "../platform";

const SOURCES = ["all", "orbit", "muse", "quill", "echo"] as const;
const MODES: { key: SearchMode; label: string }[] = [
  { key: "hybrid", label: "混合" },
  { key: "semantic", label: "语义" },
  { key: "keyword", label: "关键词" },
];

/** 将只含命中片段的检索响应补全为可在列表渲染的最小记忆对象。 */
function toDisplayMemory(hit: MemoryHit, memories: MemorySummary[]): MemorySummary & { score: number; snippet: string } {
  const memory = memories.find((item) => item.id === hit.memoryId);
  return {
    ...(memory ?? { id: hit.memoryId, source: "orbit", kind: "note" as const, title: null, content: hit.snippet, contentFormat: "plain" as const, tags: [], pinned: false, archived: false, createdAt: 0, updatedAt: 0, capturedAt: null, links: [] }),
    score: hit.score,
    snippet: hit.snippet,
  };
}

/** 渲染当前检索状态，避免在 M4 问答能力完成前把 mock 结果混入 M2 搜索流程。 */
function SearchInsight({ query }: { query: string }): React.JSX.Element {
  return (
    <section className="search-insight">
      <div className="inspector-section-title"><Sparkles size={14} /> 检索概览</div>
      <p className="inspector-question">{query ? `当前检索：${query}` : "输入关键词、想法或来源以检索本地记忆库。"}</p>
      <p className="inspector-answer">选择列表中的记忆后，可在此查看详情、编辑内容或加入集合。</p>
    </section>
  );
}

/** 渲染检索、来源筛选、快速记录与按需详情检查器协作的工作区。 */
export default function SearchPage(): React.JSX.Element {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const { present, show } = useInspector();
  const [query, setQuery] = useState("");
  const [mode, setMode] = useState<SearchMode>("hybrid");
  const [hits, setHits] = useState<MemoryHit[]>([]);
  const [memories, setMemories] = useState<MemorySummary[]>([]);
  const [selected, setSelected] = useState<MemorySummary | null>(null);
  const [source, setSource] = useState<string>("all");
  const [collections, setCollections] = useState<MemoryCollection[]>([]);
  const [notice, setNotice] = useState("正在显示全部记忆");
  const [busy, setBusy] = useState(false);
  const [captureOpen, setCaptureOpen] = useState(false);
  const [localKeywordOnly, setLocalKeywordOnly] = useState(false);
  const searchInputRef = useRef<HTMLInputElement>(null);

  /** 读取当前来源的记忆列表，并保留用户选择的来源。 */
  const refresh = useCallback(async (nextSource?: string): Promise<void> => {
    setBusy(true);
    try {
      setMemories(await listMemories(nextSource ?? source));
    } catch (error) {
      setNotice(`加载失败：${String(error)}`);
    } finally {
      setBusy(false);
    }
  }, [source]);

  useEffect(() => {
    void refresh();
    void listCollections().then(setCollections).catch((error) => setNotice(`集合加载失败：${String(error)}`));
  }, [refresh]);

  useEffect(() => {
    if (!isAndroidPlatform()) return;
    /** E2E 中继没有明文索引，切换模式时同步收窄为本机关键词检索。 */
    function refreshSearchCapability(): void {
      void getSettings().then((settings) => {
        const localOnly = settings.sync.mode === "e2e_cloud";
        setLocalKeywordOnly(localOnly);
        if (localOnly) setMode("keyword");
      }).catch(() => undefined);
    }
    refreshSearchCapability();
    window.addEventListener("orbit-settings-changed", refreshSearchCapability);
    return () => window.removeEventListener("orbit-settings-changed", refreshSearchCapability);
  }, []);

  /** 外部捕获端或另一 Orbit 窗口写入后，重新读取真实列表与集合树。 */
  useMemoryChanges(
    () => {
      void refresh();
      void listCollections().then(setCollections).catch((error) => setNotice(`集合刷新失败：${String(error)}`));
    },
    (error) => setNotice(`实时更新不可用：${String(error)}`),
  );

  useEffect(() => {
    const id = searchParams.get("id");
    if (id) void getMemory(id).then((memory) => void handleSelect(memory));
    if (searchParams.get("action") === "new") setCaptureOpen(true);
  }, [searchParams]);

  useEffect(() => {
    /** 为检索入口注册 Ctrl/⌘ K，避免隐藏高频键盘动作。 */
    function focusSearch(event: KeyboardEvent): void {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        searchInputRef.current?.focus();
      }
    }
    window.addEventListener("keydown", focusSearch);
    return () => window.removeEventListener("keydown", focusSearch);
  }, []);

  useEffect(() => {
    if (!selected) {
      present("检索概览", <SearchInsight query={query} />);
      return;
    }
    show("记忆详情", <MemoryDetail memory={selected} collections={collections} onClose={() => setSelected(null)} onSave={handleSave} onDelete={handleDelete} onAddToCollection={handleAddToCollection} onConflictResolved={handleConflictResolved} />);
  }, [collections, present, query, selected, show]);

  /** 仅执行真实 Memory Protocol 检索；问答能力属于 M4，不能阻断 M2 搜索结果。 */
  async function handleSearch(event: React.FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    if (!query.trim()) {
      setHits([]);
      setNotice("正在显示全部记忆");
      return;
    }
    setBusy(true);
    setSelected(null);
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

  /** 获取完整记忆后在全局检查器中展示，读取失败时保留已有列表摘要。 */
  async function handleSelect(memory: MemorySummary): Promise<void> {
    setSelected(memory);
    try { setSelected(await getMemory(memory.id)); } catch { setNotice("无法读取完整记忆，正在显示列表摘要"); }
  }

  /** 保存详情编辑并同步刷新当前列表行。 */
  async function handleSave(id: string, title: string | null, content: string): Promise<void> {
    const updated = await updateMemory(id, title, content);
    setMemories((current) => current.map((memory) => memory.id === id ? updated : memory));
    setSelected(updated);
    setNotice("更改已保存");
  }

  /** 冲突恢复或手工合并成功后同时刷新列表行与检查器内容。 */
  function handleConflictResolved(updated: MemorySummary): void {
    setMemories((current) => current.map((memory) => memory.id === updated.id ? updated : memory));
    setSelected(updated);
    setNotice("冲突已解决，新版本正在同步");
  }

  /** 删除当前记忆并清理本页列表与检索命中。 */
  async function handleDelete(id: string): Promise<void> {
    await deleteMemory(id);
    setMemories((current) => current.filter((memory) => memory.id !== id));
    setHits((current) => current.filter((hit) => hit.memoryId !== id));
    setSelected(null);
    setNotice("记忆已删除并等待同步");
  }

  /** 将当前记忆归入集合后刷新树计数，并在原位反馈操作结果。 */
  async function handleAddToCollection(collectionId: string): Promise<void> {
    if (!selected) return;
    try {
      await addMemoryToCollection(collectionId, selected.id);
      setCollections(await listCollections());
      setNotice("记忆已加入集合");
    } catch (error) {
      setNotice(`加入集合失败：${String(error)}`);
    }
  }

  /** 切换来源时恢复浏览模式，避免不同来源与旧命中混合。 */
  function handleSourceChange(nextSource: string): void {
    setSource(nextSource);
    setQuery("");
    setHits([]);
    setSelected(null);
    void refresh(nextSource);
  }

  /** 将新建记忆插入列表并打开全局详情检查器。 */
  function handleCreated(memory: MemorySummary): void {
    setMemories((current) => [memory, ...current]);
    setSelected(memory);
    setNotice("已写入一条新记忆");
  }

  const visibleMemories = useMemo(() => memories.filter((memory) => source === "all" || memory.source === source), [memories, source]);
  const displayList: Array<MemorySummary & { score?: number; snippet?: string }> = query ? hits.map((hit) => toDisplayMemory(hit, memories)) : visibleMemories;
  const resultLabel = query ? `${hits.length} 条结果` : `${visibleMemories.length} 条记忆`;

  return (
    <PageLayout className="search-page-workspace">
      <Topbar title="记忆" subtitle={notice} actions={<><button className="secondary-button command-entry" onClick={() => searchInputRef.current?.focus()}><Command size={14} />搜索 <kbd>⌘ K</kbd></button><button className="primary-small" onClick={() => setCaptureOpen(true)}><Plus size={15} />新建记忆</button></>} />
      <section className="search-main" aria-labelledby="search-title">
        <p id="search-title" className="search-description">{localKeywordOnly ? "在本机解密副本中检索，中继无法看到查询内容" : "从所有上下文中找到答案"}</p>
        <form className="search-box" onSubmit={handleSearch}>
          <Search size={17} aria-hidden="true" />
          <input ref={searchInputRef} value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索内容、想法或来源" aria-label="搜索记忆" />
          <select value={mode} onChange={(event) => setMode(event.target.value as SearchMode)} aria-label="搜索模式" disabled={localKeywordOnly}>{MODES.filter(({ key }) => !localKeywordOnly || key === "keyword").map(({ key, label }) => <option key={key} value={key}>{label}</option>)}</select>
          <button type="submit" disabled={busy || !query.trim()}>{busy ? "搜索中" : "搜索"}</button>
        </form>
      </section>
      <section className="results" aria-labelledby="results-title">
        <div className="results-toolbar"><div><h2 id="results-title">{query ? "搜索结果" : "最近记忆"}</h2><span>{resultLabel}</span></div><div className="filter-row" role="group" aria-label="来源筛选"><Filter size={14} aria-hidden="true" />{SOURCES.map((item) => <button key={item} className={`filter-button${source === item ? " active" : ""}`} onClick={() => handleSourceChange(item)}>{item === "all" ? "全部" : item.charAt(0).toUpperCase() + item.slice(1)}</button>)}</div></div>
        <div className="page-list-content"><div className="result-list" aria-busy={busy}>{displayList.map((memory) => <MemoryRow key={memory.id} memory={memory} selected={selected?.id === memory.id} snippet={memory.snippet} onClick={() => void handleSelect(memory)} />)}{!busy && displayList.length === 0 && <EmptyState icon={<Search size={32} />} title="没有匹配的记忆" description={query ? "尝试更换关键词或切换搜索模式。" : "当前来源还没有记忆，可先创建一条。"} action={!query ? { label: "新建记忆", onClick: () => setCaptureOpen(true) } : undefined} />}</div></div>
      </section>
      <QuickCapture open={captureOpen} onClose={() => setCaptureOpen(false)} onCreated={handleCreated} />
    </PageLayout>
  );
}
