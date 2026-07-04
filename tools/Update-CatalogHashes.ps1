#Requires -Version 7
<#
  Recompute the sha256 for every file listed in catalog.json from the
  games/ tree and rewrite the catalog in place. Run after any config or
  manifest change, then re-sign with tools/Sign-Catalog.ps1.

    pwsh tools/Update-CatalogHashes.ps1
#>
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
$catalogPath = Join-Path $repoRoot 'catalog.json'

$catalog = Get-Content $catalogPath -Raw | ConvertFrom-Json
foreach ($g in $catalog.games) {
  $g.files = @(foreach ($f in $g.files) {
    $name = if ($f -is [string]) { $f } else { $f.name }   # tolerate pre-migration entries
    $path = Join-Path $repoRoot "games/$($g.slug)/$name"
    if (-not (Test-Path $path)) { throw "Missing file listed in catalog: games/$($g.slug)/$name" }
    [ordered]@{ name = $name; sha256 = (Get-FileHash $path -Algorithm SHA256).Hash }
  })
}
($catalog | ConvertTo-Json -Depth 8) + "`n" | Set-Content $catalogPath -Encoding UTF8 -NoNewline
Write-Host "catalog.json hashes updated — remember to re-sign (tools/Sign-Catalog.ps1)"
