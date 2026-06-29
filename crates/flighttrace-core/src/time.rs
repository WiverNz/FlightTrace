//! Timestamp newtype with RFC 3339 serde representation.
//!
//! FlightTrace stores all timestamps in UTC and serializes them as RFC 3339
//! strings on the wire. Using a newtype around [`time::OffsetDateTime`] gives
//! every field that holds a moment-in-time the same JSON shape without per
//! field `#[serde(with = ...)]` annotations.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

/// UTC instant serialized as an RFC 3339 string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Timestamp(pub OffsetDateTime);

impl Timestamp {
    /// Wrap an [`OffsetDateTime`] as a `Timestamp`.
    pub const fn new(inner: OffsetDateTime) -> Self {
        Self(inner)
    }

    /// Return the underlying [`OffsetDateTime`].
    pub const fn into_inner(self) -> OffsetDateTime {
        self.0
    }
}

impl From<OffsetDateTime> for Timestamp {
    fn from(value: OffsetDateTime) -> Self {
        Self(value)
    }
}

impl From<Timestamp> for OffsetDateTime {
    fn from(value: Timestamp) -> Self {
        value.0
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let rendered = self.0.format(&Rfc3339).map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(&rendered)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        OffsetDateTime::parse(&raw, &Rfc3339)
            .map(Timestamp)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn round_trips_through_json() {
        let original = Timestamp(datetime!(2025-01-01 00:00:12.500 UTC));
        let rendered = serde_json::to_string(&original).expect("serialize timestamp");
        assert_eq!(rendered, "\"2025-01-01T00:00:12.5Z\"");
        let parsed: Timestamp = serde_json::from_str(&rendered).expect("deserialize timestamp");
        assert_eq!(parsed, original);
    }
}
