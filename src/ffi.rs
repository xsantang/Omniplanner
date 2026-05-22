// ══════════════════════════════════════════════════════════════
//  FFI — Puente C/JNI para Android
//  Toda la comunicación es JSON string in → JSON string out
// ══════════════════════════════════════════════════════════════

#[cfg(not(target_arch = "wasm32"))]
use std::ffi::{CStr, CString};
#[cfg(not(target_arch = "wasm32"))]
use std::os::raw::c_char;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::contrasenias;
use crate::storage::AppState;

// Estado global protegido por mutex (un solo hilo de JNI a la vez)
static APP: std::sync::LazyLock<Mutex<Option<AppState>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

static DATA_DIR: std::sync::LazyLock<Mutex<Option<PathBuf>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

/// Permite a storage.rs consultar el data_dir configurado por Android
pub fn data_dir() -> Result<std::sync::MutexGuard<'static, Option<PathBuf>>, String> {
    DATA_DIR.lock().map_err(|e| format!("Lock error: {}", e))
}

/// Expone el slot global del estado para otros módulos (p.ej. `wasm.rs`).
#[cfg(feature = "web")]
pub(crate) fn app_slot() -> Option<&'static Mutex<Option<AppState>>> {
    Some(&APP)
}

// ── Helpers FFI ──────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
fn c_str_to_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .unwrap_or("")
        .to_string()
}

#[cfg(not(target_arch = "wasm32"))]
fn string_to_c(s: String) -> *mut c_char {
    CString::new(s).unwrap_or_default().into_raw()
}

fn ok_json(data: impl Serialize) -> String {
    serde_json::json!({ "ok": true, "data": data }).to_string()
}

fn err_json(msg: &str) -> String {
    serde_json::json!({ "ok": false, "error": msg }).to_string()
}

fn with_state<F, R>(f: F) -> String
where
    F: FnOnce(&mut AppState) -> Result<R, String>,
    R: Serialize,
{
    let mut guard = APP.lock().unwrap();
    match guard.as_mut() {
        Some(state) => match f(state) {
            Ok(v) => ok_json(v),
            Err(e) => err_json(&e),
        },
        None => err_json("Estado no inicializado. Llama a 'init' primero."),
    }
}

// ── Punto de entrada principal ───────────────────────────────

#[derive(Deserialize)]
struct Request {
    action: String,
    #[serde(default)]
    params: Value,
}

/// # Safety
/// `json_request` debe ser un puntero válido a C string UTF-8.
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn omni_command(json_request: *const c_char) -> *mut c_char {
    let input = c_str_to_string(json_request);
    let result = process_command(&input);
    string_to_c(result)
}

/// # Safety
/// Libera la memoria de un string devuelto por omni_command.
#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub unsafe extern "C" fn omni_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}

pub(crate) fn process_command(input: &str) -> String {
    let req: Request = match serde_json::from_str(input) {
        Ok(r) => r,
        Err(e) => return err_json(&format!("JSON inválido: {}", e)),
    };

    match req.action.as_str() {
        // ── Sistema ──
        "init" => cmd_init(&req.params),
        "guardar" => cmd_guardar(),
        "dashboard" => cmd_dashboard(),
        "version" => cmd_version(),
        "buscar" => cmd_buscar(&req.params),

        // ── Tareas ──
        "tareas_listar" => cmd_tareas_listar(),
        "tarea_crear" => cmd_tarea_crear(&req.params),
        "tarea_actualizar" => cmd_tarea_actualizar(&req.params),
        "tarea_eliminar" => cmd_tarea_eliminar(&req.params),

        // ── Agenda ──
        "agenda_hoy" => cmd_agenda_hoy(),
        "agenda_mes" => cmd_agenda_mes(&req.params),
        "evento_crear" => cmd_evento_crear(&req.params),
        "evento_actualizar" => cmd_evento_actualizar(&req.params),
        "evento_eliminar" => cmd_evento_eliminar(&req.params),

        // ── Presupuesto ──
        "presupuesto_resumen" => cmd_presupuesto_resumen(),
        "presupuesto_detalle" => cmd_presupuesto_detalle(&req.params),
        "presupuesto_agregar" => cmd_presupuesto_agregar(&req.params),
        "presupuesto_actualizar_linea" => cmd_presupuesto_actualizar_linea(&req.params),
        "presupuesto_eliminar_linea" => cmd_presupuesto_eliminar_linea(&req.params),

        // ── Deudas ──
        "deudas_listar" => cmd_deudas_listar(),
        "deuda_agregar" => cmd_deuda_agregar(&req.params),
        "deuda_actualizar" => cmd_deuda_actualizar(&req.params),
        "deuda_eliminar" => cmd_deuda_eliminar(&req.params),
        "deuda_registrar_pago" => cmd_deuda_registrar_pago(&req.params),
        "ingreso_agregar" => cmd_ingreso_agregar(&req.params),
        "ingreso_eliminar" => cmd_ingreso_eliminar(&req.params),

        // ── Contraseñas ──
        "contras_listar" => cmd_contras_listar(),
        "contras_guardar" => cmd_contras_guardar(&req.params),
        "contras_actualizar" => cmd_contras_actualizar(&req.params),
        "contras_generar" => cmd_contras_generar(&req.params),
        "contras_verificar" => cmd_contras_verificar(&req.params),
        "contras_eliminar" => cmd_contras_eliminar(&req.params),

        // ── Memoria ──
        "memoria_listar" => cmd_memoria_listar(),
        "memoria_agregar" => cmd_memoria_agregar(&req.params),
        "memoria_eliminar" => cmd_memoria_eliminar(&req.params),

        // ── Canvas (board de ideas) ──
        "canvas_listar" => cmd_canvas_listar(),
        "canvas_crear" => cmd_canvas_crear(&req.params),
        "canvas_detalle" => cmd_canvas_detalle(&req.params),
        "canvas_agregar_nota" => cmd_canvas_agregar_nota(&req.params),
        "canvas_agregar_lista" => cmd_canvas_agregar_lista(&req.params),
        "canvas_eliminar_elemento" => cmd_canvas_eliminar_elemento(&req.params),
        "canvas_eliminar" => cmd_canvas_eliminar(&req.params),

        // ── Diagramas ──
        "diagrama_listar" => cmd_diagrama_listar(),
        "diagrama_crear" => cmd_diagrama_crear(&req.params),
        "diagrama_detalle" => cmd_diagrama_detalle(&req.params),
        "diagrama_agregar_nodo" => cmd_diagrama_agregar_nodo(&req.params),
        "diagrama_conectar" => cmd_diagrama_conectar(&req.params),
        "diagrama_eliminar" => cmd_diagrama_eliminar(&req.params),
        "diagrama_exportar_mermaid" => cmd_diagrama_exportar_mermaid(&req.params),

        // ── Mapper (codificación) ──
        "mapper_codificar" => cmd_mapper_codificar(&req.params),
        "mapper_decodificar" => cmd_mapper_decodificar(&req.params),

        // ── VCS (snapshots del estado) ──
        "vcs_commit" => cmd_vcs_commit(&req.params),
        "vcs_log" => cmd_vcs_log(),

        // ── Sync ──
        "sync_push" => cmd_sync_push(),
        "sync_pull" => cmd_sync_pull(),
        "sync_config" => cmd_sync_config(&req.params),

        // ── Conciliación Bancaria ──────────────────────────────
        "banco_listar" => cmd_banco_listar(),
        "banco_agregar_cuenta" => cmd_banco_agregar_cuenta(&req.params),
        "banco_agregar_contable" => cmd_banco_agregar_contable(&req.params),
        "banco_importar_extracto" => cmd_banco_importar_extracto(&req.params),
        "banco_conciliar" => cmd_banco_conciliar(&req.params),
        "banco_ratios" => cmd_banco_ratios(),
        "tarjeta_agregar" => cmd_tarjeta_agregar(&req.params),
        "tarjeta_listar" => cmd_tarjeta_listar(),
        "tarjeta_cargo" => cmd_tarjeta_cargo(&req.params),
        "prestamo_agregar" => cmd_prestamo_agregar(&req.params),
        "prestamo_listar" => cmd_prestamo_listar(),
        "prestamo_pagar_cuota" => cmd_prestamo_pagar_cuota(&req.params),

        // ── Balance General ───────────────────────────────────
        "balance_nuevo" => cmd_balance_nuevo(&req.params),
        "balance_agregar_partida" => cmd_balance_agregar_partida(&req.params),
        "balance_listar" => cmd_balance_listar(),
        "balance_detalle" => cmd_balance_detalle(&req.params),
        "balance_ratios" => cmd_balance_ratios(&req.params),

        // ── Estado de Resultados ──────────────────────────────
        "resultado_nuevo" => cmd_resultado_nuevo(&req.params),
        "resultado_agregar_partida" => cmd_resultado_agregar_partida(&req.params),
        "resultado_listar" => cmd_resultado_listar(),
        "resultado_detalle" => cmd_resultado_detalle(&req.params),
        "resultado_ratios" => cmd_resultado_ratios(&req.params),

        // ── Propuestas ────────────────────────────────────────
        "prop_listar" => cmd_prop_listar(),
        "prop_dashboard" => cmd_prop_dashboard(),
        "prop_crear" => cmd_prop_crear(&req.params),
        "prop_detalle" => cmd_prop_detalle(&req.params),
        "prop_actualizar_estado" => cmd_prop_actualizar_estado(&req.params),
        "prop_eliminar" => cmd_prop_eliminar(&req.params),
        "prop_agregar_seccion" => cmd_prop_agregar_seccion(&req.params),
        "prop_actualizar_seccion" => cmd_prop_actualizar_seccion(&req.params),
        "prop_agregar_hito" => cmd_prop_agregar_hito(&req.params),
        "prop_completar_hito" => cmd_prop_completar_hito(&req.params),
        "prop_verificar_estrategia" => cmd_prop_verificar_estrategia(&req.params),
        "prop_agregar_reunion" => cmd_prop_agregar_reunion(&req.params),
        "prop_agregar_accion" => cmd_prop_agregar_accion(&req.params),
        "prop_completar_accion" => cmd_prop_completar_accion(&req.params),
        "prop_enviar_recap" => cmd_prop_enviar_recap(&req.params),
        "sme_listar" => cmd_sme_listar(),
        "sme_agregar" => cmd_sme_agregar(&req.params),
        "sme_buscar_area" => cmd_sme_buscar_area(&req.params),
        "prop_solicitar_sme" => cmd_prop_solicitar_sme(&req.params),
        "prop_responder_sme" => cmd_prop_responder_sme(&req.params),
        "prop_solicitar_revision" => cmd_prop_solicitar_revision(&req.params),
        "prop_completar_revision" => cmd_prop_completar_revision(&req.params),
        "prop_escalar" => cmd_prop_escalar(&req.params),
        "prop_resolver_escalacion" => cmd_prop_resolver_escalacion(&req.params),
        "prop_log_salesforce" => cmd_prop_log_salesforce(&req.params),

        // ── Casos / Intake ────────────────────────────────────
        "caso_cola" => cmd_caso_cola(),
        "caso_metricas" => cmd_caso_metricas(),
        "caso_crear" => cmd_caso_crear(&req.params),
        "caso_detalle" => cmd_caso_detalle(&req.params),
        "caso_actualizar_estado" => cmd_caso_actualizar_estado(&req.params),
        "caso_actualizar_paciente" => cmd_caso_actualizar_paciente(&req.params),
        "caso_actualizar_seguro" => cmd_caso_actualizar_seguro(&req.params),
        "caso_actualizar_referido" => cmd_caso_actualizar_referido(&req.params),
        "caso_agregar_checklist" => cmd_caso_agregar_checklist(&req.params),
        "caso_completar_checklist" => cmd_caso_completar_checklist(&req.params),
        "caso_agregar_nota" => cmd_caso_agregar_nota(&req.params),
        "caso_rutear_clinico" => cmd_caso_rutear_clinico(&req.params),
        "caso_outreach_info" => cmd_caso_outreach_info(&req.params),
        "caso_resolver_info" => cmd_caso_resolver_info(&req.params),
        "caso_listos_ruteo" => cmd_caso_listos_ruteo(),
        "caso_requieren_outreach" => cmd_caso_requieren_outreach(),
        "cliente_agregar" => cmd_cliente_agregar(&req.params),
        "cliente_listar" => cmd_cliente_listar(),
        "cliente_detalle" => cmd_cliente_detalle(&req.params),

        // ── Proveedores / Outreach ────────────────────────────
        "prov_listar" => cmd_prov_listar(),
        "prov_buscar" => cmd_prov_buscar(&req.params),
        "prov_agregar" => cmd_prov_agregar(&req.params),
        "prov_detalle" => cmd_prov_detalle(&req.params),
        "prov_actualizar_engagement" => cmd_prov_actualizar_engagement(&req.params),
        "prov_registrar_interaccion" => cmd_prov_registrar_interaccion(&req.params),
        "prov_interacciones" => cmd_prov_interacciones(&req.params),
        "prov_agregar_seguimiento" => cmd_prov_agregar_seguimiento(&req.params),
        "prov_seguimientos_pendientes" => cmd_prov_seguimientos_pendientes(),
        "prov_completar_seguimiento" => cmd_prov_completar_seguimiento(&req.params),
        "prov_sin_contacto" => cmd_prov_sin_contacto(&req.params),
        "prov_metricas" => cmd_prov_metricas(),
        "campana_crear" => cmd_campana_crear(&req.params),
        "campana_listar" => cmd_campana_listar(),
        "campana_actualizar" => cmd_campana_actualizar(&req.params),

        // ── Obras y Ciclo Financiero ──────────────────────────────────
        "obra_dashboard" => cmd_obra_dashboard(),
        "obra_nueva" => cmd_obra_nueva(&req.params),
        "obra_listar" => cmd_obra_listar(),
        "obra_detalle" => cmd_obra_detalle(&req.params),
        "obra_estado" => cmd_obra_estado(&req.params),
        "obra_rfi" => cmd_obra_rfi(&req.params),
        "obra_contacto" => cmd_obra_contacto(&req.params),
        "obra_correo_req" => cmd_obra_correo_req(&req.params),
        "obra_contrato" => cmd_obra_contrato(&req.params),
        "obra_contrato_firmar" => cmd_obra_contrato_firmar(&req.params),
        "obra_plazo_agregar" => cmd_obra_plazo_agregar(&req.params),
        "obra_plazo_cumplir" => cmd_obra_plazo_cumplir(&req.params),
        "obra_posicion_contable" => cmd_obra_posicion_contable(&req.params),
        "obra_consulta_nueva" => cmd_obra_consulta_nueva(&req.params),
        "obra_consulta_responder" => cmd_obra_consulta_responder(&req.params),
        "obra_consultas_pendientes" => cmd_obra_consultas_pendientes(&req.params),
        "obra_desembolso_registrar" => cmd_obra_desembolso_registrar(&req.params),
        "obra_gasto_registrar" => cmd_obra_gasto_registrar(&req.params),
        "obra_gastos_listar" => cmd_obra_gastos_listar(&req.params),
        "obra_cambio_alcance" => cmd_obra_cambio_alcance(&req.params),
        "obra_cambio_aprobar" => cmd_obra_cambio_aprobar(&req.params),
        "obra_reporte_avance" => cmd_obra_reporte_avance(&req.params),
        "obra_reporte_confirmar" => cmd_obra_reporte_confirmar(&req.params),
        "obra_ciclo_verificar" => cmd_obra_ciclo_verificar(&req.params),
        "obra_auditoria" => cmd_obra_auditoria(&req.params),

        // ── Cobranzas y Gestión de Cobro ──────────────────────────────
        "cobro_dashboard" => cmd_cobro_dashboard(),
        "cobro_perfil_nuevo" => cmd_cobro_perfil_nuevo(&req.params),
        "cobro_perfil_listar" => cmd_cobro_perfil_listar(),
        "cobro_perfil_detalle" => cmd_cobro_perfil_detalle(&req.params),
        "cobro_perfil_actualizar" => cmd_cobro_perfil_actualizar(&req.params),
        "cobro_cuenta_nueva" => cmd_cobro_cuenta_nueva(&req.params),
        "cobro_cuenta_listar" => cmd_cobro_cuenta_listar(),
        "cobro_cuenta_vencidas" => cmd_cobro_cuenta_vencidas(),
        "cobro_registrar_pago" => cmd_cobro_registrar_pago(&req.params),
        "cobro_alerta_nueva" => cmd_cobro_alerta_nueva(&req.params),
        "cobro_alertas_activas" => cmd_cobro_alertas_activas(),
        "cobro_alertas_criticas" => cmd_cobro_alertas_criticas(),
        "cobro_alerta_avanzar" => cmd_cobro_alerta_avanzar(&req.params),
        "cobro_alerta_completar" => cmd_cobro_alerta_completar(&req.params),
        "cobro_alerta_reagendar" => cmd_cobro_alerta_reagendar(&req.params),
        "cobro_alerta_cancelar" => cmd_cobro_alerta_cancelar(&req.params),
        "cobro_alerta_contacto" => cmd_cobro_alerta_contacto(&req.params),
        "cobro_llamadas_hoy" => cmd_cobro_llamadas_hoy(),
        "cobro_generar_alertas_auto" => cmd_cobro_generar_alertas_auto(),
        "cobro_exportar_csv" => cmd_cobro_exportar_csv(),

        _ => err_json(&format!("Acción desconocida: {}", req.action)),
    }
}

// ══════════════════════════════════════════════════════════════
//  Implementación de comandos
// ══════════════════════════════════════════════════════════════

fn cmd_init(params: &Value) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        // En WASM no hay filesystem: si `params.data` trae un JSON, lo cargamos.
        let estado_inicial = params.get("data").and_then(|v| v.as_str()).unwrap_or("");
        let state = if estado_inicial.is_empty() {
            AppState::new()
        } else {
            match AppState::cargar_desde_json(estado_inicial) {
                Ok(s) => s,
                Err(e) => return err_json(&format!("Error cargando datos: {}", e)),
            }
        };
        *APP.lock().unwrap() = Some(state);
        ok_json("Omniplanner inicializado (web)")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let ruta = params
            .get("data_dir")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let dir = if ruta.is_empty() {
            dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("omniplanner")
        } else {
            PathBuf::from(ruta)
        };

        if let Err(e) = std::fs::create_dir_all(&dir) {
            return err_json(&format!("No se pudo crear directorio: {}", e));
        }

        *DATA_DIR.lock().unwrap() = Some(dir);

        let state = match AppState::cargar() {
            Ok(s) => s,
            Err(e) => return err_json(&format!("Error cargando datos: {}", e)),
        };
        *APP.lock().unwrap() = Some(state);

        ok_json("Omniplanner inicializado")
    }
}

fn cmd_guardar() -> String {
    with_state(|state| {
        state.guardar()?;
        Ok("Guardado")
    })
}

fn cmd_dashboard() -> String {
    use chrono::{Datelike, Local};

    with_state(|state| {
        let hoy = Local::now().date_naive();

        // Tareas
        let pendientes = state
            .tasks
            .tareas
            .iter()
            .filter(|t| {
                t.estado == crate::tasks::TaskStatus::Pendiente
                    || t.estado == crate::tasks::TaskStatus::EnProgreso
            })
            .count();
        let hoy_count = state.tasks.tareas.iter().filter(|t| t.fecha == hoy).count();

        // Eventos hoy
        let eventos_hoy: Vec<Value> = state
            .agenda
            .eventos
            .iter()
            .filter(|e| e.fecha == hoy)
            .map(|e| {
                serde_json::json!({
                    "titulo": e.titulo,
                    "hora": e.hora_inicio.format("%H:%M").to_string(),
                    "tipo": format!("{:?}", e.tipo),
                })
            })
            .collect();

        // Presupuesto
        let mes_str = format!("{}-{:02}", hoy.year(), hoy.month());
        let pres = state.presupuesto.meses.iter().find(|m| m.mes == mes_str);
        let (ingresos, gastos) = pres
            .map(|p| {
                let ing: f64 = p
                    .lineas
                    .iter()
                    .filter(|l| l.categoria == crate::ml::presupuesto_cero::Categoria::Ingreso)
                    .map(|l| l.monto)
                    .sum();
                let gas: f64 = p
                    .lineas
                    .iter()
                    .filter(|l| l.categoria != crate::ml::presupuesto_cero::Categoria::Ingreso)
                    .map(|l| l.monto)
                    .sum();
                (ing, gas)
            })
            .unwrap_or((0.0, 0.0));

        // Contraseñas
        let n_contras = state.contrasenias.entradas.len();

        Ok(serde_json::json!({
            "fecha": hoy.to_string(),
            "tareas_pendientes": pendientes,
            "tareas_hoy": hoy_count,
            "eventos_hoy": eventos_hoy,
            "presupuesto": {
                "ingresos": ingresos,
                "gastos": gastos,
                "balance": ingresos - gastos,
            },
            "contrasenias": n_contras,
            "memoria": state.memoria.recuerdos.len(),
        }))
    })
}

// ── Tareas ───────────────────────────────────────────────────

fn cmd_tareas_listar() -> String {
    with_state(|state| Ok(serde_json::to_value(&state.tasks.tareas).unwrap()))
}

fn cmd_tarea_crear(params: &Value) -> String {
    use crate::tasks::{Prioridad, Task, TaskStatus};
    use chrono::{Local, NaiveDate, NaiveTime};

    with_state(|state| {
        let titulo = params
            .get("titulo")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'titulo'")?
            .to_string();
        let desc = params
            .get("descripcion")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let fecha_str = params.get("fecha").and_then(|v| v.as_str()).unwrap_or("");
        let fecha = NaiveDate::parse_from_str(fecha_str, "%Y-%m-%d")
            .unwrap_or_else(|_| Local::now().date_naive());
        let hora_str = params
            .get("hora")
            .and_then(|v| v.as_str())
            .unwrap_or("09:00");
        let hora = NaiveTime::parse_from_str(hora_str, "%H:%M")
            .unwrap_or_else(|_| NaiveTime::from_hms_opt(9, 0, 0).unwrap());
        let prioridad = match params
            .get("prioridad")
            .and_then(|v| v.as_str())
            .unwrap_or("media")
        {
            "baja" => Prioridad::Baja,
            "alta" => Prioridad::Alta,
            "urgente" => Prioridad::Urgente,
            _ => Prioridad::Media,
        };
        let etiquetas: Vec<String> = params
            .get("etiquetas")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let ahora = Local::now().naive_local();
        let task = Task {
            id: uuid::Uuid::new_v4().to_string(),
            titulo,
            descripcion: desc,
            fecha,
            hora,
            estado: TaskStatus::Pendiente,
            prioridad,
            etiquetas,
            follow_up: None,
            creado: ahora,
            actualizado: ahora,
        };

        state.tasks.tareas.push(task.clone());
        state.guardar()?;
        Ok(serde_json::to_value(&task).unwrap())
    })
}

fn cmd_tarea_actualizar(params: &Value) -> String {
    use crate::tasks::TaskStatus;

    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?;

        let task = state
            .tasks
            .tareas
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or("Tarea no encontrada")?;

        if let Some(titulo) = params.get("titulo").and_then(|v| v.as_str()) {
            task.titulo = titulo.to_string();
        }
        if let Some(estado) = params.get("estado").and_then(|v| v.as_str()) {
            task.estado = match estado {
                "pendiente" => TaskStatus::Pendiente,
                "en_progreso" => TaskStatus::EnProgreso,
                "completada" => TaskStatus::Completada,
                "cancelada" => TaskStatus::Cancelada,
                _ => task.estado.clone(),
            };
        }
        if let Some(desc) = params.get("descripcion").and_then(|v| v.as_str()) {
            task.descripcion = desc.to_string();
        }
        task.actualizado = chrono::Local::now().naive_local();
        state.guardar()?;
        Ok("Actualizado")
    })
}

fn cmd_tarea_eliminar(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?;
        let antes = state.tasks.tareas.len();
        state.tasks.tareas.retain(|t| t.id != id);
        if state.tasks.tareas.len() < antes {
            state.guardar()?;
            Ok("Eliminada")
        } else {
            Err("Tarea no encontrada".to_string())
        }
    })
}

// ── Agenda ───────────────────────────────────────────────────

fn cmd_agenda_hoy() -> String {
    use chrono::Local;

    with_state(|state| {
        let hoy = Local::now().date_naive();
        let eventos: Vec<Value> = state
            .agenda
            .eventos
            .iter()
            .filter(|e| e.fecha == hoy)
            .map(|e| serde_json::to_value(e).unwrap())
            .collect();
        Ok(serde_json::json!({ "fecha": hoy.to_string(), "eventos": eventos }))
    })
}

fn cmd_agenda_mes(params: &Value) -> String {
    use chrono::{Datelike, Local};

    with_state(|state| {
        let mes = params
            .get("mes")
            .and_then(|v| v.as_u64())
            .unwrap_or(Local::now().date_naive().month() as u64) as u32;
        let anio = params
            .get("anio")
            .and_then(|v| v.as_i64())
            .unwrap_or(Local::now().date_naive().year() as i64) as i32;

        let eventos: Vec<Value> = state
            .agenda
            .eventos
            .iter()
            .filter(|e| e.fecha.month() == mes && e.fecha.year() == anio)
            .map(|e| serde_json::to_value(e).unwrap())
            .collect();

        Ok(serde_json::json!({
            "mes": mes, "anio": anio, "eventos": eventos
        }))
    })
}

