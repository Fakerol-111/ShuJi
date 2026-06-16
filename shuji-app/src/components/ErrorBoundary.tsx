import { Component, type ReactNode } from 'react';
import { Button } from './ui/Button';
import { SealLogo } from './SealLogo';
import i18n from '../i18n/config';

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  handleReload = () => {
    this.setState({ hasError: false, error: null });
  };

  render() {
    if (this.state.hasError) {
      return (
        <div className="h-screen bg-surface-paper flex items-center justify-center">
          <div className="max-w-md mx-auto p-8 text-center">
            <SealLogo size={40} />
            <h1 className="font-display text-display font-bold text-ink-900 mt-4">{i18n.t('error.genericTitle')}</h1>
            <p className="text-body text-ink-600 mt-2 mb-4">{i18n.t('error.systemError') + i18n.t('common.reload')}</p>
            <details className="text-left mb-6">
              <summary className="text-caption text-ink-500 cursor-pointer hover:text-ink-700">
                {i18n.t('common.error')}
              </summary>
              <pre className="mt-2 p-3 bg-ink-100 rounded-lg text-caption text-ink-700 overflow-x-auto whitespace-pre-wrap max-h-48 overflow-y-auto">
                {this.state.error?.message || i18n.t('common.unknownError')}
              </pre>
            </details>
            <div className="flex justify-center gap-3">
              <Button variant="secondary" onClick={() => window.location.reload()}>
                {i18n.t('common.reload')}
              </Button>
              <Button variant="seal" onClick={this.handleReload}>
                {i18n.t('common.retry')}
              </Button>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
