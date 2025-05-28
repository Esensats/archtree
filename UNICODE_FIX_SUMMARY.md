# Unicode Character Handling Fix for 7-Zip Verification

## Problem Description

The verification process was failing for archives containing files with non-English characters (especially Cyrillic characters) because:

1. The `7z l -slt` command outputs non-ASCII characters as gibberish when using default encoding
2. This caused path comparison to fail during verification, showing existing files as "missing"
3. Example problematic output:
   ```
   Path = C:\Users\these\Cisco Packet Tracer 8.0.1\saves\12.09.2022 - Òåðìèíàëüíàÿ ñåòü ñ ïðèíòåðîì.pkt
   ```
   Instead of proper Cyrillic characters, showing garbled text.

## Solution Implemented

### 1. UTF-8 Output Forcing
- Added `-sccUTF-8` flag to 7-Zip command to force UTF-8 console output
- This tells 7-Zip to encode output in UTF-8 instead of system default codepage

### 2. Dual-Method Approach
Created a robust fallback system with two methods:

**Primary Method (`list_archive_entries_utf8`)**:
- Uses `7z l -slt -sccUTF-8` command
- Parses output as strict UTF-8
- Fails gracefully if UTF-8 parsing fails

**Fallback Method (`list_archive_entries_legacy`)**:
- Uses original `7z l -slt` command (without UTF-8 flag)
- Uses lossy UTF-8 conversion for backward compatibility
- Handles cases where 7-Zip version doesn't support `-sccUTF-8`

### 3. Shared Parsing Logic
- Extracted common parsing logic into `parse_seven_zip_output` method
- Consistent handling of archive entry parsing across both methods
- Proper handling of Path, Attributes, and Size fields

## Code Changes

### Key Changes in `verifier.rs`:

1. **Main Entry Point**:
   ```rust
   async fn list_archive_entries(&self, archive_path: &str) -> Result<Vec<ArchiveEntry>> {
       self.list_archive_entries_with_encoding(archive_path).await
   }
   ```

2. **Encoding-Aware Method**:
   ```rust
   async fn list_archive_entries_with_encoding(&self, archive_path: &str) -> Result<Vec<ArchiveEntry>> {
       match self.list_archive_entries_utf8(archive_path).await {
           Ok(entries) => Ok(entries),
           Err(_) => self.list_archive_entries_legacy(archive_path).await,
       }
   }
   ```

3. **UTF-8 Method**:
   ```rust
   cmd.args([
       "l",
       "-slt",
       "-sccUTF-8", // Force UTF-8 output
       &archive_path,
   ]);
   ```

## Benefits

1. **Reliable Unicode Support**: Properly handles Cyrillic, Chinese, Japanese, and other non-ASCII characters
2. **Backward Compatibility**: Falls back to legacy method if UTF-8 approach fails
3. **Robust Error Handling**: Graceful degradation when 7-Zip versions don't support UTF-8 flags
4. **Performance**: Only uses fallback when necessary
5. **Maintainability**: Shared parsing logic reduces code duplication

## Testing Recommendations

To verify the fix works with Unicode filenames:

1. Create a test archive with files containing non-ASCII characters
2. Run verification against the archive
3. Confirm that all files are properly detected and verified

## Future Considerations

- Monitor for new 7-Zip versions that might change UTF-8 handling
- Consider adding support for other encoding methods if needed
- Potential enhancement: Auto-detect file encoding for even better compatibility

## Compatibility

This fix is compatible with:
- 7-Zip versions that support `-sccUTF-8` flag (most modern versions)
- Older 7-Zip versions (via fallback method)
- Windows systems with various locale settings
- Archives containing mixed-language filenames
