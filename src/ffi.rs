// ══════════════════════════════════════════════════════════════
//  FFI — Puente C/JNI para Android
//  Toda la comunicación es JSON string in → JSON string out
// ══════════════════════════════════════════════════════════════

use std::ffi::{CStr, CString};
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

// ── Helpers FFI ──────────────────────────────────────────────

fn c_str_to_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .unwrap_or("")
        .to_string()
}

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
#[no_mangle]
pub unsafe extern "C" fn omni_command(json_request: *const c_char) -> *mut c_char {
    let input = c_str_to_string(json_request);
    let result = process_command(&input);
    string_to_c(result)
}

/// # Safety
/// Libera la memoria de un string devuelto por omni_command.
#[no_mangle]
pub unsafe extern "C" fn omni_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}

fn process_command(input: &str) -> String {
    let req: Request = match serde_json::from_str(input) {
        Ok(r) => r,
        Err(e) => return err_json(&format!("JSON inválido: {}", e)),
    };

    match req.action.as_str() {
        // ── Sistema ──
        "init" => cmd_init(&req.params),
        "guardar" => cmd_guardar(),
        "dashboard" => cmd_dashboard(),

        // ── Tareas ──
        "tareas_listar" => cmd_tareas_listar(),
        "tarea_crear" => cmd_tarea_crear(&req.params),
        "tarea_actualizar" => cmd_tarea_actualizar(&req.params),
        "tarea_eliminar" => cmd_tarea_eliminar(&req.params),

        // ── Agenda ──
        "agenda_hoy" => cmd_agenda_hoy(),
        "agenda_mes" => cmd_agenda_mes(&req.params),
        "evento_crear" => cmd_evento_crear(&req.params),
        "evento_eliminar" => cmd_evento_eliminar(&req.params),

        // ── Presupuesto ──
        "presupuesto_resumen" => cmd_presupuesto_resumen(),
        "presupuesto_agregar" => cmd_presupuesto_agregar(&req.params),

        // ── Contraseñas ──
        "contras_listar" => cmd_contras_listar(),
        "contras_guardar" => cmd_contras_guardar(&req.params),
        "contras_generar" => cmd_contras_generar(&req.params),
        "contras_verificar" => cmd_contras_verificar(&req.params),
        "contras_eliminar" => cmd_contras_eliminar(&req.params),

        // ── Memoria ──
        "memoria_listar" => cmd_memoria_listar(),
        "memoria_agregar" => cmd_memoria_agregar(&req.params),

        // ── Sync ──
        "sync_push" => cmd_sync_push(),
        "sync_pull" => cmd_sync_pull(),
        "sync_config" => cmd_sync_config(&req.params),

        _ => err_json(&format!("Acción desconocida: {}", req.action)),
    }
}

// ══════════════════════════════════════════════════════════════
//  Implementación de comandos
// ══════════════════════════════════════════════════════════════

fn cmd_init(params: &Value) -> String {
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
        };

        state.agenda.eventos.push(evento.clone());
        state.guardar()?;
        Ok(serde_json::to_value(&evento).unwrap())
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

        mes.lineas.push(LineaPresupuesto {
            nombre,
            categoria,
            monto,
            pagado: true,
            fecha_limite: String::new(),
            notas: String::new(),
            saldo_total_deuda: None,
        });

        state.guardar()?;
        Ok("Línea agregada al presupuesto")
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

// ── Sync ─────────────────────────────────────────────────────

fn cmd_sync_push() -> String {
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

fn cmd_sync_pull() -> String {
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
//  Tests
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
