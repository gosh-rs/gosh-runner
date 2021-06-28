// [[file:../../runners.note::*imports][imports:1]]
use gosh_core::gut::prelude::*;
#[cfg(feature = "client")]
use gosh_runner::client_enter_main;
// imports:1 ends here

// [[file:../../runners.note::*main][main:1]]
fn main() -> Result<()> {
    #[cfg(feature = "client")]
    client_enter_main()?;
    Ok(())
}
// main:1 ends here
