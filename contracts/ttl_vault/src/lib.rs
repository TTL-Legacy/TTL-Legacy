use soroban_sdk::{contract, contractimpl, token, Address, Env};

mod test;

mod types;
use types::{DataKey, ReleaseStatus, Vault, VaultError};

#[contract]
pub struct TtlVaultContract;

#[contractimpl]
impl TtlVaultContract {
    /// Create a new vault. Returns the vault ID.
    pub fn create_vault(
        env: Env,
        owner: Address,
        beneficiary: Address,
        check_in_interval: u64,
    ) -> u64 {
        owner.require_auth();

        let vault_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::VaultCount)
            .unwrap_or(0u64)
            + 1;

        let vault = Vault {
            owner,
            beneficiary,
            balance: 0,
            check_in_interval,
            last_check_in: env.ledger().timestamp(),
            status: ReleaseStatus::Locked,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Vault(vault_id), &vault);
        env.storage()
            .instance()
            .set(&DataKey::VaultCount, &vault_id);

        vault_id
    }

    /// Owner checks in, resetting the TTL countdown.
    ///
    /// Auth model: `vault.owner.require_auth()` enforces that the transaction
    /// must be signed by the vault owner. The explicit `NotOwner` check below
    /// runs first so callers receive a clear error code rather than a generic
    /// auth failure when they supply a vault_id they do not own.
    pub fn check_in(env: Env, vault_id: u64, caller: Address) -> Result<(), VaultError> {
        caller.require_auth();
        let mut vault: Vault = Self::load_vault(&env, vault_id);

        if caller != vault.owner {
            return Err(VaultError::NotOwner);
        }

        if vault.status != ReleaseStatus::Locked {
            return Err(VaultError::AlreadyReleased);
        }

        vault.last_check_in = env.ledger().timestamp();
        env.storage()
            .persistent()
            .set(&DataKey::Vault(vault_id), &vault);
        Ok(())
    }

    /// Deposit XLM into the vault.
    pub fn deposit(env: Env, vault_id: u64, from: Address, amount: i128) {
        from.require_auth();
        assert!(amount > 0, "amount must be positive");

        let mut vault: Vault = Self::load_vault(&env, vault_id);
        assert!(
            vault.status == ReleaseStatus::Locked,
            "vault already released"
        );

        let xlm = token::Client::new(&env, &env.current_contract_address());
        xlm.transfer(&from, &env.current_contract_address(), &amount);

        vault.balance += amount;
        env.storage()
            .persistent()
            .set(&DataKey::Vault(vault_id), &vault);
    }

    /// Owner withdraws from the vault.
    pub fn withdraw(env: Env, vault_id: u64, amount: i128) -> Result<(), VaultError> {
        if amount <= 0 {
            return Err(VaultError::InvalidAmount);
        }
        let mut vault: Vault = Self::load_vault(&env, vault_id);
        vault.owner.require_auth();

        if vault.status != ReleaseStatus::Locked {
            return Err(VaultError::AlreadyReleased);
        }
        if vault.balance < amount {
            return Err(VaultError::InsufficientBalance);
        }

        let xlm = token::Client::new(&env, &env.current_contract_address());
        xlm.transfer(&env.current_contract_address(), &vault.owner, &amount);

        vault.balance -= amount;
        env.storage()
            .persistent()
            .set(&DataKey::Vault(vault_id), &vault);
        Ok(())
    }

    /// Anyone can call this once the TTL has lapsed to release funds to beneficiary.
    pub fn trigger_release(env: Env, vault_id: u64) {
        let mut vault: Vault = Self::load_vault(&env, vault_id);

        assert!(
            vault.status == ReleaseStatus::Locked,
            "vault already released"
        );
        assert!(Self::is_expired(&env, vault_id), "vault not yet expired");

        if vault.balance > 0 {
            let xlm = token::Client::new(&env, &env.current_contract_address());
            xlm.transfer(
                &env.current_contract_address(),
                &vault.beneficiary,
                &vault.balance,
            );
        }

        vault.balance = 0;
        vault.status = ReleaseStatus::Released;
        env.storage()
            .persistent()
            .set(&DataKey::Vault(vault_id), &vault);
    }

    /// Returns true if the check-in window has passed.
    pub fn is_expired(env: &Env, vault_id: u64) -> bool {
        let vault: Vault = Self::load_vault(env, vault_id);
        let now = env.ledger().timestamp();
        now > vault.last_check_in + vault.check_in_interval
    }

    pub fn get_vault(env: Env, vault_id: u64) -> Vault {
        Self::load_vault(&env, vault_id)
    }

    pub fn get_ttl_remaining(env: Env, vault_id: u64) -> u64 {
        let vault: Vault = Self::load_vault(&env, vault_id);
        let deadline = vault.last_check_in + vault.check_in_interval;
        let now = env.ledger().timestamp();
        if now >= deadline {
            0
        } else {
            deadline - now
        }
    }

    pub fn update_beneficiary(env: Env, vault_id: u64, new_beneficiary: Address) {
        let mut vault: Vault = Self::load_vault(&env, vault_id);
        vault.owner.require_auth();
        vault.beneficiary = new_beneficiary;
        env.storage()
            .persistent()
            .set(&DataKey::Vault(vault_id), &vault);
    }

    // --- helpers ---

    fn load_vault(env: &Env, vault_id: u64) -> Vault {
        env.storage()
            .persistent()
            .get(&DataKey::Vault(vault_id))
            .expect("vault not found")
    }
}
