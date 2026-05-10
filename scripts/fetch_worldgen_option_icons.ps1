$ErrorActionPreference = "Stop"

$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$outDir = Join-Path $root "assets/ui/worldgen_option_icons"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

$options = @(
    @{ Option = "Name:"; Keyword = "name tag line icon" },
    @{ Option = "World Type:"; Keyword = "globe line icon" },
    @{ Option = "Seed Mode:"; Keyword = "seed line icon" },
    @{ Option = "Seed Number (u64):"; Keyword = "number hashtag line icon" },
    @{ Option = "Seed Hex (u64):"; Keyword = "hexagon line icon" },
    @{ Option = "Seed Text:"; Keyword = "text document line icon" },
    @{ Option = "Random Seed:"; Keyword = "dice line icon" },
    @{ Option = "Seed Text (combined with world name):"; Keyword = "link chain line icon" },
    @{ Option = "FBM Octaves"; Keyword = "wave frequency line icon" },
    @{ Option = "Noise Scale 2D"; Keyword = "grid line icon" },
    @{ Option = "Noise Scale 3D"; Keyword = "cube wireframe line icon" },
    @{ Option = "Gravity (m/s²)"; Keyword = "down arrow gravity line icon" },
    @{ Option = "Spawn Surface Offset"; Keyword = "location pin line icon" },
    @{ Option = "Generation Backend"; Keyword = "cpu chip line icon" },
    @{ Option = "Base Terrain Voxel Style"; Keyword = "blocks cube line icon" },
    @{ Option = "Height Divisor"; Keyword = "vertical ruler line icon" },
    @{ Option = "3D Noise Strength"; Keyword = "equalizer line icon" },
    @{ Option = "Water Level (Y)"; Keyword = "water drop line icon" },
    @{ Option = "Ground Level (Y)"; Keyword = "ground horizon line icon" },
    @{ Option = "Dirt Depth"; Keyword = "layers line icon" },
    @{ Option = "Generate Trees"; Keyword = "tree line icon" },
    @{ Option = "Planet Radius"; Keyword = "circle radius line icon" },
    @{ Option = "Planet Center"; Keyword = "target center line icon" },
    @{ Option = "Shell Thickness"; Keyword = "ring line icon" },
    @{ Option = "Planet 3D Noise Strength"; Keyword = "planet texture line icon" },
    @{ Option = "Planet Inner Water"; Keyword = "planet water line icon" },
    @{ Option = "Enable Surface Decoration"; Keyword = "sparkles line icon" },
    @{ Option = "Surface Air Scan Depth"; Keyword = "scan depth line icon" },
    @{ Option = "Beach Max Y"; Keyword = "beach line icon" },
    @{ Option = "Beach Noise Scale"; Keyword = "beach wave line icon" },
    @{ Option = "Beach Noise Threshold"; Keyword = "threshold slider line icon" },
    @{ Option = "Flora Noise Scale"; Keyword = "leaf pattern line icon" },
    @{ Option = "Bush Threshold"; Keyword = "bush line icon" },
    @{ Option = "Fern Threshold"; Keyword = "fern line icon" },
    @{ Option = "Rose Threshold"; Keyword = "rose line icon" },
    @{ Option = "Vine Spawn (/256)"; Keyword = "vine line icon" },
    @{ Option = "Vine Length Factor"; Keyword = "line height line icon" },
    @{ Option = "Tree Spawn (/256)"; Keyword = "forest line icon" },
    @{ Option = "Tree Trunk Height Base"; Keyword = "tree trunk line icon" },
    @{ Option = "Tree Trunk Height Variance"; Keyword = "tree growth line icon" },
    @{ Option = "Tree Leaf Radius Base"; Keyword = "leaf circle line icon" },
    @{ Option = "Tree Leaf Radius Variance"; Keyword = "leaf size line icon" },
    @{ Option = "Tree Local Height Cap"; Keyword = "height limit line icon" }
)

$results = @()

foreach ($item in $options) {
    $q = [Uri]::EscapeDataString($item.Keyword)
    $searchUrl = "https://openclipart.org/search/?query=$q"

    try {
        $searchResp = Invoke-WebRequest -UseBasicParsing $searchUrl
        $matches = [regex]::Matches($searchResp.Content, '/detail/(\d+)/([^"''<>\s]+)')
        if ($matches.Count -eq 0) {
            throw "No search result"
        }

        $detailPath = $matches[0].Value
        $id = $matches[0].Groups[1].Value
        $slug = $matches[0].Groups[2].Value

        $fileBase = ($item.Option -replace '[^a-zA-Z0-9]+', '_').Trim('_').ToLowerInvariant()
        if ([string]::IsNullOrWhiteSpace($fileBase)) {
            $fileBase = "icon_$id"
        }

        $filePath = Join-Path $outDir ($fileBase + ".svg")
        $downloadUrl = "https://openclipart.org/download/$id"
        Invoke-WebRequest -UseBasicParsing $downloadUrl -OutFile $filePath

        $results += [PSCustomObject]@{
            option = $item.Option
            icon_name = $slug
            source = "https://openclipart.org$detailPath"
            download = $downloadUrl
            local_asset = ("assets/ui/worldgen_option_icons/" + [IO.Path]::GetFileName($filePath)).Replace('\\', '/')
            keyword = $item.Keyword
            license = "CC0 / Public Domain / Free commercial use (no attribution required)"
            status = "ok"
            error = ""
        }
    }
    catch {
        $results += [PSCustomObject]@{
            option = $item.Option
            icon_name = ""
            source = ""
            download = ""
            local_asset = ""
            keyword = $item.Keyword
            license = "CC0 / Public Domain / Free commercial use (no attribution required)"
            status = "failed"
            error = $_.Exception.Message
        }
    }
}

$jsonPath = Join-Path $outDir "manifest.json"
$results | ConvertTo-Json -Depth 4 | Set-Content -Path $jsonPath -Encoding UTF8

$md = @()
$md += "# WorldGen Option Icon Catalog"
$md += ""
$md += "Source policy: Openclipart assets are released under CC0/Public Domain and are free for commercial use without attribution."
$md += ""
$md += "| Option | Icon Name | Source | License | Keyword | Asset | Status |"
$md += "|---|---|---|---|---|---|---|"
foreach ($r in $results) {
    $src = if ($r.source) { "[link]($($r.source))" } else { "N/A" }
    $asset = if ($r.local_asset) { $r.local_asset } else { "N/A" }
    $md += "| $($r.option) | $($r.icon_name) | $src | $($r.license) | $($r.keyword) | $asset | $($r.status) |"
}

$mdPath = Join-Path $outDir "manifest.md"
$md -join "`n" | Set-Content -Path $mdPath -Encoding UTF8

$ok = ($results | Where-Object { $_.status -eq "ok" }).Count
$failed = ($results | Where-Object { $_.status -eq "failed" }).Count
Write-Output "Openclipart icon fetch finished. ok=$ok failed=$failed"