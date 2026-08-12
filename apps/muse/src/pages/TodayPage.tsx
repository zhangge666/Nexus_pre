/** 本文件实现 Muse 参考工作台风格的聚合时间线、快捷入口与任务检查器。 */

import React, { useMemo, useState } from "react";
import {
  ArrowUpRight,
  Check,
  CheckSquare2,
  ChevronRight,
  Circle,
  ClipboardList,
  FileText,
  Lightbulb,
  Link2,
  MessageSquareText,
  Mic2,
  MoreHorizontal,
  Paperclip,
  Send,
} from "lucide-react";
import { formatCompactTime, taskStatusLabel } from "../core/format";
import type { MuseClipboardItem, MuseIdea, MuseMeeting, MuseTask, MuseView } from "../core/types";

interface TodayPageProps {
  ideas: MuseIdea[];
  tasks: MuseTask[];
  meetings: MuseMeeting[];
  clipboard: MuseClipboardItem[];
  onAddIdea: (content: string) => Promise<void>;
  onAddTaskActivity: (taskId: string, detail: string) => void;
  onNavigate: (view: MuseView) => void;
}

const quickActions = [
  { id: "ideas" as const, label: "记录灵感", shortcut: "Ctrl ⇧ I", icon: Lightbulb },
  { id: "tasks" as const, label: "新建任务", shortcut: "Ctrl ⇧ T", icon: CheckSquare2 },
  { id: "meetings" as const, label: "会议记录", shortcut: "Ctrl ⇧ R", icon: Mic2 },
  { id: "clipboard" as const, label: "剪贴板推进", shortcut: "Ctrl ⇧ V", icon: ClipboardList },
];

/** 返回任务最晚一条活动时间，用于聚合工作流排序。 */
function taskTimestamp(task: MuseTask): number {
  return task.activities.at(-1)?.createdAt ?? Date.now() - 2 * 60 * 60_000;
}

