use serde::{Deserialize, Serialize};
use sqlx::Type;

use crate::error::CoreError;

/// Ecosystem projects known to Aspectus.
///
/// Each consumer project has exactly one Service Token for calling `/introspect`.
/// New projects require a code change (ADR-010).
///
/// `Aspectus` is a special internal project used for the admin service token
/// that is allowed to call management endpoints (`/tenants`, `/users`, etc.).
/// It is NOT a consumer project and must never be used as an API Key `project`
/// or OAuth2 `client_id`.
///
/// History:
/// - 2026-06-21: Tavern removed. Tavern code has been merged into Pandaria
///   as a subsystem (lives under `pandaria/crates/tavern-*`). Functionality
///   continues, but it no longer appears as a separate ecosystem consumer.
///   The PostgreSQL enum value `tavern` is retained for backwards
///   compatibility with existing rows in `service_tokens` / `api_keys`
///   (migration #15 documents the deprecation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[sqlx(type_name = "project", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Project {
    Pandaria,
    Emerald,
    Constell,
    Tokencamp,
    Heirloom,
    /// Internal admin project. Management API requires a service token
    /// registered under this project.
    Aspectus,
}

impl std::fmt::Display for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Pandaria => "pandaria",
            Self::Emerald => "emerald",
            Self::Constell => "constell",
            Self::Tokencamp => "tokencamp",
            Self::Heirloom => "heirloom",
            Self::Aspectus => "aspectus",
        };
        write!(f, "{s}")
    }
}

impl std::str::FromStr for Project {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pandaria" => Ok(Self::Pandaria),
            "emerald" => Ok(Self::Emerald),
            "constell" => Ok(Self::Constell),
            "tokencamp" => Ok(Self::Tokencamp),
            "heirloom" => Ok(Self::Heirloom),
            "aspectus" => Ok(Self::Aspectus),
            other => Err(CoreError::InvalidProject(other.to_owned())),
        }
    }
}
