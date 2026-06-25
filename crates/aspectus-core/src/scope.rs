use std::str::FromStr;

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::{error::CoreError, project::Project};

/// Permission label in `project:resource:action` format (ADR-006).
///
/// Examples: `pandaria:session:create`, `constell:agent:read`.
/// Supports `*` wildcard for whole-segment matching.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Scope {
    pub id: String,
    /// Full scope string, e.g. `"pandaria:session:create"`.
    pub name: String,
    pub description: Option<String>,
}

impl Scope {
    /// Maximum number of scopes that can be attached to a single API key.
    pub const MAX_SCOPES_PER_KEY: usize = 100;

    /// Validate that `name` conforms to `project:resource:action`.
    ///
    /// Rules:
    /// - Exactly three non-empty segments separated by `:`.
    /// - The project segment must be a known [`Project`] variant.
    /// - `resource` and `action` are either `*` or consist of ASCII
    ///   alphanumeric characters, `-`, and `_`.
    pub fn validate(name: &str) -> Result<(), CoreError> {
        let parts: Vec<&str> = name.split(':').collect();
        if parts.len() != 3 {
            return Err(CoreError::InvalidScope(format!(
                "scope '{name}' must have exactly three segments separated by ':'"
            )));
        }
        let project = parts[0];
        let resource = parts[1];
        let action = parts[2];

        if project.is_empty() || resource.is_empty() || action.is_empty() {
            return Err(CoreError::InvalidScope(format!(
                "scope '{name}' contains an empty segment"
            )));
        }

        // Ensure the project segment is a known ecosystem project.
        let _ = Project::from_str(project).map_err(|_| {
            CoreError::InvalidScope(format!(
                "scope '{name}' references unknown project '{project}'"
            ))
        })?;

        Self::validate_segment(resource, "resource", name)?;
        Self::validate_segment(action, "action", name)?;

        Ok(())
    }

    /// Extract the project segment after validation.
    pub fn project(name: &str) -> Result<Project, CoreError> {
        Self::validate(name)?;
        let part = name
            .split(':')
            .next()
            .expect("validated scope has a project segment");
        Project::from_str(part)
    }

    /// Extract the resource segment after validation.
    pub fn resource(name: &str) -> Result<&str, CoreError> {
        Self::validate(name)?;
        Ok(name
            .split(':')
            .nth(1)
            .expect("validated scope has a resource segment"))
    }

    /// Extract the action segment after validation.
    pub fn action(name: &str) -> Result<&str, CoreError> {
        Self::validate(name)?;
        Ok(name
            .split(':')
            .nth(2)
            .expect("validated scope has an action segment"))
    }

    fn validate_segment(seg: &str, label: &str, scope: &str) -> Result<(), CoreError> {
        if seg == "*" {
            return Ok(());
        }
        if seg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            Ok(())
        } else {
            Err(CoreError::InvalidScope(format!(
                "scope '{scope}' has invalid characters in {label} segment '{seg}'"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_scope_passes() {
        Scope::validate("pandaria:session:create").unwrap();
        Scope::validate("constell:agent:read").unwrap();
        Scope::validate("pandaria:session:*").unwrap();
        Scope::validate("tokencamp:*:read").unwrap();
    }

    #[test]
    fn scope_with_underscore_and_dash_passes() {
        Scope::validate("pandaria:customer-data:read-write").unwrap();
    }

    #[test]
    fn invalid_scope_format_fails() {
        assert!(Scope::validate("pandaria").is_err());
        assert!(Scope::validate("pandaria:session").is_err());
        assert!(Scope::validate("pandaria:session:create:extra").is_err());
        assert!(Scope::validate(":session:create").is_err());
        assert!(Scope::validate("pandaria::create").is_err());
    }

    #[test]
    fn unknown_project_scope_fails() {
        assert!(Scope::validate("unknown:session:create").is_err());
        assert!(Scope::validate("tavern:session:create").is_err());
    }

    #[test]
    fn invalid_characters_fail() {
        assert!(Scope::validate("pandaria:session:cre ate").is_err());
        assert!(Scope::validate("pandaria:session:create!").is_err());
    }

    #[test]
    fn scope_project_extractor_works() {
        assert_eq!(
            Scope::project("pandaria:session:create").unwrap(),
            Project::Pandaria
        );
        assert_eq!(
            Scope::project("constell:agent:*").unwrap(),
            Project::Constell
        );
    }

    #[test]
    fn scope_resource_and_action_extractors_work() {
        assert_eq!(
            Scope::resource("pandaria:session:create").unwrap(),
            "session"
        );
        assert_eq!(Scope::action("pandaria:session:create").unwrap(), "create");
    }
}
