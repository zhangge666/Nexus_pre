/** 本文件实现卡片翻转组件（FlashCard）和评分按钮组（RatingButtons）。 */
import type React from "react";
import { useState } from "react";
import { Eye } from "lucide-react";
import type { ReviewCard, Rating } from "../core";

// ---------------------------------------------------------------------------
// FlashCard
// ---------------------------------------------------------------------------

interface FlashCardProps {
  card: ReviewCard;
  flipped: boolean;
  onFlip: () => void;
}

function renderMarkdownSimple(text: string): React.JSX.Element {
  return (
    <div className="card-markdown">
      {text.split("\n").map((line, i) => {
        const html = line
          .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
          .replace(/`(.+?)`/g, "<code>$1</code>");
        if (line === "") return <br key={i} />;
        return <p key={i} dangerouslySetInnerHTML={{ __html: html }} />;
      })}
    </div>
  );
}

export function FlashCard({ card, flipped, onFlip }: FlashCardProps): React.JSX.Element {
  return (
    <div className="flash-card">
      <div className={`flash-card-inner${flipped ? " flipped" : ""}`}>
        {/* 正面 */}
        <div className="flash-card-front">
          <p className="card-face-label">问题</p>
          <div className="card-face-content">{renderMarkdownSimple(card.cardFront)}</div>
          {!flipped && (
            <button className="flip-button" onClick={onFlip}>
              <Eye size={14} />翻面查看答案
            </button>
          )}
          {card.sourceTitle && (
            <p className="card-source">📖 {card.sourceTitle}</p>
          )}
        </div>
        {/* 背面 */}
        <div className="flash-card-back">
          <p className="card-face-label answer-label">答案</p>
          <div className="card-face-content">{renderMarkdownSimple(card.cardBack)}</div>
          {card.sourceTitle && (
            <p className="card-source">📖 {card.sourceTitle}</p>
          )}
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// RatingButtons
// ---------------------------------------------------------------------------

const RATING_CONFIG: { key: Rating; label: string; interval: string; className: string }[] = [
  { key: "again", label: "Again", interval: "< 1 分钟", className: "again" },
  { key: "hard", label: "Hard", interval: "6 分钟", className: "hard" },
  { key: "good", label: "Good", interval: "13 天", className: "good" },
  { key: "easy", label: "Easy", interval: "32 天", className: "easy" },
];

interface RatingButtonsProps {
  onRate: (rating: Rating) => void;
  disabled?: boolean;
}

export function RatingButtons({ onRate, disabled }: RatingButtonsProps): React.JSX.Element {
  return (
    <div className="rating-buttons" role="group" aria-label="复习评分">
      {RATING_CONFIG.map(({ key, label, interval, className }) => (
        <button
          key={key}
          className={`rating-btn ${className}`}
          onClick={() => onRate(key)}
          disabled={disabled}
          aria-label={`${label}，下次复习：${interval}`}
        >
          <span className="rating-label">{label}</span>
          <span className="rating-interval">{interval}</span>
        </button>
      ))}
    </div>
  );
}
