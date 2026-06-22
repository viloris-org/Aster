//! Opaque runtime identifiers.

macro_rules! id_type {
    ($name:ident) => {
        #[doc = concat!("Opaque ", stringify!($name), " value.")]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u128);

        impl $name {
            /// Creates an ID from raw bits.
            pub const fn from_u128(value: u128) -> Self {
                Self(value)
            }

            /// Returns raw ID bits for serialization boundaries.
            pub const fn as_u128(self) -> u128 {
                self.0
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(&format!("{:032x}", self.0))
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct IdVisitor;

                impl serde::de::Visitor<'_> for IdVisitor {
                    type Value = $name;

                    fn expecting(
                        &self,
                        formatter: &mut std::fmt::Formatter<'_>,
                    ) -> std::fmt::Result {
                        formatter.write_str("a 128-bit ID as 32-character hex string or integer")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        let normalized = value.strip_prefix("0x").unwrap_or(value);
                        u128::from_str_radix(normalized, 16)
                            .or_else(|_| normalized.parse::<u128>())
                            .map($name::from_u128)
                            .map_err(E::custom)
                    }

                    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        Ok($name::from_u128(value as u128))
                    }

                    fn visit_u128<E>(self, value: u128) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        Ok($name::from_u128(value))
                    }
                }

                deserializer.deserialize_any(IdVisitor)
            }
        }
    };
}

id_type!(EntityId);
id_type!(AssetId);
id_type!(ResourceId);
