import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    watch: {
      // Don't watch the Rust build output — on Windows the app locks
      // target\debug\deps\*.dll and Vite's watcher crashes with EBUSY.
      ignored: ["**/src-tauri/target/**", "**/target/**", "**/node_modules/**"],
    },
  },
  build: { target: "es2021", outDir: "dist" }
});