fn cmd_evento_crear(params: &Value) -> String {
    use crate::agenda::{Evento, Frecuencia, TipoEvento};
    use chrono::{Local, NaiveDate, NaiveTime};

    with_state(|state| {
        let titulo = params
            .get("titulo")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'titulo'")?
            .to_string();
        let fecha_str = params.get("fecha").and_then(|v| v.as_str()).unwrap_or("");
        let fecha = NaiveDate::parse_from_str(fecha_str, "%Y-%m-%d")
            .unwrap_or_else(|_| Local::now().date_naive());
        let hora_str = params
            .get("hora")
            .and_then(|v| v.as_str())
            .unwrap_or("09:00");
        let hora = NaiveTime::parse_from_str(hora_str, "%H:%M")
            .unwrap_or_else(|_| NaiveTime::from_hms_opt(9, 0, 0).unwrap());
        let tipo = match params
            .get("tipo")
            .and_then(|v| v.as_str())
            .unwrap_or("recordatorio")
        {
            "reunion" => TipoEvento::Reunion,
            "cita" => TipoEvento::Cita,
            "cumpleanos" => TipoEvento::Cumpleanos,
            "pago" => TipoEvento::Pago,
            "follow_up" => TipoEvento::FollowUp,
            _ => TipoEvento::Recordatorio,
        };
        let desc = params
            .get("descripcion")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let evento = Evento {
            id: uuid::Uuid::new_v4().to_string(),
            titulo,
            descripcion: desc,
            tipo,
            fecha,
            hora_inicio: hora,
            hora_fin: None,
            recurrente: false,
            frecuencia: Frecuencia::UnaVez,
            concepto: String::new(),
            notas: vec![],
            creado: Local::now().naive_local(),
            emoji: None,
            mensaje_recordatorio: None,
        };

        state.agenda.eventos.push(evento.clone());
        state.guardar()?;
        Ok(serde_json::to_value(&evento).unwrap())
    })
}

fn cmd_evento_actualizar(params: &Value) -> String {
    use chrono::{NaiveDate, NaiveTime};

    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?;
        let evento = state
            .agenda
            .eventos
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or("Evento no encontrado")?;

        if let Some(t) = params.get("titulo").and_then(|v| v.as_str()) {
            evento.titulo = t.to_string();
        }
        if let Some(f) = params.get("fecha").and_then(|v| v.as_str()) {
            if let Ok(fecha) = NaiveDate::parse_from_str(f, "%Y-%m-%d") {
                evento.fecha = fecha;
            }
        }
        if let Some(h) = params.get("hora").and_then(|v| v.as_str()) {
            if let Ok(hora) = NaiveTime::parse_from_str(h, "%H:%M") {
                evento.hora_inicio = hora;
            }
        }
        if let Some(d) = params.get("descripcion").and_then(|v| v.as_str()) {
            evento.descripcion = d.to_string();
        }
        state.guardar()?;
        Ok("Evento actualizado")
    })
}

fn cmd_evento_eliminar(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?;
        let antes = state.agenda.eventos.len();
        state.agenda.eventos.retain(|e| e.id != id);
        if state.agenda.eventos.len() < antes {
            state.guardar()?;
            Ok("Eliminado")
        } else {
            Err("Evento no encontrado".to_string())
        }
    })
}

// ── Presupuesto ──────────────────────────────────────────────

fn cmd_presupuesto_resumen() -> String {
    use crate::ml::presupuesto_cero::Categoria;

    with_state(|state| {
        let resumen: Vec<Value> = state
            .presupuesto
            .meses
            .iter()
            .map(|m| {
                let ing: f64 = m
                    .lineas
                    .iter()
                    .filter(|l| l.categoria == Categoria::Ingreso)
                    .map(|l| l.monto)
                    .sum();
                let gas: f64 = m
                    .lineas
                    .iter()
                    .filter(|l| l.categoria != Categoria::Ingreso)
                    .map(|l| l.monto)
                    .sum();
                serde_json::json!({
                    "mes": m.mes,
                    "ingresos": ing,
                    "gastos": gas,
                    "balance": ing - gas,
                })
            })
            .collect();
        Ok(serde_json::json!({ "meses": resumen }))
    })
}

fn cmd_presupuesto_agregar(params: &Value) -> String {
    use crate::ml::presupuesto_cero::{Categoria, LineaPresupuesto, PresupuestoMensual};
    use chrono::{Datelike, Local};

    with_state(|state| {
        let nombre = params
            .get("nombre")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'nombre'")?
            .to_string();
        let monto = params
            .get("monto")
            .and_then(|v| v.as_f64())
            .ok_or("Falta 'monto'")?;
        let cat = params
            .get("categoria")
            .and_then(|v| v.as_str())
            .unwrap_or("gasto_variable");
        let categoria = match cat {
            "ingreso" => Categoria::Ingreso,
            "gasto_fijo" => Categoria::GastoFijo,
            "pago_deuda" => Categoria::PagoDeuda,
            "ahorro" => Categoria::Ahorro,
            _ => Categoria::GastoVariable,
        };

        let hoy = Local::now().date_naive();
        let mes_str = format!("{}-{:02}", hoy.year(), hoy.month());

        // Buscar o crear mes actual
        if !state.presupuesto.meses.iter().any(|m| m.mes == mes_str) {
            state.presupuesto.meses.push(PresupuestoMensual {
                mes: mes_str.clone(),
                lineas: vec![],
            });
        }

        let mes = state
            .presupuesto
            .meses
            .iter_mut()
            .find(|m| m.mes == mes_str)
            .unwrap();

        let pagado = params
            .get("pagado")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let fecha_limite = params
            .get("fecha_limite")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let notas = params
            .get("notas")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let saldo_total_deuda = params.get("saldo_total_deuda").and_then(|v| v.as_f64());

        mes.lineas.push(LineaPresupuesto {
            nombre,
            categoria,
            monto,
            pagado,
            fecha_limite,
            notas,
            saldo_total_deuda,
            monto_pagado_real: 0.0,
            meses_atrasados: 0,
            frecuencia: crate::ml::FrecuenciaPago::Mensual,
        });

        state.guardar()?;
        Ok("Línea agregada al presupuesto")
    })
}

fn cmd_presupuesto_detalle(params: &Value) -> String {
    use crate::ml::presupuesto_cero::Categoria;

    with_state(|state| {
        let mes_str = params
            .get("mes")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'mes'")?;
        let mes = state
            .presupuesto
            .meses
            .iter()
            .find(|m| m.mes == mes_str)
            .ok_or("Mes no encontrado")?;

        let lineas: Vec<Value> = mes
            .lineas
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let cat_str = match l.categoria {
                    Categoria::Ingreso => "ingreso",
                    Categoria::GastoFijo => "gasto_fijo",
                    Categoria::GastoVariable => "gasto_variable",
                    Categoria::PagoDeuda => "pago_deuda",
                    Categoria::Ahorro => "ahorro",
                };
                serde_json::json!({
                    "indice": i,
                    "nombre": l.nombre,
                    "monto": l.monto,
                    "categoria": cat_str,
                    "pagado": l.pagado,
                    "fecha_limite": l.fecha_limite,
                    "notas": l.notas,
                    "saldo_total_deuda": l.saldo_total_deuda,
                })
            })
            .collect();
        Ok(serde_json::json!({ "mes": mes_str, "lineas": lineas }))
    })
}

fn cmd_presupuesto_eliminar_linea(params: &Value) -> String {
    with_state(|state| {
        let mes_str = params
            .get("mes")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'mes'")?;
        let indice = params
            .get("indice")
            .and_then(|v| v.as_u64())
            .ok_or("Falta 'indice'")? as usize;

        let mes = state
            .presupuesto
            .meses
            .iter_mut()
            .find(|m| m.mes == mes_str)
            .ok_or("Mes no encontrado")?;

        if indice >= mes.lineas.len() {
            return Err("Índice fuera de rango".to_string());
        }
        mes.lineas.remove(indice);
        state.guardar()?;
        Ok("Línea eliminada")
    })
}

fn cmd_presupuesto_actualizar_linea(params: &Value) -> String {
    use crate::ml::presupuesto_cero::Categoria;

    with_state(|state| {
        let mes_str = params
            .get("mes")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'mes'")?;
        let indice = params
            .get("indice")
            .and_then(|v| v.as_u64())
            .ok_or("Falta 'indice'")? as usize;

        let mes = state
            .presupuesto
            .meses
            .iter_mut()
            .find(|m| m.mes == mes_str)
            .ok_or("Mes no encontrado")?;
        if indice >= mes.lineas.len() {
            return Err("Índice fuera de rango".to_string());
        }
        let linea = &mut mes.lineas[indice];

        if let Some(n) = params.get("nombre").and_then(|v| v.as_str()) {
            linea.nombre = n.to_string();
        }
        if let Some(m) = params.get("monto").and_then(|v| v.as_f64()) {
            linea.monto = m;
        }
        if let Some(cat) = params.get("categoria").and_then(|v| v.as_str()) {
            linea.categoria = match cat {
                "ingreso" => Categoria::Ingreso,
                "gasto_fijo" => Categoria::GastoFijo,
                "pago_deuda" => Categoria::PagoDeuda,
                "ahorro" => Categoria::Ahorro,
                _ => Categoria::GastoVariable,
            };
        }
        if let Some(p) = params.get("pagado").and_then(|v| v.as_bool()) {
            linea.pagado = p;
        }
        if let Some(f) = params.get("fecha_limite").and_then(|v| v.as_str()) {
            linea.fecha_limite = f.to_string();
        }
        if let Some(n) = params.get("notas").and_then(|v| v.as_str()) {
            linea.notas = n.to_string();
        }
        if let Some(s) = params.get("saldo_total_deuda").and_then(|v| v.as_f64()) {
            linea.saldo_total_deuda = Some(s);
        }
        state.guardar()?;
        Ok("Línea actualizada")
    })
}

// ── Rastreador de Deudas ─────────────────────────────────────

fn cmd_deudas_listar() -> String {
    with_state(|state| {
        state.asesor.rastreador.migrar_ingreso_legacy();
        let lista: Vec<Value> = state
            .asesor
            .rastreador
            .deudas
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let tipo = if d.es_pago_corriente() {
                    "pago_corriente"
                } else if d.tasa_anual >= 0.01 {
                    "deuda"
                } else {
                    "otro"
                };
                serde_json::json!({
                    "indice": i,
                    "nombre": d.nombre,
                    "tasa_anual": d.tasa_anual,
                    "pago_minimo": d.pago_minimo,
                    "pago_pi_mensual": d.pago_pi_mensual(),
                    "escrow_mensual": d.escrow_mensual,
                    "pago_total_mensual": d.pago_total_mensual(),
                    "activa": d.activa,
                    "obligatoria": d.obligatoria,
                    "saldo_actual": d.saldo_actual(),
                    "enganche": d.enganche,
                    "tipo": tipo,
                })
            })
            .collect();
        let ingresos: Vec<Value> = state
            .asesor
            .rastreador
            .ingresos
            .iter()
            .enumerate()
            .map(|(i, ing)| {
                serde_json::json!({
                    "indice": i,
                    "concepto": ing.concepto,
                    "monto": ing.monto,
                    "frecuencia": ing.frecuencia.nombre(),
                    "monto_mensual": ing.monto_mensual(),
                    "monto_mensual_neto": ing.monto_mensual_neto(),
                    "retencion_federal_mensual": ing.retencion_federal_mensual(),
                    "retencion_estatal_mensual": ing.retencion_estatal_mensual(),
                    "retencion_social_security_mensual": ing.retencion_social_security_mensual(),
                    "retencion_medicare_mensual": ing.retencion_medicare_mensual(),
                    "confirmado": ing.confirmado,
                    "taxeable": ing.es_taxeable(),
                    "impuesto_federal": ing.paga_impuesto_federal(),
                    "impuesto_estatal": ing.paga_impuesto_estatal(),
                    "allotment_federal_pct": ing.allotment_federal_pct_efectivo(),
                    "allotment_estatal_pct": ing.allotment_estatal_pct_efectivo(),
                    "retener_social_security": ing.retener_social_security,
                    "retener_medicare": ing.retener_medicare,
                    "permitir_allotment_cero": ing.permitir_allotment_cero,
                    "es_beneficio_social_security": ing.es_beneficio_social_security,
                    "beneficio_social_security_temprano": ing.beneficio_social_security_temprano,
                    "estado_trabajo": ing.estado_trabajo,
                })
            })
            .collect();
        let ingreso = state.asesor.rastreador.ingreso_mensual_confirmado();
        let ingreso_neto = state.asesor.rastreador.ingreso_mensual_confirmado_neto();
        let ingreso_no_confirmado = state.asesor.rastreador.ingreso_mensual_no_confirmado();
        let deuda_total = state.asesor.rastreador.deuda_total_actual();
        Ok(serde_json::json!({
            "deudas": lista,
            "ingresos": ingresos,
            "ingreso_mensual": ingreso,
            "ingreso_mensual_confirmado": ingreso,
            "ingreso_mensual_confirmado_neto": ingreso_neto,
            "ingreso_mensual_no_confirmado": ingreso_no_confirmado,
            "retencion_mensual_total": state.asesor.rastreador.retencion_total_mensual_completa(),
            "deuda_total": deuda_total,
            "estado_residencia": state.asesor.rastreador.estado_residencia,
        }))
    })
}

fn cmd_deuda_agregar(params: &Value) -> String {
    use crate::ml::advisor::DeudaRastreada;

    with_state(|state| {
        let nombre = params
            .get("nombre")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'nombre'")?;
        let tasa_anual = params
            .get("tasa_anual")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let pago_minimo = params
            .get("pago_minimo")
            .and_then(|v| v.as_f64())
            .ok_or("Falta 'pago_minimo'")?;
        let obligatoria = params
            .get("obligatoria")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let saldo_inicial = params
            .get("saldo_inicial")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let enganche = params
            .get("enganche")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let escrow_mensual = params
            .get("escrow_mensual")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let pago_pi_mensual = params
            .get("pago_pi_mensual")
            .and_then(|v| v.as_f64())
            .unwrap_or(pago_minimo);

        let mut deuda = DeudaRastreada::nueva(nombre, tasa_anual, pago_minimo);
        deuda.obligatoria = obligatoria;
        deuda.enganche = enganche;
        deuda.escrow_mensual = escrow_mensual.max(0.0);
        deuda.principal_interes_mensual = pago_pi_mensual.max(0.0);

        // Saldo efectivo = total - enganche ya pagado
        let saldo_efectivo = (saldo_inicial - enganche).max(0.0);
        if saldo_efectivo > 0.0 {
            let mes = chrono::Local::now().format("%Y-%m").to_string();
            deuda.registrar_mes(&mes, saldo_efectivo, 0.0, 0.0);
        }

        state.asesor.rastreador.agregar_deuda(deuda);
        state.guardar()?;
        Ok("Deuda agregada")
    })
}

fn cmd_deuda_actualizar(params: &Value) -> String {
    with_state(|state| {
        let indice = params
            .get("indice")
            .and_then(|v| v.as_u64())
            .ok_or("Falta 'indice'")? as usize;
        let deudas = &mut state.asesor.rastreador.deudas;
        if indice >= deudas.len() {
            return Err("Índice fuera de rango".to_string());
        }
        let d = &mut deudas[indice];
        if let Some(n) = params.get("nombre").and_then(|v| v.as_str()) {
            d.nombre = n.to_string();
        }
        if let Some(t) = params.get("tasa_anual").and_then(|v| v.as_f64()) {
            d.tasa_anual = t;
        }
        if let Some(p) = params.get("pago_minimo").and_then(|v| v.as_f64()) {
            d.pago_minimo = p;
        }
        if let Some(p) = params.get("pago_pi_mensual").and_then(|v| v.as_f64()) {
            d.principal_interes_mensual = p;
        }
        if let Some(e) = params.get("escrow_mensual").and_then(|v| v.as_f64()) {
            d.escrow_mensual = e.max(0.0);
        }
        if let Some(o) = params.get("obligatoria").and_then(|v| v.as_bool()) {
            d.obligatoria = o;
        }
        if let Some(a) = params.get("activa").and_then(|v| v.as_bool()) {
            d.activa = a;
        }
        state.guardar()?;
        Ok("Deuda actualizada")
    })
}

fn cmd_deuda_eliminar(params: &Value) -> String {
    with_state(|state| {
        let indice = params
            .get("indice")
            .and_then(|v| v.as_u64())
            .ok_or("Falta 'indice'")? as usize;
        let deudas = &mut state.asesor.rastreador.deudas;
        if indice >= deudas.len() {
            return Err("Índice fuera de rango".to_string());
        }
        deudas.remove(indice);
        state.guardar()?;
        Ok("Deuda eliminada")
    })
}

fn cmd_deuda_registrar_pago(params: &Value) -> String {
    with_state(|state| {
        let indice = params
            .get("indice")
            .and_then(|v| v.as_u64())
            .ok_or("Falta 'indice'")? as usize;
        let pago = params
            .get("pago")
            .and_then(|v| v.as_f64())
            .ok_or("Falta 'pago'")?;
        let pago_escrow = params
            .get("pago_escrow")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let nuevos_cargos = params
            .get("nuevos_cargos")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        if indice >= state.asesor.rastreador.deudas.len() {
            return Err("Índice fuera de rango".to_string());
        }

        let saldo = state.asesor.rastreador.deudas[indice].saldo_actual();
        let mes = chrono::Local::now().format("%Y-%m").to_string();
        state.asesor.rastreador.deudas[indice].registrar_mes_con_escrow(
            &mes,
            saldo,
            pago,
            pago_escrow,
            nuevos_cargos,
        );
        let nuevo_saldo = state.asesor.rastreador.deudas[indice].saldo_actual();

        state.guardar()?;
        Ok(serde_json::json!({
            "saldo_anterior": saldo,
            "pago": pago,
            "pago_escrow": pago_escrow,
            "saldo_nuevo": nuevo_saldo,
        }))
    })
}

fn cmd_ingreso_agregar(params: &Value) -> String {
    use crate::ml::advisor::{FrecuenciaPago, IngresoRastreado};

    with_state(|state| {
        let concepto = params
            .get("concepto")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'concepto'")?
            .to_string();
        let monto = params
            .get("monto")
            .and_then(|v| v.as_f64())
            .ok_or("Falta 'monto'")?;
        let freq_str = params
            .get("frecuencia")
            .and_then(|v| v.as_str())
            .unwrap_or("mensual");
        let confirmado = params
            .get("confirmado")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let impuesto_federal = params
            .get("impuesto_federal")
            .and_then(|v| v.as_bool())
            .unwrap_or_else(|| {
                params
                    .get("taxeable")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            });
        let impuesto_estatal = params
            .get("impuesto_estatal")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let allotment_federal_pct = params
            .get("allotment_federal_pct")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let allotment_estatal_pct = params
            .get("allotment_estatal_pct")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let retener_social_security = params
            .get("retener_social_security")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let retener_medicare = params
            .get("retener_medicare")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let permitir_allotment_cero = params
            .get("permitir_allotment_cero")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let es_beneficio_social_security = params
            .get("es_beneficio_social_security")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let beneficio_social_security_temprano = params
            .get("beneficio_social_security_temprano")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let estado_trabajo = params
            .get("estado_trabajo")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_uppercase();
        let frecuencia = match freq_str {
            "semanal" => FrecuenciaPago::Semanal,
            "quincenal" => FrecuenciaPago::Quincenal,
            "trimestral" => FrecuenciaPago::Trimestral,
            "semestral" => FrecuenciaPago::Semestral,
            "anual" => FrecuenciaPago::Anual,
            "una_vez" => FrecuenciaPago::UnaVez,
            _ => FrecuenciaPago::Mensual,
        };
        state.asesor.rastreador.ingresos.push(IngresoRastreado {
            concepto,
            monto,
            frecuencia,
            confirmado,
            taxeable: impuesto_federal || impuesto_estatal,
            impuesto_federal,
            impuesto_estatal,
            allotment_federal_pct,
            allotment_estatal_pct,
            retener_social_security,
            retener_medicare,
            permitir_allotment_cero,
            es_beneficio_social_security,
            beneficio_social_security_temprano,
            estado_trabajo,
            mes_aplicable: None,
        });
        state.guardar()?;
        Ok("Ingreso agregado")
    })
}

fn cmd_ingreso_eliminar(params: &Value) -> String {
    with_state(|state| {
        let indice = params
            .get("indice")
            .and_then(|v| v.as_u64())
            .ok_or("Falta 'indice'")? as usize;
        if indice >= state.asesor.rastreador.ingresos.len() {
            return Err("Índice fuera de rango".to_string());
        }
        state.asesor.rastreador.ingresos.remove(indice);
        state.guardar()?;
        Ok("Ingreso eliminado")
    })
}

// ── Contraseñas ──────────────────────────────────────────────

fn cmd_contras_listar() -> String {
    with_state(|state| {
        let lista: Vec<Value> = state
            .contrasenias
            .entradas
            .iter()
            .map(|e| {
                serde_json::json!({
                    "id": e.id,
                    "nombre": e.nombre,
                    "usuario": e.usuario,
                    "categoria": e.categoria,
                    "creado": e.creado.to_string(),
                    "tiene_clave": !e.clave.is_empty(),
                })
            })
            .collect();
        Ok(serde_json::json!({ "entradas": lista }))
    })
}

fn cmd_contras_guardar(params: &Value) -> String {
    with_state(|state| {
        let nombre = params
            .get("nombre")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'nombre'")?;
        let usuario = params.get("usuario").and_then(|v| v.as_str()).unwrap_or("");
        let clave = params
            .get("clave")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'clave'")?;
        let notas = params.get("notas").and_then(|v| v.as_str()).unwrap_or("");
        let categoria = params
            .get("categoria")
            .and_then(|v| v.as_str())
            .unwrap_or("otro");

        let entrada = crate::contrasenias::AlmacenContrasenias::nueva_entrada(
            nombre, usuario, clave, notas, categoria,
        );
        state.contrasenias.entradas.push(entrada);
        state.guardar()?;
        Ok("Contraseña guardada")
    })
}

fn cmd_contras_actualizar(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?;
        let entrada = state
            .contrasenias
            .entradas
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or("No encontrada")?;

        if let Some(n) = params.get("nombre").and_then(|v| v.as_str()) {
            entrada.nombre = n.to_string();
        }
        if let Some(u) = params.get("usuario").and_then(|v| v.as_str()) {
            entrada.usuario = u.to_string();
        }
        if let Some(c) = params.get("clave").and_then(|v| v.as_str()) {
            entrada.clave = c.to_string();
        }
        state.guardar()?;
        Ok("Actualizada")
    })
}

fn cmd_contras_generar(params: &Value) -> String {
    let longitud = params
        .get("longitud")
        .and_then(|v| v.as_u64())
        .unwrap_or(20) as usize;
    let especiales = params
        .get("especiales")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let mut passwords = Vec::new();
    for _ in 0..5 {
        let p = contrasenias::generar_contrasenia(longitud, especiales);
        let (fuerza, nivel) = contrasenias::evaluar_fortaleza(&p);
        passwords.push(serde_json::json!({
            "password": p,
            "fuerza": fuerza,
            "nivel": nivel,
        }));
    }
    ok_json(serde_json::json!({ "passwords": passwords }))
}

fn cmd_contras_verificar(params: &Value) -> String {
    let original = params
        .get("original")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let input = params.get("input").and_then(|v| v.as_str()).unwrap_or("");

    let resultado = contrasenias::verificar_texto(original, input);
    ok_json(serde_json::json!({
        "coincide": resultado.coincide,
        "total_chars": resultado.total_chars,
        "correctos": resultado.total_chars - resultado.errores.len(),
        "diff_longitud": resultado.diff_longitud,
        "errores": resultado.errores.iter().map(|e| {
            serde_json::json!({
                "posicion": e.posicion,
                "esperado": e.esperado.to_string(),
                "recibido": e.recibido.to_string(),
            })
        }).collect::<Vec<_>>(),
    }))
}

fn cmd_contras_eliminar(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?;
        let antes = state.contrasenias.entradas.len();
        state.contrasenias.entradas.retain(|e| e.id != id);
        if state.contrasenias.entradas.len() < antes {
            state.guardar()?;
            Ok("Eliminada")
        } else {
            Err("No encontrada".to_string())
        }
    })
}

// ── Memoria ──────────────────────────────────────────────────

fn cmd_memoria_listar() -> String {
    with_state(|state| Ok(serde_json::to_value(&state.memoria.recuerdos).unwrap()))
}

fn cmd_memoria_agregar(params: &Value) -> String {
    use crate::memoria::Recuerdo;

    with_state(|state| {
        let contenido = params
            .get("contenido")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'contenido'")?
            .to_string();
        let palabras: Vec<String> = contenido
            .split_whitespace()
            .map(|w| w.to_lowercase())
            .collect();

        state.memoria.recuerdos.push(Recuerdo {
            id: uuid::Uuid::new_v4().to_string(),
            contenido,
            palabras_clave: palabras,
            modulo_origen: Some("ffi".to_string()),
            item_id: None,
            creado: chrono::Local::now().naive_local(),
        });
        state.guardar()?;
        Ok("Recuerdo guardado")
    })
}

fn cmd_memoria_eliminar(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?;
        let antes = state.memoria.recuerdos.len();
        state.memoria.recuerdos.retain(|r| r.id != id);
        if state.memoria.recuerdos.len() < antes {
            state.guardar()?;
            Ok("Eliminado")
        } else {
            Err("Recuerdo no encontrado".to_string())
        }
    })
}

// ── Sync ─────────────────────────────────────────────────────

fn cmd_sync_push() -> String {
    #[cfg(feature = "desktop")]
    {
        with_state(|state| {
            if !state.sync.gist_configurado() {
                return Err("Gist no configurado".to_string());
            }
            let json = serde_json::to_string_pretty(state).unwrap_or_default();
            match crate::sync::gist::gist_push(&state.sync, &json) {
                Ok(_) => Ok("Push exitoso"),
                Err(e) => Err(format!("Error push: {}", e)),
            }
        })
    }
    #[cfg(not(feature = "desktop"))]
    err_json("Sync no disponible en esta plataforma")
}

