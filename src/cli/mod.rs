const UNPRIVILEGED_USER: &str = "nobody";
const UNPRIVILEGED_GROUP: &str = "nogroup";

use std::{io::BufReader, os::unix::net::UnixStream};

use clap::{ColorChoice, Parser, Subcommand};
use miette::{IntoDiagnostic, miette};
use mlua::Lua;
use nix::unistd::{ForkResult, Gid, Group, Uid, User, fork, setgid, setuid};

use crate::{
    bridge,
    config::Config,
    db::Db,
    pkg::{self, BridgeMap, Pkg, PkgUserDef},
    pkg_type::{self, PkgType},
    process::{ChildBridgeMessage, reserve_msg, send_msg},
    validation,
};

mod clean;
mod info;
mod link;
mod rebuild;
mod sync;
mod update;

mod helpers;

#[cfg(feature = "cli_complation")]
#[derive(Clone, Debug, clap::ValueEnum)]
pub enum Shell {
    Bash,
    Fish,
    Zsh,
    Elvish,
    Nushell,
    #[allow(clippy::enum_variant_names)]
    PowerShell, // NOTE: this is not needed really because this is unix only
}

#[derive(Parser)]
#[command(name = "pkg")]
#[command(version, about, long_about = None)] // Read from `Cargo.toml`
#[command(color = ColorChoice::Always)] // Always show colors
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Sync packages with configuration (install/remove as configured)
    #[command(alias = "build", alias = "s", alias = "b")]
    Sync {
        /// even update the installed packages via the update command
        #[arg(short, long)]
        update: bool,
        #[arg(short, long)]
        trust_bridges: bool,
    },

    /// Force sync all packages (reinstall everything)
    Rebuild,

    /// Update packages
    #[command(alias = "u")]
    Update {
        /// Specific packages to update ( default: all )
        packages: Option<Vec<String>>,
    },

    /// List installed packages
    Info {
        /// A packge to show information about ( default: all )
        packages: Option<Vec<String>>,
    },

    /// Link packages in PATH
    Link,

    /// Clean cache and temporary files
    Clean,

    /// Test a bridge with verbose loging
    Test,

    #[cfg(feature = "cli_complation")]
    /// Generate shell completion scripts for your clap::Command
    #[command(alias = "compl")]
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },
}

impl Cli {
    pub fn route() -> miette::Result<()> {
        let cli = Cli::parse();

        match cli.command {
            Commands::Info { packages } => info::info(packages),
            Commands::Link => link::link(),
            Commands::Clean => clean::clean(),
            Commands::Test => todo!(),
            command => {
                let (parent_socket, mut child_socket) = UnixStream::pair().into_diagnostic()?;

                if !Uid::current().is_root() {
                    return Err(miette!("re-run this as root"));
                }

                let lua = Lua::new();
                let config = Config::load(&lua)?;
                let db = Db::new(&config.system.database_path)?;

                let pkg_types = pkg_type::PkgType::load(
                    &lua,
                    &config.paths.pkg_types_definition,
                    &config.paths.default_out,
                )?;

                match unsafe { fork() }.into_diagnostic()? {
                    ForkResult::Child => {
                        drop(parent_socket);

                        let (mut reader, mut writer) = make_reader_and_writer(child_socket)?;

                        let go: Option<bool> = reserve_msg(&mut reader)?;

                        if !go.is_some_and(|v| v) {
                            return Err(miette!("main process stop the bridge worker"));
                        }

                        down_grade_privileges()?;

                        if Uid::current().is_root() {
                            return Err(miette!(
                                "the bridge runner worker still was root privileges, fiald to down grade."
                            ));
                        }

                        let bridges = bridge::load_bridges(&lua, &config.paths.bridges)?;
                        let input = split_pkg_base_on_state(
                            pkg::PkgUserDef::load(&lua, &config.paths.inputs)?,
                            &db,
                        )?;

                        let all_inputs = input
                            .iter()
                            .flat_map(|it| {
                                [it.1.0.clone(), it.1.1.clone(), it.1.2.clone()].concat()
                            })
                            .collect::<Vec<_>>();

                        let _ = validation::ensure_no_duplication(&all_inputs)
                            .is_none_or(|dup| todo!("return err"));
                        let _ = validation::ensure_all_depand_on_defined_pkg(&all_inputs)
                            .is_none_or(|unknow_dep| todo!("return err"));
                        validation::try_ensure_no_dep_loop(&all_inputs)?
                            .is_none_or(|dep_loop| todo!("return err"));

                        // TODO: ensure the user uses brdige that exists.

                        // TODO: do even more validation like ensure that pkg def uses the defined valid opts in the pkg type def.

                        // TODO: do that thread managment
                        // let threads_stuck = vec![];

                        if let Commands::Sync {
                            update,
                            trust_bridges,
                        } = command
                        {
                            for (name, (to_install, to_remove, to_update)) in input {
                                let Some(bridge) =
                                    bridges.iter().find(|bridge| bridge.name == name)
                                else {
                                    return Err(miette!("use of unknow bridge"));
                                };

                                helpers::install::install(bridge, &to_install, &mut writer)?;
                            }
                        } else if let Commands::Rebuild = command {
                            todo!();
                        } else if let Commands::Update { packages } = command {
                            todo!();
                        }
                    }
                    ForkResult::Parent { child } => {
                        drop(child_socket);
                        // drop(lua);
                        // drop(config);

                        let (mut reader, mut writer) = make_reader_and_writer(parent_socket)?;

                        send_msg(&mut writer, true)?; // brdige worker go
                        serve_child(&mut reader, &mut writer, &db, &pkg_types)?;

                        nix::sys::wait::waitpid(child, None).into_diagnostic()?;
                    }
                };

                Ok(())
            }
        }
    }
}

