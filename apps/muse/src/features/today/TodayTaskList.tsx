/** 本文件呈现“今天”页的紧凑任务列表与稳定任务编号。 */

import React from "react";
import { ArrowRight } from "lucide-react";
import { taskStatusLabel } from "../../core/format";
import type { MuseTask } from "../../core/types";

interface TodayTaskListProps {
  tasks: MuseTask[];
  onOpenTasks: () => void;
}

/** 为本地任务生成只用于界面识别的紧凑编号。 */
function taskCode(index: number): string {
  return `MUS-${24 - index * 3}`;
}

/** 渲染今天仍需处理的任务，使用列表而不是独立卡片。 */
export function TodayTaskList({ tasks, onOpenTasks }: TodayTaskListProps): React.JSX.Element {
  return (
    <section className="workspace-panel today-task-panel">
      <header className="panel-heading">
        <div>
          <h2>我的任务</h2>
          <span>{tasks.length}</span>
        </div>
        <button className="text-button" type="button" onClick={onOpenTasks}>
          查看全部 <ArrowRight size={12} aria-hidden="true" />
        </button>
      </header>
      <div className="linear-list">
        {tasks.map((task, index) => (
          <button className="linear-row task-overview-row" key={task.id} type="button" onClick={onOpenTasks}>
            <span className={`task-ring is-${task.status}`} />
            <span className="row-primary">{task.title}</span>
            <code>{taskCode(index)}</code>
            <span className={`status-label is-${task.status}`}>{taskStatusLabel(task.status)}</span>
            <time>{task.dueLabel}</time>
          </button>
        ))}
        {tasks.length === 0 ? <div className="panel-empty">今天没有未完成任务。</div> : null}
      </div>
      <button className="inline-create" type="button" onClick={onOpenTasks}>＋ 添加任务</button>
    </section>
  );
}
