//! # io_modulos — Export/Import por módulo (Fase 5.2)
//!
//! Conecta los módulos `tareas`, `agenda`, `memoria` y `rastreador` (pagos)
//! con la capa común [`omniplanner::io`]. Cada módulo expone:
//!
//! - **Exportar**: CSV, Markdown, JSON, Excel (`.xlsx`) y SQL.
//! - **Importar**: CSV o JSON con detección automática de cabeceras
//!   bilingües (ES/EN).

#![allow(clippy::too_many_lines)]

use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use omniplanner::agenda::{Evento, Frecuencia, TipoEvento};
use omniplanner::io;
use omniplanner::memoria::Recuerdo;
use omniplanner::ml::advisor::MesPago;
use omniplanner::storage::AppState;
use omniplanner::tasks::{Prioridad, Task, TaskStatus};

use crate::{confirmar, menu, pausa, pedir_texto, separador};

// ════════════════════════════════════════════════════════════════════════
//  Helpers comunes
// ════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy)]
enum Formato {
    Csv,
    Markdown,
    Json,
    Xlsx,
    Sql,
    Todos,
}

fn pedir_formato_export() -> Option<Formato> {
    let opciones = [
        "📄  CSV",
        "📝  Markdown",
        "📦  JSON",
        "📊  Excel (.xlsx)",
        "🗄️   SQL",
        "🌐  Todos los formatos",
        "🔙  Cancelar",
    ];
    match menu("¿En qué formato exportar?", &opciones) {
        Some(0) => Some(Formato::Csv),
        Some(1) => Some(Formato::Markdown),
        Some(2) => Some(Formato::Json),
        Some(3) => Some(Formato::Xlsx),
        Some(4) => Some(Formato::Sql),
        Some(5) => Some(Formato::Todos),
        _ => None,
    }
}

/// Lista archivos `.csv` y `.json` en `<exports>/<modulo>/` y deja al usuario
/// escoger uno o escribir una ruta absoluta.
fn pedir_archivo_para_importar(modulo: &str) -> Option<PathBuf> {
    let dir = io::dir_modulo(modulo);
    let mut archivos: Vec<PathBuf> = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .and_then(|s| s.to_str())
                .map(|s| matches!(s.to_lowercase().as_str(), "csv" | "json" | "xlsx"))
                .unwrap_or(false)
        })
        .collect();
    archivos.sort();

    let mut etiquetas: Vec<String> = archivos
        .iter()
        .map(|p| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    etiquetas.push("📂  Escribir ruta manual…".to_string());
    etiquetas.push("🔙  Cancelar".to_string());
    let refs: Vec<&str> = etiquetas.iter().map(|s| s.as_str()).collect();
    let i = menu("¿Qué archivo importar?", &refs)?;
    if i < archivos.len() {
        Some(archivos[i].clone())
    } else if i == archivos.len() {
        let ruta = pedir_texto("Ruta absoluta del archivo")?;
        Some(PathBuf::from(ruta.trim()))
    } else {
        None
    }
}

fn escribir_resultado(rutas: &[PathBuf], n: usize) {
    println!();
    if rutas.is_empty() {
        println!("  {} No se generó ningún archivo.", "ℹ️".cyan());
    } else {
        println!(
            "  {} {} registro(s) exportado(s):",
            "✅".green(),
            n.to_string().bold()
        );
        for r in rutas {
            println!("    · {}", r.display().to_string().cyan());
        }
    }
    pausa();
}

#[allow(clippy::too_many_arguments)]
fn exportar_segun_formato(
    modulo: &str,
    prefijo: &str,
    tabla_sql: &str,
    titulo_md: &str,
    cabeceras: &[String],
    filas: &[Vec<String>],
    json_serializado: &str,
    formato: Formato,
) -> Vec<PathBuf> {
    let dir = io::dir_modulo(modulo);
    let mut salidas = Vec::new();

    let escribir = |fmt: Formato| -> Option<PathBuf> {
        match fmt {
            Formato::Csv => {
                let ruta = dir.join(io::nombre_archivo(prefijo, "csv"));
                io::escribir_csv(&ruta, cabeceras, filas).ok().map(|_| ruta)
            }
            Formato::Markdown => {
                let ruta = dir.join(io::nombre_archivo(prefijo, "md"));
                io::escribir_markdown_tabla(&ruta, titulo_md, None, cabeceras, filas)
                    .ok()
                    .map(|_| ruta)
            }
            Formato::Json => {
                let ruta = dir.join(io::nombre_archivo(prefijo, "json"));
                std::fs::write(&ruta, json_serializado).ok().map(|_| ruta)
            }
            #[cfg(feature = "desktop")]
            Formato::Xlsx => {
                let ruta = dir.join(io::nombre_archivo(prefijo, "xlsx"));
                io::escribir_xlsx(&ruta, titulo_md, cabeceras, filas)
                    .ok()
                    .map(|_| ruta)
            }
            #[cfg(not(feature = "desktop"))]
            Formato::Xlsx => None,
            Formato::Sql => {
                let ruta = dir.join(io::nombre_archivo(prefijo, "sql"));
                io::escribir_sql(&ruta, tabla_sql, cabeceras, filas)
                    .ok()
                    .map(|_| ruta)
            }
            Formato::Todos => None,
        }
    };

    if matches!(formato, Formato::Todos) {
        for f in [
            Formato::Csv,
            Formato::Markdown,
            Formato::Json,
            Formato::Xlsx,
            Formato::Sql,
        ] {
            if let Some(r) = escribir(f) {
                salidas.push(r);
            }
        }
    } else if let Some(r) = escribir(formato) {
        salidas.push(r);
    }
    salidas
}

