//! Asistente Virtual Financiero — Fase 5.
//!
//! Dispatcher inteligente que toma una consulta en lenguaje natural,
//! la clasifica con el motor NLP y la enruta a la acción correspondiente
//! en los módulos de gastos, deudas, agenda, sugerencias y seguridad.
//!
//! Diseñado para que el usuario pueda escribir cosas como:
//!  - "gasté 50 en comida hoy"
//!  - "cuánto llevo gastado este mes"
//!  - "qué deuda debo pagar primero"
//!  - "agéndame el pago de la luz el 15"
//!  - "cómo voy financieramente"
//!
//! Cada intent retorna una `RespuestaAsistente` que incluye texto formateado
//! y opcionalmente acciones a ejecutar (mutaciones sobre `AppState`).

use chrono::{Datelike, Local, NaiveDate};

use super::intent::{CategoriaIntencion, Intencion};
use crate::ml::gastos::{AlmacenGastos, GastoReal};
use crate::ml::presupuesto_cero::Categoria;
use crate::ml::sugerencias::{PlanPagosMes, TipoSugerencia};
use crate::ml::AlmacenAsesor;

// ─── Resultado del asistente ─────────────────────────────────────────────────

/// Respuesta estructurada del asistente para una consulta del usuario.
#[derive(Debug, Clone)]
pub struct RespuestaAsistente {
    /// Categoría de intención detectada
    pub intent: CategoriaIntencion,
    /// Confianza del clasificador (0.0 - 1.0)
    pub confianza: f64,
    /// Texto principal a mostrar al usuario
    pub texto: String,
    /// Si la consulta requiere acción mutante, descripción de qué se hizo
    pub accion_realizada: Option<String>,
    /// Sugerencias de seguimiento (próximas preguntas útiles)
    pub seguimientos: Vec<String>,
    /// Indica si la acción modificó el estado y debe guardarse
    pub modifico_estado: bool,
}

impl RespuestaAsistente {
    fn solo_texto(intent: CategoriaIntencion, confianza: f64, texto: impl Into<String>) -> Self {
        Self {
            intent,
            confianza,
            texto: texto.into(),
            accion_realizada: None,
            seguimientos: Vec::new(),
            modifico_estado: false,
        }
    }

    fn con_accion(
        intent: CategoriaIntencion,
        confianza: f64,
        texto: impl Into<String>,
        accion: impl Into<String>,
    ) -> Self {
        Self {
            intent,
            confianza,
            texto: texto.into(),
            accion_realizada: Some(accion.into()),
            seguimientos: Vec::new(),
            modifico_estado: true,
        }
    }
}

// ─── Despachador principal ───────────────────────────────────────────────────

