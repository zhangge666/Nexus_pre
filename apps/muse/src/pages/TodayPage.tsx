/** 本文件组合 Muse “今天”工作区的标题、快速输入与三个独立内容区块。 */

import React, { useEffect, useRef } from "react";
import { CheckCircle2, ClipboardCopy } from "lucide-react";
import { IdeaComposer } from "../components/IdeaComposer";
import { PageHeader } from "../components/PageHeader";
import { TodayIdeaList } from "../features/today/TodayIdeaList";
import { TodayMeetingList } from "../features/today/TodayMeetingList";
import { TodayTaskList } from "../features/today/TodayTaskList";
import type { MuseClipboardItem, MuseIdea, MuseMeeting, MuseTask, MuseView } from "../core/types";

interface TodayPageProps {
  ideas: MuseIdea[];
  tasks: MuseTask[];
  meetings: MuseMeeting[];
  clipboard: MuseClipboardItem[];
  onAddIdea: (content: string) => Promise<void>;
  onNavigate: (view: MuseView) => void;
}

/** 生成页面标题使用的本地日期。 */
function todayLabel(): string {
  const date = new Intl.DateTimeFormat("zh-CN", { month: "long", day: "numeric" }).format(new Date());
  const weekday = new Intl.DateTimeFormat("zh-CN", { weekday: "short" }).format(new Date());
  return `${date} · ${weekday}`;
}

/** 呈现当前最相关的任务、灵感和会议，不引入装饰性统计卡片。 */
export function TodayPage({
  ideas,
  tasks,
  meetings,
  clipboard,
  onAddIdea,
  onNavigate,
}: TodayPageProps): React.JSX.Element {
  const captureRef = useRef<HTMLTextAreaElement>(null);
  const activeTasks = tasks.filter((task) => task.status !== "done").slice(0, 5);
  const pinnedClipboardCount = clipboard.filter((item) => item.pinned).length;

  useEffect(() => {
    /** 使用 Command/Ctrl + N 将焦点直接移动到快速输入。 */
    function handleQuickCapture(event: KeyboardEvent): void {
      if (!(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== "n") return;
      event.preventDefault();
      captureRef.current?.focus();
    }

    window.addEventListener("keydown", handleQuickCapture);
    return () => window.removeEventListener("keydown", handleQuickCapture);
  }, []);

  return (
    <div className="page page-today">
      <PageHeader title="今天" eyebrow={todayLabel()} description="聚焦现在要完成的工作" />
      <section className="today-capture" aria-label="快速记录">
        <IdeaComposer compact inputRef={captureRef} onSubmit={onAddIdea} />
      </section>
      <div className="today-overview-grid">
        <TodayTaskList tasks={activeTasks} onOpenTasks={() => onNavigate("tasks")} />
        <TodayIdeaList ideas={ideas} onOpenIdeas={() => onNavigate("ideas")} />
      </div>
      <TodayMeetingList meetings={meetings} onOpenMeetings={() => onNavigate("meetings")} />
      <footer className="today-status">
        <span><CheckCircle2 size={12} aria-hidden="true" /> 本地工作区已保存</span>
        <span>{ideas.length} 条灵感</span>
        <button type="button" onClick={() => onNavigate("clipboard")}>
          <ClipboardCopy size={12} aria-hidden="true" /> {pinnedClipboardCount} 条剪贴板已固定
        </button>
      </footer>
    </div>
  );
}