fn indice_por(cabeceras: &[String], claves: &[&str]) -> Option<usize> {
    let normalizadas: Vec<String> = cabeceras
        .iter()
        .map(|h| io::normalizar_cabecera(h))
        .collect();
    for clave in claves {
        if let Some(p) = normalizadas.iter().position(|h| h == *clave) {
            return Some(p);
        }
    }
    None
}

fn campo(fila: &[String], idx: Option<usize>) -> &str {
    idx.and_then(|i| fila.get(i))
        .map(|s| s.as_str())
        .unwrap_or("")
}

// ════════════════════════════════════════════════════════════════════════
//  TAREAS — export / import
// ════════════════════════════════════════════════════════════════════════

fn cabeceras_tareas() -> Vec<String> {
    vec![
        "id".to_string(),
        io::bil("titulo", "title"),
        io::bil("descripcion", "description"),
        io::bil("fecha", "date"),
        io::bil("hora", "time"),
        io::bil("estado", "status"),
        io::bil("prioridad", "priority"),
        io::bil("etiquetas", "tags"),
        io::bil("follow_up", "follow_up"),
        io::bil("creado", "created"),
        io::bil("actualizado", "updated"),
    ]
}

fn fila_tarea(t: &Task) -> Vec<String> {
    vec![
        t.id.clone(),
        t.titulo.clone(),
        t.descripcion.clone(),
        t.fecha.format("%Y-%m-%d").to_string(),
        t.hora.format("%H:%M:%S").to_string(),
        t.estado.to_string(),
        t.prioridad.to_string(),
        t.etiquetas.join(";"),
        t.follow_up
            .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default(),
        t.creado.format("%Y-%m-%d %H:%M:%S").to_string(),
        t.actualizado.format("%Y-%m-%d %H:%M:%S").to_string(),
    ]
}

fn parse_prioridad(s: &str) -> Prioridad {
    match s.trim().to_lowercase().as_str() {
        "baja" | "low" => Prioridad::Baja,
        "alta" | "high" => Prioridad::Alta,
        "urgente" | "urgent" | "⚠ urgente" => Prioridad::Urgente,
        _ => Prioridad::Media,
    }
}

fn parse_estado_tarea(s: &str) -> TaskStatus {
    match s.trim().to_lowercase().as_str() {
        "completada" | "completed" | "done" => TaskStatus::Completada,
        "en progreso" | "enprogreso" | "in progress" | "inprogress" => TaskStatus::EnProgreso,
        "cancelada" | "canceled" | "cancelled" => TaskStatus::Cancelada,
        _ => TaskStatus::Pendiente,
    }
}

fn parse_fecha(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .or_else(|_| NaiveDate::parse_from_str(s, "%d/%m/%Y"))
        .or_else(|_| NaiveDate::parse_from_str(s, "%m/%d/%Y"))
        .ok()
}

fn parse_hora(s: &str) -> NaiveTime {
    let s = s.trim();
    NaiveTime::parse_from_str(s, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(s, "%H:%M"))
        .unwrap_or_else(|_| NaiveTime::from_hms_opt(9, 0, 0).unwrap())
}

fn parse_dt(s: &str) -> Option<NaiveDateTime> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S"))
        .ok()
}

pub fn tareas_exportar(state: &AppState) {
    let formato = match pedir_formato_export() {
        Some(f) => f,
        None => return,
    };
    let cabs = cabeceras_tareas();
    let filas: Vec<Vec<String>> = state.tasks.tareas.iter().map(fila_tarea).collect();
    let json = serde_json::to_string_pretty(&state.tasks.tareas).unwrap_or_default();
    let salidas = exportar_segun_formato(
        "tareas", "tareas", "tareas", "Tareas", &cabs, &filas, &json, formato,
    );
    escribir_resultado(&salidas, filas.len());
}

