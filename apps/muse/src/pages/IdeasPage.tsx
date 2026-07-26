/** 本文件实现 Muse 灵感的独立捕捉与本机回看页面。 */

import React, { useMemo, useState } from "react";
import { Check, Cloud, Lightbulb, Search } from "lucide-react";
import { IdeaComposer } from "../components/IdeaComposer";
import { PageHeader } from "../components/PageHeader";
import { formatCompactTime } from "../core/format";
import type { MuseIdea } from "../core/types";

interface IdeasPageProps {
  ideas: MuseIdea[];
  onAddIdea: (content: string) => Promise<void>;
}

/** 呈现独立灵感页，并支持在本地内容中即时筛选。 */
export function IdeasPage({ ideas, onAddIdea }: IdeasPageProps): React.JSX.Element {
  const [query, setQuery] = useState("");
  const filteredIdeas = useMemo(
    () => ideas.filter((idea) => idea.content.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())),
    [ideas, query],
  );

  return (
    <div className="page">
      <PageHeader
        eyebrow="灵感"
        title="想到就记"
        description="保存动作不依赖 Orbit；连接后可选择同步到统一记忆库。"
      />
      <IdeaComposer onSubmit={onAddIdea} />
      <section className="content-section idea-inbox">
        <header className="section-header">
          <div>
            <h2>灵感箱</h2>
            <span>{ideas.length} 条本机内容</span>
          </div>
          <label className="inline-search">
            <Search size={13} />
            <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索灵感" />
          </label>
        </header>
        <div className="idea-list">
          {filteredIdeas.map((idea) => (
            <article className="idea-row" key={idea.id}>
              <span className="idea-symbol"><Lightbulb size={14} /></span>
              <div>
                <p>{idea.content}</p>
                <small>{formatCompactTime(idea.createdAt)} · source: muse</small>
              </div>
              <span className={`sync-badge is-${idea.syncState}`}>
                {idea.syncState === "synced" ? <Cloud size={11} /> : <Check size={11} />}
                {idea.syncState === "synced" ? "已同步" : idea.syncState === "error" ? "同步失败" : "本机"}
              </span>
            </article>
          ))}
          {filteredIdeas.length === 0 ? <div className="empty-state">没有匹配的灵感，换个关键词试试。</div> : null}
        </div>
      </section>
    </div>
  );
}
