/** 本文件实现按日期分组的记忆时间线，详情统一展示于全局右侧检查器。 */
import type React from "react";
import { useCallback, useEffect, useState } from "react";
import { BookOpenText, Filter } from "lucide-react";
import { addMemoryToCollection, getMemory, listCollectionMemories, listCollections, listMemories, updateMemory } from "../core";
import { useSearchParams } from "react-router-dom";
import type { MemoryCollection, MemorySummary } from "../core";
import { EmptyState } from "../components/EmptyState";
import { MemoryDetail } from "../components/MemoryDetail";
import { MemoryRow } from "../components/MemoryRow";
import { Topbar } from "../components/Topbar";
import { useInspector } from "../components/Inspector";
import { PageLayout } from "../components/PageLayout";
import { useMemoryChanges } from "../core/events";

const SOURCES = ["all", "orbit", "muse", "quill", "echo"] as const;

/** 将记忆按相对日期分组，保证时间线可快速按时间扫描。 */
function groupByDate(memories: MemorySummary[]): Array<{ label: string; items: MemorySummary[] }> {
  const groups = new Map<string, MemorySummary[]>();
  const now = Date.now();
  for (const memory of memories) {
    const difference = now - memory.createdAt;
    const label = difference < 86_400_000 ? "今天" : difference < 172_800_000 ? "昨天" : new Intl.DateTimeFormat("zh-CN", { month: "long", day: "numeric" }).format(memory.createdAt);
    groups.set(label, [...(groups.get(label) ?? []), memory]);
  }
  return Array.from(groups, ([label, items]) => ({ label, items }));
}

/** 渲染时间线列表及其与全局详情检查器的交互。 */
export default function TimelinePage(): React.JSX.Element {
  const { show } = useInspector();
  const [searchParams] = useSearchParams();
  const [memories, setMemories] = useState<MemorySummary[]>([]);
  const [selected, setSelected] = useState<MemorySummary | null>(null);
  const [source, setSource] = useState("all");
  const [collections, setCollections] = useState<MemoryCollection[]>([]);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("正在读取本地记忆库");
  const collectionId = searchParams.get("collection");

  /** 根据当前来源刷新时间线数据。 */
  const refresh = useCallback(async (nextSource?: string): Promise<void> => {
    setBusy(true);
    try {
      const selectedSource = nextSource ?? source;
      const loaded = collectionId
        ? await listCollectionMemories(collectionId)
        : await listMemories(selectedSource);
      setMemories(selectedSource === "all" ? loaded : loaded.filter((memory) => memory.source === selectedSource));
      setNotice("本地记忆库已同步");
    } catch (error) {
      setNotice(`时间线加载失败：${String(error)}`);
    } finally { setBusy(false); }
  }, [collectionId, source]);

  useEffect(() => {
    void refresh();
    void listCollections().then(setCollections).catch((error) => setNotice(`集合加载失败：${String(error)}`));
  }, [refresh]);

  /** 接收 core 提交后的事件，确保时间线能展示其他客户端新写入的记忆。 */
  useMemoryChanges(() => {
    void refresh();
    void listCollections().then(setCollections).catch((error) => setNotice(`集合刷新失败：${String(error)}`));
  }, (error) => setNotice(`实时更新不可用：${String(error)}`));

  useEffect(() => {
    if (!selected) return;
    show("记忆详情", <MemoryDetail memory={selected} collections={collections} onClose={() => setSelected(null)} onSave={handleSave} onAddToCollection={handleAddToCollection} />);
  }, [collections, selected, show]);

  /** 读取完整记忆并在全局检查器中打开详情。 */
  async function handleSelect(memory: MemorySummary): Promise<void> {
    setSelected(memory);
    try { setSelected(await getMemory(memory.id)); } catch (error) { setNotice(`无法读取完整记忆：${String(error)}`); }
  }

  /** 保存详情编辑后同步更新当前时间线。 */
  async function handleSave(id: string, title: string | null, content: string): Promise<void> {
    try {
      const updated = await updateMemory(id, title, content);
      setMemories((current) => current.map((memory) => memory.id === id ? updated : memory));
      setSelected(updated);
      setNotice("记忆已保存");
    } catch (error) {
      setNotice(`保存失败：${String(error)}`);
      throw error;
    }
  }

  /** 将当前选中记忆加入集合，并同步刷新集合计数。 */
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

  /** 切换来源后重新加载，避免混合展示旧数据。 */
  function handleSourceChange(nextSource: string): void {
    setSource(nextSource);
    setSelected(null);
    void refresh(nextSource);
  }

  const displayed = source === "all" ? memories : memories.filter((memory) => memory.source === source);
  const groups = groupByDate(displayed);

  return (
    <PageLayout className="timeline-page">
      <Topbar title="时间线" subtitle={collectionId ? `集合中的 ${displayed.length} 条记忆 · ${notice}` : `${displayed.length} 条记忆 · ${notice}`} actions={<button className="secondary-button" onClick={() => void refresh()} disabled={busy}>重试</button>} />
      <div className="timeline-filters page-toolbar"><div className="filter-row" role="group" aria-label="来源筛选"><Filter size={14} aria-hidden="true" />{SOURCES.map((item) => <button key={item} className={`filter-button${source === item ? " active" : ""}`} onClick={() => handleSourceChange(item)}>{item === "all" ? "全部" : item.charAt(0).toUpperCase() + item.slice(1)}</button>)}</div></div>
      <section className="timeline-list page-list-content" aria-busy={busy}>
        {busy && <p className="loading-hint">加载中…</p>}
        {!busy && groups.length === 0 && <EmptyState icon={<BookOpenText size={36} />} title="还没有记忆" description="创建一条记忆后，它会按时间出现在这里。" />}
        {groups.map(({ label, items }) => <section key={label} className="timeline-group"><div className="timeline-date"><span>{label}</span><span className="timeline-count">{items.length} 条</span></div><div className="result-list">{items.map((memory) => <MemoryRow key={memory.id} memory={memory} selected={selected?.id === memory.id} onClick={() => void handleSelect(memory)} />)}</div></section>)}
      </section>
    </PageLayout>
  );
}
