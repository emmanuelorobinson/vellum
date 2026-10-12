//! Migrations (Invariant 5: every older format stays readable forever; a newer one opens read-only).
//!
//! A migration works on the JSON of one version and leaves the JSON of the next. It cannot work on the typed
//! document, because the types describe only the newest version. The registry runs the steps in order and
//! sets `meta.formatVersion` after each, so a step cannot forget to.
//!
//! Every migration ships a before and an after fixture, `fixtures/migrations/v{N}-to-v{N+1}/`, and a test
//! checks that the step turns one into the other.

use std::fmt;

use serde_json::Value as Json;
use vellum_types::FORMAT_VERSION;

use crate::CodecError;

/// Why a migration step failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationError(String);

impl MigrationError {
    /// A failure with a reason in plain words.
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for MigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for MigrationError {}

/// One step: the JSON of version [`source_version`](Migration::source_version) becomes the JSON of the next.
pub trait Migration: Send + Sync {
    /// The version this step reads. It writes `source_version() + 1`.
    fn source_version(&self) -> u32;

    /// Rewrites `document`. Leave `meta.formatVersion` alone; the registry sets it.
    fn migrate(&self, document: &mut Json) -> Result<(), MigrationError>;
}

/// Why a registry could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// Versions start at 1.
    NoVersionZero,
    /// A version below the current one has no step leaving it.
    MissingStep {
        /// The version nothing migrates from.
        from: u32,
    },
    /// The steps are not 1→2, 2→3, and so on in order.
    OutOfOrder {
        /// The version the next step should have read.
        expected: u32,
        /// The version it reads.
        found: u32,
    },
    /// A step leaves a version at or past the current one.
    PastCurrent {
        /// The version it leaves.
        from: u32,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoVersionZero => f.write_str("format versions start at 1"),
            Self::MissingStep { from } => write!(f, "no migration leaves format {from}"),
            Self::OutOfOrder { expected, found } => {
                write!(
                    f,
                    "expected the migration from format {expected}, found the one from {found}"
                )
            }
            Self::PastCurrent { from } => write!(
                f,
                "a migration leaves format {from}, which is not below the current one"
            ),
        }
    }
}

impl std::error::Error for RegistryError {}

/// The steps from format 1 up to the current version.
pub struct Migrations {
    current: u32,
    steps: Vec<Box<dyn Migration>>,
}

impl Migrations {
    /// The registry for this build: every migration to [`FORMAT_VERSION`]. There are none yet; the first
    /// format change adds step 1→2 here.
    pub fn standard() -> Self {
        Self::new(FORMAT_VERSION, Vec::new())
            .expect("the standard steps lead to the current version")
    }

    /// A registry reaching `current` by `steps`, which must be 1→2, 2→3, ... in order with none missing.
    pub fn new(current: u32, steps: Vec<Box<dyn Migration>>) -> Result<Self, RegistryError> {
        if current == 0 {
            return Err(RegistryError::NoVersionZero);
        }
        for (index, step) in steps.iter().enumerate() {
            let expected = index as u32 + 1;
            if step.source_version() != expected {
                return Err(RegistryError::OutOfOrder {
                    expected,
                    found: step.source_version(),
                });
            }
            if expected >= current {
                return Err(RegistryError::PastCurrent { from: expected });
            }
        }
        let have = steps.len() as u32 + 1;
        if have < current {
            return Err(RegistryError::MissingStep { from: have });
        }
        Ok(Self { current, steps })
    }

    /// The version this registry brings a file to.
    pub fn current(&self) -> u32 {
        self.current
    }

    /// The steps, in order.
    pub fn steps(&self) -> &[Box<dyn Migration>] {
        &self.steps
    }

    /// Runs one step and sets the version it leads to.
    pub fn apply_step(step: &dyn Migration, document: &mut Json) -> Result<(), MigrationError> {
        step.migrate(document)?;
        let meta = document
            .get_mut("meta")
            .and_then(Json::as_object_mut)
            .ok_or_else(|| MigrationError::new("the file has no meta block"))?;
        meta.insert(
            "formatVersion".to_owned(),
            Json::from(step.source_version() + 1),
        );
        Ok(())
    }

    /// Brings `document` from the version in its meta block up to the current one.
    pub fn upgrade(&self, document: &mut Json) -> Result<(), CodecError> {
        let from = document["meta"]["formatVersion"]
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .ok_or_else(|| CodecError::Invalid {
                path: "meta.formatVersion".to_owned(),
                position: None,
                message: "the format version is not a whole number".to_owned(),
            })?;
        for step in self.steps.iter().skip(from.saturating_sub(1) as usize) {
            Self::apply_step(step.as_ref(), document).map_err(|e| CodecError::Migration {
                from: step.source_version(),
                message: e.to_string(),
            })?;
        }
        Ok(())
    }
}

impl fmt::Debug for Migrations {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let from: Vec<u32> = self.steps.iter().map(|s| s.source_version()).collect();
        f.debug_struct("Migrations")
            .field("current", &self.current)
            .field("steps_from", &from)
            .finish()
    }
}
