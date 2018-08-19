extern crate chrono;
extern crate clap;
extern crate directories;
#[macro_use]
extern crate failure;
extern crate vm;

use directories::ProjectDirs;
use failure::Error;
use std::ffi::OsString;
use std::io::Read;
use std::path::PathBuf;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

fn main() -> Result<(), Error> {
    let status = match Args::new(args_parser(), std::env::args()) {
        Ok(args) => run(&args)?,
        Err(err) => {
            eprintln!("{}", err);
            2
        },
    };

    std::process::exit(status);
}

fn run(args: &Args) -> Result<i32, Error> {
    let config_file_path = config_file_path();
    std::fs::create_dir_all(config_file_path.parent().unwrap())?;

    let config = vm::Config::from_file(&config_file_path)?;
    let vagrant = vm::Vagrant::new(config.vagrant_path());
    let mut vm = vm::Vm::new(&config_file_path, config, vagrant)?;

    if !config_file_path.exists() {
        vm.config().save_to_file(config_file_path.as_path())?;
    }

    Ok(run_vagrant(args, &mut vm)?)
}

fn run_vagrant<T: vm::RunVagrant>(args: &Args, vm: &mut vm::Vm<T>) -> Result<i32, Error> {
    match &args.subcommand {
        SubCommand::List => vm
            .list()
            .iter()
            .for_each(|info| println!("{}: {}", info.name(), info.path().to_string_lossy())),
        SubCommand::Add {
            name: name,
            path: path,
        } => {
            vm.add(name, path);
            vm.config().save_to_file(vm.config_file_path())?;
        }
        SubCommand::Remove {
            name: name,
            force: force,
        } => {
            let m =
                if let Some(info) = vm.get_info(name) {
                    print!(
                        "Delete this entry {{ name: {}, path: {} }} (y/N)",
                        info.name(),
                        info.path().to_string_lossy()
                    );

                    if let Some(result) = std::io::stdin().bytes().take(1).next() {
                        let byte = result?;
                        Some((info.clone(), byte.is_ascii_alphabetic() && byte.to_ascii_lowercase() == b'y'))
                    } else {
                        Some((info.clone(), false))
                    }
                } else {
                    None
                };

            if let Some((info, does_remove)) = m {
                if does_remove {
                    vm.remove(info.name());
                    vm.config().save_to_file(vm.config_file_path())?;
                }
            }
        }
        SubCommand::BackupConfigFile => {
            let backup_name = format!("config.toml.{}", chrono::Local::now().format("%Y%m%h-%H%M%S"));
            vm.backup_config_file(PathBuf::from(backup_name))?;
        }
        SubCommand::FindVagrantfiles {
            base_path: base_path
        } => {
            vm::find_vagrantfiles(base_path)?;
        }
        SubCommand::Raw {
            vm_name: vm_name,
            options: options,
        } => {
            return if let Some(info) = vm.get_info(vm_name) {
                std::env::set_current_dir(info.path())?;
                Ok(vm.vagrant_raw(options.as_slice())?.code().unwrap_or(0))
            } else {
                eprintln!("{} is not found in vm_list", vm_name);
                Ok(1)
            };
        }
    }

    Ok(0)
}

fn config_file_path() -> PathBuf {
    let project_dirs =
        ProjectDirs::from("org", "y8m", "vm").expect("Cannot get config directory path");
    PathBuf::from(project_dirs.config_dir()).join("config.toml")
}

struct Args {
    subcommand: SubCommand,
}