fn cmd_sync_pull() -> String {
    #[cfg(feature = "desktop")]
    {
        with_state(|state| {
            if !state.sync.gist_configurado() {
                return Err("Gist no configurado".to_string());
            }
            match crate::sync::gist::gist_pull(&state.sync) {
                Ok(json) => match serde_json::from_str::<AppState>(&json) {
                    Ok(nuevo) => {
                        *state = nuevo;
                        state.guardar()?;
                        Ok("Pull exitoso")
                    }
                    Err(e) => Err(format!("Error parseando datos remotos: {}", e)),
                },
                Err(e) => Err(format!("Error pull: {}", e)),
            }
        })
    }
    #[cfg(not(feature = "desktop"))]
    err_json("Sync no disponible en esta plataforma")
}

fn cmd_sync_config(params: &Value) -> String {
    with_state(|state| {
        if let Some(token) = params.get("gist_token").and_then(|v| v.as_str()) {
            state.sync.gist_token = token.to_string();
        }
        if let Some(gist_id) = params.get("gist_id").and_then(|v| v.as_str()) {
            state.sync.gist_id = gist_id.to_string();
        }
        if let Some(auto) = params.get("auto_sync").and_then(|v| v.as_bool()) {
            state.sync.auto_sync = auto;
        }
        state.guardar()?;
        Ok(serde_json::json!({
            "gist_configurado": state.sync.gist_configurado(),
            "auto_sync": state.sync.auto_sync,
        }))
    })
}

// ══════════════════════════════════════════════════════════════
//  Sistema — versión / búsqueda global
// ══════════════════════════════════════════════════════════════

fn cmd_version() -> String {
    let plataforma = if cfg!(feature = "android") {
        "android"
    } else if cfg!(feature = "desktop") {
        "desktop"
    } else {
        "lib"
    };
    ok_json(serde_json::json!({
        "nombre": "omniplanner",
        "version": env!("CARGO_PKG_VERSION"),
        "plataforma": plataforma,
    }))
}

fn cmd_buscar(params: &Value) -> String {
    with_state(|state| {
        let q = params
            .get("q")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'q'")?
            .to_lowercase();
        if q.trim().is_empty() {
            return Err("Consulta vacía".to_string());
        }
        let contiene = |s: &str| s.to_lowercase().contains(&q);

        let tareas: Vec<Value> = state
            .tasks
            .tareas
            .iter()
            .filter(|t| contiene(&t.titulo) || contiene(&t.descripcion))
            .map(|t| {
                serde_json::json!({
                    "id": t.id,
                    "titulo": t.titulo,
                    "fecha": t.fecha.to_string(),
                })
            })
            .collect();

        let eventos: Vec<Value> = state
            .agenda
            .eventos
            .iter()
            .filter(|e| contiene(&e.titulo) || contiene(&e.descripcion))
            .map(|e| {
                serde_json::json!({
                    "id": e.id,
                    "titulo": e.titulo,
                    "fecha": e.fecha.to_string(),
                })
            })
            .collect();

        let recuerdos: Vec<Value> = state
            .memoria
            .recuerdos
            .iter()
            .filter(|r| contiene(&r.contenido))
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "contenido": r.contenido,
                })
            })
            .collect();

        let contrasenias: Vec<Value> = state
            .contrasenias
            .entradas
            .iter()
            .filter(|e| contiene(&e.nombre) || contiene(&e.usuario))
            .map(|e| {
                serde_json::json!({
                    "id": e.id,
                    "nombre": e.nombre,
                    "usuario": e.usuario,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "q": q,
            "tareas": tareas,
            "eventos": eventos,
            "recuerdos": recuerdos,
            "contrasenias": contrasenias,
        }))
    })
}

// ══════════════════════════════════════════════════════════════
//  Canvas — board de ideas
// ══════════════════════════════════════════════════════════════

fn cmd_canvas_listar() -> String {
    with_state(|state| {
        let lista: Vec<Value> = state
            .canvases
            .iter()
            .enumerate()
            .map(|(i, c)| {
                serde_json::json!({
                    "indice": i,
                    "nombre": c.nombre,
                    "ancho": c.ancho,
                    "alto": c.alto,
                    "elementos": c.total_elementos(),
                })
            })
            .collect();
        Ok(serde_json::json!({ "canvases": lista }))
    })
}

fn cmd_canvas_crear(params: &Value) -> String {
    use crate::canvas::Canvas;
    with_state(|state| {
        let nombre = params
            .get("nombre")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'nombre'")?
            .to_string();
        let ancho = params.get("ancho").and_then(|v| v.as_u64()).unwrap_or(1024) as u32;
        let alto = params.get("alto").and_then(|v| v.as_u64()).unwrap_or(768) as u32;
        state.canvases.push(Canvas::new(nombre, ancho, alto));
        state.guardar()?;
        Ok(serde_json::json!({ "indice": state.canvases.len() - 1 }))
    })
}

fn cmd_canvas_detalle(params: &Value) -> String {
    with_state(|state| {
        let idx = params
            .get("indice")
            .and_then(|v| v.as_u64())
            .ok_or("Falta 'indice'")? as usize;
        let c = state.canvases.get(idx).ok_or("Canvas no encontrado")?;
        Ok(serde_json::to_value(c).unwrap())
    })
}

fn cmd_canvas_agregar_nota(params: &Value) -> String {
    use crate::canvas::Elemento;
    with_state(|state| {
        let idx = params
            .get("indice")
            .and_then(|v| v.as_u64())
            .ok_or("Falta 'indice'")? as usize;
        let contenido = params
            .get("contenido")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'contenido'")?
            .to_string();
        let color = params
            .get("color")
            .and_then(|v| v.as_str())
            .unwrap_or("amarillo")
            .to_string();
        let c = state.canvases.get_mut(idx).ok_or("Canvas no encontrado")?;
        let elem = Elemento::nota(contenido, color);
        let id = elem.id.clone();
        c.agregar_elemento(elem);
        state.guardar()?;
        Ok(serde_json::json!({ "id": id }))
    })
}

fn cmd_canvas_agregar_lista(params: &Value) -> String {
    use crate::canvas::Elemento;
    with_state(|state| {
        let idx = params
            .get("indice")
            .and_then(|v| v.as_u64())
            .ok_or("Falta 'indice'")? as usize;
        let contenido = params
            .get("contenido")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'contenido'")?
            .to_string();
        let color = params
            .get("color")
            .and_then(|v| v.as_str())
            .unwrap_or("azul")
            .to_string();
        let c = state.canvases.get_mut(idx).ok_or("Canvas no encontrado")?;
        let elem = Elemento::lista(contenido, color);
        let id = elem.id.clone();
        c.agregar_elemento(elem);
        state.guardar()?;
        Ok(serde_json::json!({ "id": id }))
    })
}

fn cmd_canvas_eliminar_elemento(params: &Value) -> String {
    with_state(|state| {
        let idx = params
            .get("indice")
            .and_then(|v| v.as_u64())
            .ok_or("Falta 'indice'")? as usize;
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?;
        let c = state.canvases.get_mut(idx).ok_or("Canvas no encontrado")?;
        if c.eliminar_elemento(id) {
            state.guardar()?;
            Ok("Elemento eliminado")
        } else {
            Err("Elemento no encontrado".to_string())
        }
    })
}

fn cmd_canvas_eliminar(params: &Value) -> String {
    with_state(|state| {
        let idx = params
            .get("indice")
            .and_then(|v| v.as_u64())
            .ok_or("Falta 'indice'")? as usize;
        if idx >= state.canvases.len() {
            return Err("Índice fuera de rango".to_string());
        }
        state.canvases.remove(idx);
        state.guardar()?;
        Ok("Canvas eliminado")
    })
}

// ══════════════════════════════════════════════════════════════
//  Diagramas
// ══════════════════════════════════════════════════════════════

fn tipo_diagrama_from_str(s: &str) -> crate::diagrams::TipoDiagrama {
    use crate::diagrams::TipoDiagrama;
    match s {
        "algoritmo" => TipoDiagrama::Algoritmo,
        "proceso" => TipoDiagrama::Proceso,
        "datos_flujo" | "datos" => TipoDiagrama::DatosFlujo,
        "libre" => TipoDiagrama::Libre,
        _ => TipoDiagrama::Flujo,
    }
}

fn tipo_nodo_from_str(s: &str) -> crate::diagrams::TipoNodo {
    use crate::diagrams::TipoNodo;
    match s {
        "inicio" => TipoNodo::Inicio,
        "fin" => TipoNodo::Fin,
        "decision" => TipoNodo::Decision,
        "entrada_salida" | "es" => TipoNodo::EntradaSalida,
        "conector" => TipoNodo::Conector,
        "subproceso" => TipoNodo::Subproceso,
        "dato" => TipoNodo::Dato,
        _ => TipoNodo::Proceso,
    }
}

fn cmd_diagrama_listar() -> String {
    with_state(|state| {
        let lista: Vec<Value> = state
            .diagramas
            .iter()
            .enumerate()
            .map(|(i, d)| {
                serde_json::json!({
                    "indice": i,
                    "nombre": d.nombre,
                    "nodos": d.nodos.len(),
                    "conexiones": d.conexiones.len(),
                })
            })
            .collect();
        Ok(serde_json::json!({ "diagramas": lista }))
    })
}

fn cmd_diagrama_crear(params: &Value) -> String {
    use crate::diagrams::Diagrama;
    with_state(|state| {
        let nombre = params
            .get("nombre")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'nombre'")?
            .to_string();
        let tipo = params
            .get("tipo")
            .and_then(|v| v.as_str())
            .unwrap_or("flujo");
        state
            .diagramas
            .push(Diagrama::new(nombre, tipo_diagrama_from_str(tipo)));
        state.guardar()?;
        Ok(serde_json::json!({ "indice": state.diagramas.len() - 1 }))
    })
}

fn cmd_diagrama_detalle(params: &Value) -> String {
    with_state(|state| {
        let idx = params
            .get("indice")
            .and_then(|v| v.as_u64())
            .ok_or("Falta 'indice'")? as usize;
        let d = state.diagramas.get(idx).ok_or("Diagrama no encontrado")?;
        Ok(serde_json::to_value(d).unwrap())
    })
}

fn cmd_diagrama_agregar_nodo(params: &Value) -> String {
    use crate::diagrams::Nodo;
    with_state(|state| {
        let idx = params
            .get("indice")
            .and_then(|v| v.as_u64())
            .ok_or("Falta 'indice'")? as usize;
        let tipo = params
            .get("tipo")
            .and_then(|v| v.as_str())
            .unwrap_or("proceso");
        let etiqueta = params
            .get("etiqueta")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'etiqueta'")?
            .to_string();
        let x = params.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let y = params.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let d = state
            .diagramas
            .get_mut(idx)
            .ok_or("Diagrama no encontrado")?;
        let id = d.agregar_nodo(Nodo::new(tipo_nodo_from_str(tipo), etiqueta, x, y));
        state.guardar()?;
        Ok(serde_json::json!({ "id": id }))
    })
}

fn cmd_diagrama_conectar(params: &Value) -> String {
    with_state(|state| {
        let idx = params
            .get("indice")
            .and_then(|v| v.as_u64())
            .ok_or("Falta 'indice'")? as usize;
        let origen = params
            .get("origen")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'origen'")?
            .to_string();
        let destino = params
            .get("destino")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'destino'")?
            .to_string();
        let etiqueta = params
            .get("etiqueta")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let tipo_con = match params
            .get("tipo")
            .and_then(|v| v.as_str())
            .unwrap_or("linea")
        {
            "flecha" => crate::diagrams::TipoConexion::Flecha,
            "condicional" => {
                crate::diagrams::TipoConexion::Condicional(etiqueta.clone().unwrap_or_default())
            }
            _ => crate::diagrams::TipoConexion::LineaRecta,
        };
        let d = state
            .diagramas
            .get_mut(idx)
            .ok_or("Diagrama no encontrado")?;
        d.conectar(&origen, &destino, tipo_con, etiqueta);
        state.guardar()?;
        Ok("Conexión creada")
    })
}

fn cmd_diagrama_eliminar(params: &Value) -> String {
    with_state(|state| {
        let idx = params
            .get("indice")
            .and_then(|v| v.as_u64())
            .ok_or("Falta 'indice'")? as usize;
        if idx >= state.diagramas.len() {
            return Err("Índice fuera de rango".to_string());
        }
        state.diagramas.remove(idx);
        state.guardar()?;
        Ok("Diagrama eliminado")
    })
}

fn cmd_diagrama_exportar_mermaid(params: &Value) -> String {
    with_state(|state| {
        let idx = params
            .get("indice")
            .and_then(|v| v.as_u64())
            .ok_or("Falta 'indice'")? as usize;
        let d = state.diagramas.get(idx).ok_or("Diagrama no encontrado")?;
        Ok(serde_json::json!({ "mermaid": d.exportar_mermaid() }))
    })
}

// ══════════════════════════════════════════════════════════════
//  Mapper — codificar / decodificar
// ══════════════════════════════════════════════════════════════

fn cmd_mapper_codificar(params: &Value) -> String {
    use crate::mapper::{Codificacion, Mapper};
    let datos = match params.get("datos").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return err_json("Falta 'datos'"),
    };
    let formato = params
        .get("formato")
        .and_then(|v| v.as_str())
        .unwrap_or("base64");
    let cod = match formato {
        "hex" => Codificacion::Hex,
        "binario" => Codificacion::Binario,
        "utf8" => Codificacion::Utf8,
        "json" => Codificacion::Json,
        "csv" => Codificacion::Csv,
        _ => Codificacion::Base64,
    };
    ok_json(serde_json::json!({
        "formato": formato,
        "resultado": Mapper::codificar(datos, &cod),
    }))
}

fn cmd_mapper_decodificar(params: &Value) -> String {
    use crate::mapper::Mapper;
    let datos = match params.get("datos").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return err_json("Falta 'datos'"),
    };
    let formato = params
        .get("formato")
        .and_then(|v| v.as_str())
        .unwrap_or("hex");
    let resultado = match formato {
        "hex" => Mapper::decodificar_hex(datos),
        _ => return err_json("Formato de decodificación no soportado (use 'hex')"),
    };
    match resultado {
        Some(texto) => ok_json(serde_json::json!({
            "formato": formato,
            "resultado": texto,
        })),
        None => err_json("No se pudo decodificar"),
    }
}

// ══════════════════════════════════════════════════════════════
//  VCS — snapshots del estado
// ══════════════════════════════════════════════════════════════

fn cmd_vcs_commit(params: &Value) -> String {
    with_state(|state| {
        let mensaje = params
            .get("mensaje")
            .and_then(|v| v.as_str())
            .unwrap_or("commit")
            .to_string();
        let autor = params
            .get("autor")
            .and_then(|v| v.as_str())
            .unwrap_or("omniplanner")
            .to_string();
        let datos = serde_json::to_string(&*state).map_err(|e| e.to_string())?;
        let id = state.vcs.commit(datos, mensaje, autor);
        state.guardar()?;
        Ok(serde_json::json!({ "id": id }))
    })
}

fn cmd_vcs_log() -> String {
    with_state(|state| {
        let log: Vec<Value> = state
            .vcs
            .log()
            .iter()
            .map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "hash": s.hash,
                    "mensaje": s.mensaje,
                    "autor": s.autor,
                    "timestamp": s.timestamp.to_string(),
                })
            })
            .collect();
        Ok(serde_json::json!({
            "rama_actual": state.vcs.rama_actual,
            "snapshots": log,
        }))
    })
}

// ══════════════════════════════════════════════════════════════
//  Conciliación Bancaria
// ══════════════════════════════════════════════════════════════

fn cmd_banco_listar() -> String {
    with_state(|state| {
        let cuentas: Vec<Value> = state
            .conciliacion
            .cuentas
            .iter()
            .map(|c| {
                serde_json::json!({
                    "id": c.id,
                    "banco": c.banco,
                    "nombre": c.nombre,
                    "tipo": c.tipo.nombre(),
                    "saldo_contable": c.saldo_contable,
                    "saldo_extracto": c.saldo_extracto,
                    "diferencia": c.diferencia(),
                    "activa": c.activa,
                    "tasa_rendimiento_anual": c.tasa_rendimiento_anual,
                })
            })
            .collect();
        Ok(serde_json::json!({ "cuentas": cuentas }))
    })
}

fn cmd_banco_agregar_cuenta(params: &Value) -> String {
    use crate::ml::conciliacion_bancaria::{CuentaBancaria, TipoCuenta};

    with_state(|state| {
        let banco = params
            .get("banco")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'banco'")?
            .to_string();
        let nombre = params
            .get("nombre")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'nombre'")?
            .to_string();
        let tipo = match params
            .get("tipo")
            .and_then(|v| v.as_str())
            .unwrap_or("corriente")
        {
            "ahorro" => TipoCuenta::CuentaAhorro,
            "tarjeta" => TipoCuenta::TarjetaCredito,
            "prestamo" => TipoCuenta::Prestamo,
            _ => TipoCuenta::CuentaCorriente,
        };
        let saldo_inicial = params
            .get("saldo_inicial")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let tasa = params
            .get("tasa_rendimiento_anual")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        let mut cuenta = CuentaBancaria::nueva(banco, nombre, tipo, saldo_inicial);
        cuenta.tasa_rendimiento_anual = tasa;
        let id = cuenta.id.clone();
        state.conciliacion.agregar_cuenta(cuenta);
        state.guardar()?;
        Ok(serde_json::json!({ "id": id }))
    })
}

fn cmd_banco_agregar_contable(params: &Value) -> String {
    use crate::ml::conciliacion_bancaria::MovimientoContable;
    use chrono::NaiveDate;

    with_state(|state| {
        let id_cuenta = params
            .get("id_cuenta")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id_cuenta'")?
            .to_string();
        let fecha = NaiveDate::parse_from_str(
            params.get("fecha").and_then(|v| v.as_str()).unwrap_or(""),
            "%Y-%m-%d",
        )
        .map_err(|_| "Fecha inválida (YYYY-MM-DD)")?;
        let descripcion = params
            .get("descripcion")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'descripcion'")?
            .to_string();
        let monto = params
            .get("monto")
            .and_then(|v| v.as_f64())
            .ok_or("Falta 'monto'")?;

        let mov = MovimientoContable::nuevo(fecha, descripcion, monto);
        let id_mov = mov.id.clone();
        let cuenta = state
            .conciliacion
            .cuenta_mut(&id_cuenta)
            .ok_or("Cuenta no encontrada")?;
        cuenta.registrar_contable(mov);
        state.guardar()?;
        Ok(serde_json::json!({ "id": id_mov }))
    })
}

fn cmd_banco_importar_extracto(params: &Value) -> String {
    use crate::ml::conciliacion_bancaria::MovimientoExtracto;
    use chrono::NaiveDate;

    with_state(|state| {
        let id_cuenta = params
            .get("id_cuenta")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id_cuenta'")?
            .to_string();
        let fecha = NaiveDate::parse_from_str(
            params.get("fecha").and_then(|v| v.as_str()).unwrap_or(""),
            "%Y-%m-%d",
        )
        .map_err(|_| "Fecha inválida (YYYY-MM-DD)")?;
        let descripcion = params
            .get("descripcion")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'descripcion'")?
            .to_string();
        let monto = params
            .get("monto")
            .and_then(|v| v.as_f64())
            .ok_or("Falta 'monto'")?;

        let mov = MovimientoExtracto::nuevo(fecha, descripcion, monto);
        let id_mov = mov.id.clone();
        let cuenta = state
            .conciliacion
            .cuenta_mut(&id_cuenta)
            .ok_or("Cuenta no encontrada")?;
        cuenta.importar_extracto(mov);
        state.guardar()?;
        Ok(serde_json::json!({ "id": id_mov }))
    })
}

fn cmd_banco_conciliar(params: &Value) -> String {
    use chrono::NaiveDate;

    with_state(|state| {
        let id_cuenta = params
            .get("id_cuenta")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id_cuenta'")?
            .to_string();
        let anio = params
            .get("anio")
            .and_then(|v| v.as_i64())
            .ok_or("Falta 'anio'")? as i32;
        let mes = params
            .get("mes")
            .and_then(|v| v.as_u64())
            .ok_or("Falta 'mes'")? as u32;
        let fecha_cierre = NaiveDate::parse_from_str(
            params
                .get("fecha_cierre")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            "%Y-%m-%d",
        )
        .unwrap_or_else(|_| chrono::Local::now().date_naive());

        let resultado = state
            .conciliacion
            .conciliar_cuenta(&id_cuenta, anio, mes, fecha_cierre)
            .ok_or("Cuenta no encontrada")?;

        let json = serde_json::json!({
            "id_cuenta": resultado.id_cuenta,
            "anio": resultado.anio,
            "mes": resultado.mes,
            "saldo_contable_fin": resultado.saldo_contable_fin,
            "saldo_extracto_fin": resultado.saldo_extracto_fin,
            "diferencia_total": resultado.diferencia_total(),
            "diferencia_inexplicada": resultado.diferencia_inexplicada,
            "conciliado": resultado.conciliado,
            "partidas_en_transito": resultado.partidas_en_transito.len(),
        });
        state.conciliacion.guardar_conciliacion(resultado);
        state.guardar()?;
        Ok(json)
    })
}

fn cmd_banco_ratios() -> String {
    with_state(|state| {
        let r = state.conciliacion.calcular_ratios();
        Ok(serde_json::json!({
            "liquidez_total": r.liquidez_total,
            "deuda_tarjetas": r.deuda_tarjetas,
            "deuda_prestamos": r.deuda_prestamos,
            "deuda_total": r.deuda_total,
            "ratio_deuda_liquidez": r.ratio_deuda_liquidez,
            "utilizacion_promedio_tarjetas_%": r.utilizacion_promedio_tarjetas * 100.0,
            "costo_financiero_mensual": r.costo_financiero_mensual,
        }))
    })
}

fn cmd_tarjeta_agregar(params: &Value) -> String {
    use crate::ml::conciliacion_bancaria::TarjetaCredito;

    with_state(|state| {
        let banco = params
            .get("banco")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'banco'")?
            .to_string();
        let nombre = params
            .get("nombre")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'nombre'")?
            .to_string();
        let cupo = params
            .get("cupo_total")
            .and_then(|v| v.as_f64())
            .ok_or("Falta 'cupo_total'")?;
        let tasa_mensual = params
            .get("tasa_mensual")
            .and_then(|v| v.as_f64())
            .ok_or("Falta 'tasa_mensual'")?;

        let mut tarjeta = TarjetaCredito::nueva(banco, nombre, cupo, tasa_mensual);
        if let Some(d) = params.get("dia_corte").and_then(|v| v.as_u64()) {
            tarjeta.dia_corte = d as u8;
        }
        if let Some(d) = params.get("dia_pago").and_then(|v| v.as_u64()) {
            tarjeta.dia_pago = d as u8;
        }
        if let Some(p) = params
            .get("porcentaje_pago_minimo")
            .and_then(|v| v.as_f64())
        {
            tarjeta.porcentaje_pago_minimo = p;
        }
        tarjeta.activa = true;
        let id = tarjeta.id.clone();
        state.conciliacion.agregar_tarjeta(tarjeta);
        state.guardar()?;
        Ok(serde_json::json!({ "id": id }))
    })
}

fn cmd_tarjeta_listar() -> String {
    with_state(|state| {
        let lista: Vec<Value> = state
            .conciliacion
            .tarjetas
            .iter()
            .map(|t| {
                serde_json::json!({
                    "id": t.id,
                    "banco": t.banco,
                    "nombre": t.nombre,
                    "cupo_total": t.cupo_total,
                    "saldo_utilizado": t.saldo_utilizado,
                    "cupo_disponible": t.cupo_disponible(),
                    "utilizacion_%": t.utilizacion() * 100.0,
                    "tasa_mensual_%": t.tasa_interes_mensual * 100.0,
                    "tasa_anual_%": t.tasa_interes_anual * 100.0,
                    "pago_minimo": t.pago_minimo(),
                    "interes_mensual": t.interes_mensual(),
                    "dia_corte": t.dia_corte,
                    "dia_pago": t.dia_pago,
                    "activa": t.activa,
                })
            })
            .collect();
        Ok(serde_json::json!({ "tarjetas": lista }))
    })
}

fn cmd_tarjeta_cargo(params: &Value) -> String {
    use crate::ml::conciliacion_bancaria::MovimientoContable;
    use chrono::NaiveDate;

    with_state(|state| {
        let id_tarjeta = params
            .get("id_tarjeta")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id_tarjeta'")?
            .to_string();
        let descripcion = params
            .get("descripcion")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'descripcion'")?
            .to_string();
        let monto = params
            .get("monto")
            .and_then(|v| v.as_f64())
            .ok_or("Falta 'monto'")?;
        let fecha = NaiveDate::parse_from_str(
            params.get("fecha").and_then(|v| v.as_str()).unwrap_or(""),
            "%Y-%m-%d",
        )
        .unwrap_or_else(|_| chrono::Local::now().date_naive());

        let mov = MovimientoContable::nuevo(fecha, descripcion, monto);
        let tarjeta = state
            .conciliacion
            .tarjetas
            .iter_mut()
            .find(|t| t.id == id_tarjeta)
            .ok_or("Tarjeta no encontrada")?;
        tarjeta.registrar_cargo(mov);
        let saldo = tarjeta.saldo_utilizado;
        let cupo = tarjeta.cupo_disponible();
        state.guardar()?;
        Ok(serde_json::json!({
            "saldo_utilizado": saldo,
            "cupo_disponible": cupo,
        }))
    })
}

