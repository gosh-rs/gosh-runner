// [[file:../runners.note::*imports][imports:1]]
use crate::common::*;
// imports:1 ends here

// [[file:../runners.note::*process group][process group:1]]
mod process_group {
    use super::*;

    macro_rules! setsid {
        () => {{
            // Don't check the error of setsid because it fails if we're the
            // process leader already. We just forked so it shouldn't return
            // error, but ignore it anyway.
            nix::unistd::setsid().ok();
            Ok(())
        }};
    }

    /// Create child process in new session
    pub trait ProcessGroupExt<T> {
        fn new_process_group(&mut self) -> &mut T;
    }

    impl ProcessGroupExt<std::process::Command> for std::process::Command {
        fn new_process_group(&mut self) -> &mut std::process::Command {
            use std::os::unix::process::CommandExt;

            unsafe {
                self.pre_exec(|| setsid!());
            }
            self
        }
    }

    impl ProcessGroupExt<tokio::process::Command> for tokio::process::Command {
        fn new_process_group(&mut self) -> &mut tokio::process::Command {
            unsafe {
                self.pre_exec(|| setsid!());
            }
            self
        }
    }
}
// process group:1 ends here

// [[file:../runners.note::*timestamp][timestamp:1]]
use chrono::*;

/// Convert unix timestamp in floating point seconds to `DateTime`
fn float_unix_timestamp_to_date_time(t: f64) -> DateTime<Utc> {
    let nano = t.fract() * 1_000_000_000f64;
    Utc.timestamp(t.trunc() as i64, nano.round() as u32)
}
// timestamp:1 ends here

// [[file:../runners.note::*process][process:1]]
mod impl_process_procfs {
    use super::*;
    use procfs::process;

    /// Represents a process
    #[derive(Debug, Clone)]
    pub struct Process {
        inner: process::Process,
        create_time: u64,
    }

    impl Process {
        /// Construct from process ID
        pub fn from_pid(pid: u32) -> Result<Self> {
            let p = process::Process::new(pid as i32)?;
            let create_time = p.stat()?.starttime;
            let p = Self { inner: p, create_time };
            Ok(p)
        }

        /// Return the system assigned process ID
        pub fn id(&self) -> u32 {
            self.inner.pid as u32
        }

        /// Test if process is alive
        pub fn is_alive(&self) -> bool {
            self.inner.is_alive()
        }

        /// Get the session Id of the process.
        pub fn get_session_id(&self) -> Result<u32> {
            let stat = self.inner.stat()?;
            Ok(stat.session as u32)
        }

        /// Get the working directory of the process.
        pub fn get_cwd(&self) -> Result<PathBuf> {
            let d = self.inner.cwd()?;
            Ok(d)
        }

        /// Return actual path of the executed command for the process.
        pub fn get_exe(&self) -> Result<PathBuf> {
            let exe = self.inner.exe()?;
            Ok(exe)
        }

        /// Returns the complete command line for the process.
        pub fn get_cmdline(&self) -> Result<Vec<String>> {
            let cmdline = self.inner.cmdline()?;
            Ok(cmdline)
        }

        /// Test if process is paused
        pub fn is_paused(&self) -> bool {
            if let Ok(status) = self.inner.status() {
                dbg!(status.state) == "T (stopped)"
            } else {
                false
            }
        }

        /// Send signal to the process.
        pub fn send_signal(&self, signal: &str) -> Result<()> {
            use nix::sys::signal::Signal;

            let signal: Signal = signal
                .parse()
                .with_context(|| format!("invalid signal name: {}", signal))?;
            nix::sys::signal::kill(nix::unistd::Pid::from_raw(self.inner.pid), signal)?;
            Ok(())
        }

        /// Test if is the same process, useful for avoiding re-used process ID
        pub fn is_same(&self, p: &Process) -> bool {
            self.create_time == p.create_time && self.inner.pid == p.inner.pid
        }
    }

    /// Return processes with the same session ID
    pub fn get_processes_in_session(id: u32) -> Result<Vec<Process>> {
        let all = process::all_processes()?
            .into_iter()
            .filter_map(|p| {
                if let Ok(stat) = p.stat() {
                    if stat.session == id as i32 {
                        return Some(Process {
                            inner: p,
                            create_time: stat.starttime,
                        });
                    }
                }
                None
            })
            .collect();
        Ok(all)
    }
}
// process:1 ends here

