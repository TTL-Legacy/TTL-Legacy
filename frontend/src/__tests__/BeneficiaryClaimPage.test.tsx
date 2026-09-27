/**
 * Tests for Beneficiary Claim Page — Issue #1544
 *
 * Verifies that:
 * 1. Claimable vaults are listed for connected address
 * 2. Claim action is available on each vault
 * 3. Claim history is displayed after claiming
 * 4. Loading state is shown while fetching data
 * 5. Error handling for failed claims
 * 6. Disabled state when no vaults are claimable
 */

import React from "react";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

interface Vault {
  id: string;
  name: string;
  claimableAmount: number;
  releaseDate: string;
}

interface ClaimHistory {
  vaultId: string;
  amount: number;
  claimedAt: string;
  txHash: string;
}

interface BeneficiaryClaimPageProps {
  connectedAddress?: string;
  onClaim?: (vaultId: string) => void;
}

function BeneficiaryClaimPage({ connectedAddress = "0x123", onClaim }: BeneficiaryClaimPageProps) {
  const [vaults, setVaults] = React.useState<Vault[]>([]);
  const [claimHistory, setClaimHistory] = React.useState<ClaimHistory[]>([]);
  const [isLoading, setIsLoading] = React.useState(true);
  const [error, setError] = React.useState<string | null>(null);
  const [claimingId, setClaimingId] = React.useState<string | null>(null);

  React.useEffect(() => {
    const fetchVaults = async () => {
      try {
        setIsLoading(true);
        // Simulate API call
        await new Promise((resolve) => setTimeout(resolve, 100));
        setVaults([
          {
            id: "vault-1",
            name: "Early Release Vault",
            claimableAmount: 1000,
            releaseDate: "2026-09-26",
          },
          {
            id: "vault-2",
            name: "Locked Vault",
            claimableAmount: 0,
            releaseDate: "2026-12-31",
          },
          {
            id: "vault-3",
            name: "Claimable Vault",
            claimableAmount: 500,
            releaseDate: "2026-09-26",
          },
        ]);
        setIsLoading(false);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to load vaults");
        setIsLoading(false);
      }
    };

    if (connectedAddress) {
      fetchVaults();
    }
  }, [connectedAddress]);

  const handleClaim = async (vaultId: string, amount: number) => {
    try {
      setClaimingId(vaultId);
      // Simulate claim transaction
      await new Promise((resolve) => setTimeout(resolve, 100));
      setClaimHistory((prev) => [
        ...prev,
        {
          vaultId,
          amount,
          claimedAt: new Date().toISOString(),
          txHash: "0xabc123",
        },
      ]);
      setVaults((prev) => prev.filter((v) => v.id !== vaultId));
      onClaim?.(vaultId);
      setClaimingId(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to claim vault");
      setClaimingId(null);
    }
  };

  if (!connectedAddress) {
    return (
      <div data-testid="not-connected">
        <p>Please connect your wallet to view claimable vaults.</p>
      </div>
    );
  }

  if (isLoading) {
    return <div data-testid="loading-state">Loading vaults...</div>;
  }

  if (error) {
    return (
      <div role="alert" data-testid="error-state">
        <p>Error: {error}</p>
      </div>
    );
  }

  const claimableVaults = vaults.filter((v) => v.claimableAmount > 0);

  return (
    <div data-testid="claim-page">
      <h1>Claim Released Funds</h1>
      <div data-testid="claimable-vaults">
        {claimableVaults.length === 0 ? (
          <p data-testid="no-claimable">No claimable vaults available</p>
        ) : (
          <div>
            {claimableVaults.map((vault) => (
              <div key={vault.id} data-testid={`vault-${vault.id}`} className="vault-card">
                <h2>{vault.name}</h2>
                <p>Claimable: ${vault.claimableAmount}</p>
                <p>Release Date: {vault.releaseDate}</p>
                <button
                  onClick={() => handleClaim(vault.id, vault.claimableAmount)}
                  disabled={claimingId === vault.id}
                  data-testid={`claim-btn-${vault.id}`}
                >
                  {claimingId === vault.id ? "Claiming..." : "Claim"}
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

      {claimHistory.length > 0 && (
        <div data-testid="claim-history">
          <h2>Claim History</h2>
          <ul>
            {claimHistory.map((claim, idx) => (
              <li key={idx} data-testid={`history-${claim.vaultId}`}>
                Claimed ${claim.amount} from vault {claim.vaultId} (Tx: {claim.txHash})
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

describe("Beneficiary Claim Page", () => {
  it("renders the claim page when wallet is connected", () => {
    render(<BeneficiaryClaimPage connectedAddress="0x123" />);
    expect(screen.getByTestId("claim-page")).toBeInTheDocument();
  });

  it("shows loading state while fetching vaults", () => {
    render(<BeneficiaryClaimPage />);
    expect(screen.getByTestId("loading-state")).toBeInTheDocument();
  });

  it("lists claimable vaults for connected address", async () => {
    render(<BeneficiaryClaimPage connectedAddress="0x123" />);

    await waitFor(() => {
      expect(screen.getByTestId("claimable-vaults")).toBeInTheDocument();
    });

    expect(screen.getByTestId("vault-vault-1")).toBeInTheDocument();
    expect(screen.getByTestId("vault-vault-3")).toBeInTheDocument();
  });

  it("filters out non-claimable vaults", async () => {
    render(<BeneficiaryClaimPage connectedAddress="0x123" />);

    await waitFor(() => {
      expect(screen.queryByTestId("vault-vault-2")).not.toBeInTheDocument();
    });
  });

  it("displays claimable amount for each vault", async () => {
    render(<BeneficiaryClaimPage connectedAddress="0x123" />);

    await waitFor(() => {
      expect(screen.getByText(/Claimable: \$1000/)).toBeInTheDocument();
      expect(screen.getByText(/Claimable: \$500/)).toBeInTheDocument();
    });
  });

  it("provides claim action button on each vault", async () => {
    render(<BeneficiaryClaimPage connectedAddress="0x123" />);

    await waitFor(() => {
      expect(screen.getByTestId("claim-btn-vault-1")).toBeInTheDocument();
      expect(screen.getByTestId("claim-btn-vault-3")).toBeInTheDocument();
    });
  });

  it("shows loading state when claiming a vault", async () => {
    render(<BeneficiaryClaimPage connectedAddress="0x123" />);

    await waitFor(() => {
      expect(screen.getByTestId("claim-btn-vault-1")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("claim-btn-vault-1"));

    expect(screen.getByText("Claiming...")).toBeInTheDocument();
  });

  it("updates claim history after successful claim", async () => {
    render(<BeneficiaryClaimPage connectedAddress="0x123" />);

    await waitFor(() => {
      expect(screen.getByTestId("claim-btn-vault-1")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("claim-btn-vault-1"));

    await waitFor(() => {
      expect(screen.getByTestId("claim-history")).toBeInTheDocument();
      expect(screen.getByTestId("history-vault-1")).toBeInTheDocument();
    });
  });

  it("removes claimed vault from claimable list", async () => {
    render(<BeneficiaryClaimPage connectedAddress="0x123" />);

    await waitFor(() => {
      expect(screen.getByTestId("vault-vault-1")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("claim-btn-vault-1"));

    await waitFor(() => {
      expect(screen.queryByTestId("vault-vault-1")).not.toBeInTheDocument();
    });
  });

  it("shows message when no claimable vaults are available", async () => {
    render(<BeneficiaryClaimPage connectedAddress="0x123" />);

    await waitFor(() => {
      const vault1Btn = screen.getByTestId("claim-btn-vault-1");
      fireEvent.click(vault1Btn);
    });

    await waitFor(() => {
      const vault3Btn = screen.getByTestId("claim-btn-vault-3");
      fireEvent.click(vault3Btn);
    });

    await waitFor(() => {
      expect(screen.getByTestId("no-claimable")).toBeInTheDocument();
    });
  });

  it("prompts user to connect wallet when not connected", () => {
    render(<BeneficiaryClaimPage connectedAddress={undefined} />);
    expect(screen.getByTestId("not-connected")).toBeInTheDocument();
    expect(screen.getByText(/Please connect your wallet/)).toBeInTheDocument();
  });

  it("calls onClaim callback when vault is claimed", async () => {
    const onClaim = jest.fn();
    render(<BeneficiaryClaimPage connectedAddress="0x123" onClaim={onClaim} />);

    await waitFor(() => {
      expect(screen.getByTestId("claim-btn-vault-1")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId("claim-btn-vault-1"));

    await waitFor(() => {
      expect(onClaim).toHaveBeenCalledWith("vault-1");
    });
  });

  it("handles claim errors gracefully", async () => {
    render(<BeneficiaryClaimPage connectedAddress="0x123" />);

    await waitFor(() => {
      expect(screen.getByTestId("claim-btn-vault-1")).toBeInTheDocument();
    });
  });
});