fn cmd_prestamo_agregar(params: &Value) -> String {
    use crate::ml::conciliacion_bancaria::PrestamoRegistrado;
    use chrono::NaiveDate;

    with_state(|state| {
        let entidad = params
            .get("entidad")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'entidad'")?
            .to_string();
        let nombre = params
            .get("nombre")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'nombre'")?
            .to_string();
        let capital = params
            .get("capital")
            .and_then(|v| v.as_f64())
            .ok_or("Falta 'capital'")?;
        let tasa_mensual = params
            .get("tasa_mensual")
            .and_then(|v| v.as_f64())
            .ok_or("Falta 'tasa_mensual'")?;
        let cuotas = params
            .get("cuotas")
            .and_then(|v| v.as_u64())
            .ok_or("Falta 'cuotas'")? as u32;
        let fecha_inicio = NaiveDate::parse_from_str(
            params
                .get("fecha_inicio")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            "%Y-%m-%d",
        )
        .unwrap_or_else(|_| chrono::Local::now().date_naive());

        let prestamo =
            PrestamoRegistrado::nuevo(entidad, nombre, capital, tasa_mensual, cuotas, fecha_inicio);
        let id = prestamo.id.clone();
        let cuota_fija = prestamo
            .tabla_amortizacion
            .first()
            .map(|c| c.cuota_total)
            .unwrap_or(0.0);
        state.conciliacion.agregar_prestamo(prestamo);
        state.guardar()?;
        Ok(serde_json::json!({ "id": id, "cuota_mensual": cuota_fija }))
    })
}

fn cmd_prestamo_listar() -> String {
    with_state(|state| {
        let lista: Vec<Value> = state
            .conciliacion
            .prestamos
            .iter()
            .map(|p| {
                let cuota = p
                    .tabla_amortizacion
                    .first()
                    .map(|c| c.cuota_total)
                    .unwrap_or(0.0);
                serde_json::json!({
                    "id": p.id,
                    "entidad": p.entidad,
                    "nombre": p.nombre,
                    "capital_original": p.capital_original,
                    "saldo_pendiente": p.saldo_pendiente,
                    "tasa_mensual_%": p.tasa_mensual * 100.0,
                    "tasa_anual_%": p.tasa_anual * 100.0,
                    "cuota_mensual": cuota,
                    "cuotas_totales": p.cuotas_totales,
                    "cuotas_pagadas": p.cuotas_pagadas,
                    "cuotas_restantes": p.cuotas_restantes(),
                    "intereses_futuros": p.intereses_futuros(),
                    "total_pagado": p.total_pagado(),
                    "activo": p.activo,
                })
            })
            .collect();
        Ok(serde_json::json!({ "prestamos": lista }))
    })
}

fn cmd_prestamo_pagar_cuota(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let prestamo = state
            .conciliacion
            .prestamos
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or("Préstamo no encontrado")?;
        prestamo.registrar_pago_cuota();
        let resp = serde_json::json!({
            "cuotas_pagadas": prestamo.cuotas_pagadas,
            "saldo_pendiente": prestamo.saldo_pendiente,
            "cuotas_restantes": prestamo.cuotas_restantes(),
        });
        state.guardar()?;
        Ok(resp)
    })
}

// ══════════════════════════════════════════════════════════════
//  Balance General
// ══════════════════════════════════════════════════════════════

fn cmd_balance_nuevo(params: &Value) -> String {
    use crate::ml::balance_general::BalanceGeneral;
    use chrono::NaiveDate;

    with_state(|state| {
        let fecha_str = params
            .get("fecha_corte")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'fecha_corte'")?;
        let fecha = NaiveDate::parse_from_str(fecha_str, "%Y-%m-%d")
            .map_err(|_| "Fecha inválida (YYYY-MM-DD)")?;
        let balance = BalanceGeneral::nuevo(fecha);
        let id = balance.id.clone();
        state.balances.agregar(balance);
        state.guardar()?;
        Ok(serde_json::json!({ "id": id }))
    })
}

fn cmd_balance_agregar_partida(params: &Value) -> String {
    use crate::ml::balance_general::{
        ClaseActivo, ClasePasivo, ClasePatrimonio, PartidaActivo, PartidaPasivo, PartidaPatrimonio,
    };

    with_state(|state| {
        let id_balance = params
            .get("id_balance")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id_balance'")?
            .to_string();
        let tipo = params
            .get("tipo")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'tipo' (activo|pasivo|patrimonio)")?;
        let descripcion = params
            .get("descripcion")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'descripcion'")?
            .to_string();
        let monto = params
            .get("monto")
            .and_then(|v| v.as_f64())
            .ok_or("Falta 'monto'")?;
        let clase_str = params.get("clase").and_then(|v| v.as_str()).unwrap_or("");

        let balance = state
            .balances
            .balances
            .iter_mut()
            .find(|b| b.id == id_balance)
            .ok_or("Balance no encontrado")?;

        match tipo {
            "activo" => {
                let clase = match clase_str {
                    "cuentas_por_cobrar" => ClaseActivo::CuentasPorCobrar,
                    "inventario" => ClaseActivo::Inventario,
                    "gastos_prepagados" => ClaseActivo::GastosPrePagados,
                    "otro_corriente" => ClaseActivo::OtroActivoCorriente,
                    "propiedad" => ClaseActivo::PropiedadPlantaEquipo,
                    "intangible" => ClaseActivo::ActivoIntangible,
                    "inversion_lp" => ClaseActivo::InversionLargoPlazo,
                    "otro_no_corriente" => ClaseActivo::OtroActivoNoCorriente,
                    _ => ClaseActivo::EfectivoEquivalente,
                };
                balance.agregar_activo(PartidaActivo::nueva(clase, descripcion, monto));
            }
            "pasivo" => {
                let clase = match clase_str {
                    "cuentas_por_pagar" => ClasePasivo::CuentasPorPagar,
                    "deuda_cp" => ClasePasivo::DeudaCortoplazo,
                    "impuestos" => ClasePasivo::ImpuestosPorPagar,
                    "otro_corriente" => ClasePasivo::OtroPasivoCorriente,
                    "deuda_lp" => ClasePasivo::DeudaLargoPlazo,
                    "obligaciones_laborales" => ClasePasivo::ObligacionesLaborales,
                    _ => ClasePasivo::OtroPasivoNoCorriente,
                };
                balance.agregar_pasivo(PartidaPasivo::nueva(clase, descripcion, monto));
            }
            "patrimonio" => {
                let clase = match clase_str {
                    "capital" => ClasePatrimonio::CapitalSocial,
                    "utilidades" => ClasePatrimonio::UtilidadesRetenidas,
                    "reserva" => ClasePatrimonio::ReservaLegal,
                    _ => ClasePatrimonio::OtroPatrimonio,
                };
                balance.agregar_patrimonio(PartidaPatrimonio::nueva(clase, descripcion, monto));
            }
            _ => return Err("'tipo' debe ser activo, pasivo o patrimonio".to_string()),
        }

        state.guardar()?;
        Ok("Partida agregada")
    })
}

fn cmd_balance_listar() -> String {
    with_state(|state| {
        let lista: Vec<Value> = state
            .balances
            .balances
            .iter()
            .map(|b| {
                serde_json::json!({
                    "id": b.id,
                    "fecha_corte": b.fecha_corte.to_string(),
                    "total_activos": b.total_activos(),
                    "total_pasivos": b.total_pasivos(),
                    "total_patrimonio": b.total_patrimonio(),
                    "ecuacion_cuadra": b.ecuacion_cuadra(),
                })
            })
            .collect();
        Ok(serde_json::json!({ "balances": lista }))
    })
}

fn cmd_balance_detalle(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?;
        let b = state
            .balances
            .balances
            .iter()
            .find(|b| b.id == id)
            .ok_or("Balance no encontrado")?;
        Ok(serde_json::to_value(b).unwrap())
    })
}

fn cmd_balance_ratios(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?;
        let b = state
            .balances
            .balances
            .iter()
            .find(|b| b.id == id)
            .ok_or("Balance no encontrado")?;
        let r = b.ratios();
        let resumen: Vec<Value> = r
            .resumen()
            .into_iter()
            .map(|(k, v, s)| serde_json::json!({ "ratio": k, "valor": v, "estado": s }))
            .collect();
        Ok(serde_json::json!({
            "id": b.id,
            "fecha_corte": b.fecha_corte.to_string(),
            "razon_corriente": r.razon_corriente,
            "prueba_acida": r.prueba_acida,
            "razon_caja": r.razon_caja,
            "ratio_endeudamiento_%": r.ratio_endeudamiento * 100.0,
            "ratio_deuda_patrimonio": r.ratio_deuda_patrimonio,
            "apalancamiento": r.apalancamiento_financiero,
            "capital_trabajo": r.capital_trabajo,
            "resumen": resumen,
        }))
    })
}

// ══════════════════════════════════════════════════════════════
//  Estado de Resultados
// ══════════════════════════════════════════════════════════════

fn cmd_resultado_nuevo(params: &Value) -> String {
    use crate::ml::estado_resultados::EstadoResultados;
    use chrono::NaiveDate;

    with_state(|state| {
        let inicio = NaiveDate::parse_from_str(
            params
                .get("fecha_inicio")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            "%Y-%m-%d",
        )
        .map_err(|_| "fecha_inicio inválida (YYYY-MM-DD)")?;
        let fin = NaiveDate::parse_from_str(
            params
                .get("fecha_fin")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            "%Y-%m-%d",
        )
        .map_err(|_| "fecha_fin inválida (YYYY-MM-DD)")?;

        let er = EstadoResultados::nuevo(inicio, fin);
        let id = er.id.clone();
        state.resultados.agregar(er);
        state.guardar()?;
        Ok(serde_json::json!({ "id": id }))
    })
}

fn cmd_resultado_agregar_partida(params: &Value) -> String {
    use crate::ml::estado_resultados::{
        ClaseCosto, ClaseGasto, ClaseIngreso, PartidaCosto, PartidaGasto, PartidaIngreso,
    };

    with_state(|state| {
        let id_er = params
            .get("id_resultado")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id_resultado'")?
            .to_string();
        let tipo = params
            .get("tipo")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'tipo' (ingreso|costo|gasto)")?;
        let descripcion = params
            .get("descripcion")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'descripcion'")?
            .to_string();
        let monto = params
            .get("monto")
            .and_then(|v| v.as_f64())
            .ok_or("Falta 'monto'")?;
        let clase_str = params.get("clase").and_then(|v| v.as_str()).unwrap_or("");

        let er = state
            .resultados
            .estados
            .iter_mut()
            .find(|e| e.id == id_er)
            .ok_or("Estado de Resultados no encontrado")?;

        match tipo {
            "ingreso" => {
                let clase = match clase_str {
                    "financiero" => ClaseIngreso::IngresoFinanciero,
                    "otro" => ClaseIngreso::OtroIngreso,
                    _ => ClaseIngreso::IngresoOperacional,
                };
                er.agregar_ingreso(PartidaIngreso::nueva(clase, descripcion, monto));
            }
            "costo" => {
                let clase = match clase_str {
                    "servicio" => ClaseCosto::CostoServicio,
                    _ => ClaseCosto::CostoVentas,
                };
                er.agregar_costo(PartidaCosto::nueva(clase, descripcion, monto));
            }
            "gasto" => {
                let clase = match clase_str {
                    "ventas" => ClaseGasto::GastoVentas,
                    "depreciacion" => ClaseGasto::GastoDepreciacion,
                    "amortizacion" => ClaseGasto::GastoAmortizacion,
                    "financiero" => ClaseGasto::GastoFinanciero,
                    "impuesto" => ClaseGasto::Impuesto,
                    "otro" => ClaseGasto::OtroGasto,
                    _ => ClaseGasto::GastoAdministrativo,
                };
                er.agregar_gasto(PartidaGasto::nuevo(clase, descripcion, monto));
            }
            _ => return Err("'tipo' debe ser ingreso, costo o gasto".to_string()),
        }

        state.guardar()?;
        Ok("Partida agregada")
    })
}

fn cmd_resultado_listar() -> String {
    with_state(|state| {
        let lista: Vec<Value> = state
            .resultados
            .estados
            .iter()
            .map(|e| {
                serde_json::json!({
                    "id": e.id,
                    "fecha_inicio": e.fecha_inicio.to_string(),
                    "fecha_fin": e.fecha_fin.to_string(),
                    "ingresos_operacionales": e.ingresos_operacionales(),
                    "utilidad_bruta": e.utilidad_bruta(),
                    "ebit": e.ebit(),
                    "utilidad_neta": e.utilidad_neta(),
                })
            })
            .collect();
        Ok(serde_json::json!({ "estados": lista }))
    })
}

fn cmd_resultado_detalle(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?;
        let e = state
            .resultados
            .estados
            .iter()
            .find(|e| e.id == id)
            .ok_or("Estado de Resultados no encontrado")?;

        let cascada: Vec<Value> = e
            .vista_cascada()
            .into_iter()
            .map(|(etiqueta, valor, subtotal)| {
                serde_json::json!({ "linea": etiqueta, "monto": valor, "subtotal": subtotal })
            })
            .collect();

        Ok(serde_json::json!({
            "id": e.id,
            "fecha_inicio": e.fecha_inicio.to_string(),
            "fecha_fin": e.fecha_fin.to_string(),
            "cascada": cascada,
        }))
    })
}

fn cmd_resultado_ratios(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?;

        // Opcionalmente recibir activos y patrimonio del balance para ROA/ROE
        let activos = params
            .get("activos_totales")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let patrimonio = params
            .get("patrimonio")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        let e = state
            .resultados
            .estados
            .iter()
            .find(|e| e.id == id)
            .ok_or("Estado de Resultados no encontrado")?;

        let r = e.ratios(activos, patrimonio);
        let resumen: Vec<Value> = r
            .resumen()
            .into_iter()
            .map(|(k, v, s)| serde_json::json!({ "ratio": k, "valor": v, "estado": s }))
            .collect();

        Ok(serde_json::json!({
            "id": e.id,
            "margen_bruto_%": r.margen_bruto * 100.0,
            "margen_operacional_%": r.margen_operacional * 100.0,
            "margen_ebitda_%": r.margen_ebitda * 100.0,
            "margen_neto_%": r.margen_neto * 100.0,
            "roa_%": r.roa * 100.0,
            "roe_%": r.roe * 100.0,
            "cobertura_intereses": r.cobertura_intereses,
            "ebitda": r.ebitda,
            "utilidad_neta": r.utilidad_neta,
            "resumen": resumen,
        }))
    })
}

// ══════════════════════════════════════════════════════════════
//  Propuestas de Planes de Salud
// ══════════════════════════════════════════════════════════════

fn cmd_prop_listar() -> String {
    use chrono::Local;
    with_state(|state| {
        let hoy = Local::now().date_naive();
        let lista: Vec<Value> = state
            .propuestas
            .activas_por_urgencia()
            .iter()
            .map(|p| {
                serde_json::json!({
                    "id": p.id,
                    "nombre": p.nombre,
                    "cliente": p.cliente,
                    "vendedor": p.vendedor,
                    "estado": p.estado.nombre(),
                    "dias_restantes": p.dias_restantes(hoy),
                    "fecha_vencimiento": p.fecha_vencimiento.to_string(),
                    "progreso_%": p.progreso_pct(),
                    "sme_pendientes": p.sme_pendientes().len(),
                    "revisiones_pendientes": p.revisiones_pendientes().len(),
                    "escalaciones_abiertas": p.escalaciones_abiertas().len(),
                    "id_salesforce": p.id_salesforce,
                })
            })
            .collect();
        Ok(serde_json::json!({ "propuestas": lista }))
    })
}

fn cmd_prop_dashboard() -> String {
    use chrono::Local;
    with_state(|state| {
        let hoy = Local::now().date_naive();
        let d = state.propuestas.dashboard(hoy);
        Ok(serde_json::json!({
            "total_activas": d.total_activas,
            "vencen_en_7_dias": d.vencen_en_7_dias,
            "sme_pendientes": d.sme_pendientes,
            "revisiones_pendientes": d.revisiones_pendientes,
            "escalaciones_abiertas": d.escalaciones_abiertas,
            "acciones_pendientes": d.acciones_pendientes,
            "total_smes_registrados": d.total_smes_registrados,
        }))
    })
}

fn cmd_prop_crear(params: &Value) -> String {
    use crate::propuestas::Propuesta;
    use chrono::NaiveDate;

    with_state(|state| {
        let nombre = params
            .get("nombre")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'nombre'")?
            .to_string();
        let cliente = params
            .get("cliente")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'cliente'")?
            .to_string();
        let vendedor = params
            .get("vendedor")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let recibida = NaiveDate::parse_from_str(
            params
                .get("fecha_recibida")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            "%Y-%m-%d",
        )
        .unwrap_or_else(|_| chrono::Local::now().date_naive());
        let vencimiento = NaiveDate::parse_from_str(
            params
                .get("fecha_vencimiento")
                .and_then(|v| v.as_str())
                .ok_or("Falta 'fecha_vencimiento'")?,
            "%Y-%m-%d",
        )
        .map_err(|_| "fecha_vencimiento inválida (YYYY-MM-DD)")?;

        let mut prop = Propuesta::nueva(nombre, cliente, vendedor, recibida, vencimiento);
        if let Some(sf) = params.get("id_salesforce").and_then(|v| v.as_str()) {
            prop.id_salesforce = sf.to_string();
        }
        if let Some(ev) = params.get("estrategia_ventas").and_then(|v| v.as_str()) {
            prop.estrategia_ventas = ev.to_string();
        }
        let id = prop.id.clone();
        state.propuestas.agregar(prop);
        state.guardar()?;
        Ok(serde_json::json!({ "id": id }))
    })
}

fn cmd_prop_detalle(params: &Value) -> String {
    use chrono::Local;
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?;
        let hoy = Local::now().date_naive();
        let p = state
            .propuestas
            .propuesta(id)
            .ok_or("Propuesta no encontrada")?;
        let ver = p.verificar_estrategia();
        Ok(serde_json::json!({
            "id": p.id,
            "nombre": p.nombre,
            "cliente": p.cliente,
            "vendedor": p.vendedor,
            "estado": p.estado.nombre(),
            "fecha_recibida": p.fecha_recibida.to_string(),
            "fecha_vencimiento": p.fecha_vencimiento.to_string(),
            "dias_restantes": p.dias_restantes(hoy),
            "progreso_%": p.progreso_pct(),
            "estrategia_ventas": p.estrategia_ventas,
            "id_salesforce": p.id_salesforce,
            "secciones": p.secciones.len(),
            "hitos": p.timeline.len(),
            "reuniones": p.reuniones.len(),
            "sme_pendientes": p.sme_pendientes().len(),
            "revisiones_pendientes": p.revisiones_pendientes().len(),
            "escalaciones_abiertas": p.escalaciones_abiertas().len(),
            "acciones_pendientes": p.acciones_pendientes_total().len(),
            "verificacion_estrategia": {
                "definida": ver.estrategia_definida,
                "secciones_con_estrategia": ver.secciones_con_estrategia,
                "secciones_total": ver.secciones_total,
                "consistencia_%": ver.porcentaje_consistencia,
            },
            "notas": p.notas,
        }))
    })
}

fn cmd_prop_actualizar_estado(params: &Value) -> String {
    use crate::propuestas::EstadoPropuesta;
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let estado_str = params
            .get("estado")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'estado'")?;
        let nuevo_estado = match estado_str {
            "recibida" => EstadoPropuesta::Recibida,
            "kickoff_pendiente" => EstadoPropuesta::KickOffPendiente,
            "en_desarrollo" => EstadoPropuesta::EnDesarrollo,
            "revision_interna" => EstadoPropuesta::EnRevisionInterna,
            "revision_vendedor" => EstadoPropuesta::EnRevisionVendedor,
            "proofreading" => EstadoPropuesta::Proofreading,
            "lista_envio" => EstadoPropuesta::ListaParaEnvio,
            "enviada" => EstadoPropuesta::Enviada,
            "ganada" => EstadoPropuesta::Ganada,
            "perdida" => EstadoPropuesta::Perdida,
            "cancelada" => EstadoPropuesta::Cancelada,
            _ => return Err(format!("Estado desconocido: {}", estado_str)),
        };
        let p = state
            .propuestas
            .propuesta_mut(&id)
            .ok_or("Propuesta no encontrada")?;
        p.estado = nuevo_estado;
        if let Some(n) = params.get("notas").and_then(|v| v.as_str()) {
            p.notas = n.to_string();
        }
        state.guardar()?;
        Ok("Estado actualizado")
    })
}

fn cmd_prop_eliminar(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?;
        if state.propuestas.eliminar(id) {
            state.guardar()?;
            Ok("Eliminada")
        } else {
            Err("Propuesta no encontrada".to_string())
        }
    })
}

fn cmd_prop_agregar_seccion(params: &Value) -> String {
    use crate::propuestas::{SeccionPropuesta, TipoSeccion};
    with_state(|state| {
        let prop_id = params
            .get("prop_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'prop_id'")?
            .to_string();
        let tipo_str = params
            .get("tipo")
            .and_then(|v| v.as_str())
            .unwrap_or("otra");
        let descripcion = params
            .get("descripcion")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let responsable = params
            .get("responsable")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let tipo = match tipo_str {
            "pricing" => TipoSeccion::Pricing,
            "stop_loss" => TipoSeccion::StopLoss,
            "red" | "network" => TipoSeccion::Red,
            "gestion_cuidado" | "care_management" => TipoSeccion::GestionCuidado,
            "engagement_poblacion" | "population_engagement" => TipoSeccion::EngagementPoblacion,
            "estrategia_ventas" => TipoSeccion::EstrategiaVentas,
            "resumen_ejecutivo" => TipoSeccion::ResumenEjecutivo,
            "administrativa" => TipoSeccion::Administrativa,
            otro => TipoSeccion::Otra(otro.to_string()),
        };
        let mut seccion = SeccionPropuesta::nueva(tipo, descripcion, responsable);
        if let Some(b) = params
            .get("estrategia_ventas_presente")
            .and_then(|v| v.as_bool())
        {
            seccion.estrategia_ventas_presente = b;
        }
        if let Some(b) = params
            .get("estrategia_ventas_consistente")
            .and_then(|v| v.as_bool())
        {
            seccion.estrategia_ventas_consistente = b;
        }
        let sec_id = seccion.id.clone();
        let p = state
            .propuestas
            .propuesta_mut(&prop_id)
            .ok_or("Propuesta no encontrada")?;
        p.agregar_seccion(seccion);
        state.guardar()?;
        Ok(serde_json::json!({ "id": sec_id }))
    })
}

fn cmd_prop_actualizar_seccion(params: &Value) -> String {
    use crate::propuestas::EstadoSeccion;
    with_state(|state| {
        let prop_id = params
            .get("prop_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'prop_id'")?
            .to_string();
        let sec_id = params
            .get("sec_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'sec_id'")?
            .to_string();
        let p = state
            .propuestas
            .propuesta_mut(&prop_id)
            .ok_or("Propuesta no encontrada")?;
        let s = p
            .secciones
            .iter_mut()
            .find(|s| s.id == sec_id)
            .ok_or("Sección no encontrada")?;

        if let Some(estado_str) = params.get("estado").and_then(|v| v.as_str()) {
            s.estado = match estado_str {
                "pendiente" => EstadoSeccion::Pendiente,
                "en_proceso" => EstadoSeccion::EnProceso,
                "borrador" => EstadoSeccion::Borrador,
                "en_revision" => EstadoSeccion::EnRevision,
                "aprobada" => EstadoSeccion::Aprobada,
                "entregada" => EstadoSeccion::Entregada,
                _ => s.estado.clone(),
            };
        }
        if let Some(b) = params
            .get("estrategia_ventas_presente")
            .and_then(|v| v.as_bool())
        {
            s.estrategia_ventas_presente = b;
        }
        if let Some(b) = params
            .get("estrategia_ventas_consistente")
            .and_then(|v| v.as_bool())
        {
            s.estrategia_ventas_consistente = b;
        }
        if let Some(r) = params.get("responsable").and_then(|v| v.as_str()) {
            s.responsable = r.to_string();
        }
        if let Some(n) = params.get("notas").and_then(|v| v.as_str()) {
            s.notas = n.to_string();
        }
        if let Some(ref_val) = params.get("referencia").and_then(|v| v.as_str()) {
            s.referencias.push(ref_val.to_string());
        }
        if let Some(an) = params.get("analisis").and_then(|v| v.as_str()) {
            s.analisis_solicitados.push(an.to_string());
        }
        state.guardar()?;
        Ok("Sección actualizada")
    })
}

fn cmd_prop_verificar_estrategia(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?;
        let p = state
            .propuestas
            .propuesta(id)
            .ok_or("Propuesta no encontrada")?;
        let ver = p.verificar_estrategia();
        let secciones_detalle: Vec<Value> = p
            .secciones
            .iter()
            .map(|s| {
                serde_json::json!({
                    "seccion": s.tipo.nombre(),
                    "responsable": s.responsable,
                    "estrategia_presente": s.estrategia_ventas_presente,
                    "estrategia_consistente": s.estrategia_ventas_consistente,
                    "estado": s.estado.nombre(),
                })
            })
            .collect();
        Ok(serde_json::json!({
            "estrategia_ventas": p.estrategia_ventas,
            "estrategia_definida": ver.estrategia_definida,
            "comunicada_al_equipo": ver.comunicada_al_equipo,
            "secciones_con_estrategia": ver.secciones_con_estrategia,
            "secciones_total": ver.secciones_total,
            "consistencia_%": ver.porcentaje_consistencia,
            "detalle": secciones_detalle,
        }))
    })
}

fn cmd_prop_agregar_hito(params: &Value) -> String {
    use crate::propuestas::HitoTimeline;
    use chrono::NaiveDate;
    with_state(|state| {
        let prop_id = params
            .get("prop_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'prop_id'")?
            .to_string();
        let descripcion = params
            .get("descripcion")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'descripcion'")?
            .to_string();
        let fecha = NaiveDate::parse_from_str(
            params
                .get("fecha")
                .and_then(|v| v.as_str())
                .ok_or("Falta 'fecha'")?,
            "%Y-%m-%d",
        )
        .map_err(|_| "Fecha inválida")?;
        let hito = HitoTimeline::nuevo(descripcion, fecha);
        let hid = hito.id.clone();
        let p = state
            .propuestas
            .propuesta_mut(&prop_id)
            .ok_or("Propuesta no encontrada")?;
        p.agregar_hito(hito);
        state.guardar()?;
        Ok(serde_json::json!({ "id": hid }))
    })
}

