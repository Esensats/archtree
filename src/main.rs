mod archiver;
mod config;
mod display;
mod exclusion;
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
    name = "archtree",
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

    let config = Config::builder()
        .output_path(args.output.as_deref().or(args.verify_only.as_deref()), true)
        .seven_zip_path(args.seven_zip_path.as_deref(), true)
        .show_progress(!args.quiet)
        .build()?;

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
        Some(file_path) => Box::new(FileReader::new(file_path)),
        None => Box::new(StdinReader::new()),
    };

    // Create and run backup service
    let service = BackupService::new(archiver, validator, reader, config.clone());
    service.run().await?;

    // Verify archive if requested
    if args.verify {
        let input_paths = service.get_input_paths().await?;
        verify_archive(&config.output_path, &input_paths, &args, &config).await?;
    }

    Ok(())
}

async fn verify_only_mode(archive_path: &str, args: &Args, config: &Config) -> Result<()> {
    // Create validator and reader (same as regular backup flow)
    let validator = FileSystemValidator::new();
    let reader: Box<dyn input::InputReader> = match &args.input_file {
        Some(file_path) => Box::new(FileReader::new(file_path)),
        None => Box::new(StdinReader::new()),
    };

    // Create a minimal backup service just to get the processed input paths
    // This ensures the same exclusion + validation logic as the regular backup flow
    let archiver = SevenZipArchiver::new(); // Not used, but required for the service
    let service = BackupService::new(archiver, validator, reader, config.clone());
    let input_paths = service.get_input_paths().await?;

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

        // Use strategy pattern to display missing files
        // Currently using Strategy 2 (consolidated), but Strategy 1 (detailed) is available if needed
        let display_context = display::MissingFileDisplayContext::with_consolidated_strategy();
        display_context.display_missing_files(&result);

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
