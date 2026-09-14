<#
Covers the decision plan-release-assets.ps1 makes for a release run, including
the reruns that used to replace a published archive or skip a missing
checksum. Run with `pwsh -File .github/scripts/plan-release-assets.tests.ps1`.
#>
Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$script = Join-Path $PSScriptRoot 'plan-release-assets.ps1'
$archive = 'win-make-ro-0.1.0-x86_64-pc-windows-msvc.zip'
$checksum = "$archive.sha256"
$failures = 0

function Test-Case {
    param([string]$Name, [scriptblock]$Body)
    try {
        & $Body
        Write-Host "ok   $Name"
    } catch {
        $script:failures++
        Write-Host "FAIL $Name"
        Write-Host "     $($_.Exception.Message)"
    }
}

function Assert-Plan {
    param($Plan, [string]$Action, [bool]$RestoreChecksum)
    if ($Plan.Action -ne $Action) {
        throw "expected action '$Action', got '$($Plan.Action)'"
    }
    if ($Plan.RestoreChecksum -ne $RestoreChecksum) {
        throw "expected RestoreChecksum $RestoreChecksum, got $($Plan.RestoreChecksum)"
    }
}

Test-Case 'a new release is created' {
    $plan = & $script -ArchiveName $archive
    Assert-Plan $plan 'create' $false
}

Test-Case 'an empty release gets this build' {
    $plan = & $script -ArchiveName $archive -ReleaseExists
    Assert-Plan $plan 'upload' $false
}

Test-Case 'a complete release is left alone' {
    $plan = & $script -ArchiveName $archive -ReleaseExists -PublishedAssets @($archive, $checksum)
    Assert-Plan $plan 'reuse' $false
}

# The archive went up, then the run died before the checksum did.
Test-Case 'a missing checksum is restored without touching the archive' {
    $plan = & $script -ArchiveName $archive -ReleaseExists -PublishedAssets @($archive)
    Assert-Plan $plan 'reuse' $true
}

Test-Case 'a stray checksum alone still uploads the archive' {
    $plan = & $script -ArchiveName $archive -ReleaseExists -PublishedAssets @($checksum)
    Assert-Plan $plan 'upload' $false
}

Test-Case 'assets of other releases are ignored' {
    $others = @('win-make-ro-0.2.0-x86_64-pc-windows-msvc.zip', 'notes.txt')
    $plan = & $script -ArchiveName $archive -ReleaseExists -PublishedAssets $others
    Assert-Plan $plan 'upload' $false
}

Write-Host "$($failures) failing case(s)"
exit ([int]($failures -gt 0))
