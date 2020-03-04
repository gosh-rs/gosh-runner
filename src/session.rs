// [[file:~/Workspace/Programming/gosh-rs/runner/runners.note::*imports][imports:1]]
use crate::common::*;

use tokio::prelude::*;
use tokio::process::Command;
use tokio::signal::ctrl_c;
use tokio::time::{delay_for, Duration};
// imports:1 ends here

// [[file:~/Workspace/Programming/gosh-rs/runner/runners.note::*base][base:1]]
/// Manage process session
#[derive(Debug)]
pub struct Session {
    /// Session ID
    sid: Option<u32>,

    /// Arguments that will be passed to `program`
    rest: Vec<String>,

    /// Job timeout in seconds
    timeout: Option<u64>,

    /// The external command
    command: Command,

    /// Stdin input bytes
    stdin_bytes: Vec<u8>,

    cmd_output: Option<std::process::Output>,
}

impl Session {
    /// Create a new session.
    pub fn new(program: &str) -> Self {
        // setsid -w external-cmd
        let mut command = Command::new("setsid");
        // do not kill command when `Child` drop
        command.arg("-w").arg(program).kill_on_drop(false);

        Self {
            command,
            sid: None,
            timeout: None,
            rest: vec![],
            stdin_bytes: vec![],
            cmd_output: None,
        }
    }

    /// Set program argument
    pub fn arg<S: AsRef<str>>(mut self, arg: S) -> Self {
        self.command.arg(arg.as_ref());
        self
    }

    /// Return a mutable reference to internal `Command` struct.
    pub(crate) fn command(&mut self) -> &mut Command {
        &mut self.command
    }

    /// Set program running timeout.
    pub fn timeout(mut self, t: u64) -> Self {
        self.timeout = Some(t);
        self
    }

    /// Terminate child processes in a session.
    pub fn terminate(&mut self) -> Result<()> {
        self.signal("SIGTERM")
    }

    /// Kill processes in a session.
    pub fn kill(&mut self) -> Result<()> {
        self.signal("SIGKILL")
    }

    /// Resume processes in a session.
    pub fn resume(&mut self) -> Result<()> {
        self.signal("SIGCONT")
    }

    /// Pause processes in a session.
    pub fn pause(&mut self) -> Result<()> {
        self.signal("SIGSTOP")
    }

    /// Use bytes or a string as stdin
    /// A worker thread will write the input at runtime.
    pub fn stdin_bytes<T: Into<Vec<u8>>>(&mut self, bytes: T) -> &mut Self {
        self.stdin_bytes = bytes.into();
        self
    }

    /// send signal to child processes
    pub fn signal(&mut self, sig: &str) -> Result<()> {
        if let Some(sid) = self.sid {
            crate::process::signal_processes_by_session_id(sid, sig)?;
        } else {
            debug!("process not started yet");
        }
        Ok(())
    }
}
// base:1 ends here

// [[file:~/Workspace/Programming/gosh-rs/runner/runners.note::*core][core:1]]
impl Session {
    async fn start(&mut self) -> Result<()> {
        // pipe stdin_bytes to program's stdin
        let mut child = self.command.stdin(std::process::Stdio::piped()).spawn()?;
        self.sid = Some(child.id());

        let mut stdin = child
            .stdin
            .take()
            .expect("child did not have a handle to stdin");
        stdin
            .write_all(&self.stdin_bytes)
            .await
            .expect("Failed to write to stdin");

        let cmd_output = self.command.output();

        // running timeout for 2 days
        let default_timeout = 3600 * 2;
        let timeout = delay_for(Duration::from_secs(self.timeout.unwrap_or(default_timeout)));
        // user interruption
        let ctrl_c = tokio::signal::ctrl_c();

        let v: usize = loop {
            tokio::select! {
                _ = timeout => {
                    warn!("operation timed out");
                    break 1;
                }
                _ = ctrl_c => {
                    warn!("user interruption");
                    break 1;
                }
                o = cmd_output => {
                    info!("operation completed");
                    match o {
                        Ok(o) => {
                            self.cmd_output = Some(o);
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
            info!("program running interrupted.");
            self.kill()?;
        } else {
            info!("checking orphaned processes ...");
            self.kill()?;
        }

        Ok(())
    }
}
// core:1 ends here

// [[file:~/Workspace/Programming/gosh-rs/runner/runners.note::*pub][pub:1]]
impl Session {
    /// Run command with session manager.
    pub fn run(&mut self) -> Result<std::process::Output> {
        let mut rt = tokio::runtime::Runtime::new().context("tokio runtime failure")?;
        rt.block_on(self.start())?;

        self.cmd_output.take().ok_or(format_err!("no cmd output"))
    }
}
// pub:1 ends here

// [[file:~/Workspace/Programming/gosh-rs/runner/runners.note::*cli][cli:1]]
use structopt::*;

/// A local runner that can make graceful exit
#[derive(StructOpt, Debug, Default)]
struct Runner {
    /// The program to be run.
    #[structopt(name = "program")]
    program: String,

    /// Job timeout in seconds
    #[structopt(long = "timeout", short = "t")]
    timeout: Option<u64>,

    /// Arguments that will be passed to `program`
    #[structopt(raw = true)]
    rest: Vec<String>,
}

pub fn enter_main() {
    let args = Runner::from_args();

    let mut session = Session::new(&args.program).timeout(args.timeout.unwrap_or(50));
    let o = session.run();
    dbg!(o);
}
// cli:1 ends here

// [[file:~/Workspace/Programming/gosh-rs/runner/runners.note::*test][test:1]]
#[test]
fn test_tokio() -> Result<()> {
    let mut session = Session::new("sleep").arg("10").timeout(2);
    session.run()?;

    Ok(())
}
// test:1 ends here
