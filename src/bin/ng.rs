// imports

// [[file:~/Workspace/Programming/gosh-rs/runners/runners.note::*imports][imports:1]]
#![feature(async_await)]
use std::fs;
use std::path::PathBuf;
use structopt::StructOpt;

use tokio;
use tokio::net::TcpStream;
use tokio::prelude::*;

use runners::common::*;
// imports:1 ends here

// base

// [[file:~/Workspace/Programming/gosh-rs/runners/runners.note::*base][base:1]]
/// A local runner that can make graceful exit
#[derive(StructOpt, Debug, Clone)]
pub struct Cmd {
    /// The program to be run.
    #[structopt(name = "program", parse(from_os_str))]
    program: PathBuf,

    /// Job timeout in seconds
    #[structopt(long = "timeout", short = "t")]
    timeout: Option<u64>,

    /// Arguments that will be passed to `program`
    #[structopt(raw = true)]
    args: Vec<String>,
}
// base:1 ends here

// codec

// [[file:~/Workspace/Programming/gosh-rs/runners/runners.note::*codec][codec:1]]
mod codec {
    use std::io;
    use std::path::{Path, PathBuf};
    use std::str;

    use bytes::*;
    // use bytes::{Buf, BufMut, Bytes, BytesMut};
    use tokio::codec::{Decoder, Encoder};

    #[derive(Debug, Clone)]
    pub enum InputChunk {
        Argument(String),
        Environment { key: String, val: String },
        WorkingDir(PathBuf),
        Command(String),
        Heartbeat,
        Stdin(Bytes),
        StdinEOF,
    }

    #[derive(Debug, Clone)]
    pub enum OutputChunk {
        StartReadingStdin,
        Stdout(Bytes),
        Stderr(Bytes),
        Exit(i32),
    }

    const HEADER_SIZE: usize = 5;

    #[derive(Debug)]
    pub struct Codec;

    impl Decoder for Codec {
        type Item = OutputChunk;
        type Error = io::Error;

        fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
            dbg!(&buf);

            // If we have at least a chunk header, decode it to determine how much more we need.
            if buf.len() < HEADER_SIZE {
                return Ok(None);
            }

            let mut header = buf.split_to(HEADER_SIZE).into_buf();
            let length = header.get_u32_be() as usize;

            // If we have the remainder of the chunk, decode and emit it.
            if buf.len() < length {
                return Ok(None);
            }

            let payload = buf.split_to(length).into();
            let chunk_type = match header.get_u8() {
                b'X' => OutputChunk::Exit(0),
                b'1' => OutputChunk::Stdout(payload),
                b'2' => OutputChunk::Stderr(payload),
                b'S' => OutputChunk::StartReadingStdin,
                _ => unimplemented!(),
            };

            Ok(Some(chunk_type))
        }
    }

    impl Encoder for Codec {
        type Item = InputChunk;
        type Error = io::Error;

        ///
        /// Reference
        ///
        /// - http://martiansoftware.com/nailgun/protocol.html
        ///
        fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> io::Result<()> {
            dbg!(&msg);
            use std::os::unix::ffi::OsStrExt;

            // Reserve enough space for the header
            buf.reserve(HEADER_SIZE);

            let mut payload = vec![];
            let chunk_type = match msg {
                InputChunk::Argument(ref args) => {
                    payload.put(args);
                    b'A'
                }
                InputChunk::WorkingDir(path) => {
                    payload.put(path.as_os_str().as_bytes());
                    b'D'
                }
                InputChunk::Environment { key, val } => {
                    payload.put([key, val].join("="));
                    b'E'
                }
                InputChunk::Command(cmd) => {
                    payload.put(cmd);
                    b'C'
                }
                InputChunk::Heartbeat => b'H',
                InputChunk::Stdin(buf) => {
                    payload.put(buf);
                    b'0'
                }
                InputChunk::StdinEOF => b'.',
                _ => unimplemented!(),
            };

            buf.put_u32_be(payload.len() as u32);
            buf.put(chunk_type);
            buf.put(payload);

            Ok(())
        }
    }

    fn msg<T>(message: T) -> Result<Option<T>, io::Error> {
        Ok(Some(message))
    }

    pub fn err(e: &str) -> io::Error {
        io::Error::new(io::ErrorKind::Other, e)
    }

    fn to_string(bytes: &BytesMut) -> Result<String, io::Error> {
        str::from_utf8(bytes)
            .map(|s| s.to_string())
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
}
// codec:1 ends here

