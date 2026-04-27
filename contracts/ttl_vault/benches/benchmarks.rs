use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    Address, Env,
};

// Import the contract (adjust path as needed)
// use ttl_vault::TtlVaultContract;

fn setup_env() -> (Env, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let admin = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();

    StellarAssetClient::new(&env, &token_address).mint(&owner, &10_000_000);

    (env, owner, beneficiary, admin, token_address)
}

fn bench_vault_creation(c: &mut Criterion) {
    c.bench_function("vault_creation", |b| {
        b.iter(|| {
            let (env, owner, beneficiary, admin, token_address) = setup_env();
            // Benchmark vault creation
            // client.create_vault(&owner, &beneficiary, &1000u64, &None);
        });
    });
}

fn bench_check_in(c: &mut Criterion) {
    c.bench_function("check_in", |b| {
        b.iter(|| {
            let (env, owner, beneficiary, admin, token_address) = setup_env();
            // Benchmark check-in operation
            // let vault_id = client.create_vault(&owner, &beneficiary, &1000u64, &None);
            // client.check_in(&vault_id);
        });
    });
}

fn bench_deposit(c: &mut Criterion) {
    let mut group = c.benchmark_group("deposit");
    
    for amount in [1_000i128, 100_000i128, 1_000_000i128].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(amount),
            amount,
            |b, &amount| {
                b.iter(|| {
                    let (env, owner, beneficiary, admin, token_address) = setup_env();
                    // Benchmark deposit with varying amounts
                    // let vault_id = client.create_vault(&owner, &beneficiary, &1000u64, &None);
                    // client.deposit(&vault_id, &amount);
                });
            },
        );
    }
    group.finish();
}

fn bench_withdrawal(c: &mut Criterion) {
    let mut group = c.benchmark_group("withdrawal");
    
    for amount in [1_000i128, 100_000i128, 1_000_000i128].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(amount),
            amount,
            |b, &amount| {
                b.iter(|| {
                    let (env, owner, beneficiary, admin, token_address) = setup_env();
                    // Benchmark withdrawal with varying amounts
                    // let vault_id = client.create_vault(&owner, &beneficiary, &1000u64, &None);
                    // client.deposit(&vault_id, &amount);
                    // client.withdraw(&vault_id, &(amount / 2));
                });
            },
        );
    }
    group.finish();
}

fn bench_release(c: &mut Criterion) {
    c.bench_function("release_trigger", |b| {
        b.iter(|| {
            let (env, owner, beneficiary, admin, token_address) = setup_env();
            // Benchmark release trigger after TTL expiry
            // let vault_id = client.create_vault(&owner, &beneficiary, &100u64, &None);
            // client.deposit(&vault_id, &100_000i128);
            // env.ledger().set_sequence_number(env.ledger().sequence() + 200);
            // client.trigger_release(&vault_id);
        });
    });
}

criterion_group!(
    benches,
    bench_vault_creation,
    bench_check_in,
    bench_deposit,
    bench_withdrawal,
    bench_release
);
criterion_main!(benches);
