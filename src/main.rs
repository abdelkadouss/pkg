use miette::{IntoDiagnostic, miette};
use nix::unistd::{ForkResult, fork};

#[cfg(feature = "api")]
mod api;
mod bridge;
mod config;
mod fs;
mod pkg;
mod pkg_type;
mod process;
mod utils;

fn main() -> miette::Result<()> {
    match unsafe { fork() }.into_diagnostic()? {
        ForkResult::Child => {
            for _ in 1..1000 {
                println!("child still work!");
            }
            return Err(miette!("oops!"));
        }
        ForkResult::Parent { child } => {
            let res = nix::sys::wait::waitpid(child, None).into_diagnostic()?;
            println!("finaly parent released");
            println!("child finished");
            match res {
                nix::sys::wait::WaitStatus::Exited(pid, exit_code) => {
                    println!("exited, pid: {pid}, exit code: {exit_code}")
                }
                nix::sys::wait::WaitStatus::Signaled(pid, signal, some_bool) => {
                    println!(
                        "Signaled with signal: {:#?}, pid: {pid}, some bool: {some_bool}",
                        signal
                    )
                }
                nix::sys::wait::WaitStatus::Stopped(pid, signal) => {
                    println!("he stoped, pid: {pid}, signal: {:#?}", signal)
                }
                nix::sys::wait::WaitStatus::Continued(pid) => {
                    println!("he just continued, pid {pid}")
                }
                nix::sys::wait::WaitStatus::StillAlive => println!("he still alive"),
            }
        }
    };

    Ok(())
}
