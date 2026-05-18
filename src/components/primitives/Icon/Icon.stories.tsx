import type { Meta, StoryObj } from '@storybook/react';
import { Icon } from './Icon';
import { ICONS, type IconName } from './icons';

const meta: Meta<typeof Icon> = {
  component: Icon,
  title: 'Primitives/Icon',
};
export default meta;

type Story = StoryObj<typeof Icon>;

export const Home: Story = { args: { name: 'home', size: 24 } };
export const RecordFilled: Story = { args: { name: 'record', size: 24, stroke: '#10b981' } };
export const Search: Story = { args: { name: 'search', size: 24 } };

export const Gallery: Story = {
  render: () => (
    <div style={{ display: 'grid', gridTemplateColumns: 'repeat(8, 1fr)', gap: 12, padding: 16 }}>
      {(Object.keys(ICONS) as IconName[]).map((name) => (
        <div
          key={name}
          style={{
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            gap: 4,
            fontSize: 10,
          }}
        >
          <Icon name={name} size={20} />
          <span>{name}</span>
        </div>
      ))}
    </div>
  ),
};
