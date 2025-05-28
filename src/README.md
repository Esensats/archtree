# Source Code Documentation

This directory contains the Rust source code for `archtree`, a PowerShell-compatible backup tool that creates compressed archives using 7-Zip. Below is a detailed description of each module and its actual functionality:

## Core Application

### `main.rs`
The application entry point and command-line interface. Features:
- **CLI Arguments**: Uses `clap` for parsing options like input files, output paths, 7-Zip executable paths, verification, and retry modes
- **Application Modes**: Supports normal backup mode and verify-only mode for existing archives
- **Service Orchestration**: Initializes and coordinates all components (archiver, validator, input reader, verifier)
- **Configuration Management**: Builds configuration from CLI args, environment variables (`SEVEN_ZIP_PATH`)
- **Verification Integration**: Optionally runs verification and retry after backup completion

## Archive Management

### `archiver.rs`
7-Zip integration for archive creation and management. Core functionality:
- **`Archiver` Trait**: Async interface for archive operations (`create_archive`, `add_to_archive`, availability checking)
- **`SevenZipArchiver`**: Implements the trait using 7z.exe subprocess calls
- **Temporary File Lists**: Creates Windows-compatible temp files with UTF-8 encoding for large file lists
- **Command Construction**: Builds 7z commands with proper flags (`-spf` for full paths, `-t7z` for format)
- **Error Handling**: Captures and reports both stdout and stderr from 7z processes
- **Custom Executable Paths**: Supports non-standard 7-Zip installation locations

### `verifier.rs`
Comprehensive archive verification and path processing system:
- **`ArchiveVerifier` Trait**: Interface for listing archive contents and entries
- **`SevenZipVerifier`**: Parses 7z `-slt` (technical listing) output to extract file paths, sizes, and directory flags
- **Path Expansion**: Recursively enumerates directory contents (`expand_input_paths`, `enumerate_directory_files`)
- **Verification Service**: Compares expected files against archive contents using both full paths and filename matching
- **Path Normalization**: Handles Windows/Unix path separators and case-insensitive comparison
- **Missing File Consolidation**: Groups missing files by directory and detects when entire directories are missing
- **`VerificationResult`**: Rich result structure with success rates and consolidation support

## Configuration and Input

### `config.rs`
Robust configuration management with builder pattern:
- **Environment Variable Support**: Automatically reads `SEVEN_ZIP_PATH`
- **Builder Pattern**: Fluent API for constructing configuration objects
- **Validation**: Ensures required fields (output path) are provided and non-empty
- **Priority System**: CLI arguments override environment variables
- **Comprehensive Testing**: Unit tests for all configuration scenarios

### `input.rs`
Flexible input reading system supporting multiple sources:
- **`InputReader` Trait**: Async interface for reading file paths from various sources
- **`StdinReader`**: Reads from standard input, filters empty lines, trims whitespace
- **`FileReader`**: Reads from text files with UTF-8 support and line processing
- **`VecReader`**: In-memory reader for testing and programmatic use
- **Error Context**: Provides detailed error messages with file paths and operation context

## Path Processing

### `validator.rs`
File system validation to ensure paths exist before archiving:
- **`PathValidator` Trait**: Interface for validating collections of paths
- **`FileSystemValidator`**: Uses `tokio::fs::metadata` to check path existence and accessibility
- **Batch Validation**: Processes multiple paths efficiently with detailed warning messages
- **Skip Invalid Paths**: Continues processing valid paths when some are inaccessible

### `exclusion.rs`
Sophisticated exclusion pattern system with multiple strategies:
- **`ExclusionMatcher` Trait**: Interface for different pattern matching strategies
- **`WildcardMatcher`**: Implements glob-style patterns (`*`, `?`) with regex conversion and cross-platform path normalization
- **`GitIgnoreMatcher`**: Placeholder for future .gitignore-style pattern support
- **`ExclusionService`**: Manages the complete exclusion workflow:
  - Pattern extraction from input (paths starting with `!`)
  - Pattern application to expanded file lists
  - Statistics tracking (excluded file counts)
- **Pattern Processing**: Handles Windows/Unix path separators and case-insensitive matching

## Services and Orchestration

### `service.rs`
The main orchestrator implementing the complete backup workflow:
- **`BackupService`**: Generic over archiver and validator types for flexibility
- **Workflow Pipeline**: 
  1. Path reading and exclusion pattern extraction
  2. Directory expansion to individual files
  3. Exclusion pattern application
  4. Path validation and filtering
  5. Archive creation
- **Caching**: Uses `OnceLock` to cache processed paths for verification reuse
- **Progress Reporting**: Optional progress messages throughout the process
- **Error Recovery**: Graceful handling of empty inputs and invalid paths

### `verification_service.rs`
Advanced verification and retry system with comprehensive feedback:
- **Event System**: `VerificationEvent` enum for granular progress tracking
- **Callback Interface**: `VerificationCallback` trait for customizable progress reporting
- **`ConsoleCallback`**: Rich CLI output with success rates, retry progress, and final statistics
- **Verification Modes**: `VerifyOnly` vs `VerifyWithRetry` for different use cases
- **`VerificationAndRetryService`**: Coordinates verification, retry, and re-verification cycles
- **Smart Retry**: Validates missing files before attempting to add them to archives

## User Interface

### `display.rs`
Strategy pattern implementation for flexible missing file display:
- **`MissingFileDisplayStrategy` Trait**: Interface for different display approaches
- **`DetailedDisplayStrategy`**: Shows every missing file individually
- **`ConsolidatedDisplayStrategy`**: Uses path consolidation to show directories when all files in a directory are missing
- **`MissingFileDisplayContext`**: Strategy pattern context with easy strategy switching
- **Integration**: Works with `VerificationResult.get_consolidated_missing_files()` for smart directory grouping

## Architecture Overview

The application uses a sophisticated layered architecture:

### Design Patterns
- **Strategy Pattern**: Used for display strategies, exclusion matching, and input reading
- **Builder Pattern**: Configuration construction with validation
- **Trait-based Abstraction**: All major components defined by traits for testability and extensibility
- **Service Layer**: High-level services orchestrate low-level components

### Key Technical Features
- **Full Async/Await**: Non-blocking I/O throughout the application
- **Error Propagation**: `anyhow::Result` used consistently with context
- **Cross-Platform**: Handles Windows/Unix path differences, locale settings, and line endings
- **Memory Efficiency**: Streaming file processing, temporary file cleanup, and smart caching
- **Comprehensive Testing**: Unit tests for all major components with mocking support

### Extensibility Points
- **New Archive Formats**: Implement `Archiver` and `ArchiveVerifier` traits
- **New Input Sources**: Implement `InputReader` trait (e.g., database, API)
- **Advanced Pattern Matching**: Implement `ExclusionMatcher` (e.g., regex, .gitignore)
- **Custom Display**: Implement `MissingFileDisplayStrategy` (e.g., JSON, XML)
- **Progress Integration**: Implement `VerificationCallback` for GUI integration
