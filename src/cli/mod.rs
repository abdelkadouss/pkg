const UNPRIVILEGED_USER: &str = "nobody";
const UNPRIVILEGED_GROUP: &str = "nogroup";

use clap::{ColorChoice, Parser, Subcommand};

use miette::{IntoDiagnostic, miette};
use mlua::Lua;
use nix::unistd::{ForkResult, Gid, Group, Uid, User, fork, setgid, setuid};
use std::{io::BufReader, os::unix::net::UnixStream};

use crate::{
    bridge::{self, BridgeNewPkgMetadata, BridgeOutput, default_impl},
    config::Config,
    db::Db,
    pkg::{self, PkgUserDef},
    pkg_type,
    process::{ChildBridgeMessage, reserve_msg, send_msg},
    utils::LuaResultExt,
    validation,
};

mod clean;
mod info;
mod link;
mod update;

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
            command => {
                let (parent_socket, child_socket) = UnixStream::pair().into_diagnostic()?;

                match unsafe { fork() }.into_diagnostic()? {
                    ForkResult::Child => {
                        drop(parent_socket);

                        let (mut reader, mut _writer) = make_reader_and_writer(child_socket)?;

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

                        // TODO: execute brdiges
                        let lua = Lua::new();
                        let config = Config::load(&lua)?;
                        let db = Db::new(&config.system.database_path)?;

                        let bridges = bridge::load_bridges(&lua, &config.paths.bridges)?;
                        let input = pkg::PkgUserDef::load(&lua, &config.paths.inputs)?.0;
                        let pkg_types = pkg_type::PkgType::load(
                            &lua,
                            &config.paths.pkg_types_definition,
                            &config.paths.default_out,
                        )?;

                        let all_inputs = input
                            .into_iter()
                            .map(|(_, pkgs)| pkgs.0)
                            .collect::<Vec<Vec<PkgUserDef>>>()
                            .into_iter()
                            .flatten()
                            .collect::<Vec<PkgUserDef>>();

                        validation::ensure_no_duplication(&all_inputs)
                            .ok_or(todo!("return err"))
                            .into_diagnostic()?;
                        validation::ensure_all_depand_on_defined_pkg(&all_inputs)
                            .ok_or(todo!("return err"))
                            .into_diagnostic()?;
                        validation::try_ensure_no_dep_loop(&all_inputs)?;

                        // TODO: ensure the user uses brdige that exists.

                        // TODO: do even more validation like ensure that pkg def uses the defined valid opts in the pkg type def.

                        // TODO: do that thread managment
                        // let threads_stuck = vec![];

                        for node in input {
                            let bridge =
                                bridges.iter().find(|brdige| brdige.name == node.0).unwrap();

                            if let Commands::Update { packages } = command {
                                let (_, _, mut to_update) =
                                    slit_pkg_base_on_state(all_inputs, &db)?;

                                if let Some(packages) = packages {
                                    to_update = to_update
                                        .into_iter()
                                        .filter(|pkg| packages.contains(&pkg.name))
                                        .collect::<Vec<PkgUserDef>>();
                                }

                                for pkg in to_update {
                                    let pkgs_with_new_data =
                                        if let Some(method) = bridge.bridge.update {
                                            method
                                                .call::<Option<Vec<BridgeNewPkgMetadata>>>(())
                                                .into_report()?
                                        } else {
                                            default_impl::update(&lua, pkg.name)?
                                        };

                                    send_msg(
                                        &mut child_socket,
                                        BridgeOutput::Update(pkgs_with_new_data),
                                    )?;
                                }

                                continue;
                            }

                            let (mut to_install, to_remove, mut to_update) =
                                slit_pkg_base_on_state(all_inputs, &db)?;

                            if matches!(command, Commands::Rebuild) {
                                to_install.append(&mut to_update);
                            }

                            for pkg in to_install {
                                let new_pkgs = bridge
                                    .bridge
                                    .install
                                    .call::<Vec<BridgeNewPkgMetadata>>(())
                                    .into_report()?;

                                send_msg(&mut child_socket, BridgeOutput::New(new_pkgs))?;
                            }

                            for pkg in to_remove {
                                if let Some(method) = bridge.bridge.remove {
                                    method.call::<()>(()).into_report()?;
                                } else {
                                    default_impl::remove(&lua, pkg.name)?;
                                }

                                send_msg(&mut child_socket, BridgeOutput::Remove(pkg.name))?;
                            }
                        }
                    }
                    ForkResult::Parent { child } => {
                        drop(child_socket);

                        if !Uid::current().is_root() {
                            return Err(miette!("re-run this as root"));
                        }

                        let (mut reader, mut writer) = make_reader_and_writer(parent_socket)?;

                        send_msg(&mut writer, true)?; // brdige worker go
                        serve_child(&mut reader, &mut writer)?;

                        nix::sys::wait::waitpid(child, None).into_diagnostic()?;
                    }
                };

                Ok(())
            }
        }
    }
}

fn serve_child(reader: &mut BufReader<UnixStream>, _writer: &mut UnixStream) -> miette::Result<()> {
    loop {
        let msg: Option<ChildBridgeMessage> = reserve_msg(reader)?;

        if msg.is_none() {
            break Ok(());
        }

        match msg.unwrap() {
            ChildBridgeMessage::Done(_pkg_def) => todo!("install/remove/update pkg"),
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

fn slit_pkg_base_on_state(
    pkgs: Vec<PkgUserDef>,
    db: &Db,
) -> miette::Result<(Vec<PkgUserDef>, Vec<PkgUserDef>, Vec<PkgUserDef>)> {
    todo!()
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
