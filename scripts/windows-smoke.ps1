param(
    [Parameter(Mandatory=$true)][string]$Installer,
    [string]$EvidenceDirectory = (Join-Path $env:TEMP ("MyCalibro-smoke-" + [guid]::NewGuid().ToString('N')))
)
$ErrorActionPreference = 'Stop'
$installerPath = (Resolve-Path $Installer).Path
# A unique directory and /TESTMODE avoid modifying an existing installation or registry entry.
if (Test-Path $EvidenceDirectory) { throw 'EvidenceDirectory must not already exist' }
New-Item -ItemType Directory -Path $EvidenceDirectory | Out-Null
$evidenceRoot = (Resolve-Path $EvidenceDirectory).Path
$installRoot = Join-Path $evidenceRoot 'install'
$dataRoot = Join-Path $evidenceRoot 'user-data'
$oldLocalAppData = $env:LOCALAPPDATA
$appProcess = $null
try {
    $install = Start-Process -FilePath $installerPath -ArgumentList "/S /TESTMODE /D=$installRoot" -PassThru -Wait
    if ($install.ExitCode -ne 0) { throw "Installer exited $($install.ExitCode)" }
    $executable = Join-Path $installRoot 'MyCalibro.exe'
    if (-not (Test-Path $executable)) { throw 'Installed executable is missing' }
    New-Item -ItemType Directory -Path (Join-Path $dataRoot 'MyCalibro') -Force | Out-Null
    $catalog = Join-Path $dataRoot 'MyCalibro\library.json'
    [IO.File]::WriteAllText($catalog, '[]', [Text.UTF8Encoding]::new($false))
    $originalHash = (Get-FileHash $catalog -Algorithm SHA256).Hash
    $env:LOCALAPPDATA = $dataRoot
    $screenshot = Join-Path $evidenceRoot 'installed-window.png'
    $appProcess = Start-Process -FilePath $executable -ArgumentList "--screenshot `"$screenshot`"" -PassThru -RedirectStandardError (Join-Path $evidenceRoot 'stderr.log')
    # Retain the process handle before waiting; otherwise PowerShell may report a null ExitCode.
    $null = $appProcess.Handle
    if (-not $appProcess.WaitForExit(60000)) { $appProcess.Kill(); throw 'Installed application did not finish the screenshot test in 60 seconds' }
    if ($appProcess.ExitCode -ne 0) { throw "Application exited $($appProcess.ExitCode); inspect stderr.log" }
    if (-not (Test-Path $screenshot) -or (Get-Item $screenshot).Length -eq 0) { throw 'The installed application did not produce a window screenshot' }
    if ((Get-FileHash $catalog -Algorithm SHA256).Hash -ne $originalHash) { throw 'Opening the installed application changed the catalog unexpectedly' }
    $uninstall = Start-Process -FilePath (Join-Path $installRoot 'Uninstall.exe') -ArgumentList "/S _?=$installRoot" -PassThru -Wait
    if ($uninstall.ExitCode -ne 0) { throw "Uninstaller exited $($uninstall.ExitCode)" }
    if (Test-Path $executable) { throw 'Uninstaller did not remove the application executable' }
    if ((Get-FileHash $catalog -Algorithm SHA256).Hash -ne $originalHash) { throw 'Uninstall changed or removed user data' }
    [ordered]@{
        installer = $installerPath
        installer_sha256 = (Get-FileHash $installerPath -Algorithm SHA256).Hash
        installed_launch = 'passed'
        window_screenshot = $screenshot
        catalog_unchanged = $true
        uninstall_preserves_data = $true
        timestamp_utc = [DateTime]::UtcNow.ToString('o')
    } | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $evidenceRoot 'result.json')
    Write-Output "Windows smoke test passed. Evidence: $evidenceRoot"
} finally {
    $env:LOCALAPPDATA = $oldLocalAppData
    if ($null -ne $appProcess -and -not $appProcess.HasExited) { $appProcess.Kill() }
}
