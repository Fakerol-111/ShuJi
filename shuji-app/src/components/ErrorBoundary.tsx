import { Component, type ReactNode } from 'react';
import { Button } from './ui/Button';
import { SealLogo } from './SealLogo';

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
            <h1 className="font-display text-display font-bold text-ink-900 mt-4">出了点问题</h1>
            <p className="text-body text-ink-600 mt-2 mb-4">
              枢机遇到了一个意外错误。请尝试重新加载。
            </p>
            <details className="text-left mb-6">
              <summary className="text-caption text-ink-500 cursor-pointer hover:text-ink-700">
                查看错误详情
              </summary>
              <pre className="mt-2 p-3 bg-ink-100 rounded-lg text-caption text-ink-700 overflow-x-auto whitespace-pre-wrap max-h-48 overflow-y-auto">
                {this.state.error?.message || '未知错误'}
              </pre>
            </details>
            <div className="flex justify-center gap-3">
              <Button variant="secondary" onClick={() => window.location.reload()}>
                重新加载
              </Button>
              <Button variant="seal" onClick={this.handleReload}>
                重试
              </Button>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
