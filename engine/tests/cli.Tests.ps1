<#
  Pester 5 tests for engine/lib.ps1.
  Cross-platform: pure string/logic functions run anywhere pwsh does;
  Windows-only detection functions (Steam/Epic/registry) are not covered here.
#>

# Discovery-time constant for -Skip: some tests exercise Windows path
# expansion (backslashes) or CIM and can't run on the Linux CI leg.
$IsWindowsHost = $env:OS -eq 'Windows_NT'

BeforeAll {
  $env:CHAMBER_DATA_DIR = Join-Path $TestDrive 'data'
  . (Join-Path (Split-Path $PSScriptRoot -Parent) 'lib.ps1')
  Initialize-DataDir
}

AfterAll {
  Remove-Item Env:CHAMBER_DATA_DIR -ErrorAction SilentlyContinue
}

Describe 'Test-VersionNewer' {
  It 'is true when remote is strictly newer' {
    Test-VersionNewer '1.1.0' '1.0.0' | Should -BeTrue
    Test-VersionNewer '2.0' '1.9.9'   | Should -BeTrue
  }
  It 'is false when versions are equal' {
    Test-VersionNewer '1.0.0' '1.0.0' | Should -BeFalse
  }
  It 'is false when remote is older' {
    Test-VersionNewer '1.0.0' '1.1.0' | Should -BeFalse
  }
  It 'is true when there is no local version' {
    Test-VersionNewer '1.0.0' $null | Should -BeTrue
    Test-VersionNewer '1.0.0' ''    | Should -BeTrue
  }
  It 'is false when remote is missing or unparseable (fail safe)' {
    Test-VersionNewer $null '1.0.0'      | Should -BeFalse
    Test-VersionNewer 'garbage' '1.0.0'  | Should -BeFalse
    Test-VersionNewer '1.0.0' 'garbage'  | Should -BeFalse
  }
}

Describe 'Expand-PathTokens' {
  BeforeEach {
    $env:LOCALAPPDATA = 'X:\LocalAppData'
    $env:APPDATA      = 'X:\Roaming'
  }

  It 'substitutes {localappdata} and normalizes separators' {
    Expand-PathTokens '{localappdata}/Foo/Bar.ini' |
      Should -Be 'X:\LocalAppData\Foo\Bar.ini'
  }
  It 'substitutes {appdata}' {
    Expand-PathTokens '{appdata}/Foo' | Should -Be 'X:\Roaming\Foo'
  }
  It 'substitutes {documents} with a real folder' {
    $result = Expand-PathTokens '{documents}/My Games/x.cfg'
    $result | Should -Not -Match '\{documents\}'
    $result | Should -Match 'My Games'
  }
  It 'substitutes {custom} when an override is provided' {
    Expand-PathTokens '{custom}/cfg/autoexec.cfg' 'D:\Games\CS2' |
      Should -Be 'D:\Games\CS2\cfg\autoexec.cfg'
  }
  It 'throws on {custom} without an override' {
    { Expand-PathTokens '{custom}/cfg/autoexec.cfg' } |
      Should -Throw '*game folder*'
  }
}

Describe 'Test-SafeSlug' {
  It 'accepts <_>' -ForEach 'cs2', 'rocket-league', 'apex-legends-2' {
    Test-SafeSlug $_ | Should -BeTrue
  }
  It 'rejects <_>' -ForEach '../cs2', 'CS2', 'cs2/..', 'a b', '', 'cs2\evil' {
    Test-SafeSlug $_ | Should -BeFalse
  }
}

