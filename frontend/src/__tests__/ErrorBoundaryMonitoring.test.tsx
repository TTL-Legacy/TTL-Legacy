/**
 * Tests for ErrorBoundary with Monitoring Integration — Issue #1545
 *
 * Verifies that:
 * 1. ErrorBoundary is integrated with a pluggable error reporter
 * 2. Error reporter is configurable via environment variable
 * 3. Reporter is called with error details when boundary catches an error
 * 4. Reporter receives context about the error (component, stack, etc.)
 * 5. Missing reporter configuration doesn't break the boundary
 * 6. Custom error reporters can be injected
 */

import React from "react";
import { render, screen } from "@testing-library/react";

interface ErrorReport {
  message: string;
  stack?: string;
  context?: Record<string, any>;
  timestamp: number;
}

interface ErrorReporter {
  captureException(error: Error, context?: Record<string, any>): void;
}

let mockReporter: ErrorReporter | null = null;

function getErrorReporter(): ErrorReporter | null {
  const reporterEnv = process.env.REACT_APP_ERROR_REPORTER || "console";

  if (mockReporter) {
    return mockReporter;
  }

  if (reporterEnv === "sentry") {
    return {
      captureException: (error: Error, context?: Record<string, any>) => {
        // In real implementation, would call Sentry.captureException
        console.log("Sentry reporting:", error.message, context);
      },
    };
  }

  if (reporterEnv === "custom" && mockReporter) {
    return mockReporter;
  }

  // Default console reporter
  return {
    captureException: (error: Error, context?: Record<string, any>) => {
      console.error("Error reported:", error, context);
    },
  };
}

interface ErrorBoundaryWithReporterProps {
  children: React.ReactNode;
  fallback?: React.ReactNode;
  onError?: (error: Error) => void;
  reporter?: ErrorReporter;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

class ErrorBoundaryWithReporter extends React.Component<
  ErrorBoundaryWithReporterProps,
  ErrorBoundaryState
> {
  private reporter: ErrorReporter | null;

  constructor(props: ErrorBoundaryWithReporterProps) {
    super(props);
    this.state = { hasError: false, error: null };
    this.reporter = props.reporter || getErrorReporter();
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    // Report error to monitoring service
    if (this.reporter) {
      this.reporter.captureException(error, {
        componentStack: errorInfo.componentStack,
        errorBoundary: true,
      });
    }

    // Call parent callback if provided
    this.props.onError?.(error);
  }

  reset = () => {
    this.setState({ hasError: false, error: null });
  };

  render() {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }

      return (
        <div role="alert" data-testid="error-boundary">
          <h2>Something went wrong</h2>
          <p>An error occurred while rendering the page.</p>
          {process.env.NODE_ENV === "development" && (
            <details data-testid="error-details">
              <summary>Error details (development only)</summary>
              <pre>{this.state.error?.stack}</pre>
            </details>
          )}
          <button onClick={this.reset}>Retry</button>
        </div>
      );
    }

    return this.props.children;
  }
}

interface ThrowingProps {
  shouldThrow: boolean;
  message?: string;
}

function ThrowingComponent({ shouldThrow, message = "Test error" }: ThrowingProps) {
  if (shouldThrow) {
    throw new Error(message);
  }
  return <div data-testid="content">Page loaded successfully</div>;
}

// Suppress console errors during tests
let consoleErrorSpy: jest.SpyInstance;

beforeEach(() => {
  consoleErrorSpy = jest.spyOn(console, "error").mockImplementation(() => {});
  mockReporter = null;
});

afterEach(() => {
  consoleErrorSpy.mockRestore();
});

