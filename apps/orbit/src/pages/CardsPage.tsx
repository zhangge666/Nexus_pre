/** 本文件实现知识卡片管理页面，支持按状态/复习集筛选、卡片预览和创建。 */
import type React from "react";
import { useState, useEffect } from "react";
import { Layers, Plus, BookOpen } from "lucide-react";
import { listReviewCards } from "../core";
import type { ReviewCard, ReviewState } from "../core";
import { Topbar } from "../components/Topbar";
import { EmptyState } from "../components/EmptyState";
import { Modal } from "../components/Modal";

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

function CardItem({ card, onPreview }: { card: ReviewCard; onPreview: (c: ReviewCard) => void }): React.JSX.Element {
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

export default function CardsPage(): React.JSX.Element {
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
    <div className="page-enter cards-page">
      <Topbar
        title="知识卡片"
        subtitle={`${cards.length} 张卡片 · ${decks.length} 个复习集`}
        actions={
          <button className="primary-small" onClick={() => setCreateOpen(true)}>
            <Plus size={14} />新建卡片
          </button>
        }
      />

      {/* 筛选栏 */}
      <div className="cards-filters">
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

      <div className="cards-content">
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

      {/* 卡片预览弹窗 */}
      <Modal
        open={preview !== null}
        onClose={() => setPreview(null)}
        title="卡片预览"
        footer={
          <button className="secondary-button" onClick={() => setPreview(null)}>关闭</button>
        }
      >
        {preview && (
          <div className="card-preview-modal">
            <div className="card-preview-section">
              <p className="card-preview-label">正面（问题）</p>
              <div className="card-preview-content">{preview.cardFront}</div>
            </div>
            <div className="card-preview-divider" />
            <div className="card-preview-section">
              <p className="card-preview-label answer-label">背面（答案）</p>
              <div className="card-preview-content">{preview.cardBack}</div>
            </div>
            {preview.sourceTitle && (
              <p className="card-preview-source">
                <BookOpen size={12} /> 来源：{preview.sourceTitle}
              </p>
            )}
            <div className="card-preview-stats">
              <span>状态：{STATE_CONFIG[preview.state].label}</span>
              <span>复习 {preview.reps} 次</span>
              <span>遗忘 {preview.lapses} 次</span>
            </div>
          </div>
        )}
      </Modal>

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
    </div>
  );
}