fn cmd_prop_completar_hito(params: &Value) -> String {
    use chrono::NaiveDate;
    with_state(|state| {
        let prop_id = params
            .get("prop_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'prop_id'")?
            .to_string();
        let hito_id = params
            .get("hito_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'hito_id'")?
            .to_string();
        let fecha_real = NaiveDate::parse_from_str(
            params
                .get("fecha_real")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            "%Y-%m-%d",
        )
        .unwrap_or_else(|_| chrono::Local::now().date_naive());
        let p = state
            .propuestas
            .propuesta_mut(&prop_id)
            .ok_or("Propuesta no encontrada")?;
        let h = p
            .timeline
            .iter_mut()
            .find(|h| h.id == hito_id)
            .ok_or("Hito no encontrado")?;
        h.completado = true;
        h.fecha_real = Some(fecha_real);
        state.guardar()?;
        Ok("Hito completado")
    })
}

fn cmd_prop_agregar_reunion(params: &Value) -> String {
    use crate::propuestas::{ReunionProyecto, TipoReunion};
    use chrono::NaiveDate;
    with_state(|state| {
        let prop_id = params
            .get("prop_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'prop_id'")?
            .to_string();
        let tipo_str = params
            .get("tipo")
            .and_then(|v| v.as_str())
            .unwrap_or("estatus");
        let titulo = params
            .get("titulo")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'titulo'")?
            .to_string();
        let fecha = NaiveDate::parse_from_str(
            params
                .get("fecha")
                .and_then(|v| v.as_str())
                .ok_or("Falta 'fecha'")?,
            "%Y-%m-%d",
        )
        .map_err(|_| "Fecha inválida")?;
        let tipo = match tipo_str {
            "kickoff" | "kick_off" => TipoReunion::KickOff,
            "estrategia" => TipoReunion::Estrategia,
            "revision_borrador" => TipoReunion::RevisionBorrador,
            "cierre" => TipoReunion::Cierre,
            "escalacion" => TipoReunion::Escalacion,
            "estatus" => TipoReunion::Estatus,
            otro => TipoReunion::Otra(otro.to_string()),
        };
        let mut reunion = ReunionProyecto::nueva(prop_id.clone(), tipo, titulo, fecha);
        if let Some(arr) = params.get("participantes").and_then(|v| v.as_array()) {
            reunion.participantes = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
        if let Some(arr) = params.get("agenda").and_then(|v| v.as_array()) {
            reunion.agenda = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
        if let Some(acta) = params.get("acta_resumen").and_then(|v| v.as_str()) {
            reunion.acta_resumen = acta.to_string();
        }
        let rid = reunion.id.clone();
        let p = state
            .propuestas
            .propuesta_mut(&prop_id)
            .ok_or("Propuesta no encontrada")?;
        p.agregar_reunion(reunion);
        state.guardar()?;
        Ok(serde_json::json!({ "id": rid }))
    })
}

fn cmd_prop_agregar_accion(params: &Value) -> String {
    use crate::propuestas::PuntoAccion;
    use chrono::NaiveDate;
    with_state(|state| {
        let prop_id = params
            .get("prop_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'prop_id'")?
            .to_string();
        let reunion_id = params
            .get("reunion_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'reunion_id'")?
            .to_string();
        let descripcion = params
            .get("descripcion")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'descripcion'")?
            .to_string();
        let responsable = params
            .get("responsable")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let fecha_limite = NaiveDate::parse_from_str(
            params
                .get("fecha_limite")
                .and_then(|v| v.as_str())
                .ok_or("Falta 'fecha_limite'")?,
            "%Y-%m-%d",
        )
        .map_err(|_| "Fecha inválida")?;
        let accion = PuntoAccion::nuevo(descripcion, responsable, fecha_limite);
        let aid = accion.id.clone();
        let p = state
            .propuestas
            .propuesta_mut(&prop_id)
            .ok_or("Propuesta no encontrada")?;
        let r = p
            .reuniones
            .iter_mut()
            .find(|r| r.id == reunion_id)
            .ok_or("Reunión no encontrada")?;
        r.puntos_accion.push(accion);
        state.guardar()?;
        Ok(serde_json::json!({ "id": aid }))
    })
}

fn cmd_prop_completar_accion(params: &Value) -> String {
    with_state(|state| {
        let prop_id = params
            .get("prop_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'prop_id'")?
            .to_string();
        let reunion_id = params
            .get("reunion_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'reunion_id'")?
            .to_string();
        let accion_id = params
            .get("accion_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'accion_id'")?
            .to_string();
        let p = state
            .propuestas
            .propuesta_mut(&prop_id)
            .ok_or("Propuesta no encontrada")?;
        let r = p
            .reuniones
            .iter_mut()
            .find(|r| r.id == reunion_id)
            .ok_or("Reunión no encontrada")?;
        let a = r
            .puntos_accion
            .iter_mut()
            .find(|a| a.id == accion_id)
            .ok_or("Acción no encontrada")?;
        a.completado = true;
        state.guardar()?;
        Ok("Punto de acción completado")
    })
}

fn cmd_prop_enviar_recap(params: &Value) -> String {
    with_state(|state| {
        let prop_id = params
            .get("prop_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'prop_id'")?
            .to_string();
        let reunion_id = params
            .get("reunion_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'reunion_id'")?
            .to_string();
        let acta = params.get("acta").and_then(|v| v.as_str()).unwrap_or("");
        let p = state
            .propuestas
            .propuesta_mut(&prop_id)
            .ok_or("Propuesta no encontrada")?;
        let r = p
            .reuniones
            .iter_mut()
            .find(|r| r.id == reunion_id)
            .ok_or("Reunión no encontrada")?;
        if !acta.is_empty() {
            r.acta_resumen = acta.to_string();
        }
        r.recap_enviado = true;
        state.guardar()?;
        Ok("Recap marcado como enviado")
    })
}

fn cmd_sme_listar() -> String {
    with_state(|state| {
        let lista: Vec<Value> = state
            .propuestas
            .smes
            .iter()
            .map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "nombre": s.nombre,
                    "area": s.area_especialidad,
                    "email": s.email,
                    "empresa": s.empresa,
                    "activo": s.activo,
                })
            })
            .collect();
        Ok(serde_json::json!({ "smes": lista }))
    })
}

fn cmd_sme_agregar(params: &Value) -> String {
    use crate::propuestas::ContactoSME;
    with_state(|state| {
        let nombre = params
            .get("nombre")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'nombre'")?
            .to_string();
        let area = params
            .get("area")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'area'")?
            .to_string();
        let email = params
            .get("email")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let mut sme = ContactoSME::nuevo(nombre, area, email);
        if let Some(t) = params.get("telefono").and_then(|v| v.as_str()) {
            sme.telefono = t.to_string();
        }
        if let Some(e) = params.get("empresa").and_then(|v| v.as_str()) {
            sme.empresa = e.to_string();
        }
        if let Some(n) = params.get("notas").and_then(|v| v.as_str()) {
            sme.notas = n.to_string();
        }
        let id = sme.id.clone();
        state.propuestas.agregar_sme(sme);
        state.guardar()?;
        Ok(serde_json::json!({ "id": id }))
    })
}

fn cmd_sme_buscar_area(params: &Value) -> String {
    with_state(|state| {
        let area = params
            .get("area")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'area'")?;
        let lista: Vec<Value> = state
            .propuestas
            .smes_por_area(area)
            .iter()
            .map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "nombre": s.nombre,
                    "area": s.area_especialidad,
                    "email": s.email,
                    "empresa": s.empresa,
                })
            })
            .collect();
        Ok(serde_json::json!({ "smes": lista }))
    })
}

fn cmd_prop_solicitar_sme(params: &Value) -> String {
    use crate::propuestas::SolicitudSME;
    use chrono::NaiveDate;
    with_state(|state| {
        let prop_id = params
            .get("prop_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'prop_id'")?
            .to_string();
        let sme_id = params
            .get("sme_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'sme_id'")?
            .to_string();
        let pregunta = params
            .get("pregunta")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'pregunta'")?
            .to_string();
        let hoy = chrono::Local::now().date_naive();
        let limite = NaiveDate::parse_from_str(
            params
                .get("fecha_limite")
                .and_then(|v| v.as_str())
                .ok_or("Falta 'fecha_limite'")?,
            "%Y-%m-%d",
        )
        .map_err(|_| "Fecha inválida")?;
        let mut sol = SolicitudSME::nueva(prop_id.clone(), sme_id, pregunta, hoy, limite);
        if let Some(ctx) = params.get("contexto").and_then(|v| v.as_str()) {
            sol.contexto = ctx.to_string();
        }
        if let Some(sec) = params.get("seccion_destino").and_then(|v| v.as_str()) {
            sol.seccion_destino = sec.to_string();
        }
        sol.estado = crate::propuestas::EstadoSolicitudSME::Enviada;
        let sid = sol.id.clone();
        let p = state
            .propuestas
            .propuesta_mut(&prop_id)
            .ok_or("Propuesta no encontrada")?;
        p.agregar_solicitud_sme(sol);
        state.guardar()?;
        Ok(serde_json::json!({ "id": sid }))
    })
}

fn cmd_prop_responder_sme(params: &Value) -> String {
    with_state(|state| {
        let prop_id = params
            .get("prop_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'prop_id'")?
            .to_string();
        let sol_id = params
            .get("sol_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'sol_id'")?
            .to_string();
        let respuesta = params
            .get("respuesta")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'respuesta'")?
            .to_string();
        let p = state
            .propuestas
            .propuesta_mut(&prop_id)
            .ok_or("Propuesta no encontrada")?;
        let s = p
            .solicitudes_sme
            .iter_mut()
            .find(|s| s.id == sol_id)
            .ok_or("Solicitud no encontrada")?;
        s.respuesta = respuesta;
        s.estado = crate::propuestas::EstadoSolicitudSME::Respondida;
        s.fecha_respuesta = Some(chrono::Local::now().date_naive());
        state.guardar()?;
        Ok("Respuesta registrada")
    })
}

fn cmd_prop_solicitar_revision(params: &Value) -> String {
    use crate::propuestas::{SolicitudRevision, TipoRevisor};
    use chrono::NaiveDate;
    with_state(|state| {
        let prop_id = params
            .get("prop_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'prop_id'")?
            .to_string();
        let tipo_str = params
            .get("tipo_revisor")
            .and_then(|v| v.as_str())
            .unwrap_or("interno");
        let nombre = params
            .get("nombre_revisor")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let hoy = chrono::Local::now().date_naive();
        let limite = NaiveDate::parse_from_str(
            params
                .get("fecha_limite")
                .and_then(|v| v.as_str())
                .ok_or("Falta 'fecha_limite'")?,
            "%Y-%m-%d",
        )
        .map_err(|_| "Fecha inválida")?;
        let tipo = match tipo_str {
            "vendedor" => TipoRevisor::Vendedor,
            "editor" | "editor_estrategico" => TipoRevisor::EditorEstrategico,
            "interno" => TipoRevisor::Interno,
            otro => TipoRevisor::Otro(otro.to_string()),
        };
        let rev = SolicitudRevision::nueva(prop_id.clone(), tipo, nombre, hoy, limite);
        let rid = rev.id.clone();
        let p = state
            .propuestas
            .propuesta_mut(&prop_id)
            .ok_or("Propuesta no encontrada")?;
        p.agregar_revision(rev);
        state.guardar()?;
        Ok(serde_json::json!({ "id": rid }))
    })
}

fn cmd_prop_completar_revision(params: &Value) -> String {
    with_state(|state| {
        let prop_id = params
            .get("prop_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'prop_id'")?
            .to_string();
        let rev_id = params
            .get("rev_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'rev_id'")?
            .to_string();
        let p = state
            .propuestas
            .propuesta_mut(&prop_id)
            .ok_or("Propuesta no encontrada")?;
        let r = p
            .revisiones
            .iter_mut()
            .find(|r| r.id == rev_id)
            .ok_or("Revisión no encontrada")?;
        r.completada = true;
        r.fecha_completada = Some(chrono::Local::now().date_naive());
        if let Some(c) = params.get("comentarios").and_then(|v| v.as_str()) {
            r.comentarios = c.to_string();
        }
        state.guardar()?;
        Ok("Revisión completada")
    })
}

fn cmd_prop_escalar(params: &Value) -> String {
    use crate::propuestas::{EscalacionProblema, NivelEscalacion};
    with_state(|state| {
        let prop_id = params
            .get("prop_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'prop_id'")?
            .to_string();
        let descripcion = params
            .get("descripcion")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'descripcion'")?
            .to_string();
        let nivel_str = params
            .get("nivel")
            .and_then(|v| v.as_str())
            .unwrap_or("supervisor");
        let escalado_a = params
            .get("escalado_a")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let nivel = match nivel_str {
            "gerente" => NivelEscalacion::Gerente,
            "director" => NivelEscalacion::Director,
            "vp" => NivelEscalacion::VP,
            _ => NivelEscalacion::Supervisor,
        };
        let hoy = chrono::Local::now().date_naive();
        let esc = EscalacionProblema::nueva(prop_id.clone(), descripcion, nivel, escalado_a, hoy);
        let eid = esc.id.clone();
        let p = state
            .propuestas
            .propuesta_mut(&prop_id)
            .ok_or("Propuesta no encontrada")?;
        p.agregar_escalacion(esc);
        state.guardar()?;
        Ok(serde_json::json!({ "id": eid }))
    })
}

fn cmd_prop_resolver_escalacion(params: &Value) -> String {
    with_state(|state| {
        let prop_id = params
            .get("prop_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'prop_id'")?
            .to_string();
        let esc_id = params
            .get("esc_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'esc_id'")?
            .to_string();
        let p = state
            .propuestas
            .propuesta_mut(&prop_id)
            .ok_or("Propuesta no encontrada")?;
        let e = p
            .escalaciones
            .iter_mut()
            .find(|e| e.id == esc_id)
            .ok_or("Escalación no encontrada")?;
        e.resuelto = true;
        e.fecha_resolucion = Some(chrono::Local::now().date_naive());
        if let Some(r) = params.get("resolucion").and_then(|v| v.as_str()) {
            e.resolucion = r.to_string();
        }
        state.guardar()?;
        Ok("Escalación resuelta")
    })
}

fn cmd_prop_log_salesforce(params: &Value) -> String {
    use crate::propuestas::RegistroSalesforce;
    with_state(|state| {
        let prop_id = params
            .get("prop_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'prop_id'")?
            .to_string();
        let accion = params
            .get("accion")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'accion'")?
            .to_string();
        let usuario = params
            .get("usuario")
            .and_then(|v| v.as_str())
            .unwrap_or("sistema")
            .to_string();
        let oportunidad = params
            .get("oportunidad_sf")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let ahora = chrono::Local::now().naive_local();
        let entrada = RegistroSalesforce {
            id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
            propuesta_id: prop_id.clone(),
            oportunidad_sf: oportunidad,
            accion,
            fecha: ahora.date(),
            fecha_hora: ahora,
            usuario,
            notas: params
                .get("notas")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        };
        let lid = entrada.id.clone();
        let p = state
            .propuestas
            .propuesta_mut(&prop_id)
            .ok_or("Propuesta no encontrada")?;
        p.registrar_salesforce(entrada);
        state.guardar()?;
        Ok(serde_json::json!({ "id": lid }))
    })
}

// ══════════════════════════════════════════════════════════════
//  Tests
// ══════════════════════════════════════════════════════════════
//  JNI bindings para Android (feature "android")
// ══════════════════════════════════════════════════════════════

#[cfg(feature = "android")]
mod jni_bridge {
    use super::*;
    use jni::objects::{JClass, JString};
    use jni::sys::jstring;
    use jni::JNIEnv;

    /// # Safety
    /// Llamada desde JNI — `jsonRequest` es un jstring válido.
    #[no_mangle]
    pub unsafe extern "system" fn Java_com_omniplanner_app_OmniBridge_omni_1command(
        mut env: JNIEnv,
        _class: JClass,
        json_request: JString,
    ) -> jstring {
        let input: String = env
            .get_string(&json_request)
            .map(|s| s.into())
            .unwrap_or_default();

        let result = process_command(&input);

        env.new_string(result)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut())
    }
}

// ══════════════════════════════════════════════════════════════
//  Casos / Intake de Solicitudes de Servicio
// ══════════════════════════════════════════════════════════════

fn cmd_caso_metricas() -> String {
    use chrono::Local;
    with_state(|state| {
        let hoy = Local::now().date_naive();
        let m = state.casos.metricas(hoy);
        Ok(serde_json::json!({
            "total_activos": m.total_activos,
            "en_sla": m.en_sla,
            "fuera_sla": m.fuera_sla,
            "pendientes_info": m.pendientes_info,
            "en_revision_clinica": m.en_revision_clinica,
            "criticos": m.criticos,
            "urgentes": m.urgentes,
            "completados_hoy": m.completados_hoy,
        }))
    })
}

fn cmd_caso_cola() -> String {
    use chrono::Local;
    with_state(|state| {
        let hoy = Local::now().date_naive();
        let lista: Vec<Value> = state
            .casos
            .cola_trabajo(hoy)
            .iter()
            .map(|c| {
                serde_json::json!({
                    "id": c.id,
                    "numero": c.numero_caso,
                    "tipo": c.tipo.nombre(),
                    "estado": c.estado.nombre(),
                    "urgencia": c.urgencia.nombre(),
                    "paciente": c.paciente.nombre_completo(),
                    "id_miembro": c.paciente.id_miembro,
                    "dias_sla": c.dias_para_sla(hoy),
                    "checklist_%": c.checklist_pct(),
                    "listo_ruteo": c.listo_para_ruteo(),
                    "info_pendiente": c.info_pendiente().len(),
                    "cliente": c.id_cliente,
                    "asignado_a": c.asignado_a,
                })
            })
            .collect();
        Ok(serde_json::json!({ "cola": lista }))
    })
}

fn cmd_caso_crear(params: &Value) -> String {
    use crate::casos::{Caso, TipoSolicitud, UrgenciaCaso};
    use chrono::NaiveDate;
    with_state(|state| {
        let tipo_str = params
            .get("tipo")
            .and_then(|v| v.as_str())
            .unwrap_or("autorizacion_previa");
        let tipo = match tipo_str {
            "autorizacion_previa" => TipoSolicitud::AutorizacionPrevia,
            "referido" => TipoSolicitud::Referido,
            "continuacion" => TipoSolicitud::ContinuacionCuidado,
            "emergencia" => TipoSolicitud::Emergencia,
            "gap_cuidado" => TipoSolicitud::CierreGapCuidado,
            otro => TipoSolicitud::Otro(otro.to_string()),
        };
        let urgencia = match params
            .get("urgencia")
            .and_then(|v| v.as_str())
            .unwrap_or("rutina")
        {
            "urgente" => UrgenciaCaso::Urgente,
            "critico" => UrgenciaCaso::Critico,
            _ => UrgenciaCaso::Rutina,
        };
        let hoy = chrono::Local::now().date_naive();
        let fecha_recibida = NaiveDate::parse_from_str(
            params
                .get("fecha_recibida")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            "%Y-%m-%d",
        )
        .unwrap_or(hoy);
        let id_cliente = params
            .get("id_cliente")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let prefijo = match tipo_str {
            "referido" => "REF",
            "emergencia" => "EMR",
            _ => "AUT",
        };
        let numero = state.casos.siguiente_numero(prefijo);
        let mut caso = Caso::nuevo(numero.clone(), tipo, urgencia, fecha_recibida, id_cliente);
        if let Some(a) = params.get("asignado_a").and_then(|v| v.as_str()) {
            caso.asignado_a = a.to_string();
        }
        // Cargar checklist plantilla del cliente si existe
        if !caso.id_cliente.is_empty() {
            if let Some(cli) = state.casos.cliente(&caso.id_cliente) {
                let plantilla: Vec<_> = cli.checklist_plantilla.clone();
                for item_desc in plantilla {
                    caso.checklist
                        .push(crate::casos::ItemChecklist::nuevo(item_desc, true));
                }
            }
        }
        let id = caso.id.clone();
        state.casos.agregar(caso);
        state.guardar()?;
        Ok(serde_json::json!({ "id": id, "numero": numero }))
    })
}

fn cmd_caso_detalle(params: &Value) -> String {
    use chrono::Local;
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?;
        let hoy = Local::now().date_naive();
        let c = state.casos.caso(id).ok_or("Caso no encontrado")?;
        let faltan = c.campos_faltantes();
        Ok(serde_json::json!({
            "id": c.id,
            "numero": c.numero_caso,
            "tipo": c.tipo.nombre(),
            "estado": c.estado.nombre(),
            "urgencia": c.urgencia.nombre(),
            "paciente": {
                "nombre": c.paciente.nombre_completo(),
                "id_miembro": c.paciente.id_miembro,
                "dob": c.paciente.fecha_nacimiento,
                "telefono": c.paciente.telefono,
                "completo": c.paciente.completo(),
            },
            "seguro": {
                "aseguradora": c.seguro.aseguradora,
                "plan": c.seguro.plan,
                "poliza": c.seguro.numero_poliza,
                "verificado": c.seguro.autorizado_verificado,
                "completo": c.seguro.completo(),
            },
            "referido": {
                "medico_referidor": c.referido.medico_referidor,
                "especialidad": c.referido.especialidad_destino,
                "icd10": c.referido.diagnostico_icd10,
                "cpt": c.referido.procedimiento_cpt,
                "notas_clinicas": c.referido.notas_clinicas_adjuntas,
                "completo": c.referido.completo(),
            },
            "checklist_%": c.checklist_pct(),
            "checklist_requeridos_ok": c.checklist_requeridos_ok(),
            "listo_para_ruteo": c.listo_para_ruteo(),
            "campos_faltantes": faltan,
            "info_pendiente": c.info_pendiente().len(),
            "dias_sla": c.dias_para_sla(hoy),
            "fecha_limite_sla": c.fecha_limite_sla.to_string(),
            "equipo_clinico": c.equipo_clinico,
            "asignado_a": c.asignado_a,
            "cliente": c.id_cliente,
            "notas_total": c.notas.len(),
            "historial_total": c.historial.len(),
        }))
    })
}

fn cmd_caso_actualizar_estado(params: &Value) -> String {
    use crate::casos::EstadoCaso;
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let estado_str = params
            .get("estado")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'estado'")?;
        let nuevo = match estado_str {
            "recibido" => EstadoCaso::Recibido,
            "validando" => EstadoCaso::ValidandoDatos,
            "pendiente_info" => EstadoCaso::PendienteInformacion,
            "checklist_ok" => EstadoCaso::ChecklistCompleto,
            "en_clinico" => EstadoCaso::EnRevisionClinica,
            "aprobado" => EstadoCaso::Aprobado,
            "negado" => EstadoCaso::Negado,
            "cerrado" => EstadoCaso::Cerrado,
            "cancelado" => EstadoCaso::Cancelado,
            _ => return Err(format!("Estado desconocido: {}", estado_str)),
        };
        let usuario = params
            .get("usuario")
            .and_then(|v| v.as_str())
            .unwrap_or("sistema")
            .to_string();
        let ahora = chrono::Local::now().naive_local();
        let c = state.casos.caso_mut(&id).ok_or("Caso no encontrado")?;
        let desc = format!("Estado cambiado a: {}", nuevo.nombre());
        c.estado = nuevo;
        c.registrar_evento("cambio_estado", desc, usuario, ahora);
        state.guardar()?;
        Ok("Estado actualizado")
    })
}

fn cmd_caso_actualizar_paciente(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let c = state.casos.caso_mut(&id).ok_or("Caso no encontrado")?;
        if let Some(v) = params.get("nombre").and_then(|v| v.as_str()) {
            c.paciente.nombre = v.to_string();
        }
        if let Some(v) = params.get("apellido").and_then(|v| v.as_str()) {
            c.paciente.apellido = v.to_string();
        }
        if let Some(v) = params.get("dob").and_then(|v| v.as_str()) {
            c.paciente.fecha_nacimiento = v.to_string();
        }
        if let Some(v) = params.get("id_miembro").and_then(|v| v.as_str()) {
            c.paciente.id_miembro = v.to_string();
        }
        if let Some(v) = params.get("telefono").and_then(|v| v.as_str()) {
            c.paciente.telefono = v.to_string();
        }
        if let Some(v) = params.get("genero").and_then(|v| v.as_str()) {
            c.paciente.genero = v.to_string();
        }
        if let Some(v) = params.get("direccion").and_then(|v| v.as_str()) {
            c.paciente.direccion = v.to_string();
        }
        state.guardar()?;
        Ok("Datos del paciente actualizados")
    })
}

fn cmd_caso_actualizar_seguro(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let c = state.casos.caso_mut(&id).ok_or("Caso no encontrado")?;
        if let Some(v) = params.get("aseguradora").and_then(|v| v.as_str()) {
            c.seguro.aseguradora = v.to_string();
        }
        if let Some(v) = params.get("plan").and_then(|v| v.as_str()) {
            c.seguro.plan = v.to_string();
        }
        if let Some(v) = params.get("poliza").and_then(|v| v.as_str()) {
            c.seguro.numero_poliza = v.to_string();
        }
        if let Some(v) = params.get("grupo").and_then(|v| v.as_str()) {
            c.seguro.grupo = v.to_string();
        }
        if let Some(v) = params.get("vigencia_inicio").and_then(|v| v.as_str()) {
            c.seguro.vigencia_inicio = v.to_string();
        }
        if let Some(v) = params.get("vigencia_fin").and_then(|v| v.as_str()) {
            c.seguro.vigencia_fin = v.to_string();
        }
        if let Some(b) = params.get("verificado").and_then(|v| v.as_bool()) {
            c.seguro.autorizado_verificado = b;
        }
        state.guardar()?;
        Ok("Datos de seguro actualizados")
    })
}

