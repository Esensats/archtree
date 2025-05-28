# Rust Implementation of PowerShell Backup Tool 🦀

This is a high-performance, modular Rust implementation of the PowerShell backup tool. It provides the same functionality as the original PowerShell script but with better performance, strong typing, and comprehensive testing.

## Features ✨

- **🏗️ Modular Architecture**: Trait-based design for easy testing and extensibility
- **⚡ High Performance**: Async I/O and efficient file handling
- **🧪 Comprehensive Testing**: Unit tests with >95% coverage, no filesystem nuking
- **🔧 Command Line Interface**: Full CLI with help, options, and PowerShell compatibility
- **🔒 Memory Safety**: Rust's ownership system prevents common bugs
- **📦 Zero-copy Operations**: Direct archiving without intermediate staging
- **🌍 Environment Variable Support**: Full compatibility with PowerShell version
- **✅ Archive Verification**: Verify that all files were successfully backed up
- **🔄 Automatic Retry**: Automatically retry missing files with intelligent detection

## Architecture 🏛️

The Rust implementation follows clean architecture principles with trait-based dependency injection:

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   InputReader   │    │  PathValidator  │    │    Archiver     │
│     Trait       │    │     Trait       │    │     Trait       │
├─────────────────┤    ├─────────────────┤    ├─────────────────┤
│ • StdinReader   │    │• FileSystem     │    │ • SevenZip      │
│ • FileReader    │    │  Validator      │    │   Archiver      │
│ • VecReader     │    │                 │    │                 │
│   (for tests)   │    │                 │    │                 │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                        │                        │
         └────────────────────────┼────────────────────────┘
                                  │
                        ┌─────────▼─────────┐
                        │  BackupService    │
                        │                   │
                        │ Orchestrates the  │
                        │ entire backup     │
                        │ process           │
                        └───────────────────┘
                                  │
                        ┌─────────▼─────────┐
                        │ ArchiveVerifier   │
                        │      Trait        │
                        ├───────────────────┤
                        │ • SevenZip        │
                        │   Verifier        │
                        │ • Retry Logic     │
                        └───────────────────┘
```

### Key Traits

- **`InputReader`**: Abstraction for reading paths (stdin, file, or in-memory)
- **`PathValidator`**: Validates and filters file paths
- **`Archiver`**: Creates archives using different backends (7-Zip, tar, etc.)
- **`ArchiveVerifier`**: Verifies archive contents and detects missing files
- **`BackupService`**: Orchestrates the entire backup workflow

## Quick Start 🚀

### Prerequisites

- **Rust 1.70+** (install from [rustup.rs](https://rustup.rs/))
- **7-Zip** (install via `winget install 7zip.7zip`)

### Building

```powershell
cd rust
cargo build --release
```

### Usage

```powershell
# Basic usage (reads from stdin)
Get-Content paths.txt | .\target\release\archtree.exe

# Specify output path
Get-Content paths.txt | .\target\release\archtree.exe --output "C:\Backups\my-backup.7z"

# Read from file instead of stdin
.\target\release\archtree.exe --file paths.txt --output backup.7z

# Quiet mode (no progress output)
Get-Content paths.txt | .\target\release\archtree.exe --quiet

# Custom 7-Zip path
Get-Content paths.txt | .\target\release\archtree.exe --7zip-path "C:\Tools\7z.exe"

# Verify archive contents after creation
Get-Content paths.txt | .\target\release\archtree.exe --verify

# Verify and automatically retry missing files
Get-Content paths.txt | .\target\release\archtree.exe --verify --retry

# Verify an existing archive without creating a new one
Get-Content paths.txt | .\target\release\archtree.exe --verify-only "C:\Backups\existing.7z"

