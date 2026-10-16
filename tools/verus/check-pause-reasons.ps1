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
$productToolchain = Join-Path $repository 'rust-toolchain.toml'
$suites = @(
    @{
        Name = 'pause_reasons'
        SharedModule = Join-Path $repository 'crates/birdman-game-core/src/pause_reasons.rs'
        PositiveDriver = Join-Path $repository 'proofs/pause_reasons.rs'
        RequiredProofs = @('verify_set_operations', 'verify_resume_predicate', 'verify_resume_admission', 'verify_empty_set')
        RenamedProof = 'verify_resume_admission'
        NegativeChecks = @(
            @{ Path = Join-Path $repository 'proofs/pause_reasons_negative.rs'; Assertion = 'still_present' },
            @{ Path = Join-Path $repository 'proofs/resume_admission_negative.rs'; Assertion = 'admitted' }
        )
    },
    @{
        Name = 'countdown'
        SharedModule = Join-Path $repository 'crates/birdman-game-core/src/countdown.rs'
        PositiveDriver = Join-Path $repository 'proofs/countdown.rs'
        RequiredProofs = @('verify_countdown_start', 'verify_countdown_admission', 'verify_countdown_retention', 'verify_obsolete_countdown')
        RenamedProof = 'verify_countdown_admission'
        NegativeChecks = @(
            @{ Path = Join-Path $repository 'proofs/countdown_early_launch_negative.rs'; Assertion = 'admitted' },
            @{ Path = Join-Path $repository 'proofs/countdown_stale_token_negative.rs'; Assertion = 'advanced' }
        )
    },
    @{
        Name = 'flight_execution'
        SharedModule = Join-Path $repository 'crates/birdman-game-core/src/flight_execution.rs'
        PositiveDriver = Join-Path $repository 'proofs/flight_execution.rs'
        RequiredProofs = @('verify_pause_execution', 'verify_clear_execution', 'verify_resume_execution', 'verify_running_rejection', 'verify_pause_payload', 'verify_clear_payload', 'verify_resume_payload')
        RenamedProof = 'verify_resume_execution'
        NegativeChecks = @(
            @{ Path = Join-Path $repository 'proofs/flight_execution_resume_negative.rs'; Assertion = 'resumed' },
            @{ Path = Join-Path $repository 'proofs/flight_execution_clear_negative.rs'; Assertion = 'running' },
            @{ Path = Join-Path $repository 'proofs/flight_payload_negative.rs'; Assertion = 'changed' }
        )
    },
    @{
        Name = 'configured_phase'
        SharedModule = Join-Path $repository 'crates/birdman-game-core/src/configured_phase.rs'
        PositiveDriver = Join-Path $repository 'proofs/configured_phase.rs'
        RequiredProofs = @('verify_result_replay_roundtrip', 'verify_enter_replay', 'verify_leave_replay')
        RenamedProof = 'verify_result_replay_roundtrip'
        NegativeChecks = @(
            @{ Path = Join-Path $repository 'proofs/configured_phase_rejection_negative.rs'; Assertion = 'admitted' },
            @{ Path = Join-Path $repository 'proofs/configured_phase_payload_negative.rs'; Assertion = 'changed' }
        )
    },
    @{
        Name = 'configured_session'
        SharedModule = Join-Path $repository 'crates/birdman-game-core/src/configured_session.rs'
        PositiveDriver = Join-Path $repository 'proofs/configured_session.rs'
        RequiredProofs = @('verify_retry_success', 'verify_retry_reservation_failure', 'verify_retry_rejection')
        RenamedProof = 'verify_retry_success'
        NegativeChecks = @(
            @{ Path = Join-Path $repository 'proofs/configured_session_rejection_negative.rs'; Assertion = 'admitted' },
            @{ Path = Join-Path $repository 'proofs/configured_session_retention_negative.rs'; Assertion = 'changed' }
        )
    }
)

function Invoke-NativeCapture {
    param([string]$Command, [string[]]$Arguments, [switch]$JsonOutput)
    $previousPreference = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        if ($JsonOutput) {
            $lines = @(& $Command @Arguments)
        }
        else {
            $lines = @(& $Command @Arguments 2>&1)
        }
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousPreference
    }
    $output = ($lines | ForEach-Object { $_.ToString() }) -join "`n"
    if (-not $JsonOutput -or $exitCode -ne 0) {
        Write-Host $output
    }
    return [pscustomobject]@{ ExitCode = $exitCode; Output = $output }
}

function Read-Utf8Source {
    param([string]$Path)
    return [System.IO.File]::ReadAllText($Path, [System.Text.UTF8Encoding]::new($false, $true))
}

function Assert-SharedInclude {
    param([string]$Path, [string]$ExpectedInclude)
    $source = Read-Utf8Source $Path
    $includes = [regex]::Matches($source, 'include!\s*\(\s*"([^"]+)"\s*\)\s*;')
    if ($includes.Count -ne 1 -or $includes[0].Groups[1].Value -cne $ExpectedInclude) {
        throw "Driver must include the installed core module exactly once: $Path"
    }
}

