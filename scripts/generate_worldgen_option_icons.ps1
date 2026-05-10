$ErrorActionPreference = "Stop"

Add-Type -AssemblyName System.Drawing

$iconSize = 256

function Slugify([string]$label) {
    $chars = $label.ToLowerInvariant().ToCharArray()
    $out = New-Object System.Text.StringBuilder
    $prevUnderscore = $false
    foreach ($ch in $chars) {
        if (($ch -ge 'a' -and $ch -le 'z') -or ($ch -ge '0' -and $ch -le '9')) {
            [void]$out.Append($ch)
            $prevUnderscore = $false
        }
        elseif (-not $prevUnderscore) {
            [void]$out.Append('_')
            $prevUnderscore = $true
        }
    }
    $s = $out.ToString().Trim('_')
    if ([string]::IsNullOrWhiteSpace($s)) { return "option" }
    return $s
}

function Abbrev([string]$label) {
    $parts = ($label -replace '[^A-Za-z0-9 ]', ' ').Split(' ', [System.StringSplitOptions]::RemoveEmptyEntries)
    if ($parts.Count -eq 0) { return "WG" }
    if ($parts.Count -eq 1) {
        $w = $parts[0]
        if ($w.Length -ge 2) { return $w.Substring(0,2).ToUpperInvariant() }
        return ($w + "G").Substring(0,2).ToUpperInvariant()
    }
    return ($parts[0].Substring(0,1) + $parts[1].Substring(0,1)).ToUpperInvariant()
}

$optionLabels = @(
    "Name:",
    "World Type:",
    "Seed Mode:",
    "Seed Number (u64):",
    "Seed Hex (u64):",
    "Seed Text:",
    "Random Seed:",
    "Seed Text (combined with world name):",
    "FBM Octaves",
    "Noise Scale 2D",
    "Noise Scale 3D",
    "Gravity (m/s²)",
    "Spawn Surface Offset",
    "Generation Backend",
    "Base Terrain Voxel Style",
    "Height Divisor",
    "3D Noise Strength",
    "Water Level (Y)",
    "Ground Level (Y)",
    "Dirt Depth",
    "Generate Trees",
    "Planet Radius",
    "Planet Center",
    "Shell Thickness",
    "Planet 3D Noise Strength",
    "Planet Inner Water",
    "Enable Surface Decoration",
    "Surface Air Scan Depth",
    "Beach Max Y",
    "Beach Noise Scale",
    "Beach Noise Threshold",
    "Flora Noise Scale",
    "Bush Threshold",
    "Fern Threshold",
    "Rose Threshold",
    "Vine Spawn (/256)",
    "Vine Length Factor",
    "Tree Spawn (/256)",
    "Tree Trunk Height Base",
    "Tree Trunk Height Variance",
    "Tree Leaf Radius Base",
    "Tree Leaf Radius Variance",
    "Tree Local Height Cap"
)

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$outDir = Join-Path $repoRoot "assets/ui/worldgen_option_icons"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

$manifest = @()

foreach ($label in $optionLabels) {
    $slug = Slugify $label
    $abbr = Abbrev $label
    $svgFile = Join-Path $outDir ("$slug.svg")
    $pngFile = Join-Path $outDir ("$slug.png")

        $svg = @"
<svg xmlns="http://www.w3.org/2000/svg" width="$iconSize" height="$iconSize" viewBox="0 0 256 256" fill="none">
    <rect x="10" y="10" width="236" height="236" rx="42" fill="#111827"/>
    <rect x="28" y="28" width="200" height="200" rx="28" stroke="#38BDF8" stroke-width="12"/>
    <path d="M74 176 L128 86 L182 176" stroke="#93C5FD" stroke-width="12" stroke-linecap="round" stroke-linejoin="round"/>
    <circle cx="128" cy="128" r="16" fill="#22D3EE"/>
    <text x="128" y="222" text-anchor="middle" font-family="Segoe UI,Arial,sans-serif" font-size="44" fill="#E2E8F0">$abbr</text>
</svg>
"@
    Set-Content -Path $svgFile -Value $svg -Encoding UTF8

    $bmp = New-Object System.Drawing.Bitmap $iconSize, $iconSize
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.Clear([System.Drawing.Color]::FromArgb(17, 24, 39))

    $penOuter = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(56, 189, 248)), 2
    $penTri = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(147, 197, 253)), 2
    $brushDot = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(34, 211, 238))
    $brushText = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(226, 232, 240))

    $g.DrawRectangle($penOuter, 28, 28, 200, 200)
    $g.DrawLine($penTri, 74, 176, 128, 86)
    $g.DrawLine($penTri, 128, 86, 182, 176)
    $g.FillEllipse($brushDot, 112, 112, 32, 32)

    $font = New-Object System.Drawing.Font("Segoe UI", 44, [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel)
    $fmt = New-Object System.Drawing.StringFormat
    $fmt.Alignment = [System.Drawing.StringAlignment]::Center
    $g.DrawString($abbr, $font, $brushText, 128, 196, $fmt)

    $bmp.Save($pngFile, [System.Drawing.Imaging.ImageFormat]::Png)

    $font.Dispose(); $fmt.Dispose(); $penOuter.Dispose(); $penTri.Dispose(); $brushDot.Dispose(); $brushText.Dispose(); $g.Dispose(); $bmp.Dispose()

    $manifest += [PSCustomObject]@{
        option = $label
        slug = $slug
        svg = ("assets/ui/worldgen_option_icons/$slug.svg")
        png = ("assets/ui/worldgen_option_icons/$slug.png")
        replaceable = $true
        license = "Project-generated original icon, free commercial use"
        keyword = $label
    }
}

$manifest | ConvertTo-Json -Depth 4 | Set-Content -Path (Join-Path $outDir "manifest.generated.json") -Encoding UTF8

$md = @()
$md += "# WorldGen Option Icons (Generated)"
$md += ""
$md += "Each option has a matching SVG and PNG. You can freely replace any file while keeping the same filename."
$md += ""
$md += "| Option | SVG | PNG | Replaceable |"
$md += "|---|---|---|---|"
foreach ($r in $manifest) {
    $md += "| $($r.option) | $($r.svg) | $($r.png) | yes |"
}
$md -join "`n" | Set-Content -Path (Join-Path $outDir "manifest.generated.md") -Encoding UTF8

"Generated $($manifest.Count) option icon pairs (SVG+PNG) in assets/ui/worldgen_option_icons"