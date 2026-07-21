/** 本文件配置 Orbit 前端开发服务和 Tauri 构建输出。 */

import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Tauri 在真机 iOS 调试时提供可访问开发服务器的地址；桌面与 Android 本地调试保持回环地址。
const tauriDevHost = process.env.TAURI_DEV_HOST;

/** 创建适合 Tauri WebView 与本地预览的 Vite 配置。 */
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    host: tauriDevHost || "127.0.0.1",
    port: 1420,
    strictPort: true,
    // 仅在 Tauri 指定移动真机地址时开放 HMR，避免桌面开发服务器暴露到局域网。
    hmr: tauriDevHost
      ? {
          protocol: "ws",
          host: tauriDevHost,
          port: 1421,
        }
      : undefined,
  },
  build: {
    target: "es2022",
  },
});
