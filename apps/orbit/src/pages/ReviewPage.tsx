/** 本文件实现 Orbit 的复习工作区，并将 FSRS 队列、评分和统计收敛到紧凑的单一主任务界面。 */
import type React from "react";
import { useCallback, useEffect, useState } from "react";
import { Clock, CornerDownLeft, Eye, Flame, RefreshCw, ShieldCheck, Sparkles } from "lucide-react";
import { getReviewQueue, getReviewStats, gradeCard } from "../core";
import type { Rating, ReviewCard, ReviewStats } from "../core";
import { EmptyState } from "../components/EmptyState";
import { useInspector } from "../components/Inspector";
import { PageLayout } from "../components/PageLayout";
import { Topbar } from "../components/Topbar";
import { useNavigate } from "react-router-dom";

/** 在全局检查器中展示当前卡片的真实调度信息。 */
function ReviewInspector({ card, remaining, onNavigate }: {
  card: ReviewCard | undefined;
  remaining: number;
  onNavigate: (path: string) => void;
}): React.JSX.Element {
  if (!card) {
    return <div className="inspector-placeholder"><p>当前没有待复习卡片。</p></div>;
  }

  return (
    <div className="today-inspector">
      <section className="inspector-section">
        <div className="review-inspector-title">卡片详情</div>
        <div className="review-detail-grid">
          <div className="review-detail-row"><span>来源</span><button className="detail-action" onClick={() => onNavigate(`/search?id=${card.memoryId}`)}>{card.sourceTitle ?? "未关联来源"}</button></div>
          <div className="review-detail-row"><span>复习次数</span><strong>{card.reps}</strong></div>
          <div className="review-detail-row"><span>剩余卡片</span><strong>{remaining}</strong></div>
        </div>
      </section>
      <section className="inspector-section">
        <div className="review-inspector-title">FSRS 调度</div>
        <div className="review-detail-grid">
          <div className="review-detail-row"><span>稳定度</span><strong>{card.stability.toFixed(1)}</strong></div>
          <div className="review-detail-row"><span>难度</span><strong>{card.difficulty.toFixed(1)}</strong></div>
          <div className="review-detail-row"><span>遗忘次数</span><strong>{card.lapses}</strong></div>
        </div>
      </section>
    </div>
  );
}