Describe 'Test-SafeFileName' {
  It 'accepts <_>' -ForEach 'autoexec.cfg', 'GameUserSettings.ini', 'manifest.json', 'My Config v2.txt', 'a-b_c.d' {
    Test-SafeFileName $_ | Should -BeTrue
  }
  It 'rejects <_>' -ForEach `
    '../evil.cfg', '..\evil.cfg', 'sub/file.cfg', 'sub\file.cfg', '.hidden',
    'trailing.', 'trailing ', 'CON', 'con.cfg', 'NUL.txt', 'COM1', '', 'C:file' {
    Test-SafeFileName $_ | Should -BeFalse
  }
}

Describe 'Test-SafeDest' {
  It 'accepts <_>' -ForEach `
    '{localappdata}/FortniteGame/Saved/Config/WindowsClient/GameUserSettings.ini',
    '{steamuserdata}/730/local/cfg/autoexec.cfg',
    '{documents}/My Games/Rocket League/TAGame/Config/TASystemSettings.ini',
    '{custom}/cfg/autoexec.cfg' {
    Test-SafeDest $_ | Should -BeTrue
  }
  It 'rejects <_>' -ForEach `
    'C:/Windows/System32/evil.dll',
    '/etc/passwd',
    '{localappdata}/../../Windows/evil.dll',
    '{appdata}/Microsoft/Windows/Start Menu/Programs/Startup/..\..\evil.bat',
    '{unknown}/foo.cfg',
    '{localappdata}',
    '{localappdata}/.hidden/x.cfg',
    '{documents}/foo./bar.cfg',
    '' {
    Test-SafeDest $_ | Should -BeFalse
  }
  It 'rejects the Startup-folder traversal shape specifically' {
    # the attack from the threat model: manifest dest aimed at the Startup folder via ..
    Test-SafeDest '{localappdata}/../Roaming/Microsoft/Windows/Start Menu/Programs/Startup/evil.bat' |
      Should -BeFalse
  }
}

Describe 'Resolve-DestPath' {
  BeforeEach {
    $env:LOCALAPPDATA = Join-Path $TestDrive 'LocalAppData'
    New-Item -ItemType Directory -Force -Path $env:LOCALAPPDATA | Out-Null
  }

  It 'expands a clean dest under its root' {
    $result = Resolve-DestPath '{localappdata}/Game/config.ini'
    $result | Should -Match 'config\.ini$'
  }
  It 'throws on an unsafe dest before expansion' {
    { Resolve-DestPath '{localappdata}/../evil.cfg' } | Should -Throw '*Unsafe destination*'
  }
  It 'throws on an absolute dest' {
    { Resolve-DestPath 'C:/Windows/evil.cfg' } | Should -Throw '*Unsafe destination*'
  }
  It 'throws when {custom} has no override' {
    { Resolve-DestPath '{custom}/cfg/autoexec.cfg' } | Should -Throw '*game folder*'
  }
}

Describe 'Get-GameStatus' {
  BeforeAll {
    $script:liveFile = Join-Path $TestDrive 'live.cfg'
    Set-Content $script:liveFile -Value 'chamber config v1' -NoNewline
    $script:liveHash = Get-Sha256 $script:liveFile
  }

  It 'reports guide for guide-only games' {
    $m = [pscustomobject]@{ slug = 'valorant'; guideOnly = $true }
    Get-GameStatus $m @{} | Should -Be 'guide'
  }
  It 'reports not-applied when no state entry exists' {
    $m = [pscustomobject]@{ slug = 'cs2' }
    Get-GameStatus $m @{} | Should -Be 'not-applied'
  }
  It 'reports applied when the live file matches the recorded hash' {
    $m = [pscustomobject]@{ slug = 'cs2' }
    $state = @{ cs2 = [pscustomobject]@{ targets = @([pscustomobject]@{ dest = $script:liveFile; hash = $script:liveHash }) } }
    Get-GameStatus $m $state | Should -Be 'applied'
  }
  It 'reports drifted when the live file was modified' {
    $m = [pscustomobject]@{ slug = 'cs2' }
    $drift = Join-Path $TestDrive 'drift.cfg'
    Set-Content $drift -Value 'edited by the game' -NoNewline
    $state = @{ cs2 = [pscustomobject]@{ targets = @([pscustomobject]@{ dest = $drift; hash = $script:liveHash }) } }
    Get-GameStatus $m $state | Should -Be 'drifted'
  }
  It 'reports drifted when the live file is missing' {
    $m = [pscustomobject]@{ slug = 'cs2' }
    $state = @{ cs2 = [pscustomobject]@{ targets = @([pscustomobject]@{ dest = (Join-Path $TestDrive 'gone.cfg'); hash = $script:liveHash }) } }
    Get-GameStatus $m $state | Should -Be 'drifted'
  }
}

Describe 'Test-CatalogSignature' {
  BeforeAll {
    # Ephemeral keypair so tests don't need the maintainer's private key
    $script:rsa = [System.Security.Cryptography.RSA]::Create(3072)
    $p = $script:rsa.ExportParameters($false)
    $script:testMod = [Convert]::ToBase64String($p.Modulus)
    $script:testExp = [Convert]::ToBase64String($p.Exponent)
    $script:payload = [Text.Encoding]::UTF8.GetBytes('{"catalogVersion":"1.0.0","games":[]}')
    $script:goodSig = $script:rsa.SignData($script:payload,
      [System.Security.Cryptography.HashAlgorithmName]::SHA256,
      [System.Security.Cryptography.RSASignaturePadding]::Pkcs1)
  }
  AfterAll { $script:rsa.Dispose() }

  It 'accepts a valid signature' {
    Test-CatalogSignature $script:payload $script:goodSig $script:testMod $script:testExp |
      Should -BeTrue
  }
  It 'rejects tampered content' {
    $tampered = [byte[]]$script:payload.Clone()
    $tampered[10] = $tampered[10] -bxor 1
    Test-CatalogSignature $tampered $script:goodSig $script:testMod $script:testExp |
      Should -BeFalse
  }
  It 'rejects a tampered signature' {
    $badSig = [byte[]]$script:goodSig.Clone()
    $badSig[0] = $badSig[0] -bxor 1
    Test-CatalogSignature $script:payload $badSig $script:testMod $script:testExp |
      Should -BeFalse
  }
  It 'rejects a signature from a different key' {
    $other = [System.Security.Cryptography.RSA]::Create(3072)
    try {
      $otherSig = $other.SignData($script:payload,
        [System.Security.Cryptography.HashAlgorithmName]::SHA256,
        [System.Security.Cryptography.RSASignaturePadding]::Pkcs1)
      Test-CatalogSignature $script:payload $otherSig $script:testMod $script:testExp |
        Should -BeFalse
    } finally { $other.Dispose() }
  }
  It 'verifies the committed catalog.json against the embedded public key' {
    $repoRoot = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
    $bytes = [IO.File]::ReadAllBytes((Join-Path $repoRoot 'catalog.json'))
    $sig   = [IO.File]::ReadAllBytes((Join-Path $repoRoot 'catalog.json.sig'))
    Test-CatalogSignature $bytes $sig | Should -BeTrue
  }
}

Describe 'Assert-NoCatalogRollback' {
  BeforeEach { Remove-Item $HighWaterFile -ErrorAction SilentlyContinue }
  AfterAll   { Remove-Item $HighWaterFile -ErrorAction SilentlyContinue }

  It 'allows any version when nothing was seen yet' {
    { Assert-NoCatalogRollback '1.0.0' } | Should -Not -Throw
  }
  It 'allows equal and newer versions' {
    Set-Content $HighWaterFile -Value '2.0.0'
    { Assert-NoCatalogRollback '2.0.0' } | Should -Not -Throw
    { Assert-NoCatalogRollback '2.1.0' } | Should -Not -Throw
  }
  It 'rejects an older catalog than previously seen' {
    Set-Content $HighWaterFile -Value '2.0.0'
    { Assert-NoCatalogRollback '1.9.0' } | Should -Throw '*rollback*'
  }
}

Describe 'Overrides round-trip' {
  It 'set, get, and remove an override' {
    $folder = Join-Path $TestDrive 'gamedir'
    New-Item -ItemType Directory -Path $folder | Out-Null
    Set-Override 'cs2' $folder
    (Get-Override 'cs2') | Should -Be (Resolve-Path $folder).Path
    Remove-Override 'cs2'
    Get-Override 'cs2' | Should -BeNullOrEmpty
  }
  It 'rejects a nonexistent folder' {
    { Set-Override 'cs2' (Join-Path $TestDrive 'nope') } | Should -Throw '*not found*'
  }
}

Describe 'App settings round-trip' {
  BeforeEach { Remove-Item $SettingsFile -ErrorAction SilentlyContinue }
  AfterAll   { Remove-Item $SettingsFile -ErrorAction SilentlyContinue }

  It 'defaults to no scan folders and no local source' {
    $s = Get-AppSettings
    @($s.scanFolders).Count | Should -Be 0
    $s.localSource | Should -BeNullOrEmpty
  }
  It 'adds, dedupes, and removes scan folders' {
    $f = Join-Path $TestDrive 'scanroot'
    New-Item -ItemType Directory -Force -Path $f | Out-Null
    Add-ScanFolder $f
    Add-ScanFolder $f
    @((Get-AppSettings).scanFolders).Count | Should -Be 1
    Remove-ScanFolder (Resolve-Path $f).Path
    @((Get-AppSettings).scanFolders).Count | Should -Be 0
  }
  It 'rejects a nonexistent scan folder' {
    { Add-ScanFolder (Join-Path $TestDrive 'nope-scan') } | Should -Throw '*not found*'
  }
  It 'sets and clears the local source' {
    $f = Join-Path $TestDrive 'localsrc'
    New-Item -ItemType Directory -Force -Path $f | Out-Null
    Set-LocalSource $f
    (Get-AppSettings).localSource | Should -Be (Resolve-Path $f).Path
    Clear-LocalSource
    (Get-AppSettings).localSource | Should -BeNullOrEmpty
  }
  It 'rejects a nonexistent local source' {
    { Set-LocalSource (Join-Path $TestDrive 'nope-src') } | Should -Throw '*not found*'
  }
}

Describe 'Manifest source merge' {
  BeforeAll {
    $script:src = Join-Path $TestDrive 'local-src'
    foreach ($d in 'cs2', 'newgame', 'mismatch') {
      New-Item -ItemType Directory -Force -Path (Join-Path $script:src $d) | Out-Null
    }
    Set-Content (Join-Path $script:src 'cs2/manifest.json') -Value `
      '{"name":"CS2 Local","slug":"cs2","platform":"steam","detect":{},"targets":[]}'
    Set-Content (Join-Path $script:src 'newgame/manifest.json') -Value `
      '{"name":"New Game","slug":"newgame","platform":"steam","detect":{},"targets":[]}'
    # slug/folder mismatch must be dropped: the folder name is how apply
    # resolves the slug back to files on disk
    Set-Content (Join-Path $script:src 'mismatch/manifest.json') -Value `
      '{"name":"Bad","slug":"other-slug","platform":"steam","detect":{},"targets":[]}'
    Set-LocalSource $script:src
  }
  AfterAll { Clear-LocalSource }

  It 'local source shadows the bundled copy of the same slug' {
    $m = @(Get-Manifests | Where-Object { $_.slug -eq 'cs2' })
    $m.Count    | Should -Be 1
    $m[0].name   | Should -Be 'CS2 Local'
    $m[0].source | Should -Be 'local'
  }
  It 'exposes brand-new local games alongside bundled ones' {
    $slugs = (Get-Manifests).slug
    $slugs | Should -Contain 'newgame'
    $slugs | Should -Contain 'fortnite'
  }
  It 'drops manifests whose slug does not match their folder' {
    (Get-Manifests).slug | Should -Not -Contain 'other-slug'
  }
  It 'Get-GameLocation honors source priority' {
    (Get-GameLocation 'cs2').source      | Should -Be 'local'
    (Get-GameLocation 'fortnite').source | Should -Be 'bundled'
  }
  It 'Get-GameLocation throws for an unknown slug' {
    { Get-GameLocation 'no-such-game' } | Should -Throw '*No config folder*'
  }
}

