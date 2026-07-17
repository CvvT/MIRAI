param(
    [string]$Filter,
    [switch]$ShowOutput
)

$ErrorActionPreference = "Stop"

$expectations = [ordered]@{
    "alias_same_instance"          = "write requires no live readers"
    "arc_alias_double_write"       = "write requires no live writer"
    "arc_instances_independent"    = $null
    "array_indices"                = $null
    "array_same_index"             = "write requires no live readers"
    "callback_clean"               = $null
    "callback_fnptr"               = "write requires no live writer"
    "callback_hof_annotated"       = "write requires no live writer"
    "callback_hof_direct_control"  = "annotated callback requires no live writer"
    "callback_hof_invoke_twice"    = "annotated callback requires no live writer"
    "callback_inline_closure"      = "write requires no live writer"
    "callback_reentrant"           = "write requires no live writer"
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
    "unheld_release"               = "read release requires a live reader"
}

$scriptRoot = $PSScriptRoot
$repositoryRoot = (Resolve-Path (Join-Path $scriptRoot "..\..")).Path
$manifestPath = Join-Path $scriptRoot "Cargo.toml"
$miraiPath = Join-Path $repositoryRoot "target\debug\mirai.exe"
$sweepTarget = Join-Path $repositoryRoot "target\rwlock-example-sweep"
$originalLocation = Get-Location
$originalPath = $env:PATH
$originalWrapper = $env:RUSTC_WORKSPACE_WRAPPER
$originalStartFresh = $env:MIRAI_START_FRESH
$originalTargetDir = $env:CARGO_TARGET_DIR

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
    $env:MIRAI_START_FRESH = "true"
    $env:CARGO_TARGET_DIR = $sweepTarget

    if (Test-Path $sweepTarget) {
        Remove-Item -Recurse -Force $sweepTarget
    }

    $bins = @($expectations.Keys)
    if ($Filter) {
        if (-not $expectations.Contains($Filter)) {
            Write-Error "Unknown bin '$Filter'. Available bins: $($bins -join ', ')"
            exit 2
        }
        $bins = @($Filter)
    }

    $cargoPath = (Get-Command cargo -CommandType Application | Select-Object -First 1).Source
    $passed = 0
    foreach ($bin in $bins) {
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
            $matches = $exitCode -eq 0 -and $miraiDiagnostics.Count -eq 0
            $expectedText = "silent"
        } else {
            $matchingDiagnostics = @(
                $miraiDiagnostics | Where-Object { $_ -like "*$expected*" }
            )
            $matches = $exitCode -eq 0 -and
                $miraiDiagnostics.Count -eq 1 -and
                $matchingDiagnostics.Count -eq 1
            $expectedText = $expected
        }

        $actualText = if ($miraiDiagnostics.Count -eq 0) {
            "silent"
        } elseif ($miraiDiagnostics.Count -eq 1) {
            $miraiDiagnostics[0] -replace "^warning: \[MIRAI\] ", ""
        } else {
            "$($miraiDiagnostics.Count) MIRAI diagnostics"
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
            Write-Host "Result: $(if ($matches) { 'PASS' } else { 'FAIL' })"
        } elseif ($matches) {
            Write-Host "✅ $bin — expected: $expectedText; actual: $actualText"
        } else {
            Write-Host "❌ $bin — expected: $expectedText; actual: $actualText; exit: $exitCode"
            if ($output.Count -gt 0) {
                $output | ForEach-Object { Write-Host $_ }
            } else {
                Write-Host "<no output>"
            }
        }

        if ($matches) {
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
    $env:CARGO_TARGET_DIR = $originalTargetDir
    Set-Location $originalLocation
}