function Get-VerificationSummary {
    param([string]$Output, [switch]$Selected)
    $suffix = if ($Selected) { [regex]::Escape(' (partial verification with `--verify-*`)') } else { '' }
    $summaries = [regex]::Matches(
        $Output,
        "(?m)^verification results::\s*(\d+)\s+verified,\s*(\d+)\s+errors$suffix\s*$"
    )
    if ($summaries.Count -ne 1) {
        throw 'Expected exactly one verification summary; compilation/startup failure is not a proof result.'
    }
    return [pscustomobject]@{
        Verified = [int]$summaries[0].Groups[1].Value
        Errors = [int]$summaries[0].Groups[2].Value
    }
}

function Get-MissingProofTargets {
    param($Report, [string]$Module, [string[]]$RequiredProofs)
    $resolvedNames = @($Report.'func-details'.PSObject.Properties.Name)
    foreach ($requiredProof in $RequiredProofs) {
        if ($resolvedNames -cnotcontains "${Module}::${Module}::$requiredProof") {
            $requiredProof
        }
    }
}

$proofInputs = @($productToolchain)
foreach ($suite in $suites) {
    $suite.ExpectedInclude = "../crates/birdman-game-core/src/$($suite.Name).rs"
    $proofInputs += @($suite.SharedModule, $suite.PositiveDriver) + @($suite.NegativeChecks.Path)
    Assert-SharedInclude $suite.PositiveDriver $suite.ExpectedInclude
    foreach ($check in $suite.NegativeChecks) {
        Assert-SharedInclude $check.Path $suite.ExpectedInclude
    }
}
foreach ($path in $proofInputs) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Formal integration is not installed: $path"
    }
}
$toolchainSource = Read-Utf8Source $productToolchain
if ($toolchainSource -notmatch '(?m)^channel\s*=\s*"1\.97\.0"\s*$') {
    throw 'Product toolchain must remain pinned to Rust 1.97.0.'
}
$sourceHashes = @{}
foreach ($path in $proofInputs) {
    $sourceHashes[$path] = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
}
foreach ($check in @($suites.NegativeChecks)) {
    $negativeLines = @(Get-Content -LiteralPath $check.Path -Encoding UTF8)
    $escapedAssertion = [regex]::Escape($check.Assertion)
    $assertionLines = @(
        for ($index = 0; $index -lt $negativeLines.Count; $index++) {
            if ($negativeLines[$index] -match "^\s*assert\($escapedAssertion\);\s*$") {
                $index + 1
            }
        }
    )
    if ($assertionLines.Count -ne 1) {
        throw "The negative driver must contain exactly one designated assert($($check.Assertion)): $($check.Path)"
    }
    $check.Line = $assertionLines[0]
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
$extractor = Join-Path $env:SystemRoot 'System32/tar.exe'
if (-not (Test-Path -LiteralPath $extractor -PathType Leaf)) {
    throw 'Windows system tar executable was not found.'
}
$extraction = Invoke-NativeCapture $extractor @('-xf', $archive, '-C', $toolRoot)
if ($extraction.ExitCode -ne 0) {
    throw "Windows tar extraction failed with exit $($extraction.ExitCode)."
}
$verifier = Join-Path $toolRoot 'verus-x86-win/verus.exe'
if (-not (Test-Path -LiteralPath $verifier -PathType Leaf)) {
    throw 'Verified archive did not provide the expected Windows executable.'
}
$toolchains = Invoke-NativeCapture 'rustup' @('toolchain', 'list')
if ($toolchains.ExitCode -ne 0) {
    throw 'Installed proof toolchains could not be inspected.'
}
$escapedToolchain = [regex]::Escape($proofToolchain)
if ($toolchains.Output -notmatch "(?m)^$escapedToolchain(?:-[^\s]+)?(?:\s|$)") {
    $installation = Invoke-NativeCapture 'rustup' @(
        'toolchain', 'install', $proofToolchain, '--profile', 'minimal', '--no-self-update'
    )
    if ($installation.ExitCode -ne 0) {
        throw 'Dedicated proof toolchain installation failed.'
    }
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
$baseArguments = @(
    '--crate-type', 'lib', '--edition=2024',
    '--no-cheating', '--num-threads', '1', '--color=never'
)
Push-Location -LiteralPath $repository
try {
  foreach ($suite in $suites) {
    $sharedModule = $suite.SharedModule
    $positiveDriver = $suite.PositiveDriver
    $negativeDriver = $suite.NegativeChecks[0].Path
    $negativeChecks = $suite.NegativeChecks
    $expectedInclude = $suite.ExpectedInclude
    $requiredProofs = $suite.RequiredProofs
    $commonArguments = $baseArguments + @('--crate-name', $suite.Name)
    $positive = Invoke-NativeCapture $verifier (@($positiveDriver) + $commonArguments + @('--output-json', '--time')) -JsonOutput
    $positiveReport = $positive.Output | ConvertFrom-Json
    $positiveSummary = $positiveReport.'verification-results'
    if ($positive.ExitCode -ne 0 -or $positiveSummary.Verified -le 0 -or $positiveSummary.Errors -ne 0 -or
        $positiveSummary.success -ne $true -or $positiveSummary.'is-verifying-entire-crate' -ne $true) {
        throw 'Positive proof requires exit 0, entire-crate success, verified obligations, and zero errors.'
    }
    $missingTargets = @(Get-MissingProofTargets $positiveReport $suite.Name $requiredProofs)
    if ($missingTargets.Count -ne 0) {
        throw "Required proof symbols are missing from compiler inventory: $($missingTargets -join ', ')"
    }
    Write-Host "$($suite.Name) whole proof: $($positiveSummary.Verified) verified; exact compiler symbols retained."
    foreach ($requiredProof in $requiredProofs) {
        $selected = Invoke-NativeCapture $verifier (@($positiveDriver) + $commonArguments + @(
            '--verify-only-module', $suite.Name, '--verify-function', $requiredProof
        ))
        $selectedSummary = Get-VerificationSummary $selected.Output -Selected
        if ($selected.ExitCode -ne 0 -or $selectedSummary.Verified -le 0 -or $selectedSummary.Errors -ne 0) {
            throw "Required proof did not verify: $requiredProof"
        }
    }
    $renamedProof = $suite.RenamedProof
    $positiveSource = Read-Utf8Source $positiveDriver
    $renamedSource = $positiveSource.Replace("fn $renamedProof(", "fn $($renamedProof)_retired(")
    if ($renamedSource -ceq $positiveSource) {
        throw 'The rename fixture did not remove the designated proof symbol.'
    }
    $renamedSource = $renamedSource.Replace($expectedInclude, $sharedModule.Replace('\', '/'))
    $sharedDirectory = (Join-Path $repository 'crates/birdman-game-core/src/').Replace('\', '/')
    $renamedSource = $renamedSource.Replace('../crates/birdman-game-core/src/', $sharedDirectory)
    $renamedSource += "`n/*`nfn $renamedProof() {}`n*/`n"
    $renamedDriver = Join-Path $toolRoot "$($suite.Name)_renamed.rs"
    [System.IO.File]::WriteAllText($renamedDriver, $renamedSource, [System.Text.UTF8Encoding]::new($false))
    $renamed = Invoke-NativeCapture $verifier (@($renamedDriver) + $commonArguments + @('--output-json', '--time')) -JsonOutput
    $renamedReport = $renamed.Output | ConvertFrom-Json
    $renamedSummary = $renamedReport.'verification-results'
    $renamedMissing = @(Get-MissingProofTargets $renamedReport $suite.Name $requiredProofs)
    if ($renamed.ExitCode -ne 0 -or $renamedSummary.success -ne $true -or
        $renamedSummary.'is-verifying-entire-crate' -ne $true -or
        $renamedSummary.Verified -le 0 -or $renamedSummary.Errors -ne 0 -or
        $renamedMissing.Count -ne 1 -or $renamedMissing[0] -cne $renamedProof) {
        throw 'A valid renamed proof with its old declaration in a comment must fail exact compiler inventory.'
    }
    Write-Host 'Renamed proof with commented old declaration rejected by exact compiler inventory.'
    $missingProof = $requiredProofs[0]
    $missing = Invoke-NativeCapture $verifier (@($negativeDriver) + $commonArguments + @(
        '--verify-only-module', $suite.Name, '--verify-function', $missingProof
    ))
    $missingMessage = [regex]::Escape("could not find function $missingProof specified by --verify-function")
    if ($missing.ExitCode -eq 0 -or $missing.Output -notmatch "(?m)^error: $missingMessage\s*$" -or
        $missing.Output -match '(?m)^error: assertion failed\s*$') {
        throw 'Missing proof must be rejected by the designated function lookup, not by another failure.'
    }
    foreach ($check in $negativeChecks) {
        $negative = Invoke-NativeCapture $verifier (@($check.Path) + $commonArguments)
        $negativeSummary = Get-VerificationSummary $negative.Output
        $negativeName = [regex]::Escape([System.IO.Path]::GetFileName($check.Path))
        $expectedLocation = "(?m)^\s*-->\s+[^`r`n]*$negativeName`:$($check.Line):\d+\s*$"
        if ($negative.ExitCode -eq 0 -or $negativeSummary.Verified -le 0 -or $negativeSummary.Errors -ne 1 -or
            $negative.Output -notmatch '(?m)^error: assertion failed\s*$' -or
            $negative.Output -notmatch $expectedLocation) {
            throw "Negative check must fail verification only at the designated assertion: $($check.Path)"
        }
    }
    Write-Host "$($suite.Name): positive $($positiveSummary.Verified) verified; all required named proofs verified; missing proof and designated negative assertions rejected."
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
Write-Host 'Production pause, countdown, flight execution, configured phase, and retry owner proof suites completed.'
exit 0
