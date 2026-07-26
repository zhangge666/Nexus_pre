/** 本文件实现 Muse 会议记录、会后摘要与行动项页面。 */

import React, { useState } from "react";
import { Clock3, FileText, Mic2, MoreHorizontal, Play, Sparkles, Square } from "lucide-react";
import { PageHeader } from "../components/PageHeader";
import { formatCompactTime } from "../core/format";
import type { MuseMeeting } from "../core/types";

interface MeetingsPageProps {
  meetings: MuseMeeting[];
}

/** 呈现会议列表与选中会议的可核对摘要。 */
export function MeetingsPage({ meetings }: MeetingsPageProps): React.JSX.Element {
  const [selectedId, setSelectedId] = useState(meetings[0]?.id ?? "");
  const [showTranscript, setShowTranscript] = useState(false);
  const selected = meetings.find((meeting) => meeting.id === selectedId) ?? meetings[0];

  return (
    <div className="page page-meetings">
      <PageHeader
        eyebrow="会议"
        title="录音、摘要与行动项"
        description="会议内容保存在本机；停止后再生成可回链的摘要和任务建议。"
        actions={(
          <button className="primary-button" type="button" title="录音模块将在下一开发阶段接入">
            <Mic2 size={13} /> 开始录音
          </button>
        )}
      />

      <section className="meeting-workspace">
        <aside className="meeting-list">
          <header>
            <span>最近会议</span>
            <button type="button"><MoreHorizontal size={14} /></button>
          </header>
          {meetings.map((meeting) => (
            <button
              className={meeting.id === selected?.id ? "is-selected" : ""}
              key={meeting.id}
              type="button"
              onClick={() => setSelectedId(meeting.id)}
            >
              <span className="meeting-list-icon"><Mic2 size={14} /></span>
              <span>
                <strong>{meeting.title}</strong>
                <small>{formatCompactTime(meeting.recordedAt)} · {meeting.durationLabel}</small>
              </span>
              <em>{meeting.actionItems.length}</em>
            </button>
          ))}
        </aside>

        {selected ? (
          <article className="meeting-detail">
            <header className="meeting-player">
              <button className="play-button" type="button" aria-label="播放会议录音"><Play size={15} fill="currentColor" /></button>
              <div>
                <strong>{selected.title}</strong>
                <span><Clock3 size={11} /> {selected.durationLabel} · 本地录音</span>
              </div>
              <div className="audio-track"><span /></div>
              <time>00:00 / {selected.durationLabel}</time>
              <button type="button" aria-label="停止播放"><Square size={12} /></button>
            </header>

            <div className="meeting-tabs">
              <button className={!showTranscript ? "is-active" : ""} type="button" onClick={() => setShowTranscript(false)}>会议摘要</button>
              <button className={showTranscript ? "is-active" : ""} type="button" onClick={() => setShowTranscript(true)}>完整转写</button>
              <span>本地生成 · 可编辑</span>
            </div>

            {!showTranscript ? (
              <div className="meeting-summary-layout">
                <section className="meeting-summary">
                  <header><Sparkles size={13} /><span>摘要</span><button type="button">重新整理</button></header>
                  <h2>{selected.summary}</h2>
                  <ul>
                    <li><button type="button">08:14</button> 发布范围不再增加，桌面端内容已确认。</li>
                    <li><button type="button">18:19</button> 移动端断点已修复，仍需补齐埋点验证。</li>
                    <li><button type="button">26:02</button> 检查表和回滚包随发布结果一同留档。</li>
                  </ul>
                  <div className="meeting-risk"><strong>待确认</strong> 预发布环境的按钮埋点尚未回传。</div>
                </section>
                <aside className="meeting-actions">
                  <header><span>行动项</span><em>{selected.actionItems.length}</em></header>
                  {selected.actionItems.map((item, index) => (
                    <label key={item}>
                      <input type="checkbox" defaultChecked />
                      <span><strong>{item}</strong><small>{index === 0 ? "周凯 · 今天 16:30" : "我 · 今天 18:00"}</small></span>
                    </label>
                  ))}
                  <button className="primary-button" type="button">创建任务</button>
                </aside>
              </div>
            ) : (
              <section className="transcript-panel">
                <article><time>18:06</time><strong>林然</strong><p>首页今天必须发出，移动端断点和按钮埋点需要在发布前再确认一次。</p></article>
                <article><time>18:19</time><strong>我</strong><p>390 像素断点已经修复。埋点等预发布环境回传，之后把检查表和回滚包一起发出。</p></article>
                <article><time>18:35</time><strong>周凯</strong><p>我来确认埋点，最晚四点半给结果。</p></article>
              </section>
            )}

            <footer className="meeting-detail-footer">
              <span><FileText size={12} /> 音频与转写均保存在本机</span>
              <button className="secondary-button" type="button">整理到 Quill</button>
            </footer>
          </article>
        ) : <div className="empty-state">开始第一次会议后，摘要会显示在这里。</div>}
      </section>
    </div>
  );
}
