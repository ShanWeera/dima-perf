import { defineConfig } from 'vitest/config';
import path from 'path';

export default defineConfig({
  test: {
    // Use 'node' environment by default for pure utility tests.
    // Individual test files that need DOM can override with
    // `// @vitest-environment jsdom` at the top of the file.
    environment: 'node',
    globals: true,
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
});
