//! Router NLP multi-módulo (Fase 6).
//!
//! Extiende el asistente financiero para que TODOS los módulos del sistema
//! sean accesibles mediante lenguaje natural: tareas, agenda (crear),
//! calendario, memoria, contraseñas, rastreador de deudas.
//!
//! Este archivo expone:
//!   - `detectar_intent_modulo(consulta)`: heurística de dominio cruzada
//!     que retorna la categoría correcta cuando el clasificador genérico
//!     gana (Consultar/Crear/Listar/Buscar/Desconocido).
//!   - `responder_*`: handlers individuales por intent.
//!
//! El despachador principal sigue viviendo en `asistente::responder`,
//! que ahora recibe `&mut AppState` y enruta a estos handlers.

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveTime};

use super::asistente::RespuestaAsistente;
use super::intent::CategoriaIntencion;
use crate::agenda::{Agenda, Evento, TipoEvento};
use crate::contrasenias::{self, AlmacenContrasenias};
use crate::memoria::{Memoria, Recuerdo};
use crate::tasks::{Prioridad, Task, TaskManager, TaskStatus};

// ─── Normalización ──────────────────────────────────────────────────────────

/// Quita tildes para que "cuándo" y "cuando" sean equivalentes.
pub(super) fn sin_tildes(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'Á' | 'À' | 'Ä' => 'a',
            'é' | 'è' | 'ë' | 'É' | 'È' | 'Ë' => 'e',
            'í' | 'ì' | 'ï' | 'Í' | 'Ì' | 'Ï' => 'i',
            'ó' | 'ò' | 'ö' | 'Ó' | 'Ò' | 'Ö' => 'o',
            'ú' | 'ù' | 'ü' | 'Ú' | 'Ù' | 'Ü' => 'u',
            'ñ' | 'Ñ' => 'n',
            other => other,
        })
        .collect()
}

// ─── Detector de dominio multi-módulo ───────────────────────────────────────

