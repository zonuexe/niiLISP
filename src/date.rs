//! Dates and times (ADR-0037). A pure-Rust UTC core — `date-value`/`date-list`
//! and the calendar breakdown behind `now`, computed with the standard
//! civil-date algorithms over an `i64` epoch — is always compiled. When built
//! with the `date` feature on Unix, `date`/`now` additionally report the local
//! timezone (via `libc` `localtime_r`/`strftime`) and `date-parse` (`strptime`)
//! is available; otherwise those fall back to UTC / `nil`, matching newLISP's
//! own "not on Windows" behaviour for `date-parse`.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::eval::{Interp, Signal};
use crate::value::Value;

pub fn install(interp: &Interp) {
    interp.register_builtin("now", b_now);
    interp.register_builtin("date", b_date);
    interp.register_builtin("date-value", b_date_value);
    interp.register_builtin("date-list", b_date_list);
    interp.register_builtin("date-parse", b_date_parse);
}

const DEFAULT_FORMAT: &str = "%a %b %d %H:%M:%S %Y";

// ---- pure UTC civil-date core (always compiled) --------------------------

/// Days from 1970-01-01 to `y-m-d` (proleptic Gregorian). Howard Hinnant's
/// `days_from_civil`.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// `(year, month, day)` from days since 1970-01-01.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Day of week, 0=Sunday..6=Saturday (`tm_wday`), from days since the epoch.
fn weekday(z: i64) -> i64 {
    (z.rem_euclid(7) + 4) % 7
}

/// UTC breakdown of an epoch-seconds value:
/// `[year, month, day, hour, minute, second, day-of-year, day-of-week]`.
fn utc_fields(secs: i64) -> [i64; 8] {
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    let doy = days - days_from_civil(y, 1, 1) + 1;
    [
        y,
        m,
        d,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60,
        doy,
        weekday(days),
    ]
}

fn utc_secs(y: i64, m: i64, d: i64, h: i64, mi: i64, s: i64) -> i64 {
    days_from_civil(y, m, d) * 86400 + h * 3600 + mi * 60 + s
}

/// Current time as `(epoch-seconds, microseconds)`.
fn now_parts() -> (i64, i64) {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(dur) => (dur.as_secs() as i64, dur.subsec_micros() as i64),
        Err(e) => (-(e.duration().as_secs() as i64), 0),
    }
}

fn to_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Int(n) => Some(*n),
        Value::Float(f) => Some(*f as i64),
        #[cfg(feature = "bigint")]
        Value::Bigint(_) => None,
        _ => None,
    }
}

/// Read the leading date components from either a list argument or the flat
/// argument run: `year month day [hour min sec]`.
fn read_components(args: &[Value]) -> Option<[i64; 6]> {
    let src: Vec<i64> = match args.first() {
        Some(Value::List(l)) => l.iter().filter_map(to_i64).collect(),
        _ => args.iter().filter_map(to_i64).collect(),
    };
    if src.len() < 3 {
        return None;
    }
    let g = |i: usize| src.get(i).copied().unwrap_or(0);
    Some([g(0), g(1), g(2), g(3), g(4), g(5)])
}

/// Return element `index` of `list` (newLISP's optional index argument), or the
/// whole list.
fn indexed(list: Vec<Value>, index: Option<&Value>) -> Value {
    match index.and_then(to_i64) {
        Some(i) => {
            let len = list.len() as i64;
            let i = if i < 0 { len + i } else { i };
            if i < 0 || i >= len {
                Value::Nil
            } else {
                list[i as usize].clone()
            }
        }
        None => Value::list(list),
    }
}

// ---- builtins ------------------------------------------------------------

fn b_date_value(_i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    // (date-value) -> now; (date-value y m d [h mi s]) / (date-value list) -> UTC.
    if args.is_empty() {
        return Ok(Value::Int(now_parts().0));
    }
    match read_components(args) {
        Some([y, m, d, h, mi, s]) => Ok(Value::Int(utc_secs(y, m, d, h, mi, s))),
        None => Ok(Value::Nil),
    }
}

