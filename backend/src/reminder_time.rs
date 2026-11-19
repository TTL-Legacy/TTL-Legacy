//! Timezone-aware reminder scheduling (issue #1752).
//!
//! Reminder preferences store an IANA timezone (for example
//! `America/New_York`). Recurring reminders fire at 09:00 **local** time, so
//! the UTC instant must be derived from the timezone's offset at the reminder
//! date, not from a fixed offset or from epoch arithmetic.
//!
//! The scheduler polls once per minute, so "due" is a one-hour window starting
//! at the resolved 09:00 wall time. Recomputing the window on every tick means
//! the offset is always the one in force at the scheduled date: a 09:00 EDT
//! reminder is `13:00Z`, a 09:00 EST reminder is `14:00Z`, and the scheduler
//! crosses the spring/fall transitions without drifting an hour.
//!
//! ## DST edge cases
//!
//! * **Spring forward:** if the preferred wall time does not exist (some zones
//!   skip midnight, never 09:00 in the zones we support), we schedule the first
//!   valid instant after it — the same behaviour as cron.
//! * **Fall back:** if the wall time occurs twice, we pick the first (earlier)
//!   occurrence.
//! * **Invalid zone:** an unknown IANA name yields `None` / `false`, so the
//!   scheduler never fires a reminder for a misconfigured preference.

use std::str::FromStr;

use chrono::{DateTime, Datelike, Duration, LocalResult, NaiveDate, TimeZone, Utc, Weekday};
use chrono_tz::Tz;

/// Local wall-clock hour at which recurring reminders are sent.
pub const REMINDER_LOCAL_HOUR: u32 = 9;

/// Local weekday used by weekly reminders.
pub const WEEKLY_REMINDER_WEEKDAY: Weekday = Weekday::Mon;

/// Local day of month used by monthly reminders.
pub const MONTHLY_REMINDER_DAY: u32 = 1;

/// How often a reminder repeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReminderCadence {
    Daily,
    Weekly,
    Monthly,
}

/// Which recurring reminders are due at `now` for the user's timezone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReminderWindow {
    pub daily: bool,
    pub weekly: bool,
    pub monthly: bool,
}

impl ReminderWindow {
    /// No reminder is due (`false` for every cadence).
    pub const NONE: Self = Self {
        daily: false,
        weekly: false,
        monthly: false,
    };

    /// Whether `cadence` is due.
    #[must_use]
    pub fn is_due(self, cadence: ReminderCadence) -> bool {
        match cadence {
            ReminderCadence::Daily => self.daily,
            ReminderCadence::Weekly => self.weekly,
            ReminderCadence::Monthly => self.monthly,
        }
    }
}

/// Parses an IANA timezone name, returning `None` for unknown names.
#[must_use]
pub fn parse_timezone(timezone: &str) -> Option<Tz> {
    Tz::from_str(timezone).ok()
}

/// Returns `true` when `timezone` is a valid IANA identifier.
#[must_use]
pub fn is_valid_timezone(timezone: &str) -> bool {
    parse_timezone(timezone).is_some()
}

/// Resolves the recurring reminder window at `now` in `timezone`.
///
/// Returns `None` when the timezone is not a valid IANA identifier. Callers
/// that treat an invalid timezone as "no recurring reminder" can use
/// [`ReminderWindow::NONE`] instead:
///
/// ```ignore
/// let window = reminder_window(now, &prefs.timezone).unwrap_or(ReminderWindow::NONE);
/// ```
#[must_use]
pub fn reminder_window(now: DateTime<Utc>, timezone: &str) -> Option<ReminderWindow> {
    let tz = parse_timezone(timezone)?;
    let local_date = now.with_timezone(&tz).date_naive();
    let in_window = in_reminder_window(tz, local_date, now);

    Some(ReminderWindow {
        daily: in_window,
        weekly: in_window && local_date.weekday() == WEEKLY_REMINDER_WEEKDAY,
        monthly: in_window && local_date.day() == MONTHLY_REMINDER_DAY,
    })
}

/// Returns `true` when a reminder for `cadence` is due at `now`.
#[must_use]
pub fn is_due(now: DateTime<Utc>, timezone: &str, cadence: ReminderCadence) -> bool {
    reminder_window(now, timezone).is_some_and(|window| window.is_due(cadence))
}

/// The next instant after `now` at which a reminder for `cadence` is due.
///
/// The offset is resolved for the candidate date, so the returned instant is
/// always 09:00 local — across DST transitions as well.
#[must_use]
pub fn next_occurrence_utc(
    now: DateTime<Utc>,
    timezone: &str,
    cadence: ReminderCadence,
) -> Option<DateTime<Utc>> {
    let tz = parse_timezone(timezone)?;
    let mut date = now.with_timezone(&tz).date_naive();

    // A year is more than enough to find the next day/week/month occurrence.
    for _ in 0..=366 {
        if cadence_matches(cadence, date) {
            if let Some(candidate) = local_reminder_instant(tz, date) {
                if candidate > now {
                    return Some(candidate);
                }
            }
        }
        date = date.succ_opt()?;
    }

    None
}