// imports

// [[file:~/Workspace/Programming/gosh-rs/runners/runners.note::*imports][imports:1]]
use codec::*;
use tokio::codec::Decoder;
use tokio::prelude::*;
use tokio::sync::mpsc::*;

// use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};
// imports:1 ends here

// base

// [[file:~/Workspace/Programming/gosh-rs/runners/runners.note::*base][base:1]]
type Input = Option<InputChunk>;
type Output = OutputChunk;

/// Stateful object holding the connection to the Nailgun server.
struct NailgunConnection {
    addr: String,
}

impl Default for NailgunConnection {
    fn default() -> Self {
        Self {
            addr: "192.168.0.199:2113".into(),
        }
    }
}

impl NailgunConnection {
    pub fn new(addr: &str) -> Self {
        let addr = addr.into();
        Self {
            addr,
            ..Default::default()
        }
    }
}
// base:1 ends here

// core

// [[file:~/Workspace/Programming/gosh-rs/runners/runners.note::*core][core:1]]
impl NailgunConnection {
    /// Sends the command and environment to the nailgun server, then loops
    /// forever reading the response until the server sends an exit chunk.
    /// Returns the exit value, or raises NailgunException on error.
    fn send_command(&mut self, cmd: Cmd) -> Result<()> {
        // server side stream. None indicates stream termination.
        let (srv_tx, srv_rx) = tokio::sync::mpsc::channel::<Input>(1);

        // client side stream
        let (cli_tx, cli_rx) = tokio::sync::mpsc::channel::<Output>(1);

        // exit signal
        let (ext_tx, ext_rx) = tokio::sync::mpsc::channel::<()>(1);

        // set up server/client stream pipes
        let addr = self.addr.parse()?;

        // build a client
        let client = TcpStream::connect(&addr)
            .and_then(move |sock| {
                println!("server connected.");
                // stream redirection
                setup_handlers(sock, cli_tx, srv_rx, ext_tx);

                // make sure connection is alive.
                send_heartbeat(srv_tx.clone(), ext_rx);

                // request server to run the command
                let p = format!("{}", cmd.program.display());
                send_command_chunks(srv_tx.clone(), &p);

                // client-server communication
                process_responses(cli_rx, srv_tx.clone());

                Ok(())
            })
            .map_err(|e| error!("{}", e));

        tokio::run(client);

        Ok(())
    }
}
// core:1 ends here

// setup

// [[file:~/Workspace/Programming/gosh-rs/runners/runners.note::*setup][setup:1]]
/// setup stream handlers
fn setup_handlers(
    socket: tokio::net::TcpStream,
    cli_tx: Sender<Output>,
    srv_rx: Receiver<Input>,
    ext_tx: Sender<()>,
) {
    let (sink, stream) = Codec.framed(socket).split();

    // input stream handler
    let fut = srv_rx
        .map_err(|e| error!("channel error {}", e))
        .take_while(|item| Ok(item.is_some()))
        .map(Option::unwrap)
        .forward(sink.sink_map_err(|err| error!("srv_rx, sink error: {}", err)))
        .map(|_| {
            println!("send chunk");
        });
    tokio::spawn(fut);

    // output stream handler
    let fut = stream
        .map_err(|e| error!("channel error {}", e))
        .take_while(move |item| match item {
            OutputChunk::Exit(0) => {
                println!("Command done.");
                let tx = ext_tx.clone();
                tx.send(()).wait().unwrap();
                Ok(false)
            }
            OutputChunk::Exit(ecode) => {
                error!("Command failed with status code = {}", ecode);
                let tx = ext_tx.clone();
                tx.send(()).wait().unwrap();
                Ok(false)
            }
            _ => Ok(true),
        })
        .forward(cli_tx.sink_map_err(|err| error!("cli_tx, sink error: {}", err)))
        .map(|_| {
            println!("receive chunk");
        });
    tokio::spawn(fut);
}
// setup:1 ends here

// command

