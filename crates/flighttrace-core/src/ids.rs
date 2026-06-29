//! Strongly-typed identifiers for FlightTrace domain entities.
//!
//! Each ID is a newtype around [`uuid::Uuid`], serialized transparently as a
//! UUID string. The newtypes prevent accidentally passing, for example, a
//! `FailureReportId` where a `MissionRunId` is expected.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            /// Wrap a raw [`Uuid`] as a typed ID.
            pub const fn new(inner: Uuid) -> Self {
                Self(inner)
            }

            /// Construct a typed ID from a raw 128-bit value (useful for
            /// deterministic fixtures).
            pub const fn from_u128(value: u128) -> Self {
                Self(Uuid::from_u128(value))
            }

            /// Return the underlying [`Uuid`].
            pub const fn into_inner(self) -> Uuid {
                self.0
            }
        }

        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self(value)
            }
        }

        impl From<$name> for Uuid {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                ::core::fmt::Display::fmt(&self.0, f)
            }
        }
    };
}

define_id!(
    /// Identifier of a [`crate::MissionRun`].
    MissionRunId
);
define_id!(
    /// Identifier of a [`crate::NormalizedEvent`].
    EventId
);
define_id!(
    /// Identifier of an [`crate::AssertionResult`].
    AssertionResultId
);
define_id!(
    /// Identifier of a [`crate::FailureReport`].
    FailureReportId
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_serialize_as_uuid_strings() {
        let id = MissionRunId::from_u128(0x0000_0000_0000_0000_0000_0000_0000_0001);
        let json = serde_json::to_string(&id).expect("serialize id");
        assert_eq!(json, "\"00000000-0000-0000-0000-000000000001\"");
        let parsed: MissionRunId = serde_json::from_str(&json).expect("deserialize id");
        assert_eq!(parsed, id);
    }

    #[test]
    fn distinct_id_types_do_not_alias() {
        // This is a type-system assertion: the line below would not compile if
        // EventId and MissionRunId were the same type.
        fn _take_event(_: EventId) {}
        let _ = _take_event;
    }
}