/** 渲染到期复习队列，并在评分成功后推进到下一张卡片。 */
export default function ReviewPage(): React.JSX.Element {
  const navigate = useNavigate();
  const { present } = useInspector();
  const [queue, setQueue] = useState<ReviewCard[]>([]);
  const [stats, setStats] = useState<ReviewStats | null>(null);
  const [current, setCurrent] = useState(0);
  const [reviewed, setReviewed] = useState(0);
  const [flipped, setFlipped] = useState(false);
  const [loading, setLoading] = useState(true);
  const [grading, setGrading] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  /** 重新读取服务端的到期队列和统计，并重置本轮复习进度。 */
  const load = useCallback(async (): Promise<void> => {
    setLoading(true);
    setNotice(null);
    try {
      const [nextQueue, nextStats] = await Promise.all([getReviewQueue(), getReviewStats()]);
      setQueue(nextQueue);
      setStats(nextStats);
      setCurrent(0);
      setReviewed(0);
      setFlipped(false);
    } catch (error) {
      setNotice(`复习队列加载失败：${String(error)}`);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { void load(); }, [load]);

  const card = queue[current];
  const completed = !loading && queue.length > 0 && current >= queue.length;
  const total = queue.length;
  const progress = total === 0 ? 0 : Math.min((reviewed / total) * 100, 100);

  /** 将用户评分写入协议服务；失败时保留当前卡片供用户重试。 */
  const handleRate = useCallback(async (rating: Rating): Promise<void> => {
    const activeCard = queue[current];
    if (!activeCard || grading) return;

    setGrading(true);
    setNotice(null);
    try {
      const result = await gradeCard(activeCard.memoryId, rating);
      setReviewed((count) => count + 1);
      setCurrent((index) => index + 1);
      setFlipped(false);
      void getReviewStats().then(setStats).catch(() => undefined);
      const hours = Math.max(1, Math.round((result.nextDueAt - Date.now()) / 3_600_000));
      setNotice(`评分已保存，下次约 ${hours < 48 ? `${hours} 小时` : `${Math.round(hours / 24)} 天`}后复习。`);
    } catch (error) {
      setNotice(`评分失败：${String(error)}`);
    } finally {
      setGrading(false);
    }
  }, [current, grading, queue]);

  /** 支持空格翻面及 1 至 4 的评分快捷键，避免鼠标成为唯一入口。 */
  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent): void {
      if (loading || grading || !card) return;
      if (!flipped && (event.key === " " || event.key.toLowerCase() === "s")) {
        event.preventDefault();
        setFlipped(true);
        return;
      }
      if (!flipped) return;
      const ratings: Record<string, Rating> = { "1": "again", "2": "hard", "3": "good", "4": "easy" };
      if (ratings[event.key]) void handleRate(ratings[event.key]);
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [card, flipped, grading, handleRate, loading]);

  useEffect(() => {
    present("复习详情", <ReviewInspector card={card} remaining={Math.max(total - current - 1, 0)} onNavigate={navigate} />);
  }, [card, current, navigate, present, total]);

  const topbar = (
    <Topbar
      title="复习"
      subtitle={loading ? "正在读取今日到期队列" : `今日到期 ${stats?.dueToday ?? total} 张 · 本轮已完成 ${reviewed} 张`}
      actions={<button className="secondary-button" onClick={() => void load()} disabled={loading || grading}><RefreshCw size={14} />刷新队列</button>}
    />
  );

  if (loading) {
    return <PageLayout className="review-page">{topbar}<div className="review-loading">正在加载复习队列…</div></PageLayout>;
  }

  if (!card) {
    return (
      <PageLayout className="review-page">
        {topbar}
        <EmptyState
          icon={<Sparkles size={36} />}
          title={completed ? "今日复习已完成" : "暂无到期卡片"}
          description={completed ? `本轮已复习 ${reviewed} 张卡片，明天再来看看。` : "创建知识卡片后，Orbit 会在到期时将它们加入这里。"}
          action={{ label: "前往记忆", onClick: () => navigate("/search") }}
        />
        {notice && <p className="inline-notice" role={notice.includes("失败") ? "alert" : "status"}>{notice}</p>}
      </PageLayout>
    );
  }

  return (
    <PageLayout className="review-page">
      {topbar}
      <div className="review-workspace-content">
        {notice && <p className="inline-notice" role={notice.includes("失败") ? "alert" : "status"}>{notice}</p>}
        <section className="review-summary" aria-label="复习统计">
          <div><span>今日到期</span><strong>{stats?.dueToday ?? total}</strong></div>
          <div><span>本轮完成</span><strong>{reviewed} / {total}</strong></div>
          <div><span>成熟卡片</span><strong>{stats?.mature ?? 0}</strong></div>
          <div><span>今日已复习</span><strong>{stats?.reviewedToday ?? 0}</strong></div>
        </section>

        <section className="review-progress" aria-label="复习进度">
          <span>第 {current + 1} / {total} 张</span>
          <div className="review-progress-bar"><div className="review-progress-fill" style={{ width: `${progress}%` }} /></div>
          <span>{Math.round(progress)}%</span>
        </section>

        <section className="review-card" aria-live="polite">
          <div className="review-card-header"><span>{flipped ? "答案" : "问题"}</span>{card.deck && <span className="review-deck">{card.deck}</span>}</div>
          <p className="review-card-content">{flipped ? card.cardBack : card.cardFront}</p>
          <div className="review-card-footer">
            <span>来源：{card.sourceTitle ?? "未关联来源"}</span>
            {!flipped && <button className="primary-small" onClick={() => setFlipped(true)}><Eye size={14} />显示答案</button>}
          </div>
        </section>

        {flipped ? (
          <section className="review-rating-row" aria-label="为卡片评分">
            <button className="rating-action-card again" onClick={() => void handleRate("again")} disabled={grading}><span className="rating-action-label">重来</span><span className="rating-action-desc">重新学习 · 1</span></button>
            <button className="rating-action-card hard" onClick={() => void handleRate("hard")} disabled={grading}><span className="rating-action-label">困难</span><span className="rating-action-desc">较短间隔 · 2</span></button>
            <button className="rating-action-card good" onClick={() => void handleRate("good")} disabled={grading}><span className="rating-action-label">良好</span><span className="rating-action-desc">标准间隔 · 3</span></button>
            <button className="rating-action-card easy" onClick={() => void handleRate("easy")} disabled={grading}><span className="rating-action-label">简单</span><span className="rating-action-desc">较长间隔 · 4</span></button>
          </section>
        ) : (
          <div className="review-keyboard-hint"><CornerDownLeft size={13} />按空格键或 S 显示答案</div>
        )}

        {stats && <footer className="review-stats-bar"><span><Flame size={13} />连续复习 {stats.streak} 天</span><span>新卡 {stats.newToday} 张</span><span>年轻卡片 {stats.young} 张</span><span><ShieldCheck size={13} />FSRS 调度</span></footer>}
      </div>
    </PageLayout>
  );
}
