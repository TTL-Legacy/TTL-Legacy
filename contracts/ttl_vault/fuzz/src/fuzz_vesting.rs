#![no_main]

use libfuzzer_sys::fuzz_target;

/// Mirrors `VestingSchedule` fields used in `compute_vested_amount`.
/// All arithmetic is reproduced here so the fuzz target is self-contained
/// and runnable without the Soroban host (no `no_std` / wasm dependency).
#[derive(Debug)]
struct VestingSchedule {
    start_time: u64,
    interval: u64,
    num_installments: u32,
    total_amount: i128,
    cliff_period: u64,
}

/// Reproduces the linear vesting interpolation from `claim_vested_installment`.
///
/// Returns the number of installments that would be claimable at `current_time`,
/// capped to `num_installments`.  The invariant under test is:
///
///   vested_amount ∈ [0, total_amount]
fn compute_vested_amount(schedule: &VestingSchedule, current_time: u64) -> i128 {
    if schedule.num_installments == 0 || schedule.total_amount <= 0 {
        return 0;
    }
    if current_time < schedule.start_time {
        return 0;
    }
    // Cliff: nothing claimable until start_time + cliff_period
    if schedule.cliff_period > 0 && current_time < schedule.start_time.saturating_add(schedule.cliff_period) {
        return 0;
    }
    let interval = if schedule.interval == 0 { 1 } else { schedule.interval };
    let elapsed = current_time.saturating_sub(schedule.start_time);
    let unlocked = ((elapsed / interval) + 1).min(schedule.num_installments as u64) as u32;

    let per_installment = schedule.total_amount / schedule.num_installments as i128;
    if unlocked >= schedule.num_installments {
        schedule.total_amount
    } else {
        per_installment.saturating_mul(unlocked as i128)
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 41 {
        return;
    }

    // Decode raw bytes into schedule fields
    let start_time = u64::from_le_bytes(data[0..8].try_into().unwrap());
    let interval = u64::from_le_bytes(data[8..16].try_into().unwrap());
    let num_installments = u32::from_le_bytes(data[16..20].try_into().unwrap());
    let total_amount = i128::from_le_bytes(data[20..36].try_into().unwrap());
    let cliff_period = u32::from_le_bytes(data[36..40].try_into().unwrap()) as u64;
    let current_time_offset = data[40] as u64;

    let schedule = VestingSchedule {
        start_time,
        interval,
        num_installments,
        total_amount,
        cliff_period,
    };

    let current_time = start_time.wrapping_add(current_time_offset);
    let result = compute_vested_amount(&schedule, current_time);

    // Invariant: result must be in [0, total_amount]
    let lo = 0i128;
    let hi = total_amount.max(0);
    assert!(
        result >= lo && result <= hi,
        "invariant violated: result={result} not in [0, {hi}] for schedule={schedule:?}, current_time={current_time}"
    );
});
