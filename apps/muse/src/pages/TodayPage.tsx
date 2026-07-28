/** 本文件实现 Muse 打开后用于快速行动的“今天”页面。 */

import React, { useEffect, useRef } from "react";
import { ArrowRight, CheckCircle2, Clock3, Lightbulb, Mic2, PenLine } from "lucide-react";
import { IdeaComposer } from "../components/IdeaComposer";
import { PageHeader } from "../components/PageHeader";
import { formatCompactTime, taskStatusLabel } from "../core/format";
import { isMacOS } from "../core/platform";
import type { MuseIdea, MuseTask, MuseView } from "../core/types";

interface TodayPageProps {
  ideas: MuseIdea[];
  tasks: MuseTask[];
  onAddIdea: (content: string) => Promise<void>;
  onNavigate: (view: MuseView) => void;
}

/** 呈现当前最相关的记录入口、任务和最近内容。 */
export function TodayPage({ ideas, tasks, onAddIdea, onNavigate }: TodayPageProps): React.JSX.Element {
  const captureRef = useRef<HTMLTextAreaElement>(null);
  const activeTasks = tasks.filter((task) => task.status !== "done").slice(0, 6);
  const quickCaptureShortcut = isMacOS() ? "⌘ N" : "Ctrl N";

  /** 将页面级快速记录入口与紧凑输入框保持在同一个焦点路径中。 */
  function focusCapture(): void {
    captureRef.current?.focus();
  }

  /** 在主窗口内提供参考界面所展示的快速记录快捷键。 */
  useEffect(() => {
    function handleQuickCapture(event: KeyboardEvent): void {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "n") {
        event.preventDefault();
        focusCapture();
      }
    }

    window.addEventListener("keydown", handleQuickCapture);
    return () => window.removeEventListener("keydown", handleQuickCapture);
  }, []);

  return (
    <div className="page page-today">
      <PageHeader
        title="今天"
        description="记录灵感、继续任务或开始一场会议。内容默认只保存在本机。"
        actions={(
          <button className="primary-button today-quick-action" type="button" onClick={focusCapture}>
            <PenLine size={14} aria-hidden="true" />
            记下灵感
            <kbd>{quickCaptureShortcut}</kbd>
          </button>
        )}
      />

      <section className="today-capture">
        <IdeaComposer compact inputRef={captureRef} onSubmit={onAddIdea} />
      </section>

      <div className="today-grid">
        <section className="content-section">
          <header className="section-header">
            <div>
              <h2>正在处理</h2>
              <span>{activeTasks.length} 个任务</span>
            </div>
            <button type="button" onClick={() => onNavigate("tasks")}>查看全部 <ArrowRight size={12} /></button>
          </header>
          <div className="dense-list">
            {activeTasks.map((task) => (
              <button className="dense-row" key={task.id} type="button" onClick={() => onNavigate("tasks")}>
                <span className={`task-ring is-${task.status}`} />
                <span className="row-copy">
                  <strong>{task.title}</strong>
                  <small>{task.requester} · {task.source}</small>
                </span>
                <span className="row-meta">
                  <em>{taskStatusLabel(task.status)}</em>
                  <small>{task.dueLabel}</small>
                </span>
              </button>
            ))}
          </div>
        </section>

        <section className="content-section">
          <header className="section-header">
            <div>
              <h2>最近收好</h2>
              <span>灵感与会议</span>
            </div>
            <button type="button" onClick={() => onNavigate("ideas")}>灵感箱 <ArrowRight size={12} /></button>
          </header>
          <div className="recent-stream">
            {ideas.slice(0, 3).map((idea) => (
              <article key={idea.id}>
                <small className="stream-time">{formatCompactTime(idea.createdAt)}</small>
                <span className="stream-icon"><Lightbulb size={13} /></span>
                <div>
                  <p>{idea.content}</p>
                  <small>{formatCompactTime(idea.createdAt)} · {idea.syncState === "synced" ? "已同步" : "本机"}</small>
                </div>
              </article>
            ))}
            <article>
              <small className="stream-time">昨天</small>
              <span className="stream-icon meeting"><Mic2 size={13} /></span>
              <div>
                <p>产品周会 · 已生成摘要与 2 个行动项</p>
                <small>昨天 · 32:18</small>
              </div>
            </article>
          </div>
        </section>
      </div>

      <footer className="today-footer">
        <span><CheckCircle2 size={13} /> 本地工作区已保存</span>
        <span><Clock3 size={13} /> 下次提醒：今天 18:00</span>
      </footer>
    </div>
  );
}