/// Procesa una consulta financiera en lenguaje natural.
///
/// Recibe acceso mutable a los almacenes relevantes y retorna una respuesta
/// estructurada. La capa CLI se encarga de mostrar el texto y de persistir
/// si `modifico_estado == true`.
pub fn responder(
    consulta: &str,
    intencion: &Intencion,
    rastreador: &AlmacenAsesor,
    gastos: &mut AlmacenGastos,
) -> RespuestaAsistente {
    let conf = intencion.confianza;

    // Si el clasificador NLP genérico ganó, intentar rescatar intent financiero
    // por heurística de palabras clave de dominio.
    let intent_efectivo = match &intencion.categoria {
        CategoriaIntencion::Consultar
        | CategoriaIntencion::Listar
        | CategoriaIntencion::Buscar
        | CategoriaIntencion::Modificar
        | CategoriaIntencion::Crear
        | CategoriaIntencion::Desconocido => {
            intent_financiero(consulta).unwrap_or(intencion.categoria.clone())
        }
        otro => otro.clone(),
    };

    match intent_efectivo {
        CategoriaIntencion::RegistrarGasto => {
            responder_registrar_gasto(consulta, conf, gastos, false)
        }
        CategoriaIntencion::RegistrarIngreso => {
            responder_registrar_gasto(consulta, conf, gastos, true)
        }
        CategoriaIntencion::ConsultarGastos => responder_consultar_gastos(conf, gastos),
        CategoriaIntencion::PedirSugerenciaPago => {
            responder_sugerencia_pago(conf, &rastreador.rastreador, gastos)
        }
        CategoriaIntencion::ResumenFinanciero => {
            responder_resumen_financiero(conf, &rastreador.rastreador, gastos)
        }
        CategoriaIntencion::AgendarPago => responder_agendar_pago(consulta, conf),
        CategoriaIntencion::Saludo => RespuestaAsistente::solo_texto(
            CategoriaIntencion::Saludo,
            conf,
            "¡Hola! Soy tu asistente financiero. Puedes preguntarme:\n\
             • \"cuánto llevo gastado este mes\"\n\
             • \"qué deuda debo pagar primero\"\n\
             • \"gasté 50 en comida hoy\"\n\
             • \"cómo voy financieramente\"",
        ),
        CategoriaIntencion::Ayuda => RespuestaAsistente::solo_texto(
            CategoriaIntencion::Ayuda,
            conf,
            "Comandos del asistente financiero:\n\
             • Registro: \"gasté 25 en gasolina\", \"recibí 1500 de sueldo\"\n\
             • Consulta: \"mis gastos del mes\", \"resumen financiero\"\n\
             • Estrategia: \"qué pago primero\", \"plan de pagos\"\n\
             • Agenda: \"recordarme pagar la luz el 15\"\n\
             También entiendo expresiones de fechas: hoy, ayer, el 15, mañana.",
        ),
        _ => {
            // Construir una Intencion temporal con el intent efectivo para el mensaje
            let intent_info = Intencion {
                categoria: intent_efectivo.clone(),
                confianza: conf,
                entidades: intencion.entidades.clone(),
                alternativas: intencion.alternativas.clone(),
            };
            responder_no_entendido(consulta, &intent_info)
        }
    }
}

// ─── Handlers individuales ───────────────────────────────────────────────────

fn responder_registrar_gasto(
    consulta: &str,
    conf: f64,
    gastos: &mut AlmacenGastos,
    es_ingreso: bool,
) -> RespuestaAsistente {
    let monto = match extraer_monto(consulta) {
        Some(m) => m,
        None => {
            return RespuestaAsistente::solo_texto(
                if es_ingreso {
                    CategoriaIntencion::RegistrarIngreso
                } else {
                    CategoriaIntencion::RegistrarGasto
                },
                conf,
                "No detecté el monto. Intenta así: \"gasté 50 en comida\" o \"recibí 1500 de sueldo\".",
            );
        }
    };

    let fecha = extraer_fecha(consulta).unwrap_or_else(|| Local::now().date_naive());
    let categoria = inferir_categoria(consulta, es_ingreso);
    let descripcion = extraer_descripcion(consulta, es_ingreso);

    let monto_signed = if es_ingreso { -monto } else { monto };
    let g = GastoReal::nuevo(fecha, descripcion.clone(), categoria.clone(), monto_signed);
    let id = g.id.clone();
    gastos.agregar(g);

    let etiqueta = if es_ingreso { "Ingreso" } else { "Gasto" };
    let icono = if es_ingreso { "💰" } else { "💸" };
    let texto = format!(
        "{} {} registrado: ${:.2} en \"{}\" ({}) — {}\n  ID: {}",
        icono,
        etiqueta,
        monto,
        descripcion,
        nombre_categoria(&categoria),
        fecha.format("%d/%m/%Y"),
        id,
    );
    let mut r = RespuestaAsistente::con_accion(
        if es_ingreso {
            CategoriaIntencion::RegistrarIngreso
        } else {
            CategoriaIntencion::RegistrarGasto
        },
        conf,
        texto,
        format!("{} ${:.2}", etiqueta, monto),
    );
    r.seguimientos
        .push("¿Cuánto llevo gastado este mes?".to_string());
    r.seguimientos.push("Resumen financiero".to_string());
    r
}

