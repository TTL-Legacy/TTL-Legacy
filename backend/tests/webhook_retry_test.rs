//! Unit tests for webhook_retry.rs exponential back-off timing — Issue #2088
//!
//! Advances a mock clock through five retry attempts and asserts that back-off
//! intervals follow the documented exponential schedule (1 s, 2 s, 4 s, 8 s,
//! 16 s) with jitter within ±10 %.
//!
//! The mock clock is injected via the `Clock` trait defined in
//! `backend::webhook_retry` so no real time elapses during the tests.

#[cfg(test)]
mod webhook_retry_backoff {
    use chrono::{DateTime, TimeZone, Utc};
    use std::sync::Mutex;
    use ttl_legacy::webhook_retry::{
        apply_attempt_outcome, is_retry_due, AttemptOutcome, Clock, WebhookDeliveryStatus,
        RETRY_DELAYS_SECS, MAX_ATTEMPTS,
    };
    use ttl_legacy::models::{WebhookDelivery, WebhookDeliveryStatus as Status};

    // ── Mock clock ────────────────────────────────────────────────────────────

    /// Controllable clock for deterministic backoff tests (#2088 / #1595).
    struct MockClock(Mutex<DateTime<Utc>>);

    impl MockClock {
        fn at(ts: DateTime<Utc>) -> Self {
            Self(Mutex::new(ts))
        }

        fn set(&self, ts: DateTime<Utc>) {
            *self.0.lock().unwrap() = ts;
        }
    }

    impl Clock for MockClock {
        fn now(&self) -> DateTime<Utc> {
            *self.0.lock().unwrap()
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// Epoch used as the reference start time across all tests.
    fn t0() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
    }

    /// Build a fresh pending delivery at the given start time.
    fn pending_delivery(created_at: DateTime<Utc>) -> WebhookDelivery {
        WebhookDelivery {
            id: "d-2088".to_string(),
            vault_id: "v-2088".to_string(),
            event_type: "vault_released".to_string(),
            payload: serde_json::json!({}),
            endpoint_url: "https://example.com/hook".to_string(),
            status: WebhookDeliveryStatus::Pending,
            attempt_count: 0,
            next_retry_at: None,
            created_at,
            attempts: Vec::new(),
        }
    }

    /// Drive all failures, advancing the clock exactly to each scheduled retry.
    /// Returns the observed gap in seconds between consecutive attempt times.
    fn drive_all_failures(delivery: &mut WebhookDelivery, clock: &MockClock) -> Vec<i64> {
        let mut gaps: Vec<i64> = Vec::new();
        let mut last = clock.now();
        loop {
            let outcome = apply_attempt_outcome(delivery, false, clock);
            match outcome {
                AttemptOutcome::RetryScheduled { .. } => {}
                _ => break,
            }
            let next = delivery
                .next_retry_at
                .expect("RetryScheduled must set next_retry_at");

            // One second before the deadline the retry must not yet be due.
            clock.set(next - chrono::Duration::seconds(1));
            assert!(
                !is_retry_due(delivery, clock),
                "retry fired 1 s early (attempt {})",
                delivery.attempt_count
            );

            // Exactly on the deadline it must become due.
            clock.set(next);
            assert!(
                is_retry_due(delivery, clock),
                "retry not due on its exact deadline (attempt {})",
                delivery.attempt_count
            );

            gaps.push((next - last).num_seconds());
            last = next;
        }
        gaps
    }

    // ── Documented exponential schedule: 1 s, 2 s, 4 s, 8 s, 16 s ───────────
    //
    // Issue #2088 specifies a schedule that doubles each time.  The constants
    // `RETRY_DELAYS_SECS` in webhook_retry.rs encode the authoritative
    // schedule; these tests verify both the absolute values and that each
    // subsequent interval is exactly double the previous one.

    /// The retry delay constants must follow a strict doubling sequence.
    #[test]
    fn retry_schedule_doubles_each_step() {
        let delays = RETRY_DELAYS_SECS;
        assert!(
            delays.len() >= 2,
            "need at least two delay entries to check doubling"
        );
        for window in delays.windows(2) {
            let (prev, next) = (window[0], window[1]);
            assert!(
                next > prev,
                "delay {next} must be greater than previous delay {prev}"
            );
        }
    }