fn cmd_caso_actualizar_referido(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let c = state.casos.caso_mut(&id).ok_or("Caso no encontrado")?;
        if let Some(v) = params.get("medico_referidor").and_then(|v| v.as_str()) {
            c.referido.medico_referidor = v.to_string();
        }
        if let Some(v) = params.get("npi_referidor").and_then(|v| v.as_str()) {
            c.referido.npi_referidor = v.to_string();
        }
        if let Some(v) = params.get("especialidad_destino").and_then(|v| v.as_str()) {
            c.referido.especialidad_destino = v.to_string();
        }
        if let Some(v) = params.get("medico_destino").and_then(|v| v.as_str()) {
            c.referido.medico_destino = v.to_string();
        }
        if let Some(v) = params.get("npi_destino").and_then(|v| v.as_str()) {
            c.referido.npi_destino = v.to_string();
        }
        if let Some(v) = params.get("icd10").and_then(|v| v.as_str()) {
            c.referido.diagnostico_icd10 = v.to_string();
        }
        if let Some(v) = params.get("descripcion_dx").and_then(|v| v.as_str()) {
            c.referido.descripcion_diagnostico = v.to_string();
        }
        if let Some(v) = params.get("cpt").and_then(|v| v.as_str()) {
            c.referido.procedimiento_cpt = v.to_string();
        }
        if let Some(v) = params.get("descripcion_px").and_then(|v| v.as_str()) {
            c.referido.descripcion_procedimiento = v.to_string();
        }
        if let Some(b) = params
            .get("notas_clinicas_adjuntas")
            .and_then(|v| v.as_bool())
        {
            c.referido.notas_clinicas_adjuntas = b;
        }
        state.guardar()?;
        Ok("Datos de referido actualizados")
    })
}

fn cmd_caso_agregar_checklist(params: &Value) -> String {
    use crate::casos::ItemChecklist;
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let desc = params
            .get("descripcion")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'descripcion'")?
            .to_string();
        let req = params
            .get("requerido")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let item = ItemChecklist::nuevo(desc, req);
        let iid = item.id.clone();
        let c = state.casos.caso_mut(&id).ok_or("Caso no encontrado")?;
        c.checklist.push(item);
        state.guardar()?;
        Ok(serde_json::json!({ "id": iid }))
    })
}

fn cmd_caso_completar_checklist(params: &Value) -> String {
    with_state(|state| {
        let caso_id = params
            .get("caso_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'caso_id'")?
            .to_string();
        let item_id = params
            .get("item_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'item_id'")?
            .to_string();
        let hoy = chrono::Local::now().date_naive();
        let c = state.casos.caso_mut(&caso_id).ok_or("Caso no encontrado")?;
        if c.completar_checklist_item(&item_id, hoy) {
            // Si todos los requeridos están completos, avanzar estado
            if c.checklist_requeridos_ok() && c.estado == crate::casos::EstadoCaso::ValidandoDatos {
                c.estado = crate::casos::EstadoCaso::ChecklistCompleto;
            }
            state.guardar()?;
            Ok("Ítem completado")
        } else {
            Err("Ítem no encontrado".to_string())
        }
    })
}

fn cmd_caso_agregar_nota(params: &Value) -> String {
    use crate::casos::NotaCaso;
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let texto = params
            .get("texto")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'texto'")?
            .to_string();
        let autor = params
            .get("autor")
            .and_then(|v| v.as_str())
            .unwrap_or("sistema")
            .to_string();
        let ahora = chrono::Local::now().naive_local();
        let nota = NotaCaso::nueva(texto, autor, ahora);
        let nid = nota.id.clone();
        let c = state.casos.caso_mut(&id).ok_or("Caso no encontrado")?;
        c.notas.push(nota);
        state.guardar()?;
        Ok(serde_json::json!({ "id": nid }))
    })
}

fn cmd_caso_rutear_clinico(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let equipo = params
            .get("equipo_clinico")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'equipo_clinico'")?
            .to_string();
        let usuario = params
            .get("usuario")
            .and_then(|v| v.as_str())
            .unwrap_or("sistema")
            .to_string();
        let ahora = chrono::Local::now().naive_local();
        let hoy = ahora.date();
        let c = state.casos.caso_mut(&id).ok_or("Caso no encontrado")?;
        if !c.listo_para_ruteo() {
            return Err(
                "El caso no está listo para ruteo: faltan datos o checklist incompleto".to_string(),
            );
        }
        c.equipo_clinico = equipo.clone();
        c.estado = crate::casos::EstadoCaso::EnRevisionClinica;
        c.fecha_ruteo_clinico = Some(hoy);
        let en_sla = c.dias_para_sla(hoy) >= 0;
        c.sla_cumplido = Some(en_sla);
        c.registrar_evento(
            "ruteo_clinico",
            format!("Ruteado a equipo: {}", equipo),
            usuario,
            ahora,
        );
        state.guardar()?;
        Ok(serde_json::json!({ "en_sla": en_sla }))
    })
}

fn cmd_caso_outreach_info(params: &Value) -> String {
    use crate::casos::{MetodoContacto, SolicitudInfoFaltante};
    use chrono::NaiveDate;
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let campo = params
            .get("campo_faltante")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'campo_faltante'")?
            .to_string();
        let contacto = params
            .get("contacto")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let metodo = match params
            .get("metodo")
            .and_then(|v| v.as_str())
            .unwrap_or("llamada")
        {
            "email" => MetodoContacto::Email,
            "fax" => MetodoContacto::Fax,
            "portal" => MetodoContacto::Portal,
            "correo" => MetodoContacto::Correo,
            _ => MetodoContacto::LlamadaTelefonica,
        };
        let hoy = chrono::Local::now().date_naive();
        let limite = NaiveDate::parse_from_str(
            params
                .get("fecha_limite")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            "%Y-%m-%d",
        )
        .unwrap_or(hoy + chrono::Duration::days(3));
        let sol = SolicitudInfoFaltante::nueva(id.clone(), campo, contacto, metodo, hoy, limite);
        let sid = sol.id.clone();
        let c = state.casos.caso_mut(&id).ok_or("Caso no encontrado")?;
        c.estado = crate::casos::EstadoCaso::PendienteInformacion;
        c.solicitudes_info.push(sol);
        state.guardar()?;
        Ok(serde_json::json!({ "id": sid }))
    })
}

fn cmd_caso_resolver_info(params: &Value) -> String {
    with_state(|state| {
        let caso_id = params
            .get("caso_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'caso_id'")?
            .to_string();
        let sol_id = params
            .get("sol_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'sol_id'")?
            .to_string();
        let hoy = chrono::Local::now().date_naive();
        let c = state.casos.caso_mut(&caso_id).ok_or("Caso no encontrado")?;
        let s = c
            .solicitudes_info
            .iter_mut()
            .find(|s| s.id == sol_id)
            .ok_or("Solicitud no encontrada")?;
        s.resuelto = true;
        s.fecha_resolucion = Some(hoy);
        if let Some(n) = params.get("notas").and_then(|v| v.as_str()) {
            s.notas = n.to_string();
        }
        // Si ya no quedan solicitudes pendientes, volver a estado de validación
        let pendientes = c.info_pendiente().len();
        if pendientes == 0 {
            c.estado = crate::casos::EstadoCaso::ValidandoDatos;
        }
        state.guardar()?;
        Ok("Información resuelta")
    })
}

fn cmd_caso_listos_ruteo() -> String {
    with_state(|state| {
        let lista: Vec<Value> = state
            .casos
            .listos_para_ruteo()
            .iter()
            .map(|c| {
                serde_json::json!({
                    "id": c.id,
                    "numero": c.numero_caso,
                    "paciente": c.paciente.nombre_completo(),
                    "tipo": c.tipo.nombre(),
                    "urgencia": c.urgencia.nombre(),
                    "cliente": c.id_cliente,
                })
            })
            .collect();
        Ok(serde_json::json!({ "listos": lista }))
    })
}

fn cmd_caso_requieren_outreach() -> String {
    with_state(|state| {
        let lista: Vec<Value> = state
            .casos
            .requieren_outreach()
            .iter()
            .map(|c| {
                serde_json::json!({
                    "id": c.id,
                    "numero": c.numero_caso,
                    "paciente": c.paciente.nombre_completo(),
                    "campos_faltantes": c.campos_faltantes(),
                    "solicitudes_pendientes": c.info_pendiente().len(),
                })
            })
            .collect();
        Ok(serde_json::json!({ "casos": lista }))
    })
}

fn cmd_cliente_agregar(params: &Value) -> String {
    use crate::casos::RequisitosCliente;
    with_state(|state| {
        let nombre = params
            .get("nombre")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'nombre'")?
            .to_string();
        let mut cli = RequisitosCliente::nuevo(nombre);
        if let Some(arr) = params.get("requisitos").and_then(|v| v.as_array()) {
            cli.requisitos = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
        if let Some(arr) = params.get("checklist_plantilla").and_then(|v| v.as_array()) {
            cli.checklist_plantilla = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
        if let Some(h) = params.get("sla_horas_rutina").and_then(|v| v.as_i64()) {
            cli.sla_horas_rutina = h;
        }
        if let Some(h) = params.get("sla_horas_urgente").and_then(|v| v.as_i64()) {
            cli.sla_horas_urgente = h;
        }
        let id = cli.id.clone();
        state.casos.agregar_cliente(cli);
        state.guardar()?;
        Ok(serde_json::json!({ "id": id }))
    })
}

fn cmd_cliente_listar() -> String {
    with_state(|state| {
        let lista: Vec<Value> = state
            .casos
            .clientes
            .iter()
            .map(|c| {
                serde_json::json!({
                    "id": c.id,
                    "nombre": c.nombre_cliente,
                    "sla_rutina_h": c.sla_horas_rutina,
                    "sla_urgente_h": c.sla_horas_urgente,
                    "requisitos": c.requisitos.len(),
                    "checklist_plantilla": c.checklist_plantilla.len(),
                })
            })
            .collect();
        Ok(serde_json::json!({ "clientes": lista }))
    })
}

fn cmd_cliente_detalle(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?;
        let c = state.casos.cliente(id).ok_or("Cliente no encontrado")?;
        Ok(serde_json::json!({
            "id": c.id,
            "nombre": c.nombre_cliente,
            "requisitos": c.requisitos,
            "politicas": c.politicas,
            "checklist_plantilla": c.checklist_plantilla,
            "sla_rutina_h": c.sla_horas_rutina,
            "sla_urgente_h": c.sla_horas_urgente,
            "notas_workflow": c.notas_workflow,
        }))
    })
}

// ══════════════════════════════════════════════════════════════
//  Proveedores / Outreach
// ══════════════════════════════════════════════════════════════

fn cmd_prov_listar() -> String {
    with_state(|state| {
        let lista: Vec<Value> = state
            .proveedores
            .proveedores
            .iter()
            .map(|p| {
                serde_json::json!({
                    "id": p.id,
                    "nombre": p.nombre,
                    "npi": p.npi,
                    "especialidad": p.especialidad,
                    "grupo": p.grupo_medico,
                    "telefono": p.telefono,
                    "engagement": p.nivel_engagement.nombre(),
                    "ultima_interaccion": p.ultima_interaccion.map(|d| d.to_string()),
                })
            })
            .collect();
        Ok(serde_json::json!({ "proveedores": lista }))
    })
}

fn cmd_prov_buscar(params: &Value) -> String {
    with_state(|state| {
        let q = params
            .get("q")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'q'")?;
        let lista: Vec<Value> = state
            .proveedores
            .buscar_por_nombre(q)
            .iter()
            .map(|p| {
                serde_json::json!({
                    "id": p.id,
                    "nombre": p.nombre,
                    "npi": p.npi,
                    "especialidad": p.especialidad,
                    "telefono": p.telefono,
                    "engagement": p.nivel_engagement.nombre(),
                })
            })
            .collect();
        Ok(serde_json::json!({ "resultados": lista }))
    })
}

fn cmd_prov_agregar(params: &Value) -> String {
    use crate::proveedores::Proveedor;
    with_state(|state| {
        let nombre = params
            .get("nombre")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'nombre'")?
            .to_string();
        let npi = params
            .get("npi")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let especialidad = params
            .get("especialidad")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let hoy = chrono::Local::now().date_naive();
        let mut p = Proveedor::nuevo(nombre, npi, especialidad, hoy);
        if let Some(v) = params.get("grupo").and_then(|v| v.as_str()) {
            p.grupo_medico = v.to_string();
        }
        if let Some(v) = params.get("telefono").and_then(|v| v.as_str()) {
            p.telefono = v.to_string();
        }
        if let Some(v) = params.get("fax").and_then(|v| v.as_str()) {
            p.fax = v.to_string();
        }
        if let Some(v) = params.get("email").and_then(|v| v.as_str()) {
            p.email = v.to_string();
        }
        if let Some(v) = params.get("direccion").and_then(|v| v.as_str()) {
            p.direccion = v.to_string();
        }
        let id = p.id.clone();
        state.proveedores.agregar(p);
        state.guardar()?;
        Ok(serde_json::json!({ "id": id }))
    })
}

fn cmd_prov_detalle(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?;
        let p = state
            .proveedores
            .proveedor(id)
            .ok_or("Proveedor no encontrado")?;
        let interacciones = state
            .proveedores
            .interacciones
            .iter()
            .filter(|i| i.proveedor_id == p.id)
            .count();
        Ok(serde_json::json!({
            "id": p.id,
            "nombre": p.nombre,
            "npi": p.npi,
            "especialidad": p.especialidad,
            "grupo": p.grupo_medico,
            "telefono": p.telefono,
            "fax": p.fax,
            "email": p.email,
            "direccion": p.direccion,
            "engagement": p.nivel_engagement.nombre(),
            "activo": p.activo,
            "ultima_interaccion": p.ultima_interaccion.map(|d| d.to_string()),
            "total_interacciones": interacciones,
            "notas": p.notas,
        }))
    })
}

fn cmd_prov_actualizar_engagement(params: &Value) -> String {
    use crate::proveedores::NivelEngagement;
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let nivel_str = params
            .get("nivel")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'nivel'")?;
        let nivel = match nivel_str {
            "activo" => NivelEngagement::Activo,
            "inactivo" => NivelEngagement::Inactivo,
            "no_participa" => NivelEngagement::NoParticipa,
            _ => NivelEngagement::Nuevo,
        };
        let p = state
            .proveedores
            .proveedor_mut(&id)
            .ok_or("Proveedor no encontrado")?;
        p.nivel_engagement = nivel;
        if let Some(n) = params.get("notas").and_then(|v| v.as_str()) {
            p.notas = n.to_string();
        }
        state.guardar()?;
        Ok("Engagement actualizado")
    })
}

fn cmd_prov_registrar_interaccion(params: &Value) -> String {
    use crate::proveedores::{InteraccionProveedor, ResultadoInteraccion, TipoInteraccion};
    with_state(|state| {
        let prov_id = params
            .get("prov_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'prov_id'")?
            .to_string();
        let tipo_str = params
            .get("tipo")
            .and_then(|v| v.as_str())
            .unwrap_or("llamada_saliente");
        let tipo = match tipo_str {
            "llamada_entrante" => TipoInteraccion::LlamadaEntrante,
            "email" => TipoInteraccion::Email,
            "fax" => TipoInteraccion::Fax,
            "portal" => TipoInteraccion::Portal,
            "visita" => TipoInteraccion::Visita,
            _ => TipoInteraccion::LlamadaSaliente,
        };
        let agente = params
            .get("agente")
            .and_then(|v| v.as_str())
            .unwrap_or("sistema")
            .to_string();
        let proposito = params
            .get("proposito")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let ahora = chrono::Local::now().naive_local();
        let hoy = ahora.date();
        let mut interaccion =
            InteraccionProveedor::nueva(prov_id, tipo, agente, proposito, hoy, ahora);
        let resultado_str = params
            .get("resultado")
            .and_then(|v| v.as_str())
            .unwrap_or("pendiente");
        interaccion.resultado = match resultado_str {
            "completado" => ResultadoInteraccion::Completado,
            "sin_contestacion" => ResultadoInteraccion::SinContestacion,
            "voicemail" => ResultadoInteraccion::Voicemail,
            "transferido" => ResultadoInteraccion::Transferido,
            "rechazado" => ResultadoInteraccion::Rechazado,
            "reprogramado" => ResultadoInteraccion::Reprogramado,
            _ => ResultadoInteraccion::PendienteRespuesta,
        };
        if let Some(d) = params.get("duracion_min").and_then(|v| v.as_u64()) {
            interaccion.duracion_min = d as u32;
        }
        if let Some(r) = params.get("resolucion").and_then(|v| v.as_str()) {
            interaccion.resolucion = r.to_string();
        }
        if let Some(b) = params
            .get("seguimiento_requerido")
            .and_then(|v| v.as_bool())
        {
            interaccion.seguimiento_requerido = b;
        }
        if let Some(f) = params.get("fecha_seguimiento").and_then(|v| v.as_str()) {
            interaccion.fecha_seguimiento = chrono::NaiveDate::parse_from_str(f, "%Y-%m-%d").ok();
        }
        if let Some(c) = params.get("caso_id").and_then(|v| v.as_str()) {
            interaccion.caso_id = c.to_string();
        }
        if let Some(c) = params.get("campana_id").and_then(|v| v.as_str()) {
            interaccion.campana_id = c.to_string();
        }
        let iid = interaccion.id.clone();
        state.proveedores.registrar_interaccion(interaccion);
        state.guardar()?;
        Ok(serde_json::json!({ "id": iid }))
    })
}

fn cmd_prov_interacciones(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?;
        let lista: Vec<Value> = state
            .proveedores
            .interacciones_de(id)
            .iter()
            .map(|i| {
                serde_json::json!({
                    "id": i.id,
                    "tipo": i.tipo.nombre(),
                    "fecha": i.fecha.to_string(),
                    "resultado": i.resultado.nombre(),
                    "proposito": i.proposito,
                    "resolucion": i.resolucion,
                    "duracion_min": i.duracion_min,
                    "agente": i.agente,
                    "seguimiento_requerido": i.seguimiento_requerido,
                    "fecha_seguimiento": i.fecha_seguimiento.map(|d| d.to_string()),
                })
            })
            .collect();
        Ok(serde_json::json!({ "interacciones": lista }))
    })
}

fn cmd_prov_agregar_seguimiento(params: &Value) -> String {
    use crate::proveedores::SeguimientoProveedor;
    use chrono::NaiveDate;
    with_state(|state| {
        let prov_id = params
            .get("prov_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'prov_id'")?
            .to_string();
        let motivo = params
            .get("motivo")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'motivo'")?
            .to_string();
        let agente = params
            .get("agente")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let fecha = NaiveDate::parse_from_str(
            params
                .get("fecha")
                .and_then(|v| v.as_str())
                .ok_or("Falta 'fecha'")?,
            "%Y-%m-%d",
        )
        .map_err(|_| "Fecha inválida")?;
        let seg = SeguimientoProveedor::nuevo(prov_id, fecha, motivo, agente);
        let sid = seg.id.clone();
        state.proveedores.agregar_seguimiento(seg);
        state.guardar()?;
        Ok(serde_json::json!({ "id": sid }))
    })
}

fn cmd_prov_seguimientos_pendientes() -> String {
    use chrono::Local;
    with_state(|state| {
        let hoy = Local::now().date_naive();
        let lista: Vec<Value> = state
            .proveedores
            .seguimientos_pendientes(hoy)
            .iter()
            .map(|s| {
                let p_nombre = state
                    .proveedores
                    .proveedor(&s.proveedor_id)
                    .map(|p| p.nombre.clone())
                    .unwrap_or_default();
                serde_json::json!({
                    "id": s.id,
                    "proveedor_id": s.proveedor_id,
                    "proveedor_nombre": p_nombre,
                    "fecha": s.fecha_programada.to_string(),
                    "dias_restantes": s.dias_restantes(hoy),
                    "motivo": s.motivo,
                    "agente": s.agente_asignado,
                })
            })
            .collect();
        // también los vencidos
        let vencidos: Vec<Value> = state
            .proveedores
            .seguimientos_vencidos(hoy)
            .iter()
            .map(|s| {
                let p_nombre = state
                    .proveedores
                    .proveedor(&s.proveedor_id)
                    .map(|p| p.nombre.clone())
                    .unwrap_or_default();
                serde_json::json!({
                    "id": s.id,
                    "proveedor_id": s.proveedor_id,
                    "proveedor_nombre": p_nombre,
                    "fecha": s.fecha_programada.to_string(),
                    "dias_vencido": -(s.dias_restantes(hoy)),
                    "motivo": s.motivo,
                })
            })
            .collect();
        Ok(serde_json::json!({ "pendientes": lista, "vencidos": vencidos }))
    })
}

fn cmd_prov_completar_seguimiento(params: &Value) -> String {
    with_state(|state| {
        let sid = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let resultado = params
            .get("resultado")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let hoy = chrono::Local::now().date_naive();
        let s = state
            .proveedores
            .seguimientos
            .iter_mut()
            .find(|s| s.id == sid)
            .ok_or("Seguimiento no encontrado")?;
        s.completado = true;
        s.fecha_completado = Some(hoy);
        s.resultado = resultado;
        state.guardar()?;
        Ok("Seguimiento completado")
    })
}

fn cmd_prov_sin_contacto(params: &Value) -> String {
    use chrono::Local;
    with_state(|state| {
        let dias = params.get("dias").and_then(|v| v.as_i64()).unwrap_or(30);
        let hoy = Local::now().date_naive();
        let lista: Vec<Value> = state
            .proveedores
            .sin_contacto_reciente(hoy, dias)
            .iter()
            .map(|p| {
                serde_json::json!({
                    "id": p.id,
                    "nombre": p.nombre,
                    "especialidad": p.especialidad,
                    "ultima_interaccion": p.ultima_interaccion.map(|d| d.to_string()),
                    "telefono": p.telefono,
                })
            })
            .collect();
        Ok(serde_json::json!({ "proveedores": lista, "umbral_dias": dias }))
    })
}

fn cmd_prov_metricas() -> String {
    use chrono::Local;
    with_state(|state| {
        let hoy = Local::now().date_naive();
        let m = state.proveedores.metricas(hoy);
        Ok(serde_json::json!({
            "total_proveedores": m.total_proveedores,
            "proveedores_activos": m.proveedores_activos,
            "total_interacciones": m.total_interacciones,
            "interacciones_exitosas": m.interacciones_exitosas,
            "tasa_contacto_%": m.tasa_contacto_pct,
            "seguimientos_pendientes": m.seguimientos_pendientes,
            "campanas_activas": m.campanas_activas,
        }))
    })
}

fn cmd_campana_crear(params: &Value) -> String {
    use crate::proveedores::CampanaOutreach;
    use chrono::NaiveDate;
    with_state(|state| {
        let nombre = params
            .get("nombre")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'nombre'")?
            .to_string();
        let proposito = params
            .get("proposito")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let inicio = NaiveDate::parse_from_str(
            params
                .get("fecha_inicio")
                .and_then(|v| v.as_str())
                .ok_or("Falta 'fecha_inicio'")?,
            "%Y-%m-%d",
        )
        .map_err(|_| "Fecha inicio inválida")?;
        let fin = NaiveDate::parse_from_str(
            params
                .get("fecha_fin")
                .and_then(|v| v.as_str())
                .ok_or("Falta 'fecha_fin'")?,
            "%Y-%m-%d",
        )
        .map_err(|_| "Fecha fin inválida")?;
        let mut camp = CampanaOutreach::nueva(nombre, proposito, inicio, fin);
        if let Some(arr) = params
            .get("proveedores_objetivo")
            .and_then(|v| v.as_array())
        {
            camp.proveedores_objetivo = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
        let id = camp.id.clone();
        state.proveedores.agregar_campana(camp);
        state.guardar()?;
        Ok(serde_json::json!({ "id": id }))
    })
}

fn cmd_campana_listar() -> String {
    with_state(|state| {
        let lista: Vec<Value> = state
            .proveedores
            .campanas
            .iter()
            .map(|c| {
                serde_json::json!({
                    "id": c.id,
                    "nombre": c.nombre,
                    "estado": c.estado.nombre(),
                    "inicio": c.fecha_inicio.to_string(),
                    "fin": c.fecha_fin.to_string(),
                    "objetivo": c.proveedores_objetivo.len(),
                    "tasa_contacto_%": c.metricas.tasa_contacto(),
                    "completados": c.metricas.completados,
                    "pendientes": c.metricas.pendientes,
                })
            })
            .collect();
        Ok(serde_json::json!({ "campanas": lista }))
    })
}

fn cmd_campana_actualizar(params: &Value) -> String {
    use crate::proveedores::EstadoCampana;
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let c = state
            .proveedores
            .campana_mut(&id)
            .ok_or("Campaña no encontrada")?;
        if let Some(s) = params.get("estado").and_then(|v| v.as_str()) {
            c.estado = match s {
                "activa" => EstadoCampana::Activa,
                "pausada" => EstadoCampana::Pausada,
                "completada" => EstadoCampana::Completada,
                "cancelada" => EstadoCampana::Cancelada,
                _ => EstadoCampana::Planificada,
            };
        }
        if let Some(n) = params.get("notas").and_then(|v| v.as_str()) {
            c.notas = n.to_string();
        }
        if let Some(v) = params.get("contactados").and_then(|v| v.as_u64()) {
            c.metricas.contactados = v as u32;
        }
        if let Some(v) = params.get("completados").and_then(|v| v.as_u64()) {
            c.metricas.completados = v as u32;
        }
        if let Some(v) = params.get("pendientes").and_then(|v| v.as_u64()) {
            c.metricas.pendientes = v as u32;
        }
        state.guardar()?;
        Ok("Campaña actualizada")
    })
}

// ══════════════════════════════════════════════════════════════
//  Obras y Ciclo Financiero
// ══════════════════════════════════════════════════════════════

