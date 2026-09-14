<#
Exercises the downgrade guard in update-scoop-manifest.ps1. Run with
`pwsh -File .github/scripts/update-scoop-manifest.tests.ps1`; CI runs it on
every push. Windows PowerShell 5.1 works too, which is why the script under
test avoids types that only PowerShell 7 has.
#>
Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$script = Join-Path $PSScriptRoot 'update-scoop-manifest.ps1'
$failures = 0

function New-Manifest {
    param([string]$Version)
    $path = Join-Path ([System.IO.Path]::GetTempPath()) ("scoop-" + [guid]::NewGuid() + ".json")
    $body = @{
        version      = $Version
        architecture = @{ '64bit' = @{ url = 'https://example.invalid/old.zip'; hash = 'old' } }
    }
    $body | ConvertTo-Json -Depth 10 | Set-Content $path -Encoding utf8
    return $path
}

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

function Assert-Equal {
    param($Expected, $Actual, [string]$What)
    if ($Expected -ne $Actual) {
        throw "$What : expected '$Expected', got '$Actual'"
    }
}

function Invoke-Update {
    param([string]$From, [string]$To)
    $path = New-Manifest $From
    try {
        $changed = & $script -Path $path -Version $To -Url 'https://example.invalid/new.zip' -Hash 'new'
        $after = (Get-Content $path -Raw | ConvertFrom-Json)
        return [pscustomobject]@{ Changed = [bool]$changed; Version = $after.version; Hash = $after.architecture.'64bit'.hash }
    } finally {
        Remove-Item $path -ErrorAction Ignore
    }
}

Test-Case 'moves forward' {
    $r = Invoke-Update -From '0.1.0' -To '0.2.0'
    Assert-Equal $true $r.Changed 'changed'
    Assert-Equal '0.2.0' $r.Version 'version'
    Assert-Equal 'new' $r.Hash 'hash'
}

Test-Case 'rewrites the same version' {
    $r = Invoke-Update -From '0.1.0' -To '0.1.0'
    Assert-Equal $true $r.Changed 'changed'
    Assert-Equal 'new' $r.Hash 'hash'
}

Test-Case 'refuses to go back' {
    $r = Invoke-Update -From '0.2.0' -To '0.1.0'
    Assert-Equal $false $r.Changed 'changed'
    Assert-Equal '0.2.0' $r.Version 'version'
    Assert-Equal 'old' $r.Hash 'hash'
}

Test-Case 'a release outranks its own pre-release' {
    $r = Invoke-Update -From '1.0.0' -To '1.0.0-rc.1'
    Assert-Equal $false $r.Changed 'changed'
    Assert-Equal '1.0.0' $r.Version 'version'
}

Test-Case 'a pre-release is superseded by its release' {
    $r = Invoke-Update -From '1.0.0-rc.1' -To '1.0.0'
    Assert-Equal $true $r.Changed 'changed'
    Assert-Equal '1.0.0' $r.Version 'version'
}

Test-Case 'pre-release identifiers are ordered by SemVer rules' {
    Assert-Equal $true (Invoke-Update -From '1.0.0-alpha' -To '1.0.0-beta').Changed 'alpha -> beta'
    Assert-Equal $false (Invoke-Update -From '1.0.0-beta' -To '1.0.0-alpha').Changed 'beta -> alpha'
    Assert-Equal $true (Invoke-Update -From '1.0.0-rc.2' -To '1.0.0-rc.10').Changed 'rc.2 -> rc.10'
    Assert-Equal $false (Invoke-Update -From '1.0.0-rc.10' -To '1.0.0-rc.2').Changed 'rc.10 -> rc.2'
    Assert-Equal $true (Invoke-Update -From '1.0.0-alpha' -To '1.0.0-alpha.1').Changed 'alpha -> alpha.1'
}

Test-Case 'double digits compare numerically, not as text' {
    Assert-Equal $true (Invoke-Update -From '0.9.0' -To '0.10.0').Changed '0.9.0 -> 0.10.0'
    Assert-Equal $false (Invoke-Update -From '0.10.0' -To '0.9.0').Changed '0.10.0 -> 0.9.0'
}

Test-Case 'an unparsable version stops the update' {
    foreach ($pair in @(@('latest', '1.0.0'), @('1.0.0', 'v1.0.0'))) {
        $threw = $false
        try { Invoke-Update -From $pair[0] -To $pair[1] } catch { $threw = $true }
        Assert-Equal $true $threw "$($pair[0]) -> $($pair[1]) should have thrown"
    }
}

Write-Host "$($failures) failing case(s)"
exit ([int]($failures -gt 0))