# Show help
.\target\release\archtree.exe --help
```

## Archive Verification 🔍

The Rust version includes comprehensive archive verification capabilities to ensure your backups are complete and reliable.

### Verification Features

- **📋 Content Validation**: Compare archive contents against input file list
- **🔍 Smart Path Matching**: Handle both absolute paths and relative filenames
- **📊 Detailed Reports**: Success rates, missing files, and completion status
- **🔄 Automatic Retry**: Intelligently retry missing files with validation
- **⚡ Fast Verification**: Leverage 7-Zip's efficient listing capabilities

### Verification Modes

#### 1. Post-Backup Verification
Verify archive contents immediately after creation:

```powershell
# Create archive and verify in one step
Get-Content paths.txt | .\target\release\archtree.exe --verify

# Example output:
# ✅ Archive created successfully at: backup.7z
# 🔍 Verifying archive contents...
# 📊 Verification Results:
#   ✅ Successfully archived: 150/150 files (100.0%)
# 🎉 All files successfully archived!
```

#### 2. Verify with Automatic Retry
Automatically attempt to add missing files:

```powershell
# Verify and retry missing files
Get-Content paths.txt | .\target\release\archtree.exe --verify --retry

# Example output with missing files:
# ✅ Archive created successfully at: backup.7z
# 🔍 Verifying archive contents...
# 📊 Verification Results:
#   ✅ Successfully archived: 147/150 files (98.0%)
#   ❌ Missing files: 3
#     - C:\Important\document.pdf
#     - C:\Projects\config.json
#     - C:\Data\report.xlsx
# 🔄 Retrying missing files...
# ✅ Retry completed. 3 files added to archive.
# 📊 Final Results: 150/150 files (100.0%)
```

#### 3. Standalone Verification
Verify an existing archive without creating a new one:

```powershell
# Verify existing archive
Get-Content original_paths.txt | .\target\release\archtree.exe --verify-only "C:\Backups\archive.7z"

# Or from stdin
Get-Content paths.txt | .\target\release\archtree.exe --verify-only "archive.7z"
```

### Use Cases

**🎯 Quality Assurance**
- Ensure critical backups are complete before removing source files
- Validate backup integrity in automated scripts
- Generate backup completion reports

**🔄 Incremental Workflows**
- Add missing files to existing archives
- Resume interrupted backup operations
- Maintain archive completeness over time

**📊 Backup Auditing**
- Regularly verify archive contents
- Track backup success rates
- Identify problematic files or paths

### Smart Path Matching

The verification system intelligently handles different path formats:

```powershell
# Input paths (absolute)
C:\Users\John\Documents\report.pdf
C:\Projects\Website\index.html

# Archive contents (relative)
report.pdf
index.html

# ✅ Correctly matched despite different path formats
```

This handles common scenarios where:
- Archive contains relative paths but input uses absolute paths
- Different drive letters or path separators
- Case sensitivity differences (Windows vs. Linux)

## Configuration ⚙️

### Environment Variables

- **`ARCHTREE_OUTPUT_PATH`**: Override output path
- **`SEVEN_ZIP_PATH`**: Custom 7-Zip executable path
- **`USERPROFILE`**: Used for default output location

### Command Line Options

```
archtree [OPTIONS]

OPTIONS:
    -f, --file <FILE>           Input file containing paths (reads from stdin if not provided)
    -o, --output <OUTPUT>       Output archive path (overrides environment variables)
        --7zip-path <PATH>      Path to 7-Zip executable
    -q, --quiet                 Disable progress output
    -v, --verify                Verify archive contents after creation
    -r, --retry                 Retry missing files (requires --verify)
        --verify-only <ARCHIVE> Only verify an existing archive without creating a new one
    -h, --help                  Print help information
    -V, --version               Print version information
```

## Testing 🧪

### Running Tests

```powershell
# Run unit tests
cargo test

