<#
.SYNOPSIS
Points the Scoop manifest at a release, but never at an older one.

.DESCRIPTION
Re-running an older tag must not walk the manifest backwards, so the version
in the file is compared with the one being published and the file is left
alone unless the new one is at least as new.

[System.Version] cannot do this: it rejects a SemVer pre-release such as
1.0.0-rc.1 outright, and treating that rejection as "compare failed, update
anyway" is how a stable release gets replaced by a release candidate. The
comparison below follows the SemVer 2.0.0 precedence rules, and an
unparsable version stops the update rather than waving it through.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Path,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$Url,
    [Parameter(Mandatory)][string]$Hash
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

function ConvertTo-SemVer {
    param([string]$Text)
    $pattern = '^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$'
    $m = [regex]::Match($Text, $pattern)
    if (-not $m.Success) {
        throw "'$Text' is not a SemVer version"
    }
    # Built separately: an `if` whose branch yields an empty array evaluates
    # to $null, and the comparison below counts on always having an array.
    $pre = @()
    if ($m.Groups[4].Success) {
        $pre = @($m.Groups[4].Value.Split('.'))
    }
    [pscustomobject]@{
        Major      = [int]$m.Groups[1].Value
        Minor      = [int]$m.Groups[2].Value
        Patch      = [int]$m.Groups[3].Value
        PreRelease = $pre
    }
}

function Compare-PreReleaseId {
    param([string]$Left, [string]$Right)
    $leftNumeric = $Left -match '^\d+$'
    $rightNumeric = $Right -match '^\d+$'
    if ($leftNumeric -and $rightNumeric) {
        return [int]$Left - [int]$Right
    }
    # Numeric identifiers always rank lower than alphanumeric ones.
    if ($leftNumeric) { return -1 }
    if ($rightNumeric) { return 1 }
    return [string]::CompareOrdinal($Left, $Right)
}

function Compare-SemVer {
    param($Left, $Right)
    foreach ($part in 'Major', 'Minor', 'Patch') {
        $diff = $Left.$part - $Right.$part
        if ($diff -ne 0) { return $diff }
    }
    # A pre-release ranks below the release it leads up to.
    if ($Left.PreRelease.Count -eq 0 -and $Right.PreRelease.Count -gt 0) { return 1 }
    if ($Left.PreRelease.Count -gt 0 -and $Right.PreRelease.Count -eq 0) { return -1 }
    $shared = [Math]::Min($Left.PreRelease.Count, $Right.PreRelease.Count)
    for ($i = 0; $i -lt $shared; $i++) {
        $diff = Compare-PreReleaseId $Left.PreRelease[$i] $Right.PreRelease[$i]
        if ($diff -ne 0) { return $diff }
    }
    # All shared identifiers equal: the longer set wins.
    return $Left.PreRelease.Count - $Right.PreRelease.Count
}

$manifest = Get-Content $Path -Raw | ConvertFrom-Json
$current = ConvertTo-SemVer $manifest.version
$incoming = ConvertTo-SemVer $Version

if ((Compare-SemVer $current $incoming) -gt 0) {
    Write-Host "the manifest is at $($manifest.version); not replacing it with $Version"
    return $false
}

$manifest.version = $Version
$manifest.architecture.'64bit'.url = $Url
$manifest.architecture.'64bit'.hash = $Hash
$manifest | ConvertTo-Json -Depth 10 | Set-Content $Path -Encoding utf8
Write-Host "the manifest now points at $Version"
return $true