impl Args {
    fn new<I, T>(app: clap::App, args: I) -> Result<Args, Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        let matches = app.get_matches_from_safe(args)?;
        let subcommand = match matches.subcommand() {
            ("list", Some(sub_matches)) => SubCommand::List,
            ("add", Some(sub_matches)) => SubCommand::Add {
                name: sub_matches
                    .value_of("add_name")
                    .ok_or(ArgsError::NoVmName)?
                    .to_string(),
                path: PathBuf::from(sub_matches.value_of("add_path").ok_or(ArgsError::NoVmPath)?),
            },
            ("remove", Some(sub_matches)) => SubCommand::Remove {
                name: sub_matches
                    .value_of("remove_name")
                    .ok_or(ArgsError::NoVmName)?
                    .to_string(),
                force: sub_matches.is_present("remove_force"),
            },
            ("backup_config_file", Some(sub_matches)) => SubCommand::BackupConfigFile,
            ("find_vagrantfiles", Some(sub_matches)) => SubCommand::FindVagrantfiles {
                base_path: PathBuf::from(PathBuf::from(
                    sub_matches
                        .value_of("find_vagrantfiles_base_path")
                        .ok_or(ArgsError::NoFindVagrantfilesBasePath)?,
                )),
            },
            _ => SubCommand::Raw {
                vm_name: matches
                    .value_of("vm_name")
                    .ok_or(ArgsError::NoVmName)?
                    .to_string(),
                options: matches
                    .value_of("vagrant_options")
                    .map(|options_str| {
                        // TODO: 任意パラメータなのでこれだと `'a b'` みたいなのが来るとバグる
                        options_str
                            .split_whitespace()
                            .map(|s| s.to_string())
                            .collect()
                    })
                    .unwrap_or_else(|| Vec::new()),
            },
        };

        Ok(Args { subcommand })
    }
}

enum SubCommand {
    List,
    Add {
        name: String,
        path: PathBuf,
    },
    Remove {
        name: String,
        force: bool,
    },
    BackupConfigFile,
    FindVagrantfiles {
        base_path: PathBuf,
    },
    Raw {
        vm_name: String,
        options: Vec<String>,
    },
}

#[derive(Debug, Clone, Fail)]
enum ArgsError {
    #[fail(display = "Not specified a VM name")]
    NoVmName,
    #[fail(display = "Not specified a VM directory path")]
    NoVmPath,
    #[fail(display = "Not specified a find path")]
    NoFindVagrantfilesBasePath,
}

fn args_parser<'a, 'b>() -> clap::App<'a, 'b> {
    clap::App::new("vm")
        .version(VERSION)
        .author("Yuichi Fujita <cat@y8m.org>")
        .about("A vagrant wrapper for working directory independent execution.")
        .after_help("Repository: https://github.com/fujitayy/vm")
        .arg(
            clap::Arg::with_name("vm_name")
                .help("specify a vm name in vm_list of config file")
                .index(1),
        )
        .arg(
            clap::Arg::with_name("vagrant_options")
                .short("c")
                .value_name("VAGRANT_OPTIONS")
                .help("this value passed to vagrant command")
                .takes_value(true),
        )
        .subcommand(
            clap::SubCommand::with_name("list").help("Show entries in vm_list of config file"),
        )
        .subcommand(
            clap::SubCommand::with_name("add")
                .help("Add a entry to vm_list of config file")
                .arg(
                    clap::Arg::with_name("add_name")
                        .help("a name for new vm_list entry")
                        .value_name("NAME")
                        .index(1),
                )
                .arg(
                    clap::Arg::with_name("add_path")
                        .help("a path for new vm_list entry")
                        .value_name("PATH")
                        .index(2),
                ),
        )
        .subcommand(
            clap::SubCommand::with_name("remove")
                .help("Remove a entry in vm_list of config file specified by this value")
                .arg(
                    clap::Arg::with_name("remove_name")
                        .value_name("NAME")
                        .index(1),
                )
                .arg(
                    clap::Arg::with_name("remove_force")
                        .short("f")
                        .long("force"),
                ),
        )
        .subcommand(clap::SubCommand::with_name("backup_config_file").help("Backup config file"))
        .subcommand(
            clap::SubCommand::with_name("find_vagrantfiles")
                .help("Find Vagrantfiles")
                .arg(
                    clap::Arg::with_name("find_vagrantfiles_base_path")
                        .value_name("PATH")
                        .index(1),
                ),
        )
}
