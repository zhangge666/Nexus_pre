/** 本文件实现设置页面，分类展示所有配置项。 */
import type React from "react";
import { useState, useEffect } from "react";
import { getSettings, saveSettings } from "../core";
import type { OrbitSettings } from "../core";
import { Topbar } from "../components/Topbar";

type SettingsSection =
  | "search" | "rag" | "cards" | "review"
  | "links" | "sync" | "provider" | "appearance" | "about";

const SECTIONS: { key: SettingsSection; label: string; icon: string }[] = [
  { key: "search",     label: "检索",        icon: "🔍" },
  { key: "rag",        label: "问答 (RAG)",   icon: "🤖" },
  { key: "cards",      label: "卡片与复习",   icon: "🃏" },
  { key: "review",     label: "复习调度",     icon: "📅" },
  { key: "links",      label: "关联",         icon: "🔗" },
  { key: "sync",       label: "同步",         icon: "☁️" },
  { key: "provider",   label: "AI Provider",  icon: "⚡" },
  { key: "appearance", label: "外观",         icon: "🎨" },
  { key: "about",      label: "关于",         icon: "ℹ️" },
];

interface ToggleProps { value: boolean; onChange: (v: boolean) => void; id: string }
function Toggle({ value, onChange, id }: ToggleProps): React.JSX.Element {
  return (
    <button
      id={id}
      role="switch"
      aria-checked={value}
      className={`toggle${value ? " on" : ""}`}
      onClick={() => onChange(!value)}
    />
  );
}

interface SliderProps { value: number; min: number; max: number; step: number; onChange: (v: number) => void; id: string }
function Slider({ value, min, max, step, onChange, id }: SliderProps): React.JSX.Element {
  return (
    <input
      id={id}
      type="range"
      min={min} max={max} step={step}
      value={value}
      onChange={(e) => onChange(Number(e.target.value))}
      className="settings-slider"
    />
  );
}

interface SettingRowProps {
  label: string;
  description?: string;
  children: React.ReactNode;
  id?: string;
}
function SettingRow({ label, description, children, id }: SettingRowProps): React.JSX.Element {
  return (
    <div className="setting-row">
      <div className="setting-label-group">
        <label className="setting-label" htmlFor={id}>{label}</label>
        {description && <p className="setting-desc">{description}</p>}
      </div>
      <div className="setting-control">{children}</div>
    </div>
  );
}

