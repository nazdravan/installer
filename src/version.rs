//! Compararea versiunilor scrise de mână în `program.toml` și găsite în registry.

use std::cmp::Ordering;

/// Compară două versiuni după grupurile lor de cifre: `3.0.10` > `3.0.4`, `3.0` = `3.0.0`.
/// Tot ce nu e cifră desparte segmente, deci `v24.1` și `1.2-beta` merg și ele.
pub fn compare(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (segments(a), segments(b));
    let len = a.len().max(b.len());
    a.resize(len, 0);
    b.resize(len, 0);
    a.cmp(&b)
}

fn segments(v: &str) -> Vec<u64> {
    v.split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap_or(u64::MAX))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use Ordering::*;

    #[test]
    fn compare_numeric_segments() {
        assert_eq!(compare("3.0.10", "3.0.4"), Greater);
        assert_eq!(compare("3.0.4", "3.0.4"), Equal);
        assert_eq!(compare("3.0", "3.0.0"), Equal);
        assert_eq!(compare("3.0.2", "3.0.4"), Less);
    }

    #[test]
    fn compare_ignores_junk() {
        assert_eq!(compare("v24.1.0.1234", "24.1"), Greater);
        assert_eq!(compare("1.2-beta", "1.2"), Equal);
        assert_eq!(compare("", "0"), Equal);
    }
}
