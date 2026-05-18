import type { Meta, StoryObj } from '@storybook/react';
import { Button } from './Button';

const meta: Meta<typeof Button> = {
  component: Button,
  title: 'Primitives/Button',
};
export default meta;

type Story = StoryObj<typeof Button>;

export const Default: Story = { args: { children: 'Default' } };
export const Primary: Story = { args: { children: 'Primary', variant: 'primary' } };
export const Ghost: Story = { args: { children: 'Ghost', variant: 'ghost' } };
export const Danger: Story = { args: { children: 'Danger', variant: 'danger' } };
export const Loading: Story = { args: { children: 'Loading…', loading: true } };
export const Disabled: Story = { args: { children: 'Disabled', disabled: true } };
export const Small: Story = { args: { children: 'Small', size: 'sm' } };
