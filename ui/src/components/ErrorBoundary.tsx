/**
 * DiMA Desktop - Error Boundary
 *
 * Catches unhandled React errors and displays a recovery UI
 * instead of a white screen. Provides per-panel and global variants.
 *
 * Key behaviors:
 * - `role="alert"` for screen reader announcement
 * - Uses a `resetCount` key on children to force a full remount on retry,
 *   preventing the same error from recurring immediately
 * - Compact mode still shows a truncated error message for debuggability
 */

import { Component, type ErrorInfo, type ReactNode } from 'react';
import { AlertTriangle, RefreshCw } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface ErrorBoundaryProps {
  children: ReactNode;
  /** Fallback UI rendered on error. If omitted, uses the default recovery screen. */
  fallback?: ReactNode;
  /** Compact mode for per-panel boundaries (less padding, smaller text) */
  compact?: boolean;
  /** Optional label shown in the error UI for context */
  label?: string;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
  resetCount: number;
  retryCount: number;
}

// After this many consecutive retries, stop offering automatic retry to prevent
// infinite error loops that waste resources and confuse users.
const MAX_RETRIES = 3;

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, error: null, resetCount: 0, retryCount: 0 };
  }

  static getDerivedStateFromError(error: Error): Partial<ErrorBoundaryState> {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error(
      `[ErrorBoundary${this.props.label ? `: ${this.props.label}` : ''}]`,
      error,
      errorInfo.componentStack
    );
  }

  handleReset = () => {
    this.setState((prev) => ({
      hasError: false,
      error: null,
      resetCount: prev.resetCount + 1,
      retryCount: prev.retryCount + 1,
    }));
  };

  render() {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }

      const errorMsg = this.state.error?.message || 'An unexpected error occurred.';
      const canRetry = this.state.retryCount < MAX_RETRIES;

      if (this.props.compact) {
        return (
          <div
            role="alert"
            className="flex h-full flex-col items-center justify-center gap-2 p-4 text-center"
          >
            <AlertTriangle className="h-5 w-5 text-destructive" />
            <p className="text-xs text-muted-foreground">
              {this.props.label || 'Component'} encountered an error
            </p>
            <p className="max-w-[200px] truncate text-[10px] text-muted-foreground/70" title={errorMsg}>
              {errorMsg}
            </p>
            {canRetry ? (
              <Button variant="ghost" size="sm" onClick={this.handleReset} className="gap-1">
                <RefreshCw className="h-3 w-3" />
                Retry ({MAX_RETRIES - this.state.retryCount} left)
              </Button>
            ) : (
              <p className="text-[10px] text-destructive/80">Max retries reached</p>
            )}
          </div>
        );
      }

      return (
        <div
          role="alert"
          className="flex h-full flex-col items-center justify-center gap-4 p-8"
        >
          <AlertTriangle className="h-12 w-12 text-destructive" />
          <div className="text-center">
            <h2 className="text-lg font-semibold">Something went wrong</h2>
            <p className="mt-1 max-w-md text-sm text-muted-foreground">
              {errorMsg}
            </p>
          </div>
          {canRetry ? (
            <Button onClick={this.handleReset} className="gap-2">
              <RefreshCw className="h-4 w-4" />
              Try Again ({MAX_RETRIES - this.state.retryCount} left)
            </Button>
          ) : (
            <p className="text-sm text-destructive">
              This component has failed {MAX_RETRIES} times. Please reload the application.
            </p>
          )}
        </div>
      );
    }

    // Key on resetCount forces children to fully remount after a retry,
    // so the same error doesn't recur from stale component state.
    // h-full w-full min-h-0 ensures this wrapper participates in the flex
    // height chain — without it, child charts' height:100% resolves against
    // an unsized parent and collapses to minimum height. (Fix 5.71)
    return <div key={this.state.resetCount} className="h-full w-full min-h-0">{this.props.children}</div>;
  }
}
