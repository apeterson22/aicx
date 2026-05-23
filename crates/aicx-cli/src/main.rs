use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};

use aicx_core::{
    compare_profiles, extract_archive, inspect_archive, list_archive_paths, pack_archive,
    report_archive, verify_archive, ArchiveProfile, HashAlgorithm, PackOptions, Selection,
};

#[derive(Parser, Debug)]
#[command(name = "aicx", about = "Adaptive Intelligent Compression eXchange")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Pack {
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
        #[arg(long = "out", short = 'o')]
        out: PathBuf,
        #[arg(long, default_value = "balanced")]
        profile: String,
        #[arg(long = "chunk-size", default_value_t = 4 * 1024 * 1024)]
        chunk_size: usize,
        #[arg(long, default_value = "blake3")]
        hash: String,
    },
    Unpack {
        archive: PathBuf,
        #[arg(long = "out", short = 'o')]
        out: PathBuf,
        #[arg(long)]
        exact: bool,
        #[arg(long)]
        overwrite: bool,
        #[arg(value_name = "PATH")]
        paths: Vec<String>,
    },
    Inspect {
        archive: PathBuf,
    },
    List {
        archive: PathBuf,
    },
    Extract {
        archive: PathBuf,
        path: String,
        #[arg(long = "out", short = 'o')]
        out: PathBuf,
        #[arg(long)]
        exact: bool,
        #[arg(long)]
        overwrite: bool,
    },
    Verify {
        archive: PathBuf,
    },
    Report {
        archive: PathBuf,
    },
    Sidecar {
        archive: PathBuf,
        #[arg(long, default_value = "json")]
        format: String,
    },
    CompareProfiles {
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
        #[arg(long = "chunk-size", default_value_t = 4 * 1024 * 1024)]
        chunk_size: usize,
    },
}

fn parse_profile(value: &str) -> Result<ArchiveProfile> {
    ArchiveProfile::from_str(value).map_err(|err| anyhow!(err))
}

fn parse_hash(value: &str) -> Result<HashAlgorithm> {
    HashAlgorithm::from_str(value).map_err(|err| anyhow!(err))
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Pack {
            inputs,
            out,
            profile,
            chunk_size,
            hash,
        } => {
            let options = PackOptions {
                chunk_size,
                profile: parse_profile(&profile)?,
                hash_algorithm: parse_hash(&hash)?,
            };
            let manifest = pack_archive(&inputs, &out, options)?;
            println!(
                "packed {} files into {} (ratio {:.3})",
                manifest.file_count,
                out.display(),
                manifest.compression_ratio
            );
        }
        Commands::Unpack {
            archive,
            out,
            exact,
            overwrite,
            paths,
        } => {
            let selection = Selection { paths, exact };
            let manifest = extract_archive(&archive, &out, selection, overwrite)?;
            println!("unpacked {} files into {}", manifest.file_count, out.display());
        }
        Commands::Inspect { archive } => {
            let inspection = inspect_archive(&archive)?;
            println!("{}", serde_json::to_string_pretty(&inspection)?);
        }
        Commands::List { archive } => {
            for path in list_archive_paths(&archive)? {
                println!("{path}");
            }
        }
        Commands::Extract {
            archive,
            path,
            out,
            exact,
            overwrite,
        } => {
            let selection = Selection {
                paths: vec![path],
                exact,
            };
            let manifest = extract_archive(&archive, &out, selection, overwrite)?;
            println!("extracted {} files into {}", manifest.file_count, out.display());
        }
        Commands::Verify { archive } => {
            let verification = verify_archive(&archive)?;
            println!("{}", serde_json::to_string_pretty(&verification)?);
            if !verification.valid {
                return Err(anyhow!(verification.issues.join("; ")));
            }
        }
        Commands::Report { archive } => {
            let report = report_archive(&archive)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::Sidecar { archive, format } => {
            let inspection = inspect_archive(&archive)?;
            match format.as_str() {
                "json" | "toon" => println!("{}", serde_json::to_string_pretty(&inspection.sidecar)?),
                other => return Err(anyhow!("unsupported sidecar format: {other}")),
            }
        }
        Commands::CompareProfiles { inputs, chunk_size } => {
            let rows = compare_profiles(&inputs, chunk_size)?;
            println!("{}", serde_json::to_string_pretty(&rows)?);
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

