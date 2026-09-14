<#
.SYNOPSIS
Decides what a release run has to upload, given what is already published.

.DESCRIPTION
A rerun must not replace an archive that is already on the release: a zip
records each entry's timestamp, so rebuilding the same binaries produces
different bytes and a different SHA-256, and every manifest or release note
quoting the old hash would stop matching the download.

A run can also have failed between uploading the archive and uploading its
checksum, so the two files are looked for separately and the checksum can be
restored on its own — from the published archive, not from the local build.
#>
[CmdletBinding()]
param(
    [string[]]$PublishedAssets = @(),
    [Parameter(Mandatory)][string]$ArchiveName,
    [switch]$ReleaseExists
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$checksumName = "$ArchiveName.sha256"
$assets = @($PublishedAssets)

if (-not $ReleaseExists) {
    return [pscustomobject]@{ Action = 'create'; RestoreChecksum = $false }
}
if ($assets -notcontains $ArchiveName) {
    # No archive to preserve, so this build becomes the release's archive.
    return [pscustomobject]@{ Action = 'upload'; RestoreChecksum = $false }
}
[pscustomobject]@{
    Action          = 'reuse'
    RestoreChecksum = ($assets -notcontains $checksumName)
}