pub fn tareas_importar(state: &mut AppState) {
    let ruta = match pedir_archivo_para_importar("tareas") {
        Some(r) => r,
        None => return,
    };
    let ext = ruta
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let nuevas: Vec<Task> = match ext.as_str() {
        "json" => match io::leer_json::<Vec<Task>>(&ruta) {
            Ok(v) => v,
            Err(e) => {
                println!("  {} JSON inválido: {}", "✗".red(), e);
                pausa();
                return;
            }
        },
        "csv" => match io::leer_csv(&ruta) {
            Ok((cabs, filas)) => filas_a_tareas(&cabs, &filas),
            Err(e) => {
                println!("  {} CSV inválido: {}", "✗".red(), e);
                pausa();
                return;
            }
        },
        #[cfg(feature = "desktop")]
        "xlsx" => match io::leer_xlsx(&ruta) {
            Ok((cabs, filas)) => filas_a_tareas(&cabs, &filas),
            Err(e) => {
                println!("  {} XLSX inválido: {}", "✗".red(), e);
                pausa();
                return;
            }
        },
        _ => {
            println!("  {} Formato no soportado: {}", "✗".red(), ext);
            pausa();
            return;
        }
    };

    if nuevas.is_empty() {
        println!("  {} El archivo no contenía tareas válidas.", "ℹ️".cyan());
        pausa();
        return;
    }
    let n = nuevas.len();
    if !confirmar(
        &format!("¿Importar {} tarea(s) (se anexan a las existentes)?", n),
        true,
    ) {
        return;
    }
    state.tasks.tareas.extend(nuevas);
    println!("  {} {} tarea(s) importada(s).", "✅".green(), n);
    pausa();
}

fn filas_a_tareas(cabs: &[String], filas: &[Vec<String>]) -> Vec<Task> {
    let i_id = indice_por(cabs, &["id"]);
    let i_tit = indice_por(cabs, &["titulo"]);
    let i_des = indice_por(cabs, &["descripcion"]);
    let i_fec = indice_por(cabs, &["fecha"]);
    let i_hor = indice_por(cabs, &["hora"]);
    let i_est = indice_por(cabs, &["estado"]);
    let i_pri = indice_por(cabs, &["prioridad"]);
    let i_etq = indice_por(cabs, &["etiquetas"]);
    let i_fup = indice_por(cabs, &["follow_up"]);
    let i_cre = indice_por(cabs, &["creado"]);
    let i_act = indice_por(cabs, &["actualizado"]);
    let ahora = Local::now().naive_local();
    let mut out = Vec::new();
    for fila in filas {
        let titulo = campo(fila, i_tit).trim().to_string();
        if titulo.is_empty() {
            continue;
        }
        let fecha = parse_fecha(campo(fila, i_fec)).unwrap_or_else(|| ahora.date());
        let hora = parse_hora(campo(fila, i_hor));
        let id_csv = campo(fila, i_id).trim().to_string();
        let id = if id_csv.is_empty() {
            Uuid::new_v4().to_string()[..8].to_string()
        } else {
            id_csv
        };
        let etiquetas: Vec<String> = campo(fila, i_etq)
            .split([';', ',', '|'])
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        out.push(Task {
            id,
            titulo,
            descripcion: campo(fila, i_des).to_string(),
            fecha,
            hora,
            estado: parse_estado_tarea(campo(fila, i_est)),
            prioridad: parse_prioridad(campo(fila, i_pri)),
            etiquetas,
            follow_up: parse_dt(campo(fila, i_fup)),
            creado: parse_dt(campo(fila, i_cre)).unwrap_or(ahora),
            actualizado: parse_dt(campo(fila, i_act)).unwrap_or(ahora),
        });
    }
    out
}

// ════════════════════════════════════════════════════════════════════════
//  AGENDA — export / import
// ════════════════════════════════════════════════════════════════════════

fn cabeceras_eventos() -> Vec<String> {
    vec![
        "id".to_string(),
        io::bil("titulo", "title"),
        io::bil("descripcion", "description"),
        io::bil("tipo", "type"),
        io::bil("fecha", "date"),
        io::bil("hora_inicio", "start_time"),
        io::bil("hora_fin", "end_time"),
        io::bil("recurrente", "recurring"),
        io::bil("frecuencia", "frequency"),
        io::bil("concepto", "concept"),
        io::bil("notas", "notes"),
        io::bil("creado", "created"),
    ]
}

fn fila_evento(e: &Evento) -> Vec<String> {
    vec![
        e.id.clone(),
        e.titulo.clone(),
        e.descripcion.clone(),
        e.tipo.to_string(),
        e.fecha.format("%Y-%m-%d").to_string(),
        e.hora_inicio.format("%H:%M:%S").to_string(),
        e.hora_fin
            .map(|h| h.format("%H:%M:%S").to_string())
            .unwrap_or_default(),
        e.recurrente.to_string(),
        e.frecuencia.to_string(),
        e.concepto.clone(),
        e.notas.join(" | "),
        e.creado.format("%Y-%m-%d %H:%M:%S").to_string(),
    ]
}

