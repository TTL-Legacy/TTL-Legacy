use soroban_sdk::{contracttype, symbol_short, Address, Bytes, BytesN, String, Symbol, Vec};

/// Maximum number of vesting schedules per vault.
pub const MAX_VESTING_SCHEDULES: u32 = 20;

/// Maximum number of beneficiaries allowed per vault.
pub const MAX_BENEFICIARIES: u32 = 20;

pub const RELEASE_TOPIC: Symbol = symbol_short!("release");
pub const VAULT_CREATED_TOPIC: Symbol = symbol_short!("v_created");
pub const PING_EXPIRY_TOPIC: Symbol = symbol_short!("ping_exp");
/// Emitted during check_in when the new TTL (= check_in_interval) is below EXPIRY_WARNING_THRESHOLD.
pub const TTL_WARNING_TOPIC: Symbol = symbol_short!("ttl_warn");
pub const DEPOSIT_TOPIC: Symbol = symbol_short!("deposit");
pub const WITHDRAW_TOPIC: Symbol = symbol_short!("withdraw");
pub const CHECK_IN_TOPIC: Symbol = symbol_short!("check_in");
pub const CANCEL_TOPIC: Symbol = symbol_short!("cancel");
pub const OWNERSHIP_TOPIC: Symbol = symbol_short!("own_xfer");
pub const OWNERSHIP_INITIATED_TOPIC: Symbol = symbol_short!("own_init");
pub const OWNERSHIP_ACCEPTED_TOPIC: Symbol = symbol_short!("own_acc");
pub const OWNERSHIP_CANCELLED_TOPIC: Symbol = symbol_short!("own_can");
pub const OWNERSHIP_TRANSFER_EXPIRED_TOPIC: Symbol = symbol_short!("own_exp");
pub const BENEFICIARY_UPDATED_TOPIC: Symbol = symbol_short!("ben_upd");
pub const SET_BENEFICIARIES_TOPIC: Symbol = symbol_short!("set_bens");
pub const UPDATE_INTERVAL_TOPIC: Symbol = symbol_short!("upd_intv");
pub const UPDATE_METADATA_TOPIC: Symbol = symbol_short!("upd_meta");
pub const SET_MIN_INTERVAL_TOPIC: Symbol = symbol_short!("set_min");
pub const SET_MAX_INTERVAL_TOPIC: Symbol = symbol_short!("set_max");
/// Emitted when adaptive interval is enabled/disabled or a new suggestion is computed - Issue #2
pub const ADAPTIVE_INTERVAL_TOPIC: Symbol = symbol_short!("adap_int");
pub const PAUSE_TOPIC: Symbol = symbol_short!("pause");
pub const UNPAUSE_TOPIC: Symbol = symbol_short!("unpause");
pub const SET_VESTING_TOPIC: Symbol = symbol_short!("set_vest");
pub const CLAIM_VEST_TOPIC: Symbol = symbol_short!("clm_vest");
pub const VESTING_CANCELLED_TOPIC: Symbol = symbol_short!("vest_can");
pub const PASSKEY_ANALYTICS_TOPIC: Symbol = symbol_short!("pk_ana");
pub const BACKUP_CODES_ENCRYPTED_TOPIC: Symbol = symbol_short!("bkp_enc");
// Issue #534: vesting cliff period reached
pub const CLIFF_REACHED_TOPIC: Symbol = symbol_short!("clif_rch");
pub const PAUSE_VAULT_TOPIC: Symbol = symbol_short!("v_pause");
pub const RESUME_VAULT_TOPIC: Symbol = symbol_short!("v_resume");
pub const SET_METADATA_TOPIC: Symbol = symbol_short!("set_meta");
pub const INHERITANCE_TOPIC: Symbol = symbol_short!("inherit");
pub const ADD_PASSKEY_TOPIC: Symbol = symbol_short!("add_pk");
pub const REMOVE_PASSKEY_TOPIC: Symbol = symbol_short!("rm_pk");
pub const ROTATE_PASSKEY_TOPIC: Symbol = symbol_short!("rot_pk");
pub const BACKUP_CODE_USED_TOPIC: Symbol = symbol_short!("bk_used");
pub const BACKUP_CODES_GENERATED_TOPIC: Symbol = symbol_short!("bk_gen");
pub const DELEGATE_BENEFICIARY_TOPIC: Symbol = symbol_short!("del_ben");
pub const DISPUTE_FILED_TOPIC: Symbol = symbol_short!("disp_fil");
pub const DISPUTE_RESOLVED_TOPIC: Symbol = symbol_short!("disp_res");
pub const WITHDRAWAL_SCHEDULED_TOPIC: Symbol = symbol_short!("wd_sch");
pub const WITHDRAWAL_EXECUTED_TOPIC: Symbol = symbol_short!("wd_exec");
pub const CONDITIONS_ACCEPTED_TOPIC: Symbol = symbol_short!("cond_acc");
pub const SET_SPENDING_LIMIT_TOPIC: Symbol = symbol_short!("set_slmt");
pub const SET_MAX_TTL_TOPIC: Symbol = symbol_short!("set_ttl");
pub const SET_DECAY_RATE_TOPIC: Symbol = symbol_short!("set_dec");
pub const ACCEPTANCE_DEADLINE_EXPIRED_TOPIC: Symbol = symbol_short!("acc_exp");
pub const TTL_DECAY_TOPIC: Symbol = symbol_short!("ttl_dec");
pub const SET_BURN_PERCENTAGE_TOPIC: Symbol = symbol_short!("set_burn");
pub const BURN_EVENT_TOPIC: Symbol = symbol_short!("burn_evt");
pub const SYNC_TTL_TOPIC: Symbol = symbol_short!("sync_ttl");
pub const PASSKEY_EXPIRY_EXTENDED_TOPIC: Symbol = symbol_short!("pk_exp");
pub const BENEFICIARY_ACCEPTED_TOPIC: Symbol = symbol_short!("ben_acc");
pub const BENEFICIARY_DECLINED_TOPIC: Symbol = symbol_short!("ben_dec");
pub const BENEFICIARY_CONDITION_ACCEPTED_TOPIC: Symbol = symbol_short!("ben_cond");
pub const BENEFICIARY_IDENTITY_ORACLE_SET_TOPIC: Symbol = symbol_short!("ben_id_or");
pub const BENEFICIARY_IDENTITY_VERIFIED_TOPIC: Symbol = symbol_short!("ben_id_vf");
pub const BENEFICIARY_CONFLICT_FILED_TOPIC: Symbol = symbol_short!("ben_conf");
pub const BENEFICIARY_CONFLICT_RESOLVED_TOPIC: Symbol = symbol_short!("ben_res");
pub const CONFLICT_EXPIRED_TOPIC: Symbol = symbol_short!("conf_exp");
pub const SET_RECOVERY_TOPIC: Symbol = symbol_short!("set_rec");
pub const RECOVERY_EXTEND_TOPIC: Symbol = symbol_short!("rec_ext");
// Issue #934: emergency vault recovery code
pub const EMERGENCY_RECOVERY_GENERATED_TOPIC: Symbol = symbol_short!("erc_gen");
pub const EMERGENCY_RECOVERY_USED_TOPIC: Symbol = symbol_short!("erc_used");
pub const RESTORE_VAULT_TOPIC: Symbol = symbol_short!("restore");
// Issue #944: beneficiary delegation to proxy
pub const BEN_CLAIM_DELEG_TOPIC: Symbol = symbol_short!("cl_deleg");
pub const PROXY_CLAIM_TOPIC: Symbol = symbol_short!("prxy_clm");
// Issue #945: beneficiary conditional threshold escalation
pub const ACCEPTANCE_CONDITIONS_SET_TOPIC: Symbol = symbol_short!("acc_cnds");
pub const PASSKEY_USAGE_TOPIC: Symbol = symbol_short!("pk_usage");
// Biometric binding events
pub const BIND_PASSKEY_BIOMETRIC_TOPIC: Symbol = symbol_short!("bind_pk");
pub const UNBIND_PASSKEY_BIOMETRIC_TOPIC: Symbol = symbol_short!("ubind_pk");
pub const BIO_CHECKIN_TOPIC: Symbol = symbol_short!("bio_ci");
pub const VAULT_CLONED_TOPIC: Symbol = symbol_short!("v_clone");
pub const VAULT_CLONED_OVERRIDE_TOPIC: Symbol = symbol_short!("v_clo_ov");
pub const VAULT_MERGED_TOPIC: Symbol = symbol_short!("v_merge");
pub const MULTISIG_CONFIG_TOPIC: Symbol = symbol_short!("ms_cfg");
pub const MULTISIG_PROPOSED_TOPIC: Symbol = symbol_short!("ms_prop");
pub const MULTISIG_APPROVED_TOPIC: Symbol = symbol_short!("ms_app");
pub const MULTISIG_REJECTED_TOPIC: Symbol = symbol_short!("ms_rej");
pub const MULTISIG_EXECUTED_TOPIC: Symbol = symbol_short!("ms_exec");
pub const MULTISIG_VETOED_TOPIC: Symbol = symbol_short!("ms_veto");
pub const MULTISIG_SIGNER_REMOVED_TOPIC: Symbol = symbol_short!("ms_rm_sig");
pub const MULTISIG_PROPOSAL_EXPIRY: u64 = 604_800; // 7 days

