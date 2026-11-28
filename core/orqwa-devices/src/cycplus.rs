//! Cycplus trainers. The R200 is a standard FTMS trainer, but not every platform puts its
//! services in the advertisement, so its name identifies it as well (R302).

/// The start of every R200's advertised name; the serial follows it, as in `R200 31260`.
pub const R200_PREFIX: &str = "R200 ";

/// Whether `name` is a Cycplus R200, whatever serial it carries.
#[must_use]
pub fn is_r200(name: &str) -> bool {
    name.get(..R200_PREFIX.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(R200_PREFIX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_r200_is_recognized_by_its_name_and_serial() {
        assert!(is_r200("R200 31260"));
        assert!(is_r200("r200 00001"));
        assert!(!is_r200("R200"));
        assert!(!is_r200("R2000 31260"));
        assert!(!is_r200("CYCPLUS T2 31260"));
    }
}
