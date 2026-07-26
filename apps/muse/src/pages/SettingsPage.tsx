/** 本文件实现 Muse 快捷键、本地模式与可选 Orbit 连接设置。 */

import React from "react";
import { Check, Cloud, Database, Keyboard, Link2, Shield, Unplug } from "lucide-react";
import { PageHeader } from "../components/PageHeader";
import type { ConnectionStatus } from "../api";

interface SettingsPageProps {
  connection: ConnectionStatus;
  connecting: boolean;
  onConnect: () => Promise<void>;
}

const hotkeys = [
  { label: "打开 Muse", detail: "显示快捷入口", keys: ["Ctrl", "Shift", "Space"] },
  { label: "记录灵感", detail: "直接聚焦输入框", keys: ["Ctrl", "Shift", "I"] },
  { label: "新建任务", detail: "带入当前剪贴板", keys: ["Ctrl", "Shift", "T"] },
  { label: "开始 / 停止会议", detail: "再次按下即停止", keys: ["Ctrl", "Shift", "R"] },
  { label: "剪贴板比较", detail: "显示并置顶窗口", keys: ["Ctrl", "Shift", "V"] },
];

/** 呈现本地优先设置，并把 Orbit 明确降级为可选连接。 */
export function SettingsPage({ connection, connecting, onConnect }: SettingsPageProps): React.JSX.Element {
  const connected = connection.state === "connected";

  return (
    <div className="page page-settings">
      <PageHeader
        eyebrow="设置"
        title="Muse 按自己的方式工作"
        description="单机模式始终可用；连接 Orbit 只用于跨应用检索和可选同步。"
      />

      <div className="settings-columns">
        <section className="settings-section">
          <header><Keyboard size={15} /><div><h2>功能快捷键</h2><p>每个高频动作都能一步直达。</p></div><button className="secondary-button" type="button">恢复默认</button></header>
          <div className="hotkey-list">
            {hotkeys.map((hotkey) => (
              <div className="hotkey-row" key={hotkey.label}>
                <span><strong>{hotkey.label}</strong><small>{hotkey.detail}</small></span>
                <button type="button">
                  {hotkey.keys.map((key) => <kbd key={key}>{key}</kbd>)}
                </button>
              </div>
            ))}
          </div>
          <footer className="settings-state"><Check size={12} /> 未发现快捷键冲突</footer>
        </section>

        <div className="settings-stack">
          <section className="settings-section storage-section">
            <header><Database size={15} /><div><h2>本地工作区</h2><p>Muse 的默认数据位置。</p></div></header>
            <div className="setting-row">
              <span><strong>独立模式</strong><small>灵感、任务与剪贴板不依赖其他软件</small></span>
              <span className="local-chip"><Shield size={12} /> 已启用</span>
            </div>
            <div className="setting-row">
              <span><strong>数据保留</strong><small>剪贴板未固定内容 24 小时后清理</small></span>
              <button className="secondary-button" type="button">更改</button>
            </div>
          </section>

          <section className="settings-section connection-section">
            <header>
              {connected ? <Cloud size={15} /> : <Unplug size={15} />}
              <div><h2>Orbit 连接</h2><p>可选的跨应用记忆与同步能力。</p></div>
            </header>
            <div className={`connection-card ${connected ? "is-connected" : ""}`}>
              <span className="connection-icon">{connected ? <Link2 size={15} /> : <Unplug size={15} />}</span>
              <div>
                <strong>{connected ? "已连接 Orbit" : "未连接也可正常使用"}</strong>
                <small>{connected ? connection.endpoint : "当前内容仅保存在 Muse 本机工作区。"}</small>
              </div>
              <button className={connected ? "secondary-button" : "primary-button"} type="button" onClick={() => void onConnect()} disabled={connecting}>
                {connecting ? "连接中…" : connected ? "重新连接" : "连接 Orbit"}
              </button>
            </div>
            {connection.message ? <p className="connection-message">{connection.message}</p> : null}
          </section>
        </div>
      </div>
    </div>
  );
}
