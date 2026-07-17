/** 本文件实现 Orbit 的设置工作台，集中管理搜索、问答、复习、关联与外观偏好。 */
import type React from "react";
import { useEffect, useState } from "react";
import { BrainCircuit, Info, Layers, Link2, MessageCircle, Palette, Search } from "lucide-react";
import { getSettings, saveSettings } from "../core";
import type { OrbitSettings } from "../core";
import { PageLayout } from "../components/PageLayout";
import { Topbar } from "../components/Topbar";

type SettingsSection = "search" | "rag" | "cards" | "review" | "links" | "appearance" | "about";

const SECTIONS: { key: SettingsSection; label: string; icon: React.ReactNode }[] = [
  { key: "search", label: "检索", icon: <Search size={15} /> },
  { key: "rag", label: "问答", icon: <MessageCircle size={15} /> },
  { key: "cards", label: "卡片", icon: <Layers size={15} /> },
  { key: "review", label: "复习调度", icon: <BrainCircuit size={15} /> },
  { key: "links", label: "关联", icon: <Link2 size={15} /> },
  { key: "appearance", label: "外观", icon: <Palette size={15} /> },
  { key: "about", label: "关于 Orbit", icon: <Info size={15} /> },
];

/** 渲染受控开关，并将状态变化交给设置页统一保存。 */
function Toggle({ value, onChange, id }: { value: boolean; onChange: (value: boolean) => void; id: string }): React.JSX.Element {
  return <button id={id} type="button" role="switch" aria-checked={value} className={`toggle${value ? " on" : ""}`} onClick={() => onChange(!value)} />;
}

/** 渲染一个结构统一的设置行，避免各设置分类出现不一致的卡片视觉。 */
function SettingRow({ label, description, children, id }: {
  label: string;
  description?: string;
  children: React.ReactNode;
  id?: string;
}): React.JSX.Element {
  return (
    <div className="setting-row">
      <div className="setting-label-group"><label className="setting-label" htmlFor={id}>{label}</label>{description && <p className="setting-desc">{description}</p>}</div>
      <div className="setting-control">{children}</div>
    </div>
  );
}

