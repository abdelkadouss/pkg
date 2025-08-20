use clap::Parser;
use miette::{IntoDiagnostic, Result};
use pkg::{
    bridge,
    cmd::{Cli, Commands},
    config::Config,
    db, fs, input,
};
use rpassword::read_password;
use std::process::{Command, Stdio};
use std::{
    io::{self, Write},
    path::PathBuf,
};

const DEFAULT_CONFIG_FILE_NAME: &str = "config";
const DEFAULT_CONFIG_FILE_EXTENSION: &str = "kdl";

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
    let inputs_path = config.path.clone();

    let db = db::Db::new(&db_path)?;

    let input = input::Input::load(&inputs_path)?;

    // let needed_bridges = input.bridges.iter().map(|b| b.name.clone()).collect();

    // let bridge_api = bridge::BridgeApi::new(bridges_set, needed_bridges, &db_path)?;

    // let fs = fs::Fs::new(target_dir, load_path, &db_path);

    // Handle commands
    match &cli.command {
        Commands::Build { update } => {
            println!("Building packages (syncing with config)");
            if *update {
                println!("update mode enabled");
            }
            // Call your build function here
        }

        Commands::Rebuild => {
            println!("Rebuilding packages (force sync)");
            // Call your rebuild function here
        }

        // ... rest of your command handling ...
        _ => {} // Handle other commands
    }

    Ok(())
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
    println!("This operation requires administrator privileges.");
    print!("Please enter your password: ");
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

fn build(
    db: &db::Db,
    bridge_api: &bridge::BridgeApi,
    input: &input::Input,
    fs: &fs::Fs,
) -> Result<()> {
    for bridge in &input.bridges {}

    Ok(())
}
