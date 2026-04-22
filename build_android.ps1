# ══════════════════════════════════════════════════════════════
#  build_android.ps1 — Compila libomniplanner.so para Android
# ══════════════════════════════════════════════════════════════

param(
    [ValidateSet("debug","release")]
    [string]$BuildProfile = "release",

    [ValidateSet("arm64","arm","x86_64","x86","all")]
    [string]$Arch = "arm64"
)

$ErrorActionPreference = "Stop"

# ── Rutas ─────────────────────────────────────────────────────
$NDK_HOME  = "$env:LOCALAPPDATA\Android\Sdk\ndk\27.0.12077973"
$TOOLCHAIN = "$NDK_HOME\toolchains\llvm\prebuilt\windows-x86_64"
$JNI_DIR   = "$PSScriptRoot\android\app\src\main\jniLibs"

if (-not (Test-Path $NDK_HOME)) {
    Write-Error "NDK no encontrado en $NDK_HOME"
    exit 1
}

# ── Mapa de targets ──────────────────────────────────────────
$targets = @{
    "arm64"  = @{ rust = "aarch64-linux-android";   abi = "arm64-v8a";   cc = "aarch64-linux-android35-clang.cmd" }
    "arm"    = @{ rust = "armv7-linux-androideabi";  abi = "armeabi-v7a"; cc = "armv7a-linux-androideabi35-clang.cmd" }
    "x86_64" = @{ rust = "x86_64-linux-android";    abi = "x86_64";      cc = "x86_64-linux-android35-clang.cmd" }
    "x86"    = @{ rust = "i686-linux-android";       abi = "x86";         cc = "i686-linux-android35-clang.cmd" }
}

$buildList = if ($Arch -eq "all") { $targets.Keys } else { @($Arch) }
$profileFlag = if ($BuildProfile -eq "release") { "--release" } else { "" }
$profileDir  = if ($BuildProfile -eq "release") { "release" } else { "debug" }

foreach ($arch in $buildList) {
    $t = $targets[$arch]
    Write-Host "`n>>> Compilando para $($t.abi) ($($t.rust)) [$BuildProfile]" -ForegroundColor Cyan

    $env:CC      = "$TOOLCHAIN\bin\$($t.cc)"
    $env:AR      = "$TOOLCHAIN\bin\llvm-ar.exe"
    $env:RANLIB  = "$TOOLCHAIN\bin\llvm-ranlib.exe"
    $env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER       = "$TOOLCHAIN\bin\$($t.cc)"
    $env:CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER     = "$TOOLCHAIN\bin\$($t.cc)"
    $env:CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER        = "$TOOLCHAIN\bin\$($t.cc)"
    $env:CARGO_TARGET_I686_LINUX_ANDROID_LINKER          = "$TOOLCHAIN\bin\$($t.cc)"

    $cmd = "cargo rustc --lib --crate-type cdylib --target $($t.rust) --no-default-features --features android $profileFlag"
    Write-Host $cmd
    Invoke-Expression $cmd
    if ($LASTEXITCODE -ne 0) { Write-Error "Falló compilación para $($t.abi)"; exit 1 }

    # Copiar .so al jniLibs
    $src = "target\$($t.rust)\$profileDir\libomniplanner.so"
    $dst = "$JNI_DIR\$($t.abi)"
    New-Item -ItemType Directory -Force -Path $dst | Out-Null
    Copy-Item $src $dst -Force
    Write-Host "  → $dst\libomniplanner.so" -ForegroundColor Green
}

Write-Host "`n✓ Build Android completado" -ForegroundColor Green
