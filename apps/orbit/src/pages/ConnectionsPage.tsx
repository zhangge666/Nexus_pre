/** 本文件实现 Orbit 的连接与隐私面板，包括第三方授权、数据流向审计和令牌撤销。 */
import type React from "react";
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  ArrowDownToLine,
  ArrowUpFromLine,
  Clipboard,
  CloudOff,
  KeyRound,
  Link2,
  Plus,
  RefreshCw,
  ShieldCheck,
  Trash2,
} from "lucide-react";
import { listConnectedApps, registerExternalApp, revokeApp } from "../core";
import type { ConnectedApp, RegisteredConnection } from "../core";
import { EmptyState } from "../components/EmptyState";
import { useInspector } from "../components/Inspector";
import { Modal } from "../components/Modal";
import { PageLayout } from "../components/PageLayout";
import { Topbar } from "../components/Topbar";

const EXTERNAL_SCOPES = [
  { id: "memory:read", label: "读取记忆", description: "按 ID 或列表读取授权可见的记忆" },
  { id: "memory:write", label: "写入记忆", description: "仅能写入和更新该应用自己的 external:* 来源" },
  { id: "memory:delete", label: "删除记忆", description: "仅能删除该应用自己的来源记忆" },
  { id: "search", label: "检索", description: "在授权可见范围执行关键词、语义和混合检索" },
  { id: "subscribe", label: "订阅事件", description: "接收记忆创建、更新和删除事件" },
] as const;

/** 将活动时间转换为紧凑的中文相对时间。 */
function formatRelativeTime(timestamp: number): string {
  const seconds = Math.max(0, Math.floor((Date.now() - timestamp) / 1_000));
  if (seconds < 60) return "刚刚";
  if (seconds < 3_600) return `${Math.floor(seconds / 60)} 分钟前`;
  if (seconds < 86_400) return `${Math.floor(seconds / 3_600)} 小时前`;
  return `${Math.floor(seconds / 86_400)} 天前`;
}

/** 将协议权限转换为便于辨识的中文说明。 */
function formatScope(scope: string): string {
  const labels: Record<string, string> = {
    "memory:read": "读取记忆",
    "memory:write": "写入记忆",
    "memory:delete": "删除记忆",
    search: "检索",
    subscribe: "订阅事件",
    review: "复习",
    admin: "管理权限",
  };
  return labels[scope] ?? scope;
}

/** 概括某个令牌允许的数据流向。 */
function dataFlowLabel(app: ConnectedApp): string {
  const reads = app.scopes.some((scope) => ["memory:read", "search", "subscribe"].includes(scope));
  const writes = app.scopes.some((scope) => ["memory:write", "memory:delete"].includes(scope));
  if (reads && writes) return "双向";
  if (writes) return "流入 Orbit";
  if (reads) return "流出 Orbit";
  return "无数据访问";
}

/** 显示连接页的本地边界与累计访问摘要。 */
function ConnectionsInspector({ apps }: { apps: ConnectedApp[] }): React.JSX.Element {
  const totals = apps.reduce(
    (current, app) => ({
      reads: current.reads + app.readCount,
      writes: current.writes + app.writeCount,
    }),
    { reads: 0, writes: 0 },
  );
  return (
    <div className="today-inspector">
      <section className="inspector-section">
        <div className="review-inspector-title"><ShieldCheck size={14} className="success-color" />本地授权边界</div>
        <p className="inspector-answer">每个第三方令牌都绑定独立来源与显式 scope。Memory Protocol 只监听本机回环地址，不会自行把记忆发送到远程网络。</p>
      </section>
      <section className="inspector-section">
        <div className="review-inspector-title">访问审计</div>
        <div className="review-detail-grid">
          <div className="review-detail-row"><span>已授权应用</span><strong>{apps.length} 个</strong></div>
          <div className="review-detail-row"><span>读取请求</span><strong>{totals.reads}</strong></div>
          <div className="review-detail-row"><span>写入请求</span><strong>{totals.writes}</strong></div>
          <div className="review-detail-row"><span>网络边界</span><strong>仅本机</strong></div>
        </div>
      </section>
    </div>
  );
}