export default function SettingsPage(): React.JSX.Element {
  const [section, setSection] = useState<SettingsSection>("search");
  const [settings, setSettings] = useState<OrbitSettings | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => { void getSettings().then(setSettings); }, []);

  function update<K extends keyof OrbitSettings>(
    category: K,
    patch: Partial<OrbitSettings[K]>
  ): void {
    setSettings((s) => s ? { ...s, [category]: { ...s[category], ...patch } } : s);
  }

  async function handleSave(): Promise<void> {
    if (!settings) return;
    setSaving(true);
    try {
      await saveSettings(settings);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } finally {
      setSaving(false);
    }
  }

  if (!settings) return <div className="page-enter"><Topbar title="设置" /><p className="loading-hint">加载设置…</p></div>;

  function renderContent(): React.JSX.Element {
    if (!settings) return <></>;

    if (section === "search") return (
      <div className="settings-content-inner">
        <h2 className="setting-group-title">检索设置</h2>
        <SettingRow label="默认检索模式" id="search-mode">
          <div className="radio-group">
            {(["hybrid", "semantic", "keyword"] as const).map((mode) => (
              <label key={mode} className="radio-option">
                <input type="radio" name="search-mode" value={mode}
                  checked={settings.search.defaultMode === mode}
                  onChange={() => update("search", { defaultMode: mode })} />
                {mode === "hybrid" ? "混合检索" : mode === "semantic" ? "语义检索" : "关键词"}
              </label>
            ))}
          </div>
        </SettingRow>
        <SettingRow label="启用重排" description="对检索结果进行二次排序，提升相关性（稍慢）" id="rerank">
          <Toggle id="rerank" value={settings.search.enableRerank} onChange={(v) => update("search", { enableRerank: v })} />
        </SettingRow>
      </div>
    );

    if (section === "rag") return (
      <div className="settings-content-inner">
        <h2 className="setting-group-title">问答 (RAG) 设置</h2>
        <SettingRow label="Completion Provider" id="rag-provider">
          <select className="settings-select" value={settings.rag.provider}
            onChange={(e) => update("rag", { provider: e.target.value as OrbitSettings["rag"]["provider"] })}>
            <option value="local">本地 LLM</option>
            <option value="claude">Claude</option>
            <option value="openai">OpenAI</option>
            <option value="custom">自定义端点</option>
          </select>
        </SettingRow>
        {settings.rag.provider !== "local" && (
          <SettingRow label="API Key" id="api-key">
            <input type="password" className="settings-input" value={settings.rag.apiKey}
              onChange={(e) => update("rag", { apiKey: e.target.value })}
              placeholder="sk-…" />
          </SettingRow>
        )}
        {settings.rag.provider === "custom" && (
          <SettingRow label="自定义端点" id="custom-endpoint">
            <input type="text" className="settings-input" value={settings.rag.customEndpoint}
              onChange={(e) => update("rag", { customEndpoint: e.target.value })}
              placeholder="https://your-api.example.com/v1" />
          </SettingRow>
        )}
        <SettingRow label="流式输出" description="逐 token 显示回答" id="stream">
          <Toggle id="stream" value={settings.rag.streamEnabled} onChange={(v) => update("rag", { streamEnabled: v })} />
        </SettingRow>
        <SettingRow label="发送前确认" description="发送到云端前展示将要发送的内容" id="confirm">
          <Toggle id="confirm" value={settings.rag.confirmBeforeSend} onChange={(v) => update("rag", { confirmBeforeSend: v })} />
        </SettingRow>
      </div>
    );

    if (section === "cards") return (
      <div className="settings-content-inner">
        <h2 className="setting-group-title">卡片生成设置</h2>
        <SettingRow label="生成方式" id="gen-mode">
          <div className="radio-group">
            {(["ai", "manual"] as const).map((m) => (
              <label key={m} className="radio-option">
                <input type="radio" name="gen-mode" value={m}
                  checked={settings.cards.generationMode === m}
                  onChange={() => update("cards", { generationMode: m })} />
                {m === "ai" ? "AI 自动生成" : "手动创建"}
              </label>
            ))}
          </div>
        </SettingRow>
        <SettingRow label="每篇最多抽取卡片数" id="max-cards">
          <input type="number" className="settings-input short" min={1} max={50}
            value={settings.cards.maxCardsPerNote}
            onChange={(e) => update("cards", { maxCardsPerNote: Number(e.target.value) })} />
        </SettingRow>
        <SettingRow label="默认复习集" id="default-deck">
          <input type="text" className="settings-input" value={settings.cards.defaultDeck}
            onChange={(e) => update("cards", { defaultDeck: e.target.value })} />
        </SettingRow>
      </div>
    );

    if (section === "review") return (
      <div className="settings-content-inner">
        <h2 className="setting-group-title">复习调度设置</h2>
        <SettingRow label="算法" id="algorithm">
          <div className="radio-group">
            {(["fsrs", "sm2"] as const).map((a) => (
              <label key={a} className="radio-option">
                <input type="radio" name="algorithm" value={a}
                  checked={settings.review.algorithm === a}
                  onChange={() => update("review", { algorithm: a })} />
                {a === "fsrs" ? "FSRS（推荐）" : "SM-2"}
              </label>
            ))}
          </div>
        </SettingRow>
        <SettingRow label="每日新卡上限" id="daily-new">
          <input type="number" className="settings-input short" min={0} max={200}
            value={settings.review.dailyNewLimit}
            onChange={(e) => update("review", { dailyNewLimit: Number(e.target.value) })} />
        </SettingRow>
        <SettingRow label="每日复习上限" id="daily-review">
          <input type="number" className="settings-input short" min={0} max={999}
            value={settings.review.dailyReviewLimit}
            onChange={(e) => update("review", { dailyReviewLimit: Number(e.target.value) })} />
        </SettingRow>
        <SettingRow label="启用到期提醒" id="reminder-enabled">
          <Toggle id="reminder-enabled" value={settings.review.reminderEnabled}
            onChange={(v) => update("review", { reminderEnabled: v })} />
        </SettingRow>
        {settings.review.reminderEnabled && (
          <SettingRow label="提醒时间" id="reminder-time">
            <input type="time" className="settings-input short" value={settings.review.reminderTime}
              onChange={(e) => update("review", { reminderTime: e.target.value })} />
          </SettingRow>
        )}
      </div>
    );

    if (section === "links") return (
      <div className="settings-content-inner">
        <h2 className="setting-group-title">关联设置</h2>
        <SettingRow label="自动关联" description="语义相近的记忆自动建立 related 关联" id="auto-link">
          <Toggle id="auto-link" value={settings.links.autoLink} onChange={(v) => update("links", { autoLink: v })} />
        </SettingRow>
        <SettingRow label={`去重提示阈值  ${settings.links.dedupeThreshold.toFixed(2)}`}
          description="超过该相似度时提示合并" id="dedupe">
          <Slider id="dedupe" value={settings.links.dedupeThreshold} min={0.6} max={1} step={0.01}
            onChange={(v) => update("links", { dedupeThreshold: v })} />
        </SettingRow>
        <SettingRow label={`图谱显示密度  ${settings.links.graphDensity.toFixed(1)}`} id="density">
          <Slider id="density" value={settings.links.graphDensity} min={0} max={1} step={0.1}
            onChange={(v) => update("links", { graphDensity: v })} />
        </SettingRow>
      </div>
    );

    if (section === "appearance") return (
      <div className="settings-content-inner">
        <h2 className="setting-group-title">外观设置</h2>
        <SettingRow label="主题" id="theme">
          <div className="radio-group">
            {(["dark", "light", "system"] as const).map((t) => (
              <label key={t} className="radio-option">
                <input type="radio" name="theme" value={t}
                  checked={settings.appearance.theme === t}
                  onChange={() => update("appearance", { theme: t })} />
                {t === "dark" ? "暗色" : t === "light" ? "亮色" : "跟随系统"}
              </label>
            ))}
          </div>
        </SettingRow>
      </div>
    );

    if (section === "about") return (
      <div className="settings-content-inner">
        <h2 className="setting-group-title">关于 Orbit</h2>
        <div className="about-section">
          <div className="about-logo">◎</div>
          <h3>Orbit</h3>
          <p>智能复习 / 中枢记忆库</p>
          <p className="about-version">版本 0.1.0 · Nexus 产品家族</p>
          <div className="about-links">
            <a href="#" className="about-link">文档</a>
            <a href="#" className="about-link">开源许可</a>
            <a href="#" className="about-link">反馈</a>
          </div>
          <p className="about-motto">记录只是开始，让知识环绕回来才是目的。</p>
        </div>
      </div>
    );

    return (
      <div className="settings-content-inner">
        <h2 className="setting-group-title">{SECTIONS.find((s) => s.key === section)?.label}</h2>
        <p className="setting-desc">此设置项正在开发中。</p>
      </div>
    );
  }

  return (
    <div className="page-enter settings-page">
      <Topbar
        title="设置"
        actions={
          <button className="primary-small" onClick={() => void handleSave()} disabled={saving}>
            {saved ? "✓ 已保存" : saving ? "保存中…" : "保存"}
          </button>
        }
      />
      <div className="settings-layout">
        <nav className="settings-nav" aria-label="设置分类">
          {SECTIONS.map(({ key, label, icon }) => (
            <button
              key={key}
              className={`settings-nav-item${section === key ? " active" : ""}`}
              onClick={() => setSection(key)}
            >
              <span>{icon}</span>{label}
            </button>
          ))}
        </nav>
        <div className="settings-content">{renderContent()}</div>
      </div>
    </div>
  );
}
