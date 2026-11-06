# Installs Curia for this Windows user: no admin rights, nothing outside this user's folders.
#   Copies the programs to %LOCALAPPDATA%\Programs\Curia, adds Curia to the Start menu and to
#   Settings > Apps (so it can be removed there). The host keeps its data in %LOCALAPPDATA%\Curia\data,
#   which an update or removal leaves alone.
# -To <folder> installs elsewhere; -NoShortcuts skips the Start menu and Settings entries (for tests).
param([string]$To = (Join-Path $env:LOCALAPPDATA "Programs\Curia"), [switch]$NoShortcuts)
$ErrorActionPreference = "Stop"
$from = $PSScriptRoot
$version = (Get-Content "$from\version.txt" -ErrorAction SilentlyContinue) -join ""

# An older copy that is running would hold its files: stop the programs started from that folder.
$target = [IO.Path]::GetFullPath($To).TrimEnd('\') + '\'
Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -and $_.ExecutablePath.StartsWith($target, 'OrdinalIgnoreCase') } |
    ForEach-Object { Write-Host "Stopping $($_.Name) ($($_.ProcessId)) from the old copy"; Stop-Process -Id $_.ProcessId -Force }

New-Item -ItemType Directory -Force $To | Out-Null
Get-ChildItem $from | Where-Object { $_.Name -ne "Install Curia.cmd" } | Copy-Item -Destination $To -Recurse -Force
Write-Host "Copied Curia $version to $To"

if (-not $NoShortcuts) {
    $shell = New-Object -ComObject WScript.Shell
    $link = $shell.CreateShortcut((Join-Path ([Environment]::GetFolderPath("Programs")) "Curia.lnk"))
    $link.TargetPath = Join-Path $To "curia-desktop.exe"
    $link.WorkingDirectory = $To
    $link.Description = "Curia: a control room for agent CLIs"
    $link.Save()

    $key = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Curia"
    New-Item -Force $key | Out-Null
    $size = [int]((Get-ChildItem $To -Recurse -File | Measure-Object Length -Sum).Sum / 1KB)
    $entries = @{
        DisplayName = "Curia"; DisplayVersion = $version; Publisher = "Curia"
        InstallLocation = $To; DisplayIcon = (Join-Path $To "curia-desktop.exe"); EstimatedSize = $size
        UninstallString = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$(Join-Path $To 'uninstall.ps1')`""
        NoModify = 1; NoRepair = 1
    }
    foreach ($name in $entries.Keys) { Set-ItemProperty $key $name $entries[$name] }
    Write-Host "Added Curia to the Start menu and to Settings > Apps"
}
Write-Host "Done. Open Curia from the Start menu; it starts its host by itself."
