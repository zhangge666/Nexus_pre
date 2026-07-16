/** 本文件实现知识卡片管理页面，支持按状态/复习集筛选、卡片预览和创建。 */
import type React from "react";
import { useState, useEffect } from "react";
import { Layers, Plus, BookOpen } from "lucide-react";
import { listReviewCards } from "../core";
import type { ReviewCard, ReviewState } from "../core";
import { Topbar } from "../components/Topbar";
import { EmptyState } from "../components/EmptyState";
import { Modal } from "../components/Modal";
import { useInspector } from "../components/Inspector";
import { PageLayout } from "../components/PageLayout";

const STATE_CONFIG: Record<ReviewState, { label: string; color: string }> = {
  new:        { label: "新卡",   color: "muted" },
  learning:   { label: "学习中", color: "warning" },
  review:     { label: "待复习", color: "success" },
  relearning: { label: "重学中", color: "danger" },
};

const STATE_FILTERS: { key: ReviewState | "all"; label: string }[] = [
  { key: "all",       label: "全部" },
  { key: "new",       label: "新卡" },
  { key: "learning",  label: "学习中" },
  { key: "review",    label: "待复习" },
  { key: "relearning",label: "重学中" },
];

/** 渲染与主列表相同密度的卡片行，并在点击后打开全局检查器。 */
function CardItem({ card, onPreview }: { card: ReviewCard; onPreview: (card: ReviewCard) => void }): React.JSX.Element {
  const stateCfg = STATE_CONFIG[card.state];
  return (
    <div className="card-item" onClick={() => onPreview(card)} role="button" tabIndex={0}
      onKeyDown={(e) => e.key === "Enter" && onPreview(card)}>
      <div className="card-front-preview">{card.cardFront}</div>
      <div className="card-item-footer">
        {card.sourceTitle && (
          <span className="card-source">
            <BookOpen size={11} />{card.sourceTitle}
          </span>
        )}
        <span className={`card-state state-${stateCfg.color}`}>{stateCfg.label}</span>
        {card.reps > 0 && <span className="card-reps">{card.reps} 次</span>}
      </div>
    </div>
  );
}

/** 渲染卡片的正反面与复习元数据，供全局检查器承载。 */
function CardInspector({ card }: { card: ReviewCard }): React.JSX.Element {
  const state = STATE_CONFIG[card.state];
  return (
    <section className="card-inspector">
      <div className="detail-meta"><span className={`card-state state-${state.color}`}>{state.label}</span><span className="detail-time">复习 {card.reps} 次</span></div>
      <h3>正面</h3><p>{card.cardFront}</p>
      <h3>背面</h3><p>{card.cardBack}</p>
      <div className="card-inspector-meta"><span>稳定度 {card.stability.toFixed(1)}</span><span>难度 {card.difficulty.toFixed(1)}</span><span>遗忘 {card.lapses} 次</span></div>
    </section>
  );
}

/** 渲染卡片管理工作区，并将卡片预览交由全局检查器呈现。 */
export default function CardsPage(): React.JSX.Element {
  const { show } = useInspector();
  const [cards, setCards] = useState<ReviewCard[]>([]);
  const [stateFilter, setStateFilter] = useState<ReviewState | "all">("all");
  const [deckFilter, setDeckFilter] = useState<string>("all");
  const [loading, setLoading] = useState(true);
  const [preview, setPreview] = useState<ReviewCard | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [frontText, setFrontText] = useState("");
  const [backText, setBackText] = useState("");

  useEffect(() => {
    void listReviewCards().then((cs) => { setCards(cs); setLoading(false); });
  }, []);

  useEffect(() => {
    if (preview) show("卡片详情", <CardInspector card={preview} />);
  }, [preview, show]);

  const decks = Array.from(new Set(cards.map((c) => c.deck ?? "默认"))).sort();

  const filtered = cards.filter((c) => {
    if (stateFilter !== "all" && c.state !== stateFilter) return false;
    if (deckFilter !== "all" && (c.deck ?? "默认") !== deckFilter) return false;
    return true;
  });

  const byDeck = decks.reduce<Record<string, ReviewCard[]>>((acc, deck) => {
    acc[deck] = filtered.filter((c) => (c.deck ?? "默认") === deck);
    return acc;
  }, {});

  return (
    <PageLayout className="cards-page">
      <Topbar
        title="卡片"
        subtitle={`${cards.length} 张卡片 · ${decks.length} 个复习集`}
        actions={
          <button className="primary-small" onClick={() => setCreateOpen(true)}>
            <Plus size={14} />新建卡片
          </button>
        }
      />

      {/* 筛选栏 */}
      <div className="cards-filters page-toolbar">
        <div className="filter-row" role="group" aria-label="状态筛选">
          {STATE_FILTERS.map(({ key, label }) => (
            <button
              key={key}
              className={`filter-button${stateFilter === key ? " active" : ""}`}
              onClick={() => setStateFilter(key)}
            >
              {label}
            </button>
          ))}
        </div>
        <select
          className="collection-select deck-select"
          value={deckFilter}
          onChange={(e) => setDeckFilter(e.target.value)}
          aria-label="复习集筛选"
        >
          <option value="all">全部复习集</option>
          {decks.map((d) => <option key={d} value={d}>{d}</option>)}
        </select>
      </div>

      <div className="cards-content page-list-content">
        {loading && <p className="loading-hint">加载卡片中…</p>}

        {!loading && filtered.length === 0 && (
          <EmptyState
            icon={<Layers size={40} />}
            title="还没有知识卡片"
            description="从记忆中生成卡片或手动创建"
            action={{ label: "新建卡片", onClick: () => setCreateOpen(true) }}
          />
        )}

        {Object.entries(byDeck).filter(([, cs]) => cs.length > 0).map(([deck, deckCards]) => (
          <div key={deck} className="deck-group">
            <div className="deck-header">
              <h2 className="deck-name">{deck}</h2>
              <span className="deck-count">{deckCards.length} 张</span>
            </div>
            <div className="card-grid">
              {deckCards.map((card) => (
                <CardItem key={card.memoryId} card={card} onPreview={setPreview} />
              ))}
            </div>
          </div>
        ))}
      </div>

      {/* 创建卡片弹窗 */}
      <Modal
        open={createOpen}
        onClose={() => setCreateOpen(false)}
        title="新建知识卡片"
        footer={
          <div style={{ display: "flex", gap: 8 }}>
            <button className="secondary-button" onClick={() => setCreateOpen(false)}>取消</button>
            <button
              className="primary-small"
              disabled={!frontText.trim() || !backText.trim()}
              onClick={() => {
                // TODO: 调用 createCard API
                setCreateOpen(false);
                setFrontText("");
                setBackText("");
              }}
            >
              创建卡片
            </button>
          </div>
        }
      >
        <div className="create-card-form">
          <label className="form-label">正面（问题）</label>
          <textarea
            className="form-textarea"
            value={frontText}
            onChange={(e) => setFrontText(e.target.value)}
            placeholder="输入问题…"
            rows={3}
          />
          <label className="form-label" style={{ marginTop: 12 }}>背面（答案）</label>
          <textarea
            className="form-textarea"
            value={backText}
            onChange={(e) => setBackText(e.target.value)}
            placeholder="输入答案…"
            rows={3}
          />
        </div>
      </Modal>
    </PageLayout>
  );
}
