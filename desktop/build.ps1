# Cargo for the desktop app with the build tools it needs (MSVC, Qt's qmake), from anywhere:
#   pwsh -File desktop/build.ps1 build            (debug build: desktop/target/debug/curia-desktop.exe)
#   pwsh -File desktop/build.ps1 run -- --data C:\curia-dev\data-m6
# Qt comes from Rostrum's install unless CURIA_QT points elsewhere. The first build compiles Qt's
# glue and takes several minutes; later ones are quick.
param([Parameter(ValueFromRemainingArguments)] $rest)
$vc = "C:/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/VC/Auxiliary/Build/vcvars64.bat"
if (-not (Test-Path $vc)) { Write-Error "Visual Studio 2022 Build Tools not found at $vc"; exit 1 }
cmd /c "`"$vc`" >nul && set" | ForEach-Object { if ($_ -match '^([^=]+)=(.*)$') { Set-Item "env:$($matches[1])" $matches[2] } }
$qt = if ($env:CURIA_QT) { $env:CURIA_QT } else { "$HOME/.rostrum/libs/qt/6.12.0-msvc2022_64" }
if (-not (Test-Path "$qt/bin/qmake.exe")) { Write-Error "Qt not found at $qt (set CURIA_QT)"; exit 1 }
$env:QMAKE = "$qt/bin/qmake.exe"
$env:PATH = "$qt/bin;$env:PATH"
Push-Location $PSScriptRoot
& cargo @rest
$code = $LASTEXITCODE
Pop-Location
exit $code
