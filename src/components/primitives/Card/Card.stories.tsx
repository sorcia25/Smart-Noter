import type { Meta, StoryObj } from '@storybook/react';
import { Card } from './Card';

const meta: Meta<typeof Card> = {
  component: Card,
  title: 'Primitives/Card',
};
export default meta;

type Story = StoryObj<typeof Card>;

export const Default: Story = {
  args: { children: 'A simple card surface.' },
};

export const Padded: Story = {
  args: { padded: true, children: 'Card with 18px padding.' },
};
