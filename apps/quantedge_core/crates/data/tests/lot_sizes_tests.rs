use chrono::NaiveDate;
use quantedge_data::LotSizes;

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

fn get_lots() -> LotSizes {
    LotSizes::from_toml(&config_path("lot_sizes.toml")).unwrap()
}

// ─── NIFTY LOT SIZES (50 → 25 → 75) ───────────────────────

#[test]
fn test_nifty_lot_50_before_apr2024() {
    let lots = get_lots();
    assert_eq!(
        lots.get("NIFTY", NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()),
        Some(50)
    );
}

#[test]
fn test_nifty_lot_50_last_day() {
    let lots = get_lots();
    // Apr 25, 2024 = last day at 50
    assert_eq!(
        lots.get("NIFTY", NaiveDate::from_ymd_opt(2024, 4, 25).unwrap()),
        Some(50)
    );
}

#[test]
fn test_nifty_lot_25_from_apr26() {
    let lots = get_lots();
    // Apr 26, 2024 = first day at 25
    assert_eq!(
        lots.get("NIFTY", NaiveDate::from_ymd_opt(2024, 4, 26).unwrap()),
        Some(25)
    );
}

#[test]
fn test_nifty_lot_25_last_day() {
    let lots = get_lots();
    // Nov 19, 2024 = last day at 25
    assert_eq!(
        lots.get("NIFTY", NaiveDate::from_ymd_opt(2024, 11, 19).unwrap()),
        Some(25)
    );
}

#[test]
fn test_nifty_lot_75_from_nov20() {
    let lots = get_lots();
    // Nov 20, 2024 = first day at 75
    assert_eq!(
        lots.get("NIFTY", NaiveDate::from_ymd_opt(2024, 11, 20).unwrap()),
        Some(75)
    );
}

// ─── BANKNIFTY LOT SIZES (15 → 30) ─────────────────────────

#[test]
fn test_banknifty_lot_15() {
    let lots = get_lots();
    assert_eq!(
        lots.get("BANKNIFTY", NaiveDate::from_ymd_opt(2024, 6, 15).unwrap()),
        Some(15)
    );
}

#[test]
fn test_banknifty_lot_15_last_day() {
    let lots = get_lots();
    // Nov 19, 2024 = last day at 15
    assert_eq!(
        lots.get("BANKNIFTY", NaiveDate::from_ymd_opt(2024, 11, 19).unwrap()),
        Some(15)
    );
}

#[test]
fn test_banknifty_lot_30_from_nov20() {
    let lots = get_lots();
    // Nov 20, 2024 = first day at 30
    assert_eq!(
        lots.get("BANKNIFTY", NaiveDate::from_ymd_opt(2024, 11, 20).unwrap()),
        Some(30)
    );
}

// ─── SENSEX LOT SIZES (10 → 20) ────────────────────────────

#[test]
fn test_sensex_lot_10() {
    let lots = get_lots();
    assert_eq!(
        lots.get("SENSEX", NaiveDate::from_ymd_opt(2024, 6, 15).unwrap()),
        Some(10)
    );
}

#[test]
fn test_sensex_lot_10_last_day() {
    let lots = get_lots();
    // Nov 19, 2024 = last day at 10
    assert_eq!(
        lots.get("SENSEX", NaiveDate::from_ymd_opt(2024, 11, 19).unwrap()),
        Some(10)
    );
}

#[test]
fn test_sensex_lot_20_from_nov20() {
    let lots = get_lots();
    // Nov 20, 2024 = first day at 20
    assert_eq!(
        lots.get("SENSEX", NaiveDate::from_ymd_opt(2024, 11, 20).unwrap()),
        Some(20)
    );
}

// ─── EDGE CASES ─────────────────────────────────────────────

#[test]
fn test_unknown_symbol() {
    let lots = get_lots();
    assert_eq!(
        lots.get("UNKNOWN", NaiveDate::from_ymd_opt(2024, 6, 15).unwrap()),
        None
    );
}

#[test]
fn test_case_insensitive() {
    let lots = get_lots();
    assert_eq!(
        lots.get("banknifty", NaiveDate::from_ymd_opt(2024, 6, 15).unwrap()),
        Some(15)
    );
}
