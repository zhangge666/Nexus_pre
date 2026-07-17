/** 本文件实现 Orbit 今日复习队列页面，集成 FSRS 算法控制与多维度指标检查面板。 */
import React, { useState, useEffect, useCallback } from "react";
import {
  Flame, BookOpen, RotateCcw, Clock, ShieldCheck,
  HelpCircle, Eye, RefreshCw, BarChart2, Star, MoreHorizontal,
  ChevronRight, Calendar, Sparkles, CheckCircle, CornerDownLeft
} from "lucide-react";
import { getReviewQueue, getReviewStats, gradeCard } from "../core";
import type { ReviewCard, ReviewStats, Rating } from "../core";
import { Topbar } from "../components/Topbar";
import { EmptyState } from "../components/EmptyState";
import { useInspector } from "../components/Inspector";
import { PageLayout } from "../components/PageLayout";
import { useNavigate } from "react-router-dom";

/** 渲染当前复习卡片的关键调度数据，作为根级检查器内容展示。 */
function ReviewInspector({ card, remaining, onNavigate }: { card: ReviewCard | undefined; remaining: number; onNavigate: (path: string) => void }): React.JSX.Element {
  if (!card) return <div className="inspector-placeholder"><p>当前没有待复习卡片。</p></div>;
  return <div className="today-inspector"><section className="inspector-section"><div className="review-inspector-title">卡片详情</div><div className="review-detail-grid"><div className="review-detail-row"><span>来源</span><button className="detail-action" onClick={() => onNavigate(`/search?id=${card.memoryId}`)}>{card.sourceTitle || "未命名记忆"}</button></div><div className="review-detail-row"><span>复习次数</span><strong>{card.reps}</strong></div><div className="review-detail-row"><span>剩余卡片</span><strong>{remaining}</strong></div></div></section><section className="inspector-section"><div className="review-inspector-title">FSRS 调度</div><div className="review-detail-grid"><div className="review-detail-row"><span>稳定度</span><strong>{card.stability.toFixed(1)}</strong></div><div className="review-detail-row"><span>难度</span><strong>{card.difficulty.toFixed(1)}</strong></div><div className="review-detail-row"><span>遗忘次数</span><strong>{card.lapses}</strong></div></div></section></div>;
}