/** 渲染设置分类和内容，并将保存操作明确收束到标题栏。 */
export default function SettingsPage(): React.JSX.Element {
  const [section, setSection] = useState<SettingsSection>("search");
  const [settings, setSettings] = useState<OrbitSettings | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  /** 初次进入时读取本机设置；失败后保留可见错误而不是无限加载。 */
  useEffect(() => {
    void getSettings().then(setSettings).catch((reason) => setError(`设置加载失败：${String(reason)}`));
  }, []);

  /** 合并单个设置分组的改动，保存前仅更新内存中的草稿。 */
  function update<K extends keyof OrbitSettings>(category: K, patch: Partial<OrbitSettings[K]>): void {
    setSettings((current) => current ? { ...current, [category]: { ...current[category], ...patch } } : current);
    setSaved(false);
  }

  /** 保存当前草稿，并重新读取经服务端归一化后的公开设置。 */
  async function handleSave(): Promise<void> {
    if (!settings) return;
    setSaving(true);
    setError(null);
    try {
      await saveSettings(settings);
      setSettings(await getSettings());
      setSaved(true);
      window.setTimeout(() => setSaved(false), 2_000);
    } catch (reason) {
      setError(`保存失败：${String(reason)}`);
    } finally {
      setSaving(false);
    }
  }

  const topbar = (
    <Topbar
      title="设置"
      subtitle="管理搜索、问答、复习与外观偏好"
      actions={settings ? <button className="primary-small" onClick={() => void handleSave()} disabled={saving}>{saved ? "已保存" : saving ? "保存中…" : "保存"}</button> : undefined}
    />
  );

  /** 根据当前分类仅渲染对应设置，减少不相关选项的视觉噪声。 */
  function renderContent(): React.JSX.Element {
    if (!settings) return <></>;

    if (section === "search") return <div className="settings-content-inner">
      <h2 className="setting-group-title">检索</h2>
      <SettingRow label="默认检索模式" id="search-mode">
        <div className="radio-group">
          {(["hybrid", "semantic", "keyword"] as const).map((mode) => <label className="radio-option" key={mode}><input id={mode === "hybrid" ? "search-mode" : undefined} type="radio" name="search-mode" checked={settings.search.defaultMode === mode} onChange={() => update("search", { defaultMode: mode })} />{mode === "hybrid" ? "混合检索" : mode === "semantic" ? "语义检索" : "关键词检索"}</label>)}
        </div>
      </SettingRow>
      <SettingRow label="启用重排" description="对检索结果进行二次排序以提升相关性，响应会稍慢。" id="rerank"><Toggle id="rerank" value={settings.search.enableRerank} onChange={(value) => update("search", { enableRerank: value })} /></SettingRow>
    </div>;

    if (section === "rag") return <div className="settings-content-inner">
      <h2 className="setting-group-title">问答</h2>
      <SettingRow label="回答提供方" id="rag-provider">
        <select id="rag-provider" className="settings-select" value={settings.rag.provider} onChange={(event) => update("rag", { provider: event.target.value as OrbitSettings["rag"]["provider"] })}>
          <option value="local">本地抽取式回答</option><option value="ollama">Ollama（本地 LLM）</option><option value="claude">Claude</option><option value="openai">OpenAI</option><option value="custom">自定义端点</option>
        </select>
      </SettingRow>
      {settings.rag.provider !== "local" && <>
        <SettingRow label="模型" description="留空时使用服务提供方的默认模型。" id="provider-model"><input id="provider-model" className="settings-input" value={settings.rag.model} onChange={(event) => update("rag", { model: event.target.value })} placeholder={settings.rag.provider === "ollama" ? "qwen3:8b" : "默认模型"} /></SettingRow>
        {settings.rag.provider !== "ollama" && <SettingRow label="API Key" description={settings.rag.hasApiKey ? "已保存到系统凭据库；此处不会回显。" : "保存后写入系统凭据库，不写入设置文件。"} id="api-key"><input id="api-key" type="password" autoComplete="off" className="settings-input" value={settings.rag.apiKey} onChange={(event) => update("rag", { apiKey: event.target.value })} placeholder={settings.rag.hasApiKey ? "已配置（不回显）" : "输入 API Key"} /></SettingRow>}
      </>}
      {["custom", "ollama"].includes(settings.rag.provider) && <SettingRow label={settings.rag.provider === "ollama" ? "Ollama 地址" : "自定义端点"} id="custom-endpoint"><input id="custom-endpoint" className="settings-input" value={settings.rag.customEndpoint} onChange={(event) => update("rag", { customEndpoint: event.target.value })} placeholder={settings.rag.provider === "ollama" ? "http://127.0.0.1:11434" : "https://your-api.example.com/v1"} /></SettingRow>}
      <SettingRow label="实时流式输出" description="生成过程中逐段显示文本；关闭后仍可获得完整回答。" id="stream"><Toggle id="stream" value={settings.rag.streamEnabled} onChange={(value) => update("rag", { streamEnabled: value })} /></SettingRow>
      <SettingRow label="发送前确认" description="向云端服务发送内容前先显示将要发送的上下文。" id="confirm"><Toggle id="confirm" value={settings.rag.confirmBeforeSend} onChange={(value) => update("rag", { confirmBeforeSend: value })} /></SettingRow>
    </div>;

    if (section === "cards") return <div className="settings-content-inner">
      <h2 className="setting-group-title">卡片</h2>
      <SettingRow label="生成方式" id="generation-mode"><div className="radio-group"><label className="radio-option"><input id="generation-mode" type="radio" name="generation-mode" checked={settings.cards.generationMode === "ai"} onChange={() => update("cards", { generationMode: "ai" })} />AI 自动生成</label><label className="radio-option"><input type="radio" name="generation-mode" checked={settings.cards.generationMode === "manual"} onChange={() => update("cards", { generationMode: "manual" })} />手动创建</label></div></SettingRow>
      <SettingRow label="每篇最大抽取数" id="max-cards"><input id="max-cards" type="number" min={1} max={50} className="settings-input short" value={settings.cards.maxCardsPerNote} onChange={(event) => update("cards", { maxCardsPerNote: Number(event.target.value) })} /></SettingRow>
      <SettingRow label="默认复习集" id="default-deck"><input id="default-deck" className="settings-input" value={settings.cards.defaultDeck} onChange={(event) => update("cards", { defaultDeck: event.target.value })} /></SettingRow>
    </div>;

    if (section === "review") return <div className="settings-content-inner">
      <h2 className="setting-group-title">复习调度</h2>
      <SettingRow label="算法" id="algorithm"><div className="radio-group"><label className="radio-option"><input id="algorithm" type="radio" name="algorithm" checked={settings.review.algorithm === "fsrs"} onChange={() => update("review", { algorithm: "fsrs" })} />FSRS（推荐）</label><label className="radio-option"><input type="radio" name="algorithm" checked={settings.review.algorithm === "sm2"} onChange={() => update("review", { algorithm: "sm2" })} />SM-2</label></div></SettingRow>
      <SettingRow label="每日新卡上限" id="daily-new"><input id="daily-new" type="number" min={0} max={200} className="settings-input short" value={settings.review.dailyNewLimit} onChange={(event) => update("review", { dailyNewLimit: Number(event.target.value) })} /></SettingRow>
      <SettingRow label="每日复习上限" id="daily-review"><input id="daily-review" type="number" min={0} max={999} className="settings-input short" value={settings.review.dailyReviewLimit} onChange={(event) => update("review", { dailyReviewLimit: Number(event.target.value) })} /></SettingRow>
      <SettingRow label="启用到期提醒" id="reminder-enabled"><Toggle id="reminder-enabled" value={settings.review.reminderEnabled} onChange={(value) => update("review", { reminderEnabled: value })} /></SettingRow>
      {settings.review.reminderEnabled && <SettingRow label="提醒时间" id="reminder-time"><input id="reminder-time" type="time" className="settings-input short" value={settings.review.reminderTime} onChange={(event) => update("review", { reminderTime: event.target.value })} /></SettingRow>}
    </div>;

    if (section === "links") return <div className="settings-content-inner">
      <h2 className="setting-group-title">关联</h2>
      <SettingRow label="自动关联" description="为语义相近的记忆建立 related 关联。" id="auto-link"><Toggle id="auto-link" value={settings.links.autoLink} onChange={(value) => update("links", { autoLink: value })} /></SettingRow>
      <SettingRow label={`去重提示阈值 ${settings.links.dedupeThreshold.toFixed(2)}`} description="相似度达到该阈值时提示合并。" id="dedupe"><input id="dedupe" className="settings-slider" type="range" min={0.6} max={1} step={0.01} value={settings.links.dedupeThreshold} onChange={(event) => update("links", { dedupeThreshold: Number(event.target.value) })} /></SettingRow>
      <SettingRow label={`图谱显示密度 ${settings.links.graphDensity.toFixed(1)}`} id="density"><input id="density" className="settings-slider" type="range" min={0} max={1} step={0.1} value={settings.links.graphDensity} onChange={(event) => update("links", { graphDensity: Number(event.target.value) })} /></SettingRow>
    </div>;

    if (section === "appearance") return <div className="settings-content-inner">
      <h2 className="setting-group-title">外观</h2>
      <SettingRow label="主题" id="theme"><div className="radio-group"><label className="radio-option"><input id="theme" type="radio" name="theme" checked={settings.appearance.theme === "dark"} onChange={() => update("appearance", { theme: "dark" })} />暗色</label><label className="radio-option"><input type="radio" name="theme" checked={settings.appearance.theme === "light"} onChange={() => update("appearance", { theme: "light" })} />亮色</label><label className="radio-option"><input type="radio" name="theme" checked={settings.appearance.theme === "system"} onChange={() => update("appearance", { theme: "system" })} />跟随系统</label></div></SettingRow>
    </div>;

    return <div className="settings-content-inner">
      <h2 className="setting-group-title">关于 Orbit</h2>
      <div className="about-facts">
        <div><span>应用</span><strong>Orbit 0.1.0</strong></div>
        <div><span>数据</span><strong>记忆与设置均存储在本机</strong></div>
        <div><span>凭据</span><strong>API Key 使用系统凭据库存放</strong></div>
        <div><span>问答</span><strong>默认使用本地抽取式回答</strong></div>
      </div>
    </div>;
  }

  if (!settings) {
    return <PageLayout className="settings-page">{topbar}<p className="loading-hint" role={error ? "alert" : "status"}>{error ?? "正在加载设置…"}</p></PageLayout>;
  }

  return (
    <PageLayout className="settings-page">
      {topbar}
      <div className="settings-layout">
        <nav className="settings-nav" aria-label="设置分类">
          {SECTIONS.map(({ key, label, icon }) => <button key={key} className={`settings-nav-item${section === key ? " active" : ""}`} onClick={() => setSection(key)}><span>{icon}</span>{label}</button>)}
        </nav>
        <div className="settings-content">{renderContent()}</div>
      </div>
      {error && <p className="inline-notice" role="alert">{error}</p>}
    </PageLayout>
  );
}
