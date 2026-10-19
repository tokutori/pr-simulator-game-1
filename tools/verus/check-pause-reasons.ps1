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
        RequiredProofs = @('verify_result_replay_roundtrip', 'verify_enter_replay', 'verify_leave_replay', 'verify_running_admission')
        RenamedProof = 'verify_result_replay_roundtrip'
        NegativeChecks = @(
            @{ Path = Join-Path $repository 'proofs/configured_phase_rejection_negative.rs'; Assertion = 'admitted' },
            @{ Path = Join-Path $repository 'proofs/configured_phase_payload_negative.rs'; Assertion = 'changed' },
            @{ Path = Join-Path $repository 'proofs/configured_phase_running_negative.rs'; Assertion = 'admitted' }
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
    },
    @{
        Name = 'flight_record_storage'
        SharedModule = Join-Path $repository 'crates/birdman-game-core/src/flight_record_storage.rs'
        PositiveDriver = Join-Path $repository 'proofs/flight_record_storage.rs'
        RequiredProofs = @('verify_record_push', 'verify_record_finalize', 'verify_record_validation_failure', 'verify_record_finalize_rejection')
        RenamedProof = 'verify_record_push'
        NegativeChecks = @(
            @{ Path = Join-Path $repository 'proofs/flight_record_storage_capacity_negative.rs'; Assertion = 'appended' },
            @{ Path = Join-Path $repository 'proofs/flight_record_storage_finalize_negative.rs'; Assertion = 'changed' }
        )
    }
)
$domainDriver = Join-Path $repository 'proofs/flight_record_finalization.rs'
$domainProofs = @(
    'verify_domain_session_initial_projection',
    'verify_domain_session_countdown_rejection',
    'verify_domain_archived_record_playback', 'verify_domain_result_record_transfer',
    'verify_domain_demo_record_installation',
    'verify_domain_finalization_retention', 'verify_domain_finalization_metadata', 'verify_domain_finalization_commit',
    'verify_domain_record_begin', 'verify_domain_record_append', 'verify_domain_record_no_elapsed',
    'verify_domain_no_elapsed_terminal_tick', 'verify_domain_contact_terminal_payload'
)
$domainNegatives = @(
    @{ Path = Join-Path $repository 'proofs/game_session_initial_running_negative.rs'; Assertion = 'admitted' },
    @{ Path = Join-Path $repository 'proofs/game_session_countdown_admission_negative.rs'; Assertion = 'admitted' },
    @{ Path = Join-Path $repository 'proofs/game_session_archived_record_admission_negative.rs'; Assertion = 'admitted' },
    @{ Path = Join-Path $repository 'proofs/game_session_archived_record_retention_negative.rs'; Assertion = 'changed' },
    @{ Path = Join-Path $repository 'proofs/flight_record_finalization_retention_negative.rs'; Assertion = 'changed' },
    @{ Path = Join-Path $repository 'proofs/flight_record_finalization_metadata_negative.rs'; Assertion = 'discarded' },
    @{ Path = Join-Path $repository 'proofs/flight_record_finalization_commit_negative.rs'; Assertion = 'missing' },
    @{ Path = Join-Path $repository 'proofs/flight_record_append_admission_negative.rs'; Assertion = 'appended' },
    @{ Path = Join-Path $repository 'proofs/flight_record_no_elapsed_negative.rs'; Assertion = 'added' },
    @{ Path = Join-Path $repository 'proofs/flight_record_no_elapsed_payload_negative.rs'; Assertion = 'changed' },
    @{ Path = Join-Path $repository 'proofs/flight_record_latest_payload_negative.rs'; Assertion = 'mismatched' },
    @{ Path = Join-Path $repository 'proofs/flight_record_contact_payload_negative.rs'; Assertion = 'changed' },
    @{ Path = Join-Path $repository 'proofs/flight_record_terminal_tick_negative.rs'; Assertion = 'shifted' }
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
        $qualifiedName = if ($requiredProof.Contains('::')) { $requiredProof } else { "${Module}::${Module}::$requiredProof" }
        if ($resolvedNames -cnotcontains $qualifiedName) {
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
$proofInputs += @($domainDriver) + @($domainNegatives.Path) + @(
    (Join-Path $repository 'Cargo.lock'),
    (Join-Path $repository 'crates/birdman-game-core/Cargo.toml'),
    $PSCommandPath
)
$proofInputs += @(Get-ChildItem -LiteralPath (Join-Path $repository 'crates/birdman-game-core/src') -Recurse -Filter '*.rs' -File).FullName
$proofInputs = @($proofInputs | Select-Object -Unique)
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
foreach ($check in (@($suites.NegativeChecks) + $domainNegatives)) {
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
    $renamedSource = $renamedSource.Replace("fn $renamedProof<", "fn $($renamedProof)_retired<")
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

  $coreTarget = [System.IO.Path]::GetFullPath((Join-Path $repository 'target/direct-core-proof'))
  if (-not $coreTarget.StartsWith($repository + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
      throw 'Dedicated core proof target must remain inside this worktree.'
  }
  $oldPath = $env:PATH
  $oldToolchain = $env:RUSTUP_TOOLCHAIN
  $oldBuildJobs = $env:CARGO_BUILD_JOBS
  try {
      $binaryDirectory = Split-Path -Parent $verifier
      $cargoVerifier = Join-Path $binaryDirectory 'cargo-verus.exe'
      $rustVerifier = Join-Path $binaryDirectory 'rust_verify.exe'
      $env:PATH = "$binaryDirectory;$oldPath"
      $env:RUSTUP_TOOLCHAIN = $proofToolchain
      $env:CARGO_BUILD_JOBS = '1'
      $clean = Invoke-NativeCapture 'cargo' @('clean', '-p', 'birdman-game-core', '--target-dir', $coreTarget)
      if ($clean.ExitCode -ne 0) { throw 'Dedicated core proof artifacts could not be refreshed.' }
      $coreProof = Invoke-NativeCapture $cargoVerifier @(
          'verify', '-p', 'birdman-game-core', '--features', 'verus-proof', '--locked',
          '--target-dir', $coreTarget, '--check-toolchain', '--fwd-verus-args-to', 'roots', '--',
          '--no-cheating', '--num-threads', '1', '--output-json', '--time'
      ) -JsonOutput
      if ($coreProof.ExitCode -ne 0) { throw 'Production core crate verification failed.' }
      $jsonStart = $coreProof.Output.IndexOf('{')
      if ($jsonStart -lt 0) { throw 'The core verification report is missing.' }
      $coreReport = $coreProof.Output.Substring($jsonStart) | ConvertFrom-Json
      $coreSummary = $coreReport.'verification-results'
      $coreSymbols = @(
          'birdman_game_core::game_session::GameSession::new',
          'birdman_game_core::game_session::GameSessionError::from_flight_execution',
          'birdman_game_core::game_session::GameSession::snapshot',
          'birdman_game_core::game_session::GameSession::phase',
          'birdman_game_core::game_session::GameSession::configured',
          'birdman_game_core::game_session::GameSession::configured_mut',
          'birdman_game_core::game_session::GameSession::flight_record',
          'birdman_game_core::game_session::GameSession::playback_record',
          'birdman_game_core::game_session::GameSession::install_attract_record',
          'birdman_game_core::game_session::GameSession::leave_attract',
          'birdman_game_core::game_session::GameSession::take_finalized_result_record',
          'birdman_game_core::game_session::GameSession::open_archived_replay',
          'birdman_game_core::game_session::GameSession::can_resume',
          'birdman_game_core::game_session::GameSession::pause',
          'birdman_game_core::game_session::GameSession::clear_pause_reason',
          'birdman_game_core::game_session::GameSession::resume',
          'birdman_game_core::game_session::GameSession::enter_replay',
          'birdman_game_core::game_session::GameSession::leave_replay',
          'birdman_game_core::game_session::GameSession::advance_tail_flight_tick_with',
          'birdman_game_core::game_session::GameSession::open_setup',
          'birdman_game_core::game_session::GameSession::return_to_title',
          'birdman_game_core::game_session::GameSession::prepare_flight_with_record_factory',
          'birdman_game_core::game_session::GameSession::cancel_briefing',
          'birdman_game_core::game_session::GameSession::mark_briefing_ready',
          'birdman_game_core::game_session::GameSession::fail_briefing',
          'birdman_game_core::game_session::GameSession::retry_briefing_with_record_factory',
          'birdman_game_core::game_session::GameSession::start_countdown',
          'birdman_game_core::game_session::GameSession::countdown_token',
          'birdman_game_core::game_session::GameSession::observe_countdown_with',
          'birdman_game_core::game_session::GameSession::cancel_countdown',
          'birdman_game_core::game_session::GameSession::retry_with_record_factory',
          'birdman_game_core::game_session::lifecycle::create_configured_record',
          'birdman_game_core::game_session::lifecycle::verify_game_session_configured_lifecycle',
          'birdman_game_core::game_session::lifecycle::verify_session_archived_record_ownership',
          'birdman_game_core::game_session::lifecycle::verify_session_result_record_transfer',
          'birdman_game_core::game_session::lifecycle::verify_session_demo_record_ownership',
          'birdman_game_core::flight_record::FlightRecordHeader::try_new',
          'birdman_game_core::flight_record::FlightRecordHeader::is_valid',
          'birdman_game_core::game_session::running::verify_game_session_tick_pause_projection',
          'birdman_game_core::game_session::running::verify_evaluated_report_session_payload',
          'birdman_game_core::flight_record::FlightRecord::finalize',
          'birdman_game_core::flight_record::FlightRecord::finalize_with_failure',
          'birdman_game_core::flight_record::FlightRecord::finalize_record',
          'birdman_game_core::flight_record::validate_finalization',
          'birdman_game_core::flight_record::storage::FlightRecordStorage::finalize_with',
          'birdman_game_core::configured_session::ConfiguredSession::finalize_terminal',
          'birdman_game_core::configured_session::ConfiguredSession::advance_tail_flight_tick',
          'birdman_game_core::configured_phase::ConfiguredPhase::running_state',
          'birdman_game_core::tail_simulation::TailFlightTickError::end_reason',
          'birdman_game_core::aerodynamics_contract::AerodynamicEvaluationError::cause',
          'birdman_game_core::aerodynamics_contract::HybridError::cause',
          'birdman_game_core::game_session::SessionTerminalState::record_time',
          'birdman_game_core::game_session::SessionTerminalState::flight_state',
          'birdman_game_core::flight_record::FlightRecord::finalization',
          'birdman_game_core::flight_record::FlightRecord::finalized_from',
          'birdman_game_core::flight_record::FlightRecord::begin_tail',
          'birdman_game_core::flight_record::FlightRecord::append_tail_report',
          'birdman_game_core::flight_record::FlightRecord::latest',
          'birdman_game_core::flight_record::FlightRecord::recorded_samples',
          'birdman_game_core::flight_record::FlightRecord::sample_limit',
          'birdman_game_core::flight_record::FlightRecord::latest_matches_tail_state',
          'birdman_game_core::flight_record::control::FlightRecordControls::initial_tail',
          'birdman_game_core::flight_record::control::FlightRecordControls::from_tail_report',
          'birdman_game_core::flight_record::control::FlightRecordTailInput::from_applied',
          'birdman_game_core::tail_simulation::TailFlightTickReport::outcome',
          'birdman_game_core::tail_simulation::TailFlightTickReport::applied_controls',
          'birdman_game_core::tail_simulation::contact_at_start_report',
          'birdman_game_core::tail_simulation::evaluated_tick_report',
          'birdman_game_core::tail_simulation::TailWaterContactSample::contact_point_index',
          'birdman_game_core::tail_simulation::verify_contact_at_start_record_payload',
          'birdman_game_core::tail_simulation::verify_advanced_contact_at_start_record_payload',
          'birdman_game_core::flight_record::validate_telemetry',
          'birdman_game_core::flight_record::storage::FlightRecordStorage::push'
      )
      if ($coreSummary.success -ne $true -or $coreSummary.'is-verifying-entire-crate' -ne $true -or
          $coreSummary.Verified -le 0 -or $coreSummary.Errors -ne 0 -or
          @(Get-MissingProofTargets $coreReport '' $coreSymbols).Count -ne 0) {
          throw 'Production record lifecycle requires whole-crate success and its exact compiler symbols.'
      }
      $dependencies = Join-Path $coreTarget 'debug/deps'
      $artifacts = @{}
      foreach ($entry in @(
          @{ Name = 'core'; Pattern = 'libbirdman_game_core-*.rmeta' },
          @{ Name = 'vstd'; Pattern = 'libvstd-*.rmeta' },
          @{ Name = 'builtin'; Pattern = 'libverus_builtin-*.rmeta' },
          @{ Name = 'macros'; Pattern = 'verus_builtin_macros-*.dll' }
      )) {
          $matches = @(Get-ChildItem -LiteralPath $dependencies -Filter $entry.Pattern -File)
          if ($matches.Count -ne 1) { throw "Ambiguous verified dependency: $($entry.Name)" }
          $artifacts[$entry.Name] = $matches[0].FullName
      }
      $domainArguments = @(
          'run', $proofToolchain, $rustVerifier,
          '--crate-name', 'flight_record_finalization', '--crate-type', 'lib', '--edition=2024',
          '--no-cheating', '--num-threads', '1', '--color=never', '--no-vstd', '-L', "dependency=$dependencies",
          '--extern', "birdman_game_core=$($artifacts.core)",
          '--import', "birdman_game_core=$([System.IO.Path]::ChangeExtension($artifacts.core, 'vir'))",
          '--extern', "vstd=$($artifacts.vstd)",
          '--import', "vstd=$([System.IO.Path]::ChangeExtension($artifacts.vstd, 'vir'))",
          '--extern', "verus_builtin=$($artifacts.builtin)",
          '--extern', "verus_builtin_macros=$($artifacts.macros)"
      )
      $domain = Invoke-NativeCapture 'rustup' ($domainArguments + @($domainDriver, '--output-json', '--time')) -JsonOutput
      $domainReport = $domain.Output | ConvertFrom-Json
      $domainSymbols = @($domainProofs | ForEach-Object { "flight_record_finalization::$_" })
      if ($domain.ExitCode -ne 0 -or $domainReport.'verification-results'.success -ne $true -or
          $domainReport.'verification-results'.'is-verifying-entire-crate' -ne $true -or
          $domainReport.'verification-results'.Verified -le 0 -or $domainReport.'verification-results'.Errors -ne 0 -or
          @(Get-MissingProofTargets $domainReport '' $domainSymbols).Count -ne 0) {
          throw 'Concrete record clients require whole-crate success and their exact compiler symbols.'
      }
      foreach ($name in $domainProofs) {
          $selected = Invoke-NativeCapture 'rustup' ($domainArguments + @($domainDriver, '--verify-root', '--verify-function', $name))
          $summary = Get-VerificationSummary $selected.Output -Selected
          if ($selected.ExitCode -ne 0 -or $summary.Verified -le 0 -or $summary.Errors -ne 0) {
              throw "Required concrete record client did not verify: $name"
          }
      }
      $renamedSource = (Read-Utf8Source $domainDriver).Replace("fn $($domainProofs[0])(", "fn $($domainProofs[0])_retired(")
      if ($renamedSource -ceq (Read-Utf8Source $domainDriver)) { throw 'The concrete client rename fixture did not change its declaration.' }
      $renamedSource += "`n/*`nfn $($domainProofs[0])() {}`n*/`n"
      $renamedPath = Join-Path $coreTarget 'record_finalization_renamed.rs'
      [System.IO.File]::WriteAllText($renamedPath, $renamedSource, [System.Text.UTF8Encoding]::new($false))
      $renamed = Invoke-NativeCapture 'rustup' ($domainArguments + @($renamedPath, '--output-json', '--time')) -JsonOutput
      $renamedReport = $renamed.Output | ConvertFrom-Json
      $missingSymbols = @(Get-MissingProofTargets $renamedReport '' $domainSymbols)
      if ($renamed.ExitCode -ne 0 -or $renamedReport.'verification-results'.success -ne $true -or
          $renamedReport.'verification-results'.'is-verifying-entire-crate' -ne $true -or
          $missingSymbols.Count -ne 1 -or $missingSymbols[0] -cne $domainSymbols[0]) {
          throw 'The concrete renamed client with its old name in a comment must fail compiler inventory.'
      }
      $missing = Invoke-NativeCapture 'rustup' ($domainArguments + @(
          $domainNegatives[0].Path, '--verify-root', '--verify-function', $domainProofs[0]
      ))
      $missingMessage = [regex]::Escape("could not find function $($domainProofs[0]) specified by --verify-function")
      if ($missing.ExitCode -eq 0 -or $missing.Output -notmatch "(?m)^error: $missingMessage\s*$" -or
          $missing.Output -match '(?m)^error: assertion failed\s*$') {
          throw 'A missing concrete client must fail the designated function lookup.'
      }
      foreach ($check in $domainNegatives) {
          $negative = Invoke-NativeCapture 'rustup' ($domainArguments + @($check.Path))
          $summary = Get-VerificationSummary $negative.Output
          $filename = [regex]::Escape([System.IO.Path]::GetFileName($check.Path))
          if ($negative.ExitCode -eq 0 -or $summary.Errors -ne 1 -or
              $negative.Output -notmatch '(?m)^error: assertion failed\s*$' -or
              $negative.Output -notmatch "(?m)^\s*-->\s+[^`r`n]*$filename`:$($check.Line):\d+\s*$") {
              throw "Concrete negative must fail only at the designated assertion: $($check.Path)"
          }
      }
      Write-Host "Production core: $($coreSummary.Verified) verified; concrete clients, exact inventory, lookup and designated negatives completed."
  }
  finally {
      $env:PATH = $oldPath
      $env:RUSTUP_TOOLCHAIN = $oldToolchain
      $env:CARGO_BUILD_JOBS = $oldBuildJobs
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
Write-Host 'Production pause, countdown, flight execution, configured phase, retry owner, and record storage proof suites completed.'
exit 0