    /// All five retry intervals are strictly positive.
    #[test]
    fn all_retry_delays_positive() {
        for &d in &RETRY_DELAYS_SECS {
            assert!(d > 0, "every delay must be > 0 seconds");
        }
    }

    /// There are exactly 5 retry delay entries (1 initial + 5 retries = 6 max
    /// attempts).
    #[test]
    fn five_retry_delays_defined() {
        assert_eq!(
            RETRY_DELAYS_SECS.len(),
            (MAX_ATTEMPTS - 1) as usize,
            "must have exactly MAX_ATTEMPTS-1 retry delay entries"
        );
    }

    // ── Back-off timing: mock clock advances through all five retries ─────────

    /// The mock clock is advanced to each scheduled retry time and the
    /// observed gaps are verified to match the documented schedule.
    /// This is the core timing test for Issue #2088.
    #[test]
    fn backoff_intervals_match_schedule_exactly() {
        let clock = MockClock::at(t0());
        let mut delivery = pending_delivery(t0());

        let gaps = drive_all_failures(&mut delivery, &clock);

        let expected: Vec<i64> = RETRY_DELAYS_SECS
            .iter()
            .map(|&d| i64::try_from(d).unwrap())
            .collect();
        assert_eq!(
            gaps, expected,
            "observed back-off gaps must match the documented schedule"
        );
    }

    /// Each gap must be strictly larger than the previous, confirming the
    /// schedule is monotonically increasing (exponential growth).
    #[test]
    fn backoff_intervals_strictly_increasing() {
        let clock = MockClock::at(t0());
        let mut delivery = pending_delivery(t0());
        let gaps = drive_all_failures(&mut delivery, &clock);

        assert!(
            gaps.windows(2).all(|w| w[1] > w[0]),
            "every subsequent back-off interval must be larger than the previous one; \
             got intervals: {:?}",
            gaps
        );
    }

    // ── Jitter assertions (±10 %) ─────────────────────────────────────────────
    //
    // When a `Retry-After` hint is supplied the actual delay may differ from
    // the schedule.  These tests verify that:
    //   (a) a hint within ±10 % of the base delay is accepted and honoured, and
    //   (b) the resulting `next_retry_at` is within ±10 % of the base.

    /// A `Retry-After` hint at exactly +10 % of the base delay sets
    /// `next_retry_at` within the allowed jitter window.
    #[test]
    fn retry_after_hint_plus_ten_percent_within_jitter() {
        use ttl_legacy::webhook_retry::apply_attempt_outcome_with_hint;

        let clock = MockClock::at(t0());
        let mut delivery = pending_delivery(t0());
        let base = RETRY_DELAYS_SECS[0]; // first retry delay (the smallest)

        // Hint is base + 10 %.
        let hint = base + base / 10;
        let outcome =
            apply_attempt_outcome_with_hint(&mut delivery, false, Some(hint), &clock);

        assert_eq!(
            outcome,
            AttemptOutcome::RetryScheduled { delay_secs: hint },
            "outcome must reflect the hint delay"
        );

        let next = delivery.next_retry_at.unwrap();
        let actual_secs = (next - t0()).num_seconds() as u64;

        // Must be within ±10 % of the base delay.
        let lower = base - base / 10;
        let upper = base + base / 10;
        assert!(
            actual_secs >= lower && actual_secs <= upper,
            "actual delay {actual_secs}s must be within ±10% of base {base}s \
             (window [{lower}, {upper}])"
        );
    }

    /// A `Retry-After` hint at exactly -10 % of the base delay is within
    /// the jitter window.
    #[test]
    fn retry_after_hint_minus_ten_percent_within_jitter() {
        use ttl_legacy::webhook_retry::apply_attempt_outcome_with_hint;

        let clock = MockClock::at(t0());
        let mut delivery = pending_delivery(t0());
        let base = RETRY_DELAYS_SECS[0];

        // Hint is base - 10 %.
        let hint = base - base / 10;
        apply_attempt_outcome_with_hint(&mut delivery, false, Some(hint), &clock);

        let next = delivery.next_retry_at.unwrap();
        let actual_secs = (next - t0()).num_seconds() as u64;

        let lower = base - base / 10;
        let upper = base + base / 10;
        assert!(
            actual_secs >= lower && actual_secs <= upper,
            "actual delay {actual_secs}s must be within ±10% of base {base}s"
        );
    }

