/** 本文件实现 Muse 多页面复用的灵感快速输入与本地保存反馈。 */

import React, { FormEvent, type Ref, useState } from "react";
import { Hash, Mic, Paperclip, PenLine, Send } from "lucide-react";

interface IdeaComposerProps {
  compact?: boolean;
  inputRef?: Ref<HTMLTextAreaElement>;
  onSubmit: (content: string) => Promise<void> | void;
}

/** 渲染零干扰灵感编辑器，提交成功后清空输入。 */
export function IdeaComposer({ compact = false, inputRef, onSubmit }: IdeaComposerProps): React.JSX.Element {
  const [content, setContent] = useState("");
  const [feedback, setFeedback] = useState("内容先保存在本机");

  /** 保存非空灵感，并保留异步同步反馈。 */
  async function handleSubmit(event: FormEvent): Promise<void> {
    event.preventDefault();
    const value = content.trim();
    if (!value) return;
    setFeedback("正在保存…");
    await onSubmit(value);
    setContent("");
    setFeedback("已保存到本机");
  }

  return (
    <form className={`idea-composer ${compact ? "is-compact" : ""}`} onSubmit={(event) => void handleSubmit(event)}>
      {compact ? <PenLine className="compact-composer-icon" size={18} aria-hidden="true" /> : null}
      <textarea
        ref={inputRef}
        value={content}
        onChange={(event) => setContent(event.target.value)}
        placeholder="把刚刚想到的内容交给 Muse…"
        aria-label="记录灵感"
      />
      <footer>
        <div className="composer-tools">
          <button type="button" aria-label="添加标签"><Hash size={14} /></button>
          <button type="button" aria-label="添加附件"><Paperclip size={14} /></button>
          <button type="button" aria-label="语音输入"><Mic size={14} /></button>
          <span role="status" aria-live="polite">{feedback}</span>
        </div>
        <button className="primary-button" type="submit" disabled={!content.trim()} aria-label="保存灵感">
          <Send size={13} aria-hidden="true" />
          <span>收好</span>
          <kbd>Ctrl ↵</kbd>
        </button>
      </footer>
    </form>
  );
}
