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
        let es_consulta = has("cuando es")
            || has("en cuantos dias")
            || has("que tengo")
            || has("que hay")
            || has("cuanto falta")
            || has("cuantos dias faltan")
            || has("cuantos dias para")
            || has("cuantos dias quedan")
            || has("cuantos anos tiene")
            || has("cuantos anios tiene")
            || has("que edad tiene")
            || has("cual es la edad");
        if !es_consulta {
            return Some(CategoriaIntencion::CrearEvento);
        }
    }

    // ── Calendario: cálculos de fechas ──────────────────────────────────
    // Detectar "cuando es X" / "en qué fecha es X" / "qué día cae X" → CalcularFecha.
    // Va ANTES que el bloque de "cuánto falta" para no perder estas frases.
    // Excluir si menciona cumpleaños de alguien (va a ConsultarAgenda)
    let apunta_cumple_persona = has("cumpleanios")
        || has("cumpleanos")
        || has("cumpleano ")
        || (has("cumple") && (has(" de ") || has(" del ")));
    let pregunta_cuando = (has("cuando es el")
        || has("cuando es la")
        || has("cuando cae el")
        || has("cuando cae la")
        || has("que dia es el")
        || has("que dia es la")
        || has("que dia cae el")
        || has("que dia cae la")
        || has("en que fecha es")
        || has("en que fecha cae")
        || has("cuando es fin de")
        || has("cuando es dia de")
        || has("cuando es el dia")
        || has("cuando es la noche"))
        && !apunta_cumple_persona
        && !has("mi evento")
        && !has("mi cita")
        && !has("mi reunion");
    if pregunta_cuando {
        return Some(CategoriaIntencion::CalcularFecha);
    }

    // "cuando es el cumpleaños de X" → ConsultarAgenda (ya filtrado arriba de CalcularFecha)
    if apunta_cumple_persona
        && (has("cuando es") || has("cuando cae") || has("que dia es") || has("en que fecha"))
    {
        return Some(CategoriaIntencion::ConsultarAgenda);
    }

    // Detección de "cuánto falta para <fecha/feriado>" — debe ir
    // ANTES que cualquier discriminación por "cumple/cita/evento" porque la
    // pregunta puede mencionar feriados como "navidad" sin ser un evento de
    // agenda del usuario.
    let pregunta_falta = has("cuanto falta para")
        || has("cuanto falta hasta")
        || has("cuantos dias para")
        || has("cuantos dias faltan para")
        || has("cuantos dias faltan hasta")
        || has("cuantos dias quedan para")
        || has("cuantos dias quedan hasta")
        || has("dias para ")
        || has("que falta para ");
    if pregunta_falta {
        // Si menciona el cumpleaños/evento de alguien (con o sin "mi"), va a
        // ConsultarAgenda para que busque en la agenda con matching fuzzy.
        let apunta_agenda = has("cumple") && (has(" de ") || has(" del "))
            || has("cumpleanios de")
            || has("cumpleanios del")
            || has("cita de")
            || has("cita con")
            || has("reunion de")
            || has("reunion con")
            || has("evento de")
            || has("aniversario de")
            || has("mi cumple")
            || has("mi cita")
            || has("mi reunion")
            || has("mi evento");
        if apunta_agenda {
            return Some(CategoriaIntencion::ConsultarAgenda);
        }
        return Some(CategoriaIntencion::CalcularFecha);
    }

    // ── Listado de feriados ─────────────────────────────────────────────
    if has("proximos feriados")
        || has("siguientes feriados")
        || has("que feriado")
        || has("que feriados")
        || has("dias festivos")
        || has("dia festivo")
        || has("feriados de ecuador")
        || has("feriados de usa")
        || has("feriados de estados unidos")
        || has("feriados religiosos")
        || has("dias religiosos")
        || has("listar feriados")
        || has("ver feriados")
        || has("muestrame los feriados")
        || has("muestrame feriados")
        || (has_word("feriados") && (has_word("hay") || has_word("vienen") || has_word("quedan")))
    {
        return Some(CategoriaIntencion::ConsultarFeriados);
    }

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

    // ── Agenda: consultar cumpleaños por mes ─────────────────────────────
    // "quien cumple en julio", "quien cumple el proximo mes", "cumpleanos en mayo"
    let meses = [
        "enero",
        "febrero",
        "marzo",
        "abril",
        "mayo",
        "junio",
        "julio",
        "agosto",
        "septiembre",
        "octubre",
        "noviembre",
        "diciembre",
    ];
    let menciona_mes = meses.iter().any(|m| has(m));
    let es_pregunta_cumple = has("quien cumple")
        || has("quienes cumplen")
        || has("quienes cumplean")
        || (has("cumple") && (has("este mes") || has("proximo mes") || has("siguiente mes")))
        || (has("cumpleano") && (menciona_mes || has("este mes") || has("proximo mes")))
        || (has("cumple") && menciona_mes);
    if es_pregunta_cumple {
        return Some(CategoriaIntencion::ConsultarAgenda);
    }

    // ── Edad de una persona ──────────────────────────────────────────────
    // "cuántos años tiene X", "qué edad tiene X", "cuántos años cumple X"
    // Nota: "años" → "anos" (ñ→n), pero muchos escriben "anios" sin ñ
    if has("cuantos anos tiene")
        || has("cuantos anios tiene")
        || has("cuantos anos cumple")
        || has("cuantos anios cumple")
        || has("que edad tiene")
        || has("cuanta edad tiene")
        || has("cual es la edad de")
        || has("cual es su edad")
        || has("cuantos anos va a cumplir")
        || has("cuantos anios va a cumplir")
        || has("cuantos anos cumplio")
        || has("cuantos anios cumplio")
        || has("cuantos anos lleva")
        || has("cuantos anios lleva")
    {
        return Some(CategoriaIntencion::ConsultarAgenda);
    }

    // ── Obras ────────────────────────────────────────────────────────────
    // GuiaSiguientePaso — va ANTES que ConsultarObras para capturar "qué sigue"
    let menciona_obra_o_proyecto = has_word("obra")
        || has("obras")
        || has("proyecto")
        || has("construccion")
        || has("contrato de obra");
    let pregunta_siguiente = has("siguiente paso")
        || has("que sigue")
        || has("que continua")
        || has("que falta por hacer")
        || has("proxima actividad")
        || has("proxima etapa")
        || has("siguiente etapa")
        || has("que viene")
        || has("que hacer ahora")
        || has("como avanzar")
        || has("que paso viene")
        || has("donde quedamos");
    if pregunta_siguiente && menciona_obra_o_proyecto {
        return Some(CategoriaIntencion::GuiaSiguientePaso);
    }

    // AlertasObras — ANTES que ConsultarObras para capturar alertas específicas
    let palabra_alerta = has_word("alerta")
        || has("alertas")
        || has("retraso")
        || has("retrasada")
        || has("retrasado")
        || has("vencido")
        || has("vencida")
        || has("en riesgo")
        || has("problema")
        || has("problemas")
        || has("atrasado")
        || has("atrasada")
        || has("paso vencido")
        || has("pasos vencidos");
    if palabra_alerta && (menciona_obra_o_proyecto || has("proyecto")) {
        return Some(CategoriaIntencion::AlertasObras);
    }

    // SaldoObra — saldo / desembolso / presupuesto de obra
    let palabra_saldo = has_word("saldo")
        || has("desembolso")
        || has("desembolsos")
        || has("cuanto queda")
        || has("cuanto falta") && !has("para el")
        || has("cuanto hemos gastado")
        || has("cuanto llevamos gastado")
        || has("cuanto se ha ejecutado")
        || has("ejecucion presupuestal")
        || has("partidas")
        || has("presupuesto disponible");
    if palabra_saldo && (menciona_obra_o_proyecto) {
        return Some(CategoriaIntencion::SaldoObra);
    }

    // ConsultarObras — estado general de obras
    let pregunta_obras = has("mis obras")
        || has("obras activas")
        || has("obras en curso")
        || has("como van las obras")
        || has("estado de las obras")
        || has("estado de mis obras")
        || has("ver obras")
        || has("listar obras")
        || has("lista de obras")
        || has("cuantas obras")
        || has("proyectos activos")
        || has("mis proyectos")
        || has("avance de las obras")
        || has("porcentaje de avance")
        || has("obras sin terminar")
        || has("obras pendientes")
        || has("progreso de las obras")
        || has("obras con retraso")
        || (has_word("obra") && (has_word("estado") || has_word("ver") || has_word("lista")));
    if pregunta_obras {
        return Some(CategoriaIntencion::ConsultarObras);
    }

    // ── Cobranzas ────────────────────────────────────────────────────────
    let pregunta_cobranzas = has("que me deben")
        || has("cuanto me deben")
        || has("cuentas por cobrar")
        || has("cobrar pendiente")
        || has("cartera por cobrar")
        || has("facturas pendientes de cobro")
        || has("facturas sin pagar")
        || has("clientes que deben")
        || has("deudores")
        || has("cobros pendientes")
        || has("cuantos clientes deben")
        || has("total por cobrar")
        || has("resumen de cobranzas")
        || has("estado de cobranzas")
        || has("cuanto tengo por cobrar")
        || has("quien me debe")
        || (has("cobrar")
            && (has_word("pendiente")
                || has_word("pendientes")
                || has_word("total")
                || has_word("cuanto")));
    if pregunta_cobranzas {
        return Some(CategoriaIntencion::ConsultarCobranzas);
    }

    // ── Empresa ──────────────────────────────────────────────────────────
    let pregunta_empresa = has("como va la empresa")
        || has("estado del negocio")
        || has("estado de la empresa")
        || has("resumen empresarial")
        || has("resumen de la empresa")
        || has("dashboard empresa")
        || has("panorama empresarial")
        || has("como va el negocio")
        || has("mis propuestas")
        || has("propuestas activas")
        || has("cuantas propuestas")
        || has("casos abiertos")
        || has("mis casos")
        || has("proveedores registrados")
        || has("contratos activos")
        || has("negocios en curso")
        || (has_word("empresa")
            && (has_word("resumen")
                || has_word("estado")
                || has_word("como")
                || has_word("dashboard")));
    if pregunta_empresa {
        return Some(CategoriaIntencion::ResumenEmpresa);
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
    let hoy = Local::now().date_naive();

    // 0) "¿Cuándo es / qué día es el X?" — muestra la fecha del feriado
    let pregunta_cuando = norm.contains("cuando es")
        || norm.contains("cuando cae")
        || norm.contains("que dia es el")
        || norm.contains("que dia es la")
        || norm.contains("que dia cae")
        || norm.contains("en que fecha es")
        || norm.contains("en que fecha cae");
    if pregunta_cuando {
        if let Some((fecha, nombre)) = super::feriados::resolver_nombre_feriado(consulta, hoy) {
            let semana = nombre_dia(fecha.weekday().num_days_from_monday());
            let dias = (fecha - hoy).num_days();
            let cuando = if dias == 0 {
                "HOY".to_string()
            } else if dias == 1 {
                "mañana".to_string()
            } else if dias > 0 {
                format!("en {} días", dias)
            } else {
                format!("hace {} días (ya pasó)", -dias)
            };
            return RespuestaAsistente::solo_texto(
                CategoriaIntencion::CalcularFecha,
                conf,
                format!(
                    "📅 {} es el {} ({}) — {}.",
                    nombre,
                    fecha.format("%d/%m/%Y"),
                    semana,
                    cuando
                ),
            );
        }
        // Intentar fecha textual
        if let Some(fecha) = super::feriados::extraer_fecha_textual(consulta, hoy)
            .or_else(|| extraer_fecha(consulta))
        {
            let semana = nombre_dia(fecha.weekday().num_days_from_monday());
            return RespuestaAsistente::solo_texto(
                CategoriaIntencion::CalcularFecha,
                conf,
                format!("📅 Esa fecha es {} ({}).", fecha.format("%d/%m/%Y"), semana),
            );
        }
    }

    // 1) "Cuánto falta para <fecha/feriado>"
    let pregunta_falta = norm.contains("cuanto falta para")
        || norm.contains("cuanto falta hasta")
        || norm.contains("cuantos dias para")
        || norm.contains("cuantos dias faltan para")
        || norm.contains("cuantos dias faltan hasta")
        || norm.contains("cuantos dias quedan para")
        || norm.contains("cuantos dias quedan hasta")
        || norm.contains("dias hasta")
        || norm.contains("en cuantos dias es")
        || norm.contains("que falta para");

    if pregunta_falta {
        // 1a) feriado nombrado ("navidad", "thanksgiving", ...)
        if let Some((fecha, nombre)) = super::feriados::resolver_nombre_feriado(consulta, hoy) {
            return RespuestaAsistente::solo_texto(
                CategoriaIntencion::CalcularFecha,
                conf,
                formatear_dias_para(nombre.as_str(), fecha, hoy),
            );
        }
        // 1b) "25 de diciembre" / "december 25"
        if let Some(fecha) = super::feriados::extraer_fecha_textual(consulta, hoy) {
            return RespuestaAsistente::solo_texto(
                CategoriaIntencion::CalcularFecha,
                conf,
                formatear_dias_para(&fecha.format("%d/%m/%Y").to_string(), fecha, hoy),
            );
        }
        // 1c) fecha numérica "el 25/12" o "25-12-2026"
        if let Some(fecha) = extraer_fecha(consulta) {
            return RespuestaAsistente::solo_texto(
                CategoriaIntencion::CalcularFecha,
                conf,
                formatear_dias_para(&fecha.format("%d/%m/%Y").to_string(), fecha, hoy),
            );
        }
    }

    // 2) Avanzar N días desde una fecha
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

    // 3) Distancia entre dos fechas
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

    // 4) Última red: si hay una sola fecha y la pregunta es "cuándo / cuántos días"
    if let Some(fecha) =
        super::feriados::extraer_fecha_textual(consulta, hoy).or_else(|| extraer_fecha(consulta))
    {
        return RespuestaAsistente::solo_texto(
            CategoriaIntencion::CalcularFecha,
            conf,
            formatear_dias_para(&fecha.format("%d/%m/%Y").to_string(), fecha, hoy),
        );
    }

    RespuestaAsistente::solo_texto(
        CategoriaIntencion::CalcularFecha,
        conf,
        "No entendí el cálculo. Ejemplos:\n  • \"cuántos días faltan para Navidad\"\n  • \"cuánto falta para el 25 de diciembre\"\n  • \"cuántos días entre el 5 de mayo y el 20 de julio\"\n  • \"qué fecha será 90 días después del 1 de junio\"",
    )
}

