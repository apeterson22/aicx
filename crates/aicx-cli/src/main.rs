use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{anyhow, Result, Context};
use clap::{Parser, Subcommand};

use aicx_core::{
    archive_digests, compare_profiles, extract_archive, inspect_archive, list_archive_paths,
    pack_archive, render_toon, report_archive, verify_archive, ArchiveProfile, HashAlgorithm,
    PackOptions, Selection,
};

#[derive(Parser, Debug)]
#[command(name = "aicx")]
#[command(version = "0.1.0-featured")]
#[command(about = "Adaptive Intelligent Compression eXchange")]
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
    Digest {
        archive: PathBuf,
        #[arg(long, default_value = "json")]
        format: String,
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
    License {
        #[command(subcommand)]
        sub: LicenseSub,
    },
}

#[derive(Subcommand, Debug)]
enum LicenseSub {
    Status {
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Install {
        path: PathBuf,
    },
    Show {
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

fn parse_profile(value: &str) -> Result<ArchiveProfile> {
    ArchiveProfile::from_str(value).map_err(|err| anyhow!(err))
}

fn parse_hash(value: &str) -> Result<HashAlgorithm> {
    HashAlgorithm::from_str(value).map_err(|err| anyhow!(err))
}

fn run() -> Result<()> {
    let current_time_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    aicx_core::license::handle_license_reminders(current_time_secs);

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
            println!(
                "unpacked {} files into {}",
                manifest.file_count,
                out.display()
            );
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
            println!(
                "extracted {} files into {}",
                manifest.file_count,
                out.display()
            );
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
        Commands::Digest { archive, format } => {
            let output = archive_digests(&archive)?;
            match format.as_str() {
                "json" => println!("{}", serde_json::to_string_pretty(&output)?),
                "toon" => println!("{}", render_toon(&output)?),
                other => return Err(anyhow!("unsupported digest format: {other}")),
            }
        }
        Commands::Sidecar { archive, format } => {
            let inspection = inspect_archive(&archive)?;
            match format.as_str() {
                "json" => println!("{}", serde_json::to_string_pretty(&inspection.sidecar)?),
                "toon" => println!("{}", render_toon(&inspection.sidecar)?),
                other => return Err(anyhow!("unsupported sidecar format: {other}")),
            }
        }
        Commands::CompareProfiles { inputs, chunk_size } => {
            let rows = compare_profiles(&inputs, chunk_size)?;
            println!("{}", serde_json::to_string_pretty(&rows)?);
        }
        Commands::License { sub } => match sub {
            LicenseSub::Status { json } => {
                let current_time_secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs();
                let status = aicx_core::license::verify_license_status(current_time_secs);
                if json {
                    println!("{}", serde_json::to_string_pretty(&status)?);
                } else {
                    println!("License Status: {}", status.status.to_uppercase());
                    if let Some(org) = status.organization {
                        println!("Organization:   {}", org);
                    }
                    if let Some(tier) = status.tier {
                        println!("Tier:           {}", tier);
                    }
                    if let Some(seats) = status.seats {
                        println!("Seats:          {}", seats);
                    }
                    if let Some(expires) = status.expires_at {
                        println!("Expires At:     {}", expires);
                    }
                    if let Some(rem) = status.days_remaining {
                        println!("Days Remaining: {}", rem);
                    }
                }
            }
            LicenseSub::Install { path } => {
                let bytes = std::fs::read(&path).context("failed to read source license file")?;
                let lic: aicx_core::license::LicenseFile = serde_json::from_slice(&bytes)
                    .context("invalid license file format")?;
                aicx_core::license::check_license_integrity(&lic)
                    .context("license integrity check failed (invalid signature or unknown key)")?;

                let mut installed = false;
                
                // Try /etc/aegisqr
                if std::fs::create_dir_all("/etc/aegisqr").is_ok() {
                    if std::fs::write("/etc/aegisqr/license.aqlic", &bytes).is_ok() {
                        installed = true;
                        println!("Installed license to /etc/aegisqr/license.aqlic");
                    }
                }

                if !installed {
                    // Fallback to home folder
                    if let Ok(home) = std::env::var("HOME") {
                        let user_dir = format!("{}/.config/aegisqr", home);
                        let user_path = format!("{}/license.aqlic", user_dir);
                        std::fs::create_dir_all(&user_dir)?;
                        std::fs::write(&user_path, &bytes)?;
                        installed = true;
                        println!("Installed license to {}", user_path);
                    } else if let Ok(userprofile) = std::env::var("USERPROFILE") {
                        let user_dir = format!("{}/.config/aegisqr", userprofile);
                        let user_path = format!("{}/license.aqlic", user_dir);
                        std::fs::create_dir_all(&user_dir)?;
                        std::fs::write(&user_path, &bytes)?;
                        installed = true;
                        println!("Installed license to {}", user_path);
                    }
                }

                if !installed {
                    std::fs::write("./license.aqlic", &bytes)?;
                    println!("Installed license to ./license.aqlic");
                }
            }
            LicenseSub::Show { json } => {
                let mut found = false;
                for path in aicx_core::license::get_license_search_paths() {
                    if path.is_file() {
                        let content = std::fs::read_to_string(&path)?;
                        if json {
                            println!("{}", content.trim());
                        } else {
                            let lic: aicx_core::license::LicenseFile = serde_json::from_str(&content)?;
                            println!("{:#?}", lic);
                        }
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Err(anyhow!("No installed license found."));
                }
            }
        },
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{Cli, Commands};
    use aicx_core::render_toon;
    use clap::Parser;

    #[test]
    fn parses_digest_subcommand() {
        let cli = Cli::parse_from(["aicx", "digest", "archive.aicx", "--format", "toon"]);
        assert!(matches!(&cli.command, Commands::Digest { .. }));
        if let Commands::Digest { archive, format } = &cli.command {
            assert_eq!(archive, "archive.aicx");
            assert_eq!(format, "toon");
        }
    }

    #[test]
    fn digest_output_serializes_expected_fields() {
        let output = aicx_core::ArchiveDigests {
            manifest_digest: "manifest-digest".to_string(),
            sidecar_digest: "sidecar-digest".to_string(),
        };
        let json = serde_json::to_value(&output).expect("serialize digest output");
        assert_eq!(json["manifest_digest"], "manifest-digest");
        assert_eq!(json["sidecar_digest"], "sidecar-digest");

        let toon = render_toon(&output).expect("render toon");
        assert!(toon.contains("manifest_digest"));
        assert!(toon.contains("sidecar_digest"));
    }
}
