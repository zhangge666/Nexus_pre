/** 本文件渲染 M3 范围内唯一的 Muse 文字输入、连接状态与提交反馈。 */
import React, { FormEvent, KeyboardEvent, useEffect, useRef, useState } from "react";
import { Check, Lightbulb, Link2, RefreshCw, Send } from "lucide-react";
import { connectService, getConnectionStatus, isTauriRuntime, submitIdea, type ConnectionStatus } from "./api";
import "./muse.css";

type ViewState = ConnectionStatus["state"] | "connecting";
type SubmitState = "idle" | "submitting" | "success" | "error";

/** 把未知 IPC 错误归一为可直接展示的中文提示。 */
function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** 渲染单一文字来源适配器，并在失败时保留原始输入供重试。 */
export function App(): React.JSX.Element {
  const [content, setContent] = useState("");
  const [connection, setConnection] = useState<ConnectionStatus>({
    state: "disconnected",
    endpoint: null,
    message: "正在检查 Orbit 本地服务…",
  });
  const [viewState, setViewState] = useState<ViewState>("connecting");
  const [submitState, setSubmitState] = useState<SubmitState>("idle");
  const [feedback, setFeedback] = useState("连接 Orbit 后即可写入共享记忆库。");
  const [connectionAttempted, setConnectionAttempted] = useState(false);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  /** 连接服务并把登记或发现失败映射到可重试的原位状态。 */
  async function reconnect(): Promise<void> {
    setConnectionAttempted(true);
    if (!isTauriRuntime()) {
      setViewState("disconnected");
      setFeedback("浏览器仅用于界面预览；请启动 Muse 桌面应用连接 Orbit。");
      return;
    }
    setViewState("connecting");
    setFeedback("正在发现 Orbit 并申请 memory:write 授权…");
    try {
      const status = await connectService();
      setConnection(status);
      setViewState(status.state);
      setFeedback("已连接 Orbit，本次写入将标记为 Muse · idea。");
      inputRef.current?.focus();
    } catch (error) {
      const message = errorMessage(error);
      setConnection({ state: "disconnected", endpoint: null, message });
      setViewState("disconnected");
      setFeedback(message);
    }
  }

  useEffect(() => {
    if (!isTauriRuntime()) {
      setViewState("disconnected");
      setFeedback("浏览器仅用于界面预览；请启动 Muse 桌面应用连接 Orbit。");
      return;
    }
    void getConnectionStatus()
      .then((status) => {
        if (status.state === "connected") {
          setConnection(status);
          setViewState("connected");
          setFeedback("已连接 Orbit，本次写入将标记为 Muse · idea。");
        } else {
          setConnection(status);
          setViewState("disconnected");
          setFeedback("点击“连接 Orbit”以登记 source=muse 的最小写入授权。");
        }
      })
      .catch((error) => {
        setViewState("disconnected");
        setFeedback(errorMessage(error));
      });
  }, []);

  /** 提交当前文字；成功后清空输入，失败时保留草稿并提供再次提交。 */
  async function handleSubmit(event?: FormEvent): Promise<void> {
    event?.preventDefault();
    if (content.trim().length === 0 || viewState !== "connected" || submitState === "submitting") return;
    setSubmitState("submitting");
    setFeedback("正在通过 Memory Protocol 写入…");
    try {
      const created = await submitIdea(content);
      setContent("");
      setSubmitState("success");
      setFeedback(`已写入 Orbit · ${created.id.slice(0, 8)}…`);
      inputRef.current?.focus();
    } catch (error) {
      const message = errorMessage(error);
      setSubmitState("error");
      setFeedback(message);
      if (message.includes("授权") || message.includes("连接")) {
        setConnection({ state: "disconnected", endpoint: null, message });
        setViewState("disconnected");
      }
    }
  }

  /** 支持 Ctrl/Cmd+Enter 提交，同时保留普通回车输入多行文字。 */
  function handleKeyDown(event: KeyboardEvent<HTMLTextAreaElement>): void {
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      void handleSubmit();
    }
  }

  const connected = viewState === "connected";
  const statusLabel = viewState === "connected" ? "已连接" : viewState === "connecting" ? "连接中" : "未连接";

  return (
    <main className="muse-shell">
      <section className="muse-panel" aria-labelledby="muse-title">
        <header className="muse-header">
          <div className="muse-brand" aria-hidden="true"><Lightbulb size={16} /></div>
          <div>
            <h1 id="muse-title">Muse</h1>
            <p>M3 · 最小文字来源</p>
          </div>
          <span className={`connection-badge is-${viewState}`}>
            <span className="connection-dot" />{statusLabel}
          </span>
        </header>

        <div className="connection-row">
          <Link2 size={14} aria-hidden="true" />
          <span className="connection-copy">{connection.endpoint ?? "等待 Orbit 本地服务"}</span>
          {!connected && (
            <button className="retry-button" type="button" onClick={() => void reconnect()} disabled={viewState === "connecting"}>
              <RefreshCw size={13} aria-hidden="true" />
              {viewState === "connecting" ? "连接中" : connectionAttempted ? "重试连接" : "连接 Orbit"}
            </button>
          )}
        </div>

        <form className="capture-form" onSubmit={(event) => void handleSubmit(event)}>
          <label htmlFor="muse-content">记录一个想法</label>
          <textarea
            id="muse-content"
            ref={inputRef}
            value={content}
            onChange={(event) => {
              setContent(event.target.value);
              if (submitState !== "idle") setSubmitState("idle");
            }}
            onKeyDown={handleKeyDown}
            placeholder="输入文字灵感…"
            maxLength={10_000}
            disabled={!connected}
          />
          <div className="form-actions">
            <span className={`feedback is-${submitState}`} role="status" aria-live="polite">
              {submitState === "success" && <Check size={13} aria-hidden="true" />}{feedback}
            </span>
            <button className="submit-button" type="submit" disabled={!connected || content.trim().length === 0 || submitState === "submitting"}>
              <Send size={14} aria-hidden="true" />
              {submitState === "submitting" ? "写入中…" : submitState === "error" ? "再次提交" : "写入 Orbit"}
            </button>
          </div>
        </form>

        <footer className="scope-footer">
          <span>source: muse</span><span>kind: idea</span><span>scope: memory:write</span><kbd>Ctrl ↵</kbd>
        </footer>
      </section>
    </main>
  );
}