fn b_date_list(_i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    // (date-list [seconds [index]]) -> UTC (year month day hour min sec doy dow).
    let secs = match args.first() {
        Some(v) => match to_i64(v) {
            Some(n) => n,
            None => return Ok(Value::Nil),
        },
        None => now_parts().0,
    };
    let list: Vec<Value> = utc_fields(secs).iter().map(|&n| Value::Int(n)).collect();
    Ok(indexed(list, args.get(1)))
}

fn b_now(_i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    // (now [offset-minutes [index]]) -> 11 integers, local time when available.
    let (mut secs, micros) = now_parts();
    if let Some(off) = args.first().and_then(to_i64) {
        secs += off * 60;
    }
    let list = now_list(secs, micros);
    Ok(indexed(list, args.get(1)))
}

fn b_date(_i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    // (date [seconds [offset-minutes [format]]]) -> local date/time string.
    let mut secs = match args.first() {
        Some(v) => match to_i64(v) {
            Some(n) => n,
            None => return Ok(Value::Nil),
        },
        None => now_parts().0,
    };
    if let Some(off) = args.get(1).and_then(to_i64) {
        secs += off * 60;
    }
    let fmt = match args.get(2) {
        Some(Value::Str(b)) => String::from_utf8_lossy(b).into_owned(),
        _ => DEFAULT_FORMAT.to_string(),
    };
    match format_date(secs, &fmt) {
        Some(s) => Ok(Value::str(s.into_bytes())),
        None => Ok(Value::Nil),
    }
}

fn b_date_parse(_i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    // (date-parse str format) -> UTC seconds, or nil (also nil without libc).
    let (s, f) = match (args.first(), args.get(1)) {
        (Some(Value::Str(s)), Some(Value::Str(f))) => (
            String::from_utf8_lossy(s).into_owned(),
            String::from_utf8_lossy(f).into_owned(),
        ),
        _ => {
            return Err(Signal::error(
                "date-parse: expected (date-parse str format)",
            ))
        }
    };
    Ok(parse_date(&s, &f))
}

// ---- local-time paths: libc on Unix, pure-UTC fallback otherwise ---------

#[cfg(all(feature = "date", unix))]
mod local {
    use super::*;

    /// Fill a `libc::tm` for `secs` in the local timezone.
    fn local_tm(secs: i64) -> libc::tm {
        // SAFETY: `t` points to a valid time_t; `tm` is fully written by
        // localtime_r before we read it.
        unsafe {
            let mut tm: libc::tm = std::mem::zeroed();
            let t = secs as libc::time_t;
            if libc::localtime_r(&t, &mut tm).is_null() {
                // Fall back to a UTC breakdown on failure.
                let f = utc_fields(secs);
                tm.tm_year = (f[0] - 1900) as libc::c_int;
                tm.tm_mon = (f[1] - 1) as libc::c_int;
                tm.tm_mday = f[2] as libc::c_int;
                tm.tm_hour = f[3] as libc::c_int;
                tm.tm_min = f[4] as libc::c_int;
                tm.tm_sec = f[5] as libc::c_int;
                tm.tm_yday = (f[6] - 1) as libc::c_int;
                tm.tm_wday = f[7] as libc::c_int;
            }
            tm
        }
    }

    pub fn now_list(secs: i64, micros: i64) -> Vec<Value> {
        let tm = local_tm(secs);
        #[allow(clippy::useless_conversion)] // tm_gmtoff is c_long, varies by platform
        let gmtoff: i64 = tm.tm_gmtoff.into();
        [
            i64::from(tm.tm_year) + 1900,
            i64::from(tm.tm_mon) + 1,
            i64::from(tm.tm_mday),
            i64::from(tm.tm_hour),
            i64::from(tm.tm_min),
            i64::from(tm.tm_sec),
            micros,
            i64::from(tm.tm_yday) + 1,
            i64::from(tm.tm_wday),
            gmtoff / 60,
            i64::from(tm.tm_isdst).max(0),
        ]
        .iter()
        .map(|&n| Value::Int(n))
        .collect()
    }

