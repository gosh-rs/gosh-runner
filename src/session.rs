// [[file:../runners.note::7507fa23][7507fa23]]
//! Manage a group of processes in a session
use super::*;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::signal::ctrl_c;
use tokio::time::{sleep as delay_for, Duration};
// 7507fa23 ends here

// [[file:../runners.note::1520aa92][1520aa92]]
use crate::process::SpawnSessionExt;

/// Manage a group of processes in a session
pub struct Session {
    /// Arguments that will be passed to `program`
    rest: Vec<String>,

    /// Job timeout in seconds
    timeout: Option<u32>,

    /// The external command
    command: Command,
}

impl Session {
    /// Create a new session.
    pub fn new(program: &str) -> Self {
        let mut command = Command::new(program);
        Self {
            command,
            timeout: None,
            rest: vec![],
        }
    }

    /// Adds multiple arguments to pass to the program.
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        self.command.args(args);
        self
    }

    /// Set program argument
    pub fn arg<S: AsRef<str>>(mut self, arg: S) -> Self {
        self.command.arg(arg.as_ref());
        self
    }

    /// Sets the working directory for the child process.
    pub fn dir<P: AsRef<std::path::Path>>(mut self, dir: P) -> Self {
        // FIXME: use absolute path?
        self.command.current_dir(dir);
        self
    }

    /// Inserts or updates an environment variable mapping.
    pub fn env<K, V>(mut self, key: K, val: V) -> Self
    where
        K: AsRef<std::ffi::OsStr>,
        V: AsRef<std::ffi::OsStr>,
    {
        self.command.env(key, val);
        self
    }

    /// Set program running timeout.
    pub fn timeout(mut self, t: u32) -> Self {
        self.timeout = Some(t);
        self
    }
}
// 1520aa92 ends here

// [[file:../runners.note::*core][core:1]]
impl Session {
    async fn start(&mut self) -> Result<()> {
        use crate::process::SpawnSessionExt;

        let mut session = self.command.spawn_session()?;
        // running timeout for 2 days
        let default_timeout = 3600 * 2;
        let timeout = tokio::time::sleep(Duration::from_secs(self.timeout.unwrap_or(default_timeout) as u64));
        tokio::pin!(timeout);
        // user interruption
        let ctrl_c = tokio::signal::ctrl_c();

        let v: usize = loop {
            tokio::select! {
                _ = &mut timeout => {
                    eprintln!("program timed out");
                    break 1;
                }
                _ = ctrl_c => {
                    eprintln!("user interruption");
                    break 1;
                }
                o = session.child.wait() => {
                    println!("program completed");
                    match o {
                        Ok(o) => {
                            dbg!(o);
                        }
                        Err(e) => {
                            error!("cmd error: {:?}", e);
                        }
                    }
                    break 0;
                }
            }
        };

        if v == 1 {
            info!("program was interrupted.");
            // self.kill()?;
        } else {
            info!("checking orphaned processes ...");
            // self.kill()?;
        }
        let pps = session.handler().get_processes()?;
        for p in pps {
            dbg!(p);
        }

        Ok(())
    }

    /// Run command with session manager.
    pub fn run(mut self) -> Result<()> {
        let mut rt = tokio::runtime::Runtime::new().context("tokio runtime failure")?;
        rt.block_on(self.start())?;

        Ok(())
    }
}
// core:1 ends here

// [[file:../runners.note::*test][test:1]]
#[test]
fn test_tokio() -> Result<()> {
    let mut session = Session::new("sleep").arg("10").timeout(1);
    session.run().ok();

    Ok(())
}
// test:1 ends here
