import { defineConfig } from "vite";

export default defineConfig({
  // 相对路径，方便 Tauri 以 file:// 方式加载
  base: "./",
  clearScreen: false,
  server: {
    port: 5173,
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
});
