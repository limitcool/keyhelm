import path from "path"
import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

// Keyhelm 前端：构建产物输出到 ../src/ui/static/，由 Rust rust-embed 嵌入。
// base = /ui/ 匹配后端 static_fallback 的路径（去掉 /ui/ 前缀后查嵌入资源）。
export default defineConfig({
  plugins: [react(), tailwindcss()],
  base: "/ui/",
  resolve: {
    alias: {
      "@": path.resolve(import.meta.dirname, "./src"),
    },
  },
  build: {
    outDir: path.resolve(import.meta.dirname, "../../src/ui/static"),
    emptyOutDir: true,
  },
})