/// Heurística que detecta intents de cualquier módulo (no solo financiero).
///
/// Se invoca cuando el clasificador NLP genérico ganó pero podemos rescatar
/// un intent de dominio más específico. Si retorna `None`, el llamador debe
/// caer al detector financiero (`asistente::intent_financiero`).
pub fn detectar_intent_modulo(consulta: &str) -> Option<CategoriaIntencion> {
    let norm = sin_tildes(&consulta.to_lowercase());
    let words: Vec<&str> = norm.split_whitespace().collect();
    let has_word = |w: &str| words.contains(&w);
    let has = |s: &str| norm.as_str().contains(s);

    // ── Tareas ──────────────────────────────────────────────────────────
    // Completar tarea (verbo + "tarea"/"pendiente")
    if (has("marcar")
        || has("marca ")
        || has("marco ")
        || has("completar")
        || has("complete ")
        || has("hecho ")
        || has("ya hice")
        || has("ya termine")
        || has("ya complete"))
        && (has_word("tarea") || has_word("pendiente") || has_word("pendientes"))
    {
        return Some(CategoriaIntencion::CompletarTarea);
    }
    // Consultar tareas (vencidas / pendientes / del día)
    if has("que tareas")
        || has("mis tareas")
        || has("tareas pendientes")
        || has("tareas vencidas")
        || has("tareas de hoy")
        || has("tareas para hoy")
        || has("que pendientes")
        || has("mis pendientes")
        || has("que tengo que hacer")
        || has("que se me paso")
        || has("listar tareas")
        || has("ver tareas")
    {
        return Some(CategoriaIntencion::ConsultarTareas);
    }
    // Crear tarea
    let crear_tarea = (has("agregar tarea") || has("nueva tarea") || has("crear tarea")
        || has("anadir tarea") || has("anota que") || has("apunta que")
        || has("recordar que tengo que") || has("tengo que ") || has("debo ")
        || has("hay que ") || has("crea pendiente") || has("agregar pendiente")
        || has("nuevo pendiente"))
        && !has("pago") && !has("pagar") // evitar colisión con AgendarPago
        && !has("cumple") // evitar colisión con CrearEvento
        && !has("cita") && !has("reunion");
    if crear_tarea {
        return Some(CategoriaIntencion::CrearTarea);
    }

    // ── Agenda: crear evento ────────────────────────────────────────────
    // "agendar cita con", "tengo reunion el", "el cumple de X es"
    if has("agendar cita")
        || has("agenda cita")
        || has("nueva cita")
        || has("agendar reunion")
        || has("agenda reunion")
        || has("nueva reunion")
        || has("agendar evento")
        || has("nuevo evento")
        || has("crear evento")
        || has("crear cita")
        || has("crear reunion")
        || has("agregar evento")
        || has("agregar cita")
        || has("agregar reunion")
        || has("registrar cumple")
        || has("agregar cumple")
        || has("crear cumple")
        || has("anadir cumple")
        || has("guardar cumple")
        || has("el cumple de")
        || has("el cumpleanios de")
        || has("el cumpleano de")
        || (has("nacio ") && has("el "))
        || (has_word("tengo") && (has_word("reunion") || has_word("cita")) && has_word("el"))
    {
        // Discriminación: si solo dice "consulta" sin verbo de creación, no es Crear
        let es_consulta =
            has("cuando es") || has("en cuantos dias") || has("que tengo") || has("que hay");
        if !es_consulta {
            return Some(CategoriaIntencion::CrearEvento);
        }
    }

    // ── Calendario: cálculos de fechas ──────────────────────────────────
    if has("cuantos dias entre")
        || has("cuantos dias hay entre")
        || has("distancia entre")
        || has("dias desde")
        || has("dias hasta")
        || has("que fecha sera")
        || has("que dia sera")
        || has("que dia es ")
        || has("suma ") && has(" dias")
        || has("avanza ") && has(" dias")
        || has("avanzar ") && has(" dias")
        || has("dentro de ") && has_word("dias")
        || has("en cuantos dias es")
    // ambigüo: puede ser cumple o fecha
    {
        // "en cuantos dias es el cumpleanios de X" debe ir a ConsultarAgenda,
        // no aquí. Solo capturar si NO menciona persona/evento.
        let menciona_evento = has("cumple")
            || has("cita")
            || has("reunion")
            || has("evento")
            || has("recordatorio")
            || has("aniversario");
        if !menciona_evento {
            return Some(CategoriaIntencion::CalcularFecha);
        }
    }

    // ── Memoria ─────────────────────────────────────────────────────────
    if has("recuerda que")
        || has("anota en memoria")
        || has("guarda en memoria")
        || has("guarda este recuerdo")
        || has("memoriza ")
        || has("apunta en memoria")
    {
        return Some(CategoriaIntencion::CrearRecuerdo);
    }
    if has("que sabes de")
        || has("que recuerdas de")
        || has("que recuerdas sobre")
        || has("buscar en memoria")
        || has("busca en memoria")
        || has("recuerdas algo de")
        || has("recuerdas algo sobre")
        || has("que tengo guardado de")
        || has("muestrame recuerdos")
    {
        return Some(CategoriaIntencion::BuscarMemoria);
    }

    // ── Contraseñas ─────────────────────────────────────────────────────
    if has("genera ")
        && (has("contrasenia") || has("contrasena") || has("password") || has("clave"))
        || has("generar contrasenia")
        || has("generar contrasena")
        || has("generar password")
        || has("dame una contrasenia")
        || has("dame una clave")
        || has("dame un password")
        || has("crea una contrasenia")
        || has("crear password")
        || has("contrasenia segura")
        || has("clave segura")
        || has("password seguro")
    {
        return Some(CategoriaIntencion::GenerarPassword);
    }
    if has("evaluar contrasenia")
        || has("evalua esta") && has("clave")
        || has("evaluar clave")
        || has("que tan segura")
        || has("que tan seguro")
        || has("fortaleza de") && (has("clave") || has("contrasenia") || has("password"))
        || has("es segura mi")
        || has("es seguro mi password")
    {
        return Some(CategoriaIntencion::EvaluarPassword);
    }
    if has("cual es la contrasenia")
        || has("cual es la clave")
        || has("cual es el password")
        || has("muestrame la contrasenia")
        || has("muestrame la clave")
        || has("muestrame el password")
        || has("buscar contrasenia")
        || has("buscar clave")
        || has("buscar password")
        || has("token de")
        || has("la clave de ")
        || has("contrasenia de ")
        || has("password de ")
    {
        return Some(CategoriaIntencion::BuscarPassword);
    }

    // ── Rastreador: consulta de deudas (sin pedir estrategia) ───────────
    if has("que deudas tengo")
        || has("muestrame mis deudas")
        || has("mis creditos")
        || has("cuales son mis deudas")
        || has("listame las deudas")
        || has("listar deudas")
        || has("ver deudas")
    {
        return Some(CategoriaIntencion::ConsultarDeudas);
    }

    None
}

