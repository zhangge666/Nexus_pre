/** 本文件实现 Muse 主题预设、自定义颜色、本地持久化与多窗口同步。 */

import { useEffect, useMemo, useState } from "react";

export type MuseThemePresetId = "graphite" | "warm" | "mist" | "dusk" | "custom";
export type MuseThemeColorKey = "background" | "surface" | "primary" | "foreground";

export interface MuseThemeColors {
  background: string;
  surface: string;
  primary: string;
  foreground: string;
}

export interface MuseThemeSettings {
  preset: MuseThemePresetId;
  custom: MuseThemeColors;
}

export interface MuseThemePreset {
  id: MuseThemePresetId;
  name: string;
  description: string;
  colors: MuseThemeColors;
}

/** 设置页和应用壳共享的主题控制器。 */
export interface MuseThemeController {
  settings: MuseThemeSettings;
  setPreset: (preset: MuseThemePresetId) => void;
  setCustomColor: (key: MuseThemeColorKey, value: string) => void;
  reset: () => void;
}

const STORAGE_KEY = "nexus.muse.theme.v1";

export const MUSE_THEME_PRESETS: MuseThemePreset[] = [
  {
    id: "graphite",
    name: "石墨",
    description: "沉静深灰",
    colors: { background: "#0a0a0d", surface: "#121216", primary: "#727ee6", foreground: "#ededf0" },
  },
  {
    id: "warm",
    name: "暖砂",
    description: "柔和米灰",
    colors: { background: "#eeeae2", surface: "#f8f5ef", primary: "#7465bd", foreground: "#282622" },
  },
  {
    id: "mist",
    name: "雾白",
    description: "清爽浅灰",
    colors: { background: "#edf0f4", surface: "#f9fafb", primary: "#536fc7", foreground: "#20242b" },
  },
  {
    id: "dusk",
    name: "暮蓝",
    description: "低饱和蓝灰",
    colors: { background: "#18202a", surface: "#202a36", primary: "#8498e8", foreground: "#eef1f5" },
  },
  {
    id: "custom",
    name: "自定义",
    description: "按你的偏好",
    colors: { background: "#e9e5dc", surface: "#f7f3ec", primary: "#7667c6", foreground: "#292621" },
  },
];

const DEFAULT_THEME: MuseThemeSettings = {
  preset: "graphite",
  custom: { ...MUSE_THEME_PRESETS.find((preset) => preset.id === "custom")!.colors },
};

interface HslColor {
  h: number;
  s: number;
  l: number;
}

/** 把十六进制颜色转换为 HSL，供项目 token 的 hsl(var()) 写法使用。 */
function hexToHsl(hex: string): HslColor {
  const normalized = hex.replace("#", "");
  const value = normalized.length === 3 ? normalized.split("").map((char) => `${char}${char}`).join("") : normalized;
  const red = Number.parseInt(value.slice(0, 2), 16) / 255;
  const green = Number.parseInt(value.slice(2, 4), 16) / 255;
  const blue = Number.parseInt(value.slice(4, 6), 16) / 255;
  const max = Math.max(red, green, blue);
  const min = Math.min(red, green, blue);
  const lightness = (max + min) / 2;
  if (max === min) return { h: 0, s: 0, l: Math.round(lightness * 100) };
  const delta = max - min;
  const saturation = lightness > 0.5 ? delta / (2 - max - min) : delta / (max + min);
  let hue = max === red ? (green - blue) / delta + (green < blue ? 6 : 0) : max === green ? (blue - red) / delta + 2 : (red - green) / delta + 4;
  hue /= 6;
  return { h: Math.round(hue * 360), s: Math.round(saturation * 100), l: Math.round(lightness * 100) };
}

/** 将 HSL 数值格式化成不含外层函数的 CSS token。 */
function hslToken(color: HslColor, alpha?: number): string {
  return `${color.h} ${color.s}% ${color.l}%${alpha === undefined ? "" : ` / ${alpha}`}`;
}

/** 在不改变色相的情况下移动明度，用于从四个基础色派生完整界面层级。 */
function shiftLightness(color: HslColor, amount: number): HslColor {
  return { ...color, l: Math.max(0, Math.min(100, color.l + amount)) };
}

