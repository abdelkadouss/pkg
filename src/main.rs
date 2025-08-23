use clap::Parser;
use cli_table::{Cell, Style, Table, print_stdout};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use miette::{IntoDiagnostic, Result};
use owo_colors::OwoColorize;
use pkg::{
    DEFAULT_CONFIG_FILE_EXTENSION, DEFAULT_CONFIG_FILE_NAME, DEFAULT_LOG_DIR, DEFAULT_WORKING_DIR,
    bridge,
    cmd::{Cli, Commands},
    config::Config,
    db::{self, Db, Pkg, PkgType},
    fs,
    input::{self, PkgDeclaration},
};
use rpassword::read_password;
use std::{
    io::{self, Write},
    path::PathBuf,
    process::{Command, Stdio, exit},
};

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Check if we need root privileges and prompt for password if needed
    if !check_root_privileges() {
        prompt_for_sudo()?;
    }

    let config_dir = get_valid_config_path()?;

    let config_path = config_dir
        .join(DEFAULT_CONFIG_FILE_NAME)
        .with_extension(DEFAULT_CONFIG_FILE_EXTENSION);

    // load config
    let config = Config::load(config_path)?;

    let db_path = config.db_path.clone();
    let target_dir = config.target_dir.clone();
    let load_path = config.load_path.clone();
    let bridges_set = config.bridges_set.clone();
    let inputs_path = config.source_dir.clone();

    let db = db::Db::new(&db_path)?;

    let input = input::Input::load(&inputs_path)?;

    let needed_bridges = input.bridges.iter().map(|b| b.name.clone()).collect();

    let bridge_api = bridge::BridgeApi::new(bridges_set, needed_bridges, &db_path)?;

    let fs = fs::Fs::new(target_dir, load_path, &db_path);

    match &cli.command {
        Commands::Clean => {
            if PathBuf::from(DEFAULT_LOG_DIR).exists() {
                std::fs::remove_dir_all(DEFAULT_LOG_DIR).into_diagnostic()?;
            }
            if PathBuf::from(DEFAULT_WORKING_DIR).exists() {
                std::fs::remove_dir_all(DEFAULT_WORKING_DIR).into_diagnostic()?;
            }

            println!("🧹🗑️✨");

            Ok(())
        }
        Commands::Info { package } => {
            let pkgs = if let Some(packages) = package {
                db.get_pkgs_by_name(packages)?
            } else {
                db.get_pkgs()?
            };

            let table = pkgs
                .iter()
                .map(|pkg| {
                    vec![
                        pkg.name.clone().cell(),
                        format!(
                            "{}.{}.{}",
                            pkg.version.first_cell, pkg.version.second_cell, pkg.version.third_cell
                        )
                        .cell(),
                        pkg.path.display().to_string().cell(),
                        match &pkg.pkg_type {
                            PkgType::SingleExecutable => "executable".to_string(),
                            PkgType::Directory(path) => {
                                format!("directory: {}", path.display())
                            }
                        }
                        .cell(),
                    ]
                })
                .collect::<Vec<_>>()
                .table()
                .title(vec![
                    "Name".cell().bold(true),
                    "Version".cell().bold(true),
                    "Path".cell().bold(true),
                    "Type".cell().bold(true),
                ]);

            print_stdout(table).into_diagnostic()?;
            Ok(())
        }
        _ => {
            // Handle commands
            let spinner_style =
                ProgressStyle::with_template("{prefix:.bold.dim} {spinner} {wide_msg}")
                    .unwrap()
                    .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");
            let job_style = ProgressStyle::with_template("{wide_msg}")
                .unwrap()
                .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");

            let mut total_installed_pkgs_count_index = 0;
            let mut total_removed_pkgs_count_index = 0;

            enum Job {
                Install,
                Update,
                Remove,
                Reinstall,
            }

            enum Action {
                Add(Result<Pkg>),
                Remove(Result<bool>),
            }

            for bridge in &input.bridges {
                let pkgs = bridge
                    .pkgs
                    .iter()
                    .map(|p| p.name.clone())
                    .collect::<Vec<String>>();

                let (
                    installed_pkgs_in_input,
                    not_installed_pkgs_in_input,
                    installed_pkgs_not_in_input,
                ) = filter_pkgs_by_statuses(&db, &pkgs, &bridge.pkgs)?;
                let mut installed_pkgs_in_input = installed_pkgs_in_input;

                let pkgs_to_remove_count = installed_pkgs_not_in_input.len();
                let pkgs_to_install_count = not_installed_pkgs_in_input.len();
                let mut pkgs_to_update_count = installed_pkgs_in_input.len();

                print_bridge_header(
                    &bridge.name,
                    pkgs_to_install_count,
                    pkgs_to_remove_count,
                    pkgs_to_update_count,
                );

                let m = MultiProgress::new();

                let mut jobs = vec![];
                if let Commands::Build { update } = &cli.command {
                    if *update {
                        jobs.push(Job::Update);
                    }
                    jobs.push(Job::Install);
                    jobs.push(Job::Remove);
                } else if let Commands::Rebuild = cli.command {
                    jobs.push(Job::Install);
                    jobs.push(Job::Remove);
                    jobs.push(Job::Reinstall);
                } else if let Commands::Update { packages } = &cli.command {
                    if let Some(packages) = packages {
                        let mut pkgs = Vec::new();
                        installed_pkgs_in_input.iter().for_each(|pkg| {
                            if packages.contains(&pkg.name) {
                                pkgs.push(*pkg);
                            }
                        });
                        installed_pkgs_in_input = pkgs.clone();
                        pkgs_to_update_count = pkgs.len();
                    }

                    jobs.push(Job::Update);
                }

                for job in jobs {
                    let pkgs = match job {
                        Job::Install => &not_installed_pkgs_in_input,
                        Job::Update => &installed_pkgs_in_input,
                        Job::Remove => &installed_pkgs_not_in_input,
                        Job::Reinstall => &installed_pkgs_in_input,
                    };

                    let pkgs_count = pkgs.len();

                    if pkgs_count == 0 {
                        continue;
                    }

                    match job {
                        Job::Install => print_job_header("install"),
                        Job::Update => print_job_header("update"),
                        Job::Remove => print_job_header("remove"),
                        Job::Reinstall => print_job_header("reinstall"),
                    }

                    for (i, pkg) in pkgs.iter().enumerate() {
                        let pb = m.add(ProgressBar::new(100));
                        pb.set_style(spinner_style.clone());
                        pb.set_prefix(format!("[{}/{}]", i + 1, pkgs_count));
                        pb.set_message(format!("🚚 {}", pkg.name));

                        let pkg_name = pkg.name.clone();

                        let action_result = match job {
                            Job::Install => Action::Add(bridge_api.install(&bridge.name, pkg)),
                            Job::Update => Action::Add(bridge_api.update(&bridge.name, pkg)),
                            Job::Remove => Action::Remove(bridge_api.remove(&bridge.name, pkg)),
                            Job::Reinstall => {
                                let install_result = bridge_api.install(&bridge.name, pkg);

                                if install_result.is_err() {
                                    return Err(install_result.err().unwrap());
                                }

                                let remove_result = bridge_api.remove(&bridge.name, pkg);

                                if remove_result.is_err() {
                                    return Err(remove_result.err().unwrap());
                                }

                                let db_remove_result = db.remove_pkgs(&[pkg.name.clone()]);
                                if let Err(db_err) = db_remove_result {
                                    pb.finish_with_message(format!(
                                        "❌ {},{}: {}",
                                        pkg.name.red().bold(),
                                        "at remove pkg from db".red().underline(),
                                        db_err.red()
                                    ));
                                }

                                Action::Add(install_result)
                            }
                        };

                        if let Action::Add(Err(err)) | Action::Remove(Err(err)) = action_result {
                            pb.finish_with_message(format!(
                                "❌ {},{}: {}",
                                pkg.name.red().bold(),
                                "at bridge operation".red().underline(),
                                err.red()
                            ));
                            continue;
                        }

                        match action_result {
                            Action::Add(Ok(mut pkg)) => {
                                pb.set_message(format!("🗃️ {}", pkg.name));

                                let fs_res = fs
                                    .store_or_overwrite(&mut [&mut pkg], Some(bridge.name.as_str()))
                                    .inspect_err(|err| {
                                        pb.finish_with_message(format!(
                                            "❌ {}, {}: {}",
                                            pkg.name.red().bold(),
                                            "at store the pkg".red().underline(),
                                            err.red()
                                        ));
                                    });

                                if fs_res.is_err() {
                                    continue;
                                }

                                if matches!(job, Job::Update) {
                                    let db_res =
                                        db.remove_pkgs(&[pkg.name.clone()]).inspect_err(|err| {
                                            pb.finish_with_message(format!(
                                                "❌ {}, {}: {}",
                                                pkg.name.red().bold(),
                                                "at remove pkg from db".red().underline(),
                                                err.red()
                                            ));
                                        });

                                    if db_res.is_err() {
                                        continue;
                                    }
                                }

                                let db_res = db
                                    .install_bridge_pkgs(&[&pkg], &bridge.name)
                                    .inspect_err(|err| {
                                        pb.finish_with_message(format!(
                                            "❌ {}, {}: {}",
                                            pkg.name.red().bold(),
                                            "at write pkg in db".red().underline(),
                                            err.red()
                                        ));
                                    });

                                if db_res.is_err() {
                                    continue;
                                }

                                total_installed_pkgs_count_index += 1;
                                pb.finish_with_message(format!("📦 {}.", pkg.name.green().bold()));
                            }
                            Action::Remove(Ok(true)) => {
                                pb.set_message(format!("🗃️ {}", &pkg_name));

                                let fs_res = fs.remove_pkgs(&[&pkg_name]).inspect_err(|err| {
                                    pb.finish_with_message(format!(
                                        "❌ {}, {}: {}",
                                        &pkg_name.red().bold(),
                                        "at remove the pkg".red().underline(),
                                        err.red()
                                    ));
                                });

                                if fs_res.is_err() {
                                    continue;
                                }

                                let db_res =
                                    db.remove_pkgs(&[pkg_name.clone()]).inspect_err(|err| {
                                        pb.finish_with_message(format!(
                                            "❌ {}, {}: {}",
                                            &pkg_name.red().bold(),
                                            "at remove pkg from db".red().underline(),
                                            err.red()
                                        ));
                                    });

                                if db_res.is_err() {
                                    continue;
                                }

                                total_removed_pkgs_count_index += 1;
                                pb.finish_with_message(format!("🗑️ {}.", &pkg_name.green().bold()));
                            }
                            Action::Add(Err(err)) | Action::Remove(Err(err)) => {
                                // Error already handled in the map_err above
                                return Err(err);
                            }
                            Action::Remove(Ok(false)) => {
                                pb.finish_with_message(format!(
                                    "❌ {}, {}: {}",
                                    &pkg_name.red().bold(),
                                    "at bridge operation".red().underline(),
                                    "the remove operation returned false".red().bold()
                                ));
                            }
                        }

                        pb.inc(1);
                    }
                }
            }

            perform_linking(&fs, job_style.clone())?;

            println!(
                "{}\n📦{} 🗑️ {}",
                "Summary:".green().bold(),
                total_installed_pkgs_count_index,
                total_removed_pkgs_count_index,
            );
            println!("{}", "Done 🌻, thanks to Allah".green().bold());

            Ok(())
        }
    }
}