/// Formatea el mensaje "faltan N días para X (DD/MM/YYYY, día de la semana)".
fn formatear_dias_para(nombre: &str, fecha: NaiveDate, hoy: NaiveDate) -> String {
    let dias = (fecha - hoy).num_days();
    let semana = nombre_dia(fecha.weekday().num_days_from_monday());
    if dias == 0 {
        format!(
            "🎉 ¡{} es HOY ({}, {})!",
            nombre,
            semana,
            fecha.format("%d/%m/%Y")
        )
    } else if dias == 1 {
        format!(
            "⏳ Falta 1 día para {} — mañana ({}, {}).",
            nombre,
            semana,
            fecha.format("%d/%m/%Y")
        )
    } else if dias > 0 {
        format!(
            "⏳ Faltan {} días para {} ({}, {}).",
            dias,
            nombre,
            semana,
            fecha.format("%d/%m/%Y")
        )
    } else {
        format!(
            "📅 {} ya pasó hace {} día{} ({}, {}).",
            nombre,
            -dias,
            if dias == -1 { "" } else { "s" },
            semana,
            fecha.format("%d/%m/%Y")
        )
    }
}

/// Lista los próximos feriados (por defecto Ecuador + USA + religiosos).
pub fn responder_consultar_feriados(consulta: &str, conf: f64) -> RespuestaAsistente {
    use super::feriados::{proximos_feriados, Pais};
    let norm = sin_tildes(&consulta.to_lowercase());
    let hoy = Local::now().date_naive();

    let pais = if norm.contains("ecuador") {
        Some(Pais::Ecuador)
    } else if norm.contains("usa") || norm.contains("estados unidos") {
        Some(Pais::Usa)
    } else if norm.contains("religios") {
        Some(Pais::Religioso)
    } else {
        None
    };

    let n = extraer_numero_dias(&norm)
        .map(|n| n as usize)
        .unwrap_or(10)
        .clamp(1, 30);
    let lista = proximos_feriados(hoy, n, pais);
    if lista.is_empty() {
        return RespuestaAsistente::solo_texto(
            CategoriaIntencion::ConsultarFeriados,
            conf,
            "No encontré feriados próximos en el catálogo.",
        );
    }

    let etiqueta = match pais {
        Some(Pais::Ecuador) => "Ecuador",
        Some(Pais::Usa) => "USA",
        Some(Pais::Religioso) => "religiosos",
        None => "Ecuador + USA + religiosos",
    };
    let mut texto = format!("🎌 Próximos {} feriados ({}):\n", lista.len(), etiqueta);
    for f in &lista {
        let dias = (f.fecha - hoy).num_days();
        let etq_dias = if dias == 0 {
            "hoy".to_string()
        } else if dias == 1 {
            "mañana".to_string()
        } else {
            format!("en {} días", dias)
        };
        let marca = if f.oficial { "🏛" } else { "✨" };
        texto.push_str(&format!(
            "  {} {} — {} ({}, {}) [{}]\n",
            marca,
            f.nombre,
            f.fecha.format("%d/%m/%Y"),
            nombre_dia(f.fecha.weekday().num_days_from_monday()),
            etq_dias,
            f.pais.nombre()
        ));
    }
    RespuestaAsistente::solo_texto(CategoriaIntencion::ConsultarFeriados, conf, texto)
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

// ─── Handlers: Empresa / Obras / Cobranzas (Fase 7) ─────────────────────────

/// Resumen de obras activas con avance y estado.
pub fn responder_consultar_obras(
    conf: f64,
    obras: &crate::obras::AlmacenObras,
) -> RespuestaAsistente {
    let activas = obras.activas();
    if activas.is_empty() {
        return RespuestaAsistente::solo_texto(
            CategoriaIntencion::ConsultarObras,
            conf,
            "📋 No tienes obras activas en este momento. Puedes crear una desde el módulo Empresa → Obras.",
        );
    }
    let mut texto = format!("🏗️  Tienes {} obra(s) activa(s):\n\n", activas.len());
    for obra in activas.iter().take(8) {
        let pasos = obra.validar_flujo_completo();
        let total = pasos.len();
        let completos = pasos
            .iter()
            .filter(|p| matches!(p.estado, crate::obras::EstadoPaso::Completado))
            .count();
        let pct = (completos * 100).checked_div(total).unwrap_or(0);
        let barra = {
            let llenos = pct / 10;
            format!(
                "[{}{}] {}%",
                "█".repeat(llenos),
                "░".repeat(10 - llenos),
                pct
            )
        };
        texto.push_str(&format!(
            "  🔨 {} — {} {}\n",
            obra.nombre,
            barra,
            if pct == 100 {
                "✅"
            } else if pct >= 50 {
                "🔄"
            } else {
                "⏳"
            }
        ));
    }
    if activas.len() > 8 {
        texto.push_str(&format!("  … y {} obra(s) más.\n", activas.len() - 8));
    }
    let mut r = RespuestaAsistente::solo_texto(CategoriaIntencion::ConsultarObras, conf, texto);
    r.seguimientos = vec![
        "¿Cuánto saldo tiene alguna obra?".into(),
        "¿Hay alertas en mis obras?".into(),
        "¿Cuál es el siguiente paso?".into(),
    ];
    r
}

/// Saldo / ejecución presupuestal de obras.
pub fn responder_saldo_obra(conf: f64, obras: &crate::obras::AlmacenObras) -> RespuestaAsistente {
    let activas = obras.activas();
    if activas.is_empty() {
        return RespuestaAsistente::solo_texto(
            CategoriaIntencion::SaldoObra,
            conf,
            "📋 No hay obras activas registradas.",
        );
    }
    let mut texto = String::from("💰 Presupuesto de obras activas:\n\n");
    for obra in activas.iter().take(6) {
        let partidas = &obra.presupuesto_obra.partidas;
        if partidas.is_empty() {
            texto.push_str(&format!(
                "  🏗️  {} — sin partidas de presupuesto registradas\n",
                obra.nombre
            ));
        } else {
            let (solicitado, aprobado, ejecutado): (f64, f64, f64) =
                partidas.iter().fold((0.0, 0.0, 0.0), |(s, a, e), p| {
                    (
                        s + p.monto_solicitado,
                        a + p.monto_aprobado,
                        e + p.monto_ejecutado,
                    )
                });
            let pct_ejec = if aprobado > 0.0 {
                ejecutado / aprobado * 100.0
            } else {
                0.0
            };
            texto.push_str(&format!(
                "  🏗️  {}\n     Solicitado: ${:.2}  |  Aprobado: ${:.2}  |  Ejecutado: ${:.2} ({:.1}%)\n",
                obra.nombre, solicitado, aprobado, ejecutado, pct_ejec
            ));
        }
    }
    let mut r = RespuestaAsistente::solo_texto(CategoriaIntencion::SaldoObra, conf, texto);
    r.seguimientos = vec![
        "¿Cómo van mis obras en general?".into(),
        "¿Hay alertas en los proyectos?".into(),
    ];
    r
}

/// Alertas: pasos vencidos, obras con retraso.
pub fn responder_alertas_obras(
    conf: f64,
    obras: &crate::obras::AlmacenObras,
) -> RespuestaAsistente {
    let activas = obras.activas();
    if activas.is_empty() {
        return RespuestaAsistente::solo_texto(
            CategoriaIntencion::AlertasObras,
            conf,
            "✅ No hay obras activas; no hay alertas.",
        );
    }
    let mut alertas: Vec<String> = Vec::new();
    for obra in &activas {
        let pasos = obra.validar_flujo_completo();
        let faltantes: Vec<_> = pasos
            .iter()
            .filter(|p| matches!(p.estado, crate::obras::EstadoPaso::Faltante))
            .collect();
        if !faltantes.is_empty() {
            for paso in faltantes.iter().take(2) {
                alertas.push(format!(
                    "  🔴 [{}] Paso {} «{}» — {}",
                    obra.nombre, paso.numero, paso.nombre, paso.riesgo
                ));
            }
        }
        // Verificar redundancias
        let redundancias = obra.verificar_redundancias();
        for r in redundancias.iter().take(2) {
            alertas.push(format!("  ⚠️  [{}] {}", obra.nombre, r));
        }
    }
    if alertas.is_empty() {
        let mut r = RespuestaAsistente::solo_texto(
            CategoriaIntencion::AlertasObras,
            conf,
            "✅ Todo en orden. No hay pasos vencidos ni alertas críticas.",
        );
        r.seguimientos = vec!["¿Cómo va el avance de las obras?".into()];
        return r;
    }
    let mut texto = format!("🚨 {} alerta(s) encontrada(s):\n\n", alertas.len());
    for a in alertas.iter().take(10) {
        texto.push_str(a);
        texto.push('\n');
    }
    let mut r = RespuestaAsistente::solo_texto(CategoriaIntencion::AlertasObras, conf, texto);
    r.seguimientos = vec![
        "¿Cuál es el siguiente paso en la obra?".into(),
        "¿Ver el mapa de Marco Lógico?".into(),
    ];
    r
}

/// Resumen de cuentas por cobrar.
pub fn responder_consultar_cobranzas(
    conf: f64,
    cobranzas: &crate::cobranzas::AlmacenCobranzas,
) -> RespuestaAsistente {
    use crate::cobranzas::EstadoCuenta;
    let cuentas = &cobranzas.cuentas;
    if cuentas.is_empty() {
        return RespuestaAsistente::solo_texto(
            CategoriaIntencion::ConsultarCobranzas,
            conf,
            "📋 No tienes cuentas por cobrar registradas.",
        );
    }
    let pendientes: Vec<_> = cuentas
        .iter()
        .filter(|c| !matches!(c.estado, EstadoCuenta::Pagada | EstadoCuenta::Incobrable))
        .collect();
    let total_pendiente: f64 = pendientes
        .iter()
        .map(|c| c.monto_total - c.monto_cobrado)
        .sum();
    let total_cuentas = cuentas.len();
    let mut texto = format!(
        "💵 Cobranzas:\n   Total de cuentas: {}\n   Pendientes de cobro: {}\n   Monto total por cobrar: ${:.2}\n\n",
        total_cuentas, pendientes.len(), total_pendiente
    );
    for c in pendientes.iter().take(8) {
        let saldo = c.monto_total - c.monto_cobrado;
        texto.push_str(&format!(
            "  • {} — ${:.2} pendiente (total ${:.2})\n",
            c.cliente, saldo, c.monto_total
        ));
    }
    if pendientes.len() > 8 {
        texto.push_str(&format!("  … y {} cuenta(s) más.\n", pendientes.len() - 8));
    }
    let mut r = RespuestaAsistente::solo_texto(CategoriaIntencion::ConsultarCobranzas, conf, texto);
    r.seguimientos = vec![
        "¿Cómo va el estado general de la empresa?".into(),
        "¿Qué deudas tengo yo mismo?".into(),
    ];
    r
}

/// Resumen empresarial: propuestas + obras + cobranzas.
pub fn responder_resumen_empresa(
    conf: f64,
    obras: &crate::obras::AlmacenObras,
    cobranzas: &crate::cobranzas::AlmacenCobranzas,
    propuestas: &crate::propuestas::AlmacenPropuestas,
    casos: &crate::casos::AlmacenCasos,
) -> RespuestaAsistente {
    use crate::casos::EstadoCaso;
    use crate::cobranzas::EstadoCuenta;
    use crate::propuestas::EstadoPropuesta;

    let obras_activas = obras.activas().len();
    let cobranzas_pendientes: f64 = cobranzas
        .cuentas
        .iter()
        .filter(|c| !matches!(c.estado, EstadoCuenta::Pagada | EstadoCuenta::Incobrable))
        .map(|c| c.monto_total - c.monto_cobrado)
        .sum();
    let propuestas_activas = propuestas
        .propuestas
        .iter()
        .filter(|p| {
            !matches!(
                p.estado,
                EstadoPropuesta::Ganada | EstadoPropuesta::Perdida | EstadoPropuesta::Cancelada
            )
        })
        .count();
    let casos_abiertos = casos
        .casos
        .iter()
        .filter(|c| !matches!(c.estado, EstadoCaso::Cerrado | EstadoCaso::Cancelado))
        .count();

    let texto = format!(
        "🏢 Resumen Empresarial:\n\n\
         🏗️  Obras activas:        {}\n\
         📄  Propuestas vigentes:  {}\n\
         📁  Casos abiertos:       {}\n\
         💵  Por cobrar (total):   ${:.2}\n\n\
         Usa el módulo Empresa para gestionar cada área en detalle.",
        obras_activas, propuestas_activas, casos_abiertos, cobranzas_pendientes
    );
    let mut r = RespuestaAsistente::solo_texto(CategoriaIntencion::ResumenEmpresa, conf, texto);
    r.seguimientos = vec![
        "¿Cómo van mis obras?".into(),
        "¿Cuánto me deben mis clientes?".into(),
        "¿Mis propuestas activas?".into(),
    ];
    r
}

/// Guía del siguiente paso en obras (resumen rápido).
pub fn responder_guia_siguiente_paso(
    conf: f64,
    obras: &crate::obras::AlmacenObras,
) -> RespuestaAsistente {
    let activas = obras.activas();
    if activas.is_empty() {
        return RespuestaAsistente::solo_texto(
            CategoriaIntencion::GuiaSiguientePaso,
            conf,
            "📋 No hay obras activas. Créa una desde Empresa → Obras.",
        );
    }
    let mut texto = String::from("🗺️  Siguiente paso por obra:\n\n");
    for obra in activas.iter().take(5) {
        // Buscar el primer paso pendiente o en progreso
        let pasos = obra.validar_flujo_completo();
        let siguiente = pasos.iter().find(|p| {
            matches!(
                p.estado,
                crate::obras::EstadoPaso::Faltante | crate::obras::EstadoPaso::Pendiente
            )
        });
        match siguiente {
            Some(p) => {
                texto.push_str(&format!(
                    "  🔨 {} → Paso {} «{}»\n     {}\n",
                    obra.nombre, p.numero, p.nombre, p.detalle
                ));
            }
            None => {
                texto.push_str(&format!(
                    "  ✅ {} — todos los pasos completados.\n",
                    obra.nombre
                ));
            }
        }
    }
    let mut r = RespuestaAsistente::solo_texto(CategoriaIntencion::GuiaSiguientePaso, conf, texto);
    r.seguimientos = vec![
        "¿Hay alertas en mis obras?".into(),
        "¿Ver el mapa Marco Lógico completo?".into(),
    ];
    r
}
