import type { Meta, StoryObj } from '@storybook/react';
import { useState } from 'react';
import { SearchBox } from './SearchBox';

const meta: Meta<typeof SearchBox> = {
  component: SearchBox,
  title: 'Primitives/SearchBox',
};
export default meta;

type Story = StoryObj<typeof SearchBox>;

export const Empty: Story = {
  render: () => {
    const [v, setV] = useState('');
    return <SearchBox value={v} onChange={setV} placeholder="Buscar reunión…" />;
  },
};

export const Prefilled: Story = {
  render: () => {
    const [v, setV] = useState('Comité');
    return <SearchBox value={v} onChange={setV} placeholder="Buscar reunión…" />;
  },
};
