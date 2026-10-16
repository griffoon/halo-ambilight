# Builds dist\Halo.exe: release build, icon stamped in. Runs at low priority so it never fights a game.
param([switch]$Test)
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
# Rust's bundled MinGW lacks most system import libraries (shlwapi, opengl32, ...),
# so link with a full MinGW-w64: `winget install BrechtSanders.WinLibs.POSIX.MSVCRT --scope user`.
$winlibs = Get-ChildItem (Join-Path $env:LOCALAPPDATA 'Microsoft\WinGet\Packages') -Directory -Filter 'BrechtSanders.WinLibs*' -ErrorAction SilentlyContinue |
    ForEach-Object { Join-Path $_.FullName 'mingw64\bin' } | Where-Object { Test-Path (Join-Path $_ 'gcc.exe') } | Select-Object -First 1
if (-not $winlibs) { throw 'MinGW-w64 not found. Install it with: winget install BrechtSanders.WinLibs.POSIX.MSVCRT --scope user' }
$env:PATH = "$cargoBin;$winlibs;$env:PATH"

function Invoke-Low([string[]]$CargoArgs) {
    $p = Start-Process cargo -ArgumentList $CargoArgs -NoNewWindow -PassThru
    $null = $p.Handle # without this PowerShell never reports ExitCode
    try { $p.PriorityClass = 'BelowNormal' } catch {}
    $p.WaitForExit()
    if ($p.ExitCode -ne 0) { throw "cargo $($CargoArgs -join ' ') failed" }
}

if ($Test) { Invoke-Low @('test', '--release', '--bin', 'halo') }
Invoke-Low @('build', '--release', '--bins')

New-Item -ItemType Directory -Force dist | Out-Null
Copy-Item target\release\halo.exe dist\Halo.exe -Force
& target\release\halo-pack.exe dist\Halo.exe
Write-Host "dist\Halo.exe ($([math]::Round((Get-Item dist\Halo.exe).Length / 1MB, 1)) MB)"
