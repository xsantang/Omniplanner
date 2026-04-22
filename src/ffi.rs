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

/// Permite a storage.rs consultar el data_dir configurado por Android
pub fn data_dir() -> Result<std::sync::MutexGuard<'static, Option<PathBuf>>, String> {
    DATA_DIR.lock().map_err(|e| format!("Lock error: {}", e))
}

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

fn cmd_evento_actualizar(params: &Value) -> String {
    use chrono::{NaiveDate, NaiveTime};

    with_state(|state| {
        let id = params.get("id").and_then(|v| v.as_str()).ok_or("Falta 'id'")?;
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

        let pagado = params.get("pagado").and_then(|v| v.as_bool()).unwrap_or(false);
        let fecha_limite = params.get("fecha_limite").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let notas = params.get("notas").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let saldo_total_deuda = params.get("saldo_total_deuda").and_then(|v| v.as_f64());

        mes.lineas.push(LineaPresupuesto {
            nombre,
            categoria,
            monto,
            pagado,
            fecha_limite,
            notas,
            saldo_total_deuda,
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
        let mes_str = params.get("mes").and_then(|v| v.as_str()).ok_or("Falta 'mes'")?;
        let indice = params.get("indice").and_then(|v| v.as_u64()).ok_or("Falta 'indice'")? as usize;

        let mes = state.presupuesto.meses.iter_mut().find(|m| m.mes == mes_str)
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
        let nombre = params.get("nombre").and_then(|v| v.as_str()).ok_or("Falta 'nombre'")?;
        let tasa_anual = params.get("tasa_anual").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let pago_minimo = params.get("pago_minimo").and_then(|v| v.as_f64()).ok_or("Falta 'pago_minimo'")?;
        let obligatoria = params.get("obligatoria").and_then(|v| v.as_bool()).unwrap_or(false);
        let saldo_inicial = params.get("saldo_inicial").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let enganche = params.get("enganche").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let escrow_mensual = params.get("escrow_mensual").and_then(|v| v.as_f64()).unwrap_or(0.0);
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
        let indice = params.get("indice").and_then(|v| v.as_u64()).ok_or("Falta 'indice'")? as usize;
        let deudas = &mut state.asesor.rastreador.deudas;
        if indice >= deudas.len() {
            return Err("Índice fuera de rango".to_string());
        }
        let d = &mut deudas[indice];
        if let Some(n) = params.get("nombre").and_then(|v| v.as_str()) { d.nombre = n.to_string(); }
        if let Some(t) = params.get("tasa_anual").and_then(|v| v.as_f64()) { d.tasa_anual = t; }
        if let Some(p) = params.get("pago_minimo").and_then(|v| v.as_f64()) { d.pago_minimo = p; }
        if let Some(p) = params.get("pago_pi_mensual").and_then(|v| v.as_f64()) { d.principal_interes_mensual = p; }
        if let Some(e) = params.get("escrow_mensual").and_then(|v| v.as_f64()) { d.escrow_mensual = e.max(0.0); }
        if let Some(o) = params.get("obligatoria").and_then(|v| v.as_bool()) { d.obligatoria = o; }
        if let Some(a) = params.get("activa").and_then(|v| v.as_bool()) { d.activa = a; }
        state.guardar()?;
        Ok("Deuda actualizada")
    })
}

fn cmd_deuda_eliminar(params: &Value) -> String {
    with_state(|state| {
        let indice = params.get("indice").and_then(|v| v.as_u64()).ok_or("Falta 'indice'")? as usize;
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
        let indice = params.get("indice").and_then(|v| v.as_u64()).ok_or("Falta 'indice'")? as usize;
        let pago = params.get("pago").and_then(|v| v.as_f64()).ok_or("Falta 'pago'")?;
        let pago_escrow = params
            .get("pago_escrow")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let nuevos_cargos = params.get("nuevos_cargos").and_then(|v| v.as_f64()).unwrap_or(0.0);

        if indice >= state.asesor.rastreador.deudas.len() {
            return Err("Índice fuera de rango".to_string());
        }

        let saldo = state.asesor.rastreador.deudas[indice].saldo_actual();
        let mes = chrono::Local::now().format("%Y-%m").to_string();
        state.asesor.rastreador.deudas[indice]
            .registrar_mes_con_escrow(&mes, saldo, pago, pago_escrow, nuevos_cargos);
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
        let concepto = params.get("concepto").and_then(|v| v.as_str()).ok_or("Falta 'concepto'")?.to_string();
        let monto = params.get("monto").and_then(|v| v.as_f64()).ok_or("Falta 'monto'")?;
        let freq_str = params.get("frecuencia").and_then(|v| v.as_str()).unwrap_or("mensual");
        let confirmado = params.get("confirmado").and_then(|v| v.as_bool()).unwrap_or(true);
        let impuesto_federal = params
            .get("impuesto_federal")
            .and_then(|v| v.as_bool())
            .unwrap_or_else(|| params.get("taxeable").and_then(|v| v.as_bool()).unwrap_or(false));
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
        });
        state.guardar()?;
        Ok("Ingreso agregado")
    })
}

fn cmd_ingreso_eliminar(params: &Value) -> String {
    with_state(|state| {
        let indice = params.get("indice").and_then(|v| v.as_u64()).ok_or("Falta 'indice'")? as usize;
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
        let id = params.get("id").and_then(|v| v.as_str()).ok_or("Falta 'id'")?;
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
        let id = params.get("id").and_then(|v| v.as_str()).ok_or("Falta 'id'")?;
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
