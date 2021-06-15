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

// [[file:../runners.note::*signal][signal:1]]
use nix::sys::signal::Signal;
#[test]
fn test_unix_signal() {
    let s: Signal = "SIGINT".parse().unwrap();
}
// signal:1 ends here

// [[file:../runners.note::*timestamp][timestamp:1]]
use chrono::*;

/// Convert unix timestamp in floating point seconds to `DateTime`
fn float_unix_timestamp_to_date_time(t: f64) -> DateTime<Utc> {
    let nano = t.fract() * 1_000_000_000f64;
    Utc.timestamp(t.trunc() as i64, nano.round() as u32)
}
// timestamp:1 ends here

// [[file:../runners.note::*unique process][unique process:1]]
use std::collections::HashSet;
use std::time::Duration;

#[derive(Clone, PartialEq, Eq, Hash, Copy, Debug)]
pub(crate) struct UniqueProcessId(u32, Duration);

impl UniqueProcessId {
    /// construct from pid. return error if the process `pid` not alive.
    fn from_pid(pid: u32) -> Result<Self> {
        if let Ok(p) = psutil::process::Process::new(pid) {
            if p.is_running() {
                return Ok(Self::from_process(p));
            }
        }
        bail!("invalid pid: {}", pid)
    }

    /// construct from psutil `Process` struct (1.x branch only)
    fn from_process(p: psutil::process::Process) -> Self {
        Self(p.pid(), p.create_time())
    }

    /// Process Id
    pub fn pid(&self) -> u32 {
        self.0
    }
}
// unique process:1 ends here

// [[file:../runners.note::*impl/psutil][impl/psutil:1]]
/// Find child processes using psutil (without using shell commands)
///
/// # Reference
///
/// https://github.com/borntyping/rust-psutil/blob/master/examples/ps.rs
fn get_child_processes_by_session_id(sid: u32) -> Result<HashSet<UniqueProcessId>> {
    // for Process::procfs_stat method
    use psutil::process::os::linux::ProcessExt;

    let child_processes = psutil::process::pids()?
        .into_iter()
        .filter_map(|pid| psutil::process::Process::new(pid).ok())
        .filter_map(|p| p.procfs_stat().ok().map(|s| (p, s)))
        .filter_map(|(p, s)| {
            if s.session as u32 == sid {
                Some(UniqueProcessId::from_process(p))
            } else {
                None
            }
        })
        .collect();

    Ok(child_processes)
}

/// Signal child processes by session id
///
/// Note: currently, psutil has no API for kill with signal other than SIGKILL
///
fn impl_signal_processes_by_session_id(sid: u32, signal: &str) -> Result<()> {
    let signal: Signal = signal.parse().with_context(|| format!("invalid signal: {}", signal))?;

    let child_processes = get_child_processes_by_session_id(sid)?;
    debug!("found {} child processes in session {} ", child_processes.len(), sid);

    for child in child_processes {
        trace!("{:?}", child);
        // refresh process id from /proc before kill
        // check starttime to avoid re-used pid
        let pid = child.pid();
        if let Ok(process) = UniqueProcessId::from_pid(pid) {
            if process == child {
                nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), signal)?;
                debug!("process {} was killed", pid);
            } else {
                warn!("process id {} was reused?", pid);
            }
        } else {
            info!("process {} already terminated.", pid);
        }
    }

    Ok(())
}
// impl/psutil:1 ends here

// [[file:../runners.note::*session][session:1]]
mod session {
    use super::*;

    pub trait ProcessSessionId {
        fn get_session_id(&self) -> Option<u32>;
    }

    /// Handle a group of processes in the same session.
    pub struct SessionHandler {
        process: Option<UniqueProcessId>,
    }

    impl SessionHandler {
        fn from(p: impl ProcessSessionId) -> Self {
            let process = p.get_session_id().and_then(|id| UniqueProcessId::from_pid(id).ok());

            Self { process }
        }

        /// Send signal to all processes in the session
        fn send_signal(&self, signal: &str) -> Result<()> {
            if let Some(p_old) = &self.process {
                let id = p_old.pid();
                let p_now = UniqueProcessId::from_pid(id)?;
                // send signal when the session leader still exists and look
                // like the same as created before
                if p_now == *p_old {
                    duct::cmd!("pkill", "--signal", signal, "-s", p_now.pid().to_string())
                        .unchecked()
                        .run()
                        .context("send signal using pkill")?;
                } else {
                    warn!("Send signal {} to a resued process {}", signal, id);
                }
            } else {
                bail!("no session leader!");
            }
            Ok(())
        }

        /// Pause all processes in the session.
        pub fn pause(&self) -> Result<()> {
            self.send_signal("SIGSTOP")?;
            Ok(())
        }
        /// Resume processes in the session.
        pub fn resume(&self) -> Result<()> {
            self.send_signal("SIGCONT")?;
            Ok(())
        }
        /// Terminate processes in the session.
        pub fn terminate(&self) -> Result<()> {
            // If process was paused, terminate it directly could result a deadlock or zombie.
            self.send_signal("SIGCONT")?;
            gut::utils::sleep(0.2);
            self.send_signal("SIGTERM")?;
            Ok(())
        }
    }

    impl ProcessSessionId for std::process::Child {
        fn get_session_id(&self) -> Option<u32> {
            self.id().into()
        }
    }

    impl ProcessSessionId for tokio::process::Child {
        fn get_session_id(&self) -> Option<u32> {
            self.id()
        }
    }
}
// session:1 ends here

// [[file:../runners.note::*pub][pub:1]]
/// Signal all child processes in session `sid`
pub fn signal_processes_by_session_id(sid: u32, signal: &str) -> Result<()> {
    info!("killing session {} with signal {}", sid, signal);
    impl_signal_processes_by_session_id(sid, signal)
}

pub use process_group::ProcessGroupExt;
pub use session::SessionHandler;
// pub:1 ends here

// [[file:../runners.note::*test][test:1]]

// test:1 ends here