// [[file:~/Workspace/Programming/gosh-rs/runners/runners.note::*command][command:1]]
/// request server to run a command
fn send_command_chunks(tx: Sender<Input>, command: &str) {
    let cwd = InputChunk::WorkingDir("/tmp".into());
    let cmd = InputChunk::Command(command.into());
    tokio::spawn(
        send_chunk(tx, cwd)
            .and_then(move |tx| send_chunk(tx, cmd))
            .map(|_| ())
            .map_err(|e| {
                error!("{}", e);
            }),
    );
}
// command:1 ends here

// heartbeat

// [[file:~/Workspace/Programming/gosh-rs/runners/runners.note::*heartbeat][heartbeat:1]]
/// request server to run a command
fn send_heartbeat(tx: Sender<Input>, shutdown: tokio::sync::mpsc::Receiver<()>) {
    use std::time::Duration;
    use tokio::timer::Interval;

    // The stream of received `usize` values will be merged with a 30
    // second interval stream. The value types of each stream must
    // match. This enum is used to track the various values.
    #[derive(Eq, PartialEq)]
    enum Item {
        Tick,
        Done,
    }

    // Interval at which the current sum is written to STDOUT.
    let tick_dur = Duration::from_secs(1);
    let interval = Interval::new_interval(tick_dur)
        .map(move |_| {
            if let Ok(_) = send_chunk(tx.clone(), InputChunk::Heartbeat).wait() {
                Item::Tick
            } else {
                tx.clone().send(None).wait();
                Item::Done
            }
        })
        .map_err(|e| panic!("timer failed; err={:?}", e));

    let fut = shutdown
        .map_err(|_| ())
        .map(|x| Item::Done)
        .select(interval)
        .take_while(|item| Ok(*item != Item::Done))
        .for_each(|_| Ok(()));

    tokio::spawn(fut);
}
// heartbeat:1 ends here

// input chunk

// [[file:~/Workspace/Programming/gosh-rs/runners/runners.note::*input%20chunk][input chunk:1]]
/// handle client requests
fn send_chunk(
    tx: Sender<Input>,
    chunk: InputChunk,
) -> impl Future<Item = Sender<Input>, Error = String> {
    tx.send(Some(chunk)).map_err(|_| "send-error".into())
}
// input chunk:1 ends here

// output chunk

// [[file:~/Workspace/Programming/gosh-rs/runners/runners.note::*output%20chunk][output chunk:1]]
// process server responses
fn process_responses(rx: Receiver<Output>, tx: Sender<Input>) {
    let fut = rx
        .map_err(|_| ())
        .for_each(move |item| match item {
            // process error stream
            OutputChunk::Stderr(err) => {
                dbg!(err);
                Ok(())
            }
            // process output stream
            OutputChunk::Stdout(out) => {
                dbg!(out);
                Ok(())
            }
            // send input stream
            OutputChunk::StartReadingStdin => {
                // let mut buf = vec![];
                // tokio::io::stdin()
                //     .read_to_end(&mut buf)
                //     .expect("read stdin");
                // if !buf.is_empty() {
                //     let chunk = InputChunk::Stdin(buf.into());
                //     send_chunk(tx.clone(), chunk).wait().unwrap();
                // }
                // let eof = InputChunk::StdinEOF;
                // send_chunk(tx.clone(), eof).wait().unwrap();
                Ok(())
            }
            _ => {
                dbg!(item);
                Ok(())
            }
        })
        .map(|_| ());

    tokio::spawn(fut);
}
// output chunk:1 ends here

// structopt

// [[file:~/Workspace/Programming/gosh-rs/runners/runners.note::*structopt][structopt:1]]
/// Nailgun client
#[derive(StructOpt, Debug)]
struct NailgunClient {
    #[structopt(flatten)]
    verbosity: Verbosity,

    #[structopt(flatten)]
    cmd: Cmd,
}
// structopt:1 ends here

// main

// [[file:~/Workspace/Programming/gosh-rs/runners/runners.note::*main][main:1]]
fn main() -> Result<()> {
    let args = NailgunClient::from_args();
    args.verbosity.setup_env_logger(&env!("CARGO_PKG_NAME"))?;

    let mut ng = NailgunConnection::default();
    ng.send_command(args.cmd.clone())?;

    Ok(())
}
// main:1 ends here