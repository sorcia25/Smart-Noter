import { Component, type ErrorInfo, type ReactNode } from 'react';
import i18n from './i18n';
import { tauri } from './ipc/commands';

interface State {
  error: Error | null;
}

export class ErrorBoundary extends Component<{ children: ReactNode }, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    void tauri.logFrontendError('error', error.message, error.stack ?? info.componentStack ?? '');
  }

  render() {
    if (this.state.error) {
      return (
        <div style={{ padding: 32, fontFamily: 'system-ui' }}>
          <h1>{i18n.t('errorTitle')}</h1>
          <pre style={{ whiteSpace: 'pre-wrap' }}>{this.state.error.message}</pre>
        </div>
      );
    }
    return this.props.children;
  }
}