// ─── Handlers: Tareas ───────────────────────────────────────────────────────

pub fn responder_crear_tarea(
    consulta: &str,
    conf: f64,
    tasks: &mut TaskManager,
) -> RespuestaAsistente {
    let titulo = extraer_titulo_tarea(consulta);
    if titulo.is_empty() {
        return RespuestaAsistente::solo_texto(
            CategoriaIntencion::CrearTarea,
            conf,
            "No detecté el título de la tarea. Intenta así: \"agregar tarea estudiar cálculo mañana\".",
        );
    }
    let fecha = extraer_fecha(consulta).unwrap_or_else(|| Local::now().date_naive());
    let hora = NaiveTime::from_hms_opt(9, 0, 0).unwrap();
    let prioridad = if has_norm(consulta, "urgente") {
        Prioridad::Urgente
    } else if has_norm(consulta, "alta") || has_norm(consulta, "importante") {
        Prioridad::Alta
    } else {
        Prioridad::Media
    };
    let t = Task::new(titulo.clone(), String::new(), fecha, hora, prioridad);
    let id = t.id.clone();
    tasks.agregar(t);
    RespuestaAsistente::con_accion(
        CategoriaIntencion::CrearTarea,
        conf,
        format!(
            "✅ Tarea creada: \"{}\"\n   📅 Fecha: {}\n   🆔 ID: {}",
            titulo,
            fecha.format("%d/%m/%Y"),
            id
        ),
        format!("Crear tarea \"{}\" para {}", titulo, fecha),
    )
}

pub fn responder_consultar_tareas(
    consulta: &str,
    conf: f64,
    tasks: &TaskManager,
) -> RespuestaAsistente {
    let norm = sin_tildes(&consulta.to_lowercase());
    let hoy = Local::now().date_naive();

    // Vencidas
    if norm.contains("vencida") || norm.contains("se me paso") {
        let venc = tasks.listar_vencidas();
        if venc.is_empty() {
            return RespuestaAsistente::solo_texto(
                CategoriaIntencion::ConsultarTareas,
                conf,
                "🎉 No tienes tareas vencidas.",
            );
        }
        let mut texto = format!("⚠️ Tareas vencidas ({}):\n", venc.len());
        for t in venc.iter().take(10) {
            texto.push_str(&format!(
                "  • [{}] {} — {} ({})\n",
                t.id,
                t.titulo,
                t.fecha.format("%d/%m/%Y"),
                t.prioridad
            ));
        }
        return RespuestaAsistente::solo_texto(CategoriaIntencion::ConsultarTareas, conf, texto);
    }

    // De hoy
    if norm.contains("de hoy") || norm.contains("para hoy") {
        let hoy_t = tasks.listar_por_fecha(hoy);
        if hoy_t.is_empty() {
            return RespuestaAsistente::solo_texto(
                CategoriaIntencion::ConsultarTareas,
                conf,
                "📋 No tienes tareas para hoy.",
            );
        }
        let mut texto = format!("📋 Tareas de hoy ({}):\n", hoy_t.len());
        for t in &hoy_t {
            texto.push_str(&format!(
                "  • [{}] {} {} — {}\n",
                t.id, t.estado, t.titulo, t.prioridad
            ));
        }
        return RespuestaAsistente::solo_texto(CategoriaIntencion::ConsultarTareas, conf, texto);
    }

    // Pendientes (default)
    let pend = tasks.listar_pendientes();
    if pend.is_empty() {
        return RespuestaAsistente::solo_texto(
            CategoriaIntencion::ConsultarTareas,
            conf,
            "🎉 No tienes tareas pendientes.",
        );
    }
    let mut texto = format!("📋 Tareas pendientes ({}):\n", pend.len());
    for t in pend.iter().take(15) {
        texto.push_str(&format!(
            "  • [{}] {} — {} ({})\n",
            t.id,
            t.titulo,
            t.fecha.format("%d/%m/%Y"),
            t.prioridad
        ));
    }
    if pend.len() > 15 {
        texto.push_str(&format!("  ... y {} más\n", pend.len() - 15));
    }
    RespuestaAsistente::solo_texto(CategoriaIntencion::ConsultarTareas, conf, texto)
}

