import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { Modal } from './Modal';

describe('Modal', () => {
  it('does not render when open is false', () => {
    render(
      <Modal open={false} onClose={() => {}} title="Hi">
        body
      </Modal>
    );
    expect(screen.queryByText('body')).not.toBeInTheDocument();
  });

  it('renders title, body and footer when open', () => {
    render(
      <Modal
        open
        onClose={() => {}}
        title="Export"
        subtitle="Choose a format"
        footer={<span>FOOT</span>}
      >
        body
      </Modal>
    );
    expect(screen.getByText('Export')).toBeInTheDocument();
    expect(screen.getByText('Choose a format')).toBeInTheDocument();
    expect(screen.getByText('body')).toBeInTheDocument();
    expect(screen.getByText('FOOT')).toBeInTheDocument();
  });

  it('closes when the backdrop is clicked', async () => {
    const onClose = vi.fn();
    render(
      <Modal open onClose={onClose} title="x">
        body
      </Modal>
    );
    const backdrop = screen.getByText('body').closest('[role="dialog"]')?.parentElement;
    if (!backdrop) throw new Error('backdrop not found');
    await userEvent.click(backdrop);
    expect(onClose).toHaveBeenCalled();
  });
});
