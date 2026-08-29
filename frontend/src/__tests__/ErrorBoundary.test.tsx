/**
 * Tests for ErrorBoundary — Issue #1198
 *
 * Verifies that:
 * 1. When a child component throws, the fallback UI is rendered instead.
 * 2. The fallback UI shows "Something went wrong" and a "Retry" button.
 * 3. The error stack is shown only in development mode.
 * 4. Clicking "Retry" resets the error state and re-renders the children.
 * 5. Expected errors (e.g., API 401 handled in-component) do NOT trigger
 *    the boundary — the boundary only catches unhandled rendering errors.
 * 6. A custom `fallback` prop replaces the default fallback UI.
 *
 * Dependencies: React Testing Library + Jest (or Vitest).
 * No Sentry / monitoring SDK is required.
 */

import React from "react";
import { render, screen, fireEvent } from "@testing-library/react";
import ErrorBoundary from "../components/ErrorBoundary";

// ---------------------------------------------------------------------------
// Helper: a component that throws when its `shouldThrow` prop is true.
// ---------------------------------------------------------------------------
interface ThrowingProps {
  shouldThrow: boolean;
  message?: string;
}

function ThrowingComponent({ shouldThrow, message = "Test render error" }: ThrowingProps) {
  if (shouldThrow) {
    throw new Error(message);
  }
  return <div data-testid="child-content">Dashboard content loaded</div>;
}

// ---------------------------------------------------------------------------
// Suppress React's console.error output during intentional throw tests.
// ---------------------------------------------------------------------------
let consoleErrorSpy: jest.SpyInstance;

beforeEach(() => {
  consoleErrorSpy = jest.spyOn(console, "error").mockImplementation(() => {});
});

afterEach(() => {
  consoleErrorSpy.mockRestore();
});

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("ErrorBoundary", () => {
  it("renders children when no error is thrown", () => {
    render(
      <ErrorBoundary>
        <ThrowingComponent shouldThrow={false} />
      </ErrorBoundary>
    );

    expect(screen.getByTestId("child-content")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("displays the fallback UI when a child component throws", () => {
    render(
      <ErrorBoundary>
        <ThrowingComponent shouldThrow={true} />
      </ErrorBoundary>
    );

    // Fallback region with role="alert" is visible
    expect(screen.getByRole("alert")).toBeInTheDocument();

    // User-friendly heading and message are present
    expect(screen.getByText("Something went wrong")).toBeInTheDocument();
    expect(
      screen.getByText(/An unexpected error occurred while rendering the dashboard/)
    ).toBeInTheDocument();

    // Child content must NOT be visible
    expect(screen.queryByTestId("child-content")).not.toBeInTheDocument();
  });

  it("renders a Retry button in the fallback UI", () => {
    render(
      <ErrorBoundary>
        <ThrowingComponent shouldThrow={true} />
      </ErrorBoundary>
    );

    const retryButton = screen.getByRole("button", { name: /retry/i });
    expect(retryButton).toBeInTheDocument();
  });

  it("resets the error state and re-renders children when Retry is clicked", () => {
    // We need a stateful wrapper to allow toggling shouldThrow after retry.
    function TestWrapper() {
      const [shouldThrow, setShouldThrow] = React.useState(true);
      return (
        <ErrorBoundary>
          {shouldThrow ? (
            // Clicking retry resets the boundary; the child no longer throws.
            <ThrowingComponent
              shouldThrow={true}
              // After mount it throws; we simulate the fixed state via a separate render.
            />
          ) : (
            <div data-testid="child-content">Recovered content</div>
          )}
        </ErrorBoundary>
      );
    }

    const { rerender } = render(
      <ErrorBoundary>
        <ThrowingComponent shouldThrow={true} />
      </ErrorBoundary>
    );

    // Fallback is visible
    expect(screen.getByRole("alert")).toBeInTheDocument();

    // Click retry → boundary resets
    fireEvent.click(screen.getByRole("button", { name: /retry/i }));

    // After reset the boundary tries to render children again.  Re-render with
    // a non-throwing child to simulate the fixed state.
    rerender(
      <ErrorBoundary>
        <ThrowingComponent shouldThrow={false} />
      </ErrorBoundary>
    );

    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByTestId("child-content")).toBeInTheDocument();
  });

  it("hides the error stack in production mode", () => {
    const originalEnv = process.env.NODE_ENV;
    // @ts-ignore
    process.env.NODE_ENV = "production";

    render(
      <ErrorBoundary>
        <ThrowingComponent shouldThrow={true} message="Sensitive stack trace" />
      </ErrorBoundary>
    );

    expect(screen.queryByText(/Sensitive stack trace/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Error details/)).not.toBeInTheDocument();

    // Restore
    // @ts-ignore
    process.env.NODE_ENV = originalEnv;
  });

  it("shows the error stack details in development mode", () => {
    const originalEnv = process.env.NODE_ENV;
    // @ts-ignore
    process.env.NODE_ENV = "development";

    render(
      <ErrorBoundary>
        <ThrowingComponent shouldThrow={true} message="DevOnlyError" />
      </ErrorBoundary>
    );

    // The details element should be present in dev
    expect(screen.getByText(/Error details \(development only\)/)).toBeInTheDocument();

    // @ts-ignore
    process.env.NODE_ENV = originalEnv;
  });

  it("renders a custom fallback element when the fallback prop is provided", () => {
    render(
      <ErrorBoundary fallback={<div data-testid="custom-fallback">Custom error UI</div>}>
        <ThrowingComponent shouldThrow={true} />
      </ErrorBoundary>
    );

    expect(screen.getByTestId("custom-fallback")).toBeInTheDocument();
    expect(screen.queryByText("Something went wrong")).not.toBeInTheDocument();
  });

  it("does not intercept expected errors handled inside child components (API 401 example)", () => {
    // A component that handles a 401 internally (redirects to login) and does
    // NOT throw during render.  The Error Boundary should remain transparent.
    function HandledErrorComponent() {
      const [authed] = React.useState(false);
      if (!authed) {
        // Simulate redirect to login — no throw, just conditional rendering.
        return <div data-testid="login-redirect">Redirecting to login…</div>;
      }
      return <div data-testid="dashboard">Dashboard</div>;
    }

    render(
      <ErrorBoundary>
        <HandledErrorComponent />
      </ErrorBoundary>
    );

    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByTestId("login-redirect")).toBeInTheDocument();
  });
});