pub fn responder_completar_tarea(
    consulta: &str,
    conf: f64,
    tasks: &mut TaskManager,
) -> RespuestaAsistente {
    let titulo_buscado = extraer_titulo_tarea(consulta);
    if titulo_buscado.is_empty() {
        return RespuestaAsistente::solo_texto(
            CategoriaIntencion::CompletarTarea,
            conf,
            "¿Qué tarea quieres marcar como completada? Indica su título o ID.",
        );
    }
    // Buscar primera tarea pendiente cuyo título contenga el texto
    let needle = sin_tildes(&titulo_buscado.to_lowercase());
    let id_match = tasks
        .listar_pendientes()
        .iter()
        .find(|t| sin_tildes(&t.titulo.to_lowercase()).contains(&needle))
        .map(|t| t.id.clone());

    if let Some(id) = id_match {
        let titulo = tasks
            .buscar(&id)
            .map(|t| t.titulo.clone())
            .unwrap_or_default();
        if let Some(t) = tasks.buscar_mut(&id) {
            t.cambiar_estado(TaskStatus::Completada);
        }
        return RespuestaAsistente::con_accion(
            CategoriaIntencion::CompletarTarea,
            conf,
            format!("✅ Tarea \"{}\" marcada como completada.", titulo),
            format!("Completar tarea {}", id),
        );
    }
    RespuestaAsistente::solo_texto(
        CategoriaIntencion::CompletarTarea,
        conf,
        format!(
            "No encontré ninguna tarea pendiente con \"{}\". Prueba con \"mis tareas\" para ver la lista.",
            titulo_buscado
        ),
    )
}

// ─── Handlers: Agenda crear ─────────────────────────────────────────────────

pub fn responder_crear_evento(
    consulta: &str,
    conf: f64,
    agenda: &mut Agenda,
) -> RespuestaAsistente {
    let norm = sin_tildes(&consulta.to_lowercase());
    let tipo = if norm.contains("cumple") || norm.contains("nacio") {
        TipoEvento::Cumpleanos
    } else if norm.contains("cita") {
        TipoEvento::Cita
    } else if norm.contains("reunion") || norm.contains("junta") {
        TipoEvento::Reunion
    } else if norm.contains("recordatorio") {
        TipoEvento::Recordatorio
    } else {
        TipoEvento::Otro("evento".to_string())
    };
    let titulo = extraer_titulo_evento(consulta, &tipo);
    if titulo.is_empty() {
        return RespuestaAsistente::solo_texto(
            CategoriaIntencion::CrearEvento,
            conf,
            "No detecté el título del evento. Intenta así: \"agendar cita con dentista el 15 a las 10am\" o \"el cumple de Lucho es el 12 de julio\".",
        );
    }
    let fecha = match extraer_fecha(consulta) {
        Some(f) => f,
        None => {
            return RespuestaAsistente::solo_texto(
                CategoriaIntencion::CrearEvento,
                conf,
                format!(
                    "Detecté \"{}\" pero no entendí la fecha. Indica un día como \"el 15 de mayo\" o \"mañana\".",
                    titulo
                ),
            );
        }
    };
    let hora = extraer_hora(consulta).unwrap_or_else(|| NaiveTime::from_hms_opt(9, 0, 0).unwrap());

    let mut ev = Evento::new(
        titulo.clone(),
        String::new(),
        tipo.clone(),
        fecha,
        hora,
        None,
    );
    if matches!(tipo, TipoEvento::Cumpleanos) {
        ev = ev.con_frecuencia(crate::agenda::Frecuencia::Anual);
    }
    let id = ev.id.clone();
    agenda.agregar_evento(ev);

    let icono = match tipo {
        TipoEvento::Cumpleanos => "🎂",
        TipoEvento::Cita => "📅",
        TipoEvento::Reunion => "🤝",
        TipoEvento::Recordatorio => "🔔",
        TipoEvento::Pago => "💸",
        TipoEvento::FollowUp => "🔄",
        TipoEvento::Otro(_) => "📌",
    };
    RespuestaAsistente::con_accion(
        CategoriaIntencion::CrearEvento,
        conf,
        format!(
            "{} Evento creado: \"{}\"\n   📅 Fecha: {}\n   🕐 Hora: {}\n   🆔 ID: {}",
            icono,
            titulo,
            fecha.format("%d/%m/%Y"),
            hora.format("%H:%M"),
            id
        ),
        format!("Crear evento \"{}\" para {}", titulo, fecha),
    )
}

// ─── Handlers: Calendario ───────────────────────────────────────────────────

