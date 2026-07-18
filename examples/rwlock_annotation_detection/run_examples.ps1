param(
    [string]$Filter,
    [switch]$ShowOutput,
    [switch]$SummaryOnly,
    [switch]$KnownLimitations
)

$ErrorActionPreference = "Stop"
if ($KnownLimitations) {
    $SummaryOnly = $true
}

$expectations = [ordered]@{
    "alias_same_instance"          = "write requires no live readers"
    "arc_alias_double_write"       = "write requires no live writer"
    "arc_clone_or_fresh_false"     = $null
    "arc_clone_or_fresh_true"      = "write requires no live writer"
    "arc_clone_wrapper_double_write" = "write requires no live writer"
    "arc_instances_independent"    = $null
    "array_indices"                = $null
    "array_same_index"             = "write requires no live readers"
    "callback_clean"               = $null
    "callback_conditional_false"   = $null
    "callback_conditional_true"    = "conditional callback requires no live writer"
    "callback_fnptr"               = "write requires no live writer"
    "callback_fnptr_specialization_clean" = $null
    "callback_fnptr_specialization_violation" = "read requires no live writer"
    "callback_generic_fnonce_clean" = $null
    "callback_generic_fnonce_violation" = "read requires no live writer"
    "callback_hof_annotated"       = "write requires no live writer"
    "callback_hof_direct_control"  = "annotated callback requires no live writer"
    "callback_hof_invoke_twice"    = "annotated callback requires no live writer"
    "callback_inline_closure"      = "write requires no live writer"
    "callback_loop_clean"          = $null
    "callback_loop_violation"      = "read requires no live writer"
    "callback_reentrant"           = "write requires no live writer"
    "callback_sequential_counter_clean" = $null
    "callback_sequential_counter_violation" = "second callback incorrectly requires two readers"
    "callback_specialization_clean" = $null
    "callback_specialization_violation" = "read requires no live writer"
    "callback_unresolvable"         = "callback invocation could not be resolved"
    "clean"                        = $null
    "double_write"                 = "write requires no live writer"
    "drop_then_release"            = "read release requires a live reader"
    "instance_violation"           = "write requires no live writer"
    "instances_independent"        = $null
    "nested_deep_double_write"     = "write requires no live writer"
    "nested_deep_independent"      = $null
    "nested_field_double_write"    = "write requires no live writer"
    "nested_field_independent"     = $null
    "read_then_write"              = "write requires no live readers"
    "rc_alias_double_write"        = "write requires no live writer"
    "rc_instances_independent"     = $null
    "seeded_writer_other_instance" = $null
    "struct_field_double_write"    = "write requires no live writer"
    "struct_fields_independent"    = $null
    "two_readers_drop_both"        = $null
    "two_readers_drop_one"         = "write requires no live readers"
    "unheld_release"               = "read release requires a live reader"
    "write_drop_then_reacquire"    = $null
    "write_then_read"              = "read requires no live writer"
}

$summaryOnlyBins = @(
    "callback_clean",
    "callback_conditional_false",
    "callback_conditional_true",
    "callback_fnptr_specialization_clean",
    "callback_fnptr_specialization_violation",
    "callback_generic_fnonce_clean",
    "callback_generic_fnonce_violation",
    "callback_hof_annotated",
    "callback_hof_invoke_twice",
    "callback_loop_clean",
    "callback_loop_violation",
    "callback_sequential_counter_clean",
    "callback_sequential_counter_violation",
    "callback_specialization_clean",
    "callback_specialization_violation",
    "callback_unresolvable",
    "two_readers_drop_both",
    "two_readers_drop_one",
    "write_drop_then_reacquire",
    "write_then_read"
)

$resolvedKnownLimitationBins = @(
    "callback_fnptr_specialization_violation"
)

$knownLimitationBins = @()

$summaryOnlyOnlyBins = @(
    "callback_loop_violation",
    "callback_unresolvable"
)

$summaryOnlyHofBins = @(
    "callback_clean",
    "callback_conditional_false",
    "callback_conditional_true",
    "callback_fnptr_specialization_clean",
    "callback_fnptr_specialization_violation",
    "callback_generic_fnonce_clean",
    "callback_generic_fnonce_violation",
    "callback_hof_annotated",
    "callback_hof_invoke_twice",
    "callback_loop_clean",
    "callback_loop_violation",
    "callback_sequential_counter_clean",
    "callback_sequential_counter_violation",
    "callback_specialization_clean",
    "callback_specialization_violation",
    "callback_unresolvable"
)

$verifyBins = @(
    "callback_unresolvable"
)

