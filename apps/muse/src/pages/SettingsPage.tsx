/** 本文件实现 Muse 快捷键、本地模式与可选 Orbit 连接设置。 */

import React from "react";
import { Cloud, Database, Info, Keyboard, Link2, Palette, RotateCcw, Shield, Unplug } from "lucide-react";
import { PageHeader } from "../components/PageHeader";
import type { ConnectionStatus } from "../api";
import { MUSE_THEME_PRESETS, resolveThemeColors, type MuseThemeColorKey, type MuseThemeController } from "../core/theme";

interface SettingsPageProps {
  connection: ConnectionStatus;
  connecting: boolean;
  onConnect: () => Promise<void>;
  theme: MuseThemeController;
}

const hotkeys = [
  { label: "打开 Muse", detail: "显示快捷入口", keys: ["Ctrl", "Shift", "Space"] },
  { label: "记录灵感", detail: "直接聚焦输入框", keys: ["Ctrl", "Shift", "I"] },
  { label: "新建任务", detail: "可读取当前剪贴板", keys: ["Ctrl", "Shift", "T"] },
  { label: "会议记录", detail: "打开计时与文字记录窗", keys: ["Ctrl", "Shift", "R"] },
  { label: "剪贴板推进", detail: "显示并置顶窗口", keys: ["Ctrl", "Shift", "V"] },
];

/** 呈现本地优先设置，并把 Orbit 明确降级为可选连接。 */
export function SettingsPage({ connection, connecting, onConnect, theme }: SettingsPageProps): React.JSX.Element {
  const connected = connection.state === "connected";
  const activeColors = resolveThemeColors(theme.settings);
  const customColorFields: Array<{ key: MuseThemeColorKey; label: string; description: string }> = [
    { key: "background", label: "页面底色", description: "主工作区背景" },
    { key: "surface", label: "面板颜色", description: "侧栏与浮层" },
    { key: "primary", label: "强调颜色", description: "选中与主要操作" },
    { key: "foreground", label: "文字颜色", description: "标题与正文" },
  ];

  return (
    <div className="page page-settings">
      <PageHeader
        eyebrow="设置"
        title="Muse 按自己的方式工作"
        description="单机模式始终可用；连接 Orbit 只用于跨应用检索和可选同步。"
      />

      <section className="settings-section theme-section">
        <header>
          <Palette size={15} />
          <div><h2>外观主题</h2><p>选择柔和预设，或组合自己的界面颜色；修改会同步到快捷工具窗。</p></div>
          <button className="secondary-button" type="button" onClick={theme.reset}><RotateCcw size={12} /> 恢复默认</button>
        </header>
        <div className="theme-preset-list" role="radiogroup" aria-label="界面主题">
          {MUSE_THEME_PRESETS.map((preset) => {
            const colors = preset.id === "custom" ? theme.settings.custom : preset.colors;
            return (
              <button
                className={theme.settings.preset === preset.id ? "is-active" : ""}
                key={preset.id}
                type="button"
                role="radio"
                aria-checked={theme.settings.preset === preset.id}
                onClick={() => theme.setPreset(preset.id)}
              >
                <span className="theme-preview" style={{ background: colors.background }}>
                  <i style={{ background: colors.surface }} />
                  <i style={{ background: colors.primary }} />
                  <i style={{ background: colors.foreground }} />
                </span>
                <span><strong>{preset.name}</strong><small>{preset.description}</small></span>
              </button>
            );
          })}
        </div>
        <div className="custom-theme-controls">
          <div className="custom-theme-heading">
            <span><strong>自定义配色</strong><small>调整任意颜色后会自动切换到“自定义”。</small></span>
            <span className="active-theme-chip"><i style={{ background: activeColors.primary }} />当前：{MUSE_THEME_PRESETS.find((preset) => preset.id === theme.settings.preset)?.name}</span>
          </div>
          <div className="color-field-grid">
            {customColorFields.map((field) => (
              <label className="color-field" key={field.key}>
                <input
                  type="color"
                  value={theme.settings.custom[field.key]}
                  onChange={(event) => theme.setCustomColor(field.key, event.target.value)}
                  aria-label={field.label}
                />
                <span><strong>{field.label}</strong><small>{field.description}</small></span>
                <code>{theme.settings.custom[field.key].toUpperCase()}</code>
              </label>
            ))}
          </div>
        </div>
      </section>

      <div className="settings-columns">
        <section className="settings-section">
          <header><Keyboard size={15} /><div><h2>功能快捷键</h2><p>每个高频动作都能一步直达。</p></div><button className="secondary-button" type="button" disabled>自定义即将接入</button></header>
          <div className="hotkey-list">
            {hotkeys.map((hotkey) => (
              <div className="hotkey-row" key={hotkey.label}>
                <span><strong>{hotkey.label}</strong><small>{hotkey.detail}</small></span>
                <button type="button" disabled title="当前版本使用桌面壳默认快捷键">
                  {hotkey.keys.map((key) => <kbd key={key}>{key}</kbd>)}
                </button>
              </div>
            ))}
          </div>
          <footer className="settings-state"><Info size={12} /> 默认快捷键由桌面壳注册；单个冲突不会阻止 Muse 启动</footer>
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
