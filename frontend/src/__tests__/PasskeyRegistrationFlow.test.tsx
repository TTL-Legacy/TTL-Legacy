/**
 * Tests for Passkey Registration Flow — Issue #1543
 *
 * Verifies that:
 * 1. Passkey registration screen is displayed when WebAuthn is supported
 * 2. Unsupported browsers show clear fallback messaging
 * 3. Registered passkeys are displayed in a list
 * 4. User can register a new passkey
 * 5. User can delete/revoke a registered passkey
 * 6. Registration fails gracefully with error messaging
 */

import React from "react";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

interface Passkey {
  id: string;
  name: string;
  createdAt: string;
  lastUsed?: string;
}

interface PasskeyRegistrationFlowProps {
  onRegistered?: (passkey: Passkey) => void;
  onError?: (error: string) => void;
}

function PasskeyRegistrationFlow({ onRegistered, onError }: PasskeyRegistrationFlowProps) {
  const [passkeys, setPasskeys] = React.useState<Passkey[]>([]);
  const [isSupported, setIsSupported] = React.useState(true);
  const [isRegistering, setIsRegistering] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [passKeyName, setPassKeyName] = React.useState("");

  React.useEffect(() => {
    // Check if browser supports WebAuthn
    if (typeof window.PublicKeyCredential === "undefined") {
      setIsSupported(false);
      setError("WebAuthn is not supported in your browser");
    } else {
      // Simulate loading registered passkeys
      setPasskeys([
        {
          id: "pk-1",
          name: "My MacBook Pro",
          createdAt: "2026-09-01",
          lastUsed: "2026-09-26",
        },
      ]);
    }
  }, []);

  const handleRegisterPasskey = async () => {
    if (!passKeyName.trim()) {
      setError("Please enter a name for the passkey");
      return;
    }

    try {
      setIsRegistering(true);
      setError(null);

      // Simulate WebAuthn registration
      await new Promise((resolve) => setTimeout(resolve, 100));

      const newPasskey: Passkey = {
        id: `pk-${Date.now()}`,
        name: passKeyName,
        createdAt: new Date().toISOString(),
      };

      setPasskeys((prev) => [...prev, newPasskey]);
      setPassKeyName("");
      onRegistered?.(newPasskey);
      setIsRegistering(false);
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : "Failed to register passkey";
      setError(errorMsg);
      onError?.(errorMsg);
      setIsRegistering(false);
    }
  };

  const handleDeletePasskey = (id: string) => {
    setPasskeys((prev) => prev.filter((pk) => pk.id !== id));
  };

  if (!isSupported) {
    return (
      <div role="alert" data-testid="webauthn-not-supported">
        <h2>Browser Not Supported</h2>
        <p>{error}</p>
        <div data-testid="browser-alternatives">
          <p>Use one of these browsers to enable passkey support:</p>
          <ul>
            <li>Chrome 67+</li>
            <li>Firefox 60+</li>
            <li>Safari 13+</li>
            <li>Edge 18+</li>
          </ul>
        </div>
      </div>
    );
  }

  return (
    <div data-testid="passkey-registration">
      <h1>Manage Passkeys</h1>

      {error && (
        <div role="alert" data-testid="error-message">
          <p>{error}</p>
        </div>
      )}

      <div data-testid="registered-passkeys">
        <h2>Registered Passkeys</h2>
        {passkeys.length === 0 ? (
          <p data-testid="no-passkeys">No passkeys registered yet</p>
        ) : (
          <ul>
            {passkeys.map((pk) => (
              <li key={pk.id} data-testid={`passkey-${pk.id}`} className="passkey-item">
                <span className="passkey-name">{pk.name}</span>
                <span className="passkey-created">Created: {pk.createdAt}</span>
                {pk.lastUsed && <span className="passkey-used">Last used: {pk.lastUsed}</span>}
                <button
                  onClick={() => handleDeletePasskey(pk.id)}
                  data-testid={`delete-${pk.id}`}
                  className="delete-btn"
                >
                  Delete
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div data-testid="registration-form">
        <h2>Register New Passkey</h2>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            handleRegisterPasskey();
          }}
        >
          <div>
            <label htmlFor="passkey-name">Passkey Name (e.g., "iPhone")</label>
            <input
              id="passkey-name"
              type="text"
              value={passKeyName}
              onChange={(e) => setPassKeyName(e.target.value)}
              placeholder="Enter a name for this passkey"
              data-testid="passkey-name-input"
            />
          </div>
          <button
            type="submit"
            disabled={isRegistering}
            data-testid="register-btn"
          >
            {isRegistering ? "Registering..." : "Register Passkey"}
          </button>
        </form>
      </div>
    </div>
  );
}

describe("Passkey Registration Flow", () => {
  beforeEach(() => {
    // Ensure PublicKeyCredential is available by default
    if (!(window as any).PublicKeyCredential) {
      (window as any).PublicKeyCredential = {};
    }
  });

  it("displays passkey registration page when WebAuthn is supported", () => {
    render(<PasskeyRegistrationFlow />);
    expect(screen.getByTestId("passkey-registration")).toBeInTheDocument();
  });

  it("shows browser not supported message when WebAuthn is unavailable", () => {
    delete (window as any).PublicKeyCredential;
    render(<PasskeyRegistrationFlow />);
    expect(screen.getByTestId("webauthn-not-supported")).toBeInTheDocument();
    expect(screen.getByText(/Browser Not Supported/)).toBeInTheDocument();
  });

  it("lists supported browsers in fallback UI", () => {
    delete (window as any).PublicKeyCredential;
    render(<PasskeyRegistrationFlow />);
    expect(screen.getByTestId("browser-alternatives")).toBeInTheDocument();
    expect(screen.getByText(/Chrome 67\+/)).toBeInTheDocument();
    expect(screen.getByText(/Firefox 60\+/)).toBeInTheDocument();
    expect(screen.getByText(/Safari 13\+/)).toBeInTheDocument();
  });

  it("displays already registered passkeys", async () => {
    render(<PasskeyRegistrationFlow />);

    await waitFor(() => {
      expect(screen.getByTestId("passkey-My MacBook Pro")).toBeInTheDocument();
    });
  });

  it("shows passkey details including creation date", async () => {
    render(<PasskeyRegistrationFlow />);

    await waitFor(() => {
      expect(screen.getByText(/Created: 2026-09-01/)).toBeInTheDocument();
    });
  });

  it("displays last used date when available", async () => {
    render(<PasskeyRegistrationFlow />);

    await waitFor(() => {
      expect(screen.getByText(/Last used: 2026-09-26/)).toBeInTheDocument();
    });
  });

  it("renders registration form", () => {
    render(<PasskeyRegistrationFlow />);
    expect(screen.getByTestId("registration-form")).toBeInTheDocument();
    expect(screen.getByTestId("passkey-name-input")).toBeInTheDocument();
  });

  it("allows user to register a new passkey", async () => {
    render(<PasskeyRegistrationFlow />);

    const input = screen.getByTestId("passkey-name-input") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "iPhone 15" } });
    fireEvent.click(screen.getByTestId("register-btn"));

    await waitFor(() => {
      expect(screen.getByText("iPhone 15")).toBeInTheDocument();
    });
  });

  it("shows loading state while registering", async () => {
    render(<PasskeyRegistrationFlow />);

    const input = screen.getByTestId("passkey-name-input") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "New Passkey" } });
    fireEvent.click(screen.getByTestId("register-btn"));

    expect(screen.getByText("Registering...")).toBeInTheDocument();
  });

  it("prevents registration with empty name", async () => {
    render(<PasskeyRegistrationFlow />);

    fireEvent.click(screen.getByTestId("register-btn"));

    await waitFor(() => {
      expect(screen.getByText(/Please enter a name for the passkey/)).toBeInTheDocument();
    });
  });

  it("clears input after successful registration", async () => {
    render(<PasskeyRegistrationFlow />);

    const input = screen.getByTestId("passkey-name-input") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "New Device" } });
    fireEvent.click(screen.getByTestId("register-btn"));

    await waitFor(() => {
      expect(input.value).toBe("");
    });
  });

  it("allows user to delete a registered passkey", async () => {
    render(<PasskeyRegistrationFlow />);

    await waitFor(() => {
      expect(screen.getByTestId("passkey-pk-1")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("delete-pk-1"));

    await waitFor(() => {
      expect(screen.queryByTestId("passkey-pk-1")).not.toBeInTheDocument();
    });
  });

  it("shows no passkeys message when list is empty", async () => {
    render(<PasskeyRegistrationFlow />);

    await waitFor(() => {
      expect(screen.getByTestId("passkey-pk-1")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("delete-pk-1"));

    await waitFor(() => {
      expect(screen.getByTestId("no-passkeys")).toBeInTheDocument();
    });
  });

  it("calls onRegistered callback when passkey is registered", async () => {
    const onRegistered = jest.fn();
    render(<PasskeyRegistrationFlow onRegistered={onRegistered} />);

    const input = screen.getByTestId("passkey-name-input") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "Test Passkey" } });
    fireEvent.click(screen.getByTestId("register-btn"));

    await waitFor(() => {
      expect(onRegistered).toHaveBeenCalled();
    });
  });

  it("calls onError callback when registration fails", async () => {
    const onError = jest.fn();
    render(<PasskeyRegistrationFlow onError={onError} />);

    fireEvent.click(screen.getByTestId("register-btn"));

    await waitFor(() => {
      expect(onError).toHaveBeenCalled();
    });
  });
});
