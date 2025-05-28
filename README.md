# archtree 🦀

A modern, efficient backup tool written in Rust that creates and verifies compressed archives using 7-Zip.

Features intelligent path processing, exclusion patterns, comprehensive verification, and automatic retry capabilities for reliable backups.

> Why "archtree"? Because it builds an *arch*ive while preserving the hier*arch*y of your filesystem *tree*! 🌳

## Features ✨

- **🎯 Two-Mode Operation**: Backup creation and standalone archive verification
- **🏗️ Clean Architecture**: Trait-based design with dependency injection for testability
- **⚡ Optimized Performance**: Efficient path processing with early exclusion filtering
- **🚫 Smart Exclusions**: Wildcard patterns (`*.tmp`, `cache/*`) with support for inline patterns  
- **✅ Advanced Verification**: Compare archive contents against expected files with intelligent matching
- **🔄 Automatic Retry**: Add missing files to existing archives with validation
- **📊 Rich Progress Reporting**: Real-time feedback with success rates and consolidated file displays
- **🔒 Memory Safe**: Rust's ownership model prevents common backup tool vulnerabilities
- **📁 Path Flexibility**: Handle both absolute and relative paths with automatic normalization
- **🌐 Cross-Platform**: Windows and Unix path handling with proper separator normalization

## Quick Start 🚀

### Prerequisites

