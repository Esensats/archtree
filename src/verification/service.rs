use crate::{
    core::Result,
    io::Archiver,
    processing::validation::PathValidator,
    verification::{
        display,
        verifier::{ArchiveVerifier, VerificationResult},
    },
};

/// Events that occur during verification process
#[derive(Debug, Clone)]
pub enum VerificationEvent {
    /// Verification process is starting
    Starting,
    /// Archive listing completed
    ArchiveListingComplete { entries_found: usize },
    /// File comparison completed
    ComparisonComplete {
        missing: usize,
        found: usize,
        total_expected: usize,
    },
    /// Displaying missing files to user
    DisplayingMissingFiles { count: usize },
    /// Freshness checking is starting
    FreshnessCheckStarting,
    /// Freshness checking completed
    FreshnessCheckComplete {
        outdated: usize,
        up_to_date: usize,
        unverifiable: usize,
        total_checked: usize,
    },
    /// Displaying outdated files to user
    DisplayingOutdatedFiles { count: usize },
    /// Starting update of outdated files
    UpdatingOutdatedFiles { files_to_update: usize },
    /// Update of outdated files completed
    UpdateOutdatedComplete { files_updated: usize },
    /// Starting retry process
    RetryStarting { files_to_retry: usize },
    /// Retry operation completed
    RetryComplete { files_added: usize },
    /// Final verification after retry completed
    RetryVerificationComplete {
        final_missing: usize,
        final_found: usize,
        final_total: usize,
    },
    /// Entire process completed successfully
    Complete { mode: VerificationMode },
}

/// Trait for handling verification progress callbacks
pub trait VerificationCallback: Send + Sync {
    /// Called when a verification event occurs
    fn on_event(&self, event: VerificationEvent);
}

/// Console-based callback implementation for CLI output
pub struct ConsoleCallback {
    show_progress: bool,
}

impl ConsoleCallback {
    pub fn new(show_progress: bool) -> Self {
        Self { show_progress }
    }
}

impl VerificationCallback for ConsoleCallback {
    fn on_event(&self, event: VerificationEvent) {
        if !self.show_progress {
            return;
        }

        match event {
            VerificationEvent::Starting => {
                eprintln!("🔍 Verifying archive contents...");
            }
            VerificationEvent::ArchiveListingComplete { entries_found: _ } => {
                // Could add debug info here if needed
            }
            VerificationEvent::ComparisonComplete {
                missing,
                found,
                total_expected,
            } => {
                eprintln!("📊 Verification Results:");
                let success_rate = if total_expected > 0 {
                    found as f64 / total_expected as f64 * 100.0
                } else {
                    100.0
                };
                eprintln!(
                    "  ✅ Successfully archived: {}/{} files ({:.1}%)",
                    found, total_expected, success_rate
                );
                if missing > 0 {
                    eprintln!("  ❌ Missing files: {}", missing);
                }
            }
            VerificationEvent::DisplayingMissingFiles { count: _ } => {
                // Missing files are displayed by the display strategy
            }
            VerificationEvent::FreshnessCheckStarting => {
                eprintln!("🕒 Checking file freshness...");
            }
            VerificationEvent::FreshnessCheckComplete {
                outdated,
                up_to_date,
                unverifiable,
                total_checked,
            } => {
                eprintln!("📊 Freshness Check Results:");
                eprintln!(
                    "  ✅ Up-to-date files: {}/{} ({:.1}%)",
                    up_to_date,
                    total_checked,
                    if total_checked > 0 {
                        up_to_date as f64 / total_checked as f64 * 100.0
                    } else {
                        100.0
                    }
                );
                if outdated > 0 {
                    eprintln!("  ⚠️  Outdated files: {}", outdated);
                }
                if unverifiable > 0 {
                    eprintln!("  ❓ Unverifiable files: {}", unverifiable);
                }
            }
            VerificationEvent::DisplayingOutdatedFiles { count: _ } => {
                // Outdated files are displayed by the display strategy
            }
            VerificationEvent::UpdatingOutdatedFiles { files_to_update } => {
                eprintln!("🔄 Updating outdated files... ({} files)", files_to_update);
            }
            VerificationEvent::UpdateOutdatedComplete { files_updated } => {
                eprintln!(
                    "✅ Update completed. {} files updated in archive.",
                    files_updated
                );
            }
            VerificationEvent::RetryStarting { files_to_retry } => {
                eprintln!("🔄 Retrying missing files... ({} files)", files_to_retry);
            }
            VerificationEvent::RetryComplete { files_added } => {
                eprintln!(
                    "✅ Retry completed. {} files added to archive.",
                    files_added
                );
            }
            VerificationEvent::RetryVerificationComplete {
                final_missing: _,
                final_found,
                final_total,
            } => {
                let final_success_rate = if final_total > 0 {
                    final_found as f64 / final_total as f64 * 100.0
                } else {
                    100.0
                };
                eprintln!(
                    "📊 Final Results: {}/{} files ({:.1}%)",
                    final_found, final_total, final_success_rate
                );
            }
            VerificationEvent::Complete { mode } => {
                eprintln!("🎉 All files successfully archived!");
                match mode {
                    VerificationMode::VerifyOnly => {
                        eprintln!(
                            "💡 Use --retry flag to automatically attempt adding missing files."
                        )
                    }
                    VerificationMode::VerifyWithRetry => {}
                }
            }
        }
    }
}

