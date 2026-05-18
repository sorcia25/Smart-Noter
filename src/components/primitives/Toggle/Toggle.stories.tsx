import type { Meta, StoryObj } from '@storybook/react';
import { useState } from 'react';
import { Toggle } from './Toggle';

const meta: Meta<typeof Toggle> = {
  component: Toggle,
  title: 'Primitives/Toggle',
};
export default meta;

type Story = StoryObj<typeof Toggle>;

export const Off: Story = {
  render: () => {
    const [on, setOn] = useState(false);
    return <Toggle on={on} onChange={setOn} aria-label="example" />;
  },
};

export const On: Story = {
  render: () => {
    const [on, setOn] = useState(true);
    return <Toggle on={on} onChange={setOn} aria-label="example" />;
  },
};
