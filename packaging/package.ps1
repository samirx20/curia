# Builds a Windows package of Curia: the host, the `curia` command, the desktop app with the Qt
# files it needs, the phone web app and the fonts, in one folder with an installer, zipped.
#   pwsh -NoProfile -File packaging/package.ps1            (out: target/package/Curia-<version>-windows.zip)
# The package is unsigned: Windows SmartScreen will warn the first time it runs.
param([string]$Out = (Join-Path $PSScriptRoot "..\target\package"))
$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$qt = if ($env:CURIA_QT) { $env:CURIA_QT } else { "$HOME/.rostrum/libs/qt/6.12.0-msvc2022_64" }
$version = (Select-String -Path "$root\Cargo.toml" -Pattern '^version\s*=\s*"(.+)"').Matches[0].Groups[1].Value

function Step($what, [scriptblock]$run) {
    Write-Host "== $what"
    & $run
    if ($LASTEXITCODE) { throw "$what failed (exit $LASTEXITCODE)" }
}

Push-Location $root
try {
    Step "host and curia command (release)" { cargo build --release --locked -p curia-host -p curia-client }
    Step "desktop app (release)" { pwsh -NoProfile -File "$root\desktop\build.ps1" build --release --locked }
    Push-Location "$root\web"
    Step "phone web app" { npm run build }
    Pop-Location

    $stage = Join-Path $Out "Curia"
    if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
    New-Item -ItemType Directory -Force $stage | Out-Null
    Copy-Item "$root\target\release\curia-host.exe", "$root\target\release\curia.exe" $stage
    Copy-Item "$root\desktop\target\release\curia-desktop.exe" $stage
    Copy-Item "$root\desktop\fonts" "$stage\fonts" -Recurse
    Copy-Item "$root\web\dist" "$stage\web" -Recurse
    # The providers' marks are kept off the public repo; a local build carries them when present.
    if (Test-Path "$root\desktop\providers") { Copy-Item "$root\desktop\providers" "$stage\providers" -Recurse }

    Step "Qt files" { & "$qt\bin\windeployqt.exe" --release --qmldir "$root\desktop\qml" --no-translations --no-system-d3d-compiler --no-opengl-sw "$stage\curia-desktop.exe" }

    # The Visual C++ runtime, beside the programs (app-local), so a fresh Windows needs nothing else.
    $crt = Get-ChildItem "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Redist\MSVC\*\x64\Microsoft.VC143.CRT" -Directory | Select-Object -Last 1
    if (-not $crt) { throw "Visual C++ runtime files not found under the Build Tools' Redist folder" }
    Copy-Item "$($crt.FullName)\*.dll" $stage
    Remove-Item "$stage\vc_redist*.exe" -ErrorAction SilentlyContinue

    Copy-Item "$PSScriptRoot\install.ps1", "$PSScriptRoot\uninstall.ps1", "$PSScriptRoot\Install Curia.cmd" $stage
    Copy-Item "$root\ATTRIBUTIONS.md" $stage
    Set-Content "$stage\version.txt" $version

    $zip = Join-Path $Out "Curia-$version-windows.zip"
    if (Test-Path $zip) { Remove-Item $zip }
    Compress-Archive -Path $stage -DestinationPath $zip
    $size = [math]::Round((Get-Item $zip).Length / 1MB, 1)
    Write-Host "== done: $zip ($size MB)"
} finally {
    Pop-Location
}
