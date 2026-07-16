/** 本文件实现 Orbit 连接与隐私设置中心，集成已连接应用管理、加密同步状态及安全体检检查器。 */
import React, { useState, useEffect } from "react";
import {
  Link2, Trash2, ShieldCheck, Download, Key, Monitor,
  BookOpen, Lock, Sparkles, RefreshCw, KeyRound, CheckCircle,
  Smartphone, Laptop, HelpCircle, Eye, EyeOff
} from "lucide-react";
import { listConnectedApps, revokeApp } from "../core";
import type { ConnectedApp } from "../core";
import { Topbar } from "../components/Topbar";
import { useInspector } from "../components/Inspector";
import { PageLayout } from "../components/PageLayout";

/** 将时间戳转换为连接列表使用的紧凑相对时间。 */
function formatRelTime(ts: number): string {
  const diff = Date.now() - ts;
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`;
  return `${Math.floor(diff / 86_400_000)}d ago`;
}

/** 渲染连接与隐私摘要，作为全局检查器内容而不是页面内的固定栏位。 */
function ConnectionsInspector({ appCount }: { appCount: number }): React.JSX.Element {
  return (
    <div className="today-inspector">
      <section className="inspector-section"><div className="review-inspector-title"><ShieldCheck size={14} className="success-color" /><span>隐私状态</span></div><div className="review-detail-grid"><div className="review-detail-row"><span>隐私评分</span><strong>96 / 100</strong></div><div className="review-detail-row"><span>端到端加密</span><strong>已启用</strong></div><div className="review-detail-row"><span>已连接来源</span><strong>{appCount} 个</strong></div></div></section>
      <section className="inspector-section"><div className="review-inspector-title">安全建议</div><p className="inspector-answer">请将恢复短语离线保存，并定期撤销不再使用的应用授权。</p></section>
    </div>
  );
}

/** 渲染连接与隐私管理工作区，并将辅助信息交给全局检查器。 */
export default function ConnectionsPage(): React.JSX.Element {
  const { present } = useInspector();
  const [apps, setApps] = useState<ConnectedApp[]>([]);
  const [syncMode, setSyncMode] = useState<"local" | "e2e_cloud" | "self_hosted">("e2e_cloud");
  const [loading, setLoading] = useState(true);
  const [revoking, setRevoking] = useState<string | null>(null);
  const [showPhrase, setShowPhrase] = useState(false);

  useEffect(() => {
    void listConnectedApps().then((a) => {
      setApps(a);
      setLoading(false);
    });
  }, []);

  useEffect(() => {
    present("连接与隐私", <ConnectionsInspector appCount={apps.length} />);
  }, [apps.length, present]);

  /** 撤销指定应用的访问令牌，并在成功后同步更新本地列表。 */
  async function handleRevoke(app: ConnectedApp): Promise<void> {
    if (!confirm(`确认撤销「${app.name}」的访问权限？`)) return;
    setRevoking(app.tokenId);
    try {
      await revokeApp(app.tokenId);
      setApps((as) => as.filter((a) => a.tokenId !== app.tokenId));
    } finally {
      setRevoking(null);
    }
  }

  return (
    <PageLayout className="connections-page">
      <Topbar
        title="Connections & Privacy"
        subtitle="Control how Orbit connects, syncs, and protects your data."
        actions={
          <button className="secondary-button">
            <BookOpen size={14} />Learn more
          </button>
        }
      />

        {/* 1. 顶部统计指标卡片 */}
        <section className="stats-cards-grid" aria-label="隐私摘要指标">
          <div className="dashboard-stat-card">
            <div className="stat-card-header">
              <span>CONNECTED APPS</span>
              <Link2 size={12} style={{ color: "hsl(var(--primary))" }} />
            </div>
            <div className="stat-card-value">
              {apps.length}<span>sources</span>
            </div>
            <div className="stat-card-footer success">
              +1 new this week
            </div>
          </div>

          <div className="dashboard-stat-card">
            <div className="stat-card-header">
              <span>ACTIVE TOKENS</span>
              <Key size={12} style={{ color: "hsl(var(--warning))" }} />
            </div>
            <div className="stat-card-value">
              7<span>tokens</span>
            </div>
            <div className="stat-card-footer success">
              All healthy
            </div>
          </div>

          <div className="dashboard-stat-card">
            <div className="stat-card-header">
              <span>SYNC MODE</span>
              <ShieldCheck size={12} style={{ color: "hsl(var(--primary-hover))" }} />
            </div>
            <div className="stat-card-value" style={{ fontSize: "16px" }}>
              E2E cloud
            </div>
            <div className="stat-card-footer success">
              Encrypted & synced
            </div>
          </div>

          <div className="dashboard-stat-card">
            <div className="stat-card-header">
              <span>PROTECTED BY E2E</span>
              <Lock size={12} style={{ color: "hsl(var(--success))" }} />
            </div>
            <div className="stat-card-value">
              100%
            </div>
            <div className="stat-card-footer success">
              End-to-end encrypted
            </div>
          </div>
        </section>

        {/* 2. 已连接应用表格 */}
        <section className="dashboard-stat-card" style={{ padding: 0 }} aria-label="数据连接列表">
          <div className="section-heading" style={{ height: "42px", padding: "0 16px" }}>
            <h2 style={{ fontSize: "13px" }}>Connected sources</h2>
          </div>
          {loading ? (
            <p className="loading-hint">Loading sources...</p>
          ) : (
            <div className="conn-table-scroll">
              <table className="conn-sources-table">
              <thead>
                <tr>
                  <th>Source</th>
                  <th>Scopes</th>
                  <th>Last active</th>
                  <th>Sync status</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                {apps.map((app) => (
                  <tr key={app.id}>
                    <td>
                      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                        <span className={`source-mark src-${app.source.split(":")[0]}`} style={{ width: "20px", height: "20px", fontSize: "10px" }}>
                          {app.source.startsWith("echo") ? "🖥" : app.source.startsWith("muse") ? "💡" : "📝"}
                        </span>
                        <div>
                          <strong style={{ display: "block", color: "hsl(var(--foreground))" }}>{app.name}</strong>
                          <span style={{ fontSize: "10px", color: "hsl(var(--foreground-muted))" }}>{app.source}</span>
                        </div>
                      </div>
                    </td>
                    <td>
                      <div style={{ display: "flex", gap: "3px", flexWrap: "wrap" }}>
                        {app.scopes.map(s => (
                          <em key={s} className="scope-badge" style={{ background: "hsl(var(--muted))", fontStyle: "normal", fontSize: "9px", padding: "1px 4px", borderRadius: "3px" }}>{s}</em>
                        ))}
                      </div>
                    </td>
                    <td style={{ color: "hsl(var(--foreground-secondary))" }}>
                      {formatRelTime(app.lastActiveAt)}
                    </td>
                    <td>
                      <span style={{ display: "inline-flex", alignItems: "center", gap: "6px", color: "hsl(var(--foreground-secondary))" }}>
                        <span className="status-dot success" style={{ width: "6px", height: "6px", borderRadius: "50%", background: "hsl(var(--success))" }} />
                        Synced
                      </span>
                    </td>
                    <td>
                      <div style={{ display: "flex", gap: "6px" }}>
                        <button className="secondary-button" style={{ height: "24px", fontSize: "11px" }}>Manage</button>
                        <button
                          className="icon-button danger-hover"
                          onClick={() => void handleRevoke(app)}
                          disabled={revoking === app.tokenId}
                        >
                          <Trash2 size={13} />
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
              </table>
            </div>
          )}
          <div style={{ padding: "12px", borderTop: "1px solid hsl(var(--border-subtle))" }}>
            <button className="detail-action primary-action" style={{ fontWeight: 500, fontSize: "12px" }}>
              + Connect a new source
            </button>
          </div>
        </section>

        {/* 3. 同步、密钥与导出设置网格 */}
        <section className="connections-settings-grid">
          {/* 同步模式 */}
          <div className="dashboard-stat-card" style={{ gap: "12px" }}>
            <div className="review-inspector-title">Sync mode</div>
            <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
              <label style={{ display: "flex", alignItems: "center", gap: "10px", padding: "8px 12px", background: syncMode === "e2e_cloud" ? "hsl(var(--surface-active))" : "hsl(var(--background))", border: "1px solid hsl(var(--border))", borderRadius: "var(--radius)", cursor: "pointer" }}>
                <input type="radio" name="sync" checked={syncMode === "e2e_cloud"} onChange={() => setSyncMode("e2e_cloud")} />
                <div>
                  <strong style={{ fontSize: "12px", display: "block" }}>E2E cloud <span className="scope-badge" style={{ background: "hsl(var(--success) / 0.12)", color: "hsl(var(--success))", fontSize: "8px" }}>Recommended</span></strong>
                  <span style={{ fontSize: "10px", color: "hsl(var(--foreground-muted))" }}>End-to-end encrypted and synced across devices.</span>
                </div>
              </label>
              <label style={{ display: "flex", alignItems: "center", gap: "10px", padding: "8px 12px", background: syncMode === "local" ? "hsl(var(--surface-active))" : "hsl(var(--background))", border: "1px solid hsl(var(--border))", borderRadius: "var(--radius)", cursor: "pointer" }}>
                <input type="radio" name="sync" checked={syncMode === "local"} onChange={() => setSyncMode("local")} />
                <div>
                  <strong style={{ fontSize: "12px", display: "block" }}>Local only</strong>
                  <span style={{ fontSize: "10px", color: "hsl(var(--foreground-muted))" }}>Keep all data on this device. No cloud sync.</span>
                </div>
              </label>
              <label style={{ display: "flex", alignItems: "center", gap: "10px", padding: "8px 12px", background: syncMode === "self_hosted" ? "hsl(var(--surface-active))" : "hsl(var(--background))", border: "1px solid hsl(var(--border))", borderRadius: "var(--radius)", cursor: "pointer" }}>
                <input type="radio" name="sync" checked={syncMode === "self_hosted"} onChange={() => setSyncMode("self_hosted")} />
                <div>
                  <strong style={{ fontSize: "12px", display: "block" }}>Self-hosted</strong>
                  <span style={{ fontSize: "10px", color: "hsl(var(--foreground-muted))" }}>Host Orbit sync on your own server mid-tier.</span>
                </div>
              </label>
            </div>
            <span style={{ fontSize: "10px", color: "hsl(var(--foreground-disabled))" }}>All data is encrypted on your device before it leaves your machine. <span style={{ color: "hsl(var(--primary-hover))", cursor: "pointer" }}>How Orbit encryption works</span></span>
          </div>

          {/* 密钥与配对设备 */}
          <div className="dashboard-stat-card" style={{ gap: "12px" }}>
            <div className="review-inspector-title">Recovery phrase & devices</div>
            <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
              <div style={{ background: "hsl(var(--background))", border: "1px solid hsl(var(--border))", borderRadius: "var(--radius)", padding: "10px 12px", display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                <div>
                  <strong style={{ fontSize: "11px", display: "block", color: "hsl(var(--foreground-muted))" }}>RECOVERY PHRASE</strong>
                  <span style={{ fontSize: "12px", fontFamily: "monospace", letterSpacing: "1px" }}>
                    {showPhrase ? "alpha bravo charlie delta echo" : "•••• •••• •••• •••• ••••"}
                  </span>
                </div>
                <button className="icon-button" onClick={() => setShowPhrase(!showPhrase)}>
                  {showPhrase ? <EyeOff size={14} /> : <Eye size={14} />}
                </button>
              </div>
              <button className="secondary-button" style={{ justifyContent: "center" }}>View recovery phrase</button>

              <div style={{ borderTop: "1px solid hsl(var(--border-subtle))", paddingTop: "8px", marginTop: "4px" }}>
                <strong style={{ fontSize: "10px", color: "hsl(var(--foreground-disabled))", display: "block", marginBottom: "6px" }}>PAIRED DEVICES (3)</strong>
                <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
                  <div style={{ display: "flex", justifyContent: "space-between", fontSize: "11px" }}>
                    <span style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}><Laptop size={11} /> MacBook Pro (This device)</span>
                    <span style={{ color: "hsl(var(--success))" }}>● Active now</span>
                  </div>
                  <div style={{ display: "flex", justifyContent: "space-between", fontSize: "11px" }}>
                    <span style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}><Smartphone size={11} /> iPhone 15 Pro</span>
                    <span style={{ color: "hsl(var(--foreground-disabled))" }}>2h ago</span>
                  </div>
                </div>
              </div>
            </div>
            <button className="secondary-button" style={{ alignSelf: "flex-start", height: "24px", fontSize: "11px" }}>Manage devices</button>
          </div>
        </section>

        {/* 数据导出 */}
        <section className="dashboard-stat-card" style={{ gap: "10px" }}>
          <div className="review-inspector-title" style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <span>Data export</span>
            <span style={{ fontSize: "11px", color: "hsl(var(--primary-hover))", cursor: "pointer" }}>View export history</span>
          </div>
          <div className="connections-export-grid">
            <div style={{ border: "1px solid hsl(var(--border))", padding: "10px", borderRadius: "var(--radius)", background: "hsl(var(--background))", display: "flex", flexDirection: "column", justifyContent: "space-between", minHeight: "80px" }}>
              <span style={{ fontSize: "11px", fontWeight: 600 }}>Export all data</span>
              <span style={{ fontSize: "9px", color: "hsl(var(--foreground-muted))" }}>All accounts, cards, and tags.</span>
              <button className="icon-button" style={{ alignSelf: "flex-end" }}><Download size={12} /></button>
            </div>
            <div style={{ border: "1px solid hsl(var(--border))", padding: "10px", borderRadius: "var(--radius)", background: "hsl(var(--background))", display: "flex", flexDirection: "column", justifyContent: "space-between", minHeight: "80px" }}>
              <span style={{ fontSize: "11px", fontWeight: 600 }}>Export cards only</span>
              <span style={{ fontSize: "9px", color: "hsl(var(--foreground-muted))" }}>Export your decks and references.</span>
              <button className="icon-button" style={{ alignSelf: "flex-end" }}><Download size={12} /></button>
            </div>
            <div style={{ border: "1px solid hsl(var(--border))", padding: "10px", borderRadius: "var(--radius)", background: "hsl(var(--background))", display: "flex", flexDirection: "column", justifyContent: "space-between", minHeight: "80px" }}>
              <span style={{ fontSize: "11px", fontWeight: 600 }}>Export memories</span>
              <span style={{ fontSize: "9px", color: "hsl(var(--foreground-muted))" }}>Export raw notes and clips.</span>
              <button className="icon-button" style={{ alignSelf: "flex-end" }}><Download size={12} /></button>
            </div>
            <div style={{ border: "1px solid hsl(var(--danger) / 0.3)", padding: "10px", borderRadius: "var(--radius)", background: "hsl(var(--background))", display: "flex", flexDirection: "column", justifyContent: "space-between", minHeight: "80px" }}>
              <span style={{ fontSize: "11px", fontWeight: 600, color: "hsl(var(--danger))" }}>Request deletion</span>
              <span style={{ fontSize: "9px", color: "hsl(var(--foreground-muted))" }}>Permanently delete server sync.</span>
              <button className="icon-button" style={{ alignSelf: "flex-end", color: "hsl(var(--danger))" }}><Trash2 size={12} /></button>
            </div>
          </div>
          <span style={{ fontSize: "10px", color: "hsl(var(--foreground-disabled))" }}>Exports are end-to-end encrypted and available for 7 days.</span>
        </section>
    </PageLayout>
  );
}
