//! Capture a short git revision and a build date for the `--version` banner
//! (php/ruby style). Both are best-effort: a crate built from a published
//! tarball has no `.git`, and reproducible builds may pin the date via
//! `SOURCE_DATE_EPOCH` — handled below.

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    // Short git revision; "unknown" when built without a working tree.
    let rev = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=NIILISP_GIT_REV={rev}");

    // Build date (UTC, YYYY-MM-DD). Honour SOURCE_DATE_EPOCH for reproducible
    // builds; otherwise the wall clock at build time.
    let epoch = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        });
    println!("cargo:rustc-env=NIILISP_BUILD_DATE={}", ymd_utc(epoch));

    // Re-run when the checked-out revision changes.
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
}

/// `YYYY-MM-DD` (UTC) for a Unix timestamp, without a date crate — Howard
/// Hinnant's `civil_from_days` algorithm.
fn ymd_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}
