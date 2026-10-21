//! A string wrapper that cannot be printed by accident.

use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    /// The only way to read the value. Grep for `expose()` when auditing.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(***)")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

impl<'de> serde::Deserialize<'de> for Secret {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d).map(Secret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn never_formats_value() {
        let s = Secret::new("ya29.super-secret");
        assert_eq!(format!("{s:?} {s}"), "Secret(***) ***");
        assert_eq!(s.expose(), "ya29.super-secret");
    }
}