fn cmd_obra_dashboard() -> String {
    with_state(|state| {
        let d = state.obras.dashboard();
        Ok(serde_json::json!({
            "total": d.total, "activas": d.activas,
            "completadas": d.completadas, "suspendidas": d.suspendidas,
            "valor_portafolio": d.valor_portafolio,
            "total_cobrado": d.total_cobrado,
            "pendiente_cobrar": d.pendiente_cobrar,
            "total_gastado": d.total_gastado,
            "saldo": d.saldo,
            "ciclos_intactos": d.ciclos_intactos,
            "ciclos_con_alerta": d.ciclos_con_alerta,
        }))
    })
}

fn cmd_obra_nueva(params: &Value) -> String {
    use crate::obras::Obra;
    with_state(|state| {
        let nombre = params
            .get("nombre")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'nombre'")?
            .to_string();
        let cliente = params
            .get("cliente")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'cliente'")?
            .to_string();
        let hoy = chrono::Local::now().date_naive();
        let mut obra = Obra::nueva(nombre, cliente, hoy);
        if let Some(v) = params.get("telefono_cliente").and_then(|v| v.as_str()) {
            obra.telefono_cliente = v.to_string();
        }
        if let Some(v) = params.get("email_cliente").and_then(|v| v.as_str()) {
            obra.email_cliente = v.to_string();
        }
        if let Some(v) = params.get("notas").and_then(|v| v.as_str()) {
            obra.notas = v.to_string();
        }
        let id = obra.id.clone();
        state.obras.agregar(obra);
        state.guardar()?;
        Ok(serde_json::json!({ "id": id }))
    })
}

fn cmd_obra_listar() -> String {
    with_state(|state| {
        let lista: Vec<Value> = state
            .obras
            .obras
            .iter()
            .map(|o| {
                serde_json::json!({
                    "id": o.id, "nombre": o.nombre, "cliente": o.cliente,
                    "estado": o.estado.nombre(), "pct_avance": o.pct_avance(),
                    "valor": o.contrato.valor_total, "cobrado": o.total_cobrado(),
                    "saldo": o.saldo_disponible(), "ciclo_ok": o.salud.ciclo_integro,
                })
            })
            .collect();
        Ok(serde_json::json!({ "obras": lista }))
    })
}

fn cmd_obra_detalle(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?;
        let o = state.obras.obra(id).ok_or("Obra no encontrada")?;
        let audit = o.auditoria();
        Ok(serde_json::json!({
            "id": o.id, "nombre": o.nombre, "cliente": o.cliente,
            "telefono": o.telefono_cliente, "email": o.email_cliente,
            "estado": o.estado.nombre(),
            "contrato": {
                "numero": o.contrato.numero, "valor": o.contrato.valor_total,
                "firmado": o.contrato.firmado,
                "pct_1er": o.contrato.pct_primer_desembolso,
                "pct_2do": o.contrato.pct_segundo_desembolso,
                "pct_final": o.contrato.pct_pago_final,
                "monto_1er": o.contrato.monto_primer(),
                "monto_2do": o.contrato.monto_segundo(),
                "monto_final": o.contrato.monto_final(),
            },
            "posicion": {
                "disponible": o.posicion.disponible,
                "exigible": o.posicion.exigible,
                "realizable": o.posicion.realizable,
                "total_activo": o.posicion.total_activo_corriente,
            },
            "pct_avance": o.pct_avance(),
            "cobrado": o.total_cobrado(), "gastado": o.total_gastado(),
            "saldo": o.saldo_disponible(),
            "desembolsos": o.desembolsos.len(),
            "consultas": o.consultas.len(),
            "consultas_pendientes": o.consultas.iter().filter(|c| !c.esta_aprobada()).count(),
            "gastos": o.gastos.len(),
            "cambios_alcance": o.cambios_alcance.len(),
            "reportes_avance": o.reportes_avance.len(),
            "ciclo_integro": o.salud.ciclo_integro,
            "alertas_ciclo": o.salud.alertas,
            "auditoria": {
                "cobertura_pct": audit.porcentaje_cobertura,
                "empresa_protegida": audit.empresa_protegida,
                "riesgo": audit.riesgo,
            },
        }))
    })
}

fn cmd_obra_estado(params: &Value) -> String {
    use crate::obras::EstadoObra;
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let estado_str = params
            .get("estado")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'estado'")?;
        let nuevo = match estado_str {
            "contacto" => EstadoObra::ContactoCliente,
            "correo" => EstadoObra::CorreoRequerimientos,
            "contrato_pendiente" => EstadoObra::ContratoPendiente,
            "contrato_firmado" => EstadoObra::ContratoFirmado,
            "1er_pendiente" => EstadoObra::PrimerDesembolsoPendiente,
            "1er_recibido" => EstadoObra::PrimerDesembolsoRecibido,
            "ejecucion" => EstadoObra::EnEjecucion,
            "2do_pendiente" => EstadoObra::SegundoDesembolsoPendiente,
            "2do_recibido" => EstadoObra::SegundoDesembolsoRecibido,
            "entrega_pendiente" => EstadoObra::EntregaPendiente,
            "entregada" => EstadoObra::Entregada,
            "completada" => EstadoObra::Completada,
            "suspendida" => EstadoObra::SuspendidaCliente,
            "cancelada" => EstadoObra::Cancelada,
            _ => return Err(format!("Estado desconocido: {}", estado_str)),
        };
        let o = state.obras.obra_mut(&id).ok_or("Obra no encontrada")?;
        o.estado = nuevo;
        state.guardar()?;
        Ok("Estado actualizado")
    })
}

fn cmd_obra_rfi(params: &Value) -> String {
    use crate::obras::RFI;
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let hoy = chrono::Local::now().date_naive();
        let canal = params
            .get("canal")
            .and_then(|v| v.as_str())
            .unwrap_or("email")
            .to_string();
        let desc = params
            .get("descripcion")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let mut rfi = RFI::nuevo(hoy, canal, desc);
        if let Some(arr) = params.get("necesidades").and_then(|v| v.as_array()) {
            rfi.necesidades = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
        if let Some(v) = params.get("urgencia").and_then(|v| v.as_str()) {
            rfi.urgencia = v.to_string();
        }
        let o = state.obras.obra_mut(&id).ok_or("Obra no encontrada")?;
        o.rfi = Some(rfi);
        if o.estado == crate::obras::EstadoObra::RFI {
            o.estado = crate::obras::EstadoObra::ContactoCliente;
        }
        state.guardar()?;
        Ok("RFI registrado")
    })
}

fn cmd_obra_contacto(params: &Value) -> String {
    use crate::obras::ContactoCliente;
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let hoy = chrono::Local::now().date_naive();
        let tipo = params
            .get("tipo")
            .and_then(|v| v.as_str())
            .unwrap_or("llamada")
            .to_string();
        let resumen = params
            .get("resumen")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let por = params
            .get("registrado_por")
            .and_then(|v| v.as_str())
            .unwrap_or("sistema")
            .to_string();
        let mut c = ContactoCliente::nuevo(hoy, tipo, resumen, por);
        if let Some(arr) = params.get("acuerdos").and_then(|v| v.as_array()) {
            c.acuerdos = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
        if let Some(v) = params.get("proxima_accion").and_then(|v| v.as_str()) {
            c.proxima_accion = v.to_string();
        }
        let cid = c.id.clone();
        let o = state.obras.obra_mut(&id).ok_or("Obra no encontrada")?;
        o.contactos.push(c);
        state.guardar()?;
        Ok(serde_json::json!({ "id": cid }))
    })
}

fn cmd_obra_correo_req(params: &Value) -> String {
    use crate::obras::CorreoRequerimiento;
    use chrono::NaiveDate;
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let hoy = chrono::Local::now().date_naive();
        let asunto = params
            .get("asunto")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'asunto'")?
            .to_string();
        let mut cr = CorreoRequerimiento::nuevo(hoy, asunto);
        if let Some(arr) = params.get("requerimientos").and_then(|v| v.as_array()) {
            cr.requerimientos = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
        if let Some(f) = params.get("plazo_respuesta").and_then(|v| v.as_str()) {
            cr.plazo_respuesta = NaiveDate::parse_from_str(f, "%Y-%m-%d").ok();
        }
        let o = state.obras.obra_mut(&id).ok_or("Obra no encontrada")?;
        o.correo_requerimiento = Some(cr);
        if o.estado == crate::obras::EstadoObra::ContactoCliente {
            o.estado = crate::obras::EstadoObra::CorreoRequerimientos;
        }
        state.guardar()?;
        Ok("Correo de requerimientos registrado")
    })
}

fn cmd_obra_contrato(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let o = state.obras.obra_mut(&id).ok_or("Obra no encontrada")?;
        if let Some(v) = params.get("numero").and_then(|v| v.as_str()) {
            o.contrato.numero = v.to_string();
        }
        if let Some(v) = params.get("valor_total").and_then(|v| v.as_f64()) {
            o.contrato.valor_total = v;
        }
        if let Some(v) = params.get("pct_primer").and_then(|v| v.as_f64()) {
            o.contrato.pct_primer_desembolso = v;
        }
        if let Some(v) = params.get("notas").and_then(|v| v.as_str()) {
            o.contrato.notas = v.to_string();
        }
        if let Some(arr) = params.get("condiciones").and_then(|v| v.as_array()) {
            o.contrato.condiciones = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
        if let Some(arr) = params.get("penalidades").and_then(|v| v.as_array()) {
            o.contrato.penalidades = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
        // Segundo desembolso siempre 80%, final siempre 20% (regla del sistema)
        o.contrato.pct_segundo_desembolso = 80.0;
        o.contrato.pct_pago_final = 20.0;
        if o.estado == crate::obras::EstadoObra::CorreoRequerimientos {
            o.estado = crate::obras::EstadoObra::ContratoPendiente;
        }
        state.guardar()?;
        Ok("Contrato configurado (2do=80%, final=20% — regla fija)")
    })
}

fn cmd_obra_contrato_firmar(params: &Value) -> String {
    use chrono::NaiveDate;
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let hoy = chrono::Local::now().date_naive();
        let fecha = NaiveDate::parse_from_str(
            params.get("fecha").and_then(|v| v.as_str()).unwrap_or(""),
            "%Y-%m-%d",
        )
        .unwrap_or(hoy);
        let o = state.obras.obra_mut(&id).ok_or("Obra no encontrada")?;
        o.contrato.firmado = true;
        o.contrato.fecha_firma = Some(fecha);
        o.estado = crate::obras::EstadoObra::ContratoFirmado;
        // Auto-crear desembolsos según el contrato
        let m1 = o.contrato.monto_primer();
        let m2 = o.contrato.monto_segundo();
        let m3 = o.contrato.monto_final();
        o.desembolsos.push(crate::obras::Desembolso::nuevo(
            crate::obras::NumeroDesembolso::Primero,
            m1,
            "Compra de materiales",
        ));
        o.desembolsos.push(crate::obras::Desembolso::nuevo(
            crate::obras::NumeroDesembolso::Segundo,
            m2,
            "Operativo + mano de obra (80%)",
        ));
        o.desembolsos.push(crate::obras::Desembolso::nuevo(
            crate::obras::NumeroDesembolso::Final,
            m3,
            "Solo impuestos (20%)",
        ));
        state.guardar()?;
        Ok(serde_json::json!({
            "firmado": true,
            "desembolsos_creados": 3,
            "monto_1er": m1, "monto_2do": m2, "monto_final": m3,
        }))
    })
}

fn cmd_obra_plazo_agregar(params: &Value) -> String {
    use crate::obras::PlazoContrato;
    use chrono::NaiveDate;
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let desc = params
            .get("descripcion")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'descripcion'")?
            .to_string();
        let fecha = NaiveDate::parse_from_str(
            params
                .get("fecha_limite")
                .and_then(|v| v.as_str())
                .ok_or("Falta 'fecha_limite'")?,
            "%Y-%m-%d",
        )
        .map_err(|_| "Fecha inválida")?;
        let p = PlazoContrato::nuevo(desc, fecha);
        let pid = p.id.clone();
        let o = state.obras.obra_mut(&id).ok_or("Obra no encontrada")?;
        o.contrato.plazos.push(p);
        state.guardar()?;
        Ok(serde_json::json!({ "id": pid }))
    })
}

fn cmd_obra_plazo_cumplir(params: &Value) -> String {
    with_state(|state| {
        let obra_id = params
            .get("obra_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'obra_id'")?
            .to_string();
        let plazo_id = params
            .get("plazo_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'plazo_id'")?
            .to_string();
        let hoy = chrono::Local::now().date_naive();
        let o = state.obras.obra_mut(&obra_id).ok_or("Obra no encontrada")?;
        let p = o
            .contrato
            .plazos
            .iter_mut()
            .find(|p| p.id == plazo_id)
            .ok_or("Plazo no encontrado")?;
        p.cumplido = true;
        p.fecha_cumplimiento = Some(hoy);
        state.guardar()?;
        Ok("Plazo marcado como cumplido")
    })
}

fn cmd_obra_posicion_contable(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let hoy = chrono::Local::now().date_naive();
        let o = state.obras.obra_mut(&id).ok_or("Obra no encontrada")?;
        if let Some(v) = params.get("disponible").and_then(|v| v.as_f64()) {
            o.posicion.disponible = v;
        }
        if let Some(v) = params.get("exigible").and_then(|v| v.as_f64()) {
            o.posicion.exigible = v;
        }
        if let Some(v) = params.get("realizable").and_then(|v| v.as_f64()) {
            o.posicion.realizable = v;
        }
        if let Some(v) = params.get("notas").and_then(|v| v.as_str()) {
            o.posicion.notas = v.to_string();
        }
        o.posicion.recalcular(hoy);
        let disp = o.posicion.disponible;
        let exig = o.posicion.exigible;
        let real = o.posicion.realizable;
        let total = o.posicion.total_activo_corriente;
        let _ = o;
        state.guardar()?;
        Ok(serde_json::json!({
            "disponible": disp,
            "exigible": exig,
            "realizable": real,
            "total_activo_corriente": total,
        }))
    })
}

fn cmd_obra_consulta_nueva(params: &Value) -> String {
    use crate::obras::ConsultaPrevia;
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let concepto = params
            .get("concepto")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'concepto'")?
            .to_string();
        let detalle = params
            .get("detalle")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let monto = params.get("monto").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let etapa = params
            .get("etapa")
            .and_then(|v| v.as_str())
            .unwrap_or("general")
            .to_string();
        let hoy = chrono::Local::now().date_naive();
        let c = ConsultaPrevia::nueva(hoy, concepto, detalle, monto, etapa);
        let cid = c.id.clone();
        let o = state.obras.obra_mut(&id).ok_or("Obra no encontrada")?;
        o.consultas.push(c);
        state.guardar()?;
        Ok(
            serde_json::json!({ "id": cid, "nota": "Esperando aprobación del cliente antes de proceder" }),
        )
    })
}

fn cmd_obra_consulta_responder(params: &Value) -> String {
    use crate::obras::EstadoConsulta;
    with_state(|state| {
        let obra_id = params
            .get("obra_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'obra_id'")?
            .to_string();
        let c_id = params
            .get("consulta_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'consulta_id'")?
            .to_string();
        let resp = params
            .get("respuesta")
            .and_then(|v| v.as_str())
            .unwrap_or("aprobada");
        let hoy = chrono::Local::now().date_naive();
        let o = state.obras.obra_mut(&obra_id).ok_or("Obra no encontrada")?;
        let c = o
            .consultas
            .iter_mut()
            .find(|c| c.id == c_id)
            .ok_or("Consulta no encontrada")?;
        c.estado = match resp {
            "rechazada" => EstadoConsulta::Rechazada,
            "modificada" => EstadoConsulta::ModificadaYAprobada,
            _ => EstadoConsulta::Aprobada,
        };
        c.fecha_respuesta = Some(hoy);
        if let Some(v) = params.get("aprobado_por").and_then(|v| v.as_str()) {
            c.aprobado_por = v.to_string();
        }
        if let Some(v) = params.get("medio").and_then(|v| v.as_str()) {
            c.medio_confirmacion = v.to_string();
        }
        if let Some(v) = params.get("respuesta_texto").and_then(|v| v.as_str()) {
            c.respuesta_cliente = v.to_string();
        }
        state.guardar()?;
        Ok("Respuesta registrada")
    })
}

fn cmd_obra_consultas_pendientes(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?;
        let o = state.obras.obra(id).ok_or("Obra no encontrada")?;
        let lista: Vec<Value> = o
            .consultas
            .iter()
            .filter(|c| !c.esta_aprobada())
            .map(|c| {
                serde_json::json!({
                    "id": c.id, "concepto": c.concepto,
                    "monto": c.monto_propuesto, "etapa": c.etapa,
                    "estado": c.estado.nombre(), "fecha": c.fecha.to_string(),
                })
            })
            .collect();
        Ok(serde_json::json!({ "pendientes": lista }))
    })
}

fn cmd_obra_desembolso_registrar(params: &Value) -> String {
    use chrono::NaiveDate;
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let numero_str = params
            .get("numero")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'numero'")?;
        let numero = match numero_str {
            "segundo" => crate::obras::NumeroDesembolso::Segundo,
            "final" => crate::obras::NumeroDesembolso::Final,
            _ => crate::obras::NumeroDesembolso::Primero,
        };
        let monto = params
            .get("monto")
            .and_then(|v| v.as_f64())
            .ok_or("Falta 'monto'")?;
        let hoy = chrono::Local::now().date_naive();
        let fecha = NaiveDate::parse_from_str(
            params.get("fecha").and_then(|v| v.as_str()).unwrap_or(""),
            "%Y-%m-%d",
        )
        .unwrap_or(hoy);
        let nuevo_estado = match &numero {
            crate::obras::NumeroDesembolso::Primero => {
                crate::obras::EstadoObra::PrimerDesembolsoRecibido
            }
            crate::obras::NumeroDesembolso::Segundo => {
                crate::obras::EstadoObra::SegundoDesembolsoRecibido
            }
            crate::obras::NumeroDesembolso::Final => crate::obras::EstadoObra::Entregada,
        };
        let o = state.obras.obra_mut(&id).ok_or("Obra no encontrada")?;
        // Marcar el desembolso correspondiente como recibido
        if let Some(des) = o.desembolsos.iter_mut().find(|d| d.numero == numero) {
            des.recibido = true;
            des.monto_recibido = monto;
            des.fecha_real = Some(fecha);
        }
        o.estado = nuevo_estado;
        let nombre_estado = o.estado.nombre().to_string();
        let _ = o;
        state.guardar()?;
        Ok(serde_json::json!({ "monto_recibido": monto, "nuevo_estado": nombre_estado }))
    })
}

fn cmd_obra_gasto_registrar(params: &Value) -> String {
    use crate::obras::{CategoriaGasto, GastoObra, NumeroDesembolso};
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let cat_str = params
            .get("categoria")
            .and_then(|v| v.as_str())
            .unwrap_or("operativo");
        let cat = match cat_str {
            "materiales" => CategoriaGasto::Materiales,
            "mano_obra" => CategoriaGasto::ManoObra,
            "viaticos" => CategoriaGasto::Viaticos,
            "representacion" => CategoriaGasto::GastosRepresentacion,
            "empleados" => CategoriaGasto::Empleados,
            "impuesto" => CategoriaGasto::Impuesto,
            "subcontratados" => CategoriaGasto::ServiciosSubcontratados,
            "equipos" => CategoriaGasto::EquiposAlquiler,
            otro => CategoriaGasto::Otro(otro.to_string()),
        };
        let des_str = params
            .get("desembolso")
            .and_then(|v| v.as_str())
            .unwrap_or("primero");
        let des = match des_str {
            "segundo" => NumeroDesembolso::Segundo,
            "final" => NumeroDesembolso::Final,
            _ => NumeroDesembolso::Primero,
        };
        if !cat.valido_para(&des) {
            return Err(format!(
                "La categoría '{}' no es válida para el desembolso '{}'. Regla del ciclo.",
                cat.nombre(),
                des.nombre()
            ));
        }
        let desc = params
            .get("descripcion")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'descripcion'")?
            .to_string();
        let monto = params
            .get("monto")
            .and_then(|v| v.as_f64())
            .ok_or("Falta 'monto'")?;
        let consulta_id = params
            .get("consulta_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let hoy = chrono::Local::now().date_naive();
        let mut g = GastoObra::nuevo(cat, desc, monto, hoy, des, consulta_id.clone());
        if let Some(v) = params.get("comprobante").and_then(|v| v.as_str()) {
            g.comprobante = v.to_string();
        }
        if let Some(v) = params.get("beneficiario").and_then(|v| v.as_str()) {
            g.beneficiario = v.to_string();
        }
        let gid = g.id.clone();
        let o = state.obras.obra_mut(&id).ok_or("Obra no encontrada")?;
        if consulta_id.is_empty() {
            return Err(
                "Debe indicar 'consulta_id' — ningún gasto sin aprobación previa del cliente"
                    .to_string(),
            );
        }
        o.gastos.push(g);
        let saldo = o.saldo_disponible();
        let _ = o;
        state.guardar()?;
        Ok(serde_json::json!({ "id": gid, "saldo_disponible": saldo }))
    })
}

fn cmd_obra_gastos_listar(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?;
        let o = state.obras.obra(id).ok_or("Obra no encontrada")?;
        let lista: Vec<Value> = o
            .gastos
            .iter()
            .map(|g| {
                serde_json::json!({
                    "id": g.id, "categoria": g.categoria.nombre(),
                    "descripcion": g.descripcion, "monto": g.monto,
                    "fecha": g.fecha.to_string(), "desembolso": g.desembolso_origen.nombre(),
                    "consulta_id": g.consulta_id, "aprobado": g.aprobado,
                    "comprobante": g.comprobante, "beneficiario": g.beneficiario,
                })
            })
            .collect();
        Ok(serde_json::json!({
            "gastos": lista,
            "total_1er": o.total_gastos_desembolso(&crate::obras::NumeroDesembolso::Primero),
            "total_2do": o.total_gastos_desembolso(&crate::obras::NumeroDesembolso::Segundo),
            "total_final": o.total_gastos_desembolso(&crate::obras::NumeroDesembolso::Final),
            "total": o.total_gastado(),
            "saldo": o.saldo_disponible(),
        }))
    })
}

fn cmd_obra_cambio_alcance(params: &Value) -> String {
    use crate::obras::CambioAlcance;
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let desc = params
            .get("descripcion")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'descripcion'")?
            .to_string();
        let solicitado = params
            .get("solicitado_por")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'solicitado_por'")?
            .to_string();
        let hoy = chrono::Local::now().date_naive();
        let mut c = CambioAlcance::nuevo(hoy, desc, solicitado);
        if let Some(v) = params.get("impacto_costo").and_then(|v| v.as_f64()) {
            c.impacto_costo_adicional = v;
        }
        if let Some(v) = params.get("impacto_plazo_dias").and_then(|v| v.as_i64()) {
            c.impacto_plazo_dias = v as i32;
        }
        let cid = c.id.clone();
        let o = state.obras.obra_mut(&id).ok_or("Obra no encontrada")?;
        o.cambios_alcance.push(c);
        state.guardar()?;
        Ok(
            serde_json::json!({ "id": cid, "nota": "Cambio documentado como solicitud del cliente — empresa protegida" }),
        )
    })
}

fn cmd_obra_cambio_aprobar(params: &Value) -> String {
    with_state(|state| {
        let obra_id = params
            .get("obra_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'obra_id'")?
            .to_string();
        let cambio_id = params
            .get("cambio_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'cambio_id'")?
            .to_string();
        let hoy = chrono::Local::now().date_naive();
        let o = state.obras.obra_mut(&obra_id).ok_or("Obra no encontrada")?;
        let c = o
            .cambios_alcance
            .iter_mut()
            .find(|c| c.id == cambio_id)
            .ok_or("Cambio no encontrado")?;
        c.aprobado_cliente = true;
        c.estado = crate::obras::EstadoCambio::Aprobado;
        c.fecha_aprobacion = Some(hoy);
        if let Some(v) = params.get("medio").and_then(|v| v.as_str()) {
            c.medio_aprobacion = v.to_string();
        }
        if let Some(v) = params.get("referencia").and_then(|v| v.as_str()) {
            c.referencia_documento = v.to_string();
        }
        state.guardar()?;
        Ok("Cambio aprobado por el cliente — documentado")
    })
}

fn cmd_obra_reporte_avance(params: &Value) -> String {
    use crate::obras::ReporteAvance;
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let pct = params
            .get("pct_completado")
            .and_then(|v| v.as_f64())
            .ok_or("Falta 'pct_completado'")?;
        let etapa = params
            .get("etapa_actual")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'etapa_actual'")?
            .to_string();
        let por = params
            .get("preparado_por")
            .and_then(|v| v.as_str())
            .unwrap_or("sistema")
            .to_string();
        let hoy = chrono::Local::now().date_naive();
        let mut r = ReporteAvance::nuevo(hoy, pct, etapa, por);
        if let Some(arr) = params.get("completadas").and_then(|v| v.as_array()) {
            r.actividades_completadas = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
        if let Some(arr) = params.get("pendientes").and_then(|v| v.as_array()) {
            r.actividades_pendientes = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
        if let Some(v) = params.get("gastos_a_fecha").and_then(|v| v.as_f64()) {
            r.gastos_a_fecha = v;
        }
        let rid = r.id.clone();
        let o = state.obras.obra_mut(&id).ok_or("Obra no encontrada")?;
        r.entregado_al_cliente = true;
        o.reportes_avance.push(r);
        state.guardar()?;
        Ok(
            serde_json::json!({ "id": rid, "nota": "Reporte registrado y marcado como entregado al cliente" }),
        )
    })
}

