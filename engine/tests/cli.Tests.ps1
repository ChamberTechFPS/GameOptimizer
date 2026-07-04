<#
  Pester 5 tests for engine/lib.ps1.
  Cross-platform: pure string/logic functions run anywhere pwsh does;
  Windows-only detection functions (Steam/Epic/registry) are not covered here.
#>

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
