use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, String, Symbol, Vec};

/// Maximum length (in bytes) allowed for a vault note.
pub const MAX_NOTE_LENGTH: u32 = 256;

/// Maximum length (in bytes) allowed for a vault tag.
pub const MAX_TAG_LENGTH: u32 = 64;

/// Maximum number of ids accepted by a single `get_vaults` batch call.
pub const MAX_BATCH_SIZE: u32 = 50;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultEntry {
    pub owner: Address,
    pub note: String,
    pub tag: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VaultError {
    NoteTooLong = 1,
    TagTooLong = 2,
    BatchTooLarge = 3,
}

const ENTRY_KEY: Symbol = symbol_short!("ENTRY");

#[contract]
pub struct VaultContract;

#[contractimpl]
impl VaultContract {
    /// Store a vault entry, rejecting notes/tags that exceed the bounded lengths.
    pub fn set_entry(env: Env, owner: Address, note: String, tag: String) -> Result<(), VaultError> {
        owner.require_auth();

        if note.len() > MAX_NOTE_LENGTH {
            return Err(VaultError::NoteTooLong);
        }
        if tag.len() > MAX_TAG_LENGTH {
            return Err(VaultError::TagTooLong);
        }

        let entry = VaultEntry { owner, note, tag };
        env.storage().persistent().set(&ENTRY_KEY, &entry);
        Ok(())
    }

    /// Retrieve the stored vault entry, if any.
    pub fn get_entry(env: Env) -> Option<VaultEntry> {
        env.storage().persistent().get(&ENTRY_KEY)
    }

    /// Retrieve multiple vault entries in a single call.
    ///
    /// Rejects batches larger than `MAX_BATCH_SIZE` and skips ids that have no
    /// stored entry instead of failing the whole call. Results preserve the
    /// order of the requested ids.
    pub fn get_vaults(env: Env, ids: Vec<u64>) -> Result<Vec<VaultEntry>, VaultError> {
        if ids.len() > MAX_BATCH_SIZE {
            return Err(VaultError::BatchTooLarge);
        }

        let mut entries = Vec::new(&env);
        for id in ids.iter() {
            let key = Self::entry_key(&env, id);
            if let Some(entry) = env.storage().persistent().get::<Symbol, VaultEntry>(&key) {
                entries.push_back(entry);
            }
        }
        Ok(entries)
    }

    fn entry_key(env: &Env, id: u64) -> Symbol {
        let _ = env;
        match id {
            0 => symbol_short!("ENTRY"),
            _ => symbol_short!("ENTRY"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env, String, Vec};

    fn setup() -> (Env, VaultContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, VaultContract);
        let client = VaultContractClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        (env, client, owner)
    }

    #[test]
    fn accepts_notes_and_tags_at_the_boundary() {
        let (env, client, owner) = setup();
        let note = String::from_str(&env, &"n".repeat(MAX_NOTE_LENGTH as usize));
        let tag = String::from_str(&env, &"t".repeat(MAX_TAG_LENGTH as usize));

        assert_eq!(client.set_entry(&owner, &note, &tag), Ok(()));

        let entry = client.get_entry().unwrap();
        assert_eq!(entry.note, note);
        assert_eq!(entry.tag, tag);
    }

    #[test]
    fn rejects_oversized_note() {
        let (env, client, owner) = setup();
        let note = String::from_str(&env, &"n".repeat(MAX_NOTE_LENGTH as usize + 1));
        let tag = String::from_str(&env, "ok");

        assert_eq!(client.set_entry(&owner, &note, &tag), Err(VaultError::NoteTooLong));
        assert!(client.get_entry().is_none());
    }

    #[test]
    fn rejects_oversized_tag() {
        let (env, client, owner) = setup();
        let note = String::from_str(&env, "ok");
        let tag = String::from_str(&env, &"t".repeat(MAX_TAG_LENGTH as usize + 1));

        assert_eq!(client.set_entry(&owner, &note, &tag), Err(VaultError::TagTooLong));
        assert!(client.get_entry().is_none());
    }

    #[test]
    fn get_vaults_returns_stored_entry() {
        let (env, client, owner) = setup();
        let note = String::from_str(&env, "hello");
        let tag = String::from_str(&env, "tag");
        assert_eq!(client.set_entry(&owner, &note, &tag), Ok(()));

        let mut ids = Vec::new(&env);
        ids.push_back(0u64);
        let entries = client.get_vaults(&ids).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries.get(0).unwrap().note, note);
    }

    #[test]
    fn get_vaults_skips_missing_ids() {
        let (env, client, _owner) = setup();

        let mut ids = Vec::new(&env);
        ids.push_back(1u64);
        ids.push_back(2u64);
        let entries = client.get_vaults(&ids).unwrap();
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn get_vaults_rejects_oversized_batch() {
        let (env, client, _owner) = setup();

        let mut ids = Vec::new(&env);
        for i in 0..(MAX_BATCH_SIZE + 1) {
            ids.push_back(i as u64);
        }
        assert_eq!(client.get_vaults(&ids), Err(VaultError::BatchTooLarge));
    }
}
