//! Tests for Issue #1328: vault counter storage key must not collide with vault
//! data keys or any other persistent-storage entries.
//!
//! Soroban serialises `#[contracttype]` enum variants as XDR `ScVal`s whose
//! discriminant is derived from the variant **name**, not its ordinal position.
//! Two variants are therefore guaranteed to produce different storage keys as
//! long as they have different names (regardless of whether their payloads
//! overlap).  These tests confirm that guarantee holds for every key in the
//! `StorageKey` enum.

#![cfg(test)]

use super::*;
use soroban_sdk::{Address, BytesN, Env};

/// Verify that `StorageKey::VaultCount` (the global vault counter) and
/// `StorageKey::Vault(0)` (vault data for vault ID 0) produce distinct storage
/// keys.  This is the primary collision guard for Issue #1328.
#[test]
fn test_vault_counter_does_not_collide_with_vault_data() {
    let env = Env::default();

    // Write distinguishable sentinels to each key
    env.storage()
        .persistent()
        .set(&StorageKey::VaultCount, &42u64);
    env.storage()
        .persistent()
        .set(&StorageKey::Vault(0), &99u64);

    // Read them back — each must return its own sentinel
    let count: u64 = env
        .storage()
        .persistent()
        .get(&StorageKey::VaultCount)
        .expect("VaultCount should be readable");
    let vault_zero: u64 = env
        .storage()
        .persistent()
        .get(&StorageKey::Vault(0))
        .expect("Vault(0) should be readable");

    assert_eq!(
        count, 42,
        "VaultCount was overwritten by Vault(0) — storage key collision!"
    );
    assert_eq!(
        vault_zero, 99,
        "Vault(0) was overwritten by VaultCount — storage key collision!"
    );
}

/// Verify that `StorageKey::VaultCount` does not collide with any non-zero
/// vault-data entry either.
#[test]
fn test_vault_counter_does_not_collide_with_non_zero_vault() {
    let env = Env::default();

    env.storage()
        .persistent()
        .set(&StorageKey::VaultCount, &1u64);
    env.storage()
        .persistent()
        .set(&StorageKey::Vault(1), &100u64);

    let count: u64 = env
        .storage()
        .persistent()
        .get(&StorageKey::VaultCount)
        .unwrap();
    let vault_one: u64 = env
        .storage()
        .persistent()
        .get(&StorageKey::Vault(1))
        .unwrap();

    assert_eq!(count, 1, "VaultCount collides with Vault(1)");
    assert_eq!(vault_one, 100, "Vault(1) collides with VaultCount");
}

/// Verify that vault-scoped keys with the same vault_id do not collide with
/// each other.  Each key variant must produce an independent slot.
#[test]
fn test_vault_scoped_keys_do_not_collide() {
    let env = Env::default();
    let vault_id = 0u64;

    // A representative set of vault-scoped StorageKey variants
    let keys: &[(u32, StorageKey)] = &[
        (0, StorageKey::Vault(vault_id)),
        (1, StorageKey::VaultAuditLog(vault_id)),
        (2, StorageKey::VaultMetadata(vault_id)),
        (3, StorageKey::VaultPasskeys(vault_id)),
        (4, StorageKey::VaultFrozen(vault_id)),
        (5, StorageKey::VaultLocked(vault_id)),
        (6, StorageKey::VaultLowTtlThreshold(vault_id)),
        (7, StorageKey::VaultSnapshot(vault_id, 0)),
        (8, StorageKey::VaultSnapshotTimestamps(vault_id)),
        (9, StorageKey::Hibernation(vault_id)),
        (10, StorageKey::LastCheckInTime(vault_id)),
        (11, StorageKey::ReleaseAttempted(vault_id)),
        (12, StorageKey::ReleaseConditions(vault_id)),
        (13, StorageKey::ReleaseMemo(vault_id)),
        (14, StorageKey::WithdrawalAuditLog(vault_id)),
        (15, StorageKey::WithdrawalLimit(vault_id)),
        (16, StorageKey::WithdrawalTracker(vault_id)),
        (17, StorageKey::WithdrawalWhitelist(vault_id)),
        (18, StorageKey::BackupCodes(vault_id)),
        (19, StorageKey::CheckInStreak(vault_id)),
        (20, StorageKey::ProofOfLife(vault_id)),
        (21, StorageKey::ReleaseVotes(vault_id)),
        (22, StorageKey::ReleaseVoteThreshold(vault_id)),
    ];

    for (sentinel, key) in keys {
        env.storage().persistent().set(key, sentinel);
    }

    for (sentinel, key) in keys {
        let val: u32 = env
            .storage()
            .persistent()
            .get(key)
            .unwrap_or(9999);
        assert_eq!(
            val, *sentinel,
            "StorageKey collision detected for vault-scoped key (expected sentinel {sentinel})"
        );
    }
}

