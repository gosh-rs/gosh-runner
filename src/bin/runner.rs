// [[file:~/Workspace/Programming/gosh-rs/runner/runners.note::*imports][imports:1]]
use gosh_core::gut;
use gut::prelude::*;

use structopt::StructOpt;
// imports:1 ends here

// [[file:~/Workspace/Programming/gosh-rs/runner/runners.note::*main][main:1]]
fn main() -> Result<()> {
    gosh_core::gut::cli::setup_logger();
    gosh_runner::session::enter_main();

    Ok(())
}
// main:1 ends here
