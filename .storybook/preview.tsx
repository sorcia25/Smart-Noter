import type { Preview } from '@storybook/react';
import '../src/assets/fonts/fonts.css';
import '../src/theme/tokens.css';
import '../src/styles/reset.css';
import '../src/styles/globals.css';

const preview: Preview = {
  parameters: {
    backgrounds: {
      default: 'app',
      values: [
        { name: 'app', value: 'var(--bg-app-solid)' },
        { name: 'surface', value: 'var(--bg-surface)' },
      ],
    },
    controls: { matchers: { color: /(background|color)$/i } },
  },
  globalTypes: {
    theme: {
      defaultValue: 'light',
      toolbar: { items: ['light', 'dark'], title: 'Theme' },
    },
  },
  decorators: [
    (Story, ctx) => {
      const theme = (ctx.globals.theme as string) || 'light';
      document.documentElement.setAttribute('data-theme', theme);
      return <Story />;
    },
  ],
};

export default preview;
