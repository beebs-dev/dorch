use std::time::Duration;

pub mod metrics;
pub mod patch;

pub(crate) mod colors;
pub(crate) mod messages;

mod error;
mod merge;

pub use error::*;

/// The default interval for requeuing a managed resource.
pub(crate) const PROBE_INTERVAL: Duration = Duration::from_secs(30);

/// Name of the kubernetes resource manager.
pub(crate) const MANAGER_NAME: &str = "dorch-operator";
