import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '@/i18n';
import { WindowChrome } from './WindowChrome';

vi.mock('@tauri-apps/api/webviewWindow', () => ({
  getCurrentWebviewWindow: () => ({
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
    close: vi.fn(),
  }),
}));

describe('WindowChrome', () => {
  it('renders app name + provided title', () => {
    render(<WindowChrome title="Dashboard" />);
    expect(screen.getByText(/Smart Noter — Dashboard/)).toBeInTheDocument();
  });

  it('renders three window controls', () => {
    render(<WindowChrome title="X" />);
    expect(screen.getByRole('button', { name: /Minimizar|Minimize/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Maximizar|Maximize/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Cerrar|Close/ })).toBeInTheDocument();
  });
});
