/** 本文件实现会议快捷窗的计时、重点标记与文字记录状态流。 */

import React, { useEffect, useMemo, useRef, useState } from "react";
import { BookmarkPlus, Circle, Mic, Square, TimerReset } from "lucide-react";
import { formatDuration } from "../core/format";
import { useMuseWorkspace } from "../core/workspace";
import { hideToolWindow, useToolWindowLifecycle } from "./lifecycle";
import { ToolWindowFrame } from "./ToolWindowFrame";

/** 渲染会议记录控制界面；当前阶段只保存计时与文字，不伪装真实录音。 */
export function MeetingToolWindow(): React.JSX.Element {
  const { addMeeting } = useMuseWorkspace();
  const noteRef = useRef<HTMLTextAreaElement>(null);
  const [title, setTitle] = useState("");
  const [notes, setNotes] = useState("");
  const [elapsed, setElapsed] = useState(0);
  const [recording, setRecording] = useState(false);
  const [markCount, setMarkCount] = useState(0);
  const [feedback, setFeedback] = useState("音频录制尚未接入；当前保存计时和文字记录");

  useToolWindowLifecycle();

  useEffect(() => {
    if (!recording) return;
    const timer = window.setInterval(() => setElapsed((current) => current + 1), 1_000);
    return () => window.clearInterval(timer);
  }, [recording]);

  const duration = useMemo(() => formatDuration(elapsed), [elapsed]);

  /** 开始一次新的会议记录计时。 */
  function startMeeting(): void {
    setRecording(true);
    setFeedback("记录进行中 · 可随时添加重点");
    window.setTimeout(() => noteRef.current?.focus(), 40);
  }

  /** 在当前时间点插入可检索的重点标记。 */
  function addHighlight(): void {
    if (!recording) return;
    const marker = `[${duration}] 重点：`;
    setNotes((current) => `${current}${current ? "\n" : ""}${marker}`);
    setMarkCount((current) => current + 1);
    window.setTimeout(() => noteRef.current?.focus(), 20);
  }

  /** 停止计时并把会议标题、时长和文字记录保存到本地工作区。 */
  function stopAndSave(): void {
    if (!recording) return;
    addMeeting(title, duration, notes);
    setRecording(false);
    setFeedback("会议记录已保存到本机");
    window.setTimeout(() => {
      setTitle("");
      setNotes("");
      setElapsed(0);
      setMarkCount(0);
      setFeedback("音频录制尚未接入；当前保存计时和文字记录");
      void hideToolWindow();
    }, 520);
  }

  return (
    <ToolWindowFrame
      title="会议记录"
      subtitle={recording ? "计时与文字记录进行中" : "准备记录会议"}
      shortcut="Ctrl Shift R"
      icon={<Mic size={14} />}
      tone={recording ? "recording" : "default"}
    >
      <div className="meeting-tool">
        <div className="meeting-status-row">
          <div className={`meeting-pulse ${recording ? "is-recording" : ""}`}>
            <Circle size={9} fill="currentColor" aria-hidden="true" />
            {recording ? "记录中" : "待开始"}
          </div>
          <time>{duration}</time>
          <span>{markCount} 个重点</span>
          <span className="phase-chip">当前阶段：文字记录</span>
        </div>
        <input
          className="meeting-title-input"
          value={title}
          onChange={(event) => setTitle(event.target.value)}
          placeholder="会议名称（可稍后补充）"
          aria-label="会议名称"
        />
        <textarea
          ref={noteRef}
          className="meeting-notes"
          value={notes}
          onChange={(event) => setNotes(event.target.value)}
          placeholder={recording ? "边听边记下结论、分歧和行动项…" : "开始后可在这里记录会议重点…"}
          disabled={!recording}
          aria-label="会议文字记录"
        />
        <footer className="tool-footer">
          <span className="tool-feedback" role="status" aria-live="polite">{feedback}</span>
          <div className="meeting-actions">
            <button className="tool-secondary-button" type="button" onClick={addHighlight} disabled={!recording}>
              <BookmarkPlus size={13} />
              标记重点
            </button>
            {recording ? (
              <button className="tool-stop-button" type="button" onClick={stopAndSave}>
                <Square size={11} fill="currentColor" />
                停止并保存
              </button>
            ) : (
              <button className="tool-primary-button" type="button" onClick={startMeeting}>
                <TimerReset size={13} />
                开始计时
              </button>
            )}
          </div>
        </footer>
      </div>
    </ToolWindowFrame>
  );
}
