/** 本文件实现选中任务的标题、原始要求、进展时间线与更新输入。 */

import React, { FormEvent, useState } from "react";
import {
  Check,
  ChevronDown,
  FileText,
  Link2,
  MessageSquareText,
  Paperclip,
  PlayCircle,
} from "lucide-react";
import { formatCompactTime, taskStatusLabel } from "../../core/format";
import type { MuseTask, TaskStatus } from "../../core/types";
import { TaskContextPanel } from "./TaskContextPanel";
import { taskCode } from "./taskPresentation";

interface TaskDetailPaneProps {
  task: MuseTask;
  taskIndex: number;
  onSetStatus: (id: string, status: TaskStatus) => void;
  onAddActivity: (taskId: string, detail: string) => void;
}

/** 根据活动类型返回一致的时间线图标。 */
function ActivityIcon({ type }: { type: MuseTask["activities"][number]["type"] }): React.JSX.Element {
  if (type === "source") return <Link2 size={12} aria-hidden="true" />;
  if (type === "file") return <FileText size={12} aria-hidden="true" />;
  if (type === "status") return <Check size={12} aria-hidden="true" />;
  return <MessageSquareText size={12} aria-hidden="true" />;
}

/** 渲染任务主详情，并确保新增进展只追加、不覆盖既有留痕。 */
export function TaskDetailPane({
  task,
  taskIndex,
  onSetStatus,
  onAddActivity,
}: TaskDetailPaneProps): React.JSX.Element {
  const [activity, setActivity] = useState("");
  const sourceActivity = task.activities.find((item) => item.type === "source");

  /** 保存一条非空进展后清空输入。 */
  function handleActivity(event: FormEvent): void {
    event.preventDefault();
    if (!activity.trim()) return;
    onAddActivity(task.id, activity);
    setActivity("");
  }

  return (
    <article className="task-detail">
      <header className="task-detail-header">
        <div className="task-heading-copy">
          <span className="task-code">{taskCode(taskIndex)}</span>
          <h2>{task.title}</h2>
          <div className="task-heading-meta">
            <button className={`status-button is-${task.status}`} type="button">
              {taskStatusLabel(task.status)} <ChevronDown size={11} aria-hidden="true" />
            </button>
            <span><span className="priority-dot" /> 高优先级</span>
            <span>{task.dueLabel}</span>
            <span className="saved-label">已保存到本机</span>
          </div>
        </div>
        <button className="secondary-button focus-button" type="button">
          <PlayCircle size={13} aria-hidden="true" /> 开始专注
        </button>
      </header>

      <div className="task-detail-layout">
        <main className="task-detail-main">
          <section className="task-source">
            <header><h3>原始要求</h3><button className="text-button" type="button">查看来源 ↗</button></header>
            <p>{sourceActivity?.detail ?? (task.description || "还没有绑定来源。可在下方粘贴消息或拖入文件。")}</p>
            <div className="attachment-list">
              <button type="button"><FileText size={13} aria-hidden="true" /> 首页需求.md</button>
              <button type="button"><Paperclip size={13} aria-hidden="true" /> 品牌资产.fig</button>
            </div>
          </section>

          <section className="timeline-section">
            <header><h3>进展记录</h3><button className="text-button" type="button">仅看证据</button></header>
            <ol className="task-timeline">
              {task.activities.map((item) => (
                <li key={item.id}>
                  <span className={`timeline-icon is-${item.type}`}><ActivityIcon type={item.type} /></span>
                  <div>
                    <strong>{item.title}</strong>
                    <p>{item.detail}</p>
                  </div>
                  <time>{formatCompactTime(item.createdAt)}</time>
                </li>
              ))}
              {task.activities.length === 0 ? <li className="empty-timeline">任务建立后，来源和进展会依次显示在这里。</li> : null}
            </ol>
          </section>

          <form className="task-update" onSubmit={handleActivity}>
            <Paperclip size={14} aria-hidden="true" />
            <input
              value={activity}
              onChange={(event) => setActivity(event.target.value)}
              placeholder="记录新进展…"
              aria-label="记录新进展"
            />
            <button
              className="secondary-button"
              type="button"
              onClick={() => onSetStatus(task.id, task.status === "done" ? "doing" : "done")}
            >
              {task.status === "done" ? "重新打开" : "完成任务"}
            </button>
            <button className="primary-button" type="submit" disabled={!activity.trim()}>记录</button>
          </form>
        </main>
        <TaskContextPanel task={task} />
      </div>
    </article>
  );
}