pub const META_VERSION_TOPIC: Symbol = symbol_short!("meta_ver");
pub const META_REVERT_TOPIC: Symbol = symbol_short!("meta_rev");
pub const VAULT_ARCHIVED_TOPIC: Symbol = symbol_short!("v_arch");
pub const VAULT_CAP_TOPIC: Symbol = symbol_short!("v_cap");
// Issue #480: check-in delegation events
pub const DELEGATE_CHECKIN_TOPIC: Symbol = symbol_short!("del_ci");
pub const REVOKE_DELEGATE_TOPIC: Symbol = symbol_short!("rev_del");
/// Emitted when check-in score is updated - Issue #947
pub const CHECKIN_SCORE_UPDATED_TOPIC: Symbol = symbol_short!("ci_score");
// Issue #481: proof-of-work event
pub const CHECKIN_POW_TOPIC: Symbol = symbol_short!("ci_pow");
// Issue #482: TTL prediction event
pub const TTL_PREDICTED_TOPIC: Symbol = symbol_short!("ttl_pred");
// Issue #483: batch check-in event
pub const BATCH_CHECKIN_TOPIC: Symbol = symbol_short!("b_ci");
// Issue #472: state transition audit
pub const STATE_TRANSITION_TOPIC: Symbol = symbol_short!("st_trans");
// Issue #473: ownership proof
pub const OWNERSHIP_PROOF_TOPIC: Symbol = symbol_short!("own_prf");
// Issue #474: integrity check
pub const INTEGRITY_TOPIC: Symbol = symbol_short!("integ");
// Issue #475: batch status query
pub const BATCH_STATUS_TOPIC: Symbol = symbol_short!("b_stat");
// Issue #498: beneficiary proof of life
pub const PROOF_OF_LIFE_TOPIC: Symbol = symbol_short!("pol_sub");
// Issue #499: beneficiary voting
pub const RELEASE_VOTE_TOPIC: Symbol = symbol_short!("rel_vote");
pub const RELEASE_VOTE_PASSED_TOPIC: Symbol = symbol_short!("vote_ok");
// Hibernation events
pub const HIBERNATION_ENTERED_TOPIC: Symbol = symbol_short!("hib_ent");
pub const HIBERNATION_EXITED_TOPIC: Symbol = symbol_short!("hib_ext");

pub const DUPLICATE_VAULT_TOPIC: Symbol = symbol_short!("dup_vault");
pub const MIN_THRESHOLD_SET_TOPIC: Symbol = symbol_short!("min_thr");
pub const MIN_THRESHOLD_SKIP_TOPIC: Symbol = symbol_short!("min_skip");
pub const MIN_THRESHOLD_REDISTRIBUTE_TOPIC: Symbol = symbol_short!("min_rdst");
// Issue #565: withdrawal scheduling validation
pub const WITHDRAWAL_VALIDATION_TOPIC: Symbol = symbol_short!("wd_val");
// Issue #566: withdrawal limits by time
pub const WITHDRAWAL_LIMIT_SET_TOPIC: Symbol = symbol_short!("wd_lim");
pub const WITHDRAWAL_LIMIT_EXCEEDED_TOPIC: Symbol = symbol_short!("wd_exc");
// Issue #567: withdrawal destination whitelist
pub const WHITELIST_ADDED_TOPIC: Symbol = symbol_short!("wl_add");
pub const WHITELIST_REMOVED_TOPIC: Symbol = symbol_short!("wl_rem");
pub const WHITELIST_VIOLATION_TOPIC: Symbol = symbol_short!("wl_vio");
pub const TOKEN_WHITELIST_VALIDATED_TOPIC: Symbol = symbol_short!("tok_wl");
pub const TOKEN_CONVERSION_TOPIC: Symbol = symbol_short!("tok_conv");
pub const TOKEN_STAKING_TOPIC: Symbol = symbol_short!("tok_stk");
pub const TOKEN_UNSTAKING_TOPIC: Symbol = symbol_short!("tok_ust");
pub const YIELD_DISTRIBUTED_TOPIC: Symbol = symbol_short!("yld_dst");
pub const YIELD_REINVESTED_TOPIC: Symbol = symbol_short!("yld_rin");
// Wrapped token registration for cross-chain compatibility
pub const WRAPPED_TOKEN_REGISTERED_TOPIC: Symbol = symbol_short!("wrp_reg");
pub const WRAPPED_TOKEN_UNREGISTERED_TOPIC: Symbol = symbol_short!("wrp_unr");
// Issue #568: withdrawal reversal
pub const WITHDRAWAL_REVERSED_TOPIC: Symbol = symbol_short!("wd_rev");
// Issue #1134: withdrawal cancellation
pub const WITHDRAWAL_CANCELLED_TOPIC: Symbol = symbol_short!("wd_cancel");
pub const REVERSAL_GRACE_EXPIRED_TOPIC: Symbol = symbol_short!("rev_exp");
// Issue #547: vesting penalty applied
pub const VESTING_PENALTY_TOPIC: Symbol = symbol_short!("vest_pen");
// Issue #548: vesting claim reversed / finalized
pub const VESTING_REVERSED_TOPIC: Symbol = symbol_short!("vest_rev");
pub const VESTING_FINALIZED_TOPIC: Symbol = symbol_short!("vest_fin");
// Milestone vesting topic symbols
pub const MILESTONE_VEST_TOPIC: Symbol = symbol_short!("ms_vest");
pub const MILESTONE_PAUSE_TOPIC: Symbol = symbol_short!("ms_paus");
pub const MILESTONE_RESUME_TOPIC: Symbol = symbol_short!("ms_resm");
pub const MILESTONE_ADJUST_TOPIC: Symbol = symbol_short!("ms_adj");
pub const MILESTONE_EMERGENCY_TOPIC: Symbol = symbol_short!("ms_emer");
pub const MILESTONE_PROGRESS_TOPIC: Symbol = symbol_short!("ms_prog");
pub const MILESTONE_CLAIM_TOPIC: Symbol = symbol_short!("ms_clam");
// Multi-vesting-schedule topic
pub const VESTING_SCHEDULE_ADDED_TOPIC: Symbol = symbol_short!("vs_add");
pub const VESTING_SCHEDULE_REMOVED_TOPIC: Symbol = symbol_short!("vs_rem");
// Clawback of unvested funds
pub const CLAWBACK_UNVESTED_TOPIC: Symbol = symbol_short!("clawback");
// Issue #549: passkey expired during check-in
pub const PASSKEY_EXPIRED_TOPIC: Symbol = symbol_short!("pk_expd");
// Issue #550: passkey compromise detected or reported
pub const PASSKEY_COMPROMISED_TOPIC: Symbol = symbol_short!("pk_comp");
// Issue #564: withdrawal approval workflow
pub const WITHDRAWAL_APPROVAL_REQUESTED_TOPIC: Symbol = symbol_short!("wd_req");
pub const WITHDRAWAL_APPROVAL_GRANTED_TOPIC: Symbol = symbol_short!("wd_grant");
pub const WITHDRAWAL_APPROVAL_DENIED_TOPIC: Symbol = symbol_short!("wd_deny");
// Issue #563: passkey recovery
pub const PASSKEY_RECOVERY_INITIATED_TOPIC: Symbol = symbol_short!("pk_rec");
pub const PASSKEY_RECOVERED_TOPIC: Symbol = symbol_short!("pk_rcvd");
// Issue #562: passkey compromise response
pub const PASSKEY_LOCKOUT_TOPIC: Symbol = symbol_short!("pk_lock");
pub const PASSKEY_UNLOCKED_TOPIC: Symbol = symbol_short!("pk_unlk");
// Issue #561: passkey rotation enforcement
pub const PASSKEY_ROTATION_REQUIRED_TOPIC: Symbol = symbol_short!("pk_rot_r");
pub const PASSKEY_ROTATION_ENFORCED_TOPIC: Symbol = symbol_short!("pk_rot_e");

// Issue: TTL Borrowing
pub const TTL_BORROW_TOPIC: Symbol = symbol_short!("ttl_bor");
pub const TTL_REPAY_TOPIC: Symbol = symbol_short!("ttl_rep");

// Issue #541: Vesting Rollover
pub const VESTING_ROLLOVER_TOPIC: Symbol = symbol_short!("vest_rol");
// Issue #542: Vesting Forfeiture
pub const VESTING_FORFEITURE_TOPIC: Symbol = symbol_short!("vest_frf");
// Issue #543: Vesting Acceleration on Death
pub const VESTING_ACCELERATED_TOPIC: Symbol = symbol_short!("vest_acc");
// Issue #544: Vesting Staggering
pub const VESTING_STAGGER_TOPIC: Symbol = symbol_short!("vest_stg");

// Issue #545: Vesting Catch-Up
pub const VESTING_CATCHUP_SET_TOPIC: Symbol = symbol_short!("vest_cu");
pub const VESTING_CATCHUP_CLAIMED_TOPIC: Symbol = symbol_short!("vest_cuc");

// Issue #546: Vesting Bonus
pub const VESTING_BONUS_SET_TOPIC: Symbol = symbol_short!("vest_bon");
pub const VESTING_BONUS_CLAIMED_TOPIC: Symbol = symbol_short!("vest_bonc");

// Issue #585: Token Lending
pub const TOKEN_LENDING_TOPIC: Symbol = symbol_short!("tok_lend");
pub const TOKEN_LEND_REPAY_TOPIC: Symbol = symbol_short!("tok_lrep");
// Issue #586: Token Collateral
pub const TOKEN_COLLATERAL_TOPIC: Symbol = symbol_short!("tok_coll");
pub const TOKEN_COLLAT_RLSD_TOPIC: Symbol = symbol_short!("tok_crls");
// Issue #587: Token Hedging
pub const TOKEN_HEDGE_TOPIC: Symbol = symbol_short!("tok_hedg");
pub const TOKEN_HEDGE_CLOSE_TOPIC: Symbol = symbol_short!("tok_hcls");
// Issue #588: Token Rebalancing
pub const TOKEN_REBALANCE_TOPIC: Symbol = symbol_short!("tok_rebl");
pub const TOKEN_REBALANCED_TOPIC: Symbol = symbol_short!("tok_rebd");

// Issue #529: beneficiary pooling
pub const POOL_CREATED_TOPIC: Symbol = symbol_short!("pool_crt");

// Issue #813: admin transfer timelock
pub const ADMIN_TRANSFER_PROPOSED_TOPIC: Symbol = symbol_short!("adm_prop");
pub const ADMIN_TRANSFER_COMPLETED_TOPIC: Symbol = symbol_short!("adm_done");

// Issue #814: pause reason tracking
pub const PAUSE_REASON_TOPIC: Symbol = symbol_short!("pau_rsn");

