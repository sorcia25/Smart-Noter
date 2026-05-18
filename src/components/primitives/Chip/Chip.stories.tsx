import type { Meta, StoryObj } from '@storybook/react';
import { Chip } from './Chip';

const meta: Meta<typeof Chip> = {
  component: Chip,
  title: 'Primitives/Chip',
};
export default meta;

type Story = StoryObj<typeof Chip>;

export const Default: Story = { args: { children: 'Filter' } };
export const Accent: Story = { args: { children: 'Selected', variant: 'accent' } };
export const Disabled: Story = { args: { children: 'Read-only', disabled: true } };
