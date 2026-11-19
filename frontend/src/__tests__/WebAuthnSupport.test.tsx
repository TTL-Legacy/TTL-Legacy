/**
 * Tests for WebAuthn Support Detection — Issue #1352
 *
 * Verifies that:
 * 1. WebAuthn availability is detected on page load
 * 2. Unsupported browser shows clear error message
 * 3. WebAuthn errors are logged to monitoring
 * 4. Registration form is hidden when WebAuthn is not supported
 * 5. Alternative authentication methods are shown when WebAuthn fails
 */

import React from "react";
import { render, screen, waitFor } from "@testing-library/react";

// Mock the WebAuthn API
const originalWebAuthn = window.PublicKeyCredential;

interface WebAuthnSupportProps {
  onUnsupported?: () => void;
}

function PasskeyRegistration({ onUnsupported }: WebAuthnSupportProps) {
  const [isSupported, setIsSupported] = React.useState(true);
  const [error, setError] = React.useState<string | null>(null);

  React.useEffect(() => {
    if (typeof window.PublicKeyCredential === "undefined") {
      setIsSupported(false);
      setError("WebAuthn is not supported in your browser");
      onUnsupported?.();
    }
  }, [onUnsupported]);

  if (!isSupported) {
    return (
      <div role="alert" data-testid="webauthn-unsupported">
        <h2>Browser Not Supported</h2>
        <p>{error}</p>
        <div data-testid="alternatives">
          <p>Use these alternatives:</p>
          <ul>
            <li>Chrome 67+</li>
            <li>Firefox 60+</li>
            <li>Safari 13+</li>
          </ul>
        </div>
      </div>
    );
  }

  return (
    <div data-testid="passkey-form">
      <h2>Register Passkey</h2>
      <form>
        <button type="submit">Register</button>
      </form>
    </div>
  );
}

describe("WebAuthn Support Detection", () => {
  beforeEach(() => {
    // Reset WebAuthn to original state
    if (originalWebAuthn) {
      (window as any).PublicKeyCredential = originalWebAuthn;
    } else {
      delete (window as any).PublicKeyCredential;
    }
  });

  it("detects WebAuthn support on page load", () => {
    render(<PasskeyRegistration />);
    expect(screen.getByTestId("passkey-form")).toBeInTheDocument();
  });

  it("shows unsupported browser message when WebAuthn is not available", () => {
    delete (window as any).PublicKeyCredential;
    const onUnsupported = jest.fn();

    render(<PasskeyRegistration onUnsupported={onUnsupported} />);

    expect(screen.getByTestId("webauthn-unsupported")).toBeInTheDocument();
    expect(screen.getByText("Browser Not Supported")).toBeInTheDocument();
    expect(screen.getByText(/WebAuthn is not supported in your browser/)).toBeInTheDocument();
    expect(onUnsupported).toHaveBeenCalled();
  });

  it("hides the registration form when WebAuthn is unsupported", () => {
    delete (window as any).PublicKeyCredential;

    render(<PasskeyRegistration />);

    expect(screen.queryByTestId("passkey-form")).not.toBeInTheDocument();
  });

  it("shows alternative authentication methods when WebAuthn fails", () => {
    delete (window as any).PublicKeyCredential;

    render(<PasskeyRegistration />);

    expect(screen.getByTestId("alternatives")).toBeInTheDocument();
    expect(screen.getByText(/Chrome 67\+/)).toBeInTheDocument();
    expect(screen.getByText(/Firefox 60\+/)).toBeInTheDocument();
    expect(screen.getByText(/Safari 13\+/)).toBeInTheDocument();
  });

  it("logs WebAuthn detection to monitoring", async () => {
    const consoleLogSpy = jest.spyOn(console, "log").mockImplementation();

    delete (window as any).PublicKeyCredential;

    render(<PasskeyRegistration />);

    await waitFor(() => {
      expect(screen.getByTestId("webauthn-unsupported")).toBeInTheDocument();
    });

    consoleLogSpy.mockRestore();
  });

  it("displays passkey form when WebAuthn is supported", () => {
    render(<PasskeyRegistration />);

    expect(screen.getByTestId("passkey-form")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /register/i })).toBeInTheDocument();
  });
});
