/** 本文件实现 Muse 全局命令面板，让页面和核心工具可以通过键盘快速到达。 */

import React, { useEffect, useMemo, useState } from "react";
import {
  CalendarDays,
  CheckSquare2,
  ClipboardCopy,
  Lightbulb,
  Mic2,
  Search,
  Settings2,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { MuseView } from "../core/types";
import { isMacOS } from "../core/platform";

interface CommandPaletteProps {
  activeView: MuseView;
  open: boolean;
  onClose: () => void;
  onNavigate: (view: MuseView) => void;
}

interface MuseCommand {
  id: MuseView;
  label: string;
  description: string;
  shortcut: string;
  icon: LucideIcon;
}

const commands: MuseCommand[] = [
  { id: "today", label: "回到今天", description: "查看待办与最近内容", shortcut: "G T", icon: CalendarDays },
  { id: "ideas", label: "记录灵感", description: "捕捉想法、标签与草稿", shortcut: "⇧ I", icon: Lightbulb },
  { id: "tasks", label: "新建任务", description: "绑定要求并开始留痕", shortcut: "⇧ T", icon: CheckSquare2 },
  { id: "meetings", label: "开始会议", description: "记录、摘要与行动项", shortcut: "⇧ R", icon: Mic2 },
  { id: "clipboard", label: "打开剪贴板", description: "固定、摘取与比较内容", shortcut: "⇧ V", icon: ClipboardCopy },
  { id: "settings", label: "打开设置", description: "本地工作区与 Orbit 连接", shortcut: "G S", icon: Settings2 },
];

/** 呈现可搜索的轻量命令列表，并负责 Escape 关闭行为。 */
export function CommandPalette({
  activeView,
  open,
  onClose,
  onNavigate,
}: CommandPaletteProps): React.JSX.Element | null {
  const [query, setQuery] = useState("");
  const modifier = isMacOS() ? "⌘" : "Ctrl";
  const filteredCommands = useMemo(() => {
    const value = query.trim().toLocaleLowerCase();
    if (!value) return commands;
    return commands.filter((command) =>
      `${command.label} ${command.description}`.toLocaleLowerCase().includes(value),
    );
  }, [query]);

  useEffect(() => {
    if (!open) {
      setQuery("");
      return undefined;
    }

    /** 允许用户不离开键盘即可收起命令面板。 */
    function handleEscape(event: KeyboardEvent): void {
      if (event.key !== "Escape") return;
      event.preventDefault();
      onClose();
    }

    window.addEventListener("keydown", handleEscape);
    return () => window.removeEventListener("keydown", handleEscape);
  }, [onClose, open]);

  if (!open) return null;

  /** 跳转后立即关闭浮层，保持单一主焦点。 */
  function handleNavigate(view: MuseView): void {
    onNavigate(view);
    onClose();
  }

  return (
    <div
      className="command-backdrop"
      role="presentation"
      onMouseDown={(event) => {
        if (event.currentTarget === event.target) onClose();
      }}
    >
      <section className="command-panel" role="dialog" aria-modal="true" aria-label="快速打开">
        <label className="command-search">
          <Search size={15} aria-hidden="true" />
          <input
            autoFocus
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="输入功能名称…"
            aria-label="搜索 Muse 功能"
          />
          <kbd>Esc</kbd>
        </label>
        <div className="command-list" role="list">
          {filteredCommands.map(({ id, label, description, shortcut, icon: Icon }) => (
            <button
              className={activeView === id ? "is-current" : ""}
              key={id}
              type="button"
              onClick={() => handleNavigate(id)}
            >
              <span className="command-icon"><Icon size={15} aria-hidden="true" /></span>
              <span className="command-copy">
                <strong>{label}</strong>
                <small>{description}</small>
              </span>
              <kbd>{shortcut.startsWith("⇧") ? `${modifier} ${shortcut}` : shortcut}</kbd>
            </button>
          ))}
          {filteredCommands.length === 0 ? <p className="command-empty">没有匹配的功能</p> : null}
        </div>
        <footer>
          <span><kbd>Tab</kbd> 切换</span>
          <span><kbd>↵</kbd> 打开</span>
          <span>内容默认保存在本机</span>
        </footer>
      </section>
    </div>
  );
}
