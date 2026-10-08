[CmdletBinding()]
param(
    [string]$RepositoryRoot
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$OutputEncoding = [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
    $RepositoryRoot = Join-Path $PSScriptRoot '../..'
}
$repository = (Resolve-Path -LiteralPath $RepositoryRoot).Path
$verifierVersion = '0.2026.10.04.426d8b0'
$proofToolchain = '1.98.1'
$archiveDigest = 'b36968a0f333036e7e1770bb73fdd407a35c8fd163124c1d309972574150da61'
$sharedModule = Join-Path $repository 'crates/birdman-game-core/src/pause_reasons.rs'
$positiveDriver = Join-Path $repository 'proofs/pause_reasons.rs'
$negativeDriver = Join-Path $repository 'proofs/pause_reasons_negative.rs'
$productToolchain = Join-Path $repository 'rust-toolchain.toml'
$expectedInclude = '../crates/birdman-game-core/src/pause_reasons.rs'

function Invoke-NativeCapture {
    param([string]$Command, [string[]]$Arguments)
    $previousPreference = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $lines = @(& $Command @Arguments 2>&1)
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousPreference
    }
    $output = ($lines | ForEach-Object { $_.ToString() }) -join "`n"
    Write-Host $output
    return [pscustomobject]@{ ExitCode = $exitCode; Output = $output }
}

function Read-Utf8Source {
    param([string]$Path)
    return [System.IO.File]::ReadAllText($Path, [System.Text.UTF8Encoding]::new($false, $true))
}

function Assert-SharedInclude {
    param([string]$Path)
    $source = Read-Utf8Source $Path
    $includes = [regex]::Matches($source, 'include!\s*\(\s*"([^"]+)"\s*\)\s*;')
    if ($includes.Count -ne 1 -or $includes[0].Groups[1].Value -cne $expectedInclude) {
        throw "Driver must include the installed core module exactly once: $Path"
    }
}

function Get-VerificationSummary {
    param([string]$Output)
    $summaries = [regex]::Matches(
        $Output,
        '(?m)^verification results::\s*(\d+)\s+verified,\s*(\d+)\s+errors\s*$'
    )
    if ($summaries.Count -ne 1) {
        throw 'Expected exactly one verification summary; compilation/startup failure is not a proof result.'
    }
    return [pscustomobject]@{
        Verified = [int]$summaries[0].Groups[1].Value
        Errors = [int]$summaries[0].Groups[2].Value
    }
}

foreach ($path in @($sharedModule, $positiveDriver, $negativeDriver, $productToolchain)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Formal integration is not installed: $path"
    }
}
Assert-SharedInclude $positiveDriver
Assert-SharedInclude $negativeDriver
$toolchainSource = Read-Utf8Source $productToolchain
if ($toolchainSource -notmatch '(?m)^channel\s*=\s*"1\.97\.0"\s*$') {
    throw 'Product toolchain must remain pinned to Rust 1.97.0.'
}
$sourceHashes = @{}
foreach ($path in @($sharedModule, $positiveDriver, $negativeDriver, $productToolchain)) {
    $sourceHashes[$path] = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
}
$negativeLines = @(Get-Content -LiteralPath $negativeDriver -Encoding UTF8)
$assertionLines = @(
    for ($index = 0; $index -lt $negativeLines.Count; $index++) {
        if ($negativeLines[$index] -match '^\s*assert\(still_present\);\s*$') {
            $index + 1
        }
    }
)
if ($assertionLines.Count -ne 1) {
    throw 'The negative driver must contain exactly one designated assert(still_present).'
}

$toolRoot = Join-Path $repository "target/verus-proof-tools/$verifierVersion"
[void](New-Item -ItemType Directory -Path $toolRoot -Force)
$archive = Join-Path $toolRoot "verus-$verifierVersion-x86-win.zip"
if (-not (Test-Path -LiteralPath $archive -PathType Leaf)) {
    $releaseTag = [Uri]::EscapeDataString("release/$verifierVersion")
    $archiveUrl = "https://github.com/verus-lang/verus/releases/download/$releaseTag/verus-$verifierVersion-x86-win.zip"
    Invoke-WebRequest -UseBasicParsing -Uri $archiveUrl -OutFile $archive
}
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -cne $archiveDigest) {
    throw 'Official Verus archive SHA256 mismatch; the archive is not extracted or executed.'
}
Expand-Archive -LiteralPath $archive -DestinationPath $toolRoot -Force
$verifier = Join-Path $toolRoot 'verus-x86-win/verus.exe'
if (-not (Test-Path -LiteralPath $verifier -PathType Leaf)) {
    throw 'Verified archive did not provide the expected Windows executable.'
}
$installation = Invoke-NativeCapture 'rustup' @(
    'toolchain', 'install', $proofToolchain, '--profile', 'minimal', '--no-self-update'
)
if ($installation.ExitCode -ne 0) {
    throw 'Dedicated proof toolchain installation failed.'
}
$compiler = Invoke-NativeCapture 'rustup' @('run', $proofToolchain, 'rustc', '--version')
if ($compiler.ExitCode -ne 0 -or $compiler.Output -notmatch '(?m)^rustc 1\.98\.1\s') {
    throw 'Dedicated proof compiler does not match Rust 1.98.1.'
}
$version = Invoke-NativeCapture $verifier @('--version')
$escapedVersion = [regex]::Escape($verifierVersion)
if ($version.ExitCode -ne 0 -or $version.Output -notmatch "(?<![A-Za-z0-9.])$escapedVersion(?![A-Za-z0-9.])") {
    throw 'Verifier executable does not match the pinned release.'
}
$commonArguments = @(
    '--crate-type', 'lib', '--edition=2024', '--no-cheating', '--num-threads', '1', '--color=never'
)
Push-Location -LiteralPath $repository
try {
    $positive = Invoke-NativeCapture $verifier (@($positiveDriver) + $commonArguments)
    $positiveSummary = Get-VerificationSummary $positive.Output
    if ($positive.ExitCode -ne 0 -or $positiveSummary.Verified -le 0 -or $positiveSummary.Errors -ne 0) {
        throw 'Positive proof requires exit 0, at least one verified obligation, and zero errors.'
    }
    $negative = Invoke-NativeCapture $verifier (@($negativeDriver) + $commonArguments)
    $negativeSummary = Get-VerificationSummary $negative.Output
    $negativeName = [regex]::Escape([System.IO.Path]::GetFileName($negativeDriver))
    $expectedLocation = "(?m)^\s*-->\s+[^`r`n]*$negativeName`:$($assertionLines[0]):\d+\s*$"
    if ($negative.ExitCode -eq 0 -or $negativeSummary.Verified -le 0 -or $negativeSummary.Errors -ne 1 -or
        $negative.Output -notmatch '(?m)^error: assertion failed\s*$' -or
        $negative.Output -notmatch $expectedLocation) {
        throw 'Negative check must fail verification only at the designated removal assertion.'
    }
}
finally {
    Pop-Location
}
foreach ($path in $sourceHashes.Keys) {
    if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -cne $sourceHashes[$path]) {
        throw "Proof input or product toolchain changed during execution: $path"
    }
}
Write-Host "PauseReasons: positive $($positiveSummary.Verified) verified; designated negative assertion rejected."
exit 0