pub fn responder_calcular_fecha(consulta: &str, conf: f64) -> RespuestaAsistente {
    let norm = sin_tildes(&consulta.to_lowercase());

    // Avanzar N días desde una fecha
    if norm.contains("avanza")
        || norm.contains("avanzar")
        || norm.contains("dentro de")
        || norm.contains("suma ")
        || norm.contains("que fecha sera")
    {
        if let Some(dias) = extraer_numero_dias(&norm) {
            let base = extraer_fecha(consulta).unwrap_or_else(|| Local::now().date_naive());
            let resultado = base + Duration::days(dias as i64);
            return RespuestaAsistente::solo_texto(
                CategoriaIntencion::CalcularFecha,
                conf,
                format!(
                    "📐 {} días después de {} es {} ({}).",
                    dias,
                    base.format("%d/%m/%Y"),
                    resultado.format("%d/%m/%Y"),
                    nombre_dia(resultado.weekday().num_days_from_monday())
                ),
            );
        }
    }

    // Distancia entre dos fechas
    let fechas = extraer_fechas_multiples(consulta);
    if fechas.len() >= 2 {
        let (a, b) = (fechas[0], fechas[1]);
        let dias = (b - a).num_days().abs();
        return RespuestaAsistente::solo_texto(
            CategoriaIntencion::CalcularFecha,
            conf,
            format!(
                "📏 Entre {} y {} hay {} día{}.",
                a.format("%d/%m/%Y"),
                b.format("%d/%m/%Y"),
                dias,
                if dias == 1 { "" } else { "s" }
            ),
        );
    }

    RespuestaAsistente::solo_texto(
        CategoriaIntencion::CalcularFecha,
        conf,
        "No entendí el cálculo. Ejemplos: \"cuántos días entre el 5 de mayo y el 20 de julio\" o \"qué fecha será 90 días después del 1 de junio\".",
    )
}

// ─── Handlers: Memoria ──────────────────────────────────────────────────────

pub fn responder_crear_recuerdo(
    consulta: &str,
    conf: f64,
    memoria: &mut Memoria,
) -> RespuestaAsistente {
    let contenido = extraer_contenido_recuerdo(consulta);
    if contenido.is_empty() {
        return RespuestaAsistente::solo_texto(
            CategoriaIntencion::CrearRecuerdo,
            conf,
            "¿Qué quieres recordar? Intenta así: \"recuerda que la wifi del café es CafeNet123\".",
        );
    }
    // Palabras clave: tomar sustantivos del contenido (heurística simple)
    let palabras: Vec<String> = contenido
        .split_whitespace()
        .filter(|w| w.len() >= 4)
        .take(5)
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect();
    let r = Recuerdo::new(contenido.clone(), palabras);
    let id = r.id.clone();
    memoria.agregar_recuerdo(r);
    RespuestaAsistente::con_accion(
        CategoriaIntencion::CrearRecuerdo,
        conf,
        format!("🧠 Recuerdo guardado: \"{}\"\n   🆔 ID: {}", contenido, id),
        "Crear recuerdo".to_string(),
    )
}

pub fn responder_buscar_memoria(
    consulta: &str,
    conf: f64,
    memoria: &Memoria,
) -> RespuestaAsistente {
    let kw = extraer_keyword_memoria(consulta);
    if kw.is_empty() {
        return RespuestaAsistente::solo_texto(
            CategoriaIntencion::BuscarMemoria,
            conf,
            "¿Sobre qué quieres que busque? Intenta: \"qué sabes de la reunión con Acme\".",
        );
    }
    let resultados = memoria.buscar(&kw);
    if resultados.is_empty() {
        return RespuestaAsistente::solo_texto(
            CategoriaIntencion::BuscarMemoria,
            conf,
            format!("🤔 No encontré recuerdos sobre \"{}\".", kw),
        );
    }
    let mut texto = format!("🧠 {} recuerdo(s) sobre \"{}\":\n", resultados.len(), kw);
    for r in resultados.iter().take(8) {
        texto.push_str(&format!(
            "  • [{}] {} — {}\n",
            r.id,
            r.contenido,
            r.creado.format("%d/%m/%Y")
        ));
    }
    RespuestaAsistente::solo_texto(CategoriaIntencion::BuscarMemoria, conf, texto)
}

// ─── Handlers: Contraseñas ──────────────────────────────────────────────────

