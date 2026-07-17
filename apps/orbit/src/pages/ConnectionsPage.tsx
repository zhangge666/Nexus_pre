/** 本文件实现 Orbit 的已授权应用列表，仅呈现本地 Memory Protocol 的真实连接数据。 */
import type React from "react";
import { useCallback, useEffect, useState } from "react";
import { Link2, RefreshCw, ShieldCheck, Trash2 } from "lucide-react";
import { listConnectedApps, revokeApp } from "../core";
import type { ConnectedApp } from "../core";
import { EmptyState } from "../components/EmptyState";
import { useInspector } from "../components/Inspector";
import { PageLayout } from "../components/PageLayout";
import { Topbar } from "../components/Topbar";

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
    admin: "管理权限",
  };
  return labels[scope] ?? scope;
}

/** 显示连接页的低干扰说明，避免虚构同步、设备或导出能力。 */
function ConnectionsInspector({ appCount }: { appCount: number }): React.JSX.Element {
  return (
    <div className="today-inspector">
      <section className="inspector-section">
        <div className="review-inspector-title"><ShieldCheck size={14} className="success-color" />本地授权</div>
        <p className="inspector-answer">这里仅显示已通过本地 Memory Protocol 授权的应用。撤销授权后，该应用将无法继续访问你的记忆服务。</p>
      </section>
      <section className="inspector-section">
        <div className="review-inspector-title">当前状态</div>
        <div className="review-detail-grid">
          <div className="review-detail-row"><span>已授权应用</span><strong>{appCount} 个</strong></div>
          <div className="review-detail-row"><span>连接范围</span><strong>本机</strong></div>
        </div>
      </section>
    </div>
  );
}

/** 渲染已连接应用，并提供重新读取和撤销本地授权两个真实操作。 */
export default function ConnectionsPage(): React.JSX.Element {
  const { present } = useInspector();
  const [apps, setApps] = useState<ConnectedApp[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [revokingTokenId, setRevokingTokenId] = useState<string | null>(null);

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
  useEffect(() => { present("连接说明", <ConnectionsInspector appCount={apps.length} />); }, [apps.length, present]);

  /** 撤销指定应用的令牌，并在成功后直接从当前列表中移除。 */
  async function handleRevoke(app: ConnectedApp): Promise<void> {
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
        title="连接"
        subtitle="管理可访问本地记忆服务的应用"
        actions={<button className="secondary-button" onClick={() => void load()} disabled={loading || revokingTokenId !== null}><RefreshCw size={14} />刷新</button>}
      />
      <div className="connections-content">
        {error && <p className="inline-notice" role="alert">{error} <button className="detail-action" onClick={() => void load()}>重试</button></p>}
        {loading ? (
          <div className="connections-loading">正在读取已授权应用…</div>
        ) : apps.length === 0 ? (
          <EmptyState icon={<Link2 size={36} />} title="尚无已授权应用" description="启动 Muse 或其他 Nexus 应用并完成连接后，它们会显示在这里。" />
        ) : (
          <section className="connection-list" aria-label="已授权应用">
            <div className="connection-list-heading"><span>应用</span><span>权限范围</span><span>最近活动</span><span>操作</span></div>
            {apps.map((app) => (
              <article className="connection-row" key={app.tokenId}>
                <div className="connection-app"><span className="connection-source">{app.source.slice(0, 1).toUpperCase()}</span><div><strong>{app.name}</strong><span>{app.source} · 已写入 {app.memoriesCount} 条记忆</span></div></div>
                <div className="connection-scopes">{app.scopes.length ? app.scopes.map((scope) => <span key={scope}>{formatScope(scope)}</span>) : <span>未声明</span>}</div>
                <time dateTime={new Date(app.lastActiveAt).toISOString()}>{formatRelativeTime(app.lastActiveAt)}</time>
                <button className="detail-action connection-revoke" onClick={() => void handleRevoke(app)} disabled={revokingTokenId !== null}>{revokingTokenId === app.tokenId ? "撤销中…" : <><Trash2 size={13} />撤销授权</>}</button>
              </article>
            ))}
          </section>
        )}
      </div>
    </PageLayout>
  );
}
