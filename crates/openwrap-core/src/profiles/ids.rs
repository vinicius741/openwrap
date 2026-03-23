use uuid::Uuid;

macro_rules! uuid_newtype {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = uuid::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Ok(Self(Uuid::parse_str(value)?))
            }
        }
    };
}

uuid_newtype!(ProfileId);
uuid_newtype!(AssetId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_id_new_generates_unique_ids() {
        let id1 = ProfileId::new();
        let id2 = ProfileId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn profile_id_default() {
        let id = ProfileId::default();
        assert_ne!(id.0, uuid::Uuid::nil());
    }

    #[test]
    fn profile_id_display() {
        let id = ProfileId::new();
        assert_eq!(id.to_string(), id.0.to_string());
    }

    #[test]
    fn profile_id_from_str_valid() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let id: ProfileId = uuid_str.parse().unwrap();
        assert_eq!(id.0.to_string(), uuid_str);
    }

    #[test]
    fn profile_id_from_str_invalid() {
        let result: Result<ProfileId, _> = "not-a-uuid".parse();
        assert!(result.is_err());
    }

    #[test]
    fn asset_id_new_generates_unique_ids() {
        let id1 = AssetId::new();
        let id2 = AssetId::new();
        assert_ne!(id1, id2);
    }
}