/// Whether `date` is a scheduled day for `cadence`.
fn cadence_matches(cadence: ReminderCadence, date: NaiveDate) -> bool {
    match cadence {
        ReminderCadence::Daily => true,
        ReminderCadence::Weekly => date.weekday() == WEEKLY_REMINDER_WEEKDAY,
        ReminderCadence::Monthly => date.day() == MONTHLY_REMINDER_DAY,
    }
}

/// Whether `now` falls in the one-hour window that starts at the local
/// reminder time on `date`.
fn in_reminder_window(tz: Tz, date: NaiveDate, now: DateTime<Utc>) -> bool {
    match local_reminder_instant(tz, date) {
        Some(start) => now >= start && now < start + Duration::hours(1),
        None => false,
    }
}

/// Resolves 09:00 on `date` in `tz` to a UTC instant.
///
/// When the wall time is skipped by a forward transition, the first valid
/// minute after it is used. When it is repeated by a backwards transition, the
/// earlier occurrence is used.
fn local_reminder_instant(tz: Tz, date: NaiveDate) -> Option<DateTime<Utc>> {
    let mut minutes = REMINDER_LOCAL_HOUR.checked_mul(60)?;
    let end_of_day = 24 * 60;

    while minutes < end_of_day {
        let naive = date.and_hms_opt(minutes / 60, minutes % 60, 0)?;
        match tz.from_local_datetime(&naive) {
            LocalResult::Single(dt) => return Some(dt.with_timezone(&Utc)),
            LocalResult::Ambiguous(earliest, _) => return Some(earliest.with_timezone(&Utc)),
            LocalResult::None => minutes += 1,
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{
        is_due, is_valid_timezone, next_occurrence_utc, reminder_window, ReminderCadence,
        ReminderWindow, REMINDER_LOCAL_HOUR,
    };
    use chrono::{Datelike, TimeZone, Utc, Weekday};

    fn utc(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
            .unwrap()
    }

    #[test]
    fn parses_iana_timezones_and_rejects_unknown_names() {
        assert!(is_valid_timezone("America/New_York"));
        assert!(is_valid_timezone("Europe/London"));
        assert!(is_valid_timezone("UTC"));
        assert!(!is_valid_timezone("Mars/Olympus_Mons"));
        assert!(!is_valid_timezone("not/a-timezone"));
        assert!(!is_valid_timezone(""));
    }

    #[test]
    fn next_occurrence_uses_the_offset_in_force_on_the_reminder_date() {
        // 2026-03-08 is the US spring-forward date (02:00 EST -> 03:00 EDT).
        // The day before, 09:00 is EST (14:00Z).
        assert_eq!(
            next_occurrence_utc(
                utc(2026, 3, 7, 15, 0, 0), // 10:00 EST, today's slot has passed
                "America/New_York",
                ReminderCadence::Daily,
            ),
            Some(utc(2026, 3, 8, 13, 0, 0)), // 09:00 EDT
        );

        // 2026-11-01 is the US fall-back date (02:00 EDT -> 01:00 EST).
        // The next 09:00 is EST (14:00Z); adding a fixed 24h would produce
        // 13:00Z (08:00 local) instead.
        assert_eq!(
            next_occurrence_utc(
                utc(2026, 10, 31, 14, 0, 0), // 10:00 EDT
                "America/New_York",
                ReminderCadence::Daily,
            ),
            Some(utc(2026, 11, 1, 14, 0, 0)), // 09:00 EST
        );
    }

    #[test]
    fn next_occurrence_handles_uk_bst_transition() {
        // UK clocks go forward on 2026-03-29. 09:00 BST is 08:00Z.
        assert_eq!(
            next_occurrence_utc(
                utc(2026, 3, 28, 10, 0, 0), // 10:00 GMT, today's 09:00 passed
                "Europe/London",
                ReminderCadence::Daily,
            ),
            Some(utc(2026, 3, 29, 8, 0, 0)),
        );
    }

    #[test]
    fn daily_window_is_local_nine_am_across_dst() {
        // Before the spring-forward day, 09:30 local is 14:30Z.
        assert!(is_due(
            utc(2026, 3, 7, 14, 30, 0),
            "America/New_York",
            ReminderCadence::Daily
        ));
        // On the spring-forward day, 09:30 local is 13:30Z.
        assert!(is_due(
            utc(2026, 3, 8, 13, 30, 0),
            "America/New_York",
            ReminderCadence::Daily
        ));
        // 08:30 and 10:30 local on the transition day are outside the window.
        assert!(!is_due(
            utc(2026, 3, 8, 12, 30, 0),
            "America/New_York",
            ReminderCadence::Daily
        ));
        assert!(!is_due(
            utc(2026, 3, 8, 14, 30, 0),
            "America/New_York",
            ReminderCadence::Daily
        ));
    }

    #[test]
    fn fall_back_day_fires_once_at_local_nine() {
        // On 2026-11-01 the 09:00 EST window is 14:00Z–15:00Z.
        assert!(!is_due(
            utc(2026, 11, 1, 13, 30, 0), // 08:30 EST
            "America/New_York",
            ReminderCadence::Daily
        ));
        assert!(is_due(
            utc(2026, 11, 1, 14, 30, 0), // 09:30 EST
            "America/New_York",
            ReminderCadence::Daily
        ));
        assert!(!is_due(
            utc(2026, 11, 1, 15, 30, 0), // 10:30 EST
            "America/New_York",
            ReminderCadence::Daily
        ));
    }

    #[test]
    fn weekly_reminders_fire_on_monday_local_time() {
        // Wednesday 2026-03-04 07:00 EST -> next Monday 2026-03-09 09:00 EDT.
        let next = next_occurrence_utc(
            utc(2026, 3, 4, 12, 0, 0),
            "America/New_York",
            ReminderCadence::Weekly,
        )
        .unwrap();
        assert_eq!(next, utc(2026, 3, 9, 13, 0, 0));
        assert_eq!(
            next.with_timezone(&chrono_tz::America::New_York).weekday(),
            Weekday::Mon
        );

        // A Monday inside the 09:00–09:59 local window is due.
        assert!(is_due(
            utc(2026, 3, 9, 13, 15, 0),
            "America/New_York",
            ReminderCadence::Weekly
        ));
        // The same clock time on Tuesday is not.
        assert!(!is_due(
            utc(2026, 3, 10, 13, 15, 0),
            "America/New_York",
            ReminderCadence::Weekly
        ));
    }

    #[test]
    fn monthly_reminders_fire_on_the_first_local_day() {
        let next = next_occurrence_utc(
            utc(2026, 3, 15, 12, 0, 0),
            "America/New_York",
            ReminderCadence::Monthly,
        )
        .unwrap();
        assert_eq!(next, utc(2026, 4, 1, 13, 0, 0)); // 09:00 EDT

        assert!(is_due(
            utc(2026, 4, 1, 13, 30, 0),
            "America/New_York",
            ReminderCadence::Monthly
        ));
    }

    #[test]
    fn utc_timezone_is_unaffected_by_dst() {
        assert_eq!(
            next_occurrence_utc(utc(2026, 3, 8, 10, 0, 0), "UTC", ReminderCadence::Daily),
            Some(utc(2026, 3, 9, 9, 0, 0))
        );
    }

    #[test]
    fn invalid_timezone_never_fires() {
        assert_eq!(
            reminder_window(utc(2026, 3, 8, 13, 0, 0), "not/a-zone"),
            None
        );
        assert!(!is_due(
            utc(2026, 3, 8, 13, 0, 0),
            "not/a-zone",
            ReminderCadence::Daily
        ));
        assert_eq!(
            next_occurrence_utc(
                utc(2026, 3, 8, 13, 0, 0),
                "not/a-zone",
                ReminderCadence::Daily
            ),
            None
        );
    }

    #[test]
    fn skipped_wall_time_falls_forward_to_the_first_valid_minute() {
        // Samoa skipped 2011-12-30 entirely when it moved across the date
        // line, so 09:00 local does not exist that day.
        let next = next_occurrence_utc(
            utc(2011, 12, 29, 19, 0, 0), // 09:00 local on 2011-12-29 (UTC-10)
            "Pacific/Apia",
            ReminderCadence::Daily,
        )
        .unwrap();

        // The skipped day has no valid 09:00, so the next occurrence is
        // 2011-12-31 09:00 local (UTC+14) = 2011-12-30 19:00Z. The key point
        // is that the offset is re-resolved (+13h jump) rather than carried
        // over from the previous day.
        assert_eq!(next, utc(2011, 12, 30, 19, 0, 0));
    }

    #[test]
    fn window_reports_each_cadence_independently() {
        // 2026-06-01 is a Monday and the first of the month.
        let window = reminder_window(utc(2026, 6, 1, 13, 15, 0), "America/New_York");
        assert_eq!(
            window,
            Some(ReminderWindow {
                daily: true,
                weekly: true,
                monthly: true,
            })
        );

        // A regular Tuesday only reports the daily slot.
        assert_eq!(
            reminder_window(utc(2026, 6, 2, 13, 15, 0), "America/New_York"),
            Some(ReminderWindow {
                daily: true,
                weekly: false,
                monthly: false,
            })
        );
    }

    #[test]
    fn reminder_local_hour_is_nine() {
        assert_eq!(REMINDER_LOCAL_HOUR, 9);
    }
}
