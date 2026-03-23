//! Profile import module.
//!
//! This module provides functionality for importing OpenVPN profiles,
//! including validation, asset processing, and persistence.
//!
//! # Architecture
//!
//! The import process is split into several specialized submodules:
//!
//! - [`validator`] - Pure validation logic for profile directives
//! - [`assets`] - Asset path resolution, copying, and hashing
//! - [`report`] - Import report assembly
//! - [`importer`] - Main `ProfileImporter` coordinator
//!
//! # Example
//!
//! ```ignore
//! use openwrap_core::profiles::import::{ProfileImporter, ImportProfileRequest};
//!
//! let importer = ProfileImporter::new(paths, repository);
//! let response = importer.import_profile(ImportProfileRequest {
//!     source_path: "/path/to/profile.ovpn".into(),
//!     display_name: Some("My VPN".into()),
//!     allow_warnings: false,
//! })?;
//! ```

mod assets;
mod importer;
mod report;
mod validator;

#[cfg(test)]
mod tests;

// Re-export public API
pub use importer::{ImportProfileRequest, ImportProfileResponse, ProfileImporter};
