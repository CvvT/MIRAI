param(
    [string]$MiraiPath = (Join-Path $PSScriptRoot "target/debug/mirai")
)

$ErrorActionPreference = "Stop"
$repositoryRoot = $PSScriptRoot
$sysroot = (& rustc --print sysroot).Trim()
$rustcLibraryPath = Join-Path $sysroot "lib"
$env:LD_LIBRARY_PATH = if ($env:LD_LIBRARY_PATH) {
    "$rustcLibraryPath$([System.IO.Path]::PathSeparator)$($env:LD_LIBRARY_PATH)"
} else {
    $rustcLibraryPath
}
$annotations = Get-ChildItem (Join-Path $repositoryRoot "target/debug/deps") `
    -Filter "libmirai_annotations-*.rlib" |
    Select-Object -First 1

if (-not $annotations) {
    throw "Could not locate the built mirai_annotations rlib."
}

$rows = [ordered]@{
    "model_field_wrapper_field_double_lock" = @{
        Fixture = "checker/tests/run-pass/model_field_wrapper_field_double_lock.rs"
        ExpectedDiagnostic = "unsatisfied precondition"
    }
    "arc_load_thin_pointer_capture_state_loss" = @{
        Fixture = "checker/tests/run-pass/arc_load_thin_pointer_known_limitation.rs"
        ExpectedDiagnostic = $null
    }
}

$outputDirectory = Join-Path ([System.IO.Path]::GetTempPath()) "mirai-known-limits-$PID"
New-Item -ItemType Directory -Path $outputDirectory | Out-Null

try {
    foreach ($entry in $rows.GetEnumerator()) {
        $fixture = Join-Path $repositoryRoot $entry.Value.Fixture
        $output = & $MiraiPath `
            --crate-name mirai `
            $fixture `
            --crate-type lib `
            --edition=2021 `
            -C debuginfo=2 `
            --out-dir $outputDirectory `
            --sysroot $sysroot `
            -Z span_free_formats `
            --extern "mirai_annotations=$($annotations.FullName)" 2>&1 |
            Out-String

        $diagnostic = $entry.Value.ExpectedDiagnostic
        $observed = if ($diagnostic) { $output.Contains($diagnostic) } else {
            -not $output.Contains("warning: [MIRAI]")
        }
        if (-not $observed) {
            Write-Host $output
            throw "Known-limit row '$($entry.Key)' did not produce its expected outcome."
        }

        $expected = if ($diagnostic) { "fires: $diagnostic" } else { "XFAIL: silent" }
        Write-Host "$($entry.Key): PASS ($expected)"
    }
}
finally {
    Remove-Item -Recurse -Force $outputDirectory -ErrorAction SilentlyContinue
}