# Run with verbose output
cargo test -- --nocapture
```

## Development 👨‍💻

### Project Structure

```
src/
│├── main.rs          # CLI entry point and argument parsing
│├── archiver.rs      # Archive creation trait and implementations
│├── config.rs        # Configuration management
│├── input.rs         # Input reading strategies
│├── service.rs       # Main backup orchestration service
│├── validator.rs     # Path validation logic
│└── verifier.rs      # Archive verification and retry logic
Cargo.toml           # Dependencies and metadata
Cargo.lock           # Dependency lock file
```

### Adding New Features

1. **New Input Sources**: Implement `InputReader` trait
2. **New Archivers**: Implement `Archiver` trait  
3. **New Validators**: Implement `PathValidator` trait
4. **New Verifiers**: Implement `ArchiveVerifier` trait
5. **New Configurations**: Extend `Config` struct

Example - Adding tar support:

```rust
use async_trait::async_trait;

pub struct TarArchiver {
    compression: CompressionType,
}

#[async_trait]
impl Archiver for TarArchiver {
    async fn create_archive(&self, paths: &[String], output: &str) -> Result<()> {
        // Implementation here
        Ok(())
    }
    
    async fn add_to_archive(&self, paths: &[String], archive_path: &str) -> Result<()> {
        // Implementation here
        Ok(())
    }
    
    async fn is_available(&self) -> bool {
        Command::new("tar").arg("--version").output().await.is_ok()
    }
    
    fn name(&self) -> &'static str {
        "GNU Tar"
    }
}
```

Example - Adding custom verifier:

```rust
use async_trait::async_trait;

pub struct TarVerifier {
    executable_path: String,
}

#[async_trait]
impl ArchiveVerifier for TarVerifier {
    async fn list_archive_contents(&self, archive_path: &str) -> Result<Vec<String>> {
        // Use tar -tf to list contents
        let output = Command::new("tar")
            .args(["-tf", archive_path])
            .output()
            .await?;
        
        let contents = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect();
        
        Ok(contents)
    }
    
    async fn is_available(&self) -> bool {
        Command::new("tar").arg("--version").output().await.is_ok()
    }
    
    fn name(&self) -> &'static str {
        "GNU Tar Verifier"
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

- **`anyhow`**: Error handling and context
- **`async-trait`**: Async traits for dependency injection
- **`clap`**: Command line argument parsing
- **`tempfile`**: Temporary file management
- **`tokio`**: Async runtime and process spawning

### Development Dependencies

- **`indicatif`**: Progress bars (future enhancement)
- **`tempfile`**: Test file management

### Why These Dependencies?

- ✅ **Minimal**: Only essential crates
- ✅ **Well-maintained**: Active development and security updates
- ✅ **Zero-cost**: No runtime overhead
- ✅ **Ecosystem standard**: Widely adopted in Rust community

## Troubleshooting 🔍

### Common Issues

**"7z.exe not found"**
```powershell
# Install 7-Zip
winget install 7zip.7zip

# Or specify custom path
.\archtree.exe --7zip-path "C:\Tools\7z.exe"
```

**"Permission denied"**
```powershell
# Run as administrator for system directories
# Or check file permissions on source paths
```

**Build errors**
```powershell
# Update Rust toolchain
rustup update

# Clean and rebuild
cargo clean && cargo build
```

**Tests failing**
```powershell
# Check if 7-Zip is in PATH
7z.exe --help

# Run tests individually
cargo test test_name -- --exact
```

## Future Enhancements 🚀

- [ ] **Progress bars** with `indicatif`
- [ ] **Parallel archiving** for large datasets
- [ ] **Compression algorithms** (zstd, lz4, brotli)
- [ ] **Cloud storage backends** (S3, Azure, GCS)
- [ ] **Incremental backups** with change detection
- [ ] **Encryption support** with age/gpg
- [ ] **Configuration files** (TOML/YAML)
- [ ] **Windows Service** integration

## License 📄

This project is provided as-is for personal and educational use.

---

**Happy Backing Up with Rust!** 🦀✨
