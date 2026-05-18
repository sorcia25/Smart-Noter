import type { Meta, StoryObj } from '@storybook/react';
import { useState } from 'react';
import { SegmentedControl } from './SegmentedControl';

const meta: Meta<typeof SegmentedControl> = {
  component: SegmentedControl,
  title: 'Primitives/SegmentedControl',
};
export default meta;

type Story = StoryObj<typeof SegmentedControl>;

const captureModes = [
  { value: 'system', label: 'Sistema' },
  { value: 'mic', label: 'Mic' },
  { value: 'mix', label: 'Mezcla' },
];

export const Default: Story = {
  render: () => {
    const [v, setV] = useState('system');
    return <SegmentedControl<string> value={v} options={captureModes} onChange={setV} />;
  },
};

const speeds = [
  { value: '0.5x', label: '0.5×' },
  { value: '1x', label: '1×' },
  { value: '1.5x', label: '1.5×' },
  { value: '2x', label: '2×' },
];

export const FourOptions: Story = {
  render: () => {
    const [v, setV] = useState('1x');
    return <SegmentedControl<string> value={v} options={speeds} onChange={setV} />;
  },
};