/** 从四个用户可理解的颜色生成完整设计 token，并自动判断明暗模式。 */
function createThemeTokens(colors: MuseThemeColors): { dark: boolean; tokens: Record<string, string> } {
  const background = hexToHsl(colors.background);
  const surface = hexToHsl(colors.surface);
  const primary = hexToHsl(colors.primary);
  const foreground = hexToHsl(colors.foreground);
  const dark = background.l < 48;
  const direction = dark ? 1 : -1;
  return {
    dark,
    tokens: {
      "--background": hslToken(background),
      "--surface": hslToken(surface),
      "--surface-elevated": hslToken(shiftLightness(surface, direction * 3)),
      "--surface-hover": hslToken(shiftLightness(surface, direction * 6)),
      "--surface-active": hslToken({ ...primary, s: Math.max(18, primary.s - 26), l: dark ? Math.min(30, background.l + 13) : Math.max(82, background.l - 9) }),
      "--muted": hslToken(shiftLightness(surface, direction * 4)),
      "--foreground": hslToken(foreground),
      "--foreground-secondary": hslToken({ ...foreground, s: Math.min(foreground.s, 16), l: dark ? 70 : 38 }),
      "--foreground-muted": hslToken({ ...foreground, s: Math.min(foreground.s, 12), l: dark ? 50 : 52 }),
      "--foreground-disabled": hslToken({ ...foreground, s: Math.min(foreground.s, 10), l: dark ? 35 : 66 }),
      "--border": hslToken(foreground, dark ? 0.08 : 0.12),
      "--border-strong": hslToken(foreground, dark ? 0.14 : 0.2),
      "--border-subtle": hslToken(foreground, dark ? 0.05 : 0.075),
      "--primary": hslToken(primary),
      "--primary-hover": hslToken(shiftLightness(primary, dark ? 5 : -5)),
      "--primary-foreground": primary.l > 64 ? "220 18% 12%" : "0 0% 100%",
      "--ring": hslToken(primary),
    },
  };
}

/** 返回当前设置实际使用的基础颜色。 */
export function resolveThemeColors(settings: MuseThemeSettings): MuseThemeColors {
  if (settings.preset === "custom") return settings.custom;
  return MUSE_THEME_PRESETS.find((preset) => preset.id === settings.preset)?.colors ?? DEFAULT_THEME.custom;
}

/** 将主题设置应用到根节点，使所有页面继续只消费统一设计 token。 */
function applyTheme(settings: MuseThemeSettings): void {
  const colors = resolveThemeColors(settings);
  const { dark, tokens } = createThemeTokens(colors);
  const root = document.documentElement;
  root.classList.toggle("dark", dark);
  root.dataset.museTheme = settings.preset;
  root.style.colorScheme = dark ? "dark" : "light";
  Object.entries(tokens).forEach(([name, value]) => root.style.setProperty(name, value));
  document.querySelector('meta[name="theme-color"]')?.setAttribute("content", colors.background);
}

/** 从本地存储恢复主题，旧版本或损坏内容回退到石墨主题。 */
function loadTheme(): MuseThemeSettings {
  try {
    const saved = window.localStorage.getItem(STORAGE_KEY);
    if (!saved) return DEFAULT_THEME;
    const parsed = JSON.parse(saved) as Partial<MuseThemeSettings>;
    const validPreset = MUSE_THEME_PRESETS.some((preset) => preset.id === parsed.preset);
    return {
      preset: validPreset ? parsed.preset! : DEFAULT_THEME.preset,
      custom: { ...DEFAULT_THEME.custom, ...parsed.custom },
    };
  } catch {
    return DEFAULT_THEME;
  }
}

/** 提供主题切换和自定义颜色动作，并同步所有已打开的 Muse 窗口。 */
export function useMuseTheme(): MuseThemeController {
  const [settings, setSettings] = useState<MuseThemeSettings>(loadTheme);

  useEffect(() => {
    applyTheme(settings);
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  }, [settings]);

  useEffect(() => {
    /** 接收其他 Muse WebView 中的主题修改。 */
    function handleStorage(event: StorageEvent): void {
      if (event.key !== STORAGE_KEY || !event.newValue) return;
      try {
        setSettings(JSON.parse(event.newValue) as MuseThemeSettings);
      } catch {
        // 保留当前可用主题，不采用其他窗口写入的损坏数据。
      }
    }
    window.addEventListener("storage", handleStorage);
    return () => window.removeEventListener("storage", handleStorage);
  }, []);

  return useMemo(() => ({
    settings,
    setPreset: (preset: MuseThemePresetId) => setSettings((current) => ({ ...current, preset })),
    setCustomColor: (key: MuseThemeColorKey, value: string) => setSettings((current) => ({
      preset: "custom",
      custom: { ...current.custom, [key]: value },
    })),
    reset: () => setSettings(DEFAULT_THEME),
  }), [settings]);
}
