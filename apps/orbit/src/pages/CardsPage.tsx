/** 本文件实现知识卡片管理页面，支持按状态/复习集筛选、卡片预览和创建。 */
import type React from "react";
import { useState, useEffect } from "react";
import { Layers, Plus, BookOpen } from "lucide-react";
import { createCard, generateCards, listMemories, listReviewCards } from "../core";
import type { MemorySummary, ReviewCard, ReviewState } from "../core";
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
  const [deckText, setDeckText] = useState("默认");
  const [createMode, setCreateMode] = useState<"manual" | "ai">("manual");
  const [sources, setSources] = useState<MemorySummary[]>([]);
  const [sourceMemoryId, setSourceMemoryId] = useState("");
  const [instruction, setInstruction] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    void Promise.all([listReviewCards(), listMemories()])
      .then(([cs, memories]) => {
        setCards(cs);
        setSources(memories.filter((memory) => memory.kind !== "card"));
      })
      .catch((error) => setNotice(`卡片加载失败：${String(error)}`))
      .finally(() => setLoading(false));
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

  /** 创建手动卡片或从选定来源生成卡片；失败时保留全部输入。 */
  async function handleCreate(): Promise<void> {
    if (submitting) return;
    setSubmitting(true);
    setNotice(null);
    try {
      const created = createMode === "manual"
        ? [await createCard({
            cardFront: frontText,
            cardBack: backText,
            deck: deckText || null,
          })]
        : await generateCards({
            sourceMemoryId,
            instruction: instruction || null,
            deck: deckText || null,
            maxCards: 3,
          });
      setCards((current) => [...created, ...current]);
      setCreateOpen(false);
      setFrontText("");
      setBackText("");
      setInstruction("");
      setNotice(`已创建 ${created.length} 张卡片`);
    } catch (error) {
      setNotice(`创建失败：${String(error)}`);
    } finally {
      setSubmitting(false);
    }
  }

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
        {notice && <p className="inline-notice" role="status" aria-live="polite">{notice}</p>}
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
              disabled={submitting || (createMode === "manual"
                ? !frontText.trim() || !backText.trim()
                : !sourceMemoryId)}
              onClick={() => void handleCreate()}
            >
              {submitting ? "创建中…" : createMode === "manual" ? "创建卡片" : "生成卡片"}
            </button>
          </div>
        }
      >
        <div className="create-card-form">
          <div className="filter-row" role="group" aria-label="卡片创建方式">
            <button type="button" className={`filter-button${createMode === "manual" ? " active" : ""}`} onClick={() => setCreateMode("manual")}>手动创建</button>
            <button type="button" className={`filter-button${createMode === "ai" ? " active" : ""}`} onClick={() => setCreateMode("ai")}>从记忆生成</button>
          </div>
          {createMode === "manual" ? (
            <>
              <label className="form-label" htmlFor="card-front">正面（问题）</label>
              <textarea id="card-front" className="form-textarea" value={frontText}
                onChange={(e) => setFrontText(e.target.value)} placeholder="输入问题…" rows={3} />
              <label className="form-label" htmlFor="card-back">背面（答案）</label>
              <textarea id="card-back" className="form-textarea" value={backText}
                onChange={(e) => setBackText(e.target.value)} placeholder="输入答案…" rows={3} />
            </>
          ) : (
            <>
              <label className="form-label" htmlFor="card-source">来源记忆</label>
              <select id="card-source" className="settings-select" value={sourceMemoryId}
                onChange={(event) => setSourceMemoryId(event.target.value)}>
                <option value="">选择一条记忆…</option>
                {sources.map((memory) => <option key={memory.id} value={memory.id}>{memory.title ?? memory.content.slice(0, 42)}</option>)}
              </select>
              <label className="form-label" htmlFor="card-instruction">补充要求（可选）</label>
              <textarea id="card-instruction" className="form-textarea" value={instruction}
                onChange={(event) => setInstruction(event.target.value)} placeholder="例如：聚焦核心概念" rows={2} />
              <p className="form-help">只会把这条来源记忆的必要文本发送给当前 Completion Provider。</p>
            </>
          )}
          <label className="form-label" htmlFor="card-deck">复习集</label>
          <input id="card-deck" className="settings-input" value={deckText}
            onChange={(event) => setDeckText(event.target.value)} placeholder="默认" />
          {notice?.startsWith("创建失败") && <p className="form-error" role="alert">{notice}</p>}
        </div>
      </Modal>
    </PageLayout>
  );
}
