// [[file:../../runners.note::cb0a82ee][cb0a82ee]]
use super::*;
// cb0a82ee ends here

// [[file:../../runners.note::8e91b7e1][8e91b7e1]]
use gut::fs::ShellEscapeExt;

#[derive(Debug, Clone, ArgEnum)]
enum PathOp {
    Append,
    Prepend,
    Remove,
}

impl PathOp {
    fn apply(&self, path_key: &str, path_value: &str) -> String {
        let path_key = path_key.trim();
        let path_value = path_value.trim();

        if let Some(paths) = std::env::var_os(path_key) {
            let path_value_: std::path::PathBuf = path_value.into();
            let mut paths = std::env::split_paths(&paths).collect_vec();
            paths.retain(|path| path != &path_value_);

            match self {
                Self::Remove => {}
                Self::Append => {
                    paths.push(path_value_);
                }
                Self::Prepend => {
                    paths.insert(0, path_value_);
                }
            }
            match std::env::join_paths(paths) {
                Ok(path_values) => path_values.to_string_lossy().into_owned(),
                Err(err) => {
                    eprintln!("found invalid char in path: {err:?}");
                    format!("{}", path_value)
                }
            }
        } else {
            format!("{path_value}")
        }
    }
}

#[test]
fn test_environ() {
    let value = PathOp::Append.apply("PATH", "/usr/bin");
    assert!(value.ends_with("/usr/bin"));

    let value = PathOp::Prepend.apply("PATH", "/usr/bin");
    assert!(value.starts_with("/usr/bin"));

    let value = PathOp::Remove.apply("PATH", "/usr/bin");
    let usr_bin: &Path = "/usr/bin".as_ref();
    assert!(!std::env::split_paths(&value).any(|path| path == usr_bin));
}

pub fn prepend_path(path_key: &str, new_path: &str) -> String {
    let path_value = PathOp::Prepend.apply(path_key, new_path);
    format!("export {path_key}={}", path_value.as_str().shell_escape())
}

pub fn append_path(path_key: &str, new_path: &str) -> String {
    let path_value = PathOp::Append.apply(path_key, new_path);
    format!("export {path_key}={}", path_value.as_str().shell_escape())
}

pub fn remove_path(path_key: &str, new_path: &str) -> String {
    let path_value = PathOp::Remove.apply(path_key, new_path);
    format!("export {path_key}={}", path_value.as_str().shell_escape())
}
// 8e91b7e1 ends here

// [[file:../../runners.note::b185bee5][b185bee5]]
fn set_module_env_vars(apps_root_dir: &Path, module_name: &str, remove: bool) -> Result<String> {
    let mod_root = apps_root_dir.join(module_name);
    let mod_bin = mod_root.join("bin");

    let mut lines = String::new();
    // PATH
    if mod_bin.is_dir() {
        let line = if remove {
            remove_path("PATH", &format!("{}", mod_bin.display()))
        } else {
            prepend_path("PATH", &format!("{}", mod_bin.display()))
        };
        lines.push_str(&format!("{line};"));
    }

    // LD_LIBRARY_PATH
    let mod_lib = mod_root.join("lib");
    if mod_lib.is_dir() {
        for path in ["CPATH", "LIBRARY_PATH", "LD_LIBRARY_PATH", "LD_RUN_PATH"] {
            let line = if remove {
                remove_path(path, &format!("{}", mod_lib.display()))
            } else {
                prepend_path(path, &format!("{}", mod_lib.display()))
            };
            lines.push_str(&format!("{line};"));
        }
    }

    let mod_envrc = mod_root.join(".envrc");
    if mod_envrc.is_file() {
        info!("load .envrc {mod_envrc:?}");
        let dir = mod_root.shell_escape_lossy();
        // source .envrc in module's root dir
        lines.push_str(&format!("pushd {dir};"));
        lines.push_str("source .envrc;");
        lines.push_str("popd;")
    }

    Ok(lines)
}

#[test]
#[ignore]
fn test_apps_module() {
    let x = set_module_env_vars("/share/apps".as_ref(), "mpich3");
}
// b185bee5 ends here

// [[file:../../runners.note::54d72d8a][54d72d8a]]
#[derive(Debug, Clone, ArgEnum)]
enum AppsOp {
    /// Load module environment variables
    Load,
    /// Unload module environment variables
    Unload,
}

/// A shell environment manager as a poor-man's modulefiles (for bash only now)
#[derive(Parser)]
#[clap(author, version, about)]
pub struct Apps {
    #[clap(flatten)]
    verbose: gut::cli::Verbosity,

    #[clap(arg_enum)]
    action: AppsOp,

    /// The requested module
    module: String,
}

impl Apps {
    pub fn enter_main() -> Result<()> {
        let args = Self::parse();
        args.verbose.setup_logger();

        let apps_root_dir = std::env::var("BBM_APPS_DIR").unwrap_or("/share/apps".into());
        match args.action {
            AppsOp::Load => {
                let bash_script = set_module_env_vars(apps_root_dir.as_ref(), &args.module, false)?;
                debug!("Load env vars in bash:\n{bash_script}");
                println!("{bash_script}");
            }
            AppsOp::Unload => {
                let bash_script = set_module_env_vars(apps_root_dir.as_ref(), &args.module, true)?;
                debug!("Unload env vars in bash:\n{bash_script}");
                println!("{bash_script}");
            }
        }

        Ok(())
    }
}
// 54d72d8a ends here
