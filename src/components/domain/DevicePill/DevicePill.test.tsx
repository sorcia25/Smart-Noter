import type { AudioDevice } from '@/ipc/bindings';
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import '@/i18n';
import { DevicePill } from './DevicePill';

const device: AudioDevice = {
  id: 'd1',
  name: 'Realtek loopback',
  kind: 'loopback',
  sampleRate: 48000,
  channels: 2,
  isDefault: false,
  recommended: true,
};

describe('DevicePill', () => {
  it('renders device name and kind caption', () => {
    render(<DevicePill device={device} />);
    expect(screen.getByText('Realtek loopback')).toBeInTheDocument();
    // Default lang is 'es' in tests (i18n initialised above)
    expect(screen.getByText('Audio del sistema')).toBeInTheDocument();
  });

  it('appends Predeterminado when isDefault is true', () => {
    const defaultDevice: AudioDevice = { ...device, isDefault: true };
    render(<DevicePill device={defaultDevice} />);
    expect(screen.getByText(/Predeterminado/)).toBeInTheDocument();
  });

  it('renders Micrófono caption for input kind', () => {
    const micDevice: AudioDevice = { ...device, kind: 'input' };
    render(<DevicePill device={micDevice} />);
    expect(screen.getByText('Micrófono')).toBeInTheDocument();
  });

  it('renders a fallback when device is undefined', () => {
    render(<DevicePill device={undefined} />);
    expect(screen.getByText('—')).toBeInTheDocument();
  });
});
