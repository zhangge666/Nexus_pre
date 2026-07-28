/** 本文件呈现任务右侧的元数据、关联会议、摘要与行动项。 */

import React, { useState } from "react";
import { CalendarDays, CheckSquare2, ChevronRight, Target, Tag } from "lucide-react";
import { taskStatusLabel } from "../../core/format";
import type { MuseTask } from "../../core/types";

interface TaskContextPanelProps {
  task: MuseTask;
}

const defaultActions = ["输出首屏 2 版视觉方案", "确认主视觉图与文案", "补充移动端适配方案"];

/** 渲染任务的次要上下文，并允许原位展开会议摘要。 */
export function TaskContextPanel({ task }: TaskContextPanelProps): React.JSX.Element {
  const [summaryOpen, setSummaryOpen] = useState(true);

  return (
    <aside className="task-context">
      <section className="context-section">
        <header><h3>任务信息</h3></header>
        <dl className="task-meta-list">
          <div><dt><Target size={13} aria-hidden="true" /> 状态</dt><dd>{taskStatusLabel(task.status)}</dd></div>
          <div><dt><CheckSquare2 size={13} aria-hidden="true" /> 优先级</dt><dd><span className="priority-dot" /> 高优先级</dd></div>
          <div><dt><CalendarDays size={13} aria-hidden="true" /> 截止时间</dt><dd>{task.dueLabel}</dd></div>
          <div><dt><Tag size={13} aria-hidden="true" /> 标签</dt><dd><span className="tag">{task.project}</span></dd></div>
        </dl>
      </section>

      <section className="context-section related-meeting">
        <header><h3>关联会议</h3></header>
        <button className="meeting-link-row" type="button" onClick={() => setSummaryOpen((open) => !open)}>
          <CalendarDays size={14} aria-hidden="true" />
          <span>产品设计评审会 · 45 分钟</span>
          <ChevronRight className={summaryOpen ? "is-open" : ""} size={13} aria-hidden="true" />
        </button>
        {summaryOpen ? (
          <div className="meeting-summary-popover">
            <strong>会议摘要</strong>
            <ul>
              <li>确定首屏核心信息与行动引导。</li>
              <li>补充品牌信任背书与客户案例。</li>
            </ul>
            <button className="primary-button" type="button">转为任务</button>
          </div>
        ) : null}
      </section>

      <section className="context-section task-actions">
        <header><h3>行动项</h3><span>{defaultActions.length}</span></header>
        {defaultActions.map((action) => (
          <label key={action}>
            <input type="checkbox" />
            <span>{action}</span>
          </label>
        ))}
        <button className="inline-create" type="button">＋ 添加行动项</button>
      </section>
    </aside>
  );
}