/** 渲染聚合工作流，并在右侧展示当前任务的完整上下文。 */
export function TodayPage({
  ideas,
  tasks,
  meetings,
  clipboard,
  onAddIdea,
  onAddTaskActivity,
  onNavigate,
}: TodayPageProps): React.JSX.Element {
  const [selectedTaskId, setSelectedTaskId] = useState(tasks[0]?.id ?? "");
  const [comment, setComment] = useState("");
  const [quickIdea, setQuickIdea] = useState("");
  const selectedTask = tasks.find((task) => task.id === selectedTaskId) ?? tasks[0];
  const recentTasks = useMemo(
    () => [...tasks].sort((a, b) => taskTimestamp(b) - taskTimestamp(a)).slice(0, 5),
    [tasks],
  );

  /** 从工作流内快速保存灵感，保留键盘优先的轻量捕捉路径。 */
  async function saveQuickIdea(): Promise<void> {
    if (!quickIdea.trim()) return;
    await onAddIdea(quickIdea);
    setQuickIdea("");
  }

  /** 把检查器中的补充说明追加为任务活动并保留既有留痕。 */
  function saveTaskComment(): void {
    if (!selectedTask || !comment.trim()) return;
    onAddTaskActivity(selectedTask.id, comment);
    setComment("");
  }

  return (
    <div className="workflow-page">
      <section className="workflow-main">
        <header className="workspace-toolbar">
          <div>
            <h1>工作流</h1>
            <span>今天 · {tasks.filter((task) => task.status !== "done").length} 项进行中</span>
          </div>
          <button className="quiet-icon-button" type="button" aria-label="更多工作流操作">
            <MoreHorizontal size={15} />
          </button>
        </header>

        <div className="quick-action-strip">
          {quickActions.map(({ id, label, shortcut, icon: Icon }) => (
            <button key={id} type="button" onClick={() => onNavigate(id)}>
              <span><Icon size={14} aria-hidden="true" />{label}</span>
              <kbd>{shortcut}</kbd>
            </button>
          ))}
        </div>

        <div className="inline-capture">
          <Lightbulb size={14} aria-hidden="true" />
          <input
            value={quickIdea}
            onChange={(event) => setQuickIdea(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") void saveQuickIdea();
            }}
            placeholder="记下一闪而过的想法…"
            aria-label="快速记录灵感"
          />
          <button type="button" onClick={() => void saveQuickIdea()} disabled={!quickIdea.trim()} aria-label="保存灵感">
            <ArrowUpRight size={14} />
          </button>
        </div>

        <section className="timeline-section">
          <header className="timeline-heading">
            <span>今天</span>
            <small>{new Intl.DateTimeFormat("zh-CN", { month: "long", day: "numeric", weekday: "short" }).format(new Date())}</small>
          </header>
          <div className="workflow-list">
            {recentTasks.map((task) => (
              <button
                className={`workflow-row ${selectedTask?.id === task.id ? "is-selected" : ""}`}
                key={task.id}
                type="button"
                onClick={() => setSelectedTaskId(task.id)}
              >
                <span className={`row-glyph status-${task.status}`}><Circle size={10} /></span>
                <span className="workflow-copy">
                  <strong>{task.title}</strong>
                  <small>{task.activities.at(-1)?.detail || task.description}</small>
                </span>
                <span className="workflow-meta">
                  <small>{task.project}</small>
                  <time>{formatCompactTime(taskTimestamp(task))}</time>
                </span>
                <ChevronRight size={13} aria-hidden="true" />
              </button>
            ))}

            {ideas.slice(0, 2).map((idea) => (
              <button className="workflow-row" key={idea.id} type="button" onClick={() => onNavigate("ideas")}>
                <span className="row-glyph idea"><Lightbulb size={12} /></span>
                <span className="workflow-copy"><strong>收好一条灵感</strong><small>{idea.content}</small></span>
                <span className="workflow-meta"><small>灵感</small><time>{formatCompactTime(idea.createdAt)}</time></span>
                <ChevronRight size={13} aria-hidden="true" />
              </button>
            ))}

            {meetings.slice(0, 1).map((meeting) => (
              <button className="workflow-row" key={meeting.id} type="button" onClick={() => onNavigate("meetings")}>
                <span className="row-glyph meeting"><Mic2 size={12} /></span>
                <span className="workflow-copy"><strong>{meeting.title}已归档</strong><small>{meeting.summary}</small></span>
                <span className="workflow-meta"><small>{meeting.durationLabel}</small><time>{formatCompactTime(meeting.recordedAt)}</time></span>
                <ChevronRight size={13} aria-hidden="true" />
              </button>
            ))}

            {clipboard[0] ? (
              <button className="workflow-row" type="button" onClick={() => onNavigate("clipboard")}>
                <span className="row-glyph clipboard"><ClipboardList size={12} /></span>
                <span className="workflow-copy"><strong>剪贴板清单可继续推进</strong><small>{clipboard[0].title}</small></span>
                <span className="workflow-meta"><small>{clipboard[0].source}</small><time>{formatCompactTime(clipboard[0].copiedAt)}</time></span>
                <ChevronRight size={13} aria-hidden="true" />
              </button>
            ) : null}
          </div>
        </section>
      </section>

      <aside className="workflow-inspector">
        {selectedTask ? (
          <>
            <header className="inspector-header">
              <span>{selectedTask.project}</span>
              <div><button type="button" aria-label="复制任务链接"><Link2 size={14} /></button><button type="button" aria-label="更多任务操作"><MoreHorizontal size={15} /></button></div>
            </header>
            <div className="inspector-scroll">
              <div className="inspector-title">
                <span className={`task-state-dot status-${selectedTask.status}`} />
                <div><small>{taskStatusLabel(selectedTask.status)}</small><h2>{selectedTask.title}</h2></div>
              </div>
              <p className="inspector-description">{selectedTask.description || "还没有补充任务说明。"}</p>

              <dl className="inspector-meta">
                <div><dt>截止</dt><dd>{selectedTask.dueLabel}</dd></div>
                <div><dt>提出人</dt><dd>{selectedTask.requester}</dd></div>
                <div><dt>来源</dt><dd>{selectedTask.source}</dd></div>
                <div><dt>项目</dt><dd><span>{selectedTask.project}</span></dd></div>
              </dl>

              <section className="inspector-block">
                <header><strong>原始要求</strong><Paperclip size={13} /></header>
                <p>{selectedTask.activities.find((item) => item.type === "source")?.detail || "该任务没有绑定原始消息。"}</p>
              </section>

              <section className="inspector-block activity-block">
                <header><strong>活动</strong><span>{selectedTask.activities.length}</span></header>
                {selectedTask.activities.slice(-3).reverse().map((activity) => (
                  <article key={activity.id}>
                    <span className="activity-icon">{activity.type === "file" ? <FileText size={11} /> : activity.type === "status" ? <Check size={11} /> : <MessageSquareText size={11} />}</span>
                    <div><strong>{activity.title}</strong><p>{activity.detail}</p><small>{formatCompactTime(activity.createdAt)}</small></div>
                  </article>
                ))}
              </section>
            </div>
            <footer className="inspector-composer">
              <textarea value={comment} onChange={(event) => setComment(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter" && event.ctrlKey) saveTaskComment(); }} placeholder="记录下一步或补充进展…" aria-label="任务进展" />
              <div><span>仅保存在本机 · Ctrl Enter 保存</span><button type="button" disabled={!comment.trim()} onClick={saveTaskComment} aria-label="发送进展"><Send size={13} /></button></div>
            </footer>
          </>
        ) : <div className="empty-state">选择一项任务查看详情。</div>}
      </aside>
    </div>
  );
}