/// Verification mode enumeration
#[derive(Debug, Clone, Copy)]
pub enum VerificationMode {
    /// Only verify, don't retry missing files
    VerifyOnly,
    /// Verify and retry missing files if any are found
    VerifyWithRetry,
}

/// Service for handling verification and retry operations with callback support
pub struct VerificationAndRetryService;

impl VerificationAndRetryService {
    /// Verify archive contents with optional retry and progress callbacks
    pub async fn verify<A, V, R, C>(
        archive_path: &str,
        input_paths: &[String],
        archiver: &A,
        validator: &V,
        verifier: &R,
        mode: VerificationMode,
        callback: C,
    ) -> Result<VerificationResult>
    where
        A: Archiver,
        V: PathValidator,
        R: ArchiveVerifier + Clone,
        C: VerificationCallback,
    {
        callback.on_event(VerificationEvent::Starting);

        // Verify archive directly with the verifier
        let result = verifier.verify_archive(archive_path, input_paths).await?;

        // Notify completion of comparison
        callback.on_event(VerificationEvent::ComparisonComplete {
            missing: result.missing_files.len(),
            found: result.archived_files.len(),
            total_expected: result.total_expected,
        });

        if !result.missing_files.is_empty() {
            // Display missing files using the strategy pattern
            callback.on_event(VerificationEvent::DisplayingMissingFiles {
                count: result.missing_files.len(),
            });
            let display_context = display::MissingFileDisplayContext::with_consolidated_strategy();
            display_context.display_missing_files(&result);

            // Handle retry if requested
            match mode {
                VerificationMode::VerifyWithRetry => {
                    return Self::retry_missing_files(
                        archive_path,
                        input_paths,
                        &result,
                        archiver,
                        validator,
                        verifier,
                        callback,
                    )
                    .await;
                }
                VerificationMode::VerifyOnly => {
                    // No action needed
                }
            }
        } else {
            callback.on_event(VerificationEvent::Complete { mode });
        }

        Ok(result)
    }

    /// Retry adding missing files to the archive
    async fn retry_missing_files<A, V, R, C>(
        archive_path: &str,
        input_paths: &[String],
        verification_result: &VerificationResult,
        archiver: &A,
        validator: &V,
        verifier: &R,
        callback: C,
    ) -> Result<VerificationResult>
    where
        A: Archiver,
        V: PathValidator,
        R: ArchiveVerifier + Clone,
        C: VerificationCallback,
    {
        // Validate missing files before retry
        let valid_missing = validator
            .validate_paths(&verification_result.missing_files)
            .await?;

        if !valid_missing.is_empty() {
            callback.on_event(VerificationEvent::RetryStarting {
                files_to_retry: valid_missing.len(),
            });

            // Use archiver to add missing files
            archiver
                .add_to_archive(&valid_missing, archive_path)
                .await?;

            callback.on_event(VerificationEvent::RetryComplete {
                files_added: valid_missing.len(),
            });

            // Verify again after retry
            let retry_result = verifier.verify_archive(archive_path, input_paths).await?;

            callback.on_event(VerificationEvent::RetryVerificationComplete {
                final_missing: retry_result.missing_files.len(),
                final_found: retry_result.archived_files.len(),
                final_total: retry_result.total_expected,
            });

            if retry_result.missing_files.is_empty() {
                callback.on_event(VerificationEvent::Complete {
                    mode: VerificationMode::VerifyWithRetry,
                });
            }

            Ok(retry_result)
        } else {
            eprintln!("⚠️  No valid missing files found to retry.");
            Ok(verification_result.clone())
        }
    }

