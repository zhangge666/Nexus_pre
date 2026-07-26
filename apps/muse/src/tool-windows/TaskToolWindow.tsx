/** 本文件实现任务与原始要求一次完成绑定的快捷新建窗口。 */

import React, { FormEvent, useCallback, useRef, useState } from "react";
import { CalendarClock, ClipboardPaste, ListTodo, Save } from "lucide-react";
import { useMuseWorkspace } from "../core/workspace";
import { hideToolWindow, useToolWindowLifecycle } from "./lifecycle";
import { ToolWindowFrame } from "./ToolWindowFrame";

/** 渲染标题、来源、截止时间与责任上下文齐备的任务专用界面。 */
export function TaskToolWindow(): React.JSX.Element {
  const { addTask } = useMuseWorkspace();
  const titleRef = useRef<HTMLInputElement>(null);
  const [title, setTitle] = useState("");
  const [sourceText, setSourceText] = useState("");
  const [dueLabel, setDueLabel] = useState("");
  const [requester, setRequester] = useState("");
  const [project, setProject] = useState("");
  const [feedback, setFeedback] = useState("来源会成为任务的第一条留痕");
  const focusTitle = useCallback(() => {
    window.setTimeout(() => titleRef.current?.focus(), 40);
  }, []);

  useToolWindowLifecycle({ hideOnBlur: true, onFocus: focusTitle });

  /** 尝试读取当前系统剪贴板作为待确认来源，不覆盖用户已输入内容。 */
  async function pasteCurrentClipboard(): Promise<void> {
    try {
      const value = await navigator.clipboard.readText();
      if (!value.trim()) {
        setFeedback("当前剪贴板没有文字");
        return;
      }
      setSourceText(value);
      setFeedback("已带入当前剪贴板，请确认后保存");
    } catch {
      setFeedback("未获得剪贴板读取权限，可直接粘贴");
    }
  }

  /** 创建任务及首条来源活动，清空表单后隐藏窗口。 */
  async function handleSubmit(event: FormEvent): Promise<void> {
    event.preventDefault();
    if (!title.trim()) return;
    addTask(title, sourceText, { dueLabel, requester, project });
    setTitle("");
    setSourceText("");
    setDueLabel("");
    setRequester("");
    setProject("");
    setFeedback("任务与来源已保存");
    window.setTimeout(() => {
      setFeedback("来源会成为任务的第一条留痕");
      void hideToolWindow();
    }, 260);
  }

  return (
    <ToolWindowFrame
      title="新建任务"
      subtitle="把要求和上下文一起留下"
      shortcut="Ctrl Shift T"
      icon={<ListTodo size={14} />}
    >
      <form className="task-tool-form" onSubmit={(event) => void handleSubmit(event)}>
        <label className="tool-field tool-field--title">
          <span>任务标题</span>
          <input
            ref={titleRef}
            value={title}
            onChange={(event) => setTitle(event.target.value)}
            placeholder="例如：更新客户报价并重新发送"
            autoFocus
          />
        </label>
        <label className="tool-field tool-field--source">
          <span>
            原始要求 / 来源
            <button type="button" onClick={() => void pasteCurrentClipboard()}>
              <ClipboardPaste size={12} />
              读取当前剪贴板
            </button>
          </span>
          <textarea
            value={sourceText}
            onChange={(event) => setSourceText(event.target.value)}
            placeholder="粘贴微信、钉钉、邮件或其他任务原文…"
          />
        </label>
        <div className="task-meta-grid">
          <label className="tool-field">
            <span><CalendarClock size={12} /> 截止</span>
            <input value={dueLabel} onChange={(event) => setDueLabel(event.target.value)} placeholder="今天 18:00" />
          </label>
          <label className="tool-field">
            <span>提出人</span>
            <input value={requester} onChange={(event) => setRequester(event.target.value)} placeholder="姓名 / 团队" />
          </label>
          <label className="tool-field">
            <span>项目</span>
            <input value={project} onChange={(event) => setProject(event.target.value)} placeholder="未分类" />
          </label>
        </div>
        <footer className="tool-footer">
          <span className="tool-feedback" role="status" aria-live="polite">{feedback}</span>
          <button className="tool-primary-button" type="submit" disabled={!title.trim()}>
            <Save size={13} aria-hidden="true" />
            建立任务
          </button>
        </footer>
      </form>
    </ToolWindowFrame>
  );
}