/** 渲染第三方授权表单或只展示一次的令牌。 */
function GrantModal({
  open,
  grant,
  creating,
  error,
  appId,
  name,
  scopes,
  onAppIdChange,
  onNameChange,
  onToggleScope,
  onCreate,
  onClose,
}: {
  open: boolean;
  grant: RegisteredConnection | null;
  creating: boolean;
  error: string | null;
  appId: string;
  name: string;
  scopes: string[];
  onAppIdChange: (value: string) => void;
  onNameChange: (value: string) => void;
  onToggleScope: (scope: string) => void;
  onCreate: () => void;
  onClose: () => void;
}): React.JSX.Element {
  const [copyState, setCopyState] = useState("复制令牌");

  /** 把敏感令牌复制到剪贴板，不在页面关闭后保留副本。 */
  async function copyToken(): Promise<void> {
    if (!grant) return;
    await navigator.clipboard.writeText(grant.token);
    setCopyState("已复制");
  }

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={grant ? "保存访问令牌" : "授权第三方应用"}
      width="560px"
      footer={grant ? (
        <button className="primary-small" onClick={onClose}>我已安全保存</button>
      ) : (
        <>
          <button className="secondary-button" onClick={onClose} disabled={creating}>取消</button>
          <button className="primary-small" onClick={onCreate} disabled={creating || scopes.length === 0}>
            <KeyRound size={14} />{creating ? "创建中…" : "创建授权"}
          </button>
        </>
      )}
    >
      {grant ? (
        <div className="connection-token-result">
          <div className="connection-token-warning">
            <ShieldCheck size={16} />
            <p><strong>令牌只显示这一次</strong><span>关闭窗口前将它保存到目标 SDK、MCP 客户端或本机宿主的安全配置中。</span></p>
          </div>
          <div className="connection-token-value">
            <code>{grant.token}</code>
            <button className="secondary-button" onClick={() => void copyToken()}><Clipboard size={14} />{copyState}</button>
          </div>
          <dl className="connection-grant-facts">
            <div><dt>来源</dt><dd>{grant.source}</dd></div>
            <div><dt>权限</dt><dd>{grant.scopes.map(formatScope).join("、")}</dd></div>
          </dl>
        </div>
      ) : (
        <div className="connection-grant-form">
          <p className="form-help">只为你信任的本机程序创建授权。第三方应用不能申请管理或复习权限，写入操作固定在自己的来源内。</p>
          <label className="connection-field">
            <span>应用名称</span>
            <input className="settings-input" value={name} maxLength={80} autoFocus onChange={(event) => onNameChange(event.target.value)} placeholder="例如 Claude MCP" />
          </label>
          <label className="connection-field">
            <span>应用标识</span>
            <input className="settings-input" value={appId} maxLength={80} onChange={(event) => onAppIdChange(event.target.value.toLowerCase())} placeholder="例如 mcp" />
            <small>将固定生成来源 <code>external:{appId || "<app_id>"}</code></small>
          </label>
          <fieldset className="connection-scope-picker">
            <legend>最小权限</legend>
            {EXTERNAL_SCOPES.map((scope) => (
              <label key={scope.id}>
                <input type="checkbox" checked={scopes.includes(scope.id)} onChange={() => onToggleScope(scope.id)} />
                <span><strong>{scope.label}</strong><small>{scope.description}</small></span>
              </label>
            ))}
          </fieldset>
          {error && <p className="form-error" role="alert">{error}</p>}
        </div>
      )}
    </Modal>
  );
}

