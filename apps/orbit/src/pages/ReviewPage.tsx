/** 本文件实现今日复习页面，包含卡片翻转、FSRS 评分和复习统计。 */
import type React from "react";
import { useState, useEffect, useCallback } from "react";
import { PartyPopper, Flame, RotateCcw, BookOpen } from "lucide-react";
import { getReviewQueue, getReviewStats, gradeCard } from "../core";
import type { ReviewCard, ReviewStats, Rating } from "../core";
import { Topbar } from "../components/Topbar";
import { FlashCard, RatingButtons } from "../components/FlashCard";
import { EmptyState } from "../components/EmptyState";

export default function ReviewPage(): React.JSX.Element {
  const [queue, setQueue] = useState<ReviewCard[]>([]);
  const [current, setCurrent] = useState(0);
  const [flipped, setFlipped] = useState(false);
  const [stats, setStats] = useState<ReviewStats | null>(null);
  const [reviewed, setReviewed] = useState(0);
  const [total, setTotal] = useState(0);
  const [grading, setGrading] = useState(false);
  const [loading, setLoading] = useState(true);
  const [done, setDone] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [q, s] = await Promise.all([getReviewQueue(), getReviewStats()]);
      setQueue(q);
      setTotal(q.length);
      setStats(s);
      setDone(q.length === 0);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { void load(); }, [load]);

  async function handleRate(rating: Rating): Promise<void> {
    const card = queue[current];
    if (!card || grading) return;
    setGrading(true);
    try {
      await gradeCard(card.memoryId, rating);
      const next = current + 1;
      setReviewed((r) => r + 1);
      if (next >= queue.length) {
        setDone(true);
      } else {
        setCurrent(next);
        setFlipped(false);
      }
    } finally {
      setGrading(false);
    }
  }

  const card = queue[current];
  const progress = total > 0 ? (reviewed / total) * 100 : 0;

  if (loading) {
    return (
      <div className="page-enter review-page">
        <Topbar title="今日复习" />
        <div className="review-loading">加载复习队列…</div>
      </div>
    );
  }

  if (done) {
    return (
      <div className="page-enter review-page">
        <Topbar title="今日复习" />
        <div className="review-done-wrap">
          <EmptyState
            icon={<PartyPopper size={48} />}
            title="🎉 今日复习完成！"
            description={`太棒了，今天复习了 ${reviewed} 张卡片。明天再来！`}
            action={{ label: "返回检索", onClick: () => history.back() }}
          />
          {stats && (
            <div className="review-stats-grid">
              <div className="stat-card">
                <Flame size={20} />
                <span className="stat-value">{stats.streak}</span>
                <span className="stat-label">连续天数</span>
              </div>
              <div className="stat-card">
                <BookOpen size={20} />
                <span className="stat-value">{stats.totalCards}</span>
                <span className="stat-label">总卡片数</span>
              </div>
            </div>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="page-enter review-page">
      <Topbar
        title="今日复习"
        subtitle={`${reviewed} / ${total} 已完成`}
        actions={
          <button className="secondary-button" onClick={() => void load()}>
            <RotateCcw size={14} />刷新队列
          </button>
        }
      />

      {/* 进度条 */}
      <div className="review-progress-wrap">
        <div className="review-progress-bar">
          <div className="review-progress-fill" style={{ width: `${progress}%` }} />
        </div>
        <span className="review-progress-label">剩余 {total - reviewed} 张</span>
      </div>

      {/* 卡片区 */}
      <div className="review-card-area">
        {card && (
          <FlashCard
            card={card}
            flipped={flipped}
            onFlip={() => setFlipped(true)}
          />
        )}
      </div>

      {/* 评分按钮 */}
      {flipped && (
        <div className="review-rating-area">
          <RatingButtons onRate={(r) => void handleRate(r)} disabled={grading} />
        </div>
      )}

      {/* 底部统计 */}
      {stats && (
        <div className="review-stats-bar">
          <span><Flame size={13} /> 连续 {stats.streak} 天</span>
          <span>今日新卡 {stats.newToday}</span>
          <span>已复习 {reviewed}</span>
          <span>遗忘 {queue.slice(0, current).filter((c) => c.lapses > 0).length}</span>
        </div>
      )}
    </div>
  );
}
