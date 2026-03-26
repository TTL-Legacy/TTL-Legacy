use soroban_sdk::{contract, contractimpl, symbol_short, token, Address, Env};

mod test;

mod types;
use types::{DataKey, ReleaseEvent, ReleaseStatus, Vault, RELEASE_TOPIC};

#[contract]
pub struct TtlVaultContract;

#[contractimpl]
impl TtlVaultContract {
    /// One-time initializer — stores the token address used for all transfers.
    pub fn initialize(env: Env, token_address: Address) {
        assert!(
            !env.storage().instance().has(&DataKey::TokenAddress),
            "already initialized"
        );
        env.storage()
            .instance()
            .set(&DataKey::TokenAddress, &token_address);
    }
    /// Create a new vault. Returns the vault ID.
    pub fn create_vault(
        env: Env,
        owner: Address,
        beneficiary: Address,
        check_in_interval: u64,
    ) -> u64 {
        owner.require_auth();

        if check_in_interval == 0 {
            panic_with_error!(&env, ContractError::InvalidInterval);
        }

        let vault_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::VaultCount)
            .unwrap_or(0u64)
            + 1;

        let vault = Vault {
            owner: owner.clone(),
            beneficiary: beneficiary.clone(),
            balance: 0,
            check_in_interval,
            last_check_in: env.ledger().timestamp(),
            status: ReleaseStatus::Locked,
        };

        Self::save_vault(&env, vault_id, &vault);
        env.storage()
            .instance()
            .set(&DataKey::VaultCount, &vault_id);
        env.storage()
            .instance()
            .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_LEDGERS);

        // Emit vault creation event
        let event = VaultCreatedEvent::new(vault_id, owner, beneficiary, check_in_interval);
        env.events().publish((event.topic(),), event.to_tuple());

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

        env.events().publish(
            (symbol_short!("check_in"), vault_id),
            vault.last_check_in,
        );
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

        let xlm = token::Client::new(&env, &Self::load_token(&env));
        xlm.transfer(&from, &env.current_contract_address(), &amount);

        vault.balance += amount;
        Self::save_vault(&env, vault_id, &vault);
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

        let xlm = token::Client::new(&env, &Self::load_token(&env));
        xlm.transfer(&env.current_contract_address(), &vault.owner, &amount);

        vault.balance -= amount;
        Self::save_vault(&env, vault_id, &vault);
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

        if vault.balance == 0 {
            panic_with_error!(&env, ContractError::EmptyVault);
        }

        let released_amount = vault.balance;

        vault.balance = 0;
        vault.status = ReleaseStatus::Released;
        env.storage()
            .persistent()
            .set(&DataKey::Vault(vault_id), &vault);

        env.events().publish(
            (RELEASE_TOPIC,),
            ReleaseEvent {
                vault_id,
                beneficiary: vault.beneficiary,
                amount: released_amount,
            },
        );
    }

    /// Returns true if the check-in window has passed.
    pub fn is_expired(env: &Env, vault_id: u64) -> bool {
        let vault: Vault = Self::load_vault(env, vault_id);
        let now = env.ledger().timestamp();
        now >= vault.last_check_in + vault.check_in_interval
    }

    pub fn get_vault(env: Env, vault_id: u64) -> Vault {
        Self::load_vault(&env, vault_id)
    }

    pub fn get_ttl_remaining(env: Env, vault_id: u64) -> Option<u64> {
        let vault: Vault = match env.storage()
            .persistent()
            .get(&DataKey::Vault(vault_id)) {
            Some(v) => v,
            None => return None,
        };
        let deadline = vault.last_check_in + vault.check_in_interval;
        let now = env.ledger().timestamp();
        if now >= deadline {
            Some(0)
        } else {
            Some(deadline - now)
        }
    }

    pub fn update_beneficiary(env: Env, vault_id: u64, new_beneficiary: Address) {
        let mut vault: Vault = Self::load_vault(&env, vault_id);
        vault.owner.require_auth();

        assert!(
            vault.status == ReleaseStatus::Locked,
            "vault already released"
        );

        vault.beneficiary = new_beneficiary;
        Self::save_vault(&env, vault_id, &vault);
    }

    // --- helpers ---

    fn load_token(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::TokenAddress)
            .expect("not initialized")
    }

    fn load_vault(env: &Env, vault_id: u64) -> Vault {
        env.storage()
            .persistent()
            .get(&DataKey::Vault(vault_id))
            .unwrap_or_else(|| panic_with_error!(env, ContractError::VaultNotFound))
    }

    /// Persist a vault and extend its TTL so it is never silently archived.
    fn save_vault(env: &Env, vault_id: u64, vault: &Vault) {
        let key = DataKey::Vault(vault_id);
        env.storage().persistent().set(&key, vault);
        env.storage()
            .persistent()
            .extend_ttl(&key, VAULT_TTL_THRESHOLD, VAULT_TTL_LEDGERS);
    }
}
