# Omniplanner — Cliente Web (PWA-ready)

Cliente web mínimo que consume la misma librería Rust compilada a **WebAssembly**.
Usa exactamente el mismo contrato JSON que el FFI nativo de Android.

## Requisitos (solo la primera vez)

```powershell
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.115
```

## Compilar

Desde la raíz del repo:

```powershell
./build_web.ps1               # release
./build_web.ps1 -BuildProfile debug
```

Se generará `web/pkg/` con los artefactos (`omniplanner_bg.wasm`, `omniplanner.js`).

## Probar en el navegador

Los navegadores bloquean `import` desde `file://`; sirve `web/` con cualquier servidor local:

```powershell
# Opción A: Python
python -m http.server 8080 --directory web

# Opción B: Node (npx)
npx --yes serve web -l 8080
```

Luego abre <http://localhost:8080/>.

## Arquitectura

- La lógica de negocio completa (tareas, agenda, deudas, presupuesto, canvas, diagramas, …)
  vive en la `lib` de Rust y se comparte entre PC, Android y Web.
- El host (JS, Kotlin/JNI, o el CLI nativo) solo traduce eventos de UI a
  `{"action": "...", "params": {...}}` y renderiza la respuesta JSON.
- En Web, la persistencia es `localStorage`: se llama `omni_export_state()` después
  de cada mutación y se pasa de vuelta al inicializar con `omni_command({action:"init", params:{data: ...}})`.

## Próximos pasos sugeridos

- Empaquetar como **PWA** (manifest + service worker) para instalación offline en móvil/tablet.
- Sustituir `localStorage` por **IndexedDB** para datasets grandes.
- Puente `sync` vía `fetch` (GitHub Gist, Drive) desde el lado JS.