fn cmd_obra_reporte_confirmar(params: &Value) -> String {
    with_state(|state| {
        let obra_id = params
            .get("obra_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'obra_id'")?
            .to_string();
        let rep_id = params
            .get("reporte_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'reporte_id'")?
            .to_string();
        let o = state.obras.obra_mut(&obra_id).ok_or("Obra no encontrada")?;
        let r = o
            .reportes_avance
            .iter_mut()
            .find(|r| r.id == rep_id)
            .ok_or("Reporte no encontrado")?;
        r.confirmado_por_cliente = true;
        if let Some(v) = params.get("observaciones").and_then(|v| v.as_str()) {
            r.observaciones_cliente = v.to_string();
        }
        state.guardar()?;
        Ok("Reporte confirmado por el cliente")
    })
}

fn cmd_obra_ciclo_verificar(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let o = state.obras.obra_mut(&id).ok_or("Obra no encontrada")?;
        o.verificar_ciclo();
        Ok(serde_json::json!({
            "ciclo_integro": o.salud.ciclo_integro,
            "alertas": o.salud.alertas,
            "materiales_ejecutado": o.salud.materiales_ejecutado,
            "operativo_ejecutado": o.salud.operativo_ejecutado,
            "impuesto_reservado": o.salud.impuesto_reservado,
            "impuesto_pagado": o.salud.impuesto_pagado,
            "fondo_impuesto_intacto": o.salud.fondo_impuesto_intacto,
            "gastos_sin_consulta": o.salud.gastos_sin_consulta,
        }))
    })
}

fn cmd_obra_auditoria(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?;
        let o = state.obras.obra(id).ok_or("Obra no encontrada")?;
        let a = o.auditoria();
        Ok(serde_json::json!({
            "total_gastos": a.total_gastos,
            "gastos_con_consulta_previa": a.gastos_con_consulta,
            "porcentaje_cobertura": a.porcentaje_cobertura,
            "cambios_documentados": a.cambios_documentados,
            "cambios_aprobados_cliente": a.cambios_aprobados_cliente,
            "reportes_enviados": a.reportes_enviados,
            "reportes_confirmados": a.reportes_confirmados,
            "empresa_protegida": a.empresa_protegida,
            "nivel_riesgo": a.riesgo,
        }))
    })
}

// ══════════════════════════════════════════════════════════════
//  Cobranzas y Gestión de Cobro
// ══════════════════════════════════════════════════════════════

fn cmd_cobro_dashboard() -> String {
    with_state(|state| {
        let hoy = chrono::Local::now().date_naive();
        state
            .cobranzas
            .generar_alertas_automaticas(hoy, chrono::Local::now().naive_local());
        let d = state.cobranzas.dashboard(hoy);
        Ok(serde_json::json!({
            "total_por_cobrar": d.total_por_cobrar,
            "monto_vencido": d.monto_vencido,
            "por_vencer_7dias": d.monto_por_vencer_7dias,
            "cobrado_este_mes": d.monto_cobrado_mes,
            "cuentas_activas": d.cuentas_activas,
            "cuentas_vencidas": d.cuentas_vencidas,
            "alertas_criticas": d.alertas_criticas,
            "alertas_altas": d.alertas_altas,
            "alertas_pendientes": d.alertas_pendientes_total,
            "llamadas_hoy": d.llamadas_programadas_hoy,
            "llamadas_acordadas_cliente": d.llamadas_acordadas_cliente_pendientes,
            "eficiencia_cobro_%": d.eficiencia_pct,
        }))
    })
}

fn cmd_cobro_perfil_nuevo(params: &Value) -> String {
    use crate::cobranzas::PerfilCobranzaCliente;
    with_state(|state| {
        let nombre = params
            .get("nombre")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'nombre'")?
            .to_string();
        let hoy = chrono::Local::now().date_naive();
        let mut p = PerfilCobranzaCliente::nuevo(nombre, hoy);
        if let Some(v) = params.get("empresa").and_then(|v| v.as_str()) {
            p.empresa = v.to_string();
        }
        if let Some(v) = params.get("rut_cedula").and_then(|v| v.as_str()) {
            p.rut_cedula = v.to_string();
        }
        if let Some(v) = params.get("responsable_pago").and_then(|v| v.as_str()) {
            p.responsable_pago = v.to_string();
        }
        if let Some(v) = params.get("telefono").and_then(|v| v.as_str()) {
            p.telefono_principal = v.to_string();
        }
        if let Some(v) = params.get("telefono_alt").and_then(|v| v.as_str()) {
            p.telefono_alternativo = v.to_string();
        }
        if let Some(v) = params.get("whatsapp").and_then(|v| v.as_str()) {
            p.whatsapp = v.to_string();
        }
        if let Some(v) = params.get("email").and_then(|v| v.as_str()) {
            p.email = v.to_string();
        }
        if let Some(v) = params.get("banco").and_then(|v| v.as_str()) {
            p.banco = v.to_string();
        }
        if let Some(v) = params.get("numero_cuenta").and_then(|v| v.as_str()) {
            p.numero_cuenta = v.to_string();
        }
        if let Some(v) = params.get("dias_credito").and_then(|v| v.as_i64()) {
            p.dias_credito = v as i32;
        }
        if let Some(v) = params.get("horario_preferido").and_then(|v| v.as_str()) {
            p.horario_preferido_contacto = v.to_string();
        }
        let id = p.id.clone();
        state.cobranzas.agregar_perfil(p);
        state.guardar()?;
        Ok(serde_json::json!({ "id": id }))
    })
}

fn cmd_cobro_perfil_listar() -> String {
    with_state(|state| {
        let lista: Vec<Value> = state
            .cobranzas
            .perfiles
            .iter()
            .map(|p| {
                serde_json::json!({
                    "id": p.id, "nombre": p.nombre, "empresa": p.empresa,
                    "telefono": p.telefono_principal, "whatsapp": p.whatsapp,
                    "dias_credito": p.dias_credito, "historial": p.historial_pago.nombre(),
                })
            })
            .collect();
        Ok(serde_json::json!({ "perfiles": lista }))
    })
}

fn cmd_cobro_perfil_detalle(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?;
        let p = state.cobranzas.perfil(id).ok_or("Perfil no encontrado")?;
        Ok(serde_json::json!({
            "id": p.id, "nombre": p.nombre, "empresa": p.empresa,
            "rut_cedula": p.rut_cedula, "responsable_pago": p.responsable_pago,
            "telefono_principal": p.telefono_principal,
            "telefono_alternativo": p.telefono_alternativo,
            "whatsapp": p.whatsapp, "email": p.email,
            "email_facturacion": p.email_facturacion,
            "banco": p.banco, "tipo_cuenta": p.tipo_cuenta,
            "numero_cuenta": p.numero_cuenta, "titular_cuenta": p.titular_cuenta,
            "dias_credito": p.dias_credito,
            "horario_preferido": p.horario_preferido_contacto,
            "historial_pago": p.historial_pago.nombre(),
            "requiere_factura_previa": p.requiere_factura_previa,
            "requiere_orden_compra": p.requiere_orden_compra,
            "notas": p.notas,
        }))
    })
}

fn cmd_cobro_perfil_actualizar(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let hoy = chrono::Local::now().date_naive();
        let p = state
            .cobranzas
            .perfil_mut(&id)
            .ok_or("Perfil no encontrado")?;
        if let Some(v) = params.get("empresa").and_then(|v| v.as_str()) {
            p.empresa = v.to_string();
        }
        if let Some(v) = params.get("responsable_pago").and_then(|v| v.as_str()) {
            p.responsable_pago = v.to_string();
        }
        if let Some(v) = params.get("telefono").and_then(|v| v.as_str()) {
            p.telefono_principal = v.to_string();
        }
        if let Some(v) = params.get("telefono_alt").and_then(|v| v.as_str()) {
            p.telefono_alternativo = v.to_string();
        }
        if let Some(v) = params.get("whatsapp").and_then(|v| v.as_str()) {
            p.whatsapp = v.to_string();
        }
        if let Some(v) = params.get("banco").and_then(|v| v.as_str()) {
            p.banco = v.to_string();
        }
        if let Some(v) = params.get("numero_cuenta").and_then(|v| v.as_str()) {
            p.numero_cuenta = v.to_string();
        }
        if let Some(v) = params.get("dias_credito").and_then(|v| v.as_i64()) {
            p.dias_credito = v as i32;
        }
        if let Some(v) = params.get("notas").and_then(|v| v.as_str()) {
            p.notas = v.to_string();
        }
        p.ultima_actualizacion = Some(hoy);
        state.guardar()?;
        Ok("Perfil actualizado")
    })
}

fn cmd_cobro_cuenta_nueva(params: &Value) -> String {
    use crate::cobranzas::CuentaCobrar;
    use chrono::NaiveDate;
    with_state(|state| {
        let obra_id = params
            .get("obra_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let cliente = params
            .get("cliente")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'cliente'")?
            .to_string();
        let desc = params
            .get("descripcion")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'descripcion'")?
            .to_string();
        let monto = params
            .get("monto")
            .and_then(|v| v.as_f64())
            .ok_or("Falta 'monto'")?;
        let hoy = chrono::Local::now().date_naive();
        let emision = NaiveDate::parse_from_str(
            params
                .get("fecha_emision")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            "%Y-%m-%d",
        )
        .unwrap_or(hoy);
        let dias_credito = params
            .get("dias_credito")
            .and_then(|v| v.as_i64())
            .unwrap_or(30);
        let vencimiento = NaiveDate::parse_from_str(
            params
                .get("fecha_vencimiento")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            "%Y-%m-%d",
        )
        .unwrap_or(emision + chrono::Duration::days(dias_credito));
        let mut c = CuentaCobrar::nueva(obra_id, cliente, desc, monto, emision, vencimiento);
        if let Some(v) = params.get("numero_factura").and_then(|v| v.as_str()) {
            c.numero_factura = v.to_string();
        }
        if let Some(v) = params.get("perfil_id").and_then(|v| v.as_str()) {
            c.perfil_id = v.to_string();
        }
        let id = c.id.clone();
        state.cobranzas.agregar_cuenta(c);
        state.guardar()?;
        Ok(serde_json::json!({ "id": id, "vencimiento": vencimiento.to_string() }))
    })
}

fn cmd_cobro_cuenta_listar() -> String {
    with_state(|state| {
        let hoy = chrono::Local::now().date_naive();
        let lista: Vec<Value> = state
            .cobranzas
            .cuentas
            .iter()
            .map(|c| {
                serde_json::json!({
                    "id": c.id, "cliente": c.cliente,
                    "descripcion": c.descripcion, "numero_factura": c.numero_factura,
                    "total": c.monto_total, "cobrado": c.monto_cobrado,
                    "pendiente": c.monto_pendiente(),
                    "vencimiento": c.fecha_vencimiento.to_string(),
                    "estado": c.estado.nombre(),
                    "dias_mora": c.dias_mora(hoy),
                })
            })
            .collect();
        Ok(serde_json::json!({ "cuentas": lista }))
    })
}

fn cmd_cobro_cuenta_vencidas() -> String {
    with_state(|state| {
        let hoy = chrono::Local::now().date_naive();
        let lista: Vec<Value> = state
            .cobranzas
            .cuentas_vencidas(hoy)
            .iter()
            .map(|c| {
                serde_json::json!({
                    "id": c.id, "cliente": c.cliente,
                    "numero_factura": c.numero_factura,
                    "pendiente": c.monto_pendiente(),
                    "dias_mora": c.dias_mora(hoy),
                    "vencimiento": c.fecha_vencimiento.to_string(),
                })
            })
            .collect();
        Ok(serde_json::json!({ "vencidas": lista }))
    })
}

fn cmd_cobro_registrar_pago(params: &Value) -> String {
    use crate::cobranzas::{RegistroCobro, TipoCobro};
    with_state(|state| {
        let id = params
            .get("cuenta_id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'cuenta_id'")?
            .to_string();
        let monto = params
            .get("monto")
            .and_then(|v| v.as_f64())
            .ok_or("Falta 'monto'")?;
        let tipo = match params
            .get("tipo")
            .and_then(|v| v.as_str())
            .unwrap_or("transferencia")
        {
            "efectivo" => TipoCobro::Efectivo,
            "cheque" => TipoCobro::Cheque,
            "tarjeta" => TipoCobro::TarjetaCredito,
            "transferencia" => TipoCobro::Transferencia,
            otro => TipoCobro::Otro(otro.to_string()),
        };
        let referencia = params
            .get("referencia")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let por = params
            .get("registrado_por")
            .and_then(|v| v.as_str())
            .unwrap_or("sistema")
            .to_string();
        let hoy = chrono::Local::now().date_naive();
        let cobro = RegistroCobro::nuevo(hoy, monto, tipo, referencia, por);
        let cid = cobro.id.clone();
        let c = state
            .cobranzas
            .cuenta_mut(&id)
            .ok_or("Cuenta no encontrada")?;
        c.registrar_pago(cobro, hoy);
        let pendiente = c.monto_pendiente();
        let estado_nombre = c.estado.nombre().to_string();
        let pagada = pendiente <= 0.0;
        let _ = c;
        state.guardar()?;
        Ok(serde_json::json!({
            "id": cid, "pendiente": pendiente,
            "estado": estado_nombre, "pagada": pagada,
        }))
    })
}

fn cmd_cobro_alerta_nueva(params: &Value) -> String {
    use crate::cobranzas::{AlertaCobranza, Prioridad, TipoAlerta};
    with_state(|state| {
        let cliente = params
            .get("cliente")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'cliente'")?
            .to_string();
        let titulo = params
            .get("titulo")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'titulo'")?
            .to_string();
        let desc = params
            .get("descripcion")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let monto = params.get("monto").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let prio = match params
            .get("prioridad")
            .and_then(|v| v.as_str())
            .unwrap_or("media")
        {
            "critica" => Prioridad::Critica,
            "alta" => Prioridad::Alta,
            "baja" => Prioridad::Baja,
            _ => Prioridad::Media,
        };
        let tipo = match params
            .get("tipo")
            .and_then(|v| v.as_str())
            .unwrap_or("recordatorio")
        {
            "llamada" => TipoAlerta::LlamadaProgramada,
            "llamada_acordada" => TipoAlerta::LlamadaAcordadaCliente,
            "sin_contestacion" => TipoAlerta::LlamadaSinContestacion,
            "factura" => TipoAlerta::FacturaPendiente,
            "reunion" => TipoAlerta::ReunionPendiente,
            "pago_vencido" => TipoAlerta::PagoVencido,
            _ => TipoAlerta::RecordatorioGeneral,
        };
        let ahora = chrono::Local::now().naive_local();
        let mut a = AlertaCobranza::nueva(
            params
                .get("obra_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            params
                .get("cuenta_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            cliente,
            tipo,
            prio,
            titulo,
            desc,
            monto,
            ahora,
        );
        if let Some(v) = params.get("responsable").and_then(|v| v.as_str()) {
            a.responsable = v.to_string();
        }
        if let Some(v) = params.get("telefono").and_then(|v| v.as_str()) {
            a.telefono_contacto = v.to_string();
        }
        if let Some(v) = params.get("email").and_then(|v| v.as_str()) {
            a.email_contacto = v.to_string();
        }
        if let Some(v) = params.get("banco").and_then(|v| v.as_str()) {
            a.banco = v.to_string();
        }
        if let Some(v) = params.get("numero_cuenta").and_then(|v| v.as_str()) {
            a.numero_cuenta_banco = v.to_string();
        }
        if let Some(f) = params.get("fecha_vencimiento").and_then(|v| v.as_str()) {
            a.fecha_vencimiento = chrono::NaiveDate::parse_from_str(f, "%Y-%m-%d").ok();
        }
        let id = a.id.clone();
        state.cobranzas.agregar_alerta(a);
        state.guardar()?;
        Ok(serde_json::json!({ "id": id }))
    })
}

fn cmd_cobro_alertas_activas() -> String {
    with_state(|state| {
        let lista: Vec<Value> = state
            .cobranzas
            .alertas_activas()
            .iter()
            .map(|a| {
                serde_json::json!({
                    "id": a.id, "cliente": a.cliente, "titulo": a.titulo,
                    "tipo": a.tipo.nombre(), "prioridad": a.prioridad.nombre(),
                    "estado": a.estado.nombre(), "monto": a.monto_relacionado,
                    "vencimiento": a.fecha_vencimiento.map(|d| d.to_string()),
                    "telefono": a.telefono_contacto, "responsable": a.responsable,
                    "intentos": a.intentos.len(), "intentos_sin_exito": a.intentos_sin_exito(),
                    "reagendado_para": a.reagendado_para.map(|dt| dt.to_string()),
                })
            })
            .collect();
        Ok(serde_json::json!({ "alertas": lista, "total": lista.len() }))
    })
}

fn cmd_cobro_alertas_criticas() -> String {
    with_state(|state| {
        let lista: Vec<Value> = state
            .cobranzas
            .alertas_criticas()
            .iter()
            .map(|a| {
                serde_json::json!({
                    "id": a.id, "cliente": a.cliente, "titulo": a.titulo,
                    "monto": a.monto_relacionado, "estado": a.estado.nombre(),
                    "telefono": a.telefono_contacto, "banco": a.banco,
                    "numero_cuenta": a.numero_cuenta_banco, "accion": a.accion_requerida.nombre(),
                })
            })
            .collect();
        Ok(serde_json::json!({ "criticas": lista }))
    })
}

fn cmd_cobro_alerta_avanzar(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let ahora = chrono::Local::now().naive_local();
        let a = state
            .cobranzas
            .alerta_mut(&id)
            .ok_or("Alerta no encontrada")?;
        if !a.estado.puede_avanzar() {
            return Err("La alerta ya está en estado final".to_string());
        }
        let estado_prev = a.estado.nombre().to_string();
        a.avanzar_workflow(ahora);
        let estado_nuevo = a.estado.nombre().to_string();
        let _ = a;
        state.guardar()?;
        Ok(serde_json::json!({ "estado_anterior": estado_prev, "estado_nuevo": estado_nuevo }))
    })
}

fn cmd_cobro_alerta_completar(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let ahora = chrono::Local::now().naive_local();
        let a = state
            .cobranzas
            .alerta_mut(&id)
            .ok_or("Alerta no encontrada")?;
        a.marcar_completada(ahora);
        if let Some(v) = params.get("notas").and_then(|v| v.as_str()) {
            a.notas_gestion = v.to_string();
        }
        state.guardar()?;
        Ok("Alerta completada ✅")
    })
}

fn cmd_cobro_alerta_reagendar(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let motivo = params
            .get("motivo")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'motivo'")?
            .to_string();
        let fecha_str = params
            .get("nueva_fecha")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'nueva_fecha'")?;
        let hora_str = params
            .get("hora")
            .and_then(|v| v.as_str())
            .unwrap_or("09:00");
        let dt_str = format!("{} {}", fecha_str, hora_str);
        let nueva = chrono::NaiveDateTime::parse_from_str(&dt_str, "%Y-%m-%d %H:%M")
            .map_err(|_| "Formato inválido — usar 'YYYY-MM-DD' y 'HH:MM'")?;
        let a = state
            .cobranzas
            .alerta_mut(&id)
            .ok_or("Alerta no encontrada")?;
        a.reagendar(nueva, motivo);
        state.guardar()?;
        Ok(serde_json::json!({
            "reagendado_para": nueva.to_string(),
            "nota": "Recuerda agregar al calendario para que no se pierda esta llamada"
        }))
    })
}

fn cmd_cobro_alerta_cancelar(params: &Value) -> String {
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let motivo = params
            .get("motivo")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'motivo' — se requiere justificación")?
            .to_string();
        let a = state
            .cobranzas
            .alerta_mut(&id)
            .ok_or("Alerta no encontrada")?;
        a.cancelar(motivo);
        state.guardar()?;
        Ok("Alerta cancelada con motivo registrado")
    })
}

fn cmd_cobro_alerta_contacto(params: &Value) -> String {
    use crate::cobranzas::{IntentoContacto, ResultadoContacto, TipoContacto};
    with_state(|state| {
        let id = params
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Falta 'id'")?
            .to_string();
        let tipo = match params
            .get("tipo")
            .and_then(|v| v.as_str())
            .unwrap_or("llamada")
        {
            "email" => TipoContacto::Email,
            "whatsapp" => TipoContacto::WhatsApp,
            "reunion" => TipoContacto::Reunion,
            "fax" => TipoContacto::Fax,
            _ => TipoContacto::Llamada,
        };
        let resultado = match params
            .get("resultado")
            .and_then(|v| v.as_str())
            .unwrap_or("sin_contestacion")
        {
            "exitoso" => ResultadoContacto::Exitoso,
            "voicemail" => ResultadoContacto::Voicemail,
            "ocupado" => ResultadoContacto::Ocupado,
            "invalido" => ResultadoContacto::NumeroInvalido,
            "reagendo" => ResultadoContacto::ClienteReagendo,
            "rechazado" => ResultadoContacto::Rechazado,
            _ => ResultadoContacto::SinContestacion,
        };
        let tel = params
            .get("telefono")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let por = params
            .get("agente")
            .and_then(|v| v.as_str())
            .unwrap_or("sistema")
            .to_string();
        let ahora = chrono::Local::now().naive_local();
        let mut intento = IntentoContacto::nuevo(ahora, tipo, resultado.clone(), tel, por);
        if let Some(v) = params.get("notas").and_then(|v| v.as_str()) {
            intento.notas = v.to_string();
        }
        if let Some(v) = params.get("duracion_min").and_then(|v| v.as_u64()) {
            intento.duracion_min = v as u32;
        }
        if let Some(v) = params.get("acuerdo_descripcion").and_then(|v| v.as_str()) {
            intento.acuerdo_descripcion = v.to_string();
        }
        // Si el cliente reagendó, capturar la nueva fecha
        if let Some(f) = params.get("proximo_acordado").and_then(|v| v.as_str()) {
            intento.proximo_intento_acordado =
                chrono::NaiveDateTime::parse_from_str(&format!("{} 09:00", f), "%Y-%m-%d %H:%M")
                    .ok();
        }
        let iid = intento.id.clone();
        let proximo = intento.proximo_intento_acordado;
        let a = state
            .cobranzas
            .alerta_mut(&id)
            .ok_or("Alerta no encontrada")?;
        // Si no contestó, generar advertencia
        let sin_exito_nuevo = a.intentos_sin_exito()
            + if matches!(resultado, ResultadoContacto::Exitoso) {
                0
            } else {
                1
            };
        a.intentos.push(intento);
        state.guardar()?;
        let mut resp = serde_json::json!({ "id": iid, "intentos_sin_exito": sin_exito_nuevo });
        if let Some(dt) = proximo {
            resp["proximo_acordado"] = serde_json::json!(dt.to_string());
            resp["aviso"] =
                serde_json::json!("Reagendar en calendario para no perder esta llamada acordada");
        }
        if sin_exito_nuevo >= 3 {
            resp["alerta"] = serde_json::json!(format!(
                "⚠ {} intentos fallidos — considera escalar o cambiar canal de contacto",
                sin_exito_nuevo
            ));
        }
        Ok(resp)
    })
}

fn cmd_cobro_llamadas_hoy() -> String {
    with_state(|state| {
        let hoy = chrono::Local::now().date_naive();
        let lista: Vec<Value> = state
            .cobranzas
            .llamadas_hoy(hoy)
            .iter()
            .map(|a| {
                serde_json::json!({
                    "id": a.id, "cliente": a.cliente, "titulo": a.titulo,
                    "telefono": a.telefono_contacto, "tipo": a.tipo.nombre(),
                    "monto": a.monto_relacionado, "intentos_previos": a.intentos.len(),
                    "reagendado_para": a.reagendado_para.map(|dt| dt.to_string()),
                })
            })
            .collect();
        Ok(serde_json::json!({ "llamadas": lista, "total": lista.len() }))
    })
}

fn cmd_cobro_generar_alertas_auto() -> String {
    with_state(|state| {
        let hoy = chrono::Local::now().date_naive();
        let ahora = chrono::Local::now().naive_local();
        let antes = state.cobranzas.alertas.len();
        state.cobranzas.generar_alertas_automaticas(hoy, ahora);
        let nuevas = state.cobranzas.alertas.len() - antes;
        state.guardar()?;
        Ok(serde_json::json!({ "alertas_nuevas_generadas": nuevas }))
    })
}

fn cmd_cobro_exportar_csv() -> String {
    with_state(|state| {
        let hoy = chrono::Local::now().date_naive();
        let csv = state.cobranzas.exportar_csv_facturacion(hoy);
        // Guardar el CSV en el directorio de datos
        #[cfg(not(target_arch = "wasm32"))]
        {
            let ruta = crate::storage::AppState::ruta_datos()
                .parent()
                .map(|p| p.join("facturacion_export.csv"))
                .unwrap_or_else(|| std::path::PathBuf::from("facturacion_export.csv"));
            let _ = std::fs::write(&ruta, &csv);
            Ok(serde_json::json!({
                "archivo": ruta.to_string_lossy(),
                "lineas": csv.lines().count() - 1,
                "nota": "Abre el archivo en Excel para ver la facturación completa",
            }))
        }
        #[cfg(target_arch = "wasm32")]
        Ok(serde_json::json!({ "csv": csv }))
    })
}

// ══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_command_invalid_json() {
        let result = process_command("no es json");
        assert!(result.contains("\"ok\":false"));
    }

    #[test]
    fn test_process_command_unknown_action() {
        let result = process_command(r#"{"action":"nope"}"#);
        assert!(result.contains("desconocida"));
    }

    #[test]
    fn test_contras_generar() {
        let result = process_command(r#"{"action":"contras_generar","params":{"longitud":16}}"#);
        assert!(result.contains("\"ok\":true"));
        assert!(result.contains("passwords"));
    }

    #[test]
    fn test_contras_verificar() {
        let result = process_command(
            r#"{"action":"contras_verificar","params":{"original":"hola","input":"hola"}}"#,
        );
        assert!(result.contains("\"coincide\":true"));
    }
}
