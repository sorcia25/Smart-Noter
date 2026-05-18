import type { Meta, StoryObj } from '@storybook/react';
import { Input } from './Input';

const meta: Meta<typeof Input> = {
  component: Input,
  title: 'Primitives/Input',
};
export default meta;

type Story = StoryObj<typeof Input>;

export const Default: Story = {
  args: { placeholder: 'Type something…' },
};

export const WithLabel: Story = {
  args: { label: 'Meeting name', placeholder: 'e.g. Steering committee' },
};

export const Password: Story = {
  args: { label: 'API key', type: 'password', placeholder: 'sk-…' },
};