fn check_root_privileges() -> bool {
    let output = Command::new("id")
        .arg("-u")
        .output()
        .expect("Failed to check user ID");

    if output.status.success() {
        let output_str = String::from_utf8_lossy(&output.stdout);
        let uid = output_str.trim();
        uid == "0"
    } else {
        false
    }
}

fn prompt_for_sudo() -> Result<()> {
    print!("{}: ", "password".blue().bold());
    io::stdout().flush().into_diagnostic()?;

    let password = read_password().into_diagnostic()?;

    // Validate the password by trying to run a simple sudo command
    let mut validation = Command::new("sudo")
        .arg("-S")
        .arg("echo")
        .arg("password_valid")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .into_diagnostic()?;

    let stdin = validation.stdin.as_mut();

    // Send the password to sudo
    if let Some(stdin) = stdin {
        stdin.write_all(password.as_bytes()).into_diagnostic()?;
        stdin.write_all(b"\n").into_diagnostic()?;
    }

    let status = validation.wait().into_diagnostic()?;

    if status.success() {
        // Password is valid, re-run the command with sudo
        re_run_with_sudo()?;
    } else {
        eprintln!("Incorrect password or sudo access denied.");
        std::process::exit(1);
    }

    Ok(())
}

fn re_run_with_sudo() -> Result<()> {
    let current_exe = std::env::current_exe().into_diagnostic()?;
    let args: Vec<String> = std::env::args().collect();

    let status = Command::new("sudo")
        .arg(&current_exe)
        .args(&args[1..]) // Skip the program name
        .status()
        .into_diagnostic()?;

    std::process::exit(status.code().unwrap_or(1));
}