pub fn responder_generar_password(consulta: &str, conf: f64) -> RespuestaAsistente {
    let norm = sin_tildes(&consulta.to_lowercase());
    let longitud = extraer_numero_dias(&norm)
        .map(|n| n as usize)
        .unwrap_or(16)
        .clamp(8, 64);
    let con_especiales = !(norm.contains("sin especiales") || norm.contains("sin simbolos"));
    let pwd = contrasenias::generar_contrasenia(longitud, con_especiales);
    let (puntaje, _) = contrasenias::evaluar_fortaleza(&pwd);
    RespuestaAsistente::solo_texto(
        CategoriaIntencion::GenerarPassword,
        conf,
        format!(
            "🔑 Contraseña generada ({} caracteres, fortaleza {}/100):\n   {}\n   ⚠️  Guárdala antes de cerrar.",
            longitud, puntaje, pwd
        ),
    )
}

pub fn responder_evaluar_password(consulta: &str, conf: f64) -> RespuestaAsistente {
    // Extraer la contraseña entre comillas o última palabra
    let pwd = if let Some(start) = consulta.find('"') {
        if let Some(end) = consulta[start + 1..].find('"') {
            consulta[start + 1..start + 1 + end].to_string()
        } else {
            String::new()
        }
    } else if let Some(start) = consulta.find('\'') {
        if let Some(end) = consulta[start + 1..].find('\'') {
            consulta[start + 1..start + 1 + end].to_string()
        } else {
            String::new()
        }
    } else {
        // Última palabra de longitud >= 4
        consulta
            .split_whitespace()
            .rfind(|w| w.len() >= 4)
            .unwrap_or("")
            .to_string()
    };
    if pwd.is_empty() {
        return RespuestaAsistente::solo_texto(
            CategoriaIntencion::EvaluarPassword,
            conf,
            "Indica la contraseña entre comillas. Ejemplo: evalúa \"miClave2024!\"",
        );
    }
    let (puntaje, sugerencias) = contrasenias::evaluar_fortaleza(&pwd);
    let nivel = match puntaje {
        0..=30 => "🔴 Débil",
        31..=60 => "🟡 Aceptable",
        61..=80 => "🟢 Fuerte",
        _ => "🟢 Excelente",
    };
    RespuestaAsistente::solo_texto(
        CategoriaIntencion::EvaluarPassword,
        conf,
        format!("🔐 Fortaleza: {} ({}/100)\n{}", nivel, puntaje, sugerencias),
    )
}

pub fn responder_buscar_password(
    consulta: &str,
    conf: f64,
    almacen: &AlmacenContrasenias,
) -> RespuestaAsistente {
    let kw = extraer_keyword_password(consulta);
    if kw.is_empty() {
        return RespuestaAsistente::solo_texto(
            CategoriaIntencion::BuscarPassword,
            conf,
            "¿Qué contraseña buscas? Ejemplo: \"cuál es la contraseña de Netflix\".",
        );
    }
    let resultados = almacen.buscar(&kw);
    if resultados.is_empty() {
        return RespuestaAsistente::solo_texto(
            CategoriaIntencion::BuscarPassword,
            conf,
            format!("🔒 No encontré contraseñas para \"{}\".", kw),
        );
    }
    let mut texto = format!("🔐 {} resultado(s) para \"{}\":\n", resultados.len(), kw);
    for e in resultados.iter().take(5) {
        texto.push_str(&format!(
            "  • {} ({}) — usuario: {}\n    🆔 {}  [usa el menú 🔐 Contraseñas para ver la clave]\n",
            e.nombre, e.categoria, e.usuario, e.id
        ));
    }
    RespuestaAsistente::solo_texto(CategoriaIntencion::BuscarPassword, conf, texto)
}

// ─── Handlers: Rastreador deudas ────────────────────────────────────────────

