<#
  Chamber Game Optimizer — engine library
  All functions and shared state live here; cli.ps1 dot-sources this file
  and dispatches. Tests dot-source it directly (with $env:CHAMBER_DATA_DIR
  pointed at a scratch dir) without triggering side effects.
#>

$Repo       = 'ChamberTechFPS/GameOptimizer'   # <- update if repo name differs
$RawBase    = "https://raw.githubusercontent.com/$Repo/main"
$AppVersion = '0.1.0'

$RepoRoot  = Split-Path $PSScriptRoot -Parent
$DataDir   = if ($env:CHAMBER_DATA_DIR) { $env:CHAMBER_DATA_DIR }
             elseif ($env:LOCALAPPDATA) { Join-Path $env:LOCALAPPDATA 'ChamberTech\GameOptimizer' }
             else { Join-Path ([IO.Path]::GetTempPath()) 'ChamberTech\GameOptimizer' }
$CacheDir  = Join-Path $DataDir 'games'
$BackupDir = Join-Path $DataDir 'backups'
$StateFile = Join-Path $DataDir 'state.json'
$OverrideFile = Join-Path $DataDir 'overrides.json'

function Initialize-DataDir {
  New-Item -ItemType Directory -Force -Path $DataDir, $BackupDir | Out-Null
}

# ---------- helpers ----------

function Out-Json($obj) { ConvertTo-Json -InputObject $obj -Depth 8 }

function ConvertTo-Hashtable($obj) {
  $ht = @{}
  if ($null -ne $obj) { $obj.PSObject.Properties | ForEach-Object { $ht[$_.Name] = $_.Value } }
  return $ht
}

function Get-GamesDir {
  # Updated configs (pulled from GitHub) take priority over the bundled copy
  if (Test-Path $CacheDir) { return $CacheDir }
  return (Join-Path $RepoRoot 'games')
}

function Get-LocalCatalog {
  $updated = Join-Path $DataDir 'catalog.json'
  $path = if (Test-Path $updated) { $updated } else { Join-Path $RepoRoot 'catalog.json' }
  return (Get-Content $path -Raw | ConvertFrom-Json)
}

function Get-Manifests {
  Get-ChildItem (Get-GamesDir) -Directory | ForEach-Object {
    $m = Join-Path $_.FullName 'manifest.json'
    if (Test-Path $m) { Get-Content $m -Raw | ConvertFrom-Json }
  }
}

function Get-State {
  if (Test-Path $StateFile) { return ConvertTo-Hashtable (Get-Content $StateFile -Raw | ConvertFrom-Json) }
  return @{}
}

function Save-State($state) {
  Out-Json $state | Set-Content $StateFile -Encoding UTF8
}

function Get-Overrides {
  if (Test-Path $OverrideFile) { return ConvertTo-Hashtable (Get-Content $OverrideFile -Raw | ConvertFrom-Json) }
  return @{}
}

function Save-Overrides($overrides) {
  Out-Json $overrides | Set-Content $OverrideFile -Encoding UTF8
}

function Get-Override($slug) { (Get-Overrides)[$slug] }

function Set-Override($slug, $path) {
  if (-not (Test-Path $path)) { throw "Folder not found: $path" }
  $overrides = Get-Overrides
  $overrides[$slug] = (Resolve-Path $path).Path
  Save-Overrides $overrides
}

function Remove-Override($slug) {
  $overrides = Get-Overrides
  if ($overrides.ContainsKey($slug)) { $overrides.Remove($slug); Save-Overrides $overrides }
}

function Get-Sha256($path) { (Get-FileHash -Path $path -Algorithm SHA256).Hash }

# ---------- validation ----------
# Security boundary for content arriving over the update channel (catalog,
# manifests, templates). CI validates the same rules, but runtime must not
# trust the repo: enforce before any Join-Path / file write.

function Test-SafeSlug($slug) {
  return [bool]($slug -cmatch '^[a-z0-9-]+$')
}

