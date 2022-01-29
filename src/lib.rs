// [[file:../runners.note::16bab924][16bab924]]
use gosh_core::*;
use gut::prelude::*;

use std::path::{Path, PathBuf};

/// Return current timestamp string
fn timestamp_now() -> String {
    use chrono::prelude::*;
    let now: DateTime<Local> = Local::now();
    format!("{}", now)
}
// 16bab924 ends here

// [[file:../runners.note::*mods][mods:1]]
#[cfg(feature = "client")]
mod client;
mod local;
mod server;
mod session;
// mods:1 ends here

// [[file:../runners.note::d8e4f605][d8e4f605]]
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

// [[file:../runners.note::c6e9d2bf][c6e9d2bf]]
#[cfg(feature = "adhoc")]
/// Documentation for local development
pub mod docs {
    macro_rules! export_doc {
        ($l:ident) => {
            pub mod $l {
                pub use crate::$l::*;
            }
        };
    }

    export_doc!(job);
    export_doc!(process);

    #[cfg(feature = "client")]
    export_doc!(client);
}
// c6e9d2bf ends here
