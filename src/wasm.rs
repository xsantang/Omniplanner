// ══════════════════════════════════════════════════════════════
//  WASM — Puente JavaScript para Web / PWA
//  Mismo contrato que el FFI nativo: JSON string in → JSON string out.
// ══════════════════════════════════════════════════════════════
//
//  Uso desde JavaScript (con wasm-bindgen / wasm-pack):
//
//    import init, { omni_command, omni_import_state, omni_export_state }
//        from "./pkg/omniplanner.js";
//
//    await init();
//    // Restaurar estado previo persistido en localStorage (opcional)
//    const saved = localStorage.getItem("omniplanner");
//    omni_command(JSON.stringify({
//        action: "init",
//        params: saved ? { data: saved } : {},
//    }));
//
//    const res = JSON.parse(omni_command(JSON.stringify({
//        action: "dashboard",
//    })));
//
//    // Persistir: pedir export y guardar en localStorage.
//    localStorage.setItem("omniplanner", omni_export_state());
//
//  ══════════════════════════════════════════════════════════════

use wasm_bindgen::prelude::*;

use crate::ffi::process_command;
use crate::storage::AppState;

/// Procesa un comando `{action, params}` en formato JSON y devuelve
/// `{ok, data|error}` como string. Idéntico al FFI nativo.
#[wasm_bindgen]
pub fn omni_command(json_request: &str) -> String {
    process_command(json_request)
}

/// Carga un estado completo (JSON) previamente exportado.
/// Devuelve un JSON `{ok: bool, error?: string}`.
#[wasm_bindgen]
pub fn omni_import_state(json_state: &str) -> String {
    match AppState::cargar_desde_json(json_state) {
        Ok(state) => {
            if let Some(slot) = crate::ffi::app_slot() {
                *slot.lock().unwrap() = Some(state);
                r#"{"ok":true}"#.to_string()
            } else {
                r#"{"ok":false,"error":"No se pudo acceder al estado"}"#.to_string()
            }
        }
        Err(e) => format!(r#"{{"ok":false,"error":{:?}}}"#, e),
    }
}

/// Serializa el estado actual a JSON para que el host lo guarde
/// (por ejemplo, `localStorage`). Devuelve `""` si no hay estado cargado.
#[wasm_bindgen]
pub fn omni_export_state() -> String {
    if let Some(slot) = crate::ffi::app_slot() {
        if let Ok(guard) = slot.lock() {
            if let Some(state) = guard.as_ref() {
                return state.exportar_json().unwrap_or_default();
            }
        }
    }
    String::new()
}

/// Versión del binario — útil para debug desde JS.
#[wasm_bindgen]
pub fn omni_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// Panic hook opcional (comentado — requiere la dependencia `console_error_panic_hook`
// si quieres stack traces legibles en DevTools):
//
// #[wasm_bindgen(start)]
// pub fn __wasm_start() {
//     console_error_panic_hook::set_once();
// }
