/**
 * ErrorBoundary — Issue #1198
 *
 * A React class-based Error Boundary that wraps the main dashboard route tree.
 * It catches unhandled rendering errors, displays a user-friendly fallback UI
 * with a "Retry" button, and optionally reports the error to a monitoring
 * service (e.g. Sentry) when the REACT_APP_ERROR_REPORTING_ENABLED env var is set.
 *
 * Usage:
 *   <ErrorBoundary>
 *     <App />
 *   </ErrorBoundary>
 *
 * Notes:
 *  - The full error stack is only shown in development mode
 *    (process.env.NODE_ENV === 'development').
 *  - Expected / handled errors such as API 401 responses should be caught and
 *    redirected in the component layer (e.g. with an auth context), not here.
 *    This boundary only catches *unexpected* rendering / JS errors.
 */

import React, { Component, ErrorInfo, ReactNode } from "react";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface Props {
  /** Content to render when no error has occurred. */
  children: ReactNode;
  /**
   * Optional custom fallback element.  When provided, this replaces the
   * built-in fallback UI entirely.
   */
  fallback?: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
  errorInfo: ErrorInfo | null;
}

// ---------------------------------------------------------------------------
// Monitoring hook (optional, controlled by env var)
// ---------------------------------------------------------------------------

/**
 * Reports the error to an external monitoring service if the feature flag is
 * enabled via the REACT_APP_ERROR_REPORTING_ENABLED environment variable.
 *
 * Replace the console.error stub with your Sentry / Datadog SDK call, e.g.:
 *   Sentry.captureException(error, { contexts: { react: { componentStack } } });
 */
function reportError(error: Error, errorInfo: ErrorInfo): void {
  const enabled =
    process.env.REACT_APP_ERROR_REPORTING_ENABLED === "true" ||
    process.env.VITE_ERROR_REPORTING_ENABLED === "true";

  if (!enabled) return;

  // TODO: Replace with your preferred monitoring SDK call.
  // Example with Sentry:
  //   import * as Sentry from "@sentry/react";
  //   Sentry.captureException(error, { extra: { componentStack: errorInfo.componentStack } });
  console.error("[ErrorBoundary] Reporting error to monitoring service:", error, errorInfo);
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null, errorInfo: null };
    this.handleRetry = this.handleRetry.bind(this);
  }

  // React lifecycle: called when a descendant throws during render.
  static getDerivedStateFromError(error: Error): Partial<State> {
    return { hasError: true, error };
  }

  // React lifecycle: called after the error has been captured.
  componentDidCatch(error: Error, errorInfo: ErrorInfo): void {
    this.setState({ errorInfo });
    reportError(error, errorInfo);
  }

  /** Resets the error state so the component tree re-renders. */
  handleRetry(): void {
    this.setState({ hasError: false, error: null, errorInfo: null });
  }

  render(): ReactNode {
    const { hasError, error, errorInfo } = this.state;
    const { children, fallback } = this.props;

    if (!hasError) {
      return children;
    }

    // If a custom fallback was provided, use it.
    if (fallback) {
      return fallback;
    }

    const isDev = process.env.NODE_ENV === "development";

    return (
      <div
        role="alert"
        style={{
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          minHeight: "60vh",
          padding: "2rem",
          textAlign: "center",
          fontFamily: "system-ui, -apple-system, sans-serif",
        }}
      >
        <h1
          style={{
            fontSize: "1.75rem",
            fontWeight: 700,
            marginBottom: "0.75rem",
            color: "#111",
          }}
        >
          Something went wrong
        </h1>

        <p
          style={{
            fontSize: "1rem",
            color: "#555",
            maxWidth: "480px",
            marginBottom: "1.5rem",
            lineHeight: 1.6,
          }}
        >
          An unexpected error occurred while rendering the dashboard. Your vault
          data is safe — this is a display issue only. Please try again.
        </p>

        <button
          onClick={this.handleRetry}
          aria-label="Retry loading the dashboard"
          style={{
            padding: "0.6rem 1.4rem",
            fontSize: "1rem",
            fontWeight: 600,
            color: "#fff",
            background: "#2563eb",
            border: "none",
            borderRadius: "6px",
            cursor: "pointer",
          }}
        >
          Retry
        </button>

        {/* Show full stack trace only in development */}
        {isDev && error && (
          <details
            style={{
              marginTop: "2rem",
              textAlign: "left",
              maxWidth: "800px",
              width: "100%",
              background: "#fef2f2",
              border: "1px solid #fca5a5",
              borderRadius: "6px",
              padding: "1rem",
            }}
          >
            <summary
              style={{
                cursor: "pointer",
                fontWeight: 600,
                color: "#991b1b",
                marginBottom: "0.5rem",
              }}
            >
              Error details (development only)
            </summary>
            <pre
              style={{
                fontSize: "0.8rem",
                whiteSpace: "pre-wrap",
                wordBreak: "break-all",
                color: "#7f1d1d",
                margin: 0,
              }}
            >
              {error.toString()}
              {errorInfo?.componentStack}
            </pre>
          </details>
        )}
      </div>
    );
  }
}

export default ErrorBoundary;
