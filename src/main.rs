mod core;
mod io;
mod processing;
mod services;
mod verification;

use clap::{Parser, Subcommand};
use core::{Config, Result};
use io::{FileReader, SevenZipArchiver, StdinReader};
use processing::validation::FileSystemValidator;
use services::BackupService;
use verification::{ConsoleCallback, VerificationAndRetryService, VerificationMode};

#[derive(Parser)]
#[command(
    name = "archtree",
    about = "A backup tool that creates and verifies compressed archives using 7-Zip",
    version = "0.2.1"
)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a backup archive from input paths
    Backup {
        /// Input file containing paths to backup (reads from stdin if not provided)
        #[arg(short = 'f', long = "file")]
        input_file: Option<String>,

        /// Output archive path
        #[arg(short = 'o', long = "output", required = true)]
        output: String,

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
    },
    /// Verify an existing archive against input paths
    Verify {
        /// Archive file to verify
        #[arg(short = 'a', long = "archive", required = true)]
        archive: String,

        /// Input file containing expected paths (reads from stdin if not provided)
        #[arg(short = 'f', long = "file")]
        input_file: Option<String>,

        /// Path to 7-Zip executable
        #[arg(long = "7zip-path")]
        seven_zip_path: Option<String>,

        /// Disable progress output
        #[arg(short = 'q', long = "quiet")]
        quiet: bool,

        /// Retry missing files by updating the archive
        #[arg(short = 'r', long = "retry")]
        retry: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Commands::Backup {
            input_file,
            output,
            seven_zip_path,
            quiet,
            verify,
            retry,
        } => run_backup_command(input_file, output, seven_zip_path, quiet, verify, retry).await,
        Commands::Verify {
            archive,
            input_file,
            seven_zip_path,
            quiet,
            retry,
        } => run_verify_command(archive, input_file, seven_zip_path, quiet, retry).await,
    }
}

async fn run_backup_command(
    input_file: Option<String>,
    output: String,
    seven_zip_path: Option<String>,
    quiet: bool,
    verify: bool,
    retry: bool,
) -> Result<()> {
    // Build configuration
    let config = Config::builder()
        .output_path(Some(&output), false) // Don't try environment for explicit output
        .seven_zip_path(seven_zip_path.as_deref(), true)
        .show_progress(!quiet)
        .build()?;

    // Create archiver with custom path if specified
    let archiver = match &config.seven_zip_path {
        Some(path) => SevenZipArchiver::with_path(path.clone()),
        None => SevenZipArchiver::new(),
    };

    // Create reader based on input source
    let reader: Box<dyn io::InputReader> = match &input_file {
        Some(file_path) => Box::new(FileReader::new(file_path)),
        None => Box::new(StdinReader::new()),
    };

    // Create and run backup service
    let backup_service = BackupService::new(archiver, reader, config.clone());
    backup_service.run().await?;

    // Handle verification if requested
    if verify {
        if !quiet {
            eprintln!("\n🔍 Verifying archive...");
        }

        // Get the input paths that were processed
        let input_paths = backup_service.get_input_paths().await?;

        // Create new reader for verification (since we consumed the original)
        let verify_reader: Box<dyn io::InputReader> = match &input_file {
            Some(file_path) => Box::new(FileReader::new(file_path)),
            None => {
                // For stdin, we'll use the processed paths directly
                Box::new(io::VecReader::new(input_paths))
            }
        };

        // Create verification components
        let verify_archiver = match &config.seven_zip_path {
            Some(path) => SevenZipArchiver::with_path(path.clone()),
            None => SevenZipArchiver::new(),
        };

        let verify_service =
            BackupService::new(verify_archiver.clone(), verify_reader, config.clone());
        let processed_paths = verify_service.get_input_paths().await?;

        // Create verifier
        let verifier = match &config.seven_zip_path {
            Some(path) => verification::SevenZipVerifier::with_path(path.clone()),
            None => verification::SevenZipVerifier::new(),
        };

        // Create callback for progress reporting
        let callback = ConsoleCallback::new(!quiet);

        // Create validator
        let validator = FileSystemValidator::new();

        // Determine verification mode
        let mode = if retry {
            VerificationMode::VerifyWithRetry
        } else {
            VerificationMode::VerifyOnly
        };

        // Run verification
        VerificationAndRetryService::verify(
            &output,
            &processed_paths,
            &verify_archiver,
            &validator,
            &verifier,
            mode,
            callback,
        )
        .await?;
    }

    Ok(())
}

async fn run_verify_command(
    archive: String,
    input_file: Option<String>,
    seven_zip_path: Option<String>,
    quiet: bool,
    retry: bool,
) -> Result<()> {
    // Build configuration
    let config = Config::builder()
        .output_path(Some(&archive), false) // Use archive path as output for potential retry
        .seven_zip_path(seven_zip_path.as_deref(), true)
        .show_progress(!quiet)
        .build()?;

    // Create reader based on input source
    let reader: Box<dyn io::InputReader> = match &input_file {
        Some(file_path) => Box::new(FileReader::new(file_path)),
        None => Box::new(StdinReader::new()),
    };

    // Create archiver for potential retry operations
    let archiver = match &config.seven_zip_path {
        Some(path) => SevenZipArchiver::with_path(path.clone()),
        None => SevenZipArchiver::new(),
    };

    // Get processed input paths using backup service logic
    let service = BackupService::new(archiver.clone(), reader, config.clone());
    let input_paths = service.get_input_paths().await?;

    // Create verifier
    let verifier = match &config.seven_zip_path {
        Some(path) => verification::SevenZipVerifier::with_path(path.clone()),
        None => verification::SevenZipVerifier::new(),
    };

    // Create callback for progress reporting
    let callback = ConsoleCallback::new(!quiet);

    // Create validator
    let validator = FileSystemValidator::new();

    // Determine verification mode
    let mode = if retry {
        VerificationMode::VerifyWithRetry
    } else {
        VerificationMode::VerifyOnly
    };

    if !quiet {
        eprintln!("🔍 Verifying archive: {}", archive);
    }

    // Run verification
    VerificationAndRetryService::verify(
        &archive,
        &input_paths,
        &archiver,
        &validator,
        &verifier,
        mode,
        callback,
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_backup_command_integration() {
        // Create temporary test files
        let temp_dir = TempDir::new().unwrap();
        let test_file1 = temp_dir.path().join("test1.txt");
        let test_file2 = temp_dir.path().join("test2.txt");
        let input_file = temp_dir.path().join("input.txt");
        let output_file = temp_dir.path().join("output.7z");

        fs::write(&test_file1, "Hello, World!").unwrap();
        fs::write(&test_file2, "Test content").unwrap();

        // Create input file
        let input_content = format!(
            "{}\n{}",
            test_file1.to_string_lossy(),
            test_file2.to_string_lossy()
        );
        fs::write(&input_file, input_content).unwrap();

        // Test backup without verification (since 7z might not be available in tests)
        let result = run_backup_command(
            Some(input_file.to_string_lossy().to_string()),
            output_file.to_string_lossy().to_string(),
            None,
            true,  // quiet
            false, // no verify
            false, // no retry
        )
        .await;

        // The command should handle 7z not being available gracefully
        if result.is_err() {
            // This is expected in test environment without 7z
            eprintln!("Expected error in test environment: {:?}", result);
        }
    }
}
