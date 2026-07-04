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
#>
[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [ValidateSet('scan', 'apply', 'revert', 'check-update', 'update', 'set-path', 'clear-path', 'version')]
  [string]$Action,
  [string]$Game,
  [string]$Path
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

. (Join-Path $PSScriptRoot 'lib.ps1')
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
}