    /// Verify archive contents with optional freshness checking
    pub async fn verify_with_freshness<A, V, R, C>(
        archive_path: &str,
        input_paths: &[String],
        archiver: &A,
        validator: &V,
        verifier: &R,
        mode: VerificationMode,
        check_freshness: bool,
        update_outdated: bool,
        callback: C,
    ) -> Result<VerificationResult>
    where
        A: Archiver,
        V: PathValidator,
        R: ArchiveVerifier + Clone,
        C: VerificationCallback,
    {
        callback.on_event(VerificationEvent::Starting);

        // Verify archive directly with the verifier
        let result = verifier.verify_archive(archive_path, input_paths).await?;

        // Notify completion of comparison
        callback.on_event(VerificationEvent::ComparisonComplete {
            missing: result.missing_files.len(),
            found: result.archived_files.len(),
            total_expected: result.total_expected,
        });

        if !result.missing_files.is_empty() {
            // Display missing files using the strategy pattern
            callback.on_event(VerificationEvent::DisplayingMissingFiles {
                count: result.missing_files.len(),
            });
            let display_context = display::MissingFileDisplayContext::with_consolidated_strategy();
            display_context.display_missing_files(&result);

            // Handle retry if requested
            match mode {
                VerificationMode::VerifyWithRetry => {
                    return Self::retry_missing_files(
                        archive_path,
                        input_paths,
                        &result,
                        archiver,
                        validator,
                        verifier,
                        callback,
                    )
                    .await;
                }
                VerificationMode::VerifyOnly => {
                    // No action needed
                }
            }
        } else {
            callback.on_event(VerificationEvent::Complete { mode });
        }

        // If freshness checking is requested and there are no missing files,
        // proceed with freshness verification
        if check_freshness && result.missing_files.is_empty() {
            callback.on_event(VerificationEvent::FreshnessCheckStarting);

            let freshness_result = verifier
                .verify_archive_freshness(archive_path, input_paths)
                .await?;

            callback.on_event(VerificationEvent::FreshnessCheckComplete {
                outdated: freshness_result.outdated_files.len(),
                up_to_date: freshness_result.up_to_date_files.len(),
                unverifiable: freshness_result.unverifiable_files.len(),
                total_checked: freshness_result.total_checked,
            });

            if !freshness_result.outdated_files.is_empty() {
                callback.on_event(VerificationEvent::DisplayingOutdatedFiles {
                    count: freshness_result.outdated_files.len(),
                });

                // Display outdated files
                println!("⚠️  Outdated files found in archive:");
                for outdated in &freshness_result.outdated_files {
                    println!("  📄 {}", outdated.path);
                    if let (Some(archive_time), Some(fs_time)) =
                        (&outdated.archive_modified, &outdated.filesystem_modified)
                    {
                        // Convert SystemTime to more readable format
                        use std::time::UNIX_EPOCH;
                        let archive_duration =
                            archive_time.duration_since(UNIX_EPOCH).unwrap_or_default();
                        let fs_duration = fs_time.duration_since(UNIX_EPOCH).unwrap_or_default();
                        let archive_secs = archive_duration.as_secs();
                        let fs_secs = fs_duration.as_secs();

                        // Simple time difference display
                        let time_diff = fs_secs.saturating_sub(archive_secs);
                        if time_diff > 3600 {
                            println!(
                                "    📅 Archive is {:.1} hours older than filesystem",
                                time_diff as f64 / 3600.0
                            );
                        } else if time_diff > 60 {
                            println!(
                                "    📅 Archive is {} minutes older than filesystem",
                                time_diff / 60
                            );
                        } else {
                            println!(
                                "    📅 Archive is {} seconds older than filesystem",
                                time_diff
                            );
                        }
                    }
                }

                // Handle updating outdated files if requested
                if update_outdated && !freshness_result.outdated_files.is_empty() {
                    callback.on_event(VerificationEvent::UpdatingOutdatedFiles {
                        files_to_update: freshness_result.outdated_files.len(),
                    });

                    // Extract paths of outdated files for updating
                    let outdated_paths: Vec<String> = freshness_result
                        .outdated_files
                        .iter()
                        .map(|f| f.path.clone())
                        .collect();

                    // Use archiver to update the outdated files in the archive
                    archiver
                        .add_to_archive(&outdated_paths, archive_path)
                        .await?;

                    callback.on_event(VerificationEvent::UpdateOutdatedComplete {
                        files_updated: outdated_paths.len(),
                    });

                    println!("✅ All outdated files have been updated in the archive!");
                }
            }

            if !freshness_result.unverifiable_files.is_empty() {
                println!("❓ Files that could not be verified for freshness:");
                for file in &freshness_result.unverifiable_files {
                    println!("  📄 {}", file);
                }
            }
        }

        Ok(result)
    }
}
