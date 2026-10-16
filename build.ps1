# Builds dist\: the Rust tray service (Halo.exe, icon stamped in) and the WinUI 3 settings window (ui\).
# Everything runs at low priority so a build never fights a game.
param([switch]$Test)
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
# Rust's bundled MinGW lacks most system import libraries, so link with a full MinGW-w64:
# `winget install BrechtSanders.WinLibs.POSIX.MSVCRT --scope user`.
$winlibs = Get-ChildItem (Join-Path $env:LOCALAPPDATA 'Microsoft\WinGet\Packages') -Directory -Filter 'BrechtSanders.WinLibs*' -ErrorAction SilentlyContinue |
    ForEach-Object { Join-Path $_.FullName 'mingw64\bin' } | Where-Object { Test-Path (Join-Path $_ 'gcc.exe') } | Select-Object -First 1
if (-not $winlibs) { throw 'MinGW-w64 not found. Install it with: winget install BrechtSanders.WinLibs.POSIX.MSVCRT --scope user' }
$env:PATH = "$cargoBin;$winlibs;$env:PATH"

function Invoke-Low([string]$Tool, [string[]]$ToolArgs) {
    $p = Start-Process $Tool -ArgumentList $ToolArgs -NoNewWindow -PassThru
    $null = $p.Handle # without this PowerShell never reports ExitCode
    try { $p.PriorityClass = 'BelowNormal' } catch {}
    $p.WaitForExit()
    if ($p.ExitCode -ne 0) { throw "$Tool $($ToolArgs -join ' ') failed" }
}

if ($Test) { Invoke-Low cargo @('test', '--release', '--bin', 'halo') }
Invoke-Low cargo @('build', '--release', '--bins')

New-Item -ItemType Directory -Force dist | Out-Null
Copy-Item target\release\halo.exe dist\Halo.exe -Force
& target\release\halo-pack.exe dist\Halo.exe

$ui = Join-Path $PSScriptRoot 'dist\ui'
if (Test-Path $ui) { Remove-Item -Recurse -Force $ui }
Invoke-Low dotnet @('publish', 'HaloSettings\HaloSettings.csproj', '-c', 'Release', '-p:Platform=x64', '-o', $ui, '-nologo', '-v:q')

$size = (Get-ChildItem dist -Recurse -File | Measure-Object Length -Sum).Sum / 1MB
Write-Host ("dist ready: Halo.exe {0:N1} MB, total {1:N0} MB" -f ((Get-Item dist\Halo.exe).Length / 1MB), $size)
