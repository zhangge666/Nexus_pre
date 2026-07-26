/**
 * 本文件实现 Orbit 的 Today 工作台。
 * 页面只展示来自 Memory Protocol 的真实记忆与集合数据，并提供快速记录与诊断入口。
 */
import type React from "react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Database, FolderTree, Plus, Search, Sun } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { getServiceStatus, listCollections, listMemories } from "../core";
import type { MemoryCollection, MemorySummary, ServiceStatus } from "../core";
import { MemoryRow } from "../components/MemoryRow";
import { QuickCapture } from "../components/QuickCapture";
import { Topbar } from "../components/Topbar";
import { useInspector } from "../components/Inspector";
import { PageLayout } from "../components/PageLayout";
import { useMemoryChanges } from "../core/events";
import { isAndroidPlatform } from "../platform";

interface TodayInspectorProps {
  collectionCount: number;
  totalMemories: number;
  status: ServiceStatus | null;
}

/** 显示数据规模与本地服务健康度，作为工作台右侧的基础诊断内容。 */
function TodayInspector({ collectionCount, totalMemories, status }: TodayInspectorProps): React.JSX.Element {
  const remote = status?.role === "remote";
  const serviceText = status?.available ? `${remote ? "远程" : "本地"}服务已连接` : status?.message ?? "正在检查记忆服务";
  return (
    <div className="today-inspector">
      <section className="inspector-section">
        <div className="review-inspector-title"><Database size={14} /><span>{remote ? "远程记忆服务" : "本地记忆库"}</span></div>
        <div className="review-detail-grid">
          <div className="review-detail-row"><span>已保存记忆</span><strong>{totalMemories} 条</strong></div>
          <div className="review-detail-row"><span>集合</span><strong>{collectionCount} 个</strong></div>
        </div>
      </section>
      <section className="inspector-section">
        <div className="review-inspector-title"><span>服务诊断</span></div>
        <p className="inspector-answer">{serviceText}</p>
        {status && <p className="inspector-question">{status.role === "holder" ? "当前 Orbit 正在持有本地服务。" : status.role === "remote" ? "当前 Android 客户端通过 HTTPS 连接远程记忆服务。" : "当前 Orbit 已连接到另一实例持有的本地服务。"}</p>}
      </section>
    </div>
  );
}

/** 判断一条记忆是否在当前自然日创建，用于展示真实的今日写入数量。 */
function isCreatedToday(memory: MemorySummary): boolean {
  const date = new Date(memory.createdAt);
  const today = new Date();
  return date.getFullYear() === today.getFullYear()
    && date.getMonth() === today.getMonth()
    && date.getDate() === today.getDate();
}

/** 渲染真实数据驱动的 Today 工作台，并接收其他本地客户端写入的实时刷新。 */
export default function TodayPage(): React.JSX.Element {
  const android = isAndroidPlatform();
  const navigate = useNavigate();
  const { present } = useInspector();
  const [memories, setMemories] = useState<MemorySummary[]>([]);
  const [collections, setCollections] = useState<MemoryCollection[]>([]);
  const [serviceStatus, setServiceStatus] = useState<ServiceStatus | null>(null);
  const [notice, setNotice] = useState(android ? "正在连接远程记忆服务" : "正在读取本地记忆库");
  const [captureOpen, setCaptureOpen] = useState(false);

  /** 同步工作台的记忆、集合与本地服务状态，并保留可读的失败原因。 */
  const refresh = useCallback(async (): Promise<void> => {
    try {
      const [nextMemories, nextCollections, nextStatus] = await Promise.all([
        listMemories(),
        listCollections(),
        getServiceStatus(),
      ]);
      setMemories(nextMemories);
      setCollections(nextCollections);
      setServiceStatus(nextStatus);
      setNotice(nextStatus.available ? `${android ? "远程" : "本地"}记忆服务已同步` : nextStatus.message ?? "记忆服务不可用");
    } catch (error) {
      setNotice(`读取记忆服务失败：${String(error)}`);
    }
  }, [android]);

  useEffect(() => { void refresh(); }, [refresh]);

  /** core 事务提交后立刻刷新，覆盖其他产品后续经 Protocol 写入的场景。 */
  useMemoryChanges(
    () => { void refresh(); },
    (error) => setNotice(`实时更新不可用：${String(error)}`),
  );

  const recentMemories = useMemo(() => memories.slice(0, 6), [memories]);
  const createdToday = useMemo(() => memories.filter(isCreatedToday).length, [memories]);

  useEffect(() => {
    present("今日概览", <TodayInspector collectionCount={collections.length} totalMemories={memories.length} status={serviceStatus} />);
  }, [collections.length, memories.length, present, serviceStatus]);

  /** 新建记忆后将其放入当前列表并触发完整刷新，以同步集合计数与服务状态。 */
  function handleCreated(memory: MemorySummary): void {
    setMemories((current) => [memory, ...current.filter((item) => item.id !== memory.id)]);
    setNotice(`新记忆已写入${android ? "远程" : "本地"}服务`);
    void refresh();
  }

  return (
    <PageLayout className="today-page">
      <Topbar
        title="Today"
        subtitle={notice}
        actions={<button className="primary-small" onClick={() => setCaptureOpen(true)}><Plus size={15} />新建记忆</button>}
      />
      <section className="stats-cards-grid" aria-label="记忆服务概览">
        <button className="dashboard-stat-card" onClick={() => navigate("/search")}>
          <span className="stat-card-header">全部记忆 <Database size={14} /></span>
          <strong className="stat-card-value">{memories.length}<span>条</span></strong>
          <span className="stat-card-footer muted">浏览与检索本地知识</span>
        </button>
        <button className="dashboard-stat-card" onClick={() => navigate("/timeline")}>
          <span className="stat-card-header">今日新增 <Sun size={14} /></span>
          <strong className="stat-card-value">{createdToday}<span>条</span></strong>
          <span className="stat-card-footer success">按时间线查看</span>
        </button>
        <button className="dashboard-stat-card" onClick={() => navigate("/timeline")}>
          <span className="stat-card-header">集合 <FolderTree size={14} /></span>
          <strong className="stat-card-value">{collections.length}<span>个</span></strong>
          <span className="stat-card-footer muted">整理你的记忆</span>
        </button>
      </section>
      <section className="recent-memories-section" aria-labelledby="recent-title">
        <div className="results-toolbar">
          <div><h2 id="recent-title">最近记忆</h2><span>{recentMemories.length} 条</span></div>
          <button className="secondary-button" onClick={() => navigate("/search")}><Search size={14} />打开检索</button>
        </div>
        <div className="result-list">
          {recentMemories.map((memory) => <MemoryRow key={memory.id} memory={memory} onClick={() => navigate(`/search?id=${memory.id}`)} />)}
          {recentMemories.length === 0 && <p className="loading-hint">还没有记忆。创建第一条记忆后，它会出现在这里。</p>}
        </div>
      </section>
      <QuickCapture open={captureOpen} onClose={() => setCaptureOpen(false)} onCreated={handleCreated} />
    </PageLayout>
  );
}
