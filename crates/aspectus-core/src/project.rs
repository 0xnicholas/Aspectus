use serde::{Deserialize, Serialize};
use sqlx::Type;

use crate::error::CoreError;

/// Ecosystem projects known to Aspectus.
///
/// Each project has exactly one Service Token for calling `/introspect`.
/// New projects require a code change (ADR-010).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[sqlx(type_name = "project", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Project {
    Pandaria,
    Tavern,
    Emerald,
    Constell,
    Tokencamp,
    Heirloom,
}

impl std::fmt::Display for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Pandaria => "pandaria",
            Self::Tavern => "tavern",
            Self::Emerald => "emerald",
            Self::Constell => "constell",
            Self::Tokencamp => "tokencamp",
            Self::Heirloom => "heirloom",
        };
        write!(f, "{s}")
    }
}

impl std::str::FromStr for Project {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pandaria" => Ok(Self::Pandaria),
            "tavern" => Ok(Self::Tavern),
            "emerald" => Ok(Self::Emerald),
            "constell" => Ok(Self::Constell),
            "tokencamp" => Ok(Self::Tokencamp),
            "heirloom" => Ok(Self::Heirloom),
            other => Err(CoreError::InvalidProject(other.to_owned())),
        }
    }
}
