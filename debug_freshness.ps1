# Debug script to test freshness verification
Write-Host "=== Debug Freshness Verification ==="

# First, let's check the file timestamps
Write-Host "`n1. Checking filesystem timestamps:"
Get-ChildItem "testing_files\test_unicode_files\файл_тест.txt" | Select-Object Name, LastWriteTime

# Check archive contents with 7-Zip to see modification times
Write-Host "`n2. Checking archive modification times:"
& "7z.exe" l -slt "testing_files\unicode.7z" | Select-String -Pattern "Path = |Modified = "

Write-Host "`n3. Running freshness verification with verbose output:"
# We'll need to modify the Rust code to add debug output