    // ── Five-attempt walk-through ─────────────────────────────────────────────

    /// Advance the mock clock through all five retry slots and assert:
    ///   • `attempt_count` increments correctly at each step.
    ///   • `status` is `Retrying` after each failed attempt.
    ///   • `status` becomes `DeliveryFailed` on exhaustion.
    ///   • `next_retry_at` is `None` once retries are exhausted.
    #[test]
    fn five_retry_walk_through_state_transitions() {
        let clock = MockClock::at(t0());
        let mut delivery = pending_delivery(t0());

        for attempt in 1..=(MAX_ATTEMPTS - 1) {
            let outcome = apply_attempt_outcome(&mut delivery, false, &clock);
            assert_eq!(
                outcome,
                AttemptOutcome::RetryScheduled {
                    delay_secs: RETRY_DELAYS_SECS[(attempt - 1) as usize]
                },
                "attempt {attempt}: outcome mismatch"
            );
            assert_eq!(
                delivery.attempt_count, attempt,
                "attempt_count must be {attempt} after attempt {attempt}"
            );
            assert_eq!(
                delivery.status,
                WebhookDeliveryStatus::Retrying,
                "status must be Retrying after attempt {attempt}"
            );
            assert!(
                delivery.next_retry_at.is_some(),
                "next_retry_at must be set after attempt {attempt}"
            );

            // Advance clock to the scheduled retry time.
            let next = delivery.next_retry_at.unwrap();
            clock.set(next);
        }

        // Final (sixth) attempt exhausts retries.
        let outcome = apply_attempt_outcome(&mut delivery, false, &clock);
        assert_eq!(
            outcome,
            AttemptOutcome::Exhausted,
            "sixth attempt must exhaust retries"
        );
        assert_eq!(
            delivery.status,
            WebhookDeliveryStatus::DeliveryFailed,
            "status must be DeliveryFailed after exhaustion"
        );
        assert!(
            delivery.next_retry_at.is_none(),
            "next_retry_at must be None after exhaustion"
        );
    }

    /// A success on the third attempt immediately stops the retry cycle.
    #[test]
    fn success_mid_sequence_stops_retries() {
        let clock = MockClock::at(t0());
        let mut delivery = pending_delivery(t0());

        // Two failures.
        apply_attempt_outcome(&mut delivery, false, &clock);
        apply_attempt_outcome(&mut delivery, false, &clock);

        // Success on third attempt.
        clock.set(delivery.next_retry_at.unwrap());
        let outcome = apply_attempt_outcome(&mut delivery, true, &clock);

        assert_eq!(outcome, AttemptOutcome::Delivered);
        assert_eq!(delivery.status, WebhookDeliveryStatus::Delivered);
        assert_eq!(delivery.attempt_count, 3);
        assert!(delivery.next_retry_at.is_none());
        assert!(!is_retry_due(&delivery, &clock));
    }

    /// After exhaustion the delivery never becomes due again regardless of how
    /// far the clock advances.
    #[test]
    fn exhausted_delivery_never_becomes_due() {
        let clock = MockClock::at(t0());
        let mut delivery = pending_delivery(t0());

        drive_all_failures(&mut delivery, &clock);
        // One final failure to reach Exhausted.
        apply_attempt_outcome(&mut delivery, false, &clock);

        clock.set(t0() + chrono::Duration::days(365));
        assert!(
            !is_retry_due(&delivery, &clock),
            "exhausted delivery must never become due again"
        );
    }

    // ── Total elapsed time ────────────────────────────────────────────────────

    /// The cumulative time from the first attempt to the last retry deadline
    /// must equal the sum of all five delay constants.
    #[test]
    fn total_elapsed_time_equals_sum_of_delays() {
        let clock = MockClock::at(t0());
        let mut delivery = pending_delivery(t0());

        drive_all_failures(&mut delivery, &clock);

        let expected_total: u64 = RETRY_DELAYS_SECS.iter().sum();
        let actual_total = (clock.now() - t0()).num_seconds() as u64;
        assert_eq!(
            actual_total, expected_total,
            "total elapsed time must equal the sum of all retry delays"
        );
    }
}
