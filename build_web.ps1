# ══════════════════════════════════════════════════════════════
#  build_web.ps1 — Compila omniplanner para navegador (WASM)
# ══════════════════════════════════════════════════════════════
#
#  Requisitos (solo la primera vez):
#    rustup target add wasm32-unknown-unknown
#    cargo install wasm-bindgen-cli --version 0.2.115
#    # o alternativamente:  cargo install wasm-pack
#
#  Uso:
#    ./build_web.ps1                 # release
#    ./build_web.ps1 -BuildProfile debug
#
#  Salida:
#    web/pkg/omniplanner_bg.wasm
#    web/pkg/omniplanner.js  (glue de wasm-bindgen)
# ══════════════════════════════════════════════════════════════

param(
    [ValidateSet("debug", "release")]
    [string]$BuildProfile = "release"
)

$ErrorActionPreference = "Stop"

$ROOT     = $PSScriptRoot
$OUT_DIR  = Join-Path $ROOT "web\pkg"
$TARGET   = "wasm32-unknown-unknown"
$CRATE    = "omniplanner"

$profileFlag = if ($BuildProfile -eq "release") { "--release" } else { "" }
$profileDir  = if ($BuildProfile -eq "release") { "release" } else { "debug" }

Write-Host "`n>>> Compilando $CRATE para WebAssembly [$BuildProfile]" -ForegroundColor Cyan

$cmd = "cargo build --lib --target $TARGET --no-default-features --features web $profileFlag"
Write-Host $cmd
Invoke-Expression $cmd
if ($LASTEXITCODE -ne 0) { Write-Error "Compilación WASM falló"; exit 1 }

$wasmSrc = "target\$TARGET\$profileDir\$CRATE.wasm"
if (-not (Test-Path $wasmSrc)) { Write-Error "No se encontró $wasmSrc"; exit 1 }

New-Item -ItemType Directory -Force -Path $OUT_DIR | Out-Null

# Generar glue JS/TS con wasm-bindgen
Write-Host "`n>>> Generando bindings JS con wasm-bindgen..." -ForegroundColor Cyan
$bindgen = Get-Command wasm-bindgen -ErrorAction SilentlyContinue
if (-not $bindgen) {
    Write-Error "wasm-bindgen-cli no instalado. Corre: cargo install wasm-bindgen-cli"
    exit 1
}
& wasm-bindgen --target web --out-dir $OUT_DIR $wasmSrc
if ($LASTEXITCODE -ne 0) { Write-Error "wasm-bindgen falló"; exit 1 }

Write-Host "`n[OK] Salida en: $OUT_DIR" -ForegroundColor Green
Get-ChildItem $OUT_DIR | Format-Table Name, Length -AutoSize
