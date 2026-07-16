/** 本文件配置 Muse M3 前端开发服务和 Tauri 构建输出。 */
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

/** 创建 Muse 最小 WebView 使用的 Vite 配置。 */
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { host: "127.0.0.1", port: 1421, strictPort: true },
  build: { target: "es2022" },
});
