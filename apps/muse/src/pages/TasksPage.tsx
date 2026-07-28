/** 本文件组合任务页面标题、创建入口、左侧列表和右侧任务详情。 */

import React, { FormEvent, useEffect, useState } from "react";
import { Plus } from "lucide-react";
import { PageHeader } from "../components/PageHeader";
import { TaskDetailPane } from "../features/tasks/TaskDetailPane";
import { TaskListPane } from "../features/tasks/TaskListPane";
import type { MuseTask, TaskStatus } from "../core/types";

interface TasksPageProps {
  tasks: MuseTask[];
  onAddTask: (title: string, source?: string) => MuseTask;
  onSetStatus: (id: string, status: TaskStatus) => void;
  onAddActivity: (taskId: string, detail: string) => void;
}

/** 呈现分栏任务工作区，并把列表与详情状态保持同步。 */
export function TasksPage({ tasks, onAddTask, onSetStatus, onAddActivity }: TasksPageProps): React.JSX.Element {
  const [selectedId, setSelectedId] = useState(tasks[0]?.id ?? "");
  const [showCreate, setShowCreate] = useState(false);
  const [newTitle, setNewTitle] = useState("");
  const [sourceText, setSourceText] = useState("");
  const selected = tasks.find((task) => task.id === selectedId) ?? tasks[0];
  const selectedIndex = selected ? tasks.findIndex((task) => task.id === selected.id) : 0;

  useEffect(() => {
    if (!selected && tasks[0]) setSelectedId(tasks[0].id);
  }, [selected, tasks]);

  /** 创建任务后直接打开对应详情，方便继续补充上下文。 */
  function handleCreate(event: FormEvent): void {
    event.preventDefault();
    if (!newTitle.trim()) return;
    const task = onAddTask(newTitle, sourceText);
    setSelectedId(task.id);
    setNewTitle("");
    setSourceText("");
    setShowCreate(false);
  }

  return (
    <div className="page page-tasks">
      <PageHeader
        eyebrow="工作区 / 任务"
        title="任务"
        description="要求、文件与进展始终保留在同一条时间线上"
        actions={(
          <button className="primary-button" type="button" onClick={() => setShowCreate((open) => !open)}>
            <Plus size={13} aria-hidden="true" /> 新任务
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
        <TaskListPane tasks={tasks} selectedId={selected?.id ?? ""} onSelect={setSelectedId} />
        {selected ? (
          <TaskDetailPane
            task={selected}
            taskIndex={selectedIndex}
            onSetStatus={onSetStatus}
            onAddActivity={onAddActivity}
          />
        ) : <div className="empty-state">创建第一个任务后即可开始工作留痕。</div>}
      </section>
    </div>
  );
}