// Vault state snapshots
pub const SNAPSHOT_CREATED_TOPIC: Symbol = symbol_short!("snap_crt");
pub const SNAPSHOT_RESTORED_TOPIC: Symbol = symbol_short!("snap_rst");

// Configurable countdown notifications
pub const COUNTDOWN_NOTIF_TOPIC: Symbol = symbol_short!("cd_notif");
pub const SET_COUNTDOWN_TOPIC: Symbol = symbol_short!("set_cd");

// Issue: Check-in Rate Limiting
pub const CHECKIN_RATE_LIMITED_TOPIC: Symbol = symbol_short!("ci_rl");

// Beneficiary capacity limit
pub const BENEFICIARY_CAP_TOPIC: Symbol = symbol_short!("ben_cap");

// Issue: Accelerated TTL Decay
pub const TTL_ACCELERATE_TOPIC: Symbol = symbol_short!("ttl_acc");

// Emergency freeze events
pub const EMERGENCY_FREEZE_TOPIC: Symbol = symbol_short!("emg_frz");
pub const FREEZE_RESOLVED_TOPIC: Symbol = symbol_short!("frz_res");
// Per-vault admin freeze/unfreeze events
pub const FREEZE_VAULT_TOPIC: Symbol = symbol_short!("frz_vlt");
pub const UNFREEZE_VAULT_TOPIC: Symbol = symbol_short!("ufrz_vlt");

// Beneficiary rotation
pub const BEN_ROTATION_TOPIC: Symbol = symbol_short!("ben_rot");

// Inactivity penalty
pub const INACTIVITY_PENALTY_TOPIC: Symbol = symbol_short!("inact_pen");

// Issue #1163: CheckInRecorded event emitted on successful check-in
pub const CHECK_IN_RECORDED_TOPIC: Symbol = symbol_short!("ci_rec");

// Issue: Geographic Check-in Tracking
pub const CHECKIN_GEO_TOPIC: Symbol = symbol_short!("ci_geo");

// Issue #494: Beneficiary Succession Planning
pub const SUCCESSION_SET_TOPIC: Symbol = symbol_short!("suc_set");
pub const SUCCESSION_ACTIVATED_TOPIC: Symbol = symbol_short!("suc_act");

// Issue #495: Beneficiary Escrow
pub const ESCROW_CREATED_TOPIC: Symbol = symbol_short!("esc_cre");
pub const ESCROW_ACCEPTED_TOPIC: Symbol = symbol_short!("esc_acc");
pub const ESCROW_REJECTED_TOPIC: Symbol = symbol_short!("esc_rej");
pub const ESCROW_EXPIRED_TOPIC: Symbol = symbol_short!("esc_exp");

// Issue #496: Dispute Arbitration
pub const ARBITRATOR_SET_TOPIC: Symbol = symbol_short!("arb_set");
pub const ARBITRATION_RULED_TOPIC: Symbol = symbol_short!("arb_rul");

// Issue #497: Beneficiary Notification
pub const VAULT_NOTIFY_TOPIC: Symbol = symbol_short!("v_notif");

// Issue #1337: beneficiary archival notification opt-in/out
pub const BENEFICIARY_ARCHIVAL_OPTIN_TOPIC: Symbol = symbol_short!("ben_ao");
pub const BENEFICIARY_CONTACT_SET_TOPIC: Symbol = symbol_short!("ben_cs");

/// On-chain beneficiary contact record for archival notifications (Issue #1337).
///
/// Stores only an opaque, owner-encrypted contact blob — the plaintext is never
/// readable on-chain.  The vault owner or beneficiary stores a base64-encoded,
/// symmetrically-encrypted payload (e.g., AES-256-GCM) so that only the
/// designated off-chain notification service (which holds the decryption key)
/// can read the contact details.  The `opted_in` flag is stored in cleartext
/// so the scheduler can honour opt-out requests without decrypting.
#[contracttype]
#[derive(Clone)]
pub struct BeneficiaryContactInfo {
    /// Opaque encrypted contact blob (max 512 bytes).
    /// Plaintext (before encryption) format: `email:<addr>|sms:<phone>`
    pub encrypted_contact: Bytes,
    /// Whether this beneficiary has opted in to archival notifications.
    /// Defaults to `true` on first set.
    pub opted_in: bool,
    /// Ledger timestamp when this entry was last updated.
    pub updated_at: u64,
}

// Issue #569: Withdrawal Audit Trail
pub const WITHDRAWAL_AUDIT_TOPIC: Symbol = symbol_short!("wd_audit");
pub const WITHDRAWAL_FAILED_TOPIC: Symbol = symbol_short!("wd_fail");

// Issue #571: Withdrawal Notifications
pub const WITHDRAWAL_NOTIF_TOPIC: Symbol = symbol_short!("wd_notif");

// Issue #572: Withdrawal Dispute
pub const WITHDRAWAL_DISPUTE_FILED_TOPIC: Symbol = symbol_short!("wd_disp");
pub const WITHDRAWAL_DISPUTE_RESOLVED_TOPIC: Symbol = symbol_short!("wd_disp_r");

pub const BENEFICIARY_TRIGGER_SET_TOPIC: Symbol = symbol_short!("ben_trg");
pub const BENEFICIARY_TIER_SET_TOPIC: Symbol = symbol_short!("ben_tier");
pub const BENEFICIARY_WATERFALL_TOPIC: Symbol = symbol_short!("ben_wfl");
pub const BENEFICIARY_REBALANCED_TOPIC: Symbol = symbol_short!("ben_reb");
pub const BEN_COMMITTED_TOPIC: Symbol = symbol_short!("ben_cmt");
pub const BEN_REVEALED_TOPIC: Symbol = symbol_short!("ben_rev");

// Issue #573: Withdrawal Proof
pub const WITHDRAWAL_PROOF_TOPIC: Symbol = symbol_short!("wd_prf");
// Issue #574: Withdrawal Rollback
pub const WITHDRAWAL_ROLLBACK_TOPIC: Symbol = symbol_short!("wd_rbk");
// Issue #575: Withdrawal Rate Limiting
pub const WITHDRAWAL_RATE_LIMITED_TOPIC: Symbol = symbol_short!("wd_rl");
// Issue #576: Withdrawal Escrow
pub const WITHDRAWAL_ESCROW_CREATED_TOPIC: Symbol = symbol_short!("wd_esc");
pub const WITHDRAWAL_ESCROW_VERIFIED_TOPIC: Symbol = symbol_short!("wd_ver");

// Vault Partial Liquidation Before Release (TTL-Legacy);
// Emits (vault_id, amount, ttl_remaining_before). The TTL is intentionally
// not extended on liquidation so the countdown keeps running.
pub const PARTIAL_LIQUIDATE_TOPIC: Symbol = symbol_short!("part_lq");

/// Contract upgrade event topics - Issue #1120
pub const UPGRADE_PROPOSED_TOPIC: Symbol = symbol_short!("upg_prop");
pub const UPGRADE_EXECUTED_TOPIC: Symbol = symbol_short!("upg_exec");
pub const UPGRADE_CANCELLED_TOPIC: Symbol = symbol_short!("upg_canc");

/// Token allowlist event topics - Issue #1118
pub const TOKEN_ALLOWLIST_ADDED_TOPIC: Symbol = symbol_short!("tok_add");
pub const TOKEN_ALLOWLIST_REMOVED_TOPIC: Symbol = symbol_short!("tok_rem");

// Issue 2: vault owner lock/unlock events
pub const VAULT_LOCK_TOPIC: Symbol = symbol_short!("v_lock");
pub const VAULT_UNLOCK_TOPIC: Symbol = symbol_short!("v_unlk");
// Issue 3: low-TTL warning event (per-vault configurable threshold)
pub const LOW_TTL_WARNING_TOPIC: Symbol = symbol_short!("low_ttl");

/// Warning threshold in seconds. If TTL remaining < this value, ping_expiry emits an event.
pub const EXPIRY_WARNING_THRESHOLD: u64 = 86_400; // 24 hours

/// Recovery extension duration in seconds (30 days)
#[allow(dead_code)]
pub const RECOVERY_EXTENSION_DURATION: u64 = 2_592_000;

/// Maximum length for vault metadata string
pub const MAX_METADATA_LEN: u32 = 256;

/// Maximum length for vault name
pub const MAX_NAME_LEN: u32 = 64;

/// Maximum length for vault description
pub const MAX_DESCRIPTION_LEN: u32 = 512;

/// Maximum length for vault notes
pub const MAX_NOTES_LEN: u32 = 1024;

/// Maximum length for custom metadata bytes (2KB) - Issue #378
pub const MAX_CUSTOM_METADATA_LEN: u32 = 2048;
/// Maximum length, in bytes, of a vault's release memo (Issue #791).
pub const MAX_RELEASE_MEMO_LEN: u32 = 256;

