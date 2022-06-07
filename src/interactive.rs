// [[file:../runners.note::83fae954][83fae954]]
//! Interaction with child process's stdin for computation and read result from
//! stdout in a simple line based style.
// 83fae954 ends here

// [[file:../runners.note::173702c1][173702c1]]
use super::*;
use process::{Session, SessionHandler, SpawnSessionExt};

use std::process::{Child, Command};

type InnerSession = Session<Child>;
// 173702c1 ends here

// [[file:../runners.note::b6cd06ef][b6cd06ef]]
mod stdin {
    use super::*;
    use std::io::Write;
    use std::process::ChildStdin;

    pub struct StdinWriter {
        stdin: ChildStdin,
    }

    impl StdinWriter {
        pub fn new(stdin: ChildStdin) -> Self {
            Self { stdin }
        }

        /// Write `input` into self's stdin
        pub fn write(&mut self, input: &str) -> Result<()> {
            self.stdin.write_all(input.as_bytes())?;
            self.stdin.flush()?;
            trace!("wrote stdin done: {} bytes", input.len());

            Ok(())
        }
    }
}
// b6cd06ef ends here

// [[file:../runners.note::0069c099][0069c099]]
mod stdout {
    use super::*;

    use std::io::{self, BufRead, Write};
    use std::process::ChildStdout;

    pub struct StdoutReader {
        reader: io::Lines<io::BufReader<ChildStdout>>,
    }

    impl StdoutReader {
        pub fn new(stdout: ChildStdout) -> Self {
            let reader = io::BufReader::new(stdout).lines();
            Self { reader }
        }

        /// Read stdout until finding a line containing the `pattern`
        pub fn read_until(&mut self, pattern: &str) -> Result<String> {
            trace!("Read stdout until finding pattern: {:?}", pattern);
            let mut text = String::new();
            while let Some(line) = self.reader.next() {
                let line = line.context("invalid encoding?")?;
                writeln!(&mut text, "{}", line)?;
                if line.contains(&pattern) {
                    trace!("found pattern: {:?}", pattern);
                    return Ok(text);
                }
            }
            bail!("Expected pattern not found: {:?}!", pattern);
        }
    }
}
// 0069c099 ends here

// [[file:../runners.note::55863db6][55863db6]]
/// Interactive with a long running process communicated in a simple line based
/// style.
///
/// Feed child process's stdin for starting computation and read result from its
/// stdout.
pub struct InteractiveSession {
    command: Option<Command>,
    stream0: Option<stdin::StdinWriter>,
    stream1: Option<stdout::StdoutReader>,
    session_handler: Option<SessionHandler>,
    // the dropping order could be important here
    inner: Option<InnerSession>,
}
// 55863db6 ends here

// [[file:../runners.note::4b7494ae][4b7494ae]]
impl InteractiveSession {
    /// Create a new interactive session for `command`
    pub fn new(command: Command) -> Self {
        Self {
            command: command.into(),
            stream0: None,
            stream1: None,
            inner: None,
            session_handler: None,
        }
    }

    /// Interact with child process's stdin using `input` and return stdout
    /// read-in until the line matching `read_pattern`. The `spawn` method
    /// should be called before `interact`.
    ///
    /// # Panics
    ///
    /// * panic if child process is not spawned yet.
    pub fn interact(&mut self, input: &str, read_pattern: &str) -> Result<String> {
        // ignore interaction with empty input
        let stdin = self.stream0.as_mut().expect("interactive session stdin");
        if !input.is_empty() {
            trace!("send input for child process's stdin ({} bytes)", input.len());
            stdin.write(input)?;
        }

        trace!("send read pattern for child process's stdout: {:?}", read_pattern);
        let stdout = self.stream1.as_mut().unwrap();
        let txt = stdout.read_until(read_pattern)?;
        if txt.is_empty() {
            bail!("Got nothing for pattern: {}", read_pattern);
        }
        Ok(txt)
    }

    /// Spawn child process in new session (progress group), and return a
    /// `SessionHandler` that can be shared between threads.
    pub fn spawn(&mut self) -> Result<SessionHandler> {
        use std::process::Stdio;

        // we want to interact with child process's stdin and stdout
        let mut command = self.command.take().unwrap();
        let mut session = command.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn_session()?;
        self.stream0 = stdin::StdinWriter::new(session.child.stdin.take().unwrap()).into();
        self.stream1 = stdout::StdoutReader::new(session.child.stdout.take().unwrap()).into();

        let h = session.handler().clone();
        self.session_handler = h.clone().into();
        // dropping `session` will kill all processes in the session
        self.inner = session.into();
        trace!("start child process in new session: {:?}", h.id());

        Ok(h.clone())
    }

    /// Create a session handler for shared between threads.
    pub fn get_handler(&self) -> Option<SessionHandler> {
        self.session_handler.clone()
    }
}
// 4b7494ae ends here

// [[file:../runners.note::c0e24463][c0e24463]]
#[test]
fn test_interactive_session() -> Result<()> {
    let mut cmd = Command::new("bash");
    let script = "echo hello; while read -r xx; do sleep 1; echo output for $xx; echo hello; done";
    cmd.arg("-c").arg(script);

    let mut s = InteractiveSession::new(cmd);
    let h = s.spawn()?;
    let o = s.interact("", "hello")?;
    assert_eq!(o, "hello\n");
    let o = s.interact("pwd\n", "hello")?;
    assert_eq!(o, "output for pwd\nhello\n");

    Ok(())
}
// c0e24463 ends here
