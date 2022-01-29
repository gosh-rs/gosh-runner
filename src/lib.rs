// [[file:../runners.note::*mods][mods:1]]
#[cfg(feature = "client")]
mod client;
mod local;
mod server;
mod session;
// mods:1 ends here

// [[file:../runners.note::d8e4f605][d8e4f605]]
// shared imports between mods
mod common {
    // FIXME: remove
    pub use gosh_core::*;

    pub use gut::prelude::*;
    pub use std::path::{Path, PathBuf};

    /// Return current timestamp string
    pub fn timestamp_now() -> String {
        use chrono::prelude::*;
        let now: DateTime<Local> = Local::now();
        format!("{}", now)
    }
}
use common::*;

// for command line binaries
#[cfg(feature = "client")]
pub use client::enter_main as client_enter_main;
#[cfg(feature = "client")]
pub use client::Client;
pub use local::ctrlc_enter_main;
pub use local::enter_main as local_enter_main;
pub use server::enter_main as server_enter_main;

/// Some extension traits
pub mod prelude {
    pub use crate::process::SpawnSessionExt;
}

pub mod cli;
pub mod job;
pub mod process;
pub mod stop;
// d8e4f605 ends here

// [[file:../runners.note::*docs][docs:1]]
#[cfg(feature = "adhoc")]
/// Documentation for local development
pub mod docs {
    pub use crate::job::*;
    pub use crate::process::*;
}
// docs:1 ends here
