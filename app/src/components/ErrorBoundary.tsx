import { Component, type ReactNode } from 'react';
import { Button, EmptyState } from './ui';

export class ErrorBoundary extends Component<
  { children: ReactNode; onReset?: () => void },
  { error: Error | null }
> {
  state = { error: null as Error | null };
  static getDerivedStateFromError(error: Error) {
    return { error };
  }
  render() {
    if (!this.state.error) return this.props.children;
    return (
      <EmptyState
        icon="alert"
        title="Something went wrong in this view"
        action={
          <Button
            variant="primary"
            onClick={() => {
              this.setState({ error: null });
              this.props.onReset?.();
            }}
          >
            Try again
          </Button>
        }
      >
        {this.state.error.message} Your workspaces are saved automatically and have not been affected.
      </EmptyState>
    );
  }
}