fn parse_tipo_evento(s: &str) -> TipoEvento {
    let t = s.trim().to_lowercase();
    match t.as_str() {
        "reunion" | "reunión" | "meeting" => TipoEvento::Reunion,
        "recordatorio" | "reminder" => TipoEvento::Recordatorio,
        "follow-up" | "followup" | "follow_up" => TipoEvento::FollowUp,
        "cita" | "appointment" => TipoEvento::Cita,
        "cumpleanos" | "cumpleaños" | "birthday" => TipoEvento::Cumpleanos,
        "pago" | "payment" => TipoEvento::Pago,
        "" => TipoEvento::Otro("Otro".to_string()),
        _ => TipoEvento::Otro(s.trim().to_string()),
    }
}

fn parse_frecuencia(s: &str) -> Frecuencia {
    match s.trim().to_lowercase().as_str() {
        "semanal" | "weekly" => Frecuencia::Semanal,
        "mensual" | "monthly" => Frecuencia::Mensual,
        "trimestral" | "quarterly" => Frecuencia::Trimestral,
        "semestral" | "semiannual" => Frecuencia::Semestral,
        "anual" | "yearly" | "annual" => Frecuencia::Anual,
        _ => Frecuencia::UnaVez,
    }
}

pub fn agenda_exportar(state: &AppState) {
    let formato = match pedir_formato_export() {
        Some(f) => f,
        None => return,
    };
    let cabs = cabeceras_eventos();
    let filas: Vec<Vec<String>> = state.agenda.eventos.iter().map(fila_evento).collect();
    let json = serde_json::to_string_pretty(&state.agenda.eventos).unwrap_or_default();
    let salidas = exportar_segun_formato(
        "agenda", "agenda", "eventos", "Agenda", &cabs, &filas, &json, formato,
    );
    escribir_resultado(&salidas, filas.len());
}

pub fn agenda_importar(state: &mut AppState) {
    let ruta = match pedir_archivo_para_importar("agenda") {
        Some(r) => r,
        None => return,
    };
    let ext = ruta
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let nuevos: Vec<Evento> = match ext.as_str() {
        "json" => match io::leer_json::<Vec<Evento>>(&ruta) {
            Ok(v) => v,
            Err(e) => {
                println!("  {} JSON inválido: {}", "✗".red(), e);
                pausa();
                return;
            }
        },
        "csv" => match io::leer_csv(&ruta) {
            Ok((c, f)) => filas_a_eventos(&c, &f),
            Err(e) => {
                println!("  {} CSV inválido: {}", "✗".red(), e);
                pausa();
                return;
            }
        },
        #[cfg(feature = "desktop")]
        "xlsx" => match io::leer_xlsx(&ruta) {
            Ok((c, f)) => filas_a_eventos(&c, &f),
            Err(e) => {
                println!("  {} XLSX inválido: {}", "✗".red(), e);
                pausa();
                return;
            }
        },
        _ => {
            println!("  {} Formato no soportado.", "✗".red());
            pausa();
            return;
        }
    };

    if nuevos.is_empty() {
        println!("  {} El archivo no contenía eventos válidos.", "ℹ️".cyan());
        pausa();
        return;
    }
    let n = nuevos.len();
    if !confirmar(&format!("¿Importar {} evento(s)?", n), true) {
        return;
    }
    state.agenda.eventos.extend(nuevos);
    println!("  {} {} evento(s) importado(s).", "✅".green(), n);
    pausa();
}

fn filas_a_eventos(cabs: &[String], filas: &[Vec<String>]) -> Vec<Evento> {
    let i_id = indice_por(cabs, &["id"]);
    let i_tit = indice_por(cabs, &["titulo"]);
    let i_des = indice_por(cabs, &["descripcion"]);
    let i_tip = indice_por(cabs, &["tipo"]);
    let i_fec = indice_por(cabs, &["fecha"]);
    let i_hi = indice_por(cabs, &["hora_inicio", "hora"]);
    let i_hf = indice_por(cabs, &["hora_fin"]);
    let i_rec = indice_por(cabs, &["recurrente"]);
    let i_fre = indice_por(cabs, &["frecuencia"]);
    let i_con = indice_por(cabs, &["concepto"]);
    let i_not = indice_por(cabs, &["notas"]);
    let i_cre = indice_por(cabs, &["creado"]);
    let ahora = Local::now().naive_local();
    let mut out = Vec::new();
    for fila in filas {
        let titulo = campo(fila, i_tit).trim().to_string();
        if titulo.is_empty() {
            continue;
        }
        let fecha = parse_fecha(campo(fila, i_fec)).unwrap_or_else(|| ahora.date());
        let hora_inicio = parse_hora(campo(fila, i_hi));
        let hora_fin = {
            let s = campo(fila, i_hf);
            if s.trim().is_empty() {
                None
            } else {
                Some(parse_hora(s))
            }
        };
        let id_csv = campo(fila, i_id).trim().to_string();
        let id = if id_csv.is_empty() {
            Uuid::new_v4().to_string()[..8].to_string()
        } else {
            id_csv
        };
        let recurrente = matches!(
            campo(fila, i_rec).trim().to_lowercase().as_str(),
            "true" | "1" | "si" | "sí" | "yes"
        );
        let notas: Vec<String> = campo(fila, i_not)
            .split('|')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        out.push(Evento {
            id,
            titulo,
            descripcion: campo(fila, i_des).to_string(),
            tipo: parse_tipo_evento(campo(fila, i_tip)),
            fecha,
            hora_inicio,
            hora_fin,
            recurrente,
            frecuencia: parse_frecuencia(campo(fila, i_fre)),
            concepto: campo(fila, i_con).to_string(),
            notas,
            creado: parse_dt(campo(fila, i_cre)).unwrap_or(ahora),
        });
    }
    out
}