pub fn responder_consultar_deudas(
    conf: f64,
    rastreador: &crate::ml::advisor::RastreadorDeudas,
) -> RespuestaAsistente {
    let activas = rastreador.deudas_activas();
    if activas.is_empty() {
        return RespuestaAsistente::solo_texto(
            CategoriaIntencion::ConsultarDeudas,
            conf,
            "🎉 No tienes deudas activas registradas.",
        );
    }
    let total = rastreador.deuda_total_actual();
    let pago_min = rastreador.pagos_minimos_mensuales();
    let mut texto = format!(
        "💳 Tienes {} deuda(s) activa(s):\n   💰 Total: ${:.2}\n   📉 Pago mínimo mensual: ${:.2}\n\n",
        activas.len(),
        total,
        pago_min
    );
    for d in activas.iter().take(10) {
        texto.push_str(&format!(
            "  • {} — saldo ${:.2}\n",
            d.nombre,
            d.saldo_actual()
        ));
    }
    RespuestaAsistente::solo_texto(CategoriaIntencion::ConsultarDeudas, conf, texto)
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn has_norm(consulta: &str, needle: &str) -> bool {
    sin_tildes(&consulta.to_lowercase()).contains(needle)
}

fn extraer_titulo_tarea(consulta: &str) -> String {
    // Quita los disparadores comunes
    let bajada = consulta.to_lowercase();
    let triggers = [
        "agregar tarea ",
        "nueva tarea ",
        "crear tarea ",
        "anadir tarea ",
        "anota que ",
        "apunta que ",
        "tengo que ",
        "debo ",
        "hay que ",
        "marcar ",
        "completar ",
        "marca ",
        "marco ",
        "ya hice ",
        "ya termine ",
        "ya complete ",
        "agregar pendiente ",
        "nuevo pendiente ",
        "crea pendiente ",
        "recordar que tengo que ",
    ];
    let bajada_norm = sin_tildes(&bajada);
    for t in triggers {
        if let Some(pos) = bajada_norm.find(t) {
            let resto = &consulta[pos + t.len()..];
            // Cortar al detectar "el ", "mañana", "hoy", número de día
            let resto = recortar_antes_de_fecha(resto);
            return resto.trim().trim_matches('.').to_string();
        }
    }
    String::new()
}

fn extraer_titulo_evento(consulta: &str, tipo: &TipoEvento) -> String {
    let bajada = consulta.to_lowercase();
    let bajada_norm = sin_tildes(&bajada);
    let triggers: &[&str] = match tipo {
        TipoEvento::Cumpleanos => &[
            "el cumple de ",
            "el cumpleanios de ",
            "el cumpleano de ",
            "cumpleanios de ",
            "cumple de ",
            "registrar cumple de ",
            "agregar cumple de ",
            "crear cumple de ",
            "guardar cumple de ",
            "anadir cumple de ",
            "nacio ",
        ],
        TipoEvento::Cita => &[
            "agendar cita con ",
            "agenda cita con ",
            "nueva cita con ",
            "crear cita con ",
            "agregar cita con ",
            "cita con ",
        ],
        TipoEvento::Reunion => &[
            "agendar reunion con ",
            "agenda reunion con ",
            "nueva reunion con ",
            "crear reunion con ",
            "agregar reunion con ",
            "tengo reunion con ",
            "reunion con ",
            "junta con ",
        ],
        _ => &[
            "agendar evento ",
            "nuevo evento ",
            "crear evento ",
            "agregar evento ",
        ],
    };
    for t in triggers {
        if let Some(pos) = bajada_norm.find(t) {
            let resto = &consulta[pos + t.len()..];
            let resto = recortar_antes_de_fecha(resto);
            let limpio = resto.trim().trim_matches('.').to_string();
            if !limpio.is_empty() {
                let prefijo = match tipo {
                    TipoEvento::Cumpleanos => "Cumpleaños de ",
                    TipoEvento::Cita => "Cita con ",
                    TipoEvento::Reunion => "Reunión con ",
                    _ => "",
                };
                return format!("{}{}", prefijo, limpio);
            }
        }
    }
    String::new()
}

fn extraer_contenido_recuerdo(consulta: &str) -> String {
    let triggers = [
        "recuerda que ",
        "anota en memoria que ",
        "guarda en memoria que ",
        "guarda este recuerdo: ",
        "memoriza que ",
        "memoriza ",
        "apunta en memoria que ",
    ];
    let bajada = sin_tildes(&consulta.to_lowercase());
    for t in triggers {
        if let Some(pos) = bajada.find(t) {
            return consulta[pos + t.len()..].trim().to_string();
        }
    }
    String::new()
}

fn extraer_keyword_memoria(consulta: &str) -> String {
    let triggers = [
        "que sabes de ",
        "que sabes sobre ",
        "que recuerdas de ",
        "que recuerdas sobre ",
        "buscar en memoria ",
        "busca en memoria ",
        "recuerdas algo de ",
        "recuerdas algo sobre ",
        "que tengo guardado de ",
        "muestrame recuerdos de ",
    ];
    let bajada = sin_tildes(&consulta.to_lowercase());
    for t in triggers {
        if let Some(pos) = bajada.find(t) {
            let resto = &consulta[pos + t.len()..];
            return resto
                .trim()
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_string();
        }
    }
    String::new()
}

fn extraer_keyword_password(consulta: &str) -> String {
    let triggers = [
        "cual es la contrasenia de ",
        "cual es la clave de ",
        "cual es el password de ",
        "muestrame la contrasenia de ",
        "muestrame la clave de ",
        "muestrame el password de ",
        "buscar contrasenia ",
        "buscar clave ",
        "buscar password ",
        "token de ",
        "la clave de ",
        "contrasenia de ",
        "password de ",
    ];
    let bajada = sin_tildes(&consulta.to_lowercase());
    for t in triggers {
        if let Some(pos) = bajada.find(t) {
            let resto = &consulta[pos + t.len()..];
            return resto
                .trim()
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_string();
        }
    }
    String::new()
}

fn extraer_numero_dias(norm: &str) -> Option<u32> {
    // Busca el primer número en la cadena
    let mut buf = String::new();
    for c in norm.chars() {
        if c.is_ascii_digit() {
            buf.push(c);
        } else if !buf.is_empty() {
            return buf.parse().ok();
        }
    }
    if !buf.is_empty() {
        buf.parse().ok()
    } else {
        None
    }
}

fn nombre_dia(idx: u32) -> &'static str {
    match idx {
        0 => "lunes",
        1 => "martes",
        2 => "miércoles",
        3 => "jueves",
        4 => "viernes",
        5 => "sábado",
        _ => "domingo",
    }
}

