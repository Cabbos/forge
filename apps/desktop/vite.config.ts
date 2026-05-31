import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
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
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (!id.includes("node_modules")) return;

          if (id.includes("@codemirror")) return "vendor-codemirror";
          if (id.includes("react-diff-viewer")) return "vendor-diff";
          if (id.includes("react-markdown") || id.includes("remark-") || id.includes("rehype-") || id.includes("micromark") || id.includes("mdast-") || id.includes("hast-") || id.includes("unified")) {
            return "vendor-markdown";
          }
          if (id.includes("gsap") || id.includes("@gsap")) return "vendor-motion";
          if (id.includes("lucide-react")) return "vendor-icons";
          if (id.includes("react") || id.includes("scheduler") || id.includes("zustand") || id.includes("@tanstack/react-virtual")) {
            return "vendor-react";
          }
        },
      },
    },
  },
}));
