param(
    [string]$OutDir = "release"
)

if (-not (Test-Path $OutDir)) {
    New-Item -ItemType Directory -Path $OutDir | Out-Null
}

# Finds any .apk under target/ and copies to release folder with original filename
Get-ChildItem -Path "target" -Recurse -Filter "*.apk" | ForEach-Object {
    $dest = Join-Path -Path $OutDir -ChildPath $_.Name
    Copy-Item -Path $_.FullName -Destination $dest -Force
    Write-Output "Copied $($_.FullName) -> $dest"
}