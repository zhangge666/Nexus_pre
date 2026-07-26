/** 本文件实现由独立快捷键唤起的轻量灵感捕捉窗口。 */

import React, { FormEvent, useCallback, useRef, useState } from "react";
import { Hash, Lightbulb, Mic, Paperclip, Send } from "lucide-react";
import { useMuseWorkspace } from "../core/workspace";
import { hideToolWindow, useToolWindowLifecycle } from "./lifecycle";
import { ToolWindowFrame } from "./ToolWindowFrame";

/** 渲染自动聚焦、保存后离开的灵感专用界面。 */
export function IdeaToolWindow(): React.JSX.Element {
  const { addIdea } = useMuseWorkspace();
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const [content, setContent] = useState("");
  const [feedback, setFeedback] = useState("内容仅保存在本机");
  const focusInput = useCallback(() => {
    window.setTimeout(() => inputRef.current?.focus(), 40);
  }, []);

  useToolWindowLifecycle({ hideOnBlur: true, onFocus: focusInput });

  /** 保存非空灵感，并隐藏而不是销毁工具窗。 */
  async function handleSubmit(event: FormEvent): Promise<void> {
    event.preventDefault();
    const value = content.trim();
    if (!value) return;
    addIdea(value);
    setContent("");
    setFeedback("已收好");
    window.setTimeout(() => {
      setFeedback("内容仅保存在本机");
      void hideToolWindow();
    }, 220);
  }

  /** 在光标处插入标签起始符，减少键盘切换。 */
  function insertTag(): void {
    setContent((current) => `${current}${current && !current.endsWith(" ") ? " " : ""}#`);
    focusInput();
  }

  return (
    <ToolWindowFrame
      title="记下灵感"
      subtitle="想到什么，先收进 Muse"
      shortcut="Ctrl Shift I"
      icon={<Lightbulb size={14} />}
    >
      <form className="idea-tool-form" onSubmit={(event) => void handleSubmit(event)}>
        <textarea
          ref={inputRef}
          value={content}
          onChange={(event) => setContent(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && event.ctrlKey) void handleSubmit(event);
          }}
          placeholder="直接写下刚刚想到的内容…"
          aria-label="灵感内容"
          autoFocus
        />
        <footer className="tool-footer">
          <div className="quiet-tools">
            <button type="button" onClick={insertTag} aria-label="插入标签">
              <Hash size={14} />
            </button>
            <button type="button" disabled title="附件能力将在后续接入" aria-label="附件能力尚未接入">
              <Paperclip size={14} />
            </button>
            <button type="button" disabled title="语音能力将在后续接入" aria-label="语音能力尚未接入">
              <Mic size={14} />
            </button>
            <span role="status" aria-live="polite">{feedback}</span>
          </div>
          <button className="tool-primary-button" type="submit" disabled={!content.trim()}>
            <Send size={13} aria-hidden="true" />
            收好
            <kbd>Ctrl ↵</kbd>
          </button>
        </footer>
      </form>
    </ToolWindowFrame>
  );
}