// ════════════════════════════════════════════════════════════════════════
//  MEMORIA — export / import
// ════════════════════════════════════════════════════════════════════════

fn cabeceras_recuerdos() -> Vec<String> {
    vec![
        "id".to_string(),
        io::bil("contenido", "content"),
        io::bil("palabras_clave", "keywords"),
        io::bil("modulo_origen", "source_module"),
        io::bil("item_id", "item_id"),
        io::bil("creado", "created"),
    ]
}

fn fila_recuerdo(r: &Recuerdo) -> Vec<String> {
    vec![
        r.id.clone(),
        r.contenido.clone(),
        r.palabras_clave.join(";"),
        r.modulo_origen.clone().unwrap_or_default(),
        r.item_id.clone().unwrap_or_default(),
        r.creado.format("%Y-%m-%d %H:%M:%S").to_string(),
    ]
}

pub fn memoria_exportar(state: &AppState) {
    let formato = match pedir_formato_export() {
        Some(f) => f,
        None => return,
    };
    let cabs = cabeceras_recuerdos();
    let filas: Vec<Vec<String>> = state.memoria.recuerdos.iter().map(fila_recuerdo).collect();
    let json = serde_json::to_string_pretty(&state.memoria.recuerdos).unwrap_or_default();
    let salidas = exportar_segun_formato(
        "memoria",
        "memoria",
        "recuerdos",
        "Memoria",
        &cabs,
        &filas,
        &json,
        formato,
    );
    escribir_resultado(&salidas, filas.len());
}

pub fn memoria_importar(state: &mut AppState) {
    let ruta = match pedir_archivo_para_importar("memoria") {
        Some(r) => r,
        None => return,
    };
    let ext = ruta
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let nuevos: Vec<Recuerdo> = match ext.as_str() {
        "json" => match io::leer_json::<Vec<Recuerdo>>(&ruta) {
            Ok(v) => v,
            Err(e) => {
                println!("  {} JSON inválido: {}", "✗".red(), e);
                pausa();
                return;
            }
        },
        "csv" => match io::leer_csv(&ruta) {
            Ok((c, f)) => filas_a_recuerdos(&c, &f),
            Err(e) => {
                println!("  {} CSV inválido: {}", "✗".red(), e);
                pausa();
                return;
            }
        },
        #[cfg(feature = "desktop")]
        "xlsx" => match io::leer_xlsx(&ruta) {
            Ok((c, f)) => filas_a_recuerdos(&c, &f),
            Err(e) => {
                println!("  {} XLSX inválido: {}", "✗".red(), e);
                pausa();
                return;
            }
        },
        _ => {
            println!("  {} Formato no soportado.", "✗".red());
            pausa();
            return;
        }
    };

    if nuevos.is_empty() {
        println!(
            "  {} El archivo no contenía recuerdos válidos.",
            "ℹ️".cyan()
        );
        pausa();
        return;
    }
    let n = nuevos.len();
    if !confirmar(&format!("¿Importar {} recuerdo(s)?", n), true) {
        return;
    }
    state.memoria.recuerdos.extend(nuevos);
    println!("  {} {} recuerdo(s) importado(s).", "✅".green(), n);
    pausa();
}