Describe '{scan} token detection' {
  BeforeAll {
    $script:scanRoot = Join-Path $TestDrive 'scan-root'
    New-Item -ItemType Directory -Force -Path (Join-Path $script:scanRoot 'Cool Game') | Out-Null
  }
  AfterAll { Remove-Item $SettingsFile -ErrorAction SilentlyContinue }

  It 'does not match when no scan folders are configured' {
    Remove-Item $SettingsFile -ErrorAction SilentlyContinue
    $m = [pscustomobject]@{ detect = [pscustomobject]@{ paths = @('{scan}/Cool Game') } }
    Test-GameInstalled $m | Should -BeFalse
  }
  It 'matches a game folder under a configured scan folder' -Skip:(-not $IsWindowsHost) {
    Add-ScanFolder $script:scanRoot
    $m = [pscustomobject]@{ detect = [pscustomobject]@{ paths = @('{scan}/Cool Game') } }
    Test-GameInstalled $m | Should -BeTrue
  }
  It 'does not match a folder absent from every scan folder' -Skip:(-not $IsWindowsHost) {
    Add-ScanFolder $script:scanRoot
    $m = [pscustomobject]@{ detect = [pscustomobject]@{ paths = @('{scan}/Missing Game') } }
    Test-GameInstalled $m | Should -BeFalse
  }
  It 'rejects {scan} in a dest path' {
    Test-SafeDest '{scan}/evil/file.cfg' | Should -BeFalse
  }
}

