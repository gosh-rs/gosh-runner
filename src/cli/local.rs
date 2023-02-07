// [[file:../../runners.note::ef8e07e8][ef8e07e8]]
use super::*;
use crate::session::Session;
// ef8e07e8 ends here

// [[file:../../runners.note::bff78206][bff78206]]
use gut::cli::*;

/// A local runner that can make graceful exit
#[derive(Parser, Debug, Default)]
struct RunnerCli {
    #[command(flatten)]
    verbose: gut::cli::Verbosity,

    /// Job timeout in seconds. The default timeout is 30 days.
    #[arg(long, short)]
    timeout: Option<u32>,

    /// Command line to call a program
    #[arg(raw = true, required = true)]
    cmdline: Vec<String>,
}

impl RunnerCli {
    fn enter_main<I>(iter: I) -> Result<()>
    where
        Self: Sized,
        I: IntoIterator,
        I::Item: Into<std::ffi::OsString> + Clone,
    {
        let args = RunnerCli::try_parse_from(iter)?;
        args.verbose.setup_logger();

        let program = &args.cmdline[0];
        let rest = &args.cmdline[1..];

        Session::new(program)
            .args(rest)
            .timeout(args.timeout.unwrap_or(3600 * 24 * 30))
            .run()?;

        Ok(())
    }
}

pub fn local_enter_main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    assert!(args.len() >= 1, "{:?}", args);
    // The path to symlink file that invoking the real program
    let invoke_path: &Path = &args[0].as_ref();

    // check file extension for sure (should be foo.run)
    // REVIEW: must be carefully here: not to enter infinite loop
    if let Some("run") = invoke_path.extension().and_then(|s| s.to_str()) {
        // apply symlink magic
        // call the program that symlink pointing to
        let invoke_exe = invoke_path.file_stem().context("invoke exe name")?;

        // The path to real executable binary
        let real_path = std::env::current_exe().context("Failed to get exe path")?;
        println!("Runner exe path: {:?}", real_path);
        let real_exe = real_path.file_name().context("real exe name")?;

        if real_exe != invoke_exe {
            let runner_args = [&real_exe.to_string_lossy(), "-v", "--", &invoke_exe.to_string_lossy()];

            let cmdline: Vec<_> = runner_args
                .iter()
                .map(|s| s.to_string())
                .chain(args.iter().cloned().skip(1))
                .collect();
            println!("runner will call {:?} with {:?}", invoke_exe, cmdline.join(" "));
            return RunnerCli::enter_main(cmdline);
        }
    }
    // run in a normal way
    RunnerCli::enter_main(std::env::args())
}
// bff78206 ends here

// [[file:../../runners.note::*ctrlc][ctrlc:1]]
use gut::prelude::*;

/// Run main process with ctrl-c handler
pub fn ctrlc_enter_main(enter_main: fn() -> Result<()>) -> Result<()> {
    // Create the runtime
    let rt = tokio::runtime::Runtime::new().unwrap();
    // Execute the future, blocking the current thread until completion
    rt.block_on(ctrlc_enter_main_(enter_main))?;

    Ok(())
}

async fn ctrlc_enter_main_(enter_main: fn() -> Result<()>) -> Result<()> {
    let ctrl_c = tokio::signal::ctrl_c();
    let main_task = tokio::task::spawn_blocking(move || {
        // This is running on a blocking thread.
        enter_main()
    });

    tokio::select! {
        result = main_task => {
            result?;
            info!("Done");
        }
        result = ctrl_c => {
            result?;
            info!("Received SIGINT, exiting");
        }
    }

    Ok(())
}
// ctrlc:1 ends here