#[contracttype(export = false)]
#[derive(Clone)]
pub enum StorageKey {
    Vault(u64),
    OwnerVaults(Address),
    MaxVaultsPerOwner,
    BeneficiaryVaults(Address),
    VaultCount,
    TokenAddress,
    Admin,
    Paused,
    PendingAdmin,
    MinCheckInInterval,
    MaxCheckInInterval,
    Version,
    VestingSchedule(u64, u32),
    VestingPenalty(u64),
    VestingPendingClaim(u64),
    VestingScheduleCount(u64),
    MilestoneVestingSchedule(u64),
    CountdownFired(u64),
    TokenWhitelist(Address),
    WrappedToken(Address),
    VaultMetadata(u64),
    ParentVault(u64),
    VaultPasskeys(u64),
    BackupCodes(u64),
    BeneficiaryDelegate(u64),
    BeneficiaryDelegationChain(u64),
    WithdrawalSchedule(u64),
    DisputeStatus(u64),
    ConditionalAcceptance(u64),
    ConditionalDecline(u64),
    ArchivedVault(u64),
    MaxTtlSeconds,
    TtlDecayRate,
    ReleaseGracePeriodSeconds,
    BridgeConfig(u32),
    TokenConversion(u64),
    TokenStaking(u64),
    PasskeyUsage(u64),
    BeneficiaryStatus(u64),
    PasskeyExpiry(u64, BytesN<32>),
    PendingOwnership(u64),
    PendingBeneficiaryUpdate(u64),
    VaultAuditLog(u64),
    MultiSigConfig(u64),
    MultiSigProposal(u64, u64),
    MultiSigProposalCount(u64),
    MetadataHistory(u64),
    CustomMetadataHistory(u64),
    OwnerVaultCount(Address),
    // Issue #472: state transition audit trail
    StateTransitionLog(u64),
    PasskeyChallenge(u64, BytesN<32>),
    WithdrawalApprovals(u64),
    VaultSnapshot(u64, u64),
    VaultSnapshotTimestamps(u64),
    // Issue #482: TTL prediction history
    CheckInHistory(u64),
    // Issue #873: individual check-in history entry for paginated access
    CheckInEntry(u64, u32),
    // Issue #873: ring buffer head pointer for check-in history
    CheckInHistoryHead(u64),
    // Issue #791: owner-supplied release memo, emitted with the release event
    ReleaseMemo(u64),
    // Issue #873: number of check-in history entries
    CheckInHistoryLen(u64),
    /// Stores the most recently computed adaptive interval suggestion (seconds) - Issue #2
    AdaptiveIntervalSuggestion(u64),
    CheckInStreak(u64),
    // Issue #481: proof-of-work nonce
    CheckInNonce(u64),
    // Issue #480: check-in delegates
    CheckInDelegates(u64),
    // Per-delegation nonce to prevent check-in replay attacks
    DelegateNonce(u64, Address),
    // Issue #946: expiry timestamp for each check-in delegate
    CheckInDelegateExpiry(u64, Address),
    // Issue #498: beneficiary proof of life
    ProofOfLife(u64),
    // Issue #499: beneficiary release votes
    ReleaseVotes(u64),
    ReleaseVoteThreshold(u64),
    BeneficiaryIdentityOracle(u64),
    BeneficiaryIdentityVerification(u64),
    BeneficiaryReleaseTriggers(u64),
    BeneficiaryTierThreshold(u64, Address),
    BeneficiaryStatusEntry(u64, Address),
    // Issue: beneficiary veto of owner-defined release conditions before expiry
    BeneficiaryReleaseConditionVeto(u64),
    // Issue #1291: multi-condition release triggers
    ReleaseConditions(u64),
    // Track whether a vault has already been released once to prevent replayed releases
    ReleaseAttempted(u64),
    // Hibernation: temporary suspension of check-in requirement
    Hibernation(u64),
    LastCheckInTime(u64),
    MinCheckInCooldown,
    VaultDuplicate(Address, Address, u64),
    BeneficiaryRotationSchedule(u64),
    CheckInGeoLog(u64),
    TtlBorrow(u64),
    // Issue #553: encrypted backup codes
    EncryptedBackupCodes(u64),
    // Issue #569: Withdrawal Audit Trail
    WithdrawalAuditLog(u64),
    // Issue #572: Withdrawal Dispute
    WithdrawalDisputes(u64),
    // Issue #565: withdrawal scheduling validation
    WithdrawalScheduleValidation(u64),
    // Issue #566: withdrawal limits by time
    WithdrawalLimit(u64),
    WithdrawalTracker(u64),
    // Issue #567: withdrawal destination whitelist
    WithdrawalWhitelist(u64),
    // Issue #568: withdrawal reversal
    WithdrawalReversal(u64, u64), // (vault_id, withdrawal_id)
    WithdrawalReversalCounter(u64),
    // Issue #545: vesting catch-up
    VestingCatchUp(u64),
    // Issue #546: vesting bonus
    VestingBonus(u64),
    // Issue #584: yield distribution config
    YieldDistributionConfig(u64),
    // Issue #585: token lending
    TokenLending(u64),
    // Issue #586: token collateral
    TokenCollateral(u64),
    // Issue #587: token hedging
    TokenHedge(u64),
    // Issue #588: token rebalancing
    TokenRebalance(u64),
    // Issue #529: beneficiary pooling
    BeneficiaryPool(u64),
    BeneficiaryPoolAlloc(u64),
    // Optional privacy layer: hash commitment to beneficiary identity before release.
    BeneficiaryCommitment(u64),
    RevealedBeneficiary(u64),
    // Issue #525: beneficiary vesting schedules
    BeneficiaryVestingSchedule(u64, Address),
    BeneficiaryVestingCount(u64),
    // Issue #527: beneficiary auctions
    BeneficiaryAuction(u64),
    BeneficiaryAuctionBid(u64, Address),
    BeneficiaryAuctionCount,
    // Issue #809: two-step protocol configuration
    PendingProtocolConfig,
    ProtocolConfigProposedAt,
    // Issue #871: metadata UTF-8 enforcement
    RequireUtf8Metadata,
    // Issue #796: open proposals tracking
    OpenProposals(u64),
    // Issue #965: two-factor authentication
    TwoFactorConfig(u64),
    TwoFactorVerified(u64),
    /// Per-vault admin freeze flag. When `true`, deposit/withdraw/check_in/trigger_release
    /// are all rejected with `ContractError::VaultFrozen`.
    VaultFrozen(u64),
    // Issue #1117: pending multi-sig operations with nonce
    PendingMultiSigOp(u64, u64), // (vault_id, nonce)
    PendingMultiSigOpNonce(u64), // counter per vault
    // Issue #1120: timelock-gated contract upgrade
    PendingUpgrade,
    // Issue #1118: admin-controlled token allowlist
    AllowedTokens,
    // Issue 2: owner-initiated vault lock (separate from admin freeze)
    VaultLocked(u64),
    // Issue 3: per-vault configurable low-TTL warning threshold (seconds)
    VaultLowTtlThreshold(u64),
    // Issue #1337: beneficiary archival notification contact info
    BeneficiaryContactInfo(u64, Address),
    // Issue #1288: multi-beneficiary splits and previously-missing storage keys
    AcceptanceConditions(u64),
    AdminTransferProposedAt,
    BeneficiaryClaimDelegation(u64),
    BeneficiaryConditionalAcceptance(u64),
    BeneficiaryConflict(u64),
    BeneficiaryVaultLimit,
    CompromisedPasskeys(u64),
    CountdownConfig(u64),
    LastPasskeyRotation(u64, BytesN<32>),
    Lending(u64),
    PasskeyLockout(u64),
    PasskeyRecoveryRequest(u64),
    PasskeyRotationPolicy(u64),
    PauseRecord,
    RecoveryCodeHash(u64),
    RecoveryContacts(u64),
    ReleaseSchedule(u64),
    VestingAcceleration(u64),
    VestingForfeiture(u64),
    VestingMilestoneCount(u64),
    VestingMilestones(u64),
    VestingRollover(u64),
    VestingStagger(u64),
    WithdrawalApprovalRequest(u64),
    WithdrawalApprovers(u64),
    WithdrawalEscrow(u64),
    WithdrawalProof(u64, u64),
    WithdrawalRateLimit(u64),
    WithdrawalRollback(u64),
}

/// Check-in history entry for TTL prediction - Issue #482
#[contracttype]
#[derive(Clone)]
pub struct CheckInHistoryEntry {
    pub timestamp: u64,
}

/// Check-in streak tracking - Issue #482
#[contracttype]
#[derive(Clone)]
pub struct CheckInStreak {
    pub current: u32,
    pub best: u32,
    pub last_timestamp: u64,
}

/// A vesting schedule attached to a vault.
/// Funds are released in `num_installments` equal tranches, each separated by `interval` seconds.
/// The first installment becomes claimable at `start_time`.
/// If `cliff_period` > 0, no installments can be claimed until `start_time + cliff_period` has elapsed.
#[contracttype]
#[derive(Clone)]
pub struct VestingSchedule {
    /// Unix timestamp when the first installment becomes claimable.
    pub start_time: u64,
    /// Seconds between consecutive installments.
    pub interval: u64,
    /// Total number of installments.
    pub num_installments: u32,
    /// Number of installments already claimed.
    pub claimed_installments: u32,
    /// Total amount to vest (in stroops). Each installment = total_amount / num_installments,
    /// with the last installment absorbing any remainder.
    pub total_amount: i128,
    /// Cliff duration in seconds from `start_time`. No funds are claimable until
    /// `start_time + cliff_period` has elapsed. Set to 0 to disable.
    pub cliff_period: u64,
}

/// Penalty configuration for late-claim vesting installments.
#[contracttype]
#[derive(Clone)]
pub struct VestingPenaltyConfig {
    /// Penalty in basis points (e.g., 500 = 5%).
    pub penalty_bps: u32,
    /// Seconds after an installment unlocks before the penalty applies.
    pub grace_period_seconds: u64,
}

/// A pending vesting claim awaiting finalization (Issue #548).
#[contracttype]
#[derive(Clone)]
pub struct VestingPendingClaim {
    /// Amount escrowed for the beneficiary.
    pub amount: i128,
    /// Address that will receive the funds once finalized.
    pub beneficiary: Address,
    /// Timestamp when the pending claim was initiated.
    pub initiated_at: u64,
    /// Timestamp after which the reversal window closes.
    pub reversal_deadline: u64,
    /// Updated claimed_installments value (used for rollback on reversal).
    pub new_installments_claimed: u32,
    /// Previous claimed_installments value before initiation.
    pub prev_installments_claimed: u32,
}

/// A single milestone entry in a milestone-based vesting schedule.
/// Each milestone represents a condition (e.g., company revenue target) that,
/// when fulfilled, unlocks a portion of the vault's funds.
#[contracttype]
#[derive(Clone)]
pub struct MilestoneEntry {
    /// Human-readable label for the milestone (e.g., "Revenue reaches $1M")
    pub label: String,
    /// The target value that must be reached to fulfill this milestone
    pub target_value: i128,
    /// The current reported progress toward the target
    pub current_value: i128,
    /// Basis points of total_amount allocated to this milestone (must sum to 10_000 across all milestones)
    pub bps: u32,
    /// Whether this milestone has been marked as fulfilled (current_value >= target_value)
    pub is_fulfilled: bool,
    /// Whether funds for this milestone have been claimed
    pub claimed: bool,
}

