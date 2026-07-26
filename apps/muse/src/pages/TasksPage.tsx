/** 本文件实现 Muse 任务列表、来源证据与不可覆盖的工作时间线。 */

import React, { FormEvent, useEffect, useState } from "react";
import {
  Check,
  ChevronDown,
  FileText,
  Link2,
  MessageSquareText,
  Paperclip,
  Plus,
  Search,
} from "lucide-react";
import { PageHeader } from "../components/PageHeader";
import { formatCompactTime, taskStatusLabel } from "../core/format";
import type { MuseTask, TaskStatus } from "../core/types";

interface TasksPageProps {
  tasks: MuseTask[];
  onAddTask: (title: string, source?: string) => MuseTask;
  onSetStatus: (id: string, status: TaskStatus) => void;
  onAddActivity: (taskId: string, detail: string) => void;
}

/** 把活动类型映射为时间线图标。 */
function ActivityIcon({ type }: { type: MuseTask["activities"][number]["type"] }): React.JSX.Element {
  if (type === "source") return <Link2 size={12} />;
  if (type === "file") return <FileText size={12} />;
  if (type === "status") return <Check size={12} />;
  return <MessageSquareText size={12} />;
}

/** 呈现可操作的任务留痕页面。 */
export function TasksPage({ tasks, onAddTask, onSetStatus, onAddActivity }: TasksPageProps): React.JSX.Element {
  const [selectedId, setSelectedId] = useState(tasks[0]?.id ?? "");
  const [showCreate, setShowCreate] = useState(false);
  const [newTitle, setNewTitle] = useState("");
  const [sourceText, setSourceText] = useState("");
  const [activity, setActivity] = useState("");
  const selected = tasks.find((task) => task.id === selectedId) ?? tasks[0];

  useEffect(() => {
    if (!selected && tasks[0]) setSelectedId(tasks[0].id);
  }, [selected, tasks]);

  /** 创建任务并直接选中新记录。 */
  function handleCreate(event: FormEvent): void {
    event.preventDefault();
    if (!newTitle.trim()) return;
    const task = onAddTask(newTitle, sourceText);
    setSelectedId(task.id);
    setNewTitle("");
    setSourceText("");
    setShowCreate(false);
  }

  /** 追加一条任务进展并清空输入。 */
  function handleActivity(event: FormEvent): void {
    event.preventDefault();
    if (!selected || !activity.trim()) return;
    onAddActivity(selected.id, activity);
    setActivity("");
  }

  return (
    <div className="page page-tasks">
      <PageHeader
        eyebrow="任务"
        title="任务与工作留痕"
        description="把原始要求、文件、过程和交付结果绑定在同一条时间线上。"
        actions={(
          <button className="primary-button" type="button" onClick={() => setShowCreate((value) => !value)}>
            <Plus size={13} /> 新任务
          </button>
        )}
      />

      {showCreate ? (
        <form className="task-create-bar" onSubmit={handleCreate}>
          <input value={newTitle} onChange={(event) => setNewTitle(event.target.value)} placeholder="任务标题" autoFocus />
          <input value={sourceText} onChange={(event) => setSourceText(event.target.value)} placeholder="粘贴原始要求（可选）" />
          <button className="secondary-button" type="button" onClick={() => setShowCreate(false)}>取消</button>
          <button className="primary-button" type="submit" disabled={!newTitle.trim()}>建立任务</button>
        </form>
      ) : null}

      <section className="task-workspace">
        <aside className="task-master">
          <label className="task-search">
            <Search size={13} />
            <input placeholder="搜索任务和来源" />
            <kbd>Ctrl K</kbd>
          </label>
          <div className="task-filter">
            <button className="is-active" type="button">进行中 <span>{tasks.filter((task) => task.status !== "done").length}</span></button>
            <button type="button">等待</button>
            <button type="button">完成</button>
          </div>
          <div className="task-list">
            {tasks.map((task) => (
              <button
                className={`task-list-row ${task.id === selected?.id ? "is-selected" : ""}`}
                key={task.id}
                type="button"
                onClick={() => setSelectedId(task.id)}
              >
                <span className={`task-ring is-${task.status}`} />
                <span>
                  <strong>{task.title}</strong>
                  <small>{task.requester} · {task.dueLabel}</small>
                </span>
                <em>{task.activities.length}</em>
              </button>
            ))}
          </div>
        </aside>

        {selected ? (
          <article className="task-detail">
            <header className="task-detail-header">
              <div>
                <div className="task-detail-meta">
                  <button className={`status-button is-${selected.status}`} type="button">
                    {taskStatusLabel(selected.status)} <ChevronDown size={11} />
                  </button>
                  <span>{selected.project}</span>
                  <span>{selected.dueLabel}</span>
                </div>
                <h2>{selected.title}</h2>
                <p>{selected.description || "尚未补充任务说明。"}</p>
              </div>
              <div className="requester-avatar" title={`提出人：${selected.requester}`}>{selected.requester.slice(0, 1)}</div>
            </header>

            <section className="task-source">
              <header><span>原始要求</span><button type="button">查看来源 ↗</button></header>
              <blockquote>
                {selected.activities.find((item) => item.type === "source")?.detail ?? "还没有绑定来源。可在下方粘贴消息或拖入文件。"}
              </blockquote>
              <footer>
                <span className="source-mark">{selected.source.slice(0, 1)}</span>
                <span>{selected.requester} · {selected.source}</span>
                <span>内容保存在本机</span>
              </footer>
            </section>

            <section className="timeline-section">
              <header><span>工作时间线</span><button type="button">仅看证据</button></header>
              <ol className="task-timeline">
                {selected.activities.map((item) => (
                  <li key={item.id}>
                    <span className={`timeline-icon is-${item.type}`}><ActivityIcon type={item.type} /></span>
                    <div>
                      <strong>{item.title}</strong>
                      <p>{item.detail}</p>
                    </div>
                    <time>{formatCompactTime(item.createdAt)}</time>
                  </li>
                ))}
                {selected.activities.length === 0 ? <li className="empty-timeline">任务建立后，来源和进展会依次显示在这里。</li> : null}
              </ol>
            </section>

            <form className="task-update" onSubmit={handleActivity}>
              <button type="button" aria-label="添加附件"><Paperclip size={14} /></button>
              <input value={activity} onChange={(event) => setActivity(event.target.value)} placeholder="记录进展、粘贴消息或拖入文件…" />
              <button
                className="secondary-button"
                type="button"
                onClick={() => onSetStatus(selected.id, selected.status === "done" ? "doing" : "done")}
              >
                {selected.status === "done" ? "重新打开" : "完成任务"}
              </button>
              <button className="primary-button" type="submit" disabled={!activity.trim()}>记录</button>
            </form>
          </article>
        ) : <div className="empty-state">创建第一个任务后即可开始工作留痕。</div>}
      </section>
    </div>
  );
}