/// Exhaustive check: write every `StorageKey` variant (with the same sentinel
/// payload 0 / zero-address where applicable) and assert that the round-trip
/// read returns the *exact index* that was written.  Any collision would cause
/// a later write to overwrite an earlier one, producing the wrong index on
/// read-back.
#[test]
fn test_storage_key_no_collisions() {
    let env = Env::default();

    let keys: soroban_sdk::Vec<StorageKey> = soroban_sdk::vec![
        &env,
        StorageKey::Vault(0),
        StorageKey::OwnerVaults(Address::generate(&env)),
        StorageKey::MaxVaultsPerOwner,
        StorageKey::BeneficiaryVaults(Address::generate(&env)),
        StorageKey::VaultCount,
        StorageKey::TokenAddress,
        StorageKey::Admin,
        StorageKey::Paused,
        StorageKey::PendingAdmin,
        StorageKey::MinCheckInInterval,
        StorageKey::MaxCheckInInterval,
        StorageKey::Version,
        StorageKey::VestingSchedule(0, 0),
        StorageKey::VestingPenalty(0, 0),
        StorageKey::VestingPendingClaim(0, 0),
        StorageKey::VestingScheduleCount(0),
        StorageKey::MilestoneVestingSchedule(0),
        StorageKey::CountdownFired(0),
        StorageKey::TokenWhitelist(Address::generate(&env)),
        StorageKey::WrappedToken(Address::generate(&env)),
        StorageKey::VaultMetadata(0),
        StorageKey::ParentVault(0),
        StorageKey::VaultPasskeys(0),
        StorageKey::BackupCodes(0),
        StorageKey::BeneficiaryDelegate(0),
        StorageKey::BeneficiaryDelegationChain(0),
        StorageKey::WithdrawalSchedule(0),
        StorageKey::DisputeStatus(0),
        StorageKey::ConditionalAcceptance(0),
        StorageKey::ConditionalDecline(0),
        StorageKey::ArchivedVault(0),
        StorageKey::MaxTtlSeconds,
        StorageKey::TtlDecayRate,
        StorageKey::ReleaseGracePeriodSeconds,
        StorageKey::BridgeConfig(0),
        StorageKey::TokenConversion(0),
        StorageKey::TokenStaking(0),
        StorageKey::PasskeyUsage(0),
        StorageKey::BeneficiaryStatus(0),
        StorageKey::PasskeyExpiry(0, BytesN::from_array(&env, &[0u8; 32])),
        StorageKey::PendingOwnership(0),
        StorageKey::PendingBeneficiaryUpdate(0),
        StorageKey::VaultAuditLog(0),
        StorageKey::MultiSigConfig(0),
        StorageKey::MultiSigProposal(0, 0),
        StorageKey::MultiSigProposalCount(0),
        StorageKey::MetadataHistory(0),
        StorageKey::CustomMetadataHistory(0),
        StorageKey::OwnerVaultCount(Address::generate(&env)),
        StorageKey::StateTransitionLog(0),
        StorageKey::PasskeyChallenge(0, BytesN::from_array(&env, &[0u8; 32])),
        StorageKey::WithdrawalApprovals(0),
        StorageKey::VaultSnapshot(0, 0),
        StorageKey::VaultSnapshotTimestamps(0),
        StorageKey::CheckInHistory(0),
        StorageKey::CheckInEntry(0, 0),
        StorageKey::CheckInHistoryHead(0),
        StorageKey::CheckInHistoryLen(0),
        StorageKey::ReleaseMemo(0),
        StorageKey::AdaptiveIntervalSuggestion(0),
        StorageKey::CheckInStreak(0),
        StorageKey::CheckInNonce(0),
        StorageKey::CheckInDelegates(0),
        StorageKey::DelegateNonce(0, Address::generate(&env)),
        StorageKey::CheckInDelegateExpiry(0, Address::generate(&env)),
        StorageKey::ProofOfLife(0),
        StorageKey::ReleaseVotes(0),
        StorageKey::ReleaseVoteThreshold(0),
        StorageKey::BeneficiaryIdentityOracle(0),
        StorageKey::BeneficiaryIdentityVerification(0),
        StorageKey::BeneficiaryReleaseTriggers(0),
        StorageKey::BeneficiaryTierThreshold(0, Address::generate(&env)),
        StorageKey::BeneficiaryStatusEntry(0, Address::generate(&env)),
        StorageKey::BeneficiaryReleaseConditionVeto(0),
        StorageKey::ReleaseConditions(0),
        StorageKey::ReleaseAttempted(0),
        StorageKey::Hibernation(0),
        StorageKey::LastCheckInTime(0),
        StorageKey::MinCheckInCooldown,
        StorageKey::VaultDuplicate(Address::generate(&env), Address::generate(&env), 0),
        StorageKey::BeneficiaryRotationSchedule(0),
        StorageKey::CheckInGeoLog(0),
        StorageKey::TtlBorrow(0),
        StorageKey::EncryptedBackupCodes(0),
        StorageKey::WithdrawalAuditLog(0),
        StorageKey::WithdrawalDisputes(0),
        StorageKey::WithdrawalScheduleValidation(0),
        StorageKey::WithdrawalLimit(0),
        StorageKey::WithdrawalTracker(0),
        StorageKey::WithdrawalWhitelist(0),
        StorageKey::WithdrawalReversal(0, 0),
        StorageKey::WithdrawalReversalCounter(0),
        StorageKey::VestingCatchUp(0),
        StorageKey::VestingBonus(0),
        StorageKey::YieldDistributionConfig(0),
        StorageKey::TokenLending(0),
        StorageKey::TokenCollateral(0),
        StorageKey::TokenHedge(0),
        StorageKey::TokenRebalance(0),
        StorageKey::BeneficiaryPool(0),
        StorageKey::BeneficiaryPoolAlloc(0),
        StorageKey::BeneficiaryCommitment(0),
        StorageKey::RevealedBeneficiary(0),
        StorageKey::BeneficiaryVestingSchedule(0, Address::generate(&env)),
        StorageKey::BeneficiaryVestingCount(0),
        StorageKey::BeneficiaryAuction(0),
        StorageKey::BeneficiaryAuctionBid(0, Address::generate(&env)),
        StorageKey::BeneficiaryAuctionCount,
        StorageKey::PendingProtocolConfig,
        StorageKey::ProtocolConfigProposedAt,
        StorageKey::RequireUtf8Metadata,
        StorageKey::OpenProposals(0),
        StorageKey::TwoFactorConfig(0),
        StorageKey::TwoFactorVerified(0),
        StorageKey::VaultFrozen(0),
        StorageKey::PendingMultiSigOp(0, 0),
        StorageKey::PendingMultiSigOpNonce(0),
        StorageKey::PendingUpgrade,
        StorageKey::AllowedTokens,
        StorageKey::VaultLocked(0),
        StorageKey::VaultLowTtlThreshold(0),
    ];

    for (i, key) in keys.iter().enumerate() {
        env.storage().persistent().set(key, &(i as u32));
    }

    for (i, key) in keys.iter().enumerate() {
        let val: u32 = env.storage().persistent().get(key).unwrap_or(9999);
        assert_eq!(val, i as u32, "StorageKey collision detected at index {i}");
    }
}