fn get_valid_config_path() -> Result<PathBuf> {
    let xdg_config_home: String = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
        let home_dir = std::env::var("HOME").expect("HOME environment variable not set");
        format!("{home_dir}/.config")
    });

    let xdg_config_home = format!("{xdg_config_home}/pkg");

    let xdg_config_home = PathBuf::from(&xdg_config_home);

    std::fs::create_dir_all(&xdg_config_home).expect("Failed to make the defuld config dir");

    Ok(xdg_config_home)
}

fn filter_pkgs_by_statuses<'a>(
    db: &Db,
    inputs_pkgs: &[String],
    pkgs_declarations: &'a [PkgDeclaration],
) -> Result<(
    Vec<&'a PkgDeclaration>,
    Vec<&'a PkgDeclaration>,
    Vec<&'a PkgDeclaration>,
)> {
    let all_installed_pkgs = db.get_pkgs()?;

    let installed_pkgs_in_input_names = db.which_pkgs_are_installed(inputs_pkgs)?;
    let not_installed_pkgs_in_input_names = db.which_pkgs_are_not_installed(inputs_pkgs)?;
    let installed_pkgs_not_in_input_names: Vec<String> = all_installed_pkgs
        .iter()
        .filter(|p| !inputs_pkgs.contains(&p.name))
        .map(|p| p.name.clone())
        .collect();

    let installed_pkgs_in_input = pkgs_declarations
        .iter()
        .filter(|p| installed_pkgs_in_input_names.iter().any(|n| **n == p.name))
        .collect();

    let not_installed_pkgs_in_input = pkgs_declarations
        .iter()
        .filter(|p| {
            not_installed_pkgs_in_input_names
                .iter()
                .any(|n| **n == p.name)
        })
        .collect();

    let installed_pkgs_not_in_input = pkgs_declarations
        .iter()
        .filter(|p| installed_pkgs_not_in_input_names.contains(&p.name))
        .collect();

    Ok((
        installed_pkgs_in_input,
        not_installed_pkgs_in_input,
        installed_pkgs_not_in_input,
    ))
}

fn print_bridge_header(
    bridge_name: &str,
    pkgs_to_install_count: usize,
    pkgs_to_remove_count: usize,
    pkgs_to_update_count: usize,
) {
    println!(
        "{} {}: {}⤵️ {} 🗑️ {} 📦",
        "bridge:".green().bold(),
        bridge_name.underline().blue(),
        &pkgs_to_install_count.blue().bold(),
        &pkgs_to_remove_count.blue().bold(),
        &pkgs_to_update_count.blue().bold()
    );
}

fn print_job_header(job_name: &str) {
    println!("{} {}", "job:".green().bold(), job_name.purple());
}

fn perform_linking(fs: &fs::Fs, pb_style: ProgressStyle) -> Result<()> {
    let pb = ProgressBar::new(100);
    pb.set_style(pb_style);
    pb.set_message(format!("🔌 {}", "linking...".blue().bold()));
    let res = fs.link().map_err(|err| {
        pb.finish_with_message(format!("🔌 {}", "failed".red().bold()));
        println!("{}", err.red().bold());
        exit(1);
    });
    pb.finish_with_message(format!("🔌 {}", "done.".green().bold()));
    res
}