/** 渲染连接与隐私治理页面。 */
export default function ConnectionsPage(): React.JSX.Element {
  const { present } = useInspector();
  const [apps, setApps] = useState<ConnectedApp[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [revokingTokenId, setRevokingTokenId] = useState<string | null>(null);
  const [grantOpen, setGrantOpen] = useState(false);
  const [grant, setGrant] = useState<RegisteredConnection | null>(null);
  const [creating, setCreating] = useState(false);
  const [grantError, setGrantError] = useState<string | null>(null);
  const [appId, setAppId] = useState("");
  const [name, setName] = useState("");
  const [scopes, setScopes] = useState<string[]>(["memory:write"]);

  const privacySummary = useMemo(
    () => apps.every((app) => !app.sendsDataRemote) ? "Memory Protocol 数据边界：仅本机" : "存在远程数据流",
    [apps],
  );

  /** 读取协议服务已登记的应用连接。 */
  const load = useCallback(async (): Promise<void> => {
    setLoading(true);
    setError(null);
    try {
      setApps(await listConnectedApps());
    } catch (reason) {
      setError(`连接列表加载失败：${String(reason)}`);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { void load(); }, [load]);
  useEffect(() => { present("连接与隐私", <ConnectionsInspector apps={apps} />); }, [apps, present]);

  /** 打开一条全新的授权流程，避免上次显示的令牌残留。 */
  function openGrant(): void {
    setGrant(null);
    setGrantError(null);
    setAppId("");
    setName("");
    setScopes(["memory:write"]);
    setGrantOpen(true);
  }

  /** 关闭授权窗口并立即清除页面中的敏感令牌。 */
  function closeGrant(): void {
    if (creating) return;
    setGrantOpen(false);
    setGrant(null);
  }

  /** 切换一个第三方最小权限。 */
  function toggleScope(scope: string): void {
    setScopes((current) => current.includes(scope)
      ? current.filter((item) => item !== scope)
      : [...current, scope]);
  }

  /** 调用持有者接口创建授权，并在成功后刷新审计列表。 */
  async function createGrant(): Promise<void> {
    if (!name.trim()) {
      setGrantError("请输入应用名称");
      return;
    }
    if (!/^[a-z0-9][a-z0-9._-]{0,79}$/.test(appId)) {
      setGrantError("应用标识必须以小写字母或数字开头，且只能包含小写字母、数字、点、短横线和下划线");
      return;
    }
    setCreating(true);
    setGrantError(null);
    try {
      setGrant(await registerExternalApp(appId, name.trim(), scopes));
      await load();
    } catch (reason) {
      setGrantError(`授权创建失败：${String(reason)}`);
    } finally {
      setCreating(false);
    }
  }

  /** 撤销指定应用的令牌，并在成功后直接从当前列表中移除。 */
  async function handleRevoke(app: ConnectedApp): Promise<void> {
    if (!window.confirm(`撤销“${app.name}”的访问令牌？该应用将立即失去访问权限。`)) return;
    setRevokingTokenId(app.tokenId);
    setError(null);
    try {
      await revokeApp(app.tokenId);
      setApps((current) => current.filter((item) => item.tokenId !== app.tokenId));
    } catch (reason) {
      setError(`撤销“${app.name}”授权失败：${String(reason)}`);
    } finally {
      setRevokingTokenId(null);
    }
  }

  return (
    <PageLayout className="connections-page">
      <Topbar
        title="连接与隐私"
        subtitle={privacySummary}
        actions={(
          <>
            <button className="secondary-button" onClick={() => void load()} disabled={loading || revokingTokenId !== null}><RefreshCw size={14} />刷新</button>
            <button className="primary-small" onClick={openGrant}><Plus size={14} />授权应用</button>
          </>
        )}
      />
      <div className="connections-content">
        <div className="connection-boundary-note"><CloudOff size={15} /><span><strong>本地边界</strong> 服务仅监听回环地址；令牌正文不会出现在连接列表或日志中。</span></div>
        {error && <p className="inline-notice" role="alert">{error} <button className="detail-action" onClick={() => void load()}>重试</button></p>}
        {loading ? (
          <div className="connections-loading">正在读取已授权应用…</div>
        ) : apps.length === 0 ? (
          <EmptyState icon={<Link2 size={36} />} title="尚无已授权应用" description="为 MCP、SDK 或浏览器剪藏器创建一条最小权限授权。" action={{ label: "授权应用", onClick: openGrant }} />
        ) : (
          <section className="connection-list" aria-label="已授权应用">
            <div className="connection-list-heading"><span>应用</span><span>数据流向</span><span>权限范围</span><span>最近活动</span><span>操作</span></div>
            {apps.map((app) => (
              <article className="connection-row" key={app.tokenId}>
                <div className="connection-app"><span className="connection-source">{app.name.slice(0, 1).toUpperCase()}</span><div><strong>{app.name}</strong><span>{app.source} · {app.memoriesCount} 条来源记忆</span></div></div>
                <div className="connection-flow">
                  <strong>{dataFlowLabel(app)}</strong>
                  <span><ArrowDownToLine size={11} />{app.writeCount}<ArrowUpFromLine size={11} />{app.readCount}</span>
                </div>
                <div className="connection-scopes">{app.scopes.length ? app.scopes.map((scope) => <span key={scope}>{formatScope(scope)}</span>) : <span>未声明</span>}</div>
                <time dateTime={new Date(app.lastActiveAt).toISOString()} title={`授权于 ${new Date(app.createdAt).toLocaleString()}`}>{formatRelativeTime(app.lastActiveAt)}{app.lastScope && <small>{formatScope(app.lastScope)}</small>}</time>
                <button className="detail-action connection-revoke" onClick={() => void handleRevoke(app)} disabled={revokingTokenId !== null}>{revokingTokenId === app.tokenId ? "撤销中…" : <><Trash2 size={13} />撤销</>}</button>
              </article>
            ))}
          </section>
        )}
      </div>
      <GrantModal
        open={grantOpen}
        grant={grant}
        creating={creating}
        error={grantError}
        appId={appId}
        name={name}
        scopes={scopes}
        onAppIdChange={setAppId}
        onNameChange={setName}
        onToggleScope={toggleScope}
        onCreate={() => void createGrant()}
        onClose={closeGrant}
      />
    </PageLayout>
  );
}
