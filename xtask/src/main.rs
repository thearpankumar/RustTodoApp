use std::{env, path::PathBuf, process::Command};

type DynError = Box<dyn std::error::Error>;

fn main() {
    if let Err(e) = try_main() {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

fn try_main() -> Result<(), DynError> {
    let task = env::args().nth(1);

    match task.as_deref() {
        Some("ci") => task_ci()?,
        _ => print_help(),
    }
    Ok(())
}

fn print_help() {
    eprintln!(
        "Tasks:
ci     Run formatting, linting, and check
"
    )
}

fn task_ci() -> Result<(), DynError> {
    println!("cargo-xtask: Running CI checks...");

    // 1. Format Check
    println!("\n➔ Running cargo fmt...");
    let status = Command::new("cargo")
        .args(["fmt", "--", "--check"])
        .current_dir(project_root())
        .status()?;
    if !status.success() {
        return Err("cargo fmt failed. Please run 'cargo fmt' to fix formatting.".into());
    }

    // 2. Clippy
    println!("\n➔ Running cargo clippy...");
    let status = Command::new("cargo")
        .args(["clippy", "--", "-D", "warnings"])
        .current_dir(project_root())
        .status()?;
    if !status.success() {
        return Err("cargo clippy failed. Please fix lint errors.".into());
    }

    // 3. Check (or Test)
    println!("\n➔ Running cargo check...");
    let status = Command::new("cargo")
        .args(["check"])
        .current_dir(project_root())
        .status()?;
    if !status.success() {
        return Err("cargo check failed. Compilation error.".into());
    }

    println!("\n✅ CI commands passed successfully!");
    Ok(())
}

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}