/// Milestone-based vesting schedule attached to a vault.
/// Instead of releasing funds on a time-based schedule, funds are released
/// when external milestones (e.g., company revenue targets) are met,
/// as reported by a designated oracle address.
#[contracttype]
#[derive(Clone)]
pub struct MilestoneVestingSchedule {
    /// Total amount to vest across all milestones
    pub total_amount: i128,
    /// The list of milestones with their targets and current progress
    pub milestones: Vec<MilestoneEntry>,
    /// Total amount already claimed from fulfilled milestones
    pub claimed_amount: i128,
    /// Address authorized to report milestone progress
    pub oracle: Address,
    /// Whether vesting is paused (progress reporting and claiming blocked)
    pub paused: bool,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum ReleaseStatus {
    Locked,
    Released,
    Cancelled,
    EmergencyFrozen,
    /// Issue #1281: a release was initiated (conditions passed, release
    /// attempted) but the token transfer to the beneficiary/beneficiaries
    /// failed, leaving the vault's funds in the contract rather than either
    /// still-Locked or fully Released.
    Failed,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum ReleaseCondition {
    TTLExpiry,
    OwnerInitiated,
    Oracle(Address),
}

#[contracttype]
#[derive(Clone)]
pub struct ReleaseEvent {
    pub vault_id: u64,
    pub beneficiary: Address,
    pub amount: i128,
    pub memo: Bytes,
}

/// A single beneficiary entry: (address, basis_points, minimum_threshold).
/// All entries in a vault's beneficiaries must sum to 10_000 bps (100%).
/// If a beneficiary's calculated share is below minimum_threshold (in stroops),
/// they receive nothing and those funds are redistributed to other beneficiaries.
/// Set to 0 to disable the minimum threshold for this beneficiary.
#[contracttype]
#[derive(Clone)]
pub struct BeneficiaryEntry {
    pub address: Address,
    pub bps: u32,
    /// Minimum amount in stroops. If calculated share < minimum_threshold, beneficiary gets 0.
    pub minimum_threshold: i128,
}

/// A percentage-based beneficiary split for use with `create_vault_with_splits`.
///
/// Each entry specifies an address and an integer percentage (0–100). The
/// percentages across all entries in a splits list must sum to exactly 100.
/// Internally the percentage is converted to basis points (BPS) by multiplying
/// by 100 before being stored in `BeneficiaryEntry.bps`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BeneficiarySplit {
    /// The beneficiary's Stellar address.
    pub address: Address,
    /// Whole-number percentage share (1–100). Must be ≥ 1 and the sum of all
    /// entries must equal exactly 100.
    pub percentage: u32,
}

/// Privacy-preserving commitment for a vault beneficiary.
/// The plain beneficiary address remains available in `Vault.beneficiary` for
/// compatibility and public indexing, while the commitment stores a hash that
/// keeps the identity hidden until release time. The hash is computed as
/// `sha256(raw_beneficiary_address_bytes)` and revealed with `reveal_beneficiary`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BeneficiaryCommitment {
    pub commitment: BytesN<32>,
    pub committed_at: u64,
}

/// Bridge configuration for cross-chain support.
#[contracttype]
#[derive(Clone)]
pub struct BridgeConfig {
    pub chain_id: u32,
    pub bridge_address: Address,
    pub is_active: bool,
}

/// Token conversion configuration for a vault.
#[contracttype]
#[derive(Clone)]
pub struct TokenConversion {
    pub vault_id: u64,
    pub from_token: Address,
    pub to_token: Address,
    pub conversion_rate: i128,
    pub enabled: bool,
    pub created_at: u64,
}

