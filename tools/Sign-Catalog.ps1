#Requires -Version 7
<#
  Sign catalog.json with the offline maintainer key, producing
  catalog.json.sig (RSA-3072 + SHA-256, PKCS#1 v1.5 — verified by
  Test-CatalogSignature in engine/lib.ps1).

    pwsh tools/Sign-Catalog.ps1                 # uses the default key path
    pwsh tools/Sign-Catalog.ps1 -KeyPath ~/usb/catalog-signing.key

  The private key must never enter the repo or CI. Only the keyholder
  can publish config updates; keep an offline backup of the key.
#>
param(
  [string]$KeyPath = (Join-Path $HOME '.config/chamber-signing/catalog-signing.key')
)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
$catalogPath = Join-Path $repoRoot 'catalog.json'
$sigPath = "$catalogPath.sig"

if (-not (Test-Path $KeyPath)) { throw "Signing key not found: $KeyPath" }

$rsa = [System.Security.Cryptography.RSA]::Create()
try {
  $rsa.ImportFromPem((Get-Content $KeyPath -Raw))
  $bytes = [IO.File]::ReadAllBytes($catalogPath)
  $sig = $rsa.SignData($bytes,
    [System.Security.Cryptography.HashAlgorithmName]::SHA256,
    [System.Security.Cryptography.RSASignaturePadding]::Pkcs1)
  [IO.File]::WriteAllBytes($sigPath, $sig)
} finally { $rsa.Dispose() }

# Sanity: verify with the engine's own function + embedded public key
. (Join-Path $repoRoot 'engine/lib.ps1')
if (-not (Test-CatalogSignature ([IO.File]::ReadAllBytes($catalogPath)) ([IO.File]::ReadAllBytes($sigPath)))) {
  Remove-Item $sigPath
  throw 'Signature did not verify against the public key embedded in engine/lib.ps1 — wrong key?'
}
Write-Host "catalog.json.sig written and verified against the embedded public key"
