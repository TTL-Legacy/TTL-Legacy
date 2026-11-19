use soroban_sdk::{contracttype, symbol_short, Address, Env, Map, Vec};

const FULL_ALLOCATION_BPS: u32 = 10_000;

pub const RANKING_SET_TOPIC: soroban_sdk::Symbol = symbol_short!("rank_set");
pub const DISTRIBUTED_BY_RANK_TOPIC: soroban_sdk::Symbol = symbol_short!("dist_rank");

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RankedBeneficiary {
    pub address: Address,
    pub priority: u32,
    pub allocation_bps: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct RankingSetEvent {
    pub beneficiary: Address,
    pub priority: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct DistributedByRankEvent {
    pub beneficiary: Address,
    pub priority: u32,
    pub amount: i128,
}

#[contracttype]
#[derive(Clone)]
pub enum RankingKey {
    BeneficiaryRanks,
}

pub fn set_rank(env: &Env, caller: &Address, beneficiary: Address, priority: u32) {
    caller.require_auth();

    let mut ranks: Map<Address, RankedBeneficiary> = env
        .storage()
        .persistent()
        .get(&RankingKey::BeneficiaryRanks)
        .unwrap_or_else(|| Map::new(env));

    let allocation_bps = ranks
        .get(beneficiary.clone())
        .map(|b| b.allocation_bps)
        .unwrap_or(FULL_ALLOCATION_BPS);

    ranks.set(
        beneficiary.clone(),
        RankedBeneficiary {
            address: beneficiary.clone(),
            priority,
            allocation_bps,
        },
    );
    env.storage()
        .persistent()
        .set(&RankingKey::BeneficiaryRanks, &ranks);

    env.events().publish(
        (RANKING_SET_TOPIC,),
        RankingSetEvent {
            beneficiary,
            priority,
        },
    );
}

pub fn distribute_by_rank(env: &Env, total_amount: i128) -> Map<Address, i128> {
    let ranks: Map<Address, RankedBeneficiary> = env
        .storage()
        .persistent()
        .get(&RankingKey::BeneficiaryRanks)
        .unwrap_or_else(|| Map::new(env));

    let mut ordered = Vec::new(env);
    for (_, beneficiary) in ranks.iter() {
        ordered.push_back(beneficiary);
    }

    let len = ordered.len();
    for i in 0..len {
        for j in 0..len.saturating_sub(i + 1) {
            if ordered.get(j).unwrap().priority > ordered.get(j + 1).unwrap().priority {
                let left = ordered.get(j).unwrap();
                let right = ordered.get(j + 1).unwrap();
                ordered.set(j, right);
                ordered.set(j + 1, left);
            }
        }
    }

    let mut distributions = Map::new(env);
    let mut remaining = total_amount;
    let mut i = 0;

    while i < ordered.len() && remaining > 0 {
        let priority = ordered.get(i).unwrap().priority;
        let mut tier = Vec::new(env);
        let mut j = i;
        while j < ordered.len() && ordered.get(j).unwrap().priority == priority {
            tier.push_back(ordered.get(j).unwrap());
            j += 1;
        }

        let mut total_bps = 0u32;
        for beneficiary in tier.iter() {
            total_bps = total_bps.saturating_add(beneficiary.allocation_bps);
        }

        for beneficiary in tier.iter() {
            if remaining <= 0 {
                break;
            }
            let amount = if total_bps == 0 {
                0
            } else {
                (remaining * beneficiary.allocation_bps as i128) / total_bps as i128
            };
            let amount = amount.min(remaining);
            distributions.set(beneficiary.address.clone(), amount);
            remaining -= amount;
            env.events().publish(
                (DISTRIBUTED_BY_RANK_TOPIC,),
                DistributedByRankEvent {
                    beneficiary: beneficiary.address,
                    priority: beneficiary.priority,
                    amount,
                },
            );
        }

        i = j;
    }

    distributions
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Events},
        Address, Env, IntoVal, TryIntoVal, Val,
    };

    #[test]
    fn higher_priority_receives_before_lower_priority() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);

        set_rank(&env, &admin, alice.clone(), 1);
        set_rank(&env, &admin, bob.clone(), 2);

        let result = distribute_by_rank(&env, 500);

        assert_eq!(result.get(alice).unwrap(), 500);
        assert_eq!(result.get(bob).unwrap_or(0), 0);
    }

    #[test]
    fn same_priority_splits_by_allocation_bps() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);

        set_rank(&env, &admin, alice.clone(), 1);
        set_rank(&env, &admin, bob.clone(), 1);

        let result = distribute_by_rank(&env, 1_000);

        assert_eq!(result.get(alice).unwrap(), 500);
        assert_eq!(result.get(bob).unwrap(), 500);
    }

    #[test]
    fn ranking_set_event_is_emitted() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let alice = Address::generate(&env);

        set_rank(&env, &admin, alice, 1);

        assert!(env.events().all().iter().any(|event| {
            let topics: Vec<Val> = event.1.clone().into_val(&env);
            topics
                .get(0)
                .and_then(|topic| topic.try_into_val(&env).ok())
                .map(|topic: soroban_sdk::Symbol| topic == RANKING_SET_TOPIC)
                .unwrap_or(false)
        }));
    }

    #[test]
    fn set_rank_preserves_allocation_bps_on_update() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let alice = Address::generate(&env);

        set_rank(&env, &admin, alice.clone(), 1);

        let mut ranks: Map<Address, RankedBeneficiary> = env
            .storage()
            .persistent()
            .get(&RankingKey::BeneficiaryRanks)
            .unwrap();
        let mut entry = ranks.get(alice.clone()).unwrap();
        assert_eq!(entry.allocation_bps, FULL_ALLOCATION_BPS);
        entry.allocation_bps = 2_500;
        ranks.set(alice.clone(), entry);
        env.storage()
            .persistent()
            .set(&RankingKey::BeneficiaryRanks, &ranks);

        set_rank(&env, &admin, alice.clone(), 3);

        let ranks: Map<Address, RankedBeneficiary> = env
            .storage()
            .persistent()
            .get(&RankingKey::BeneficiaryRanks)
            .unwrap();
        let entry = ranks.get(alice).unwrap();
        assert_eq!(entry.priority, 3);
        assert_eq!(entry.allocation_bps, 2_500);
    }

    #[test]
    fn set_rank_event_payload_matches() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let alice = Address::generate(&env);

        set_rank(&env, &admin, alice.clone(), 7);

        let mut found = false;
        for event in env.events().all().iter() {
            let topics: Vec<Val> = event.1.clone().into_val(&env);
            let is_rank_set = topics
                .get(0)
                .and_then(|topic| topic.try_into_val(&env).ok())
                .map(|topic: soroban_sdk::Symbol| topic == RANKING_SET_TOPIC)
                .unwrap_or(false);
            if is_rank_set {
                let payload: RankingSetEvent = event.2.clone().into_val(&env);
                assert_eq!(payload.beneficiary, alice);
                assert_eq!(payload.priority, 7);
                found = true;
            }
        }
        assert!(found);
    }

    #[test]
    fn empty_ranking_map_distributes_nothing() {
        let env = Env::default();
        let result = distribute_by_rank(&env, 1_000);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn zero_total_bps_tier_yields_zero_amounts() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);

        set_rank(&env, &admin, alice.clone(), 1);
        set_rank(&env, &admin, bob.clone(), 1);

        let mut ranks: Map<Address, RankedBeneficiary> = env
            .storage()
            .persistent()
            .get(&RankingKey::BeneficiaryRanks)
            .unwrap();
        for addr in [alice.clone(), bob.clone()].iter() {
            let mut entry = ranks.get(addr.clone()).unwrap();
            entry.allocation_bps = 0;
            ranks.set(addr.clone(), entry);
        }
        env.storage()
            .persistent()
            .set(&RankingKey::BeneficiaryRanks, &ranks);

        let result = distribute_by_rank(&env, 1_000);

        assert_eq!(result.get(alice).unwrap(), 0);
        assert_eq!(result.get(bob).unwrap(), 0);
    }

    #[test]
    fn remaining_exhausted_mid_tier() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        let carol = Address::generate(&env);

        set_rank(&env, &admin, alice.clone(), 1);
        set_rank(&env, &admin, bob.clone(), 1);
        set_rank(&env, &admin, carol.clone(), 2);

        let result = distribute_by_rank(&env, 1_000);

        assert_eq!(result.get(alice).unwrap(), 500);
        assert_eq!(result.get(bob).unwrap(), 500);
        assert_eq!(result.get(carol).unwrap_or(0), 0);
    }

    #[test]
    fn distribute_by_rank_event_payloads_match() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);

        set_rank(&env, &admin, alice.clone(), 1);
        set_rank(&env, &admin, bob.clone(), 2);

        let result = distribute_by_rank(&env, 400);

        let mut seen = 0;
        for event in env.events().all().iter() {
            let topics: Vec<Val> = event.1.clone().into_val(&env);
            let is_dist = topics
                .get(0)
                .and_then(|topic| topic.try_into_val(&env).ok())
                .map(|topic: soroban_sdk::Symbol| topic == DISTRIBUTED_BY_RANK_TOPIC)
                .unwrap_or(false);
            if is_dist {
                let payload: DistributedByRankEvent = event.2.clone().into_val(&env);
                assert_eq!(payload.beneficiary, alice);
                assert_eq!(payload.priority, 1);
                assert_eq!(payload.amount, 400);
                seen += 1;
            }
        }
        assert_eq!(seen, 1);
        assert_eq!(result.get(alice).unwrap(), 400);
        assert_eq!(result.get(bob).unwrap_or(0), 0);
    }
}