fn responder_consultar_gastos(conf: f64, gastos: &AlmacenGastos) -> RespuestaAsistente {
    let hoy = Local::now().date_naive();
    let resumen = gastos.resumen_mes(hoy.year(), hoy.month());
    let por_cat = gastos.por_categoria(
        NaiveDate::from_ymd_opt(hoy.year(), hoy.month(), 1).unwrap(),
        hoy,
    );

    let mut texto = format!(
        "📊 Gastos de {}/{}\n\n  💸 Gastos:    ${:.2}\n  💰 Ingresos:  ${:.2}\n  ⚖  Balance:  ${:.2}\n  📝 Transacciones: {}",
        hoy.month(),
        hoy.year(),
        resumen.total_gastos,
        resumen.total_ingresos,
        resumen.balance,
        resumen.num_transacciones,
    );

    if !por_cat.is_empty() {
        texto.push_str("\n\n  Por categoría:\n");
        let mut ordenadas = por_cat.clone();
        ordenadas.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        for (cat, total) in ordenadas.iter().take(5) {
            texto.push_str(&format!(
                "    • {:<20} ${:.2}\n",
                nombre_categoria(cat),
                total
            ));
        }
    }

    let mut r = RespuestaAsistente::solo_texto(CategoriaIntencion::ConsultarGastos, conf, texto);
    r.seguimientos
        .push("¿Qué deuda debo pagar primero?".to_string());
    r.seguimientos
        .push("Resumen financiero completo".to_string());
    r
}

fn responder_sugerencia_pago(
    conf: f64,
    rastreador: &crate::ml::advisor::RastreadorDeudas,
    gastos: &AlmacenGastos,
) -> RespuestaAsistente {
    let hoy = Local::now().date_naive();
    let plan = PlanPagosMes::generar(rastreador, gastos, hoy.year(), hoy.month());

    if plan.sugerencias.is_empty() {
        return RespuestaAsistente::solo_texto(
            CategoriaIntencion::PedirSugerenciaPago,
            conf,
            "No hay deudas activas registradas. Si tienes deudas, regístralas primero en el menú \"Asesor Financiero\".",
        );
    }

    let mut texto = format!(
        "💡 Plan de pagos sugerido para {}/{}\n\n  Ingreso:        ${:.2}\n  Gastos reales:  ${:.2}\n  Pagos mínimos:  ${:.2}\n  Excedente:      ${:.2}\n\n  Sugerencias priorizadas:\n",
        hoy.month(),
        hoy.year(),
        plan.ingreso_mensual,
        plan.gastos_reales_mes,
        plan.pagos_minimos_total,
        plan.excedente,
    );

    for (i, s) in plan.sugerencias.iter().take(5).enumerate() {
        let marcador = match s.tipo {
            TipoSugerencia::Urgente => "⚠",
            TipoSugerencia::AbonoExtra => "🔥",
            TipoSugerencia::CasiLiquidada => "🎯",
            TipoSugerencia::BolaNieve => "❄",
            TipoSugerencia::SoloMinimo => "·",
        };
        texto.push_str(&format!(
            "  {}. {} {} → pagar ${:.2} (mín ${:.2}, APR {:.1}%)\n     {}\n",
            i + 1,
            marcador,
            s.nombre_deuda,
            s.monto_sugerido,
            s.pago_minimo,
            s.tasa_anual,
            s.razon,
        ));
    }

    if !plan.advertencias.is_empty() {
        texto.push_str("\n  ⚠ Advertencias:\n");
        for a in &plan.advertencias {
            texto.push_str(&format!("    • {}\n", a));
        }
    }

    let mut r =
        RespuestaAsistente::solo_texto(CategoriaIntencion::PedirSugerenciaPago, conf, texto);
    r.seguimientos
        .push("Resumen financiero general".to_string());
    r.seguimientos.push("Agendar el pago sugerido".to_string());
    r
}