/// Token staking configuration for a vault.
#[contracttype]
#[derive(Clone)]
pub struct TokenStaking {
    pub vault_id: u64,
    pub staking_pool: Address,
    pub staked_amount: i128,
    pub staking_start: u64,
    pub annual_yield_bps: u32,
    pub is_active: bool,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum YieldDistributionMode {
    DistributeToBeneficiary,
    Reinvest,
    Split(u32),
}

#[contracttype]
#[derive(Clone)]
pub struct YieldDistributionConfig {
    pub vault_id: u64,
    pub mode: YieldDistributionMode,
    pub last_distribution: u64,
    pub total_distributed: i128,
    pub total_reinvested: i128,
}

/// Passkey hash for multi-passkey support - Issue #394
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PasskeyHash {
    pub hash: BytesN<32>,
    pub added_at: u64,
    /// Optional biometric credential hash bound to this passkey (SHA-256 commitment)
    pub biometric_hash: Option<BytesN<32>>,
    pub deprecated_at: Option<u64>,
    pub usage_count: u64,
    pub last_used_timestamp: u64,
}

/// Backup code entry - Issue #393
#[contracttype]
#[derive(Clone)]
pub struct BackupCode {
    pub hash: BytesN<32>,
    pub used: bool,
}

/// Two-factor authentication configuration - Issue #965
#[contracttype]
#[derive(Clone)]
pub struct TwoFactorConfigData {
    pub enabled: bool,
    /// 0 = TOTP, 1 = SMS, 2 = Email
    pub method: u32,
}

/// Withdrawal approval request - Issue #404
#[contracttype]
#[derive(Clone)]
pub struct WithdrawalRequest {
    pub request_id: u64,
    pub amount: i128,
    pub requested_at: u64,
    pub approved: bool,
}

/// Batch withdrawal instruction - Issue #1292
///
/// Describes a single pending withdrawal to be executed as part of a batched
/// `batch_withdraw` call. By grouping several instructions into a single
/// transaction an owner can settle multiple withdrawals while paying network
/// fees only once.
#[contracttype]
#[derive(Clone)]
pub struct BatchWithdrawal {
    pub vault_id: u64,
    pub destination: Address,
    pub amount: i128,
}

/// Deposit proof - Issue #405
#[contracttype]
#[derive(Clone)]
pub struct DepositProof {
    pub vault_id: u64,
    pub amount: i128,
    pub timestamp: u64,
    pub proof_hash: BytesN<32>,
}

/// Withdrawal proof for compliance - Issue #573
#[contracttype]
#[derive(Clone)]
pub struct WithdrawalProof {
    pub vault_id: u64,
    pub amount: i128,
    pub timestamp: u64,
    pub proof_hash: BytesN<32>,
    pub nonce: u64,
}

/// Withdrawal escrow entry - Issue #576
#[contracttype]
#[derive(Clone)]
pub struct WithdrawalEscrow {
    pub vault_id: u64,
    pub amount: i128,
    pub timestamp: u64,
    pub beneficiary: Address,
    pub verified: bool,
}

/// Withdrawal rollback entry - Issue #574
#[contracttype]
#[derive(Clone)]
pub struct WithdrawalRollback {
    pub vault_id: u64,
    pub original_amount: i128,
    pub rollback_amount: i128,
    pub timestamp: u64,
    pub reason: String,
}

/// Withdrawal rate limit entry - Issue #575
#[contracttype]
#[derive(Clone)]
pub struct WithdrawalRateLimit {
    pub vault_id: u64,
    pub last_withdrawal_time: u64,
    pub withdrawal_count: u32,
    pub cooldown_seconds: u64,
}

/// Recurring withdrawal configuration - Issue #1086
#[contracttype]
#[derive(Clone)]
pub struct RecurringWithdrawal {
    pub amount: i128,
    pub interval_seconds: u64,
    pub destination: Address,
    pub next_at: u64,
}

/// Loan terms for a token loan advanced into a vault by a lender. The vault
/// owner must repay `amount` (plus a late penalty if repaid after
/// `repayment_deadline`) to fully settle the loan.
#[contracttype]
#[derive(Clone)]
pub struct TokenLending {
    pub lender: Address,
    pub amount: i128,
    pub repayment_deadline: u64, // ledger timestamp
    pub late_penalty_bps: u32,
    pub repaid: bool,
}

/// Point-in-time status snapshot for a single vault, used by batch status lookups.
#[contracttype]
#[derive(Clone)]
pub struct VaultStatusSummary {
    pub vault_id: u64,
    pub status: ReleaseStatus,
    pub balance: i128,
    pub ttl_remaining: Option<u64>,
}

#[contracttype]
#[derive(Clone)]
pub struct Vault {
    pub owner: Address,
    /// Primary beneficiary kept for backwards-compatible single-beneficiary reads.
    /// When beneficiaries is non-empty, this field is ignored during trigger_release.
    pub beneficiary: Address,
    pub balance: i128,
    pub check_in_interval: u64, // seconds
    pub last_check_in: u64,     // ledger timestamp
    pub created_at: u64,        // vault creation timestamp
    pub creation_ledger: u64,   // ledger sequence number at vault creation
    pub status: ReleaseStatus,
    /// Multi-beneficiary split. Empty means use `beneficiary` (100%).
    pub beneficiaries: Vec<BeneficiaryEntry>,
    /// Optional short metadata string (label or IPFS hash).
    pub metadata: String,
    /// Token contract address for this vault. Uses default XLM token if not specified.
    pub token_address: Address,
    /// Custom metadata as bytes (max 2KB) - Issue #378
    pub custom_metadata: Bytes,
    /// Whether the vault is paused - Issue #380
    pub is_paused: bool,
    /// Release condition for the vault - Issue #379
    pub release_condition: ReleaseCondition,
    /// Parent vault ID for inheritance chain - Issue #381
    pub parent_vault_id: Option<u64>,
    /// Primary passkey hash for backwards compatibility - Issue #392, #394
    pub passkey_hash: Option<Bytes>,
    /// Maximum deposit amount - Issue #403
    pub max_deposit_amount: Option<i128>,
    /// Withdrawal approval threshold - Issue #404
    pub withdrawal_approval_threshold: Option<i128>,
    /// Maximum amount releasable per trigger_release call - Issue #382
    pub spending_limit: Option<i128>,
    /// Penalty in basis points deducted per missed check-in interval
    pub inactivity_penalty_bps: Option<u32>,
    /// Burn percentage in basis points (0-10000). 0 means no burn.
    pub burn_percentage: u32,
    /// Address that receives inactivity penalty transfers
    pub penalty_recipient: Option<Address>,
    /// Passkey rotation grace period in seconds - Issue #936
    pub passkey_rotation_period: u64,
    /// Challenge-response timeout window in seconds - Issue #938
    pub challenge_timeout_seconds: u64,
    /// Multi-sig passkey threshold for withdrawals - Issue #939
    pub multi_sig_threshold: u32,
    /// Operations that require multi-sig approval (2-of-N) - Issue #1117
    pub multisig_required_ops: Vec<MultiSigOperation>,
    /// Whether adaptive interval adjustment is enabled for this vault - Issue #2
    pub adaptive_interval_enabled: bool,
    /// Composite check-in reliability score scaled to 10000 (100%) - check-in scoring
    pub check_in_score: u32,
    /// Total number of check-ins recorded for the vault
    pub total_check_ins: u32,
    /// Number of check-ins performed on time (within the check-in interval)
    pub on_time_check_ins: u32,
    /// Minimum balance guard to prevent vault drainage - Issue #1088
    pub min_balance_guard: Option<i128>,
    /// Recurring withdrawal configuration - Issue #1086
    pub recurring_withdrawal: Option<RecurringWithdrawal>,
    /// Withdrawal rate limit per time window - Issue #1084
    pub withdrawal_limit_per_window: Option<i128>,
    /// Time window for withdrawal rate limiting in seconds - Issue #1084
    pub withdrawal_window_seconds: u64,
    /// Amount withdrawn in current window - Issue #1084
    pub withdrawn_in_window: i128,
    /// Start time of current withdrawal window - Issue #1084
    pub window_start: u64,
}

/// Passkey usage entry for tracking check-ins - Issue #395
#[contracttype]
#[derive(Clone)]
pub struct PasskeyUsageEntry {
    pub passkey_hash: BytesN<32>,
    pub timestamp: u64,
}

/// Passkey analytics report - Issue #937
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PasskeyAnalytics {
    pub passkey_hash: BytesN<32>,
    pub usage_count: u64,
    pub last_used_timestamp: u64,
}

/// Beneficiary status enum - Issue #397
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum BeneficiaryStatus {
    Pending,
    Accepted,
    Declined,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum ReleaseTrigger {
    Expiry,
    Manual,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct BeneficiaryTriggerSetEvent {
    pub vault_id: u64,
    pub beneficiary: Address,
    pub triggers: Vec<ReleaseTrigger>,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct BeneficiaryTierSetEvent {
    pub vault_id: u64,
    pub beneficiary: Address,
    pub tier_threshold: i128,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct BeneficiaryWaterfallEvent {
    pub vault_id: u64,
    pub skipped_beneficiary: Address,
    pub reason: String,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct BeneficiaryRebalancedEvent {
    pub vault_id: u64,
    pub remaining_bps: u32,
}

/// Dispute status enum - Issue #399
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum DisputeStatus {
    None,
    Filed,
    Resolved,
}

/// Withdrawal schedule entry - Issue #402
#[contracttype]
#[derive(Clone)]
pub struct WithdrawalScheduleEntry {
    pub timestamp: u64,
    pub amount: i128,
}

/// Conditional acceptance entry - Issue #400, #503
#[contracttype]
#[derive(Clone)]
pub struct ConditionalAcceptanceEntry {
    pub conditions: String,
    pub approved_by_owner: bool,
    pub acceptance_deadline: Option<u64>,
    pub min_balance_threshold: Option<i128>,
}

/// Beneficiary conditional acceptance with threshold - Issue #503
#[contracttype]
#[derive(Clone)]
pub struct BeneficiaryConditionalAcceptance {
    pub min_balance_threshold: i128,
    pub accepted_at: u64,
}

/// Beneficiary conditional decline with threshold - Issue #503
/// Allows beneficiary to decline vault assignment if balance is below a configurable minimum threshold
#[contracttype]
#[derive(Clone)]
pub struct BeneficiaryConditionalDecline {
    pub max_balance_threshold: i128,
    pub declined_at: u64,
    pub reason: String,
}

/// Beneficiary delegation of claim rights to a trusted proxy address - Issue #944
#[contracttype]
#[derive(Clone)]
pub struct BeneficiaryClaimDelegation {
    pub proxy: Address,
    pub expiry: u64,
}

/// Beneficiary identity verification entry.
#[contracttype]
#[derive(Clone)]
pub struct BeneficiaryIdentityVerificationEntry {
    pub beneficiary: Address,
    pub verifier: Address,
    pub verified_at: u64,
}

/// Beneficiary conflict claim - Issue #502
#[contracttype]
#[derive(Clone)]
pub struct BeneficiaryConflictClaim {
    pub claimant: Address,
    pub reason: String,
    pub filed_at: u64,
}

/// Beneficiary conflict resolution - Issue #502
#[contracttype]
#[derive(Clone, PartialEq)]
pub enum ConflictResolution {
    Pending,
    Approved(Address),
    Rejected,
}

/// Beneficiary conflict entry - Issue #502
#[contracttype]
#[derive(Clone)]
pub struct BeneficiaryConflict {
    pub vault_id: u64,
    pub claims: Vec<BeneficiaryConflictClaim>,
    pub resolution: ConflictResolution,
    pub resolved_at: Option<u64>,
}

/// Activity log entry for forensic audit trail
#[contracttype]
#[derive(Clone)]
pub struct ActivityLogEntry {
    pub action: String,
    pub caller: Address,
    pub timestamp: u64,
    pub details: String,
}

/// Archived vault info for restoration - Issue #443
#[contracttype]
#[derive(Clone)]
pub struct ArchivedVaultInfo(pub Vault);

/// A single metadata version snapshot - Issue #468
#[contracttype]
#[derive(Clone)]
pub struct MetadataVersionEntry {
    pub version: u32,
    pub metadata: String,
    pub updated_at: u64,
    pub updated_by: Address,
}

/// A single custom metadata history entry (raw bytes + timestamp) - Issue #931
#[contracttype]
#[derive(Clone)]
pub struct CustomMetadataEntry {
    pub metadata: Bytes,
    pub timestamp: u64,
}

/// Ownership transfer request
#[contracttype]
#[derive(Clone)]
pub struct OwnershipTransferRequest {
    pub new_owner: Address,
    pub initiated_at: u64,
    pub unlocks_at: u64,
    pub expires_at: u64,
}

/// Pending beneficiary update request - Issue #490
#[contracttype]
#[derive(Clone)]
pub struct PendingBeneficiaryUpdate {
    pub new_beneficiary: Address,
    pub initiated_at: u64,
    pub unlocks_at: u64,
}

/// Audit entry for vault operations
#[contracttype]
#[derive(Clone)]
pub struct AuditEntry {
    pub action: String,
    pub caller: Address,
    pub timestamp: u64,
    pub operation: String,
    pub actor: Address,
    pub details: String,
}

/// Withdrawal audit trail entry - Issue #569
#[contracttype]
#[derive(Clone)]
pub struct WithdrawalAuditEntry {
    pub vault_id: u64,
    pub caller: Address,
    pub amount: i128,
    pub timestamp: u64,
    pub success: bool,
    pub error_reason: String,
}

/// Withdrawal dispute entry - Issue #572
#[contracttype]
#[derive(Clone)]
pub struct WithdrawalDispute {
    pub vault_id: u64,
    pub withdrawal_timestamp: u64,
    pub dispute_filed_at: u64,
    pub dispute_expires_at: u64,
    pub status: DisputeStatus,
    pub reason: String,
    pub resolved_at: Option<u64>,
}

/// Contract upgrade proposal with timelock - Issue #1120
#[contracttype]
#[derive(Clone)]
pub struct UpgradeProposal {
    /// Hash of the new WASM bytecode
    pub new_wasm_hash: Bytes,
    /// Address of the admin who proposed the upgrade
    pub proposed_by: Address,
    /// Timestamp when the upgrade was proposed
    pub proposed_at: u64,
    /// Timestamp when the upgrade can be executed (proposed_at + 72 hours)
    pub executable_at: u64,
}

/// Multi-signature configuration
#[contracttype]
#[derive(Clone)]
pub struct MultiSigConfig {
    pub signers: Vec<Address>,
    pub threshold: u32,
}

/// Multi-signature proposal
#[contracttype]
#[derive(Clone)]
pub struct MultiSigProposal {
    pub id: u64,
    pub operation: MultiSigOperation,
    pub approvals: Vec<Address>,
    pub status: ProposalStatus,
    pub expires_at: u64,
    pub vault_id: u64,
    pub payload: Bytes,
    pub address_payload: Option<Address>,
    pub created_at: u64,
}

/// Multi-signature operation types
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum MultiSigOperation {
    Withdraw,
    UpdateBeneficiary,
    CancelVault,
    UpdateCheckInInterval,
    TransferOwnership,
}

/// Proposal status
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum ProposalStatus {
    Pending,
    Approved,
    Rejected,
    Executed,
    Expired,
    Vetoed,
}

/// State transition record for vault status changes - Issue #472
#[contracttype]
#[derive(Clone)]
pub struct StateTransitionEntry {
    pub from_status: ReleaseStatus,
    pub to_status: ReleaseStatus,
    pub actor: Address,
    pub timestamp: u64,
}

/// Pending multi-signature operation - Issue #1117
/// Represents an operation awaiting co-signatures from passkeys.
#[contracttype]
#[derive(Clone)]
pub struct PendingMultiSigOp {
    pub nonce: u64,
    pub vault_id: u64,
    pub operation: MultiSigOperation,
    pub signers: Vec<Address>, // Addresses that have signed
    pub payload: Bytes,
    pub address_payload: Option<Address>,
    pub created_at: u64,
    pub expires_at: u64,
    pub threshold: u32,
}

/// Ownership proof result - Issue #473
#[contracttype]
#[derive(Clone)]
pub struct OwnershipProof {
    pub vault_id: u64,
    pub owner_hash: BytesN<32>,
    pub timestamp: u64,
    pub is_active: bool,
}

/// Vault integrity report - Issue #474
#[contracttype]
#[derive(Clone)]
pub struct IntegrityReport {
    pub vault_id: u64,
    pub checksum: BytesN<32>,
    pub is_valid: bool,
    pub timestamp: u64,
}

/// A shared TTL pool that multiple vaults can join.
/// A single `pool_check_in` resets `last_check_in` for all member vaults.
#[contracttype]
#[derive(Clone)]
pub struct TtlPool {
    pub pool_id: u64,
    pub owner: Address,
    pub check_in_interval: u64,
    pub last_check_in: u64,
    pub created_at: u64,
}

/// A biometric credential entry (fingerprint or face template hash).
/// The raw biometric data never leaves the device — only the SHA-256
/// hash commitment is stored on-chain.
#[contracttype]
#[derive(Clone)]
pub struct BiometricEntry {
    pub credential_hash: BytesN<32>,
    pub added_at: u64,
}

/// Hibernation entry — records when a vault entered hibernation and for how long.
/// While hibernating, the vault's expiry deadline is extended by `duration_seconds`,
/// so no check-ins are required during that period.
#[contracttype]
#[derive(Clone)]
pub struct HibernationEntry {
    /// Ledger timestamp when hibernation started.
    pub started_at: u64,
    /// How many seconds the hibernation lasts.
    pub duration_seconds: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct TtlBorrowRecord {
    pub borrower_vault_id: u64,
    pub lender_vault_id: u64,
    pub borrowed_seconds: u64,
    pub borrowed_at: u64,
    pub repaid: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct GeoCheckInEntry {
    pub latitude_micro: i64,
    pub longitude_micro: i64,
    pub country_code: String,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct ProofOfLifeEntry {
    pub beneficiary: Address,
    pub submitted_at: u64,
    pub valid_until: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct ReleaseVoteEntry {
    pub voter: Address,
    pub approve: bool,
    pub voted_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct BeneficiaryRotationEntry {
    pub effective_timestamp: u64,
    pub new_beneficiaries: Vec<BeneficiaryEntry>,
}

/// Configurable countdown notification thresholds for a vault.
/// Each threshold (in seconds before expiry) triggers a `cd_notif` event
/// when `check_countdown` is called and the TTL crosses that boundary.
/// Default thresholds: 7 days (604800), 3 days (259200), 1 day (86400).
#[contracttype]
#[derive(Clone)]
pub struct CountdownConfig {
    /// Sorted descending list of thresholds in seconds (e.g. [604800, 259200, 86400]).
    pub thresholds: Vec<u64>,
}

/// Withdrawal limit configuration - Issue #566
#[contracttype]
#[derive(Clone)]
pub struct WithdrawalLimit {
    pub daily_limit: i128,
    pub weekly_limit: i128,
    pub monthly_limit: i128,
}

/// Withdrawal tracking entry - Issue #566
#[contracttype]
#[derive(Clone)]
pub struct WithdrawalTracker {
    pub daily_withdrawn: i128,
    pub daily_reset_at: u64,
    pub weekly_withdrawn: i128,
    pub weekly_reset_at: u64,
    pub monthly_withdrawn: i128,
    pub monthly_reset_at: u64,
}

/// Withdrawal destination whitelist entry - Issue #567
#[contracttype]
#[derive(Clone)]
pub struct WhitelistEntry {
    pub address: Address,
    pub added_at: u64,
    pub label: String,
}

/// Withdrawal reversal entry - Issue #568
#[contracttype]
#[derive(Clone)]
pub struct WithdrawalReversal {
    pub withdrawal_id: u64,
    pub amount: i128,
    pub withdrawn_at: u64,
    pub grace_period_until: u64,
    pub reversed: bool,
}

/// Vesting catch-up configuration - Issue #545.
/// When enabled, a beneficiary who missed claiming periods can catch up
/// and claim all accumulated missed installments in a single call.
#[contracttype]
#[derive(Clone)]
pub struct VestingCatchUpConfig {
    /// Whether catch-up claiming is enabled for this vault.
    pub enabled: bool,
    /// Maximum number of missed installments that can be caught up in one call.
    /// 0 means unlimited (all missed installments can be claimed at once).
    pub max_catchup_installments: u32,
}

/// Vesting bonus configuration - Issue #546.
/// Awards a bonus to the beneficiary when they claim on time (within the grace window).
#[contracttype]
#[derive(Clone)]
pub struct VestingBonusConfig {
    /// Bonus in basis points awarded for on-time claims (e.g., 100 = 1%).
    pub bonus_bps: u32,
    /// Seconds after an installment unlocks within which a claim is considered "on time".
    pub on_time_window_seconds: u64,
}

/// Token collateral configuration - Issue #586.
/// Vault tokens used as collateral for a loan.
#[contracttype]
#[derive(Clone)]
pub struct TokenCollateral {
    pub vault_id: u64,
    pub collateral_amount: i128,
    pub loan_amount: i128,
    /// Collateral ratio in basis points (e.g., 15000 = 150%).
    pub collateral_ratio_bps: u32,
    pub active: bool,
    pub created_at: u64,
}

/// Token hedge configuration - Issue #587.
/// Hedge vault token price risk using a derivative position.
#[contracttype]
#[derive(Clone)]
pub struct TokenHedge {
    pub vault_id: u64,
    /// Token used for the hedge (e.g., a stablecoin).
    pub hedge_token: Address,
    pub notional_amount: i128,
    /// Strike price in basis points relative to current price.
    pub strike_price_bps: u32,
    /// Unix timestamp when the hedge expires.
    pub expiry: u64,
    pub active: bool,
    pub created_at: u64,
}

/// A single token weight entry for rebalancing - Issue #588.
#[contracttype]
#[derive(Clone)]
pub struct TokenWeight {
    pub token: Address,
    /// Target allocation in basis points (all entries must sum to 10000).
    pub target_bps: u32,
}

/// Token rebalancing configuration - Issue #588.
/// Automatically rebalances a multi-token portfolio based on target weights.
#[contracttype]
#[derive(Clone)]
pub struct TokenRebalanceConfig {
    pub vault_id: u64,
    pub target_weights: Vec<TokenWeight>,
    pub last_rebalance: u64,
    /// Drift threshold in basis points that triggers a rebalance (e.g., 500 = 5%).
    pub rebalance_threshold_bps: u32,
    pub total_rebalances: u32,
}

/// A pool of beneficiaries whose BPS allocations are combined - Issue #529.
#[contracttype]
#[derive(Clone)]
pub struct BeneficiaryPool {
    pub pool_id: u64,
    pub members: Vec<Address>,
    pub total_bps: u32,
}

/// Beneficiary-specific vesting schedule - Issue #525.
/// Different beneficiaries can have different vesting timelines.
#[contracttype]
#[derive(Clone)]
pub struct BeneficiaryVestingSchedule {
    pub beneficiary: Address,
    pub vault_id: u64,
    /// Unix timestamp when this beneficiary's vesting starts.
    pub start_time: u64,
    /// Seconds between installments.
    pub interval: u64,
    /// Total number of installments.
    pub num_installments: u32,
    /// Claimed installments for this beneficiary.
    pub claimed_installments: u32,
    /// Total amount allocated to this beneficiary.
    pub total_amount: i128,
    /// Cliff duration in seconds from start_time.
    pub cliff_period: u64,
}

/// Beneficiary auction bid - Issue #527.
/// Allow beneficiaries to bid for larger allocations.
#[contracttype]
#[derive(Clone)]
pub struct BeneficiaryAuctionBid {
    pub auction_id: u64,
    pub bidder: Address,
    pub bid_amount: i128,
    /// Desired allocation in basis points (e.g., 5000 = 50%).
    pub desired_allocation_bps: u32,
    pub bid_timestamp: u64,
    /// True if bid was accepted.
    pub accepted: bool,
}

/// Beneficiary auction configuration - Issue #527.
/// Auction for determining final beneficiary allocations.
#[contracttype]
#[derive(Clone)]
pub struct BeneficiaryAuction {
    pub auction_id: u64,
    pub vault_id: u64,
    /// Start of bidding period (Unix timestamp).
    pub start_time: u64,
    /// End of bidding period (Unix timestamp).
    pub end_time: u64,
    /// Total allocation being auctioned in basis points.
    pub total_allocation_bps: u32,
    /// Minimum bid amount in stroops.
    pub minimum_bid: i128,
    pub bids: Vec<BeneficiaryAuctionBid>,
    /// True if auction has concluded.
    pub finalized: bool,
    /// Winner address (if finalized).
    pub winner: Option<Address>,
}

/// Event topics for vesting and auction - Issue #525, #527
pub const SET_BENEFICIARY_VESTING_TOPIC: Symbol = symbol_short!("set_bvst");
pub const CLAIM_BENEFICIARY_VESTING_TOPIC: Symbol = symbol_short!("clm_bvst");
pub const AUCTION_CREATED_TOPIC: Symbol = symbol_short!("auc_crt");
pub const AUCTION_BID_TOPIC: Symbol = symbol_short!("auc_bid");
pub const AUCTION_FINALIZED_TOPIC: Symbol = symbol_short!("auc_fin");

// Issue #809: two-step protocol configuration update
pub const PROTOCOL_CONFIG_PROPOSED_TOPIC: Symbol = symbol_short!("pc_prop");
pub const PROTOCOL_CONFIG_APPLIED_TOPIC: Symbol = symbol_short!("pc_apply");

/// Issue #1117: Pending multi-sig operation topics
pub const PENDING_MULTISIG_OP_CREATED_TOPIC: Symbol = symbol_short!("pm_crtd");
pub const PENDING_MULTISIG_OP_COSIGNED_TOPIC: Symbol = symbol_short!("pm_cosign");
pub const PENDING_MULTISIG_OP_EXECUTED_TOPIC: Symbol = symbol_short!("pm_exec");
pub const PENDING_MULTISIG_OP_EXPIRED_TOPIC: Symbol = symbol_short!("pm_exp");

/// Aggregated protocol-level configuration — Issue #810.
/// Returned by `get_protocol_config` so off-chain clients avoid raw storage key coupling.
#[contracttype]
#[derive(Clone)]
pub struct ProtocolConfig {
    /// Minimum check-in interval in seconds. Defaults to MIN_CHECK_IN_INTERVAL (3600s = 1 hour) if None.
    pub min_check_in_interval: Option<u64>,
    pub max_check_in_interval: Option<u64>,
    pub max_ttl_seconds: u64,
    pub ttl_decay_rate: u32,
    /// When true, `set_vault_metadata`, `update_metadata`, and `update_metadata_versioned`
    /// reject metadata bytes that are not valid UTF-8 — Issue #871.
    pub require_utf8_metadata: bool,
    /// Maximum number of vaults a single owner may create — Issue #767.
    pub max_vaults_per_owner: u32,
}

/// Vault state snapshot at a specific point in time.
#[contracttype]
#[derive(Clone)]
pub struct VaultSnapshot {
    pub vault: Vault,
    pub timestamp: u64,
    pub content_hash: BytesN<32>,
}

// ============================================================
// Issue #951: Graduated Release Schedule
// ============================================================

/// A single tranche in a graduated release schedule.
/// The beneficiary can claim this tranche once `release_timestamp` has passed.
#[contracttype]
#[derive(Clone)]
pub struct ReleaseTranche {
    /// Amount (in stroops) allocated to this tranche.
    pub amount: i128,
    /// Unix timestamp after which this tranche can be claimed.
    pub release_timestamp: u64,
    /// Whether this tranche has already been claimed.
    pub claimed: bool,
}

/// A graduated release schedule attached to a vault.
/// On `trigger_release`, tranches are not distributed immediately; instead,
/// the beneficiary calls `claim_tranche` as each tranche unlocks.
#[contracttype]
#[derive(Clone)]
pub struct ReleaseSchedule {
    /// Ordered list of tranches. Amounts must sum to the vault balance at
    /// the time `set_release_schedule` is called.
    pub tranches: Vec<ReleaseTranche>,
    /// Total amount escrowed across all tranches.
    pub total_amount: i128,
    /// Amount already claimed by the beneficiary.
    pub claimed_amount: i128,
    /// Whether the schedule has been activated (i.e., `trigger_release` fired).
    pub active: bool,
}

// Issue #951: topic constants for release schedule events
pub const SET_RELEASE_SCHEDULE_TOPIC: Symbol = symbol_short!("rl_sched");
pub const TRANCHE_CLAIMED_TOPIC: Symbol = symbol_short!("tr_claim");

// Issue #1338: vault export/import for disaster recovery
pub const VAULT_EXPORTED_TOPIC: Symbol = symbol_short!("v_export");
pub const VAULT_IMPORTED_TOPIC: Symbol = symbol_short!("v_import");

/// Exported vault configuration for disaster recovery (Issue #1338).
///
/// This struct captures all configuration needed to reconstruct a vault
/// if its on-chain state is lost due to TTL expiry/archival. It does NOT
/// include the balance (funds must be re-deposited) or runtime state
/// (last_check_in, creation_ledger, status) — those are reset on import.
///
/// The `exported_at` timestamp and `original_vault_id` serve as a
/// content-fingerprint so importers can verify they are re-creating the
/// right vault and detect stale / tampered exports.
#[contracttype]
#[derive(Clone)]
pub struct VaultExportConfig {
    /// ID of the vault this config was exported from.
    pub original_vault_id: u64,
    /// Vault owner address.
    pub owner: Address,
    /// Primary beneficiary address.
    pub beneficiary: Address,
    /// Check-in interval in seconds.
    pub check_in_interval: u64,
    /// Token contract address used by the vault.
    pub token_address: Address,
    /// Multi-beneficiary split (empty = 100% to `beneficiary`).
    pub beneficiaries: Vec<BeneficiaryEntry>,
    /// Optional short metadata / IPFS label.
    pub metadata: String,
    /// Optional custom metadata bytes (max 2 KB).
    pub custom_metadata: Bytes,
    /// Optional spending limit per trigger_release call (stroops).
    pub spending_limit: Option<i128>,
    /// Optional maximum deposit amount (stroops).
    pub max_deposit_amount: Option<i128>,
    /// Release condition (TTLExpiry, OwnerInitiated, or Oracle).
    pub release_condition: ReleaseCondition,
    /// Ledger timestamp when this config was exported.
    pub exported_at: u64,
}

/// Structured event emitted by `create_vault` on successful vault creation.
/// Issue #1325: allows off-chain indexers to detect new vaults without polling.
#[contracttype]
#[derive(Clone)]
pub struct VaultCreatedEvent {
    pub vault_id: u64,
    pub owner: Address,
    pub beneficiary: Address,
    pub check_in_interval: u64,
}

/// Structured event emitted by `check_in` on successful TTL extension.
/// Issue #1323: allows off-chain listeners (reminders, dashboards) to detect check-ins
/// without polling vault state.
#[contracttype]
#[derive(Clone)]
pub struct CheckInEvent {
    pub vault_id: u64,
    /// The new expiry timestamp after the check-in (last_check_in + check_in_interval).
    pub new_ttl: u64,
    pub caller: Address,
}

/// Structured event emitted by `update_beneficiary` when a beneficiary change is initiated.
/// Issue #1326: allows off-chain systems to audit beneficiary changes.
#[contracttype]
#[derive(Clone)]
pub struct BeneficiaryUpdatedEvent {
    pub vault_id: u64,
    pub old_beneficiary: Address,
    pub new_beneficiary: Address,
}

/// Lockout state recorded when a vault is temporarily locked after repeated failed
/// passkey attempts (Issue #562).
#[contracttype]
#[derive(Clone)]
pub struct PasskeyLockout {
    pub locked_at: u64,
    pub unlock_at: u64,
    pub failed_attempts: u32,
}

/// Pending passkey recovery request initiated via approved contacts (Issue #563).
#[contracttype]
#[derive(Clone)]
pub struct PasskeyRecoveryRequest {
    pub new_passkey_hash: BytesN<32>,
    pub initiated_at: u64,
    pub recovery_code: String,
    pub approved_contacts: Vec<Address>,
    pub required_contacts: u32,
}

/// Policy enforcing periodic passkey rotation (Issue #561).
#[contracttype]
#[derive(Clone)]
pub struct PasskeyRotationPolicy {
    pub rotation_period_days: u32,
    pub enforce: bool,
}

/// Record stored when the contract is paused, tracking who paused it and why
/// (Issue #820).
#[contracttype]
#[derive(Clone)]
pub struct PauseRecord {
    pub paused_by: Address,
    pub reason: Bytes,
    pub paused_at: u64,
}

/// An individual vesting milestone with a human-readable description and a
/// designated attestor (Issue #827).
#[contracttype]
#[derive(Clone)]
pub struct VestingMilestone {
    pub milestone_id: u64,
    pub description: String,
    pub attestor: Address,
    pub unlocked: bool,
}

/// Configuration enabling oracle-accelerated vesting (Issue #855).
#[contracttype]
#[derive(Clone)]
pub struct VestingAccelerationConfig {
    pub oracle: Address,
    pub accelerated: bool,
}

/// Configuration for vesting forfeiture to a designated recipient (Issue #856).
#[contracttype]
#[derive(Clone)]
pub struct VestingForfeitureConfig {
    pub forfeiture_recipient: Address,
    pub forfeited: bool,
}

/// Configuration allowing unclaimed installments to roll over (Issue #857).
#[contracttype]
#[derive(Clone)]
pub struct VestingRolloverConfig {
    pub enabled: bool,
    pub rolled_amount: i128,
}

/// A staggered vesting entry for one beneficiary (Issue #544).
#[contracttype]
#[derive(Clone)]
pub struct VestingStaggerEntry {
    pub beneficiary: Address,
    pub bps: u32,
    pub start_time: u64,
    pub interval: u64,
    pub num_installments: u32,
    pub claimed_installments: u32,
}

/// A withdrawal requiring approval from multiple parties before execution.
#[contracttype]
#[derive(Clone)]
pub struct WithdrawalApprovalRequest {
    pub amount: i128,
    pub requested_at: u64,
    pub approvals: Vec<Address>,
    pub required_approvals: u32,
    pub expires_at: u64,
}

/// Timelock delay (seconds) applied to admin ownership transfers.
pub const ADMIN_TRANSFER_TIMELOCK: u64 = 172_800;

/// Default minimum interval (seconds) between consecutive check-ins.
pub const DEFAULT_MIN_CHECKIN_COOLDOWN: u64 = 60;

/// Maximum number of seconds a single acceleration call may advance the deadline.
pub const MAX_ACCELERATE_SECONDS: u64 = 2_592_000;

/// Expiry (seconds) for pending multi-signature operations once created.
pub const PENDING_MULTISIG_OP_EXPIRY: u64 = 604_800;

/// Timelock delay (seconds) before a proposed protocol config takes effect.
pub const PROTOCOL_CONFIG_TIMELOCK: u64 = 172_800;

/// Versioned milestone topic labels.
pub const MILESTONE_ADDED_TOPIC: Symbol = symbol_short!("ms_add");
pub const MILESTONE_ATTESTED_TOPIC: Symbol = symbol_short!("ms_attest");
pub const LOAN_ENABLED_TOPIC: Symbol = symbol_short!("loan_en");
pub const LOAN_REPAID_TOPIC: Symbol = symbol_short!("loan_rep");
pub const TWO_FACTOR_ENABLED_TOPIC: Symbol = symbol_short!("2fa_en");
pub const TWO_FACTOR_DISABLED_TOPIC: Symbol = symbol_short!("2fa_dis");
pub const TWO_FACTOR_VERIFIED_TOPIC: Symbol = symbol_short!("2fa_vrf");
