// [[file:../runners.note::809ad587][809ad587]]
//! A simple file based handler for user interruption.
use super::*;
use std::path::PathBuf;

/// A simple file based user interruption handler: return Err if STOP file
/// exits.
pub struct StopFileHandler {
    stop_file: PathBuf,
}

impl StopFileHandler {
    pub fn new() -> Self {
        let stop_file = PathBuf::from("STOP");
        if stop_file.exists() {
            println!("Removing existing STOP file ...");
            let _ = std::fs::remove_file(&stop_file);
        }
        Self {
            stop_file: PathBuf::from("STOP"),
        }
    }

    fn is_interrupted(&self) -> bool {
        self.stop_file.exists()
    }

    /// Return error if finding a STOP file.
    pub fn handle_user_interruption(&self) -> Result<()> {
        if self.is_interrupted() {
            bail!("found STOP file, stopping now ...");
        } else {
            Ok(())
        }
    }
}
// 809ad587 ends here
