import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '@/i18n';
import { WindowChrome } from './WindowChrome';

const { getCurrentWebviewWindow } = vi.hoisted(() => ({
  getCurrentWebviewWindow: vi.fn(),
}));

vi.mock('@tauri-apps/api/webviewWindow', () => ({ getCurrentWebviewWindow }));

beforeEach(() => {
  getCurrentWebviewWindow.mockReset();
  getCurrentWebviewWindow.mockReturnValue({
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
    close: vi.fn(),
  });
});

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

  it('still renders outside the Tauri runtime (browser/dev-server context)', () => {
    // Outside Tauri, getCurrentWebviewWindow() throws reading window.__TAURI_INTERNALS__.
    // The shell must degrade gracefully instead of crashing the whole app.
    getCurrentWebviewWindow.mockImplementation(() => {
      throw new TypeError("Cannot read properties of undefined (reading 'metadata')");
    });
    render(<WindowChrome title="X" />);
    expect(screen.getByText(/Smart Noter — X/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Cerrar|Close/ })).toBeInTheDocument();
  });
});