// [[file:../runners.note::*session][session:1]]
mod session {
    use super::*;

    /// Handle a group of processes in the same session.
    #[derive(Debug, Clone)]
    pub struct SessionHandler {
        process: Option<Process>,
    }

    /// Create child process in new session
    pub trait SpawnSessionExt<T> {
        fn spawn_session(&mut self) -> Result<(T, SessionHandler)>;
    }

    impl SessionHandler {
        fn from(id: u32) -> Self {
            let process = Process::from_pid(id).ok();
            Self { process }
        }

        fn id(&self) -> Option<u32> {
            self.process.as_ref().map(|p| p.id())
        }

        /// Send signal to all processes in the session
        fn send_signal(&self, signal: &str) -> Result<()> {
            if let Some(p_old) = &self.process {
                let id = p_old.id();
                let p_now = Process::from_pid(id)?;
                // send signal only when the session leader still exists and
                // look like the same as created before (PID could be reused)
                if p_now.is_same(p_old) {
                    signal_processes_by_session_id(id, signal)?;
                } else {
                    warn!("Send signal {} to a resued process {}", signal, id);
                }
            } else {
                bail!("no session leader!");
            }
            Ok(())
        }

        /// Return the processes in the session.
        pub fn get_processes(&self) -> Result<Vec<Process>> {
            if let Some(id) = self.id() {
                let p = get_processes_in_session(id)?;
                Ok(p)
            } else {
                bail!("session is not alive");
            }
        }

        /// Pause all processes in the session.
        pub fn pause(&self) -> Result<()> {
            debug!("pause session {:?}", self.id());
            self.send_signal("SIGSTOP")?;
            Ok(())
        }
        /// Resume processes in the session.
        pub fn resume(&self) -> Result<()> {
            debug!("resume session {:?}", self.id());
            self.send_signal("SIGCONT")?;
            Ok(())
        }
        /// Terminate processes in the session.
        pub fn terminate(&self) -> Result<()> {
            debug!("termate session {:?}", self.id());
            // If process was paused, terminate it directly could result a deadlock or zombie.
            self.send_signal("SIGCONT")?;
            gut::utils::sleep(0.2);
            self.send_signal("SIGTERM")?;
            Ok(())
        }
    }

    impl SpawnSessionExt<std::process::Child> for std::process::Command {
        fn spawn_session(&mut self) -> Result<(std::process::Child, SessionHandler)> {
            let child = self.new_process_group().spawn()?;
            let id = child.id();
            let session_handler = SessionHandler::from(id);
            Ok((child, session_handler))
        }
    }

    impl SpawnSessionExt<tokio::process::Child> for tokio::process::Command {
        fn spawn_session(&mut self) -> Result<(tokio::process::Child, SessionHandler)> {
            let child = self.new_process_group().spawn()?;
            if let Some(id) = child.id() {
                let session_handler = SessionHandler::from(id);
                Ok((child, session_handler))
            } else {
                bail!("Spawned child process died?");
            }
        }
    }
}
// session:1 ends here

// [[file:../runners.note::*pub][pub:1]]
/// Signal all child processes in session `sid`
pub(crate) fn signal_processes_by_session_id(sid: u32, signal: &str) -> Result<()> {
    info!("Send signal {} to processes in session {}", signal, sid);

    let pp = get_processes_in_session(sid)?;
    info!("found {} processes in session {}", pp.len(), sid);
    for p in pp {
        p.send_signal(signal)?;
    }

    Ok(())
}

pub use impl_process_procfs::{get_processes_in_session, Process};
pub use process_group::ProcessGroupExt;
pub use session::{SessionHandler, SpawnSessionExt};
// pub:1 ends here

// [[file:../runners.note::*test][test:1]]
#[test]
fn test_spawn_session() -> Result<()> {
    use std::process::Command;

    gut::cli::setup_logger_for_test();

    let mut command = Command::new("scripts/test_runner.sh");
    let (mut child, session_handler) = command.spawn_session()?;

    session_handler.pause()?;
    session_handler.resume()?;
    gut::utils::sleep(0.5);
    session_handler.terminate()?;
    gut::utils::sleep(0.2);
    assert!(child.try_wait().is_ok());

    Ok(())
}
// test:1 ends here
