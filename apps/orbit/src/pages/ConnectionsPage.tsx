/** 本文件实现连接与隐私管理页面。 */
import type React from "react";
import { useState, useEffect } from "react";
import { Link2, Trash2, ShieldCheck, Download, Key, Monitor } from "lucide-react";
import { listConnectedApps, revokeApp } from "../core";
import type { ConnectedApp } from "../core";
import { Topbar } from "../components/Topbar";

function formatRelTime(ts: number): string {
  const diff = Date.now() - ts;
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)} 分钟前`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)} 小时前`;
  return `${Math.floor(diff / 86_400_000)} 天前`;
}

const APP_ICONS: Record<string, string> = {
  echo: "🖥", muse: "💡", quill: "📝",
};

type SyncMode = "local" | "e2e_cloud" | "self_hosted";
const SYNC_OPTIONS: { key: SyncMode; label: string; desc: string }[] = [
  { key: "local", label: "纯本地", desc: "所有数据仅存储在本设备，不联网" },
  { key: "e2e_cloud", label: "E2E 云同步", desc: "端到端加密，服务端零知识" },
  { key: "self_hosted", label: "自托管中继", desc: "使用自己的服务器中继" },
];

export default function ConnectionsPage(): React.JSX.Element {
  const [apps, setApps] = useState<ConnectedApp[]>([]);
  const [syncMode, setSyncMode] = useState<SyncMode>("local");
  const [loading, setLoading] = useState(true);
  const [revoking, setRevoking] = useState<string | null>(null);

  useEffect(() => {
    void listConnectedApps().then((a) => { setApps(a); setLoading(false); });
  }, []);

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
    <div className="page-enter connections-page">
      <Topbar title="连接与隐私" subtitle="管理已连接的应用和隐私设置" />

      <div className="connections-content">
        {/* 已连接应用 */}
        <section className="conn-section">
          <h2 className="conn-section-title"><Link2 size={16} />已连接应用 ({apps.length})</h2>
          {loading && <p className="loading-hint">加载中…</p>}
          <div className="conn-apps-list">
            {apps.map((app) => (
              <div key={app.id} className="conn-app-card">
                <div className="conn-app-icon">
                  {APP_ICONS[app.source.split(":")[0]] ?? "🔌"}
                </div>
                <div className="conn-app-body">
                  <div className="conn-app-name">{app.name}</div>
                  <div className="conn-app-meta">
                    <span className="conn-app-source">{app.source}</span>
                    <span>最近活跃：{formatRelTime(app.lastActiveAt)}</span>
                    <span>写入 {app.memoriesCount} 条记忆</span>
                  </div>
                  <div className="conn-app-scopes">
                    {app.scopes.map((s) => (
                      <span key={s} className="scope-badge">{s}</span>
                    ))}
                  </div>
                </div>
                <button
                  className="icon-button danger-hover"
                  onClick={() => void handleRevoke(app)}
                  disabled={revoking === app.tokenId}
                  title="撤销访问"
                  aria-label={`撤销 ${app.name} 的访问权限`}
                >
                  <Trash2 size={15} />
                </button>
              </div>
            ))}
          </div>
        </section>

        {/* 同步设置 */}
        <section className="conn-section">
          <h2 className="conn-section-title"><ShieldCheck size={16} />同步设置</h2>
          <div className="sync-options">
            {SYNC_OPTIONS.map(({ key, label, desc }) => (
              <label key={key} className={`sync-option${syncMode === key ? " active" : ""}`}>
                <input
                  type="radio"
                  name="sync-mode"
                  value={key}
                  checked={syncMode === key}
                  onChange={() => setSyncMode(key)}
                />
                <div>
                  <strong>{label}</strong>
                  <p>{desc}</p>
                </div>
              </label>
            ))}
          </div>
          {syncMode === "local" && (
            <p className="sync-status"><span className="status-dot success" />所有数据仅存储在本设备</p>
          )}
        </section>

        {/* 安全与密钥 */}
        <section className="conn-section">
          <h2 className="conn-section-title"><Key size={16} />安全与密钥</h2>
          <div className="security-rows">
            <div className="security-row">
              <div>
                <strong>设备密钥</strong>
                <p>本设备的端到端加密密钥</p>
              </div>
              <span className="status-chip success">✅ 已生成</span>
            </div>
            <div className="security-row">
              <div>
                <strong>恢复短语</strong>
                <p className="warn-text">⚠️ 丢失短语则数据不可恢复</p>
              </div>
              <button className="secondary-button">查看恢复短语</button>
            </div>
            <div className="security-row">
              <div>
                <strong>已配对设备</strong>
                <p>1 台设备</p>
              </div>
              <div className="device-list">
                <span className="device-chip">
                  <Monitor size={12} />mac-studio-01 <em>(当前)</em>
                </span>
              </div>
            </div>
          </div>
        </section>

        {/* 数据导出 */}
        <section className="conn-section">
          <h2 className="conn-section-title"><Download size={16} />数据导出</h2>
          <div className="export-buttons">
            <button className="secondary-button"><Download size={14} />导出为 Markdown</button>
            <button className="secondary-button"><Download size={14} />导出为 JSON</button>
            <button className="secondary-button"><Download size={14} />全量导出（含关系）</button>
          </div>
          <p className="export-hint">导出文件包含所有记忆、标签、集合与关联关系，保证数据可带走。</p>
        </section>
      </div>
    </div>
  );
}