fn responder_resumen_financiero(
    conf: f64,
    rastreador: &crate::ml::advisor::RastreadorDeudas,
    gastos: &AlmacenGastos,
) -> RespuestaAsistente {
    let hoy = Local::now().date_naive();
    let resumen = gastos.resumen_mes(hoy.year(), hoy.month());
    let ingreso = rastreador.ingreso_mensual_confirmado();
    let pagos_min = rastreador.pagos_minimos_mensuales();
    let deuda_total: f64 = rastreador.deudas.iter().map(|d| d.saldo_actual()).sum();
    let num_deudas = rastreador.deudas.len();

    let flujo_libre = ingreso - resumen.total_gastos - pagos_min;
    let salud = if ingreso <= 0.0 {
        ("Sin datos de ingreso", "⚪")
    } else if flujo_libre < 0.0 {
        ("Déficit — gastas más de lo que ingresas", "🔴")
    } else if flujo_libre / ingreso < 0.1 {
        ("Justa — margen muy estrecho", "🟡")
    } else if flujo_libre / ingreso < 0.3 {
        ("Saludable", "🟢")
    } else {
        ("Excelente — gran capacidad de ahorro", "✨")
    };

    let texto = format!(
        "🧭 Resumen Financiero — {}/{}\n\n  {} Salud financiera: {}\n\n  💰 Ingreso mensual:    ${:.2}\n  💸 Gastos del mes:     ${:.2}\n  🧾 Pagos mínimos:      ${:.2}\n  🪙 Flujo disponible:   ${:.2}\n\n  📉 Deudas activas:     {} ({} deuda{})\n  ⚖  Balance del mes:    ${:.2}\n  📝 Transacciones:      {}",
        hoy.month(),
        hoy.year(),
        salud.1,
        salud.0,
        ingreso,
        resumen.total_gastos,
        pagos_min,
        flujo_libre,
        format_args!("${:.2}", deuda_total),
        num_deudas,
        if num_deudas == 1 { "" } else { "s" },
        resumen.balance,
        resumen.num_transacciones,
    );

    let mut r = RespuestaAsistente::solo_texto(CategoriaIntencion::ResumenFinanciero, conf, texto);
    if num_deudas > 0 {
        r.seguimientos
            .push("¿Qué deuda debo pagar primero?".to_string());
    }
    r.seguimientos
        .push("Detalle de gastos por categoría".to_string());
    r
}

fn responder_agendar_pago(consulta: &str, conf: f64) -> RespuestaAsistente {
    let monto = extraer_monto(consulta);
    let fecha = extraer_fecha(consulta);
    let mut texto = String::from("📅 Para agendar un pago necesito:\n");
    if monto.is_none() {
        texto.push_str("  • Monto (ej: \"pagar 200\")\n");
    }
    if fecha.is_none() {
        texto.push_str("  • Fecha (ej: \"el 15\", \"mañana\")\n");
    }
    texto.push_str("\nUsa el menú \"Agenda → Nuevo evento\" con tipo \"Pago\" para registrar todos los detalles.");
    if let (Some(m), Some(f)) = (monto, fecha) {
        texto = format!(
            "📅 Detecté pago de ${:.2} para {}.\nAbre el menú Agenda y selecciona \"Nuevo evento\" tipo Pago para confirmar los detalles (descripción, hora, recordatorio).",
            m,
            f.format("%d/%m/%Y"),
        );
    }
    RespuestaAsistente::solo_texto(CategoriaIntencion::AgendarPago, conf, texto)
}

fn responder_no_entendido(consulta: &str, intencion: &Intencion) -> RespuestaAsistente {
    let texto = format!(
        "🤔 No estoy seguro de qué quieres hacer (intent: {}, confianza: {:.0}%).\n\nIntenta:\n  • \"gasté 50 en comida\"\n  • \"cuánto llevo gastado\"\n  • \"qué deuda pago primero\"\n  • \"resumen financiero\"\n\nTu consulta fue: \"{}\"",
        intencion.categoria.nombre(),
        intencion.confianza * 100.0,
        consulta,
    );
    RespuestaAsistente::solo_texto(intencion.categoria.clone(), intencion.confianza, texto)
}

// ─── Extracción de entidades ─────────────────────────────────────────────────

