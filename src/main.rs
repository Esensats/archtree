mod archiver;
mod config;
mod input;
mod service;
mod validator;
mod verifier;

use anyhow::Result;
use archiver::{Archiver, SevenZipArchiver};
use clap::Parser;
use config::Config;
use input::{FileReader, StdinReader};
use service::BackupService;
use validator::{FileSystemValidator, PathValidator};
use verifier::{SevenZipVerifier, VerificationService};

#[derive(Parser)]
#[command(
    name = "make-archive",
    about = "A PowerShell-compatible backup tool that creates compressed archives using 7-Zip",
    version = "1.0.0"
)]
struct Args {
    /// Input file containing paths to backup (reads from stdin if not provided)
    #[arg(short = 'f', long = "file")]
    input_file: Option<String>,

    /// Output archive path (overrides environment variables)
    #[arg(short = 'o', long = "output")]
    output: Option<String>,

    /// Path to 7-Zip executable
    #[arg(long = "7zip-path")]
    seven_zip_path: Option<String>,

    /// Disable progress output
    #[arg(short = 'q', long = "quiet")]
    quiet: bool,

    /// Verify archive contents after creation
    #[arg(short = 'v', long = "verify")]
    verify: bool,

    /// Retry missing files (requires --verify)
    #[arg(short = 'r', long = "retry")]
    retry: bool,

    /// Only verify an existing archive without creating a new one
    #[arg(long = "verify-only")]
    verify_only: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load base configuration from environment
    let mut config = Config::from_env()?;

    // Override with command line arguments
    if let Some(ref output) = args.output {
        config = config.with_output_path(output.clone());
    }

    if let Some(ref seven_zip_path) = args.seven_zip_path {
        config = config.with_seven_zip_path(seven_zip_path.clone());
    }

    if args.quiet {
        config = config.with_progress(false);
    }

    // Handle verify-only mode
    if let Some(ref archive_path) = args.verify_only {
        return verify_only_mode(archive_path, &args, &config).await;
    }

    // Create archiver with custom path if specified
    let archiver = match &config.seven_zip_path {
        Some(path) => SevenZipArchiver::with_path(path.clone()),
        None => SevenZipArchiver::new(),
    };

    // Create validator
    let validator = FileSystemValidator::new();

    // Create reader based on input source
    let reader: Box<dyn input::InputReader> = match &args.input_file {
        Some(file_path) => Box::new(FileReader::new(file_path.clone())),
        None => Box::new(StdinReader::new()),
    };

    // Create and run backup service
    let service = BackupService::new(archiver, validator, reader, config.clone());
    let input_paths = service.get_input_paths().await?;
    service.run().await?;

    // Verify archive if requested
    if args.verify {
        verify_archive(&config.output_path, &input_paths, &args, &config).await?;
    }

    Ok(())
}

async fn verify_only_mode(archive_path: &str, args: &Args, config: &Config) -> Result<()> {
    // Create reader to get input paths for verification
    let reader: Box<dyn input::InputReader> = match &args.input_file {
        Some(file_path) => Box::new(FileReader::new(file_path.clone())),
        None => Box::new(StdinReader::new()),
    };

    let input_paths = reader.read_paths().await?;
    verify_archive(archive_path, &input_paths, args, config).await
}

async fn verify_archive(
    archive_path: &str,
    input_paths: &[String],
    args: &Args,
    config: &Config,
) -> Result<()> {
    if config.show_progress {
        println!("🔍 Verifying archive contents...");
    }

    // Create verifier with same path as archiver
    let verifier = match &config.seven_zip_path {
        Some(path) => SevenZipVerifier::with_path(path.clone()),
        None => SevenZipVerifier::new(),
    };

    let verification_service = VerificationService::new(verifier);
    let result = verification_service
        .verify_archive(archive_path, input_paths)
        .await?;

    // Display results
    println!("📊 Verification Results:");
    println!(
        "  ✅ Successfully archived: {}/{} files ({:.1}%)",
        result.archived_files.len(),
        result.total_expected,
        result.success_rate()
    );

    if !result.missing_files.is_empty() {
        println!("  ❌ Missing files: {}", result.missing_files.len());
        for missing in &result.missing_files {
            println!("    - {}", missing);
        }

        // Offer to retry missing files
        if args.retry {
            if config.show_progress {
                println!("🔄 Retrying missing files...");
            }

            // Create archiver for retry
            let archiver = match &config.seven_zip_path {
                Some(path) => SevenZipArchiver::with_path(path.clone()),
                None => SevenZipArchiver::new(),
            };

            // Validate missing files before retry
            let validator = FileSystemValidator::new();
            let valid_missing = validator.validate_paths(&result.missing_files).await?;

            if !valid_missing.is_empty() {
                // Use 7z update command to add missing files
                archiver
                    .add_to_archive(&valid_missing, archive_path)
                    .await?;
                println!(
                    "✅ Retry completed. {} files added to archive.",
                    valid_missing.len()
                );

                // Verify again after retry
                let retry_result = verification_service
                    .verify_archive(archive_path, input_paths)
                    .await?;
                println!(
                    "📊 Final Results: {}/{} files ({:.1}%)",
                    retry_result.archived_files.len(),
                    retry_result.total_expected,
                    retry_result.success_rate()
                );
            } else {
                println!("⚠️  No valid missing files found to retry.");
            }
        } else {
            println!("💡 Use --retry flag to automatically attempt adding missing files.");
        }
    } else {
        println!("🎉 All files successfully archived!");
    }

    Ok(())
}