- **Rust 1.70+** (install from [rustup.rs](https://rustup.rs/))
- **7-Zip** (install via `winget install 7zip.7zip`)

### Installation

```powershell
cd rust
cargo build --release
```

### Basic Usage

**Create a backup:**
```powershell
# From stdin
Get-Content paths.txt | .\target\release\archtree.exe backup -o backup.7z

# From file  
.\target\release\archtree.exe backup -f paths.txt -o backup.7z

# With verification and retry
.\target\release\archtree.exe backup -f paths.txt -o backup.7z --verify --retry
```

**Verify existing archive:**
```powershell
# Verify only
.\target\release\archtree.exe verify -a backup.7z -f original_paths.txt

# Verify and add missing files
.\target\release\archtree.exe verify -a backup.7z -f paths.txt --retry
```

## Command Line Interface 🔧

### Commands

#### `backup` - Create Archive
```
archtree backup [OPTIONS] --output <OUTPUT>

Options:
  -f, --file <FILE>           Input file with paths (uses stdin if not provided)
  -o, --output <OUTPUT>       Output archive path (required)
  --7zip-path <PATH>          Custom 7-Zip executable path
  -q, --quiet                 Disable progress output
  -v, --verify                Verify archive after creation
  -r, --retry                 Retry missing files (requires --verify)
```

#### `verify` - Verify Archive  
```
archtree verify [OPTIONS] --archive <ARCHIVE>

Options:
  -a, --archive <ARCHIVE>     Archive file to verify (required)
  -f, --file <FILE>           Input file with expected paths (uses stdin if not provided)
  --7zip-path <PATH>          Custom 7-Zip executable path  
  -q, --quiet                 Disable progress output
  -r, --retry                 Add missing files to archive
```

### Global Options
- **Environment Variables**: `SEVEN_ZIP_PATH`
- **Help**: `archtree --help` or `archtree <command> --help`

## Exclusion Patterns 🚫

Archtree supports inline exclusion patterns within your input file or stdin. Exclusion patterns start with `!` and support wildcards:

### Pattern Syntax
- `!*.tmp` - Exclude all `.tmp` files
- `!cache/*` - Exclude everything in `cache` directories
- `!**/node_modules/**` - Exclude `node_modules` directories recursively
- `!temp_*` - Exclude files starting with `temp_`

### Example Input File
```
# Regular paths to include
C:\Projects\source\
C:\Documents\important.pdf
test_files\data.json

# Exclusion patterns
!*.tmp
!*.log
!**/cache/**
!node_modules/**
```

### How Exclusions Work
1. **Early Filtering**: Exclusions are applied before directory expansion for efficiency
2. **Wildcard Matching**: Uses regex-based matching for flexible patterns  
3. **Cross-Platform**: Handles both Windows (`\`) and Unix (`/`) path separators
4. **Case Insensitive**: Pattern matching works regardless of case on Windows

## Configuration ⚙️

## Configuration ⚙️

### Environment Variables

- **`SEVEN_ZIP_PATH`**: Custom 7-Zip executable path

### Command Line Integration

The CLI supports two main workflows:

1. **Create and Verify**: `backup` command with optional `--verify` and `--retry`
2. **Standalone Verification**: `verify` command for existing archives

All commands support custom 7-Zip paths, quiet mode, and flexible input sources (files or stdin).

## Testing 🧪

### Running Tests

```powershell
# Run all tests
cargo test

# Run tests with detailed output
cargo test -- --nocapture

# Run only unit tests (exclude integration tests)
cargo test --lib

# Run specific test module
cargo test processing::exclusions

# Run with multiple threads for faster execution
cargo test --release
```

### Test Coverage

The project maintains comprehensive test coverage across:

- **Unit Tests**: Individual component testing for each module
- **Integration Tests**: End-to-end workflow testing  
- **Mock Tests**: External dependency simulation (7-Zip not required)
- **Error Handling**: Comprehensive error condition coverage

### Test Environment Setup

```powershell
# Create test files for local testing
mkdir test_files
echo "test content" > test_files\sample.txt

# Run specific integration test
cargo test test_backup_command_integration
```

## Architecture Overview 🏗️

Archtree follows a clean, modular architecture with trait-based dependency injection:

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   CLI Layer     │───▶│  Services Layer  │───▶│ Verification    │
│   (main.rs)     │    │  (BackupService) │    │ (VerifyService) │
└─────────────────┘    └──────────────────┘    └─────────────────┘
         │                        │                       │
         ▼                        ▼                       ▼
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│ Core Traits     │    │ Processing Layer │    │ I/O Layer       │
│ • Archiver      │    │ • PathProcessor  │    │ • InputReader   │
│ • Verifier      │    │ • Exclusions     │    │ • Archiver      │
│ • Validator     │    │ • Validation     │    │ • FileSystem    │
└─────────────────┘    └──────────────────┘    └─────────────────┘
```

### Key Components

- **Core Traits**: Define interfaces for testability and extensibility
- **Service Layer**: Orchestrates the backup and verification workflows  
- **Processing Layer**: Handles path expansion, exclusions, and validation
- **I/O Layer**: Manages file reading, archive creation, and external tool integration
- **Verification Layer**: Advanced comparison and retry logic with rich feedback

## Development Guide 👨‍💻

### Project Structure

```
src/
├── main.rs                    # CLI entry point and subcommand routing
├── core/
│   ├── mod.rs                # Core types and result handling
│   ├── config.rs             # Configuration management with environment variables
│   └── error.rs              # Error types and context management
├── io/
│   ├── mod.rs                # I/O module exports
│   ├── input.rs              # InputReader trait and implementations (stdin, file)
│   └── archiver.rs           # Archiver trait and 7-Zip implementation
├── processing/
│   ├── mod.rs                # Processing module exports
│   ├── path_processor.rs     # Directory expansion and file enumeration
│   ├── exclusions.rs         # Wildcard pattern matching and filtering
│   └── validation.rs         # Path validation and filesystem checks
├── services/
│   ├── mod.rs                # Service module exports
│   └── backup.rs             # Main backup orchestration service
└── verification/
    ├── mod.rs                # Verification module exports
    ├── verifier.rs           # Archive content verification and comparison
    ├── service.rs            # Verification workflow with retry and callbacks
    └── display.rs            # Missing file display strategies
```

### Extension Points

The modular architecture provides several extension points:

1. **New Input Sources**: Implement `InputReader` trait for database queries, APIs, etc.
2. **New Archive Formats**: Implement `Archiver` trait for tar, zip, rar support
3. **New Validators**: Implement `PathValidator` trait for custom validation logic
4. **New Verifiers**: Implement `ArchiveVerifier` trait for different archive tools
5. **New Display Strategies**: Implement display patterns for verification results

### Example Extensions

**Adding PostgreSQL input source:**
```rust
use async_trait::async_trait;

pub struct PostgresInputReader {
    connection: PgConnection,
    query: String,
}

#[async_trait]
impl InputReader for PostgresInputReader {
    async fn read_input_paths(&self) -> Result<Vec<String>> {
        // Execute query and return file paths
        Ok(self.connection.query(&self.query).await?)
    }
}
```

**Adding zip format support:**
```rust
use async_trait::async_trait;

pub struct ZipArchiver {
    compression_level: u8,
}

#[async_trait]
impl Archiver for ZipArchiver {
    async fn create_archive(&self, paths: &[String], output: &str) -> Result<()> {
        // Use zip library or external zip command
        Ok(())
    }
    
    async fn add_to_archive(&self, paths: &[String], archive_path: &str) -> Result<()> {
        // Add files to existing zip archive
        Ok(())
    }
    
    async fn is_available(&self) -> bool {
        // Check if zip tools are available
        true
    }
    
    fn name(&self) -> &'static str {
        "Zip Archiver"
    }
}
```

### Contributing Guidelines

1. **Follow Rust conventions**: Use `cargo fmt` and `cargo clippy`
2. **Write tests**: Maintain >90% test coverage
3. **Document public APIs**: Use rustdoc comments
4. **Handle errors properly**: Use `anyhow` for error propagation
5. **Keep it simple**: Prefer composition over inheritance

## Dependencies 📦

### Production Dependencies

- **`anyhow`** (1.0.98): Error handling and context chaining
- **`async-trait`** (0.1.88): Async traits for dependency injection  
- **`clap`** (4.5.38): Command line argument parsing with derive macros
- **`regex`** (1.11.1): Pattern matching for exclusion wildcards
- **`tempfile`** (3.20.0): Temporary file management for testing
- **`tokio`** (1.45.1): Async runtime with full feature set
- **`walkdir`** (2.5.0): Recursive directory traversal

### Development Dependencies

- **`indicatif`** (0.17.11): Progress bars for future CLI enhancements
- **`tempfile`** (3.20.0): Test file and directory management

### Dependency Rationale

- ✅ **Minimal footprint**: Only essential, well-maintained crates
- ✅ **Zero-cost abstractions**: No runtime performance overhead
- ✅ **Ecosystem standards**: Widely adopted and battle-tested
- ✅ **Active maintenance**: Regular security updates and improvements
- ✅ **Async-first**: Built for modern Rust async patterns

## Troubleshooting 🔍

### Common Issues

**"7z.exe not found" or "7z command failed"**
```powershell
# Install 7-Zip via Windows Package Manager
winget install 7zip.7zip

# Or download from official site and specify custom path
.\target\release\archtree.exe backup -f paths.txt -o backup.7z --7zip-path "C:\Program Files\7-Zip\7z.exe"

# Verify 7-Zip is accessible
7z --help
```

**"Permission denied" errors**
```powershell
# Run PowerShell as Administrator for system directories
# Check file/directory permissions on source paths
# Ensure output directory is writable

# Example: Check permissions
Get-Acl "C:\path\to\file" | Format-List
```

**Input file encoding issues**
```powershell
# Ensure input files use UTF-8 encoding
# PowerShell example to convert:
Get-Content input.txt | Out-File -Encoding UTF8 input_utf8.txt
```

**Archive verification failures**
```powershell
# Check if paths in input file exist
Get-Content paths.txt | ForEach-Object { Test-Path $_ }

# Run verification separately to debug
.\target\release\archtree.exe verify -a backup.7z -f paths.txt --retry
```

**Build compilation errors**
```powershell
# Update Rust toolchain to latest stable
rustup update stable

# Clean build artifacts and rebuild
cargo clean
cargo build --release

# Check for missing system dependencies
rustc --version
cargo --version
```

**Test failures**
```powershell
# Run tests with output to see details
cargo test -- --nocapture

# Run specific test
cargo test test_backup_service -- --exact

# Check if 7-Zip is in PATH for integration tests
where.exe 7z
```

### Performance Issues

**Slow directory traversal**
- Large directories with many files benefit from exclusion patterns
- Use specific file paths instead of broad directory includes when possible

**Memory usage with large file lists**
- The tool processes paths in batches to manage memory
- Consider splitting very large input files (>100K paths)

**Archive creation timeouts**
- 7-Zip compression can be CPU intensive
- Monitor system resources during backup operations

## Potential Future Enhancements 🚀

- [ ] **Progress bars** with `indicatif`
- [ ] **Parallel archiving** for large datasets
- [ ] **Compression methods** (zstd, lz4, brotli)
- [ ] **Configuration files** (TOML/YAML)
- [ ] **GUI frontend** for non-technical users
- [ ] **Library mode** for embedding in other Rust applications

## License 📄

This project is provided as-is for personal and educational use.

> No affiliation with 7-Zip. This is a personal project built for robust, everyday backup needs.

---

**Happy Backing Up with Rust!** 🦀✨