/// Cuando el clasificador NLP genérico gana (Consultar / Listar / Buscar /
/// Desconocido), este detector de dominio comprueba si el texto contiene
/// léxico financiero y devuelve el intent correcto.
fn intent_financiero(consulta: &str) -> Option<CategoriaIntencion> {
    let lower = consulta.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    let has_word = |w: &str| words.contains(&w);
    let has = |s: &str| lower.as_str().contains(s);

    // ¿Es una pregunta? (interrogativo al inicio)
    let es_pregunta = has_word("cuanto")
        || has("cuánto")
        || has_word("que")
        || has("qué")
        || has_word("como")
        || has("cómo")
        || has_word("cual")
        || has("cuál");

    // ── 1. Consultas interrogativas sobre gastos (ANTES de RegistrarGasto) ──
    // "cuánto gasté" es consulta, no registro
    let cuanto_gaste = (has_word("cuanto") || has("cuánto"))
        && (has_word("gaste") || has("gasté") || has_word("gastado") || has_word("gasto"));
    if cuanto_gaste
        || has("cuánto gasté")
        || has("cuánto he gastado")
        || has("en qué gasté")
        || has("cuánto llevo")
        || has("gastos por categoría")
        || has("gastos del mes")
        || has("mis gastos")
        || has("cuanto llevo")
        || has("en que gaste")
        || has("balance del mes")
        || has("balance mes")
        || has_word("transacciones")
        || has_word("movimientos")
    {
        return Some(CategoriaIntencion::ConsultarGastos);
    }

    // ── 2. Resumen financiero ────────────────────────────────────────
    let queda_financiero = has_word("queda")
        && (has_word("dinero")
            || has_word("mes")
            || has_word("disponible")
            || has_word("pagar")
            || has_word("pago"));
    let cuanto_queda = (has_word("cuanto") || has("cuánto"))
        && (has_word("queda") || has_word("tengo") || has_word("debo") || has_word("disponible"))
        && (has_word("dinero") || has_word("mes") || has_word("pagar"));
    if has("como voy")
        || has("cómo voy")
        || has("situacion financiera")
        || has("situación financiera")
        || has("estado financiero")
        || has("mis finanzas")
        || has("resumen financiero")
        || has("panorama financiero")
        || has("mis deudas")
        || has("total deudas")
        || has("dinero disponible")
        || has("saldo disponible")
        || has("cuánto me queda")
        || has("cuánto debo")
        || has("cuánto queda")
        || queda_financiero
        || cuanto_queda
        || (has_word("deudas") && !has_word("primero"))
        || (has_word("finanzas") && !has("gasté"))
    {
        return Some(CategoriaIntencion::ResumenFinanciero);
    }

    // ── 3. Registro de gasto (declarativo, no pregunta) ──────────────
    let gasto_verbos = ["gaste", "pague", "compre", "desembolse"];
    if !es_pregunta
        && (gasto_verbos.iter().any(|v| has_word(v))
            || has("gasté")
            || has("pagué")
            || has("compré")
            || has("desembolsé")
            || has("me costó")
            || has("me costo")
            || has("cobré un cargo"))
    {
        return Some(CategoriaIntencion::RegistrarGasto);
    }

    // ── 4. Registro de ingreso (declarativo) ─────────────────────────
    if !es_pregunta
        && (has_word("recibi")
            || has_word("depositaron")
            || has_word("sueldo")
            || has_word("nomina")
            || has_word("salario")
            || has("recibí")
            || has("cobré")
            || has("entró")
            || has("nómina")
            || has("comisión")
            || has("me pagaron")
            || has("me depositaron")
            || has("entro dinero"))
    {
        return Some(CategoriaIntencion::RegistrarIngreso);
    }

    // ── 5. Estrategia de pago ─────────────────────────────────────────
    if has("bola de nieve")
        || has("avalancha")
        || has("plan de pagos")
        || has("plan pagos")
        || has("orden de pago")
        || has("estrategia de pago")
        || (has_word("primero") && (has_word("deuda") || has_word("pago")))
        || (has_word("antes") && has_word("deuda"))
    {
        return Some(CategoriaIntencion::PedirSugerenciaPago);
    }

    // ── 6. Agendar / recordar pago ────────────────────────────────────
    if has("agendar pago")
        || has("recordar pago")
        || has("programar pago")
        || has("recordarme pagar")
        || has("pagar el dia")
        || has("pagar el día")
    {
        return Some(CategoriaIntencion::AgendarPago);
    }

    None
}
fn extraer_monto(s: &str) -> Option<f64> {
    let mut buf = String::new();
    let mut visto_punto = false;
    let mut resultado: Option<f64> = None;
    for c in s.chars() {
        if c.is_ascii_digit() {
            buf.push(c);
        } else if c == '.' || c == ',' {
            if !buf.is_empty() && !visto_punto {
                buf.push('.');
                visto_punto = true;
            }
        } else if !buf.is_empty() {
            if let Ok(v) = buf.parse::<f64>() {
                resultado = Some(v);
                break;
            }
            buf.clear();
            visto_punto = false;
        }
    }
    if resultado.is_none() && !buf.is_empty() {
        resultado = buf.parse::<f64>().ok();
    }
    resultado.filter(|v| *v > 0.0)
}

