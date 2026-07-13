/** 本文件实现记忆问答（RAG）页面，支持流式输出和引用溯源。 */
import type React from "react";
import { useState, useRef, useEffect, FormEvent } from "react";
import { Send, MessageCircle, AlertTriangle } from "lucide-react";
import { askMemory } from "../core";
import type { ChatMessage, Citation } from "../core";
import { Topbar } from "../components/Topbar";
import { EmptyState } from "../components/EmptyState";

function CitationCard({ citation }: { citation: Citation }): React.JSX.Element {
  const kindIcon: Record<string, string> = {
    idea: "💡", note: "📝", screen: "🖥", voice: "🎤", card: "🃏", clip: "📎", file: "📄",
  };
  return (
    <div className="citation-card">
      <span className="citation-icon">{kindIcon[citation.sourceKind ?? "note"] ?? "📎"}</span>
      <div className="citation-body">
        <strong>{citation.sourceTitle ?? citation.memoryId}</strong>
        <p>{citation.snippet}</p>
      </div>
    </div>
  );
}

function ChatBubble({ msg }: { msg: ChatMessage }): React.JSX.Element {
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
                <CitationCard key={c.blockId} citation={c} />
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
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [streamText, setStreamText] = useState("");
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streamText]);

  async function handleSubmit(e: FormEvent): Promise<void> {
    e.preventDefault();
    const question = input.trim();
    if (!question || loading) return;

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
      // 模拟流式输出（每隔 30ms 追加一个字符）
      const response = await askMemory({ question });
      let displayed = "";
      for (const char of response.answer) {
        displayed += char;
        setStreamText(displayed);
        await new Promise((r) => setTimeout(r, 18));
      }
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
          <ChatBubble key={msg.id} msg={msg} />
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
          本次问答将使用本地 Mock，真实环境下会调用你配置的 AI Provider 处理检索片段
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
