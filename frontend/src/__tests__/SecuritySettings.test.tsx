/**
 * Tests for SecuritySettings — Issue #2147
 *
 * Verifies that:
 * 1. The friendly name is rendered when set.
 * 2. The AAGUID-derived device name is used as fallback when the friendly
 *    name is null or empty.
 * 3. Editing and saving a new friendly name calls
 *    `PUT /auth/passkeys/{id}/name` and updates the displayed name.
 * 4. Errors while saving are surfaced without clearing the name.
 */

import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import type { Passkey } from "../api/generated";
import { apiClient } from "../api/client";
import { SecuritySettings } from "../components/SecuritySettings";

jest.mock("../api/client", () => ({
  apiClient: {
    listPasskeys: jest.fn(),
    updatePasskeyFriendlyName: jest.fn(),
  },
}));

const mockListPasskeys = jest.mocked(apiClient.listPasskeys);
const mockUpdatePasskeyFriendlyName = jest.mocked(apiClient.updatePasskeyFriendlyName);

function makePasskey(overrides: Partial<Passkey> = {}): Passkey {
  return {
    passkey_id: "pk-1",
    owner: "GTEST123456789",
    vault_id: "vault-1",
    credential_id: "cred-1",
    device_name: "YubiKey 5",
    friendly_name: null,
    registered_at: "2026-01-01T00:00:00Z",
    last_used: null,
    is_backup: false,
    ...overrides,
  };
}

describe("SecuritySettings", () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it("loads passkeys via the API when none are provided", async () => {
    mockListPasskeys.mockResolvedValue([
      makePasskey({ passkey_id: "pk-9", friendly_name: "Loaded Name" }),
    ]);
    render(<SecuritySettings vaultId="vault-9" owner="GTEST123456789" />);

    await waitFor(() => {
      expect(mockListPasskeys).toHaveBeenCalledWith("vault-9", "GTEST123456789");
    });
    await waitFor(() => {
      expect(screen.getByTestId("passkey-name-pk-9")).toHaveTextContent("Loaded Name");
    });
    expect(screen.queryByTestId("security-passkeys-loading")).not.toBeInTheDocument();
  });

  it("renders the friendly name when it is set", () => {
    render(
      <SecuritySettings passkeys={[makePasskey({ passkey_id: "pk-1", friendly_name: "My Key" })]} />,
    );

    expect(screen.getByTestId("passkey-name-pk-1")).toHaveTextContent("My Key");
    // The AAGUID-derived device name is shown as secondary metadata.
    expect(screen.getByTestId("passkey-device-pk-1")).toHaveTextContent("YubiKey 5");
  });

  it("falls back to the AAGUID-derived device name when friendly_name is null", () => {
    render(
      <SecuritySettings
        passkeys={[
          makePasskey({ passkey_id: "pk-2", friendly_name: null, device_name: "MacBook Pro" }),
        ]}
      />,
    );

    expect(screen.getByTestId("passkey-name-pk-2")).toHaveTextContent("MacBook Pro");
    expect(screen.queryByTestId("passkey-device-pk-2")).not.toBeInTheDocument();
  });

  it("falls back to the device name when friendly_name is empty", () => {
    render(
      <SecuritySettings
        passkeys={[makePasskey({ passkey_id: "pk-3", friendly_name: "", device_name: "iPhone 15" })]}
      />,
    );

    expect(screen.getByTestId("passkey-name-pk-3")).toHaveTextContent("iPhone 15");
  });

  it("canceling the edit keeps the original name", () => {
    render(
      <SecuritySettings
        passkeys={[makePasskey({ passkey_id: "pk-1", friendly_name: "My Key" })]}
      />,
    );

    fireEvent.click(screen.getByTestId("edit-name-pk-1"));
    expect(screen.getByTestId("edit-form-pk-1")).toBeInTheDocument();
    fireEvent.change(screen.getByTestId("friendly-name-input-pk-1"), {
      target: { value: "Temp Name" },
    });
    fireEvent.click(screen.getByTestId("cancel-edit-pk-1"));

    expect(screen.queryByTestId("edit-form-pk-1")).not.toBeInTheDocument();
    expect(screen.getByTestId("passkey-name-pk-1")).toHaveTextContent("My Key");
    expect(mockUpdatePasskeyFriendlyName).not.toHaveBeenCalled();
  });

  it("editing and saving a new friendly name calls PUT /auth/passkeys/{id}/name and updates the UI", async () => {
    const updated = makePasskey({ passkey_id: "pk-1", friendly_name: "New Name" });
    mockUpdatePasskeyFriendlyName.mockResolvedValue(updated);
    render(<SecuritySettings passkeys={[makePasskey({ passkey_id: "pk-1" })]} />);

    fireEvent.click(screen.getByTestId("edit-name-pk-1"));
    fireEvent.change(screen.getByTestId("friendly-name-input-pk-1"), {
      target: { value: "  New Name  " },
    });
    fireEvent.click(screen.getByTestId("save-name-pk-1"));

    await waitFor(() => {
      expect(mockUpdatePasskeyFriendlyName).toHaveBeenCalledWith("pk-1", {
        friendly_name: "New Name",
      });
    });
    await waitFor(() => {
      expect(screen.getByTestId("passkey-name-pk-1")).toHaveTextContent("New Name");
    });
    expect(screen.queryByTestId("edit-form-pk-1")).not.toBeInTheDocument();
    expect(mockUpdatePasskeyFriendlyName).toHaveBeenCalledTimes(1);
  });

  it("shows an error when saving the new name fails", async () => {
    mockUpdatePasskeyFriendlyName.mockRejectedValue(new Error("rename failed"));
    render(<SecuritySettings passkeys={[makePasskey({ passkey_id: "pk-1" })]} />);

    fireEvent.click(screen.getByTestId("edit-name-pk-1"));
    fireEvent.click(screen.getByTestId("save-name-pk-1"));

    await waitFor(() => {
      expect(screen.getByTestId("security-passkey-error")).toHaveTextContent("rename failed");
    });
  });
});
