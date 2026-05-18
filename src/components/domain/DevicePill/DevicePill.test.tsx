import type { AudioDevice } from '@/ipc/bindings';
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import '@/i18n';
import { DevicePill } from './DevicePill';

const device: AudioDevice = {
  id: 'd1',
  name: { es: 'Realtek loopback', en: 'Realtek loopback' },
  desc: { es: 'WASAPI Loopback · 48 kHz', en: 'WASAPI Loopback · 48 kHz' },
  icon: 'monitor',
  recommended: true,
  active: true,
};

describe('DevicePill', () => {
  it('renders device name and description', () => {
    render(<DevicePill device={device} />);
    expect(screen.getByText('Realtek loopback')).toBeInTheDocument();
    expect(screen.getByText('WASAPI Loopback · 48 kHz')).toBeInTheDocument();
  });

  it('renders a fallback when device is undefined', () => {
    render(<DevicePill device={undefined} />);
    expect(screen.getByText('—')).toBeInTheDocument();
  });
});