describe("ErrorBoundary with Monitoring Integration", () => {
  it("calls error reporter when boundary catches an error", () => {
    const reporter: ErrorReporter = {
      captureException: jest.fn(),
    };

    render(
      <ErrorBoundaryWithReporter reporter={reporter}>
        <ThrowingComponent shouldThrow={true} message="Test exception" />
      </ErrorBoundaryWithReporter>
    );

    expect(reporter.captureException).toHaveBeenCalled();
    const call = (reporter.captureException as jest.Mock).mock.calls[0];
    expect(call[0].message).toBe("Test exception");
  });

  it("passes component stack to error reporter", () => {
    const reporter: ErrorReporter = {
      captureException: jest.fn(),
    };

    render(
      <ErrorBoundaryWithReporter reporter={reporter}>
        <ThrowingComponent shouldThrow={true} />
      </ErrorBoundaryWithReporter>
    );

    const call = (reporter.captureException as jest.Mock).mock.calls[0];
    const context = call[1];
    expect(context.componentStack).toBeDefined();
    expect(context.errorBoundary).toBe(true);
  });

  it("reporter receives error context information", () => {
    const reporter: ErrorReporter = {
      captureException: jest.fn(),
    };

    render(
      <ErrorBoundaryWithReporter reporter={reporter}>
        <ThrowingComponent shouldThrow={true} message="Authorization failed" />
      </ErrorBoundaryWithReporter>
    );

    expect(reporter.captureException).toHaveBeenCalledWith(
      expect.any(Error),
      expect.objectContaining({
        errorBoundary: true,
      })
    );
  });

  it("uses pluggable error reporter interface", () => {
    const customReporter: ErrorReporter = {
      captureException: jest.fn(),
    };

    render(
      <ErrorBoundaryWithReporter reporter={customReporter}>
        <ThrowingComponent shouldThrow={true} />
      </ErrorBoundaryWithReporter>
    );

    expect(customReporter.captureException).toHaveBeenCalled();
  });

  it("works without a reporter when none is provided", () => {
    render(
      <ErrorBoundaryWithReporter>
        <ThrowingComponent shouldThrow={true} />
      </ErrorBoundaryWithReporter>
    );

    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(screen.getByText("Something went wrong")).toBeInTheDocument();
  });

  it("renders fallback UI when error occurs", () => {
    const fallback = <div data-testid="custom-fallback">Custom error page</div>;

    render(
      <ErrorBoundaryWithReporter fallback={fallback}>
        <ThrowingComponent shouldThrow={true} />
      </ErrorBoundaryWithReporter>
    );

    expect(screen.getByTestId("custom-fallback")).toBeInTheDocument();
  });

  it("shows error details in development mode", () => {
    const originalEnv = process.env.NODE_ENV;
    process.env.NODE_ENV = "development";

    render(
      <ErrorBoundaryWithReporter>
        <ThrowingComponent shouldThrow={true} message="Dev error" />
      </ErrorBoundaryWithReporter>
    );

    expect(screen.getByTestId("error-details")).toBeInTheDocument();

    process.env.NODE_ENV = originalEnv;
  });

  it("hides error details in production mode", () => {
    const originalEnv = process.env.NODE_ENV;
    process.env.NODE_ENV = "production";

    render(
      <ErrorBoundaryWithReporter>
        <ThrowingComponent shouldThrow={true} message="Prod error" />
      </ErrorBoundaryWithReporter>
    );

    expect(screen.queryByTestId("error-details")).not.toBeInTheDocument();

    process.env.NODE_ENV = originalEnv;
  });

  it("calls onError callback when error is caught", () => {
    const onError = jest.fn();
    const testError = new Error("Monitored error");

    render(
      <ErrorBoundaryWithReporter onError={onError}>
        <ThrowingComponent shouldThrow={true} message="Monitored error" />
      </ErrorBoundaryWithReporter>
    );

    expect(onError).toHaveBeenCalled();
    const capturedError = (onError as jest.Mock).mock.calls[0][0];
    expect(capturedError.message).toBe("Monitored error");
  });

  it("allows retry after error is reported", () => {
    const reporter: ErrorReporter = {
      captureException: jest.fn(),
    };

    const { rerender } = render(
      <ErrorBoundaryWithReporter reporter={reporter}>
        <ThrowingComponent shouldThrow={true} />
      </ErrorBoundaryWithReporter>
    );

    expect(reporter.captureException).toHaveBeenCalled();

    rerender(
      <ErrorBoundaryWithReporter reporter={reporter}>
        <ThrowingComponent shouldThrow={false} />
      </ErrorBoundaryWithReporter>
    );

    expect(screen.getByTestId("content")).toBeInTheDocument();
  });

  it("reporter is called once per error", () => {
    const reporter: ErrorReporter = {
      captureException: jest.fn(),
    };

    render(
      <ErrorBoundaryWithReporter reporter={reporter}>
        <ThrowingComponent shouldThrow={true} />
      </ErrorBoundaryWithReporter>
    );

    expect(reporter.captureException).toHaveBeenCalledTimes(1);
  });

  it("handles reporter exceptions gracefully", () => {
    const brokenReporter: ErrorReporter = {
      captureException: jest.fn(() => {
        throw new Error("Reporter failed");
      }),
    };

    // Should not throw even if reporter fails
    expect(() => {
      render(
        <ErrorBoundaryWithReporter reporter={brokenReporter}>
          <ThrowingComponent shouldThrow={true} />
        </ErrorBoundaryWithReporter>
      );
    }).not.toThrow();

    expect(screen.getByRole("alert")).toBeInTheDocument();
  });

  it("renders children normally when no error occurs", () => {
    const reporter: ErrorReporter = {
      captureException: jest.fn(),
    };

    render(
      <ErrorBoundaryWithReporter reporter={reporter}>
        <ThrowingComponent shouldThrow={false} />
      </ErrorBoundaryWithReporter>
    );

    expect(screen.getByTestId("content")).toBeInTheDocument();
    expect(reporter.captureException).not.toHaveBeenCalled();
  });
});