fn filas_a_recuerdos(cabs: &[String], filas: &[Vec<String>]) -> Vec<Recuerdo> {
    let i_id = indice_por(cabs, &["id"]);
    let i_con = indice_por(cabs, &["contenido"]);
    let i_pal = indice_por(cabs, &["palabras_clave", "etiquetas"]);
    let i_mod = indice_por(cabs, &["modulo_origen", "modulo"]);
    let i_iid = indice_por(cabs, &["item_id"]);
    let i_cre = indice_por(cabs, &["creado"]);
    let ahora = Local::now().naive_local();
    let mut out = Vec::new();
    for fila in filas {
        let contenido = campo(fila, i_con).trim().to_string();
        if contenido.is_empty() {
            continue;
        }
        let id_csv = campo(fila, i_id).trim().to_string();
        let id = if id_csv.is_empty() {
            Uuid::new_v4().to_string()[..8].to_string()
        } else {
            id_csv
        };
        let palabras: Vec<String> = campo(fila, i_pal)
            .split([';', ',', '|'])
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let modulo = campo(fila, i_mod).trim().to_string();
        let item = campo(fila, i_iid).trim().to_string();
        out.push(Recuerdo {
            id,
            contenido,
            palabras_clave: palabras,
            modulo_origen: if modulo.is_empty() {
                None
            } else {
                Some(modulo)
            },
            item_id: if item.is_empty() { None } else { Some(item) },
            creado: parse_dt(campo(fila, i_cre)).unwrap_or(ahora),
        });
    }
    out
}

// ════════════════════════════════════════════════════════════════════════
//  RASTREADOR — pagos (MesPago) export / import
// ════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PagoExport {
    deuda: String,
    mes: String,
    saldo_inicio: f64,
    pago: f64,
    pago_escrow: f64,
    nuevos_cargos: f64,
    intereses: f64,
    saldo_final: f64,
    meses_cubiertos: Vec<String>,
    nota: String,
}

fn cabeceras_pagos() -> Vec<String> {
    vec![
        io::bil("deuda", "debt"),
        io::bil("mes", "month"),
        io::bil("saldo_inicio", "start_balance"),
        io::bil("pago", "payment"),
        io::bil("pago_escrow", "escrow_payment"),
        io::bil("nuevos_cargos", "new_charges"),
        io::bil("intereses", "interest"),
        io::bil("saldo_final", "end_balance"),
        io::bil("meses_cubiertos", "covered_months"),
        io::bil("nota", "note"),
    ]
}

fn pagos_a_filas(state: &AppState) -> (Vec<Vec<String>>, Vec<PagoExport>) {
    let mut filas = Vec::new();
    let mut json = Vec::new();
    for d in &state.asesor.rastreador.deudas {
        for m in &d.historial {
            json.push(PagoExport {
                deuda: d.nombre.clone(),
                mes: m.mes.clone(),
                saldo_inicio: m.saldo_inicio,
                pago: m.pago,
                pago_escrow: m.pago_escrow,
                nuevos_cargos: m.nuevos_cargos,
                intereses: m.intereses,
                saldo_final: m.saldo_final,
                meses_cubiertos: m.meses_cubiertos.clone(),
                nota: m.nota.clone(),
            });
            filas.push(vec![
                d.nombre.clone(),
                m.mes.clone(),
                format!("{:.2}", m.saldo_inicio),
                format!("{:.2}", m.pago),
                format!("{:.2}", m.pago_escrow),
                format!("{:.2}", m.nuevos_cargos),
                format!("{:.2}", m.intereses),
                format!("{:.2}", m.saldo_final),
                m.meses_cubiertos.join(";"),
                m.nota.clone(),
            ]);
        }
    }
    (filas, json)
}

pub fn pagos_exportar(state: &AppState) {
    let formato = match pedir_formato_export() {
        Some(f) => f,
        None => return,
    };
    let cabs = cabeceras_pagos();
    let (filas, json_data) = pagos_a_filas(state);
    let json = serde_json::to_string_pretty(&json_data).unwrap_or_default();
    let salidas = exportar_segun_formato(
        "pagos",
        "pagos",
        "pagos",
        "Pagos del rastreador",
        &cabs,
        &filas,
        &json,
        formato,
    );
    escribir_resultado(&salidas, filas.len());
}

