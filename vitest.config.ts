import path from 'node:path';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [react()],
  resolve: { alias: { '@': path.resolve(__dirname, 'src') } },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./tests/setup.ts'],
    css: { modules: { classNameStrategy: 'non-scoped' } },
    // Don't sweep Playwright specs into the Vitest run.
    exclude: ['**/node_modules/**', '**/dist/**', 'tests/e2e/**', 'playwright-report/**'],
    coverage: {
      provider: 'v8',
      include: ['src/**/*.{ts,tsx}'],
      exclude: ['src/**/*.stories.tsx', 'src/ipc/bindings.ts', 'src/i18n/keys.ts', 'src/mock/**'],
      thresholds: {
        'src/components/primitives/**': { lines: 90, statements: 90, branches: 85, functions: 90 },
      },
    },
  },
});
