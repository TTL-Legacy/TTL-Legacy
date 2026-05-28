use soroban_sdk::{contracttype, symbol_short, Address, Env, Map};

pub const MATCH_SET_TOPIC: soroban_sdk::Symbol = symbol_short!("match_set");
pub const MATCHED_DISTRIBUTION_TOPIC: soroban_sdk::Symbol = symbol_short!("match_dst");

#[contracttype]
#[derive(Clone, Debug)]
pub struct MatchPair {
    pub source: Address,
    pub matched: Address,
    pub match_bps: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct MatchSetEvent {
    pub source: Address,
    pub matched: Address,
    pub match_bps: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct MatchedDistributionEvent {
    pub source: Address,
    pub matched: Address,
    pub source_amount: i128,
    pub matched_amount: i128,
}

#[contracttype]
#[derive(Clone)]
pub enum MatchingKey {
    Matches,
}

pub fn set_match(
    env: &Env,
    caller: &Address,
    source: Address,
    matched: Address,
    match_bps: u32,
) {
    caller.require_auth();
    if match_bps > 10_000 {
        panic!("match_bps cannot exceed 10000");
    }

    let mut matches: Map<Address, MatchPair> = env
        .storage()
        .persistent()
        .get(&MatchingKey::Matches)
        .unwrap_or_else(|| Map::new(env));

    matches.set(
        source.clone(),
        MatchPair {
            source: source.clone(),
            matched: matched.clone(),
            match_bps,
        },
    );
    env.storage().persistent().set(&MatchingKey::Matches, &matches);

    env.events().publish(
        (MATCH_SET_TOPIC,),
        MatchSetEvent {
            source,
            matched,
            match_bps,
        },
    );
}

pub fn compute_matched_amounts(
    env: &Env,
    base_distributions: &Map<Address, i128>,
) -> Map<Address, i128> {
    let matches: Map<Address, MatchPair> = env
        .storage()
        .persistent()
        .get(&MatchingKey::Matches)
        .unwrap_or_else(|| Map::new(env));

    let mut matched_amounts = Map::new(env);

    for (source_addr, pair) in matches.iter() {
        if let Some(source_amount) = base_distributions.get(source_addr.clone()) {
            let matched_amount = (source_amount * pair.match_bps as i128) / 10_000;
            if matched_amount > 0 {
                let existing = matched_amounts.get(pair.matched.clone()).unwrap_or(0);
                matched_amounts.set(pair.matched.clone(), existing + matched_amount);

                env.events().publish(
                    (MATCHED_DISTRIBUTION_TOPIC,),
                    MatchedDistributionEvent {
                        source: source_addr,
                        matched: pair.matched,
                        source_amount,
                        matched_amount,
                    },
                );
            }
        }
    }

    matched_amounts
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Events},
        Address, Env, IntoVal, TryIntoVal, Val,
    };

    fn has_topic(env: &Env, topic: soroban_sdk::Symbol) -> bool {
        env.events().all().iter().any(|event| {
            let topics: soroban_sdk::Vec<Val> = event.1.clone().into_val(env);
            topics
                .get(0)
                .and_then(|topic| topic.try_into_val(env).ok())
                .map(|actual: soroban_sdk::Symbol| actual == topic)
                .unwrap_or(false)
        })
    }

    #[test]
    fn computes_full_match() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let alice = Address::generate(&env);
        let charity = Address::generate(&env);

        set_match(&env, &admin, alice.clone(), charity.clone(), 10_000);

        let mut base = Map::new(&env);
        base.set(alice, 1_000);

        let matched = compute_matched_amounts(&env, &base);

        assert_eq!(matched.get(charity).unwrap(), 1_000);
    }

    #[test]
    fn computes_partial_match() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let alice = Address::generate(&env);
        let partner = Address::generate(&env);

        set_match(&env, &admin, alice.clone(), partner.clone(), 5_000);

        let mut base = Map::new(&env);
        base.set(alice, 2_000);

        let matched = compute_matched_amounts(&env, &base);

        assert_eq!(matched.get(partner).unwrap(), 1_000);
    }

    #[test]
    fn missing_source_produces_no_match() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let alice = Address::generate(&env);
        let partner = Address::generate(&env);
        let bob = Address::generate(&env);

        set_match(&env, &admin, alice, partner.clone(), 10_000);

        let mut base = Map::new(&env);
        base.set(bob, 1_000);

        let matched = compute_matched_amounts(&env, &base);

        assert_eq!(matched.get(partner).unwrap_or(0), 0);
    }

    #[test]
    #[should_panic(expected = "match_bps cannot exceed 10000")]
    fn rejects_match_above_100_percent() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let alice = Address::generate(&env);
        let partner = Address::generate(&env);

        set_match(&env, &admin, alice, partner, 10_001);
    }

    #[test]
    fn emits_match_set_event() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let alice = Address::generate(&env);
        let partner = Address::generate(&env);

        set_match(&env, &admin, alice, partner, 10_000);

        assert!(has_topic(&env, MATCH_SET_TOPIC));
    }
}
