# Archtree 🌿

A backup tool that creates and verifies 7-Zip archives from file lists.

Takes a list of files and folders, creates compressed archives, and can verify that everything made it in correctly.

> The name comes from building an **arch**ive while preserving your filesystem **tree** structure.

## What it does

- **Create backups** from file lists (text files or stdin)
- **Verify archives** to make sure nothing was missed
- **Add missing files** to existing archives automatically
- **Filter files** using wildcard patterns (`*.tmp`, `cache/*`, etc.)
- **Handle large datasets** efficiently with smart path processing
- **Work with any paths** - absolute, relative, Windows, or Unix style

## Getting started

### What you need

- **7-Zip** (install with `winget install 7zip.7zip`)
- **Rust** to build from source (get it at [rustup.rs](https://rustup.rs/))
- **Git** to clone the repository (install with `winget install Git.Git`)

### Install it

```powershell
git clone https://github.com/Esensats/archtree.git
cd archtree
cargo install --path . --locked
```

### Use it

**Make a backup:**
```powershell
# From a file list
archtree backup -f my_files.txt -o backup.7z

# From stdin (pipe in file paths)
Get-Content file_list.txt | archtree backup -o backup.7z

# Create and verify in one go
archtree backup -f my_files.txt -o backup.7z --verify --retry
```

**Check an existing backup:**
```powershell
# Just verify what's in there
archtree verify -a backup.7z -f original_list.txt

# Verify and add anything that's missing
archtree verify -a backup.7z -f file_list.txt --retry
```

## Commands

### `backup` - Create archives
```
archtree backup [OPTIONS] --output <OUTPUT>

Options:
  -f, --file <FILE>           Read paths from this file (otherwise uses stdin)
  -o, --output <OUTPUT>       Where to save the archive
  --7zip-path <PATH>          Use 7-Zip from this location
  -q, --quiet                 Don't show progress
  -v, --verify                Check the archive after creating it
  -r, --retry                 Add missing files (only with --verify)
```

### `verify` - Check existing archives
```
archtree verify [OPTIONS] --archive <ARCHIVE>

Options:
  -a, --archive <ARCHIVE>     Archive file to check
  -f, --file <FILE>           Expected file list (otherwise uses stdin)
  --7zip-path <PATH>          Use 7-Zip from this location
  -q, --quiet                 Don't show progress
  -r, --retry                 Add any missing files
```

**Environment variables:**
- `SEVEN_ZIP_PATH` - Default 7-Zip location

**Help:** `archtree --help` or `archtree <command> --help`

## Filtering files

You can exclude files by adding exclusion patterns to your file list. Exclusion lines start with `!` and support wildcards:

**Pattern examples:**
- `!*.tmp` - Skip all .tmp files
- `!cache/*` - Skip everything in cache folders
- `!**/node_modules/**` - Skip node_modules anywhere
- `!temp_*` - Skip files starting with "temp_"

**Example file list:**
```
# Files to backup
C:\Projects\source\
C:\Documents\important.pdf
test_files\data.json

# Skip these
!*.tmp
!*.log
!**/cache/**
!node_modules/**
```

**How it works:**
- Exclusions are checked before scanning directories (faster)
- Works with Windows (`\`) and Unix (`/`) paths
- Case-insensitive on Windows

## Configuration

**Environment variables:**
- `SEVEN_ZIP_PATH` - Custom 7-Zip location

**Two ways to use it:**
1. **Create and verify** - Use the `backup` command with `--verify` and `--retry`
2. **Just verify** - Use the `verify` command on existing archives

All commands work with files or stdin, and you can specify a custom 7-Zip path or run in quiet mode.

## Testing

**Run tests:**
```powershell
# All tests
cargo test

# With output
cargo test -- --nocapture

# Specific module
cargo test processing::exclusions
```

**Test setup:**
```powershell
# Create test files
mkdir test_files
echo "test content" > test_files\sample.txt

# Run integration tests
cargo test test_backup_command_integration
```

The tests cover individual components, full workflows, and error scenarios. Most tests use mocks so you don't need 7-Zip installed to run them.

## How it works

The tool is built with a modular design:

```
CLI Commands → Services → Processing → File I/O
    ↓             ↓          ↓           ↓
  backup       BackupSvc   PathProc   Archiver
  verify       VerifySvc   Exclusions   Reader
```

**Main parts:**
- **CLI** - Command parsing and user interface
- **Services** - Backup and verification workflows
- **Processing** - Path handling, filtering, validation
- **I/O** - File reading and archive operations

**Key files:**
- `src/main.rs` - Command line interface
- `src/services/backup.rs` - Main backup logic
- `src/verification/` - Archive verification
- `src/processing/` - Path processing and exclusions
- `src/io/` - File I/O and 7-Zip integration

## Development

The code is organized like this:

```
src/
├── main.rs                    # CLI and commands
├── core/                     # Basic types and configuration
├── io/                       # File reading and 7-Zip integration
├── processing/               # Path handling and exclusions
├── services/                 # Main backup logic
└── verification/             # Archive verification and retry
```

**To extend it:**
- Add new input sources (databases, APIs) by implementing `InputReader`
- Support new archive formats by implementing `Archiver`
- Add validation logic by implementing `PathValidator`

**Development setup:**
```powershell
cargo fmt       # Format code
cargo clippy    # Check for issues
cargo test      # Run tests
```

## Dependencies

Uses these external libraries:
- `clap` - Command line parsing
- `regex` - Wildcard pattern matching
- `walkdir` - Directory traversal
- `tokio` - Async runtime
- `anyhow` - Error handling
- `tempfile` - Test file management

## Troubleshooting

**7-Zip not found:**
```powershell
# Install 7-Zip
winget install 7zip.7zip

# Or specify location
archtree backup -f paths.txt -o backup.7z --7zip-path "C:\Program Files\7-Zip\7z.exe"

# Check it's working
7z --help
```

**Permission errors:**
- Run as Administrator for system files
- Check source file permissions
- Make sure output directory is writable

**File encoding issues:**
```powershell
# Convert to UTF-8
Get-Content input.txt | Out-File -Encoding UTF8 input_utf8.txt
```

**Verification problems:**
```powershell
# Check if paths exist
Get-Content paths.txt | ForEach-Object { Test-Path $_ }

# Debug verification
archtree verify -a backup.7z -f paths.txt --retry
```

**Build errors:**
```powershell
# Update Rust
rustup update stable

# Clean rebuild
cargo clean
cargo build --release
```

**Performance tips:**
- Use exclusion patterns for large directories
- Specify files instead of whole directories when possible
- Split very large file lists (>100K files)

## Future ideas

- Progress bars for long operations
- Checking for file changes since last backup
- Support for zip, tar, and other archive formats and compression methods
- Configuration files instead of just command line options
- Parallel processing for faster path handling
- GUI version for less technical users

---

This is a personal project - no affiliation with 7-Zip. Built for reliable daily backups.