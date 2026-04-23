import { Component, type ErrorInfo, type ReactNode } from 'react';
import { logger } from '../../utils/logger';

interface Props {
  children: ReactNode;
  fallback?: ReactNode;
}

interface State {
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    logger.error('error_boundary', { message: error.message, stack: info.componentStack });
  }

  render() {
    if (this.state.error) {
      return (
        this.props.fallback ?? (
          <div className="app-error">
            <h1>Bir hata oluştu</h1>
            <p>{this.state.error.message}</p>
          </div>
        )
      );
    }
    return this.props.children;
  }
}
