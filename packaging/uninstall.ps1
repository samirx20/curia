# Removes Curia for this Windows user: stops its programs, removes the program folder, the Start
# menu entry, the Settings > Apps entry and the start-at-sign-in entry. The host's data
# (%LOCALAPPDATA%\Curia\data: runs, threads, paired devices) stays unless -RemoveData is given.
param([switch]$RemoveData)
$ErrorActionPreference = "Stop"
$dir = [IO.Path]::GetFullPath($PSScriptRoot).TrimEnd('\') + '\'

# The start-at-sign-in loop runs in cmd.exe and would start the host again: stop it first.
Get-CimInstance Win32_Process -Filter "Name='cmd.exe'" | Where-Object { $_.CommandLine -and $_.CommandLine.Contains("Curia host.cmd") } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force }
Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -and $_.ExecutablePath.StartsWith($dir, 'OrdinalIgnoreCase') } |
    ForEach-Object { Write-Host "Stopping $($_.Name) ($($_.ProcessId))"; Stop-Process -Id $_.ProcessId -Force }

Remove-Item (Join-Path ([Environment]::GetFolderPath("Programs")) "Curia.lnk") -ErrorAction SilentlyContinue
Remove-Item (Join-Path ([Environment]::GetFolderPath("Startup")) "Curia host.cmd") -ErrorAction SilentlyContinue
Remove-Item "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Curia" -Recurse -ErrorAction SilentlyContinue
if ($RemoveData) { Remove-Item (Join-Path $env:LOCALAPPDATA "Curia") -Recurse -Force -ErrorAction SilentlyContinue }

# A running script can't delete its own folder: a short-lived cmd does it once this one exits.
Start-Process cmd.exe -WindowStyle Hidden -ArgumentList "/c timeout /t 2 >nul & rmdir /s /q `"$($dir.TrimEnd('\'))`""
Write-Host "Curia is removed." $(if (-not $RemoveData) { "Its data stays in $env:LOCALAPPDATA\Curia\data." })
