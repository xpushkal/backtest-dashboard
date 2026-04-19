use chrono::{Datelike, NaiveDate};
use quantedge_data::ExpiryCalendar;

/// Resolve the config path relative to the workspace root.
fn config_path(name: &str) -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let workspace_root = std::path::Path::new(manifest)
        .ancestors()
        .nth(4)
        .unwrap();
    workspace_root
        .join("config")
        .join(name)
        .to_string_lossy()
        .to_string()
}

fn get_calendar() -> ExpiryCalendar {
    ExpiryCalendar::from_toml(&config_path("expiry_calendar.toml")).unwrap()
}

// ─── BANKNIFTY TESTS ────────────────────────────────────────

#[test]
fn test_banknifty_weekly_thursday_pre_sep2023() {
    let cal = get_calendar();
    // 2023-08-28 Monday → next Thursday = 2023-08-31
    let (expiry, etype, dte) = cal
        .next_expiry("BANKNIFTY", NaiveDate::from_ymd_opt(2023, 8, 28).unwrap())
        .unwrap();
    assert_eq!(etype, "weekly");
    assert_eq!(expiry.weekday(), chrono::Weekday::Thu);
    assert_eq!(dte, 3); // Mon→Thu = 3 days
}

#[test]
fn test_banknifty_weekly_wednesday_post_sep2023() {
    let cal = get_calendar();
    // 2024-01-08 Monday → next Wednesday = 2024-01-10
    let (expiry, etype, dte) = cal
        .next_expiry("BANKNIFTY", NaiveDate::from_ymd_opt(2024, 1, 8).unwrap())
        .unwrap();
    assert_eq!(etype, "weekly");
    assert_eq!(expiry.weekday(), chrono::Weekday::Wed);
    assert_eq!(dte, 2); // Mon→Wed = 2 days
}

#[test]
fn test_banknifty_monthly_after_nov2024() {
    let cal = get_calendar();
    // 2024-12-05 → should be monthly, last Wednesday of Dec 2024 = 2024-12-25
    let (expiry, etype, _) = cal
        .next_expiry("BANKNIFTY", NaiveDate::from_ymd_opt(2024, 12, 5).unwrap())
        .unwrap();
    assert_eq!(etype, "monthly");
    assert_eq!(expiry.weekday(), chrono::Weekday::Wed);
    assert_eq!(expiry, NaiveDate::from_ymd_opt(2024, 12, 25).unwrap());
}

#[test]
fn test_banknifty_transition_boundary_sep2023() {
    let cal = get_calendar();
    // Sep 3, 2023 = still Thursday based
    let (_, etype_before, _) = cal
        .next_expiry("BANKNIFTY", NaiveDate::from_ymd_opt(2023, 9, 3).unwrap())
        .unwrap();
    assert_eq!(etype_before, "weekly");

    // Sep 4, 2023 = Wednesday based
    let (expiry_after, etype_after, _) = cal
        .next_expiry("BANKNIFTY", NaiveDate::from_ymd_opt(2023, 9, 4).unwrap())
        .unwrap();
    assert_eq!(etype_after, "weekly");
    assert_eq!(expiry_after.weekday(), chrono::Weekday::Wed);
}

#[test]
fn test_banknifty_transition_weekly_to_monthly() {
    let cal = get_calendar();
    // Nov 13, 2024 = last weekly
    let (_, etype_last_weekly, _) = cal
        .next_expiry("BANKNIFTY", NaiveDate::from_ymd_opt(2024, 11, 13).unwrap())
        .unwrap();
    assert_eq!(etype_last_weekly, "weekly");

    // Nov 14, 2024 = monthly
    let (_, etype_monthly, _) = cal
        .next_expiry("BANKNIFTY", NaiveDate::from_ymd_opt(2024, 11, 14).unwrap())
        .unwrap();
    assert_eq!(etype_monthly, "monthly");
}

// ─── NIFTY TESTS ────────────────────────────────────────────

#[test]
fn test_nifty_weekly_thursday_always() {
    let cal = get_calendar();
    // 2024-06-10 Monday → next Thursday = 2024-06-13
    let (expiry, etype, _) = cal
        .next_expiry("NIFTY", NaiveDate::from_ymd_opt(2024, 6, 10).unwrap())
        .unwrap();
    assert_eq!(etype, "weekly");
    assert_eq!(expiry.weekday(), chrono::Weekday::Thu);
}

#[test]
fn test_nifty_still_weekly_after_nov2024() {
    let cal = get_calendar();
    // Nifty is the RETAINED weekly index on NSE
    let (_, etype, _) = cal
        .next_expiry("NIFTY", NaiveDate::from_ymd_opt(2024, 11, 25).unwrap())
        .unwrap();
    assert_eq!(etype, "weekly");
}

#[test]
fn test_nifty_still_weekly_2025() {
    let cal = get_calendar();
    let (expiry, etype, _) = cal
        .next_expiry("NIFTY", NaiveDate::from_ymd_opt(2025, 3, 10).unwrap())
        .unwrap();
    assert_eq!(etype, "weekly");
    assert_eq!(expiry.weekday(), chrono::Weekday::Thu);
}

// ─── SENSEX TESTS ───────────────────────────────────────────

#[test]
fn test_sensex_weekly_friday_before_2025() {
    let cal = get_calendar();
    let (expiry, etype, _) = cal
        .next_expiry("SENSEX", NaiveDate::from_ymd_opt(2024, 9, 10).unwrap())
        .unwrap();
    assert_eq!(etype, "weekly");
    assert_eq!(expiry.weekday(), chrono::Weekday::Fri);
}

#[test]
fn test_sensex_weekly_tuesday_after_2025() {
    let cal = get_calendar();
    let (expiry, etype, _) = cal
        .next_expiry("SENSEX", NaiveDate::from_ymd_opt(2025, 1, 6).unwrap())
        .unwrap();
    assert_eq!(etype, "weekly");
    assert_eq!(expiry.weekday(), chrono::Weekday::Tue);
}

#[test]
fn test_sensex_transition_friday_to_tuesday() {
    let cal = get_calendar();
    // Dec 31, 2024 = still Friday
    let (expiry_before, _, _) = cal
        .next_expiry("SENSEX", NaiveDate::from_ymd_opt(2024, 12, 30).unwrap())
        .unwrap();
    assert_eq!(expiry_before.weekday(), chrono::Weekday::Fri);

    // Jan 1, 2025 = Tuesday
    let (expiry_after, _, _) = cal
        .next_expiry("SENSEX", NaiveDate::from_ymd_opt(2025, 1, 1).unwrap())
        .unwrap();
    assert_eq!(expiry_after.weekday(), chrono::Weekday::Tue);
}

// ─── GENERAL TESTS ──────────────────────────────────────────

#[test]
fn test_dte_is_non_negative() {
    let cal = get_calendar();
    let (_, _, dte) = cal
        .next_expiry("BANKNIFTY", NaiveDate::from_ymd_opt(2024, 6, 15).unwrap())
        .unwrap();
    assert!(dte >= 0);
}

#[test]
fn test_expiry_type_method() {
    let cal = get_calendar();
    assert_eq!(
        cal.expiry_type("BANKNIFTY", NaiveDate::from_ymd_opt(2024, 6, 15).unwrap()),
        Some("weekly".to_string())
    );
    assert_eq!(
        cal.expiry_type("BANKNIFTY", NaiveDate::from_ymd_opt(2024, 12, 1).unwrap()),
        Some("monthly".to_string())
    );
}
