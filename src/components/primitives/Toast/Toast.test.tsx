import { describe, expect, it, vi } from 'vitest';

const sonnerMock = vi.hoisted(() =>
  Object.assign(vi.fn(), {
    success: vi.fn(),
    error: vi.fn(),
    dismiss: vi.fn(),
  })
);

vi.mock('sonner', () => ({
  Toaster: () => null,
  toast: sonnerMock,
}));

import { toast } from './Toast';

describe('toast wrapper', () => {
  it('forwards .success() to sonner', () => {
    toast.success('Saved', { description: 'Persisted to disk' });
    expect(sonnerMock.success).toHaveBeenCalledWith('Saved', { description: 'Persisted to disk' });
  });

  it('forwards .error() to sonner', () => {
    toast.error('Boom');
    expect(sonnerMock.error).toHaveBeenCalledWith('Boom', undefined);
  });

  it('forwards .info() to sonner default toast', () => {
    toast.info('Heads up');
    expect(sonnerMock).toHaveBeenCalledWith('Heads up', undefined);
  });

  it('forwards .dismiss()', () => {
    toast.dismiss('abc');
    expect(sonnerMock.dismiss).toHaveBeenCalledWith('abc');
  });
});