function Test-SafeFileName($name) {
  # Leaf filenames only: no separators, no leading dot, no traversal,
  # no Win32 trailing dot/space stripping, no reserved device names.
  if (-not $name) { return $false }
  if ($name -notmatch '^[A-Za-z0-9][A-Za-z0-9 ._-]*$') { return $false }
  if ($name -match '[. ]$') { return $false }
  if ($name -match '^(?i)(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(\..*)?$') { return $false }
  return $true
}

function Test-SafeDest($dest) {
  # Manifest dest/detect paths must be rooted at an allowlisted token and
  # contain only clean relative segments — absolute paths, drive letters,
  # and .. traversal are all impossible by construction.
  if (-not $dest) { return $false }
  if ($dest -cnotmatch '^\{(localappdata|appdata|documents|steamuserdata|custom)\}/') { return $false }
  $rest = $dest -replace '^\{[a-z]+\}/', ''
  foreach ($seg in ($rest -split '[/\\]')) {
    if ($seg -notmatch '^[A-Za-z0-9][A-Za-z0-9 ._()-]*$') { return $false }
    if ($seg -match '[. ]$') { return $false }
  }
  return $true
}

function Resolve-DestPath($dest, $custom = $null) {
  # Validate + expand a manifest dest, then re-check that the normalized
  # result is still inside the root the leading token expands to (defends
  # against normalization tricks and hostile {custom} contents).
  if (-not (Test-SafeDest $dest)) { throw "Unsafe destination path in manifest: $dest" }
  $token = [regex]::Match($dest, '^\{([a-z]+)\}').Groups[1].Value
  $root = switch ($token) {
    'localappdata'  { $env:LOCALAPPDATA }
    'appdata'       { $env:APPDATA }
    'documents'     { [Environment]::GetFolderPath('MyDocuments') }
    'steamuserdata' { Get-SteamUserData }
    'custom'        { if (-not $custom) { throw 'Set the game folder first' }; $custom }
  }
  $full = [IO.Path]::GetFullPath((Expand-PathTokens $dest $custom))
  $fullRoot = [IO.Path]::GetFullPath($root).TrimEnd('\', '/')
  if (-not ($full.StartsWith("$fullRoot\", 'OrdinalIgnoreCase') -or
            $full.StartsWith("$fullRoot/", 'OrdinalIgnoreCase'))) {
    throw "Destination escapes its root: $dest"
  }
  return $full
}

function Test-VersionNewer($remote, $local) {
  # True when $remote is a strictly newer version than $local.
  # Missing local = never seen = newer; unparseable input = not newer (fail safe).
  if (-not $remote) { return $false }
  if (-not $local)  { return $true }
  try { return ([version]$remote -gt [version]$local) } catch { return $false }
}

# ---------- detection ----------

function Get-SteamPath {
  foreach ($key in 'HKCU:\Software\Valve\Steam', 'HKLM:\SOFTWARE\WOW6432Node\Valve\Steam') {
    $p = Get-ItemProperty $key -ErrorAction SilentlyContinue
    if ($p) {
      if ($p.SteamPath)   { return $p.SteamPath }
      if ($p.InstallPath) { return $p.InstallPath }
    }
  }
  return $null
}

function Get-SteamLibraries {
  $steam = Get-SteamPath
  if (-not $steam) { return @() }
  $libs = @($steam)
  $vdf = Join-Path $steam 'steamapps\libraryfolders.vdf'
  if (Test-Path $vdf) {
    foreach ($line in Get-Content $vdf) {
      if ($line -match '"path"\s+"([^"]+)"') { $libs += $Matches[1] -replace '\\\\', '\' }
    }
  }
  return ($libs | Select-Object -Unique)
}

function Test-SteamGame($appId) {
  foreach ($lib in Get-SteamLibraries) {
    if (Test-Path (Join-Path $lib "steamapps\appmanifest_$appId.acf")) { return $true }
  }
  return $false
}

function Test-EpicGame($appName) {
  $dir = Join-Path $env:ProgramData 'Epic\EpicGamesLauncher\Data\Manifests'
  if (-not (Test-Path $dir)) { return $false }
  foreach ($f in Get-ChildItem $dir -Filter *.item -ErrorAction SilentlyContinue) {
    try {
      $j = Get-Content $f.FullName -Raw | ConvertFrom-Json
      if ($j.DisplayName -eq $appName -or $j.AppName -eq $appName) { return $true }
    } catch { }
  }
  return $false
}

function Test-GameInstalled($manifest, $override = $null) {
  # Auto-detection always runs first; a user-picked folder is only a fallback.
  $d = $manifest.detect
  if ($d.steamAppId)  { if (Test-SteamGame $d.steamAppId)   { return $true } }
  if ($d.epicAppName) { if (Test-EpicGame $d.epicAppName)   { return $true } }
  if ($d.paths) {
    foreach ($p in $d.paths) {
      if ($p -like '*{custom}*' -and -not $override) { continue }
      if (Test-Path (Expand-PathTokens $p $override)) { return $true }
    }
  }
  if ($override -and (Test-Path $override)) { return $true }
  return $false
}

# ---------- paths + variables ----------

function Get-SteamUserData {
  # Returns the userdata folder for the most-recently-used Steam account,
  # falling back to the first real profile. Fixes wrong-account writes on
  # machines with multiple Steam logins.
  $steam = Get-SteamPath
  if (-not $steam) { throw 'Steam not found' }
  $userdata = Join-Path $steam 'userdata'
  $profiles = @(Get-ChildItem $userdata -Directory -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -ne '0' })
  if (-not $profiles) { throw 'No Steam userdata profile found' }

  # loginusers.vdf keys accounts by full SteamID64; userdata folders use the
  # 32-bit account id (SteamID64 - 76561197960265728). Match the MostRecent one.
  $vdf = Join-Path $steam 'config\loginusers.vdf'
  if (Test-Path $vdf) {
    $currentId = $null; $mostRecentId = $null
    foreach ($line in Get-Content $vdf) {
      if ($line -match '"(7656119[0-9]+)"') { $currentId = [uint64]$Matches[1] }
      elseif ($line -match '"MostRecent"\s+"1"' -and $currentId) { $mostRecentId = $currentId }
    }
    if ($mostRecentId) {
      $accountId = ($mostRecentId - 76561197960265728).ToString()
      $match = $profiles | Where-Object { $_.Name -eq $accountId } | Select-Object -First 1
      if ($match) { return $match.FullName }
    }
  }
  return ($profiles | Select-Object -First 1).FullName
}

function Expand-PathTokens($path, $custom = $null) {
  $path = $path -replace '/', '\'
  $path = $path.Replace('{localappdata}', $env:LOCALAPPDATA)
  $path = $path.Replace('{appdata}', $env:APPDATA)
  $path = $path.Replace('{documents}', [Environment]::GetFolderPath('MyDocuments'))
  if ($path -like '*{custom}*') {
    if (-not $custom) { throw 'Set the game folder first' }
    $path = $path.Replace('{custom}', $custom)
  }
  if ($path -like '*{steamuserdata}*') {
    $path = $path.Replace('{steamuserdata}', (Get-SteamUserData))
  }
  return $path
}

function Get-SystemVariables {
  $gpu = Get-CimInstance Win32_VideoController -ErrorAction SilentlyContinue |
    Where-Object { $_.CurrentRefreshRate -gt 1 } |
    Sort-Object CurrentRefreshRate -Descending | Select-Object -First 1
  $refresh = 60; $w = 1920; $h = 1080
  if ($gpu) {
    $refresh = [int]$gpu.CurrentRefreshRate
    if ($gpu.CurrentHorizontalResolution) { $w = [int]$gpu.CurrentHorizontalResolution }
    if ($gpu.CurrentVerticalResolution)   { $h = [int]$gpu.CurrentVerticalResolution }
  }
  return @{ refreshRate = $refresh; resolutionWidth = $w; resolutionHeight = $h }
}

# ---------- core actions ----------

function Get-Manifest($slug) {
  $m = Get-Manifests | Where-Object { $_.slug -eq $slug }
  if (-not $m) { throw "Unknown game: $slug" }
  return $m
}

function Get-GameStatus($manifest, $state) {
  if ($manifest.guideOnly) { return 'guide' }
  $entry = $state[$manifest.slug]
  if (-not $entry) { return 'not-applied' }
  foreach ($t in $entry.targets) {
    if (-not (Test-Path $t.dest)) { return 'drifted' }
    if ((Get-Sha256 $t.dest) -ne $t.hash) { return 'drifted' }
  }
  return 'applied'
}

function Test-UsesCustom($manifest) {
  foreach ($t in $manifest.targets) { if ($t.dest -like '*{custom}*') { return $true } }
  foreach ($p in $manifest.detect.paths) { if ($p -like '*{custom}*') { return $true } }
  return $false
}

function Invoke-Scan {
  $state = Get-State
  $overrides = Get-Overrides
  $results = @()
  foreach ($m in Get-Manifests) {
    $override = $overrides[$m.slug]
    $installed = Test-GameInstalled $m $override
    $entry = $state[$m.slug]
    $results += [pscustomobject]@{
      slug      = $m.slug
      name      = $m.name
      platform  = $m.platform
      guideOnly = [bool]$m.guideOnly
      installed = $installed
      status    = if ($installed) { Get-GameStatus $m $state } else { 'not-installed' }
      appliedAt = if ($entry) { $entry.appliedAt } else { $null }
      customPath = if ($override) { $override } else { $null }
      customLabel = if ($m.detect.customLabel) { $m.detect.customLabel } else { $null }
      needsPath = [bool]((Test-UsesCustom $m) -and -not $override)
    }
  }
  Out-Json $results
}

function Invoke-Apply($slug) {
  $m = Get-Manifest $slug
  if ($m.guideOnly) { throw "$($m.name) settings are server-side; see its guide instead" }
  $vars = Get-SystemVariables
  $override = Get-Override $slug
  $gameDir = Join-Path (Get-GamesDir) $slug
  $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
  $gameBackupDir = Join-Path $BackupDir "$slug\$stamp"
  $targets = @()

  foreach ($t in $m.targets) {
    if (-not (Test-SafeFileName $t.template)) { throw "Unsafe template name in manifest: $($t.template)" }
    $dest = Resolve-DestPath $t.dest $override
    $content = Get-Content (Join-Path $gameDir $t.template) -Raw
    foreach ($k in $vars.Keys) { $content = $content.Replace("{{$k}}", [string]$vars[$k]) }

    $backup = $null
    if (Test-Path $dest) {
      New-Item -ItemType Directory -Force -Path $gameBackupDir | Out-Null
      $backup = Join-Path $gameBackupDir (Split-Path $dest -Leaf)
      Copy-Item $dest $backup -Force
      Set-ItemProperty $dest -Name IsReadOnly -Value $false -ErrorAction SilentlyContinue
    }
    New-Item -ItemType Directory -Force -Path (Split-Path $dest -Parent) | Out-Null
    Set-Content -Path $dest -Value $content -Encoding UTF8 -NoNewline
    if ($t.readOnlyAfterApply) { Set-ItemProperty $dest -Name IsReadOnly -Value $true }

    $targets += [pscustomobject]@{ dest = $dest; hash = (Get-Sha256 $dest); backup = $backup }
  }

  $state = Get-State
  $state[$slug] = [pscustomobject]@{ appliedAt = (Get-Date -Format 'o'); targets = $targets }
  Save-State $state
  Out-Json ([pscustomobject]@{ ok = $true; slug = $slug; targets = $targets.Count })
}

function Invoke-Revert($slug) {
  $state = Get-State
  $entry = $state[$slug]
  if (-not $entry) { throw "Nothing to revert for $slug" }
  foreach ($t in $entry.targets) {
    if (Test-Path $t.dest) {
      Set-ItemProperty $t.dest -Name IsReadOnly -Value $false -ErrorAction SilentlyContinue
    }
    if ($t.backup -and (Test-Path $t.backup)) {
      Copy-Item $t.backup $t.dest -Force
    } elseif (Test-Path $t.dest) {
      Remove-Item $t.dest -Force   # no pre-existing file: Chamber created it
    }
  }
  $state.Remove($slug)
  Save-State $state
  Out-Json ([pscustomobject]@{ ok = $true; slug = $slug })
}

# ---------- updates ----------

function Invoke-CheckUpdate {
  try {
    $headers = @{ 'User-Agent' = 'ChamberGameOptimizer' }
    $release = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest" -Headers $headers
    $remote  = Invoke-RestMethod "$RawBase/catalog.json" -Headers $headers
    $local   = Get-LocalCatalog

    $appLatest = ($release.tag_name -replace '^v', '')
    $localVersions = @{}
    foreach ($g in $local.games) { $localVersions[$g.slug] = $g.version }
    $configUpdates = @()
    foreach ($g in $remote.games) {
      if (Test-VersionNewer $g.version $localVersions[$g.slug]) { $configUpdates += $g.slug }
    }
    Out-Json ([pscustomobject]@{
      appCurrent         = $AppVersion
      appLatest          = $appLatest
      appUpdateAvailable = (Test-VersionNewer $appLatest $AppVersion)
      releaseUrl         = $release.html_url
      releaseNotes       = $release.name
      catalogCurrent     = $local.catalogVersion
      catalogLatest      = $remote.catalogVersion
      configUpdates      = $configUpdates
    })
  } catch {
    Out-Json ([pscustomobject]@{ error = 'offline-or-unreachable'; detail = $_.Exception.Message })
  }
}

function Invoke-Update {
  $headers = @{ 'User-Agent' = 'ChamberGameOptimizer' }
  $remote = Invoke-RestMethod "$RawBase/catalog.json" -Headers $headers
  $local  = Get-LocalCatalog
  $localVersions = @{}
  foreach ($g in $local.games) { $localVersions[$g.slug] = $g.version }

  $updated = @()
  foreach ($g in $remote.games) {
    if (-not (Test-SafeSlug $g.slug)) { throw "Unsafe game slug in catalog: $($g.slug)" }
    $lv = $localVersions[$g.slug]
    if (-not (Test-VersionNewer $g.version $lv) -and (Test-Path (Join-Path $CacheDir $g.slug))) { continue }
    $dir = Join-Path $CacheDir $g.slug
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    foreach ($f in $g.files) {
      if (-not (Test-SafeFileName $f)) { throw "Unsafe file name in catalog: $f" }
      Invoke-WebRequest "$RawBase/games/$($g.slug)/$f" -Headers $headers -OutFile (Join-Path $dir $f)
    }
    $updated += $g.slug
  }
  # Seed cache with any bundled games the remote catalog doesn't cover yet
  foreach ($g in $local.games) {
    $dir = Join-Path $CacheDir $g.slug
    if (-not (Test-Path $dir)) {
      Copy-Item (Join-Path $RepoRoot "games\$($g.slug)") $dir -Recurse -Force -ErrorAction SilentlyContinue
    }
  }
  Out-Json $remote | Set-Content (Join-Path $DataDir 'catalog.json') -Encoding UTF8
  Out-Json ([pscustomobject]@{ ok = $true; updated = $updated; catalogVersion = $remote.catalogVersion })
}