pub fn pagos_importar(state: &mut AppState) {
    let ruta = match pedir_archivo_para_importar("pagos") {
        Some(r) => r,
        None => return,
    };
    let ext = ruta
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let registros: Vec<PagoExport> = match ext.as_str() {
        "json" => match io::leer_json::<Vec<PagoExport>>(&ruta) {
            Ok(v) => v,
            Err(e) => {
                println!("  {} JSON inválido: {}", "✗".red(), e);
                pausa();
                return;
            }
        },
        "csv" => match io::leer_csv(&ruta) {
            Ok((c, f)) => filas_a_pagos(&c, &f),
            Err(e) => {
                println!("  {} CSV inválido: {}", "✗".red(), e);
                pausa();
                return;
            }
        },
        #[cfg(feature = "desktop")]
        "xlsx" => match io::leer_xlsx(&ruta) {
            Ok((c, f)) => filas_a_pagos(&c, &f),
            Err(e) => {
                println!("  {} XLSX inválido: {}", "✗".red(), e);
                pausa();
                return;
            }
        },
        _ => {
            println!("  {} Formato no soportado.", "✗".red());
            pausa();
            return;
        }
    };

    if registros.is_empty() {
        println!("  {} Sin registros válidos.", "ℹ️".cyan());
        pausa();
        return;
    }
    let n = registros.len();
    if !confirmar(
        &format!(
            "¿Importar {} pago(s) (se anexan al historial existente)?",
            n
        ),
        true,
    ) {
        return;
    }

    let mut anexados = 0usize;
    let mut deudas_no_encontradas: Vec<String> = Vec::new();
    for p in registros {
        if let Some(d) = state
            .asesor
            .rastreador
            .deudas
            .iter_mut()
            .find(|d| d.nombre.eq_ignore_ascii_case(&p.deuda))
        {
            d.historial.push(MesPago {
                mes: p.mes,
                saldo_inicio: p.saldo_inicio,
                pago: p.pago,
                pago_escrow: p.pago_escrow,
                nuevos_cargos: p.nuevos_cargos,
                intereses: p.intereses,
                saldo_final: p.saldo_final,
                meses_cubiertos: p.meses_cubiertos,
                nota: p.nota,
            });
            anexados += 1;
        } else if !deudas_no_encontradas.contains(&p.deuda) {
            deudas_no_encontradas.push(p.deuda);
        }
    }
    println!(
        "  {} {} pago(s) anexado(s).",
        "✅".green(),
        anexados.to_string().bold()
    );
    if !deudas_no_encontradas.is_empty() {
        println!(
            "  {} Deudas no encontradas (omitidas): {}",
            "⚠️".yellow(),
            deudas_no_encontradas.join(", ")
        );
    }
    pausa();
}

fn filas_a_pagos(cabs: &[String], filas: &[Vec<String>]) -> Vec<PagoExport> {
    let i_d = indice_por(cabs, &["deuda"]);
    let i_m = indice_por(cabs, &["mes"]);
    let i_si = indice_por(cabs, &["saldo_inicio"]);
    let i_p = indice_por(cabs, &["pago"]);
    let i_pe = indice_por(cabs, &["pago_escrow"]);
    let i_nc = indice_por(cabs, &["nuevos_cargos"]);
    let i_i = indice_por(cabs, &["intereses"]);
    let i_sf = indice_por(cabs, &["saldo_final"]);
    let i_mc = indice_por(cabs, &["meses_cubiertos"]);
    let i_nt = indice_por(cabs, &["nota"]);
    let parse_f = |s: &str| -> f64 { s.trim().replace(',', "").parse().unwrap_or(0.0) };
    let mut out = Vec::new();
    for fila in filas {
        let deuda = campo(fila, i_d).trim().to_string();
        let mes = campo(fila, i_m).trim().to_string();
        if deuda.is_empty() || mes.is_empty() {
            continue;
        }
        let cubiertos: Vec<String> = campo(fila, i_mc)
            .split([';', '|'])
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        out.push(PagoExport {
            deuda,
            mes,
            saldo_inicio: parse_f(campo(fila, i_si)),
            pago: parse_f(campo(fila, i_p)),
            pago_escrow: parse_f(campo(fila, i_pe)),
            nuevos_cargos: parse_f(campo(fila, i_nc)),
            intereses: parse_f(campo(fila, i_i)),
            saldo_final: parse_f(campo(fila, i_sf)),
            meses_cubiertos: cubiertos,
            nota: campo(fila, i_nt).to_string(),
        });
    }
    out
}

// ════════════════════════════════════════════════════════════════════════
//  Submenús "Importar / Exportar" reutilizables por cada módulo
// ════════════════════════════════════════════════════════════════════════

fn submenu_io(
    titulo: &str,
    state: &mut AppState,
    exportar: fn(&AppState),
    importar: fn(&mut AppState),
) {
    loop {
        crate::limpiar();
        println!("{}", titulo.bold().cyan());
        separador("Datos");
        let opciones = [
            "📤  Exportar (CSV / Markdown / JSON / Excel / SQL)",
            "📥  Importar desde CSV, JSON o Excel",
            "🔙  Volver",
        ];
        match menu("¿Qué deseas hacer?", &opciones) {
            Some(0) => exportar(state),
            Some(1) => importar(state),
            _ => return,
        }
    }
}

pub fn menu_io_tareas(state: &mut AppState) {
    submenu_io(
        "📤 Importar / Exportar Tareas",
        state,
        tareas_exportar,
        tareas_importar,
    );
}

pub fn menu_io_agenda(state: &mut AppState) {
    submenu_io(
        "📤 Importar / Exportar Agenda",
        state,
        agenda_exportar,
        agenda_importar,
    );
}