$scriptRoot = $PSScriptRoot
$repositoryRoot = (Resolve-Path (Join-Path $scriptRoot "..\..")).Path
$manifestPath = Join-Path $scriptRoot "Cargo.toml"
$miraiPath = Join-Path $repositoryRoot "target\debug\mirai.exe"
$sweepTarget = Join-Path $repositoryRoot "target\rwlock-example-sweep"
$summarySweepTarget = Join-Path $repositoryRoot "target\rwlock-summary-sweep"
$originalLocation = Get-Location
$originalPath = $env:PATH
$originalWrapper = $env:RUSTC_WORKSPACE_WRAPPER
$originalStartFresh = $env:MIRAI_START_FRESH
$originalSharePersistentStore = $env:MIRAI_SHARE_PERSISTENT_STORE
$originalFlags = $env:MIRAI_FLAGS
$originalLog = $env:MIRAI_LOG
$originalTargetDir = $env:CARGO_TARGET_DIR
$originalBuildJobs = $env:CARGO_BUILD_JOBS
$originalIncremental = $env:CARGO_INCREMENTAL

try {
    Set-Location $repositoryRoot

    if (-not (Test-Path $miraiPath)) {
        Write-Host "Building MIRAI..."
        & cargo build --package mirai --bin mirai --no-default-features
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to build MIRAI (exit $LASTEXITCODE)."
        }
    }

    $sysroot = (& rustc --print sysroot).Trim()
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to locate the Rust sysroot (exit $LASTEXITCODE)."
    }

    $env:PATH = "$(Join-Path $sysroot "bin");$originalPath"
    $env:RUSTC_WORKSPACE_WRAPPER = (Resolve-Path $miraiPath).Path
    # MIRAI_START_FRESH recreates the shared summary directory, so wrapped rustc jobs must serialize.
    $env:CARGO_BUILD_JOBS = "1"
    # Each sweep starts from an empty target; incremental state only adds Windows cleanup races.
    $env:CARGO_INCREMENTAL = "0"

    if ($SummaryOnly) {
        $env:CARGO_TARGET_DIR = $summarySweepTarget
        $env:MIRAI_SHARE_PERSISTENT_STORE = "true"
        $env:MIRAI_START_FRESH = "true"
        $env:MIRAI_FLAGS = "--print_summaries"
        $env:MIRAI_LOG = $null
        if (Test-Path $summarySweepTarget) {
            Remove-Item -Recurse -Force $summarySweepTarget
        }
        New-Item -ItemType Directory -Path $summarySweepTarget -Force | Out-Null
        [System.IO.File]::WriteAllText(
            (Join-Path $summarySweepTarget "CACHEDIR.TAG"),
            "Signature: 8a477f597d28d172789f06886806bc55`n# This file is a cache directory tag created by cargo.`n# For information about cache directory tags see https://bford.info/cachedir/`n"
        )
        $providerOutput = @(
            & cargo check -q --locked --manifest-path $manifestPath --lib 2>&1
        )
        if ($LASTEXITCODE -ne 0) {
            $providerOutput | ForEach-Object { Write-Host $_ }
            throw "Failed to seed provider summaries (exit $LASTEXITCODE)."
        }
        $env:MIRAI_START_FRESH = $null
        $env:MIRAI_FLAGS = $null
        foreach ($bin in $summaryOnlyHofBins) {
            $providerOutput = @(
                & cargo check -q --locked --manifest-path $manifestPath --bin $bin 2>&1
            )
            if ($LASTEXITCODE -ne 0) {
                $providerOutput | ForEach-Object { Write-Host $_ }
                throw "Failed to seed $bin summaries (exit $LASTEXITCODE)."
            }
        }
        & cargo clean --quiet --manifest-path $manifestPath --target-dir $summarySweepTarget -p rwlock-annotation-detection
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to remove provider artifacts (exit $LASTEXITCODE)."
        }
        & cargo check -q --locked --manifest-path $manifestPath --lib
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to restore provider artifacts (exit $LASTEXITCODE)."
        }
    } else {
        $env:CARGO_TARGET_DIR = $sweepTarget
        if (Test-Path $sweepTarget) {
            Remove-Item -Recurse -Force $sweepTarget
        }
    }

    $bins = @(
        $expectations.Keys |
            Where-Object { $_ -notin $knownLimitationBins -and $_ -notin $summaryOnlyOnlyBins }
    )
    if ($Filter) {
        if (-not $expectations.Contains($Filter)) {
            Write-Error "Unknown bin '$Filter'. Available bins: $($bins -join ', ')"
            exit 2
        }
        $bins = @($Filter)
    } elseif ($KnownLimitations) {
        $bins = $knownLimitationBins + $resolvedKnownLimitationBins
    } elseif ($SummaryOnly) {
        $bins = $summaryOnlyBins
    }
    $supportedSummaryBins = $summaryOnlyBins + $knownLimitationBins + $resolvedKnownLimitationBins
    if ($SummaryOnly -and @($bins | Where-Object { $_ -notin $supportedSummaryBins }).Count -gt 0) {
        Write-Error "-SummaryOnly supports: $($supportedSummaryBins -join ', ')"
        exit 2
    }

    $cargoPath = (Get-Command cargo -CommandType Application | Select-Object -First 1).Source
    $passed = 0
    foreach ($bin in $bins) {
        $env:MIRAI_FLAGS = if ($bin -in $verifyBins) { "--diag verify" } else { $null }
        if ($SummaryOnly) {
            $env:MIRAI_START_FRESH = $null
            $env:MIRAI_SHARE_PERSISTENT_STORE = "true"
            $env:MIRAI_LOG = "mirai::summaries=trace,mirai::body_visitor=debug"
        } elseif ($bin -like "arc_*" -or $bin -like "rc_*") {
            $env:MIRAI_START_FRESH = $null
        } else {
            $env:MIRAI_START_FRESH = "true"
        }

        $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
        $startInfo.FileName = $cargoPath
        $startInfo.Arguments = "check -q --locked --manifest-path `"$manifestPath`" --bin `"$bin`""
        $startInfo.WorkingDirectory = $repositoryRoot
        $startInfo.UseShellExecute = $false
        $startInfo.RedirectStandardOutput = $true
        $startInfo.RedirectStandardError = $true

        $process = [System.Diagnostics.Process]::new()
        $process.StartInfo = $startInfo
        [void]$process.Start()
        $stdoutRead = $process.StandardOutput.ReadToEndAsync()
        $stderrRead = $process.StandardError.ReadToEndAsync()
        $process.WaitForExit()

        $rawOutput = ($stdoutRead.Result + $stderrRead.Result).TrimEnd([char[]]"`r`n")
        $output = if ($rawOutput.Length -eq 0) {
            @()
        } else {
            @($rawOutput -split "\r?\n")
        }
        $exitCode = $process.ExitCode
        $process.Dispose()
        $miraiDiagnostics = @($output | Where-Object { $_ -match "\[MIRAI\]" })
        $expected = $expectations[$bin]

        if ($null -eq $expected) {
            $passedExpectation = $exitCode -eq 0 -and $miraiDiagnostics.Count -eq 0
            $expectedText = "silent"
        } else {
            $matchingDiagnostics = @(
                $miraiDiagnostics | Where-Object { $_ -like "*$expected*" }
            )
            $passedExpectation = $exitCode -eq 0 -and
                $miraiDiagnostics.Count -eq 1 -and
                $matchingDiagnostics.Count -eq 1
            $expectedText = $expected
        }
        $providerBodyEntries = @(
            $output | Where-Object {
                $_ -match "entered body of .*rwlock_annotation_detection.*::(read|write|release_read|drop)" -or
                $_ -match "entered body of .*callback_(clean|conditional|fnptr_specialization|generic_fnonce|hof_annotated|hof_invoke_twice|loop|sequential_counter|specialization).*::(invoke|invoke_if|invoke_generic|invoke_in_loop|invoke_twice|with_write_held|annotated_acquire|acquire_once|increment|require|read|\{closure)"
            }
        )
        $persistentSummaryLoads = @(
            $output | Where-Object {
                $_ -match "get_persistent_summary_for_db\(\) => Some\(Summary"
            }
        )
        if ($SummaryOnly) {
            $passedExpectation = $passedExpectation -and
                $providerBodyEntries.Count -eq 0 -and
                $persistentSummaryLoads.Count -gt 0
        }

        $actualText = if ($miraiDiagnostics.Count -eq 0) {
            "silent"
        } elseif ($miraiDiagnostics.Count -eq 1) {
            $miraiDiagnostics[0] -replace "^warning: \[MIRAI\] ", ""
        } else {
            "$($miraiDiagnostics.Count) MIRAI diagnostics"
        }
        if ($SummaryOnly) {
            $actualText += "; persistent loads: $($persistentSummaryLoads.Count); provider body entries: $($providerBodyEntries.Count)"
        }
        if ($bin -in $verifyBins) {
            $actualText += "; diag: verify"
        }

        if ($ShowOutput) {
            Write-Host ""
            Write-Host "=== $bin ==="
            Write-Host "Expected: $expectedText"
            Write-Host "Raw stdout/stderr:"
            if ($output.Count -gt 0) {
                $output | ForEach-Object { Write-Host $_ }
            } else {
                Write-Host "<no output>"
            }
            Write-Host "Result: $(if ($passedExpectation) { 'PASS' } else { 'FAIL' })"
        } elseif ($passedExpectation) {
            Write-Host "✅ $bin — expected: $expectedText; actual: $actualText"
        } else {
            Write-Host "❌ $bin — expected: $expectedText; actual: $actualText; exit: $exitCode"
            if ($output.Count -gt 0) {
                $output | ForEach-Object { Write-Host $_ }
            } else {
                Write-Host "<no output>"
            }
        }

        if ($passedExpectation) {
            $passed++
        }
    }

    Write-Host ""
    Write-Host "$passed/$($bins.Count) passed"
    if ($passed -ne $bins.Count) {
        exit 1
    }
} finally {
    $env:PATH = $originalPath
    $env:RUSTC_WORKSPACE_WRAPPER = $originalWrapper
    $env:MIRAI_START_FRESH = $originalStartFresh
    $env:MIRAI_SHARE_PERSISTENT_STORE = $originalSharePersistentStore
    $env:MIRAI_FLAGS = $originalFlags
    $env:MIRAI_LOG = $originalLog
    $env:CARGO_TARGET_DIR = $originalTargetDir
    $env:CARGO_BUILD_JOBS = $originalBuildJobs
    $env:CARGO_INCREMENTAL = $originalIncremental
    Set-Location $originalLocation
}
