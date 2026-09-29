#Requires -RunAsAdministrator
[CmdletBinding()]
param(
    [string]$Distribution = 'Ubuntu',
    [string]$ExpectedVhd = 'C:\Users\kamui\AppData\Local\Packages\CanonicalGroupLimited.Ubuntu_79rhkp1fndgsc\LocalState\ext4.vhdx'
)
$ErrorActionPreference = 'Stop'
$logRoot = Join-Path $env:TEMP 'codereviewbench-wsl-compaction'
New-Item -ItemType Directory -Force -Path $logRoot | Out-Null
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
Start-Transcript -Path (Join-Path $logRoot "$stamp.log")
$shutdown = $false
$mounted = $false
try {
    $distros = @(Get-ChildItem HKCU:\Software\Microsoft\Windows\CurrentVersion\Lxss |
        ForEach-Object { Get-ItemProperty $_.PSPath } |
        Where-Object DistributionName -eq $Distribution)
    if ($distros.Count -ne 1) { throw "Expected exactly one registered $Distribution distribution." }
    $vhd = [IO.Path]::GetFullPath((Join-Path $distros[0].BasePath 'ext4.vhdx'))
    if ($vhd -ine [IO.Path]::GetFullPath($ExpectedVhd)) { throw "Unexpected virtual disk: $vhd" }
    $before = (Get-Item -LiteralPath $vhd).Length
    $freeBefore = (Get-PSDrive C).Free
    Get-Command Mount-VHD, Optimize-VHD, Dismount-VHD -ErrorAction Stop | Out-Null
    Write-Host "Compacting $vhd"
    Write-Host ('Before: VHD {0:N2} GiB; C: free {1:N2} GiB' -f ($before / 1GB), ($freeBefore / 1GB))
    & wsl.exe --distribution $Distribution --user root --exec /usr/sbin/fstrim -v /
    if ($LASTEXITCODE -ne 0) { throw 'Linux trim failed; compaction was not started.' }
    & wsl.exe --shutdown
    if ($LASTEXITCODE -ne 0) { throw 'WSL shutdown failed; compaction was not started.' }
    $shutdown = $true
    Mount-VHD -Path $vhd -ReadOnly -NoDriveLetter
    $mounted = $true
    Optimize-VHD -Path $vhd -Mode Full
    Dismount-VHD -Path $vhd
    $mounted = $false
    $after = (Get-Item -LiteralPath $vhd).Length
    $freeAfter = (Get-PSDrive C).Free
    $result = [pscustomobject]@{
        Distribution = $Distribution
        VhdPath = $vhd
        BeforeBytes = $before
        AfterBytes = $after
        ReclaimedBytes = $before - $after
        CFreeBeforeBytes = $freeBefore
        CFreeAfterBytes = $freeAfter
        CompletedAt = (Get-Date).ToString('o')
    }
    $result | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $logRoot 'latest-result.json') -Encoding UTF8
    Write-Host ('After: VHD {0:N2} GiB; C: free {1:N2} GiB; reclaimed {2:N2} GiB' -f ($after / 1GB), ($freeAfter / 1GB), (($before - $after) / 1GB))
    if ($after -ge $before) { Write-Warning 'The virtual disk did not shrink. Inspect the transcript before resuming benchmarks.' }
} finally {
    try {
        if ($mounted) { Dismount-VHD -Path $vhd }
        if ($shutdown) { & wsl.exe --distribution $Distribution --exec /bin/true }
    } finally {
        Stop-Transcript
    }
}
