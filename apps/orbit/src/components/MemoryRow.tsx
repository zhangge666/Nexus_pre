/** 本文件实现记忆行项组件，用于检索结果和时间线列表。 */
import type React from "react";
import { Monitor, Lightbulb, Feather, Orbit, Mic, FileText, Paperclip } from "lucide-react";
import type { MemorySummary, MemorySource, MemoryKind } from "../core";

function SourceIcon({ source, kind }: { source: MemorySource; kind: MemoryKind }): React.JSX.Element {
  if (kind === "screen") return <Monitor size={13} />;
  if (kind === "voice") return <Mic size={13} />;
  if (kind === "clip") return <Paperclip size={13} />;
  if (source === "echo") return <Monitor size={13} />;
  if (source === "muse") return <Lightbulb size={13} />;
  if (source === "quill") return <Feather size={13} />;
  if (source === "orbit") return <Orbit size={13} />;
  return <FileText size={13} />;
}

function formatTime(timestamp: number): string {
  const diff = Date.now() - timestamp;
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}分钟前`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}小时前`;
  if (diff < 604_800_000) return `${Math.floor(diff / 86_400_000)}天前`;
  return new Intl.DateTimeFormat("zh-CN", { month: "short", day: "numeric" }).format(timestamp);
}

interface MemoryRowProps {
  memory: MemorySummary & { score?: number };
  selected?: boolean;
  onClick: () => void;
  snippet?: string;
}

export function MemoryRow({ memory, selected, onClick, snippet }: MemoryRowProps): React.JSX.Element {
  return (
    <button
      className={`memory-row${selected ? " selected" : ""}`}
      onClick={onClick}
      title={memory.title ?? memory.kind}
    >
      <span className={`source-mark src-${memory.source.split(":")[0]}`}>
        <SourceIcon source={memory.source} kind={memory.kind} />
      </span>
      <span className="memory-content">
        <span className="memory-meta">
          <span className="source-name">{memory.source === "orbit" ? "Orbit" : memory.source.charAt(0).toUpperCase() + memory.source.slice(1)}</span>
          <span className="memory-time">{memory.createdAt ? formatTime(memory.createdAt) : "检索命中"}</span>
          {memory.score !== undefined && (
            <span className="score-badge">{Math.round(memory.score * 100)}%</span>
          )}
        </span>
        <strong className="memory-title">{memory.title ?? memory.kind}</strong>
        <span className="memory-excerpt">{snippet ?? memory.content}</span>
        {memory.tags.length > 0 && (
          <span className="tag-list">
            {memory.tags.slice(0, 3).map((tag) => (
              <em key={tag}>{tag}</em>
            ))}
          </span>
        )}
      </span>
    </button>
  );
}
