/** 本文件实现 Muse 打开后用于快速行动的“今天”页面。 */

import React, { useEffect, useRef } from "react";
import {
  ArrowRight,
  CheckCircle2,
  CheckSquare2,
  ClipboardCopy,
  Lightbulb,
  Mic2,
  PenLine,
} from "lucide-react";
import { IdeaComposer } from "../components/IdeaComposer";
import { PageHeader } from "../components/PageHeader";
import { formatCompactTime, taskStatusLabel } from "../core/format";
import { isMacOS } from "../core/platform";
import type { MuseClipboardItem, MuseIdea, MuseMeeting, MuseTask, MuseView } from "../core/types";

interface TodayPageProps {
  ideas: MuseIdea[];
  tasks: MuseTask[];
  meetings: MuseMeeting[];
  clipboard: MuseClipboardItem[];
  onAddIdea: (content: string) => Promise<void>;
  onNavigate: (view: MuseView) => void;
}

/** 呈现当前最相关的记录入口、任务和最近内容。 */
export function TodayPage({
  ideas,
  tasks,
  meetings,
  clipboard,
  onAddIdea,
  onNavigate,
}: TodayPageProps): React.JSX.Element {
  const captureRef = useRef<HTMLTextAreaElement>(null);
  const activeTasks = tasks.filter((task) => task.status !== "done").slice(0, 6);
  const modifier = isMacOS() ? "⌘" : "Ctrl";
  const quickCaptureShortcut = `${modifier} N`;
  const recentMeeting = meetings[0];
  const pinnedClipboardCount = clipboard.filter((item) => item.pinned).length;
  const monthAndDay = new Intl.DateTimeFormat("zh-CN", {
    month: "long",
    day: "numeric",
  }).format(new Date());
  const weekday = new Intl.DateTimeFormat("zh-CN", { weekday: "short" }).format(new Date());
  const todayLabel = `${monthAndDay} · ${weekday}`;

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
        eyebrow={todayLabel}
        title="今天"
        description="只看现在要做的事，其他的交给 Muse 记住。"
      />

      <nav className="today-launcher" aria-label="快速开始">
        <button className="is-primary" type="button" onClick={focusCapture}>
          <span className="launcher-icon"><PenLine size={15} aria-hidden="true" /></span>
          <span><strong>记下灵感</strong><small>接住刚刚的想法</small></span>
          <kbd>{quickCaptureShortcut}</kbd>
        </button>
        <button type="button" onClick={() => onNavigate("tasks")}>
          <span className="launcher-icon"><CheckSquare2 size={15} aria-hidden="true" /></span>
          <span><strong>新建任务</strong><small>绑定原始要求</small></span>
          <kbd>{modifier} ⇧ T</kbd>
        </button>
        <button type="button" onClick={() => onNavigate("meetings")}>
          <span className="launcher-icon"><Mic2 size={15} aria-hidden="true" /></span>
          <span><strong>开始会议</strong><small>记录与提取行动项</small></span>
          <kbd>{modifier} ⇧ R</kbd>
        </button>
        <button type="button" onClick={() => onNavigate("clipboard")}>
          <span className="launcher-icon"><ClipboardCopy size={15} aria-hidden="true" /></span>
          <span><strong>剪贴板</strong><small>{pinnedClipboardCount} 条已固定</small></span>
          <kbd>{modifier} ⇧ V</kbd>
        </button>
      </nav>

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
            {recentMeeting ? (
              <article>
                <small className="stream-time">{formatCompactTime(recentMeeting.recordedAt)}</small>
                <span className="stream-icon meeting"><Mic2 size={13} /></span>
                <div>
                  <p>{recentMeeting.title} · 已生成摘要与 {recentMeeting.actionItems.length} 个行动项</p>
                  <small>{recentMeeting.durationLabel} · 本机</small>
                </div>
              </article>
            ) : null}
          </div>
        </section>
      </div>

      <footer className="today-footer">
        <span><CheckCircle2 size={13} /> 本地工作区已保存</span>
        <span><Lightbulb size={13} /> {ideas.length} 条灵感</span>
        <span><ClipboardCopy size={13} /> {pinnedClipboardCount} 条剪贴板已固定</span>
      </footer>
    </div>
  );
}