fn serve_child(
    reader: &mut BufReader<UnixStream>,
    _writer: &mut UnixStream,
    db: &Db,
    pkgs_types: &Vec<PkgType>,
) -> miette::Result<()> {
    loop {
        let msg: Option<ChildBridgeMessage> = reserve_msg(reader)?;

        if msg.is_none() {
            break Ok(());
        }

        match msg.unwrap() {
            ChildBridgeMessage::Done { output, user_input } => match output {
                bridge::BridgeOutput::New(new) => {
                    // TODO: validate:
                    //   - not exist alredy (try to install some thing alredy installed)
                    //   - output match the pkg def (link optoin, version track...)
                    db.install(
                        new.iter()
                            .map(|it| {
                                Pkg::build(
                                    &user_input,
                                    &it,
                                    &pkgs_types
                                        .iter()
                                        .find(|pkg_type| Some(pkg_type.name.clone()) == it.pkg_type)
                                        .unwrap(),
                                )
                                .unwrap()
                            })
                            .collect::<Vec<_>>(),
                    )?;
                }
                bridge::BridgeOutput::Remove(name) => todo!(),
                bridge::BridgeOutput::Update(new) => todo!(),
            },
            ChildBridgeMessage::HightPrivApiReq(_cmd) => {
                todo!("ask user and then execute cmd with hight priv")
            }
            ChildBridgeMessage::ReturnErr(_bridge_error) => todo!("handle error"),
        }
    }
}

fn find_unused_user_id() -> Option<u32> {
    const START: u32 = 1;
    const END: u32 = 200;

    for uid in START..END {
        if let Ok(None) = User::from_uid(uid.into()) {
            return Some(uid);
        }
    }
    None
}

fn find_unused_group_id() -> Option<u32> {
    const START: u32 = 1;
    const END: u32 = 200;

    for uid in START..END {
        if let Ok(None) = Group::from_gid(uid.into()) {
            return Some(uid);
        }
    }
    None
}

fn down_grade_privileges() -> miette::Result<()> {
    let uid: Uid = User::from_name(UNPRIVILEGED_USER)
        .ok()
        .and_then(|user| {
            if let Some(user) = user {
                user.uid.into()
            } else {
                Uid::from_raw(find_unused_user_id()?).into()
            }
        })
        .unwrap();

    let gid: Gid = Group::from_name(UNPRIVILEGED_GROUP)
        .ok()
        .and_then(|group| {
            if let Some(group) = group {
                group.gid.into()
            } else {
                Gid::from_raw(find_unused_group_id()?).into()
            }
        })
        .unwrap();

    setgid(gid).into_diagnostic()?;
    setuid(uid).into_diagnostic()?;

    Ok(())
}

fn make_reader_and_writer(
    stream: UnixStream,
) -> miette::Result<(BufReader<UnixStream>, UnixStream)> {
    let socket_copy = stream.try_clone().into_diagnostic()?;
    let writer = stream;
    let reader = BufReader::new(socket_copy);
    Ok((reader, writer))
}

fn split_pkg_base_on_state(
    pkgs: BridgeMap,
    db: &Db,
) -> miette::Result<Vec<(String, (Vec<PkgUserDef>, Vec<PkgUserDef>, Vec<PkgUserDef>))>> {
    let mut out: Vec<(String, (Vec<PkgUserDef>, Vec<PkgUserDef>, Vec<PkgUserDef>))> = vec![];

    let pkgs_in_db = db.load_all()?;
    let pkgs_in_db_names = &pkgs_in_db.iter().map(|it| &it.name).collect::<Vec<_>>();

    for (name, pkgs) in pkgs.0 {
        out.push((
            name,
            (
                pkgs.0
                    .iter()
                    .filter(|it| !pkgs_in_db_names.contains(&&it.name))
                    .cloned()
                    .collect::<Vec<_>>(),
                pkgs_in_db
                    .clone()
                    .iter()
                    .filter(|it| {
                        pkgs.0
                            .iter()
                            .map(|p| &p.name)
                            .collect::<Vec<_>>()
                            .contains(&&it.name)
                    })
                    .map(|it| it.clone().into())
                    .collect::<Vec<PkgUserDef>>(),
                pkgs.0
                    .into_iter()
                    .filter(|it| pkgs_in_db_names.contains(&&it.name))
                    .collect::<Vec<_>>(),
            ),
        ));
    }

    Ok(out)
}

#[cfg(test)]
#[test]
fn try_down_grade_privileges() -> miette::Result<()> {
    if !Uid::current().is_root() {
        return Ok(());
    }

    down_grade_privileges()?;

    assert!(!Uid::current().is_root());

    Ok(())
}
