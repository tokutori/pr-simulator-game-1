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

$negativeDriver = Join-Path $repository 'proofs/rejections.rs'
$negativeAssertions = @('initial_running', 'changed_on_failure', 'changed_at_contact')
$proofInputs = @(
    $productToolchain, $negativeDriver, $PSCommandPath,
    (Join-Path $repository 'Cargo.lock'),
    (Join-Path $repository 'crates/birdman-game-core/Cargo.toml')
) + @(Get-ChildItem -LiteralPath (Join-Path $repository 'crates/birdman-game-core/src') -Recurse -Filter '*.rs' -File).FullName
$sourceHashes = @{}
foreach ($path in $proofInputs) {
    $sourceHashes[$path] = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
}
if ((Read-Utf8Source $productToolchain) -notmatch '(?m)^channel\s*=\s*"1\.97\.0"\s*$') {
    throw 'Product toolchain must remain pinned to Rust 1.97.0.'
}
$negativeLines = (Read-Utf8Source $negativeDriver).Split("`n")
$assertionLines = @{}
foreach ($assertion in $negativeAssertions) {
    $locations = @(for ($index = 0; $index -lt $negativeLines.Count; $index++) {
        if ($negativeLines[$index].Trim() -ceq "assert($assertion);") { $index + 1 }
    })
    if ($locations.Count -ne 1) { throw "Expected one designated assert($assertion) in $negativeDriver" }
    $assertionLines[$assertion] = $locations[0]
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
$coreTarget = [System.IO.Path]::GetFullPath((Join-Path $repository 'target/direct-core-proof'))
if (-not $coreTarget.StartsWith($repository + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw 'Dedicated core proof target must remain inside this worktree.'
}
$oldPath = $env:PATH
$oldToolchain = $env:RUSTUP_TOOLCHAIN
$oldBuildJobs = $env:CARGO_BUILD_JOBS
$timer = [System.Diagnostics.Stopwatch]::StartNew()
Push-Location -LiteralPath $repository
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
    if ($coreSummary.success -ne $true -or $coreSummary.'is-verifying-entire-crate' -ne $true -or
        $coreSummary.Verified -le 0 -or $coreSummary.Errors -ne 0) {
        throw 'Production core requires entire-crate success, verified obligations, and zero errors.'
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
    $negativeArguments = @(
        'run', $proofToolchain, $rustVerifier, $negativeDriver,
        '--crate-name', 'rejections', '--crate-type', 'lib', '--edition=2024',
        '--no-cheating', '--num-threads', '1', '--color=never', '--no-vstd', '-L', "dependency=$dependencies",
        '--extern', "birdman_game_core=$($artifacts.core)",
        '--import', "birdman_game_core=$([System.IO.Path]::ChangeExtension($artifacts.core, 'vir'))",
        '--extern', "vstd=$($artifacts.vstd)",
        '--import', "vstd=$([System.IO.Path]::ChangeExtension($artifacts.vstd, 'vir'))",
        '--extern', "verus_builtin=$($artifacts.builtin)",
        '--extern', "verus_builtin_macros=$($artifacts.macros)"
    )
    $negative = Invoke-NativeCapture 'rustup' $negativeArguments
    $summaries = [regex]::Matches($negative.Output, '(?m)^verification results::\s*(\d+)\s+verified,\s*(\d+)\s+errors\s*$')
    $diagnostics = [regex]::Matches($negative.Output, '(?m)^error: assertion failed\s*$')
    if ($negative.ExitCode -eq 0 -or $summaries.Count -ne 1 -or
        [int]$summaries[0].Groups[1].Value -le 0 -or
        [int]$summaries[0].Groups[2].Value -ne $negativeAssertions.Count -or
        $diagnostics.Count -ne $negativeAssertions.Count) {
        throw 'Negative control must reject only the designated assertions; compilation/startup failure is not proof rejection.'
    }
    foreach ($assertion in $negativeAssertions) {
        $line = $assertionLines[$assertion]
        if ($negative.Output -notmatch "(?m)^\s*-->\s+[^`r`n]*rejections\.rs`:$line`:\d+\s*$") {
            throw "Missing intended verification failure at assert($assertion), line $line."
        }
    }
    Write-Host "Production core: $($coreSummary.Verified) verified; consolidated negative assertions rejected."
}
finally {
    $env:PATH = $oldPath
    $env:RUSTUP_TOOLCHAIN = $oldToolchain
    $env:CARGO_BUILD_JOBS = $oldBuildJobs
    Pop-Location
    foreach ($path in $sourceHashes.Keys) {
        if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -cne $sourceHashes[$path]) {
            throw "Proof input or product toolchain changed during execution: $path"
        }
    }
}
$timer.Stop()
Write-Host "Verus completed: one production-crate verification and one negative-control invocation in $($timer.Elapsed.TotalSeconds.ToString('F2', [System.Globalization.CultureInfo]::InvariantCulture)) seconds."
exit 0
