use soroban_sdk::{contracttype, symbol_short, Address, Env, Map};

pub const FLOOR_SET_TOPIC: soroban_sdk::Symbol = symbol_short!("floor_set");
pub const FLOOR_ENFORCED_TOPIC: soroban_sdk::Symbol = symbol_short!("floor_enf");

#[contracttype]
#[derive(Clone, Debug)]
pub struct FloorSetEvent {
    pub beneficiary: Address,
    pub floor: i128,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct FloorEnforcedEvent {
    pub beneficiary: Address,
    pub original: i128,
    pub floor: i128,
    pub shortfall: i128,
}

#[contracttype]
#[derive(Clone)]
pub enum FloorsKey {
    BeneficiaryFloors,
}

pub fn set_floor(env: &Env, caller: &Address, beneficiary: Address, floor: i128) {
    caller.require_auth();
    if floor <= 0 {
        panic!("floor must be positive");
    }

    let mut floors: Map<Address, i128> = env
        .storage()
        .persistent()
        .get(&FloorsKey::BeneficiaryFloors)
        .unwrap_or_else(|| Map::new(env));

    floors.set(beneficiary.clone(), floor);
    env.storage()
        .persistent()
        .set(&FloorsKey::BeneficiaryFloors, &floors);

    env.events()
        .publish((FLOOR_SET_TOPIC,), FloorSetEvent { beneficiary, floor });
}

pub fn apply_floors(
    env: &Env,
    distributions: Map<Address, i128>,
    total_available: i128,
) -> (Map<Address, i128>, i128) {
    let floors: Map<Address, i128> = env
        .storage()
        .persistent()
        .get(&FloorsKey::BeneficiaryFloors)
        .unwrap_or_else(|| Map::new(env));

    let mut result = Map::new(env);
    let mut remaining = total_available.max(0);
    let mut total_shortfall = 0i128;

    for (addr, original) in distributions.iter() {
        let floor = floors.get(addr.clone()).unwrap_or(0);
        let target = original.max(floor);
        let actual = target.min(remaining);
        let shortfall = if floor > 0 && actual < floor {
            floor - actual
        } else {
            0
        };

        result.set(addr.clone(), actual);
        remaining -= actual;
        total_shortfall += shortfall;

        if floor > 0 && (original < floor || shortfall > 0) {
            env.events().publish(
                (FLOOR_ENFORCED_TOPIC,),
                FloorEnforcedEvent {
                    beneficiary: addr,
                    original,
                    floor,
                    shortfall,
                },
            );
        }
    }

    (result, total_shortfall)
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
    fn amount_above_floor_is_unchanged() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let alice = Address::generate(&env);

        set_floor(&env, &admin, alice.clone(), 200);

        let mut dist = Map::new(&env);
        dist.set(alice.clone(), 500);

        let (result, shortfall) = apply_floors(&env, dist, 1_000);

        assert_eq!(result.get(alice).unwrap(), 500);
        assert_eq!(shortfall, 0);
    }

    #[test]
    fn amount_below_floor_is_raised_when_funded() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let alice = Address::generate(&env);

        set_floor(&env, &admin, alice.clone(), 300);

        let mut dist = Map::new(&env);
        dist.set(alice.clone(), 100);

        let (result, shortfall) = apply_floors(&env, dist, 1_000);

        assert_eq!(result.get(alice).unwrap(), 300);
        assert_eq!(shortfall, 0);
    }

    #[test]
    fn insufficient_estate_reports_shortfall() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let alice = Address::generate(&env);

        set_floor(&env, &admin, alice.clone(), 500);

        let mut dist = Map::new(&env);
        dist.set(alice.clone(), 100);

        let (result, shortfall) = apply_floors(&env, dist, 200);

        assert_eq!(result.get(alice).unwrap(), 200);
        assert_eq!(shortfall, 300);
    }

    #[test]
    fn no_floor_passes_original_amount() {
        let env = Env::default();
        let alice = Address::generate(&env);

        let mut dist = Map::new(&env);
        dist.set(alice.clone(), 750);

        let (result, shortfall) = apply_floors(&env, dist, 1_000);

        assert_eq!(result.get(alice).unwrap(), 750);
        assert_eq!(shortfall, 0);
    }

    #[test]
    fn floor_enforced_event_is_emitted() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let alice = Address::generate(&env);

        set_floor(&env, &admin, alice.clone(), 400);

        let mut dist = Map::new(&env);
        dist.set(alice, 50);
        apply_floors(&env, dist, 50);

        assert!(has_topic(&env, FLOOR_ENFORCED_TOPIC));
    }

    #[test]
    #[should_panic(expected = "floor must be positive")]
    fn rejects_zero_floor() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let alice = Address::generate(&env);

        set_floor(&env, &admin, alice, 0);
    }
}