Describe 'Apply from a local source' -Skip:(-not $IsWindowsHost) {
  BeforeAll {
    $env:LOCALAPPDATA = Join-Path $TestDrive 'ApplyLocalAppData'
    New-Item -ItemType Directory -Force -Path $env:LOCALAPPDATA | Out-Null
    $script:applySrc = Join-Path $TestDrive 'apply-src'
    New-Item -ItemType Directory -Force -Path (Join-Path $script:applySrc 'localonly') | Out-Null
    Set-Content (Join-Path $script:applySrc 'localonly/manifest.json') -Value `
      '{"name":"Local Only","slug":"localonly","platform":"steam","detect":{},"targets":[{"template":"test.cfg","dest":"{localappdata}/LocalOnly/test.cfg"}],"variables":["refreshRate"]}'
    Set-Content (Join-Path $script:applySrc 'localonly/test.cfg') -Value 'fps_max {{refreshRate}}' -NoNewline
    Set-LocalSource $script:applySrc
  }
  AfterAll { Clear-LocalSource }

  It 'applies without a catalog entry (integrity gate is cache/bundled-only)' {
    Invoke-Apply 'localonly' | Out-Null
    $dest = Join-Path $env:LOCALAPPDATA 'LocalOnly\test.cfg'
    Test-Path $dest | Should -BeTrue
    Get-Content $dest -Raw | Should -Match 'fps_max \d+'
    Invoke-Revert 'localonly' | Out-Null
    Test-Path $dest | Should -BeFalse
  }
}
