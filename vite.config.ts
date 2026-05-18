import path from 'node:path';
import react from '@vitejs/plugin-react';
import { type UserConfig, defineConfig } from 'vite';

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(
  (): UserConfig => ({
    plugins: [react()],
    resolve: {
      alias: { '@': path.resolve(__dirname, 'src') },
    },
    clearScreen: false,
    server: {
      port: 1420,
      strictPort: true,
      host: host || false,
      hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
      watch: { ignored: ['**/src-tauri/**'] },
    },
    envPrefix: ['VITE_', 'TAURI_ENV_*'],
    build: {
      target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
      minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
      sourcemap: !!process.env.TAURI_ENV_DEBUG,
      cssMinify: true,
    },
    css: {
      modules: {
        localsConvention: 'camelCaseOnly',
        generateScopedName: process.env.TAURI_ENV_DEBUG
          ? '[name]__[local]__[hash:base64:5]'
          : '[hash:base64:8]',
      },
    },
  })
);
