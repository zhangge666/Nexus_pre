/** 本文件实现 Muse 多页面复用的灵感快速输入与本地保存反馈。 */

import React, { FormEvent, useState } from "react";
import { Hash, Mic, Paperclip, Send } from "lucide-react";

interface IdeaComposerProps {
  compact?: boolean;
  onSubmit: (content: string) => Promise<void> | void;
}

/** 渲染零干扰灵感编辑器，提交成功后清空输入。 */
export function IdeaComposer({ compact = false, onSubmit }: IdeaComposerProps): React.JSX.Element {
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
      <textarea
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
        <button className="primary-button" type="submit" disabled={!content.trim()}>
          <Send size={13} aria-hidden="true" />
          收好
          <kbd>Ctrl ↵</kbd>
        </button>
      </footer>
    </form>
  );
}
