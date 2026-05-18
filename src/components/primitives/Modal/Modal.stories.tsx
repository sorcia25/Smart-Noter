import type { Meta, StoryObj } from '@storybook/react';
import { useState } from 'react';
import { Button } from '../Button/Button';
import { Modal } from './Modal';

const meta: Meta<typeof Modal> = {
  component: Modal,
  title: 'Primitives/Modal',
};
export default meta;

type Story = StoryObj<typeof Modal>;

export const SimpleConfirm: Story = {
  render: () => {
    const [open, setOpen] = useState(false);
    return (
      <>
        <Button onClick={() => setOpen(true)}>Open modal</Button>
        <Modal
          open={open}
          onClose={() => setOpen(false)}
          title="Confirm action"
          subtitle="This will discard any unsaved changes."
          footer={
            <>
              <Button variant="ghost" onClick={() => setOpen(false)}>
                Cancel
              </Button>
              <Button variant="primary" onClick={() => setOpen(false)}>
                OK
              </Button>
            </>
          }
        >
          <p style={{ margin: 0, fontSize: 13, lineHeight: 1.6 }}>
            Click the backdrop, press Escape, or use a footer button to close.
          </p>
        </Modal>
      </>
    );
  },
};
