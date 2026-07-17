/** 本文件实现记忆问答（RAG）页面，支持流式输出和引用溯源。 */
import type React from "react";
import { useState, useRef, useEffect, FormEvent } from "react";
import { Send, MessageCircle, AlertTriangle } from "lucide-react";
import { askMemory, askMemoryStream, getSettings } from "../core";
import type { ChatMessage, Citation, OrbitSettings } from "../core";
import { Topbar } from "../components/Topbar";
import { EmptyState } from "../components/EmptyState";
import { useNavigate } from "react-router-dom";

/** 判断自定义端点是否严格指向本机回环地址，避免将相似域名误判成本地服务。 */
function isLoopbackEndpoint(endpoint: string): boolean {
  try {
    const hostname = new URL(endpoint.trim()).hostname.toLowerCase();
    return hostname === "127.0.0.1" || hostname === "localhost" || hostname === "[::1]" || hostname === "::1";
  } catch {
    return false;
  }
}

/** 按当前设置预判问答是否会向远程 Provider 发送经过最小化的检索上下文。 */
function sendsDataRemote(settings: OrbitSettings["rag"] | null): boolean {
  return settings?.provider === "claude"
    || settings?.provider === "openai"
    || (settings?.provider === "custom" && !isLoopbackEndpoint(settings.customEndpoint));
}

function CitationCard({ citation, onOpen }: { citation: Citation; onOpen: () => void }): React.JSX.Element {
  const kindIcon: Record<string, string> = {
    idea: "💡", note: "📝", screen: "🖥", voice: "🎤", card: "🃏", clip: "📎", file: "📄",
  };
  return (
    <button className="citation-card" onClick={onOpen} aria-label={`打开引用：${citation.sourceTitle ?? citation.memoryId}`}>
      <span className="citation-icon">{kindIcon[citation.sourceKind ?? "note"] ?? "📎"}</span>
      <div className="citation-body">
        <strong>{citation.sourceTitle ?? citation.memoryId}</strong>
        <p>{citation.snippet}</p>
      </div>
    </button>
  );
}

function ChatBubble({ msg, onOpenCitation }: { msg: ChatMessage; onOpenCitation: (citation: Citation) => void }): React.JSX.Element {
  return (
    <div className={`chat-bubble ${msg.role}`}>
      {msg.role === "ai" ? (
        <>
          <div className="bubble-content">
            {msg.content.split("\n").map((line: string, i: number) =>
              line === "" ? <br key={i} /> : <p key={i}>{line}</p>
            )}
          </div>
          {msg.citations && msg.citations.length > 0 && (
            <div className="citations-block">
              <p className="citations-label">📎 引用来源：</p>
              {msg.citations.map((c: Citation) => (
                <CitationCard key={c.blockId} citation={c} onOpen={() => onOpenCitation(c)} />
              ))}
            </div>
          )}
        </>
      ) : (
        <p className="bubble-content">{msg.content}</p>
      )}
    </div>
  );
}

export default function AskPage(): React.JSX.Element {
  const navigate = useNavigate();
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [streamText, setStreamText] = useState("");
  const bottomRef = useRef<HTMLDivElement>(null);
  const [ragSettings, setRagSettings] = useState<OrbitSettings["rag"] | null>(null);
  const [lastFlow, setLastFlow] = useState<{ provider: string; count: number; remote: boolean } | null>(null);

  useEffect(() => {
    void getSettings().then((settings) => setRagSettings(settings.rag));
  }, []);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streamText]);

  async function handleSubmit(e: FormEvent): Promise<void> {
    e.preventDefault();
    const question = input.trim();
    if (!question || loading) return;
    if (sendsDataRemote(ragSettings) && ragSettings?.confirmBeforeSend) {
      const confirmed = window.confirm(
        `本次问答将只把本地检索命中的必要片段发送到 ${ragSettings.provider}，不会发送整库。是否继续？`,
      );
      if (!confirmed) return;
    }

    const userMsg: ChatMessage = {
      id: "msg-" + Date.now(),
      role: "user",
      content: question,
      createdAt: Date.now(),
    };
    setMessages((ms) => [...ms, userMsg]);
    setInput("");
    setLoading(true);
    setStreamText("");

    try {
      const response = ragSettings?.streamEnabled
        ? await askMemoryStream({ question }, (delta) => {
            setStreamText((current) => current + delta);
          })
        : await askMemory({ question });
      setLastFlow({ provider: response.provider, count: response.sentContextCount, remote: response.sendsDataRemote });
      setStreamText("");
      const aiMsg: ChatMessage = {
        id: "msg-" + Date.now(),
        role: "ai",
        content: response.answer,
        citations: response.citations,
        createdAt: Date.now(),
      };
      setMessages((ms) => [...ms, aiMsg]);
    } catch (e) {
      setInput(question);
      const errMsg: ChatMessage = {
        id: "msg-" + Date.now(),
        role: "ai",
        content: `问答失败：${String(e)}`,
        createdAt: Date.now(),
      };
      setMessages((ms) => [...ms, errMsg]);
    } finally {
      setLoading(false);
    }
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>): void {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void handleSubmit(e as unknown as FormEvent);
    }
  }

  return (
    <div className="page-enter ask-page">
      <Topbar title="记忆问答" subtitle="基于你的本地记忆库回答问题" />

      <div className="chat-container">
        {messages.length === 0 && !streamText && (
          <EmptyState
            icon={<MessageCircle size={40} />}
            title="问问你的第二大脑"
            description="输入问题，Orbit 会从你的记忆库中检索相关内容并给出答案"
          />
        )}

        {messages.map((msg) => (
          <ChatBubble key={msg.id} msg={msg} onOpenCitation={(citation) => navigate(`/search?id=${citation.memoryId}`)} />
        ))}

        {/* 流式输出中 */}
        {streamText && (
          <div className="chat-bubble ai streaming">
            <div className="bubble-content">
              {streamText.split("\n").map((line, i) =>
                line === "" ? <br key={i} /> : <p key={i}>{line}</p>
              )}
              <span className="streaming-cursor" aria-hidden="true" />
            </div>
          </div>
        )}

        <div ref={bottomRef} />
      </div>

      {/* 输入区 */}
      <div className="ask-input-area">
        <div className="provider-notice">
          <AlertTriangle size={12} />
          {lastFlow
            ? lastFlow.remote
              ? `上次仅向 ${lastFlow.provider} 发送了 ${lastFlow.count} 条检索片段`
              : `上次由 ${lastFlow.provider} 在本地处理，未发送数据`
            : sendsDataRemote(ragSettings)
              ? `将只向 ${ragSettings?.provider ?? "远程 Provider"} 发送本地命中的必要片段，不发送整库`
              : "使用本地 Completion，不会发送记忆数据"}
        </div>
        <form className="ask-form" onSubmit={handleSubmit}>
          <textarea
            className="ask-textarea"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="对你的第二大脑提问… (↵ 发送，Shift+↵ 换行)"
            rows={2}
            disabled={loading}
            aria-label="提问内容"
          />
          <button
            type="submit"
            className="ask-send-btn"
            disabled={loading || !input.trim()}
            aria-label="发送问题"
          >
            <Send size={16} />
          </button>
        </form>
      </div>
    </div>
  );
}
