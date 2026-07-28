/** 本文件呈现“今天”页的最近会议与会后整理状态。 */

import React from "react";
import { ArrowRight, CalendarDays, CheckCircle2 } from "lucide-react";
import { formatCompactTime } from "../../core/format";
import type { MuseMeeting } from "../../core/types";

interface TodayMeetingListProps {
  meetings: MuseMeeting[];
  onOpenMeetings: () => void;
}

/** 渲染最近会议行，并把开始会议保留为该区域唯一主要动作。 */
export function TodayMeetingList({ meetings, onOpenMeetings }: TodayMeetingListProps): React.JSX.Element {
  return (
    <section className="workspace-panel today-meeting-panel">
      <header className="panel-heading">
        <div>
          <h2>最近会议</h2>
          <span>{meetings.length}</span>
        </div>
        <button className="text-button" type="button" onClick={onOpenMeetings}>
          查看全部 <ArrowRight size={12} aria-hidden="true" />
        </button>
      </header>
      <div className="linear-list">
        {meetings.slice(0, 3).map((meeting) => (
          <button className="linear-row meeting-overview-row" key={meeting.id} type="button" onClick={onOpenMeetings}>
            <span className="row-icon row-icon--accent"><CalendarDays size={14} aria-hidden="true" /></span>
            <span className="row-primary">
              <strong>{meeting.title}</strong>
              <small>{formatCompactTime(meeting.recordedAt)}</small>
            </span>
            <span>{meeting.durationLabel}</span>
            <span className="meeting-state"><CheckCircle2 size={12} aria-hidden="true" /> 已总结</span>
            <span>{meeting.actionItems.length} 个行动项</span>
          </button>
        ))}
        {meetings.length === 0 ? <div className="panel-empty">会议结束后，摘要与行动项会显示在这里。</div> : null}
      </div>
      <footer className="panel-footer">
        <span>录音与转写默认保存在本机</span>
        <button className="primary-button" type="button" onClick={onOpenMeetings}>开始会议</button>
      </footer>
    </section>
  );
}