    pub fn format_date(secs: i64, fmt: &str) -> Option<String> {
        let tm = local_tm(secs);
        let cfmt = std::ffi::CString::new(fmt).ok()?;
        let mut buf = vec![0u8; 256];
        // SAFETY: buf/cfmt are valid for the given lengths; tm is initialized.
        let n = unsafe {
            libc::strftime(
                buf.as_mut_ptr() as *mut libc::c_char,
                buf.len(),
                cfmt.as_ptr(),
                &tm,
            )
        };
        if n == 0 {
            return Some(String::new());
        }
        buf.truncate(n);
        Some(String::from_utf8_lossy(&buf).into_owned())
    }

    pub fn parse_date(s: &str, fmt: &str) -> Value {
        let cs = match std::ffi::CString::new(s) {
            Ok(c) => c,
            Err(_) => return Value::Nil,
        };
        let cf = match std::ffi::CString::new(fmt) {
            Ok(c) => c,
            Err(_) => return Value::Nil,
        };
        // SAFETY: tm is zeroed then filled by strptime; pointers are valid.
        unsafe {
            let mut tm: libc::tm = std::mem::zeroed();
            let end = libc::strptime(cs.as_ptr(), cf.as_ptr(), &mut tm);
            if end.is_null() {
                return Value::Nil;
            }
            // strptime fills a struct tm in UTC terms here; convert via our
            // pure calendar so the result is timezone-independent (as newLISP's
            // date-parse returns UTC seconds).
            let secs = utc_secs(
                i64::from(tm.tm_year) + 1900,
                i64::from(tm.tm_mon) + 1,
                i64::from(tm.tm_mday),
                i64::from(tm.tm_hour),
                i64::from(tm.tm_min),
                i64::from(tm.tm_sec),
            );
            Value::Int(secs)
        }
    }
}

#[cfg(all(feature = "date", unix))]
use local::{format_date, now_list, parse_date};

// Fallback: UTC breakdown, a pure `strftime` subset, and no `date-parse`.
#[cfg(not(all(feature = "date", unix)))]
fn now_list(secs: i64, micros: i64) -> Vec<Value> {
    let f = utc_fields(secs);
    [f[0], f[1], f[2], f[3], f[4], f[5], micros, f[6], f[7], 0, 0]
        .iter()
        .map(|&n| Value::Int(n))
        .collect()
}

#[cfg(not(all(feature = "date", unix)))]
fn parse_date(_s: &str, _fmt: &str) -> Value {
    Value::Nil
}

#[cfg(not(all(feature = "date", unix)))]
fn format_date(secs: i64, fmt: &str) -> Option<String> {
    Some(pure_strftime(secs, fmt))
}

/// A small `strftime` subset for the pure/non-Unix build — UTC only.
#[cfg(not(all(feature = "date", unix)))]
fn pure_strftime(secs: i64, fmt: &str) -> String {
    const WDAY: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const WDAY_FULL: [&str; 7] = [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ];
    const MON: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    const MON_FULL: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let f = utc_fields(secs);
    let (y, m, d, h, mi, s, doy, dow) = (f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7]);
    let h12 = {
        let x = h % 12;
        if x == 0 {
            12
        } else {
            x
        }
    };
    let mut out = String::new();
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('Y') => out.push_str(&y.to_string()),
            Some('m') => out.push_str(&format!("{:02}", m)),
            Some('d') => out.push_str(&format!("{:02}", d)),
            Some('H') => out.push_str(&format!("{:02}", h)),
            Some('M') => out.push_str(&format!("{:02}", mi)),
            Some('S') => out.push_str(&format!("{:02}", s)),
            Some('I') => out.push_str(&format!("{:02}", h12)),
            Some('j') => out.push_str(&format!("{:03}", doy)),
            Some('w') => out.push_str(&dow.to_string()),
            Some('a') => out.push_str(WDAY[dow as usize]),
            Some('A') => out.push_str(WDAY_FULL[dow as usize]),
            Some('b') | Some('h') => out.push_str(MON[(m - 1) as usize]),
            Some('B') => out.push_str(MON_FULL[(m - 1) as usize]),
            Some('p') => out.push_str(if h < 12 { "AM" } else { "PM" }),
            Some('%') => out.push('%'),
            Some(other) => {
                out.push('%');
                out.push(other);
            }
            None => out.push('%'),
        }
    }
    out
}
