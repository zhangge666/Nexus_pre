/** 本文件呈现“今天”页最近保存的灵感与本机同步状态。 */

import React from "react";
import { ArrowRight, FileText } from "lucide-react";
import { formatCompactTime } from "../../core/format";
import type { MuseIdea } from "../../core/types";

interface TodayIdeaListProps {
  ideas: MuseIdea[];
  onOpenIdeas: () => void;
}

const ideaTags = ["产品", "效率", "功能"];

/** 渲染最近灵感，保持行高稳定并弱化同步元数据。 */
export function TodayIdeaList({ ideas, onOpenIdeas }: TodayIdeaListProps): React.JSX.Element {
  return (
    <section className="workspace-panel today-idea-panel">
      <header className="panel-heading">
        <div>
          <h2>最近灵感</h2>
          <span>{ideas.length}</span>
        </div>
        <button className="text-button" type="button" onClick={onOpenIdeas}>
          灵感箱 <ArrowRight size={12} aria-hidden="true" />
        </button>
      </header>
      <div className="linear-list">
        {ideas.slice(0, 3).map((idea, index) => (
          <button className="linear-row idea-overview-row" key={idea.id} type="button" onClick={onOpenIdeas}>
            <span className="row-icon"><FileText size={14} aria-hidden="true" /></span>
            <span className="row-primary">{idea.content}</span>
            <span className="tag">{ideaTags[index % ideaTags.length]}</span>
            <time>{formatCompactTime(idea.createdAt)}</time>
          </button>
        ))}
        {ideas.length === 0 ? <div className="panel-empty">记录的灵感会出现在这里。</div> : null}
      </div>
    </section>
  );
}
