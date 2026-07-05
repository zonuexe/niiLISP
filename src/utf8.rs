//! UTF-8 character decoding over binary-safe byte strings (ADR-0025).
//!
//! This is the single place character boundaries are computed; every
//! character-based builtin goes through it. Decoding is **lenient** — the
//! character length comes from the lead byte, and an invalid or truncated lead
//! is treated as a one-byte character — so it never panics on the invalid /
//! binary bytes a niiLISP string may hold (ADR-0013). `str::chars()` (strict
//! UTF-8) is deliberately not used.
//!
//! On-demand decoding makes character indexing O(n); a lazily-built,
//! `Rc`-attached byte-offset index (O(1), ADR-0024/0025) would slot in here
//! without changing callers, but is deferred until a workload needs it.

/// The byte length (1..=4) of the UTF-8 character whose lead byte is `lead`.
/// A continuation byte or otherwise invalid lead decodes as a single byte.
fn lead_len(lead: u8) -> usize {
    if lead < 0x80 {
        1
    } else if lead >> 5 == 0b110 {
        2
    } else if lead >> 4 == 0b1110 {
        3
    } else if lead >> 3 == 0b11110 {
        4
    } else {
        1
    }
}

/// Iterate the `(start, end)` byte range of each character in `bytes`.
pub fn char_ranges(bytes: &[u8]) -> impl Iterator<Item = (usize, usize)> + '_ {
    let mut i = 0;
    std::iter::from_fn(move || {
        if i >= bytes.len() {
            return None;
        }
        let start = i;
        let len = lead_len(bytes[i]).min(bytes.len() - i);
        i += len;
        Some((start, start + len))
    })
}

/// The number of UTF-8 characters (code points) in `bytes` — `utf8len`.
pub fn char_count(bytes: &[u8]) -> usize {
    char_ranges(bytes).count()
}

/// Iterate the Unicode code point of each character in `bytes`. A well-formed
/// multi-byte sequence yields its code point; an invalid or truncated sequence
/// (decoded as one byte by `lead_len`) yields that raw byte value, so this never
/// fails on the binary bytes a niiLISP string may hold (ADR-0013).
pub fn codepoints(bytes: &[u8]) -> impl Iterator<Item = u32> + '_ {
    char_ranges(bytes).map(move |(start, end)| {
        std::str::from_utf8(&bytes[start..end])
            .ok()
            .and_then(|s| s.chars().next())
            .map_or(u32::from(bytes[start]), u32::from)
    })
}

/// The byte range of the `idx`-th character (a negative `idx` counts from the
/// end), or `None` if out of range.
pub fn char_byte_range(bytes: &[u8], idx: i64) -> Option<(usize, usize)> {
    if idx >= 0 {
        char_ranges(bytes).nth(idx as usize)
    } else {
        let ranges: Vec<(usize, usize)> = char_ranges(bytes).collect();
        let i = ranges.len() as i64 + idx;
        usize::try_from(i).ok().and_then(|k| ranges.get(k).copied())
    }
}

/// The byte offset just past the first character (i.e. where `rest` begins);
/// `0` for an empty string.
pub fn first_char_end(bytes: &[u8]) -> usize {
    char_ranges(bytes).next().map_or(0, |(_, end)| end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_and_ranges() {
        assert_eq!(char_count(b"abc"), 3);
        // "caf\xc3\xa9" = c a f é (é is 2 bytes) -> 4 chars, 5 bytes.
        assert_eq!(char_count(b"caf\xc3\xa9"), 4);
        // 日本 = two 3-byte characters.
        assert_eq!(char_count("日本".as_bytes()), 2);
    }

    #[test]
    fn indexing_by_character() {
        let s = "caf\u{e9}".as_bytes();
        assert_eq!(char_byte_range(s, 3), Some((3, 5))); // é
        assert_eq!(char_byte_range(s, -1), Some((3, 5)));
        assert_eq!(char_byte_range(s, 0), Some((0, 1))); // c
        assert_eq!(char_byte_range(s, 4), None); // out of range
        assert_eq!(first_char_end(s), 1);
    }

    #[test]
    fn lenient_on_invalid_bytes() {
        // A stray continuation byte / invalid lead decodes as one byte each,
        // never panicking (binary-safe, ADR-0025).
        assert_eq!(char_count(&[0x80, 0xff, b'a']), 3);
        // A truncated 3-byte lead with only one following byte -> clamped.
        assert_eq!(char_count(&[0xe6, 0x97]), 1);
    }
}
