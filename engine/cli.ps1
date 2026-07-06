#Requires -Version 5.1
<#
  Chamber Game Optimizer — engine
  Single source of truth for the CLI and the GUI (the Tauri app shells out to this).
  All logic lives in lib.ps1; this file only parses arguments and dispatches.

  Actions:
    scan          -> JSON array of games with install + optimization status
    apply -Game x -> backup existing config, apply Chamber config, record state
    revert -Game x-> restore backup, clear state
    check-update  -> compare local app/catalog versions against GitHub
    update        -> download newer game configs from GitHub into local cache
    get-settings  -> scan folders + local config source as JSON
    add-scan-folder / remove-scan-folder -Path x -> manage extra detection roots
    set-local-source / clear-local-source        -> manage the local (offline)
                     config source folder that shadows cache + bundled configs
#>
[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [ValidateSet('scan', 'apply', 'revert', 'check-update', 'update', 'set-path', 'clear-path', 'version',
               'get-settings', 'add-scan-folder', 'remove-scan-folder', 'set-local-source', 'clear-local-source')]
  [string]$Action,
  [string]$Game,
  [string]$Path
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

# Tauri's resource_dir() can hand us a \\?\-prefixed extended-length script
# path; Windows PowerShell's path cmdlets cannot parse that prefix.
. (Join-Path ($PSScriptRoot -replace '^\\\\\?\\', '') 'lib.ps1')
Initialize-DataDir

switch ($Action) {
  'scan'         { Invoke-Scan }
  'apply'        { if (-not $Game) { throw '-Game required' }; Invoke-Apply $Game }
  'revert'       { if (-not $Game) { throw '-Game required' }; Invoke-Revert $Game }
  'check-update' { Invoke-CheckUpdate }
  'update'       { Invoke-Update }
  'set-path'     {
    if (-not $Game) { throw '-Game required' }
    if (-not $Path) { throw '-Path required' }
    Set-Override $Game $Path
    Out-Json ([pscustomobject]@{ ok = $true; slug = $Game; path = (Get-Override $Game) })
  }
  'clear-path'   {
    if (-not $Game) { throw '-Game required' }
    Remove-Override $Game
    Out-Json ([pscustomobject]@{ ok = $true; slug = $Game })
  }
  'version'      { Out-Json ([pscustomobject]@{ appVersion = $AppVersion; repoUrl = "https://github.com/$Repo" }) }
  'get-settings'        { Out-Json (Get-SettingsView) }
  'add-scan-folder'     { if (-not $Path) { throw '-Path required' }; Add-ScanFolder $Path; Out-Json (Get-SettingsView) }
  'remove-scan-folder'  { if (-not $Path) { throw '-Path required' }; Remove-ScanFolder $Path; Out-Json (Get-SettingsView) }
  'set-local-source'    { if (-not $Path) { throw '-Path required' }; Set-LocalSource $Path; Out-Json (Get-SettingsView) }
  'clear-local-source'  { Clear-LocalSource; Out-Json (Get-SettingsView) }
}