pub fn menu_io_memoria(state: &mut AppState) {
    submenu_io(
        "📤 Importar / Exportar Memoria",
        state,
        memoria_exportar,
        memoria_importar,
    );
}

pub fn menu_io_pagos(state: &mut AppState) {
    submenu_io(
        "📤 Importar / Exportar Pagos del Rastreador",
        state,
        pagos_exportar,
        pagos_importar,
    );
}

// ════════════════════════════════════════════════════════════════════════
//  BITÁCORA — export / import unificado (Fase 5.3)
// ════════════════════════════════════════════════════════════════════════

use omniplanner::eventos::EventoSistema;

fn cabeceras_bitacora() -> Vec<String> {
    vec![
        "id".to_string(),
        io::bil("fecha", "date"),
        io::bil("creado", "created"),
        io::bil("modulo", "module"),
        io::bil("tipo", "type"),
        io::bil("estado", "status"),
        io::bil("titulo", "title"),
        io::bil("descripcion", "description"),
        io::bil("monto", "amount"),
        io::bil("contraparte", "counterparty"),
        io::bil("etiquetas", "tags"),
        io::bil("notas", "notes"),
        io::bil("relacionados", "related"),
    ]
}

fn fila_evento_bus(ev: &EventoSistema) -> Vec<String> {
    vec![
        ev.id.clone(),
        ev.fecha.format("%Y-%m-%d").to_string(),
        ev.creado.format("%Y-%m-%d %H:%M:%S").to_string(),
        ev.origen.to_string(),
        format!("{:?}", ev.tipo),
        format!("{:?}", ev.estado),
        ev.titulo.clone(),
        ev.descripcion.clone(),
        ev.monto.map(|m| format!("{:.2}", m)).unwrap_or_default(),
        ev.contraparte.clone(),
        ev.etiquetas.join(";"),
        ev.notas.join(" | "),
        ev.eventos_relacionados.join(";"),
    ]
}

pub fn bitacora_exportar(state: &AppState) {
    let formato = match pedir_formato_export() {
        Some(f) => f,
        None => return,
    };
    let cabs = cabeceras_bitacora();
    let filas: Vec<Vec<String>> = state.bus.todos().iter().map(fila_evento_bus).collect();
    // Para JSON serializamos los EventoSistema completos (ida y vuelta perfecta)
    let json = serde_json::to_string_pretty(state.bus.todos()).unwrap_or_default();
    let salidas = exportar_segun_formato(
        "bitacora",
        "bitacora",
        "bitacora",
        "Bitácora del sistema",
        &cabs,
        &filas,
        &json,
        formato,
    );
    escribir_resultado(&salidas, filas.len());
}

pub fn bitacora_importar(state: &mut AppState) {
    let ruta = match pedir_archivo_para_importar("bitacora") {
        Some(r) => r,
        None => return,
    };
    let ext = ruta
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    // Sólo JSON permite restaurar fielmente los enums TipoEvento/Modulo/Estado
    // y las referencias/adjuntos. CSV/Excel se aplazan a una importación futura
    // más permisiva si hace falta.
    let nuevos: Vec<EventoSistema> = match ext.as_str() {
        "json" => match io::leer_json::<Vec<EventoSistema>>(&ruta) {
            Ok(v) => v,
            Err(e) => {
                println!("  {} JSON inválido: {}", "✗".red(), e);
                pausa();
                return;
            }
        },
        _ => {
            println!(
                "  {} Por ahora la bitácora sólo importa desde JSON (los enums",
                "ℹ️".cyan()
            );
            println!("      Modulo/TipoEvento/EstadoEvento exigen ida-y-vuelta exacta).");
            pausa();
            return;
        }
    };

    if nuevos.is_empty() {
        println!("  {} El archivo no contenía eventos.", "ℹ️".cyan());
        pausa();
        return;
    }
    let n = nuevos.len();
    if !confirmar(
        &format!(
            "¿Importar {} evento(s) (se omiten los IDs ya existentes)?",
            n
        ),
        true,
    ) {
        return;
    }

    let mut anexados = 0usize;
    let mut omitidos = 0usize;
    for ev in nuevos {
        if state.bus.buscar(&ev.id).is_some() {
            omitidos += 1;
            continue;
        }
        let _ = state.bus.emitir(ev);
        anexados += 1;
    }
    println!(
        "  {} {} evento(s) anexado(s)",
        "✅".green(),
        anexados.to_string().bold()
    );
    if omitidos > 0 {
        println!(
            "  {} {} evento(s) omitido(s) por ID duplicado",
            "ℹ️".cyan(),
            omitidos
        );
    }
    pausa();
}

pub fn menu_io_bitacora(state: &mut AppState) {
    submenu_io(
        "📤 Importar / Exportar Bitácora",
        state,
        bitacora_exportar,
        bitacora_importar,
    );
}
