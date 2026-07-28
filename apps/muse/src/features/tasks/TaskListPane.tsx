/** 本文件实现任务工作区左侧的搜索、筛选与紧凑任务列表。 */

import React, { useMemo, useState } from "react";
import { Search } from "lucide-react";
import { isMacOS } from "../../core/platform";
import type { MuseTask, TaskStatus } from "../../core/types";
import { taskCode } from "./taskPresentation";

interface TaskListPaneProps {
  tasks: MuseTask[];
  selectedId: string;
  onSelect: (id: string) => void;
}

type TaskFilter = "active" | "waiting" | "done";

/** 判断任务是否属于当前筛选分组。 */
function matchesFilter(status: TaskStatus, filter: TaskFilter): boolean {
  if (filter === "active") return status === "todo" || status === "doing";
  return status === filter;
}

/** 渲染紧凑任务列表，并保持搜索和状态筛选均在本地即时完成。 */
export function TaskListPane({ tasks, selectedId, onSelect }: TaskListPaneProps): React.JSX.Element {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<TaskFilter>("active");
  const searchShortcut = isMacOS() ? "⌘ K" : "Ctrl K";
  const visibleTasks = useMemo(() => {
    const value = query.trim().toLocaleLowerCase();
    return tasks.filter((task) => {
      const searchable = `${task.title} ${task.requester} ${task.source} ${task.project}`.toLocaleLowerCase();
      return matchesFilter(task.status, filter) && (!value || searchable.includes(value));
    });
  }, [filter, query, tasks]);

  return (
    <aside className="task-master">
      <label className="task-search">
        <Search size={13} aria-hidden="true" />
        <input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="搜索任务和来源"
          aria-label="搜索任务和来源"
        />
        <kbd>{searchShortcut}</kbd>
      </label>
      <div className="task-filter" role="group" aria-label="任务状态筛选">
        <button className={filter === "active" ? "is-active" : ""} type="button" onClick={() => setFilter("active")}>
          进行中 <span>{tasks.filter((task) => matchesFilter(task.status, "active")).length}</span>
        </button>
        <button className={filter === "waiting" ? "is-active" : ""} type="button" onClick={() => setFilter("waiting")}>等待</button>
        <button className={filter === "done" ? "is-active" : ""} type="button" onClick={() => setFilter("done")}>完成</button>
      </div>
      <div className="task-list">
        {visibleTasks.map((task) => {
          const sourceIndex = tasks.findIndex((candidate) => candidate.id === task.id);
          return (
            <button
              className={`task-list-row ${task.id === selectedId ? "is-selected" : ""}`}
              key={task.id}
              type="button"
              onClick={() => onSelect(task.id)}
            >
              <span className={`task-ring is-${task.status}`} />
              <span className="task-list-copy">
                <strong>{task.title}</strong>
                <small>{task.requester} · {task.dueLabel}</small>
              </span>
              <code>{taskCode(sourceIndex)}</code>
            </button>
          );
        })}
        {visibleTasks.length === 0 ? <div className="panel-empty">当前筛选下没有任务。</div> : null}
      </div>
    </aside>
  );
}