fn recortar_antes_de_fecha(s: &str) -> &str {
    let bajada = sin_tildes(&s.to_lowercase());
    let marcadores = [
        " el ",
        " mañana",
        " manana",
        " hoy",
        " ayer",
        " a las ",
        " a la ",
        " para el ",
        " para mañana",
        " para manana",
        " para hoy",
    ];
    let mut corte = s.len();
    for m in marcadores {
        if let Some(p) = bajada.find(m) {
            if p < corte {
                corte = p;
            }
        }
    }
    &s[..corte]
}

/// Extrae UNA fecha (la primera). Reutiliza la lógica de `asistente`.
fn extraer_fecha(consulta: &str) -> Option<NaiveDate> {
    super::asistente::extraer_fecha_publica(consulta)
}

/// Extrae HASTA dos fechas para "distancia entre fechas".
fn extraer_fechas_multiples(consulta: &str) -> Vec<NaiveDate> {
    let mut fechas = Vec::new();
    let mut resto = consulta.to_string();
    while let Some(f) = super::asistente::extraer_fecha_publica(&resto) {
        fechas.push(f);
        // Quitar la primera ocurrencia de la representación de la fecha:
        // imposible exacto, así que sustituimos el primer dígito por espacio
        // para forzar a que la próxima búsqueda mire después.
        if let Some(pos) = resto.find(|c: char| c.is_ascii_digit()) {
            // Saltar el bloque de dígitos completo
            let mut end = pos;
            for c in resto[pos..].chars() {
                if c.is_ascii_digit() || c == '/' || c == '-' || c == ' ' {
                    end += c.len_utf8();
                } else {
                    break;
                }
            }
            resto.replace_range(pos..end, " ");
        } else {
            break;
        }
        if fechas.len() >= 2 {
            break;
        }
    }
    fechas
}

fn extraer_hora(consulta: &str) -> Option<NaiveTime> {
    let norm = sin_tildes(&consulta.to_lowercase());
    // Buscar "a las HH" o "a las HH:MM" o "HHam"/"HHpm"
    let re_idx = norm.find("a las ")?;
    let resto = &norm[re_idx + 6..];
    let mut iter = resto.split_whitespace();
    let token = iter.next()?;
    // Aceptar formatos: 10, 10am, 10pm, 10:30, 10:30am
    let bajada = token.trim_matches(|c: char| !c.is_ascii_digit() && c != ':');
    let (h_str, m_str) = if let Some((a, b)) = bajada.split_once(':') {
        (a, b)
    } else {
        (bajada, "0")
    };
    let mut h: u32 = h_str.parse().ok()?;
    let m: u32 = m_str.parse().ok()?;
    if token.contains("pm") && h < 12 {
        h += 12;
    }
    if token.contains("am") && h == 12 {
        h = 0;
    }
    NaiveTime::from_hms_opt(h, m, 0)
}
