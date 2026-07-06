import { Component, type ErrorInfo, type ReactNode } from "react";

type ErrorBoundaryProps = {
  children: ReactNode;
  /** Changing this value resets the boundary (e.g. the active screen id). */
  resetKey?: unknown;
};

type ErrorBoundaryState = {
  error: Error | null;
};

/** Contains a render error to its subtree so one broken screen or card cannot
 * white-screen the whole app. Resets automatically when resetKey changes. */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidUpdate(prev: ErrorBoundaryProps) {
    if (prev.resetKey !== this.props.resetKey && this.state.error) {
      this.setState({ error: null });
    }
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    // Surface the detail in the console; the UI stays usable.
    console.error("Screen render failed:", error, info.componentStack);
  }

  render() {
    if (this.state.error) {
      return (
        <div className="flex h-full items-center justify-center p-8">
          <div className="max-w-md rounded-card border border-edge bg-raised p-6 text-center shadow-card">
            <p className="text-[15px] font-semibold text-ink">Something Went Wrong</p>
            <p className="mt-2 text-[13px] text-ink-muted">
              This view hit an error and could not render. Switching screens or reopening the app
              usually clears it.
            </p>
            <p className="mt-3 break-words font-mono text-[11.5px] text-danger">
              {this.state.error.message}
            </p>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}
