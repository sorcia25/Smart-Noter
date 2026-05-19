import { type ReactNode, useEffect, useState } from 'react';
import { type ExternalToast, Toaster as SonnerToaster, toast as sonnerToast } from 'sonner';

export interface ToastProviderProps {
  position?:
    | 'top-right'
    | 'top-left'
    | 'bottom-right'
    | 'bottom-left'
    | 'top-center'
    | 'bottom-center';
}

/**
 * Mounts the sonner Toaster with theme-aware styling. Reads `data-theme` off
 * `<html>` and follows changes so toasts stay in sync with our tokens.
 */
export function ToastProvider({ position = 'bottom-right' }: ToastProviderProps) {
  const [theme, setTheme] = useState<'light' | 'dark'>('light');

  useEffect(() => {
    const read = () =>
      setTheme(
        (document.documentElement.getAttribute('data-theme') as 'light' | 'dark') ?? 'light'
      );
    read();
    const observer = new MutationObserver(read);
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['data-theme'],
    });
    return () => observer.disconnect();
  }, []);

  return <SonnerToaster position={position} theme={theme} richColors closeButton />;
}

export interface ToastOptions extends Pick<ExternalToast, 'description' | 'duration' | 'id'> {}

/** Typed wrapper around sonner so callers see only the variants we support. */
export const toast = {
  success(message: ReactNode, opts?: ToastOptions) {
    return sonnerToast.success(message, opts);
  },
  info(message: ReactNode, opts?: ToastOptions) {
    return sonnerToast(message, opts);
  },
  error(message: ReactNode, opts?: ToastOptions) {
    return sonnerToast.error(message, opts);
  },
  dismiss(id?: string | number) {
    return sonnerToast.dismiss(id);
  },
};

export type Toast = typeof toast;
