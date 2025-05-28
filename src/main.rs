mod archiver;
mod config;
mod display;
mod exclusion;
mod input;
mod new_service;
mod path_processor;
mod service;
mod validator;
mod verification_service;
mod verifier;

use anyhow::Result;
use archiver::SevenZipArchiver;
use clap::Parser;
use config::Config;
use input::{FileReader, StdinReader};
use service::BackupService;
use new_service::BackupService as NewBackupService;
use validator::FileSystemValidator;
use verification_service::{ConsoleCallback, VerificationAndRetryService, VerificationMode};
use verifier::SevenZipVerifier;

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

    /// Use the new improved path processing algorithm
    #[arg(long = "new-algorithm")]
    new_algorithm: bool,
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
    if args.new_algorithm {
        let new_service = NewBackupService::new(archiver, reader, config.clone());
        new_service.run().await?;
        
        // Verify archive if requested
        if args.verify {
            let input_paths = new_service.get_input_paths().await?;

            // Create components for verification
            let verification_archiver = match &config.seven_zip_path {
                Some(path) => SevenZipArchiver::with_path(path.clone()),
                None => SevenZipArchiver::new(),
            };
            let verification_validator = FileSystemValidator::new();
            let verifier = match &config.seven_zip_path {
                Some(path) => SevenZipVerifier::with_path(path.clone()),
                None => SevenZipVerifier::new(),
            };

            // Use the new verification service
            let console_callback = ConsoleCallback::new(true); // Show progress for verification

            VerificationAndRetryService::verify(
                &config.output_path,
                &input_paths,
                &verification_archiver,
                &verification_validator,
                &verifier,
                VerificationMode::VerifyWithRetry,
                console_callback,
            )
            .await?;
        }
    } else {
        let service = BackupService::new(archiver, validator, reader, config.clone());
        service.run().await?;

        // Verify archive if requested
        if args.verify {
            let input_paths = service.get_input_paths().await?;

            // Create components for verification
            let verification_archiver = match &config.seven_zip_path {
                Some(path) => SevenZipArchiver::with_path(path.clone()),
                None => SevenZipArchiver::new(),
            };
            let verification_validator = FileSystemValidator::new();
            let verifier = match &config.seven_zip_path {
                Some(path) => SevenZipVerifier::with_path(path.clone()),
                None => SevenZipVerifier::new(),
            };

            // Use the new verification service
            let console_callback = ConsoleCallback::new(true); // Show progress for verification

            VerificationAndRetryService::verify(
                &config.output_path,
                &input_paths,
                &verification_archiver,
                &verification_validator,
                &verifier,
                VerificationMode::VerifyWithRetry,
                console_callback,
            )
            .await?;
        }
    }

    Ok(())
}

async fn verify_only_mode(archive_path: &str, args: &Args, config: &Config) -> Result<()> {
    // Create components needed for verification
    let validator = FileSystemValidator::new();
    let reader: Box<dyn input::InputReader> = match &args.input_file {
        Some(file_path) => Box::new(FileReader::new(file_path)),
        None => Box::new(StdinReader::new()),
    };

    let archiver = match &config.seven_zip_path {
        Some(path) => SevenZipArchiver::with_path(path.clone()),
        None => SevenZipArchiver::new(),
    };

    let verifier = match &config.seven_zip_path {
        Some(path) => SevenZipVerifier::with_path(path.clone()),
        None => SevenZipVerifier::new(),
    };

    // Get processed input paths using backup service logic
    let service = BackupService::new(archiver, validator, reader, config.clone());
    let input_paths = service.get_input_paths().await?;

    // Create new components for verification (since we can't access private fields)
    let verification_archiver = match &config.seven_zip_path {
        Some(path) => SevenZipArchiver::with_path(path.clone()),
        None => SevenZipArchiver::new(),
    };
    let verification_validator = FileSystemValidator::new();

    // Use the new verification service
    let console_callback = ConsoleCallback::new(true); // Show progress for verification

    VerificationAndRetryService::verify(
        archive_path,
        &input_paths,
        &verification_archiver,
        &verification_validator,
        &verifier,
        VerificationMode::VerifyOnly,
        console_callback,
    )
    .await?;

    Ok(())
}
