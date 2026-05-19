import type { Meta, StoryObj } from '@storybook/react';
import { Button } from '../Button/Button';
import { ToastProvider, toast } from './Toast';

const meta: Meta<typeof ToastProvider> = {
  component: ToastProvider,
  title: 'Primitives/Toast',
};
export default meta;

type Story = StoryObj<typeof ToastProvider>;

export const Demo: Story = {
  render: () => (
    <div style={{ display: 'flex', gap: 12 }}>
      <ToastProvider />
      <Button
        variant="primary"
        onClick={() => toast.success('Saved!', { description: '3 actions persisted.' })}
      >
        Success
      </Button>
      <Button onClick={() => toast.info('Heads up', { description: 'Something happened.' })}>
        Info
      </Button>
      <Button variant="danger" onClick={() => toast.error('Failed', { description: 'IPC error.' })}>
        Error
      </Button>
    </div>
  ),
};
