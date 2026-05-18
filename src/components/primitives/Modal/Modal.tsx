import { type ReactNode, useEffect } from 'react';
import { createPortal } from 'react-dom';
import styles from './Modal.module.css';

export interface ModalProps {
  open: boolean;
  onClose: () => void;
  title?: ReactNode;
  subtitle?: ReactNode;
  children?: ReactNode;
  footer?: ReactNode;
}

export function Modal({ open, onClose, title, subtitle, children, footer }: ModalProps) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open, onClose]);

  if (!open) return null;

  const node = (
    // biome-ignore lint/a11y/useKeyWithClickEvents: Esc-to-close is wired via window keydown listener above
    <div
      className={styles.backdrop}
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      {/* biome-ignore lint/a11y/useSemanticElements: native <dialog> requires imperative .showModal() API; we render declaratively via portal */}
      <div className={styles.modal} role="dialog" aria-modal="true">
        {(title || subtitle) && (
          <div className={styles.head}>
            {title && <h2 className={styles.title}>{title}</h2>}
            {subtitle && <p className={styles.sub}>{subtitle}</p>}
          </div>
        )}
        <div className={styles.body}>{children}</div>
        {footer && <div className={styles.foot}>{footer}</div>}
      </div>
    </div>
  );

  return createPortal(node, document.body);
}
