import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { visualizer } from "rollup-plugin-visualizer";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;
const isProduction = process.env.NODE_ENV === "production";

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [
    vue(),
    // 添加打包分析器，可以查看打包后的文件大小
    visualizer({
      open: false,
      gzipSize: true,
      brotliSize: true,
    }),
  ],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 3000,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },

  // 生产环境优化
  build: {
    target: "esnext",
    minify: "terser",
    terserOptions: {
      compress: {
        drop_console: isProduction,
        drop_debugger: isProduction,
      },
    },
    rollupOptions: {
      output: {
        manualChunks: {
          vue: ["vue"],
          tauri: ["@tauri-apps/api"],
        },
      },
    },
  },
}));