/// Extrae fecha aproximada del texto.
/// Soporta: "hoy", "ayer", "mañana", "el 15", "el 5/6", "el 5/6/2026".
fn extraer_fecha(s: &str) -> Option<NaiveDate> {
    let lower = s.to_lowercase();
    let hoy = Local::now().date_naive();

    if lower.contains("ayer") {
        return hoy.pred_opt();
    }
    if lower.contains("mañana") || lower.contains("manana") {
        return hoy.succ_opt();
    }
    if lower.contains("hoy") {
        return Some(hoy);
    }

    // Buscar patrón "el N" o "N/M" o "N/M/AAAA"
    let tokens: Vec<&str> = lower.split_whitespace().collect();
    for (i, tok) in tokens.iter().enumerate() {
        if *tok == "el" {
            if let Some(siguiente) = tokens.get(i + 1) {
                if let Some(f) = parsear_fecha_token(siguiente, hoy) {
                    return Some(f);
                }
            }
        }
        if let Some(f) = parsear_fecha_token(tok, hoy) {
            return Some(f);
        }
    }
    None
}

fn parsear_fecha_token(tok: &str, hoy: NaiveDate) -> Option<NaiveDate> {
    // Formato N/M/AAAA o N/M
    let partes: Vec<&str> = tok.split('/').collect();
    match partes.len() {
        3 => {
            let d = partes[0].parse::<u32>().ok()?;
            let m = partes[1].parse::<u32>().ok()?;
            let a = partes[2].parse::<i32>().ok()?;
            let a = if a < 100 { 2000 + a } else { a };
            NaiveDate::from_ymd_opt(a, m, d)
        }
        2 => {
            let d = partes[0].parse::<u32>().ok()?;
            let m = partes[1].parse::<u32>().ok()?;
            NaiveDate::from_ymd_opt(hoy.year(), m, d)
        }
        1 => {
            // Solo día del mes actual
            let d = tok.parse::<u32>().ok()?;
            if (1..=31).contains(&d) {
                NaiveDate::from_ymd_opt(hoy.year(), hoy.month(), d)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Inferir categoría por palabras clave en la consulta.
fn inferir_categoria(s: &str, es_ingreso: bool) -> Categoria {
    if es_ingreso {
        return Categoria::Ingreso;
    }
    let lower = s.to_lowercase();
    // Heurística simple por dominios léxicos
    let fijos = [
        "renta",
        "alquiler",
        "luz",
        "agua",
        "internet",
        "gas",
        "telefono",
        "teléfono",
        "seguro",
        "servicio",
    ];
    let deuda = [
        "tarjeta",
        "deuda",
        "credito",
        "crédito",
        "prestamo",
        "préstamo",
        "cuota",
    ];
    let ahorro = ["ahorro", "inversion", "inversión", "fondo"];

    if fijos.iter().any(|p| lower.contains(p)) {
        Categoria::GastoFijo
    } else if deuda.iter().any(|p| lower.contains(p)) {
        Categoria::PagoDeuda
    } else if ahorro.iter().any(|p| lower.contains(p)) {
        Categoria::Ahorro
    } else {
        Categoria::GastoVariable
    }
}

fn nombre_categoria(c: &Categoria) -> &'static str {
    match c {
        Categoria::GastoFijo => "Gasto Fijo",
        Categoria::GastoVariable => "Gasto Variable",
        Categoria::PagoDeuda => "Pago de Deuda",
        Categoria::Ahorro => "Ahorro",
        Categoria::Ingreso => "Ingreso",
    }
}

/// Extrae descripción aproximada — toma palabras después de "en" o el resto del texto.
fn extraer_descripcion(s: &str, es_ingreso: bool) -> String {
    let lower = s.to_lowercase();
    // Buscar " en " o " de " (preferir "en" para gastos, "de" para ingresos)
    let conector = if es_ingreso { " de " } else { " en " };
    if let Some(pos) = lower.find(conector) {
        let resto = &s[pos + conector.len()..];
        let resto = resto.trim();
        if !resto.is_empty() && resto.len() < 100 {
            return resto.to_string();
        }
    }
    // Fallback: usar consulta sin palabras de acción
    let palabras_filtro = [
        "gaste", "gasté", "pague", "pagué", "compre", "compré", "recibí", "recibi", "cobre",
        "cobré",
    ];
    let limpio: Vec<&str> = s
        .split_whitespace()
        .filter(|w| {
            let l = w.to_lowercase();
            !palabras_filtro.contains(&l.as_str()) && l.parse::<f64>().is_err()
        })
        .collect();
    let resultado = limpio.join(" ").trim().to_string();
    if resultado.is_empty() {
        if es_ingreso {
            "Ingreso".to_string()
        } else {
            "Gasto".to_string()
        }
    } else {
        resultado
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extraer_monto_decimal() {
        assert_eq!(extraer_monto("gasté 50.50 en comida"), Some(50.5));
        assert_eq!(extraer_monto("pagué 1500 de renta"), Some(1500.0));
        assert_eq!(extraer_monto("compré algo por 25,75"), Some(25.75));
        assert_eq!(extraer_monto("hola"), None);
    }

    #[test]
    fn test_extraer_fecha_relativa() {
        let hoy = Local::now().date_naive();
        assert_eq!(extraer_fecha("gasté hoy"), Some(hoy));
        assert_eq!(extraer_fecha("ayer compré"), hoy.pred_opt());
        assert_eq!(extraer_fecha("mañana pago"), hoy.succ_opt());
    }

    #[test]
    fn test_extraer_fecha_dia() {
        let hoy = Local::now().date_naive();
        let f = extraer_fecha("pagar el 15");
        assert!(f.is_some());
        assert_eq!(f.unwrap().day(), 15);
        assert_eq!(f.unwrap().month(), hoy.month());
    }

    #[test]
    fn test_inferir_categoria() {
        assert_eq!(
            inferir_categoria("gasté 50 en luz", false),
            Categoria::GastoFijo
        );
        assert_eq!(
            inferir_categoria("pago tarjeta 200", false),
            Categoria::PagoDeuda
        );
        assert_eq!(
            inferir_categoria("compré pizza", false),
            Categoria::GastoVariable
        );
        assert_eq!(inferir_categoria("recibí sueldo", true), Categoria::Ingreso);
    }

    #[test]
    fn test_extraer_descripcion() {
        assert_eq!(
            extraer_descripcion("gasté 50 en gasolina", false),
            "gasolina"
        );
        assert_eq!(extraer_descripcion("recibí 1500 de sueldo", true), "sueldo");
    }

    #[test]
    fn test_heuristica_financiera_override() {
        assert_eq!(
            intent_financiero("cuanto dinero queda este mes de Junio por pagar"),
            Some(CategoriaIntencion::ResumenFinanciero)
        );
        assert_eq!(
            intent_financiero("cuánto dinero me queda disponible"),
            Some(CategoriaIntencion::ResumenFinanciero)
        );
        assert_eq!(
            intent_financiero("cómo voy financieramente"),
            Some(CategoriaIntencion::ResumenFinanciero)
        );
        assert_eq!(
            intent_financiero("mis gastos del mes"),
            Some(CategoriaIntencion::ConsultarGastos)
        );
        assert_eq!(
            intent_financiero("cuánto gasté esta semana"),
            Some(CategoriaIntencion::ConsultarGastos)
        );
        assert_eq!(
            intent_financiero("qué deuda pago primero"),
            Some(CategoriaIntencion::PedirSugerenciaPago)
        );
        assert_eq!(
            intent_financiero("recibí 2000 de sueldo"),
            Some(CategoriaIntencion::RegistrarIngreso)
        );
        assert_eq!(
            intent_financiero("gasté 80 en supermercado"),
            Some(CategoriaIntencion::RegistrarGasto)
        );
        assert_eq!(intent_financiero("hola cómo estás"), None);
    }
}
