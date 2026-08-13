mod builder;
mod crack;
mod dictionary;
mod hash;
mod scanner;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "lol-hash-cracker",
    version,
    about = "League hash corpus scanner and smart dictionary cracker"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Build {
        #[arg(long, default_value = "output")]
        output: PathBuf,
        #[arg(long, default_value = "work")]
        work: PathBuf,
    },
    Scan {
        league: PathBuf,
        #[arg(long)]
        hashes: Vec<PathBuf>,
        #[arg(long)]
        meta: Option<PathBuf>,
        #[arg(long, default_value = "hash-scan.json")]
        output: PathBuf,
        #[arg(long)]
        scan_unknown_chunks: bool,
    },
    Crack {
        targets: PathBuf,
        #[arg(long)]
        dictionary: Vec<PathBuf>,
        #[arg(long)]
        meta: Option<PathBuf>,
        #[arg(long)]
        report: Option<PathBuf>,
        #[arg(long, default_value_t = 3)]
        depth: usize,
        #[arg(long, default_value = "cracked.txt")]
        output: PathBuf,
    },
    Run {
        league: PathBuf,
        #[arg(long)]
        hashes: Vec<PathBuf>,
        #[arg(long)]
        meta: Option<PathBuf>,
        #[arg(long)]
        dictionary: Vec<PathBuf>,
        #[arg(long, default_value = "hash-run")]
        output: PathBuf,
        #[arg(long, default_value_t = 3)]
        depth: usize,
        #[arg(long)]
        scan_unknown_chunks: bool,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Build { output, work } => builder::run(&output, &work),
        Command::Scan {
            league,
            hashes,
            meta,
            output,
            scan_unknown_chunks,
        } => {
            let known = dictionary::load_known_hashes(&hashes, meta.as_deref())?;
            let report = scanner::scan(&league, &known, scan_unknown_chunks)?;
            scanner::write_report(&output, &report)?;
            scanner::write_unresolved(&output.with_extension("unresolved.txt"), &report)
        }
        Command::Crack {
            targets,
            dictionary,
            meta,
            report,
            depth,
            output,
        } => crack::run(
            &targets,
            &dictionary,
            meta.as_deref(),
            report.as_deref(),
            depth,
            &output,
        ),
        Command::Run {
            league,
            hashes,
            meta,
            dictionary,
            output,
            depth,
            scan_unknown_chunks,
        } => {
            std::fs::create_dir_all(&output)?;
            let known = dictionary::load_known_hashes(&hashes, meta.as_deref())?;
            let report = scanner::scan(&league, &known, scan_unknown_chunks)?;
            let report_path = output.join("scan.json");
            scanner::write_report(&report_path, &report)?;
            let target_path = output.join("unresolved.txt");
            scanner::write_unresolved(&target_path, &report)?;
            crack::run(
                &target_path,
                &dictionary,
                meta.as_deref(),
                Some(&report_path),
                depth,
                &output.join("cracked.txt"),
            )
        }
    }
}