/** 渲染复习队列，并将当前卡片的辅助详情放入全局检查器。 */
export default function ReviewPage(): React.JSX.Element {
  const navigate = useNavigate();
  const { present } = useInspector();
  const [queue, setQueue] = useState<ReviewCard[]>([]);
  const [current, setCurrent] = useState(0);
  const [flipped, setFlipped] = useState(false);
  const [stats, setStats] = useState<ReviewStats | null>(null);
  const [reviewed, setReviewed] = useState(0);
  const [total, setTotal] = useState(0);
  const [grading, setGrading] = useState(false);
  const [loading, setLoading] = useState(true);
  const [done, setDone] = useState(false);
  const [shuffle, setShuffle] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  /** 重新加载复习队列与统计数据，并重置本轮复习状态。 */
  const load = useCallback(async () => {
    setLoading(true);
    setNotice(null);
    try {
      const [q, s] = await Promise.all([getReviewQueue(), getReviewStats()]);
      setQueue(q);
      setTotal(q.length);
      setStats(s);
      setDone(q.length === 0);
      setCurrent(0);
      setFlipped(false);
      setReviewed(0);
    } catch (error) {
      setNotice(`复习队列加载失败：${String(error)}`);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  // 仅在答案已经翻开时响应快捷键，避免误触直接提交评分。
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (done || loading || grading) return;
      if (!flipped && (e.key.toLowerCase() === "s" || e.key === " ")) {
        e.preventDefault();
        setFlipped(true);
        return;
      }
      if (!flipped) return;
      if (e.key === "1") void handleRate("again");
      if (e.key === "2") void handleRate("hard");
      if (e.key === "3") void handleRate("good");
      if (e.key === "4") void handleRate("easy");
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  });

  /** 提交当前卡片评分，并推进到队列中的下一张卡片。 */
  async function handleRate(rating: Rating): Promise<void> {
    const card = queue[current];
    if (!card || grading) return;
    setGrading(true);
    setNotice(null);
    try {
      const result = await gradeCard(card.memoryId, rating);
      const next = current + 1;
      setReviewed(prev => prev + 1);
      if (next >= queue.length) {
        setDone(true);
      } else {
        setCurrent(next);
        setFlipped(false);
      }
      // 评分写入成功后异步刷新统计，不阻塞下一张卡片的展示。
      void getReviewStats().then(setStats);
      const hours = Math.max(1, Math.round((result.nextDueAt - Date.now()) / 3_600_000));
      setNotice(`评分已保存，下次约 ${hours < 48 ? `${hours} 小时` : `${Math.round(hours / 24)} 天`}后复习`);
    } catch (error) {
      // 写入失败时保留当前卡片和翻面状态，方便用户直接重试。
      setNotice(`评分失败：${String(error)}`);
    } finally {
      setGrading(false);
    }
  }

  /** 随机重排尚未完成的复习队列，并回到新队列起点。 */
  const handleShuffle = (): void => {
    if (queue.length <= 1) return;
    const shuffled = [...queue].sort(() => Math.random() - 0.5);
    setQueue(shuffled);
    setCurrent(0);
    setFlipped(false);
    setShuffle(true);
    setTimeout(() => setShuffle(false), 500);
  };

  const card = queue[current];
  const progress = total > 0 ? (reviewed / total) * 100 : 0;

  useEffect(() => {
    if (card) present("复习详情", <ReviewInspector card={card} remaining={Math.max(queue.length - current - 1, 0)} onNavigate={navigate} />);
  }, [card, current, navigate, present, queue.length]);

  if (loading) {
    return (
      <PageLayout className="review-page">
        <Topbar title="Review Queue" subtitle="Strengthen your memory through spaced repetition." />
        <div className="review-loading" style={{ padding: "40px", color: "hsl(var(--foreground-muted))" }}>Loading review queue…</div>
      </PageLayout>
    );
  }

  if (done) {
    return (
      <PageLayout className="review-page">
        <Topbar title="Review Queue" subtitle="Strengthen your memory through spaced repetition." />
        <div style={{ marginTop: "40px" }}>
          <EmptyState
            icon={<Sparkles size={48} className="primary-color" />}
            title="🎉 All caught up for today!"
            description={`Fantastic! You've reviewed all ${reviewed} cards in your queue. Come back tomorrow!`}
            action={{ label: "Go to Search Page", onClick: () => navigate("/search") }}
          />
        </div>
      </PageLayout>
    );
  }

  return (
    <PageLayout className="review-workspace">
      <Topbar
        title="Review Queue"
        subtitle="Strengthen your memory through spaced repetition."
        actions={
          <button className="secondary-button" onClick={() => void load()}>
            <RotateCcw size={14} />Refresh Queue
          </button>
        }
      />

        {notice && <p className="inline-notice" role={notice.includes("失败") ? "alert" : "status"}>{notice}</p>}

        {/* 1. 统计卡片行 */}
        <section className="stats-cards-grid" aria-label="复习统计">
          <div className="dashboard-stat-card">
            <div className="stat-card-header">
              <span>DUE TODAY</span>
              <Clock size={12} />
            </div>
            <div className="stat-card-value">
              {stats?.dueToday ?? 0}<span>cards</span>
            </div>
            <div className="stat-card-footer success">
              +{stats?.newToday ?? 0} new
            </div>
          </div>

          <div className="dashboard-stat-card">
            <div className="stat-card-header">
              <span>COMPLETED</span>
              <CheckCircle size={12} style={{ color: "hsl(var(--success))" }} />
            </div>
            <div className="stat-card-value">
              {reviewed}<span>/ {total}</span>
            </div>
            <div className="stat-card-footer success" style={{ display: "flex", alignItems: "center", gap: "6px" }}>
              <div style={{ background: "hsl(var(--muted))", height: "4px", width: "60px", borderRadius: "2px", overflow: "hidden" }}>
                <div style={{ background: "hsl(var(--success))", height: "100%", width: `${progress}%` }} />
              </div>
              <span>{Math.round(progress)}%</span>
            </div>
          </div>

          <div className="dashboard-stat-card">
            <div className="stat-card-header">
              <span>MATURE</span>
              <BarChart2 size={12} />
            </div>
            <div className="stat-card-value">
              {stats?.mature ?? 0}<span>cards</span>
            </div>
            <div className="stat-card-footer success" style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <span>稳定度 ≥ 21 天</span>
            </div>
          </div>

          <div className="dashboard-stat-card">
            <div className="stat-card-header">
              <span>REVIEWED TODAY</span>
              <Clock size={12} />
            </div>
            <div className="stat-card-value">
              {stats?.reviewedToday ?? 0}<span>times</span>
            </div>
            <div className="stat-card-footer muted">
              已写入评分日志
            </div>
          </div>
        </section>

        {/* 2. 进度条与随机排序 */}
        <section style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "16px", marginTop: "12px" }}>
          <div style={{ flex: 1, display: "flex", alignItems: "center", gap: "12px", fontSize: "12px", color: "hsl(var(--foreground-muted))" }}>
            <span>Card <strong>{current + 1}</strong> of {total}</span>
            <div style={{ flex: 1, background: "hsl(var(--muted))", height: "6px", borderRadius: "3px", overflow: "hidden" }}>
              <div style={{ background: "hsl(var(--primary))", height: "100%", width: `${((current + 1) / total) * 100}%`, transition: "width 200ms ease" }} />
            </div>
            <span>{Math.round(((current + 1) / total) * 100)}% complete</span>
          </div>
          <button className="secondary-button" onClick={handleShuffle} disabled={shuffle}>
            <RefreshCw size={12} className={shuffle ? "spin" : ""} style={{ marginRight: "4px" }} />Shuffle
          </button>
        </section>

        {/* 3. 复习主卡片 */}
        <section style={{ flex: 1, display: "flex", flexDirection: "column" }}>
          {card && (
            <div className="review-card-container" style={{
              flex: 1,
              background: "hsl(var(--surface-elevated))",
              border: "1px solid hsl(var(--border-strong))",
              borderRadius: "var(--radius-lg)",
              boxShadow: "0 10px 30px -10px rgba(0, 0, 0, 0.5), inset 0 1px 0 0 rgba(255, 255, 255, 0.05)",
              padding: "40px",
              display: "flex",
              flexDirection: "column",
              position: "relative",
              justifyContent: "space-between",
              minHeight: "260px"
            }}>
              {/* 卡片头部 */}
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <span className="scope-badge" style={{ background: "hsl(var(--muted))", color: "hsl(var(--foreground-secondary))", textTransform: "uppercase", fontSize: "10px", padding: "3px 8px", borderRadius: "4px", fontWeight: 600 }}>
                  Question
                </span>
                <div style={{ display: "flex", gap: "6px", color: "hsl(var(--foreground-muted))" }}>
                  <button className="icon-button" aria-label="收藏卡片"><Star size={14} /></button>
                  <button className="icon-button" aria-label="更多卡片操作"><MoreHorizontal size={14} /></button>
                </div>
              </div>

              {/* 卡片正文 */}
              <div style={{ display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", flex: 1, padding: "20px 0" }}>
                <p style={{
                  fontSize: "20px",
                  fontWeight: 500,
                  textAlign: "center",
                  lineHeight: 1.5,
                  maxWidth: "520px",
                  color: "hsl(var(--foreground))"
                }}>
                  {flipped ? card.cardBack : card.cardFront}
                </p>
              </div>

              {/* 卡片底部 */}
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", borderTop: "1px solid hsl(var(--border-subtle))", paddingTop: "16px", marginTop: "16px" }}>
                <div style={{ display: "flex", alignItems: "center", gap: "8px", fontSize: "11px", color: "hsl(var(--foreground-muted))" }}>
                  <span className="source-mark src-quill" style={{ width: "16px", height: "16px", fontSize: "9px" }}>📝</span>
                  <span>Derived from Quill note</span>
                  <span style={{ color: "hsl(var(--foreground-secondary))", fontWeight: 500 }}>{card.sourceTitle || "Design Principles"}</span>
                </div>
                {!flipped && (
                  <button className="primary-small" onClick={() => setFlipped(true)} style={{ gap: "6px" }}>
                    <Eye size={13} />Show answer
                  </button>
                )}
              </div>
            </div>
          )}
        </section>

        {/* 4. 评分按钮 */}
        <section style={{ height: "72px", display: "flex", flexDirection: "column", gap: "8px" }}>
          {flipped ? (
            <div className="review-rating-row">
              <button className="rating-action-card again" onClick={() => void handleRate("again")} disabled={grading}>
                <span className="rating-action-label">Again</span>
                <span className="rating-action-desc">重新学习</span>
              </button>
              <button className="rating-action-card hard" onClick={() => void handleRate("hard")} disabled={grading}>
                <span className="rating-action-label">Hard</span>
                <span className="rating-action-desc">较短间隔</span>
              </button>
              <button className="rating-action-card good" onClick={() => void handleRate("good")} disabled={grading}>
                <span className="rating-action-label">Good</span>
                <span className="rating-action-desc">标准间隔</span>
              </button>
              <button className="rating-action-card easy" onClick={() => void handleRate("easy")} disabled={grading}>
                <span className="rating-action-label">Easy</span>
                <span className="rating-action-desc">较长间隔</span>
              </button>
            </div>
          ) : (
            <div style={{ display: "flex", justifyContent: "center", alignItems: "center", height: "100%", border: "1px dashed hsl(var(--border))", borderRadius: "var(--radius-md)", fontSize: "11px", color: "hsl(var(--foreground-disabled))", gap: "6px" }}>
              <CornerDownLeft size={12} /> Press Spacebar to reveal the answer
            </div>
          )}
          {flipped && (
            <div style={{ display: "flex", justifyContent: "center", fontSize: "10px", color: "hsl(var(--foreground-disabled))", gap: "12px" }}>
              <span>Tip: Use 1-4 keys or click</span>
            </div>
          )}
        </section>

        {/* 5. 底部统计条 */}
        {stats && (
          <section style={{ display: "flex", justifyContent: "space-between", fontSize: "11px", color: "hsl(var(--foreground-muted))", borderTop: "1px solid hsl(var(--border-subtle))", paddingTop: "12px" }}>
            <span style={{ display: "inline-flex", alignItems: "center", gap: "4px" }}><Flame size={12} className="warning-color" /> Streak: {stats.streak} days</span>
            <span>Young cards: {stats.young}</span>
            <span>Total cards: {stats.totalCards}</span>
            <span>Reviewed today: {stats.reviewedToday}</span>
            <span style={{ display: "inline-flex", alignItems: "center", gap: "4px" }}><ShieldCheck size={12} className="success-color" /> FSRS Algorithm</span>
          </section>
        )}
    </PageLayout>
  );
}
