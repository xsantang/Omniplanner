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
use super::router;
use crate::agenda::Agenda;
use crate::ml::gastos::{AlmacenGastos, GastoReal};
use crate::dinero::Dinero;
use crate::ml::presupuesto_cero::Categoria;
use crate::ml::sugerencias::{PlanPagosMes, TipoSugerencia};
use crate::storage::AppState;

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
    pub(super) fn solo_texto(
        intent: CategoriaIntencion,
        confianza: f64,
        texto: impl Into<String>,
    ) -> Self {
        Self {
            intent,
            confianza,
            texto: texto.into(),
            accion_realizada: None,
            seguimientos: Vec::new(),
            modifico_estado: false,
        }
    }

    pub(super) fn con_accion(
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

/// Procesa una consulta del usuario en lenguaje natural y la enruta al
/// módulo correcto (finanzas, tareas, agenda, calendario, memoria,
/// contraseñas, deudas).
///
/// Recibe `&mut AppState` completo y retorna una respuesta estructurada.
/// La capa CLI muestra el texto y persiste si `modifico_estado == true`.
pub fn responder(
    consulta: &str,
    intencion: &Intencion,
    state: &mut AppState,
) -> RespuestaAsistente {
    let conf = intencion.confianza;

    // 1) Si el clasificador genérico ganó, intentar rescatar intent de
    //    dominio: primero multi-módulo (router), luego financiero (legacy).
    //    También se re-chequean CrearEvento y CrearTarea porque frases como
    //    "cuánto falta para el cumpleaños de X" se malclasifican con alta
    //    confianza como CrearEvento cuando en realidad son ConsultarAgenda.
    //    Además, intents sociales (Saludo, Sentimiento…) con baja confianza
    //    también pasan por el router, porque la consulta puede ser una pregunta
    //    real mal clasificada (ej: "cuantos dias faltan para navidad" → Saludo).
    let intent_efectivo = match &intencion.categoria {
        CategoriaIntencion::Consultar
        | CategoriaIntencion::Listar
        | CategoriaIntencion::Buscar
        | CategoriaIntencion::Modificar
        | CategoriaIntencion::Crear
        | CategoriaIntencion::CrearEvento
        | CategoriaIntencion::Desconocido => router::detectar_intent_modulo(consulta)
            .or_else(|| intent_financiero(consulta))
            .unwrap_or(intencion.categoria.clone()),
        // Con confianza baja (<0.5), cualquier intent social/genérico también
        // intenta el router — así no se pierden preguntas reales.
        otro if conf < 0.5 => router::detectar_intent_modulo(consulta)
            .or_else(|| intent_financiero(consulta))
            .unwrap_or(otro.clone()),
        otro => otro.clone(),
    };

    match intent_efectivo {
        // ── Financiero ──
        CategoriaIntencion::RegistrarGasto => {
            responder_registrar_gasto(consulta, conf, &mut state.gastos, false)
        }
        CategoriaIntencion::RegistrarIngreso => {
            responder_registrar_gasto(consulta, conf, &mut state.gastos, true)
        }
        CategoriaIntencion::ConsultarGastos => {
            responder_consultar_gastos(consulta, conf, &state.gastos, &state.asesor.rastreador)
        }
        CategoriaIntencion::PedirSugerenciaPago => {
            responder_sugerencia_pago(conf, &state.asesor.rastreador, &state.gastos)
        }
        CategoriaIntencion::ResumenFinanciero => {
            responder_resumen_financiero(conf, &state.asesor.rastreador, &state.gastos)
        }
        CategoriaIntencion::AgendarPago => responder_agendar_pago(consulta, conf, &state.agenda),
        CategoriaIntencion::ConsultarAgenda => {
            responder_consultar_agenda(consulta, conf, &state.agenda)
        }

        // ── Multi-módulo ──
        CategoriaIntencion::CrearTarea => {
            router::responder_crear_tarea(consulta, conf, &mut state.tasks)
        }
        CategoriaIntencion::ConsultarTareas => {
            router::responder_consultar_tareas(consulta, conf, &state.tasks)
        }
        CategoriaIntencion::CompletarTarea => {
            router::responder_completar_tarea(consulta, conf, &mut state.tasks)
        }
        CategoriaIntencion::CrearEvento => {
            router::responder_crear_evento(consulta, conf, &mut state.agenda)
        }
        CategoriaIntencion::CalcularFecha => router::responder_calcular_fecha(consulta, conf),
        CategoriaIntencion::CrearRecuerdo => {
            router::responder_crear_recuerdo(consulta, conf, &mut state.memoria)
        }
        CategoriaIntencion::BuscarMemoria => {
            router::responder_buscar_memoria(consulta, conf, &state.memoria)
        }
        CategoriaIntencion::GenerarPassword => router::responder_generar_password(consulta, conf),
        CategoriaIntencion::EvaluarPassword => router::responder_evaluar_password(consulta, conf),
        CategoriaIntencion::BuscarPassword => {
            router::responder_buscar_password(consulta, conf, &state.contrasenias)
        }
        CategoriaIntencion::ConsultarDeudas => {
            router::responder_consultar_deudas(conf, &state.asesor.rastreador)
        }
        CategoriaIntencion::ConsultarFeriados => {
            router::responder_consultar_feriados(consulta, conf)
        }

        // ── Empresa / Obras / Cobranzas (Fase 7) ──
        CategoriaIntencion::ConsultarObras => router::responder_consultar_obras(conf, &state.obras),
        CategoriaIntencion::SaldoObra => router::responder_saldo_obra(conf, &state.obras),
        CategoriaIntencion::AlertasObras => router::responder_alertas_obras(conf, &state.obras),
        CategoriaIntencion::ConsultarCobranzas => {
            router::responder_consultar_cobranzas(conf, &state.cobranzas)
        }
        CategoriaIntencion::ResumenEmpresa => router::responder_resumen_empresa(
            conf,
            &state.obras,
            &state.cobranzas,
            &state.propuestas,
            &state.casos,
        ),
        CategoriaIntencion::GuiaSiguientePaso => {
            router::responder_guia_siguiente_paso(conf, &state.obras)
        }

        CategoriaIntencion::Saludo => RespuestaAsistente::solo_texto(
            CategoriaIntencion::Saludo,
            conf,
            "¡Hola! Soy tu asistente IA. Puedes pedirme en lenguaje natural:\n\
             💰 Finanzas:  \"cuánto llevo gastado este mes\" / \"resumen financiero\"\n\
             📋 Tareas:    \"agregar tarea X\" / \"mis pendientes de hoy\"\n\
             📅 Agenda:    \"el cumple de Lucho es el 12 de julio\"\n\
             🏗️  Obras:     \"cómo van mis obras\" / \"alertas en proyectos\"\n\
             💵 Cobranzas: \"qué me deben\" / \"total por cobrar\"\n\
             🏢 Empresa:   \"cómo va el negocio\" / \"propuestas activas\"\n\
             🧠 Memoria:   \"recuerda que la wifi es CafeNet123\"\n\
             🔐 Claves:    \"genera una contraseña segura\"\n\
             📆 Fechas:    \"cuántos días faltan para Navidad\"",
        ),
        CategoriaIntencion::Ayuda => RespuestaAsistente::solo_texto(
            CategoriaIntencion::Ayuda,
            conf,
            "Comandos del Asistente IA — escribe en lenguaje natural:\n\
             💰 \"gasté 25 en gasolina\" / \"resumen financiero\" / \"qué pago primero\"\n\
             📋 \"agregar tarea X\" / \"mis pendientes\" / \"marcar X como hecha\"\n\
             📅 \"agendar cita con dentista el 15\" / \"el cumple de X es el 20 de mayo\"\n\
             🏗️  \"cómo van mis obras\" / \"saldo de la obra\" / \"hay alertas\"\n\
             💵 \"qué me deben\" / \"cuentas por cobrar\"\n\
             🏢 \"cómo va la empresa\" / \"propuestas activas\"\n\
             🗺️  \"cuál es el siguiente paso en la obra\"\n\
             📆 \"cuántos días entre A y B\" / \"cuánto falta para Navidad\"\n\
             🎌 \"próximos feriados de Ecuador / USA\"\n\
             🧠 \"recuerda que...\" / \"qué sabes de X\"\n\
             🔐 \"genera contraseña\" / \"qué tan segura es '...'\"",
        ),
        _ => {
            // Fallback: detectar agenda por keywords sueltas
            let norm_q = sin_tildes(&consulta.to_lowercase());
            let es_agenda = norm_q.contains("cumplean")
                || norm_q.contains("cita")
                || norm_q.contains("evento")
                || norm_q.contains("reunion")
                || norm_q.contains("recordatorio")
                || norm_q.contains("cuantos anos tiene")
                || norm_q.contains("cuantos anios tiene")
                || norm_q.contains("que edad tiene")
                || norm_q.contains("cuanta edad tiene")
                || norm_q.contains("cual es la edad de");
            if es_agenda {
                return responder_consultar_agenda(consulta, conf, &state.agenda);
            }
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

fn responder_consultar_gastos(
    consulta: &str,
    conf: f64,
    gastos: &AlmacenGastos,
    rastreador: &crate::ml::advisor::RastreadorDeudas,
) -> RespuestaAsistente {
    // Si la consulta menciona un acreedor específico, buscar por keyword
    if let Some(keyword) = extraer_nombre_acreedor(consulta) {
        return responder_historial_acreedor(&keyword, conf, gastos, rastreador);
    }

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

    // Siempre mostrar los pagos fijos/deudas del rastreador como referencia
    let deudas_activas: Vec<_> = rastreador.deudas.iter().filter(|d| d.activa).collect();
    if !deudas_activas.is_empty() {
        let fijos: Vec<_> = deudas_activas.iter().filter(|d| d.obligatoria).collect();
        let creditos: Vec<_> = deudas_activas.iter().filter(|d| !d.obligatoria).collect();

        texto.push_str("\n\n  📋 Pagos registrados:");

        if !fijos.is_empty() {
            texto.push_str("\n  🏠 Pagos fijos:\n");
            for d in &fijos {
                texto.push_str(&format!(
                    "    • {:<25} ${:.2}/mes\n",
                    d.nombre, d.pago_minimo
                ));
            }
        }
        if !creditos.is_empty() {
            texto.push_str("  💳 Créditos/deudas:\n");
            for d in &creditos {
                texto.push_str(&format!(
                    "    • {:<25} ${:.2}/mes\n",
                    d.nombre, d.pago_minimo
                ));
            }
        }
        let total_fijos: f64 = fijos.iter().map(|d| d.pago_minimo).sum();
        let total_creditos: f64 = creditos.iter().map(|d| d.pago_minimo).sum();
        texto.push_str(&format!(
            "  ─────────────────────────────────\n  Total fijos: ${:.2}  |  Total créditos: ${:.2}",
            total_fijos, total_creditos
        ));
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

    let flujo_libre = ingreso - resumen.total_gastos.a_f64() - pagos_min;
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

fn responder_agendar_pago(consulta: &str, conf: f64, agenda: &Agenda) -> RespuestaAsistente {
    let monto = extraer_monto(consulta);
    let fecha = extraer_fecha(consulta);

    // Extraer descripción del pago (ej: "pagar luz", "recordarme pagar carrington")
    let norm = sin_tildes(&consulta.to_lowercase());
    let descripcion = {
        let triggers = [
            "pagar ",
            "pago de ",
            "recordarme pagar ",
            "recordar pago de ",
            "agendar pago de ",
        ];
        let mut desc = None;
        for t in &triggers {
            if let Some(pos) = norm.find(t) {
                let resto = norm[pos + t.len()..]
                    .split([' ', '\n'].as_ref())
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(" ");
                if !resto.is_empty() {
                    desc = Some(resto);
                    break;
                }
            }
        }
        desc
    };

    // Si ya tienen fecha y monto, mostrar confirmación y eventos de esa fecha
    if let (Some(m), Some(f)) = (monto, fecha) {
        let eventos_ese_dia = agenda.eventos_del_dia(f);
        let mut texto = format!(
            "📅 Pago detectado: ${:.2} para el {}{}\n\n\
             Para confirmar, abre 📅 Agenda → Nuevo evento → tipo \"Pago\"",
            m,
            f.format("%d/%m/%Y"),
            descripcion
                .as_deref()
                .map(|d| format!(" ({})", d))
                .unwrap_or_default(),
        );
        if !eventos_ese_dia.is_empty() {
            texto.push_str(&format!(
                "\n\n⚠️ Ese día ya tienes {} evento(s):",
                eventos_ese_dia.len()
            ));
            for e in eventos_ese_dia.iter().take(3) {
                texto.push_str(&format!("\n   • {} — {}", e.titulo, e.tipo));
            }
        }
        let mut r = RespuestaAsistente::solo_texto(CategoriaIntencion::AgendarPago, conf, texto);
        r.seguimientos
            .push("Ver todos mis pagos próximos".to_string());
        return r;
    }

    // Sin fecha/monto: pedir los datos que faltan
    let mut texto = String::from("📅 Para agendar un pago necesito:\n");
    if monto.is_none() {
        texto.push_str("  • Monto  (ej: \"pagar 200\")\n");
    }
    if fecha.is_none() {
        texto.push_str("  • Fecha  (ej: \"el 15\", \"mañana\", \"el viernes\")\n");
    }
    texto.push_str(
        "\nO abre 📅 Agenda → Nuevo evento → tipo \"Pago\" para registrar todos los detalles.",
    );

    // Mostrar próximos pagos ya agendados como referencia
    let hoy = Local::now().date_naive();
    let proximos_pagos: Vec<_> = agenda
        .eventos
        .iter()
        .filter(|e| {
            matches!(
                e.tipo,
                crate::agenda::TipoEvento::Pago | crate::agenda::TipoEvento::Recordatorio
            ) && e.fecha >= hoy
        })
        .take(3)
        .collect();
    if !proximos_pagos.is_empty() {
        texto.push_str("\n\n🔔 Pagos/recordatorios ya agendados:");
        for e in &proximos_pagos {
            texto.push_str(&format!(
                "\n   • {} — {}",
                e.fecha.format("%d/%m"),
                e.titulo
            ));
        }
    }

    RespuestaAsistente::solo_texto(CategoriaIntencion::AgendarPago, conf, texto)
}

/// Consulta la agenda: eventos de hoy, próximos, cumpleños, citas, etc.
fn responder_consultar_agenda(consulta: &str, conf: f64, agenda: &Agenda) -> RespuestaAsistente {
    let hoy = Local::now().date_naive();
    let norm = sin_tildes(&consulta.to_lowercase());

    // ── Consulta de edad de una persona ──────────────────────────────────
    // Detecta: "cuántos años tiene X", "qué edad tiene X", "cual es la edad de X", etc.
    // Nota: "años" → "anos" (ñ→n), pero muchos escriben "anios" sin ñ
    {
        let es_consulta_edad = norm.contains("cuantos anos tiene")
            || norm.contains("cuantos anios tiene")
            || norm.contains("cuantos anos cumple")
            || norm.contains("cuantos anios cumple")
            || norm.contains("que edad tiene")
            || norm.contains("cuanta edad tiene")
            || norm.contains("cual es la edad de")
            || norm.contains("cual es su edad")
            || norm.contains("cuantos anos va a cumplir")
            || norm.contains("cuantos anios va a cumplir")
            || norm.contains("cuantos anos cumplio")
            || norm.contains("cuantos anios cumplio")
            || norm.contains("cuantos anos lleva")
            || norm.contains("cuantos anios lleva");

        if es_consulta_edad {
            // Extraer el nombre buscado después del trigger de edad
            let nombre_raw: Option<String> = {
                let triggers = [
                    "cuantos anos tiene ",
                    "cuantos anios tiene ",
                    "cuantos anos cumple ",
                    "cuantos anios cumple ",
                    "que edad tiene ",
                    "cuanta edad tiene ",
                    "cual es la edad de ",
                    "cual es su edad de ",
                    "cuantos anos va a cumplir ",
                    "cuantos anios va a cumplir ",
                    "cuantos anos cumplio ",
                    "cuantos anios cumplio ",
                    "cuantos anos lleva ",
                    "cuantos anios lleva ",
                ];
                let stop_words: &[&str] = &[
                    "hoy", "manana", "ayer", "el", "la", "los", "las", "un", "una", "que", "es",
                ];
                let mut name: Option<String> = None;
                'edad_outer: for t in &triggers {
                    if let Some(pos) = norm.find(t) {
                        let resto = &norm[pos + t.len()..];
                        let palabras: Vec<&str> = resto
                            .split_whitespace()
                            .take(5)
                            .take_while(|w| !stop_words.contains(w))
                            .filter(|w| w.len() > 1)
                            .collect();
                        if !palabras.is_empty() {
                            name = Some(palabras.join(" "));
                            break 'edad_outer;
                        }
                    }
                }
                name
            };

            if let Some(nombre) = nombre_raw {
                // Buscar cumpleaños que coincidan con el nombre
                let candidatos: Vec<_> = agenda
                    .eventos
                    .iter()
                    .filter(|e| {
                        if !matches!(e.tipo, crate::agenda::TipoEvento::Cumpleanos) {
                            return false;
                        }
                        let t = sin_tildes(&e.titulo.to_lowercase());
                        let d = sin_tildes(&e.descripcion.to_lowercase());
                        let palabras: Vec<&str> =
                            nombre.split_whitespace().filter(|w| w.len() > 2).collect();
                        t.contains(nombre.as_str())
                            || d.contains(nombre.as_str())
                            || palabras.iter().any(|p| t.contains(p) || d.contains(p))
                    })
                    .collect();

                if candidatos.is_empty() {
                    let mut r = RespuestaAsistente::solo_texto(
                        CategoriaIntencion::ConsultarAgenda,
                        conf,
                        format!(
                            "🎂 No encontré el cumpleaños de \"{}\" en tu agenda.\n\
                             Agrégalo en 📅 Agenda → Nuevo evento → tipo \"Cumpleaños\".",
                            nombre
                        ),
                    );
                    r.seguimientos
                        .push("Ver todos mis eventos próximos".to_string());
                    return r;
                }

                let mut texto = String::new();
                for e in &candidatos {
                    let anio_nac = e.fecha.year();
                    // Si el cumpleaños ya ocurrió este año → edad = año_actual - año_nac
                    // Si aún no ha ocurrido este año → edad = año_actual - año_nac - 1
                    let cumple_este_anio =
                        NaiveDate::from_ymd_opt(hoy.year(), e.fecha.month(), e.fecha.day())
                            .unwrap_or_else(|| {
                                // Fallback para 29-feb en año no bisiesto
                                NaiveDate::from_ymd_opt(hoy.year(), e.fecha.month(), 28).unwrap()
                            });
                    let edad_actual = if hoy >= cumple_este_anio {
                        hoy.year() - anio_nac
                    } else {
                        hoy.year() - anio_nac - 1
                    };
                    let prox = proxima_ocurrencia_anual(e.fecha, hoy);
                    let edad_proxima = prox.year() - anio_nac;
                    let dias = (prox - hoy).num_days();
                    let frase_proximo = match dias {
                        0 => format!("¡HOY cumple {} años! 🎉", edad_proxima),
                        1 => format!("mañana cumple {} años", edad_proxima),
                        n if n > 0 => format!(
                            "el {} cumplirá {} años (en {} días)",
                            prox.format("%d/%m/%Y"),
                            edad_proxima,
                            n
                        ),
                        _ => String::new(),
                    };
                    texto.push_str(&format!(
                        "🎂 {} tiene {} años\n     Nacido/a el {}{}.\n",
                        e.titulo,
                        edad_actual,
                        e.fecha.format("%d/%m/%Y"),
                        if frase_proximo.is_empty() {
                            String::new()
                        } else {
                            format!(" — {}", frase_proximo)
                        },
                    ));
                }
                let mut r = RespuestaAsistente::solo_texto(
                    CategoriaIntencion::ConsultarAgenda,
                    conf,
                    texto,
                );
                r.seguimientos.push("Ver todos mis cumpleaños".to_string());
                return r;
            }
        }
    }

    // ── Consulta de cumpleaños por mes ───────────────────────────────────
    // Detecta: "quien cumple en julio", "quien cumple el próximo mes",
    //          "quien cumple este mes", "cumpleaños en agosto", etc.
    {
        let nombres_mes = [
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
        let mes_objetivo: Option<u32> = if norm.contains("este mes") {
            Some(hoy.month())
        } else if norm.contains("proximo mes") || norm.contains("siguiente mes") {
            Some(if hoy.month() == 12 {
                1
            } else {
                hoy.month() + 1
            })
        } else {
            nombres_mes
                .iter()
                .enumerate()
                .find(|(_, m)| norm.contains(*m))
                .map(|(i, _)| i as u32 + 1)
        };

        let es_consulta_mes = mes_objetivo.is_some()
            && (norm.contains("quien cumple")
                || norm.contains("quienes cumplen")
                || norm.contains("quienes cumplean")
                || norm.contains("cumplean")
                || norm.contains("cumple")
                || norm.contains("cumplen"));

        if es_consulta_mes {
            let mes = mes_objetivo.unwrap();
            let nombre_mes_str = nombres_mes[(mes - 1) as usize];
            let nombre_mes_actual = nombres_mes[(hoy.month() - 1) as usize];

            // Encabezado con la fecha actual para que el usuario sepa dónde estamos
            let mut texto = format!(
                "📅 Hoy es {}, {} de {} de {}\n\n",
                nombre_dia_semana(hoy.weekday()),
                hoy.day(),
                nombre_mes_actual,
                hoy.year(),
            );

            let cumples: Vec<_> = agenda
                .eventos
                .iter()
                .filter(|e| {
                    matches!(e.tipo, crate::agenda::TipoEvento::Cumpleanos)
                        && e.fecha.month() == mes
                })
                .collect();

            if cumples.is_empty() {
                texto.push_str(&format!(
                    "🎂 No hay cumpleaños registrados en {}.",
                    nombre_mes_str
                ));
            } else {
                texto.push_str(&format!(
                    "🎂 Cumpleaños en {} ({}):\n",
                    nombre_mes_str,
                    cumples.len()
                ));
                let mut lista: Vec<_> = cumples
                    .iter()
                    .map(|e| {
                        let prox = proxima_ocurrencia_anual(e.fecha, hoy);
                        (e, prox)
                    })
                    .collect();
                lista.sort_by_key(|(_, prox)| *prox);
                for (e, prox) in &lista {
                    let dias = (*prox - hoy).num_days();
                    let frase_dias = match dias {
                        0 => "¡HOY! 🎉".to_string(),
                        1 => "(mañana)".to_string(),
                        n if n > 0 => format!("(en {} días)", n),
                        n => format!("(hace {} días)", -n),
                    };
                    let edad = prox.year() - e.fecha.year();
                    let etiqueta_edad = if edad > 0 {
                        format!(" — cumple {} años", edad)
                    } else {
                        String::new()
                    };
                    texto.push_str(&format!(
                        "  🎂 {:5} — {}{} {}\n",
                        e.fecha.format("%d/%m"),
                        e.titulo,
                        etiqueta_edad,
                        frase_dias,
                    ));
                }
            }

            let mut r =
                RespuestaAsistente::solo_texto(CategoriaIntencion::ConsultarAgenda, conf, texto);
            r.seguimientos
                .push("Ver todos mis eventos próximos".to_string());
            return r;
        }
    }

    // ¿Busca un cumpleaños de alguien?
    let busca_cumple = norm.contains("cumplean");

    // Extrae la frase de búsqueda después del primer trigger, tomando hasta 4
    // palabras (cubre nombres compuestos como "Reina Amada Garcia").
    let keyword: Option<String> = {
        let triggers = [
            "cumpleanios de ",
            "cumpleanios del ",
            "cumple de ",
            "cumple del ",
            "cita con ",
            "reunion con ",
            "reunion de ",
            "evento de ",
            "recordatorio de ",
            "aniversario de ",
            "para el cumpleanios de ",
            "para el cumple de ",
            "para el evento de ",
            "de ",
        ];
        let stop_words: &[&str] = &[
            "hoy", "manana", "ayer", "el", "la", "los", "las", "un", "una", "que", "es", "para",
            "en", "con", "del",
        ];
        let mut kw: Option<String> = None;
        'outer: for t in &triggers {
            if let Some(pos) = norm.find(t) {
                let resto = &norm[pos + t.len()..];
                // Tomar hasta 4 palabras no-stopword consecutivas
                let palabras: Vec<&str> = resto
                    .split_whitespace()
                    .take(4)
                    .take_while(|w| !stop_words.contains(w) || w.len() > 4)
                    .filter(|w| w.len() > 1)
                    .collect();
                if !palabras.is_empty() {
                    kw = Some(palabras.join(" "));
                    break 'outer;
                }
            }
        }
        kw
    };

    // Búsqueda con coincidencia multi-nivel:
    // Nivel 1 — frase completa (exact substring)
    // Nivel 2 — cada palabra de la frase (any word match)
    // Nivel 3 — primera palabra de la frase (broad)
    let buscar_en_agenda = |kw: &str| -> Vec<&crate::agenda::Evento> {
        let palabras: Vec<&str> = kw.split_whitespace().filter(|w| w.len() > 2).collect();
        // Nivel 1
        let nivel1: Vec<_> = agenda
            .eventos
            .iter()
            .filter(|e| {
                let t = sin_tildes(&e.titulo.to_lowercase());
                let d = sin_tildes(&e.descripcion.to_lowercase());
                t.contains(kw) || d.contains(kw)
            })
            .collect();
        if !nivel1.is_empty() {
            return nivel1;
        }
        if palabras.is_empty() {
            return vec![];
        }
        // Nivel 2 — cualquier palabra
        let nivel2: Vec<_> = agenda
            .eventos
            .iter()
            .filter(|e| {
                let t = sin_tildes(&e.titulo.to_lowercase());
                let d = sin_tildes(&e.descripcion.to_lowercase());
                palabras.iter().any(|p| t.contains(p) || d.contains(p))
            })
            .collect();
        nivel2
    };

    // Filtrar eventos relevantes
    let proximos_dias = hoy + chrono::Duration::days(30);
    let mut eventos: Vec<&crate::agenda::Evento> = agenda
        .eventos
        .iter()
        .filter(|e| e.fecha >= hoy && e.fecha <= proximos_dias)
        .collect();
    eventos.sort_by_key(|e| e.fecha);

    // Si busca por nombre, filtrar con matching multi-nivel
    if let Some(ref kw) = keyword {
        let filtrados = buscar_en_agenda(kw.as_str());
        // Etiqueta de cuántas palabras coincidieron para mostrar al usuario
        let coincide_exacto = {
            let kw_s = kw.as_str();
            agenda.eventos.iter().any(|e| {
                sin_tildes(&e.titulo.to_lowercase()).contains(kw_s)
                    || sin_tildes(&e.descripcion.to_lowercase()).contains(kw_s)
            })
        };
        if !filtrados.is_empty() {
            // Indicar si es coincidencia exacta o parcial
            let tipo_match = if coincide_exacto {
                format!("\"{}\":", kw)
            } else {
                format!("{} (coincidencia parcial):", kw)
            };
            let mut texto = format!("🔍 Eventos relacionados con {}\n", tipo_match);
            for e in &filtrados {
                let emoji = match &e.tipo {
                    crate::agenda::TipoEvento::Cumpleanos => "🎂",
                    crate::agenda::TipoEvento::Pago => "💰",
                    crate::agenda::TipoEvento::Cita => "🩺",
                    crate::agenda::TipoEvento::Reunion => "🤝",
                    crate::agenda::TipoEvento::Recordatorio => "🔔",
                    _ => "📅",
                };
                // Calcular fecha relevante: para cumpleaños, próxima ocurrencia anual.
                // Para otros eventos, la fecha registrada.
                let (fecha_relevante, es_proximo_aniversario) =
                    if matches!(e.tipo, crate::agenda::TipoEvento::Cumpleanos) {
                        let prox = proxima_ocurrencia_anual(e.fecha, hoy);
                        (prox, prox != e.fecha)
                    } else {
                        (e.fecha, false)
                    };
                let dias = (fecha_relevante - hoy).num_days();
                let frase_dias = match dias {
                    0 => "🎉 ¡HOY!".to_string(),
                    1 => "(mañana)".to_string(),
                    -1 => "(ayer)".to_string(),
                    n if n > 0 => format!("(faltan {} días)", n),
                    n => format!("(hace {} días)", -n),
                };
                let etiqueta_fecha = if es_proximo_aniversario {
                    format!(
                        "{} ➜ próximo: {}",
                        e.fecha.format("%d/%m/%Y"),
                        fecha_relevante.format("%d/%m/%Y"),
                    )
                } else {
                    fecha_relevante.format("%d/%m/%Y").to_string()
                };
                texto.push_str(&format!(
                    "  {} {} {} — {}  ({})\n",
                    emoji, etiqueta_fecha, frase_dias, e.titulo, e.tipo
                ));
                if !e.descripcion.is_empty() {
                    texto.push_str(&format!("     {}\n", e.descripcion));
                }
                // Edad si es cumpleaños y la fecha original tenía año
                if matches!(e.tipo, crate::agenda::TipoEvento::Cumpleanos) {
                    let edad_proxima = fecha_relevante.year() - e.fecha.year();
                    if edad_proxima > 0 {
                        texto.push_str(&format!("     🎂 Cumplirá {} años\n", edad_proxima));
                    }
                }
            }
            let mut r =
                RespuestaAsistente::solo_texto(CategoriaIntencion::ConsultarAgenda, conf, texto);
            r.seguimientos
                .push("Ver todos mis eventos próximos".to_string());
            return r;
        } else if busca_cumple {
            let mut r = RespuestaAsistente::solo_texto(
                CategoriaIntencion::ConsultarAgenda,
                conf,
                format!(
                    "🎂 No encontré ningún evento con \"{}\" en tu agenda.\n\
                     Prueba con otro nombre o agrégalo en 📅 Agenda → Nuevo evento → tipo \"Cumpleaños\".",
                    kw
                ),
            );
            r.seguimientos.push("Ver próximos eventos".to_string());
            return r;
        } else {
            let mut r = RespuestaAsistente::solo_texto(
                CategoriaIntencion::ConsultarAgenda,
                conf,
                format!(
                    "🔍 No encontré eventos con \"{}\" en tu agenda.\n\
                     Verifica el nombre o intenta \"ver todos mis eventos próximos\".",
                    kw
                ),
            );
            r.seguimientos
                .push("Ver todos mis eventos próximos".to_string());
            return r;
        }
    }

    // Vista general: hoy + próximos 30 días
    let hoy_eventos = agenda.eventos_del_dia(hoy);
    let mut texto = format!(
        "📅 Agenda — hoy {} y próximos 30 días\n",
        hoy.format("%d/%m/%Y")
    );

    if hoy_eventos.is_empty() {
        texto.push_str("\n  Hoy no tienes eventos agendados.");
    } else {
        texto.push_str(&format!("\n🔴 HOY ({} evento(s)):", hoy_eventos.len()));
        for e in &hoy_eventos {
            let emoji = match &e.tipo {
                crate::agenda::TipoEvento::Cumpleanos => "🎂",
                crate::agenda::TipoEvento::Pago => "💰",
                crate::agenda::TipoEvento::Cita => "🩺",
                crate::agenda::TipoEvento::Reunion => "🤝",
                crate::agenda::TipoEvento::Recordatorio => "🔔",
                _ => "📅",
            };
            texto.push_str(&format!("\n  {} {}", emoji, e.titulo));
        }
    }

    // Próximos (excluir hoy)
    let proximos: Vec<_> = eventos.iter().filter(|e| e.fecha > hoy).take(7).collect();
    if !proximos.is_empty() {
        texto.push_str("\n\n📆 PRÓXIMOS:");
        for e in proximos {
            let emoji = match &e.tipo {
                crate::agenda::TipoEvento::Cumpleanos => "🎂",
                crate::agenda::TipoEvento::Pago => "💰",
                crate::agenda::TipoEvento::Cita => "🩺",
                crate::agenda::TipoEvento::Reunion => "🤝",
                crate::agenda::TipoEvento::Recordatorio => "🔔",
                _ => "📅",
            };
            texto.push_str(&format!(
                "\n  {} {} — {}",
                emoji,
                e.fecha.format("%d/%m"),
                e.titulo
            ));
        }
    } else if hoy_eventos.is_empty() {
        texto.push_str("\n  No tienes eventos en los próximos 30 días.");
    }

    let mut r = RespuestaAsistente::solo_texto(CategoriaIntencion::ConsultarAgenda, conf, texto);
    r.seguimientos.push("Agendar un pago".to_string());
    r.seguimientos.push("Ver resumen financiero".to_string());
    r
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

// ─── Normalización ───────────────────────────────────────────────────────────

/// Elimina tildes/acentos para que "cuánto" y "cuanto" sean equivalentes.
fn sin_tildes(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' => 'a',
            'é' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'ï' => 'i',
            'ó' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            'Á' | 'À' | 'Ä' => 'a',
            'É' | 'È' | 'Ë' => 'e',
            'Í' | 'Ì' | 'Ï' => 'i',
            'Ó' | 'Ò' | 'Ö' => 'o',
            'Ú' | 'Ù' | 'Ü' => 'u',
            'ñ' => 'n',
            'Ñ' => 'n',
            other => other,
        })
        .collect()
}

/// Para una fecha de cumpleaños registrada, devuelve la próxima ocurrencia
/// anual a partir de `hoy`. Si la fecha original ya tiene su aniversario
/// pasado este año, retorna la del año siguiente.
fn proxima_ocurrencia_anual(fecha_original: NaiveDate, hoy: NaiveDate) -> NaiveDate {
    let mes = fecha_original.month();
    let dia = fecha_original.day();
    // Probar con el año actual
    let candidato_actual = NaiveDate::from_ymd_opt(hoy.year(), mes, dia)
        .or_else(|| NaiveDate::from_ymd_opt(hoy.year(), mes, 28)) // 29-feb fallback
        .unwrap_or(hoy);
    if candidato_actual >= hoy {
        candidato_actual
    } else {
        NaiveDate::from_ymd_opt(hoy.year() + 1, mes, dia)
            .or_else(|| NaiveDate::from_ymd_opt(hoy.year() + 1, mes, 28))
            .unwrap_or(candidato_actual)
    }
}

/// Nombre del día de la semana en español.
fn nombre_dia_semana(wd: chrono::Weekday) -> &'static str {
    match wd {
        chrono::Weekday::Mon => "lunes",
        chrono::Weekday::Tue => "martes",
        chrono::Weekday::Wed => "miércoles",
        chrono::Weekday::Thu => "jueves",
        chrono::Weekday::Fri => "viernes",
        chrono::Weekday::Sat => "sábado",
        chrono::Weekday::Sun => "domingo",
    }
}

// ─── Extracción de entidades ─────────────────────────────────────────────────

/// Cuando el clasificador NLP genérico gana (Consultar / Listar / Buscar /
/// Desconocido), este detector de dominio comprueba si el texto contiene
/// léxico financiero y devuelve el intent correcto.
fn intent_financiero(consulta: &str) -> Option<CategoriaIntencion> {
    // Normalizar: minúsculas + sin tildes → "cuánto" y "cuanto" son iguales
    let norm = sin_tildes(&consulta.to_lowercase());
    let words: Vec<&str> = norm.split_whitespace().collect();
    let has_word = |w: &str| words.contains(&w);
    let has = |s: &str| norm.as_str().contains(s);

    // ¿Es una pregunta? (interrogativo)
    let es_pregunta = has_word("cuanto")
        || has_word("cuantos")
        || has_word("cuantas")
        || has_word("que")
        || has_word("como")
        || has_word("cual")
        || has_word("cuando");

    // ── 1. Consultas interrogativas sobre gastos (ANTES de RegistrarGasto) ──
    let cuanto_gaste = has_word("cuanto")
        && (has_word("gaste")
            || has_word("gasta")
            || has_word("gastado")
            || has_word("gasto")
            || has_word("he"));
    // "cuanto pago de X", "cuanto se paga en X", "cuanto pague en X"
    let cuanto_pago_de = (has_word("cuanto") || has_word("cuantos") || has_word("cuantas"))
        && (has_word("pago")
            || has_word("paga")
            || has_word("pague")
            || has_word("cobro")
            || has_word("cobra")
            || has_word("cargo")
            || has_word("cuesta")
            || has_word("vale")
            || has_word("sale"))
        && (has_word("de")
            || has_word("del")
            || has_word("por")
            || has_word("el")
            || has_word("la")
            || has_word("en")
            || has_word("a"));
    // "cuantas veces pague/pago a X"
    let cuantas_veces = (has_word("cuantas") || has_word("cuanto"))
        && has_word("veces")
        && (has_word("pague")
            || has_word("pago")
            || has_word("paga")
            || has_word("pagar")
            || has_word("cobre"));
    // "que pagos tengo de X"
    let que_pagos_tengo = has_word("que")
        && (has_word("pagos") || has_word("gastos") || has_word("cobros") || has_word("cargos"))
        && (has_word("tengo") || has_word("hay") || has_word("tiene") || has_word("existen"));
    // "pagos registrados", "mis pagos de X"
    let pagos_registrados = (has_word("registrados") || has_word("registrado"))
        && (has_word("pagos") || has_word("gastos"));
    let mis_pagos = has_word("mis") && (has_word("pagos") || has_word("cobros"));
    // "historial de X", "registro de X", "buscar X"
    let buscar_historial = has_word("historial") || has_word("registro") || has_word("registros");
    if cuanto_gaste
        || cuanto_pago_de
        || cuantas_veces
        || que_pagos_tengo
        || pagos_registrados
        || mis_pagos
        || buscar_historial
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
    let cuanto_queda = has_word("cuanto")
        && (has_word("queda") || has_word("tengo") || has_word("debo") || has_word("disponible"))
        && (has_word("dinero") || has_word("mes") || has_word("pagar"));
    let cuentas_pendientes = has_word("cuentas")
        && (has_word("pagar") || has_word("pendientes") || has_word("vencen") || has_word("tengo"));
    if has("como voy")
        || has("situacion financiera")
        || has("estado financiero")
        || has("mis finanzas")
        || has("resumen financiero")
        || has("panorama financiero")
        || has("mis deudas")
        || has("total deudas")
        || has("dinero disponible")
        || has("saldo disponible")
        || has("cuanto me queda")
        || has("cuanto debo")
        || has("cuanto queda")
        || has("cuantas cuentas")
        || has("que cuentas")
        || cuentas_pendientes
        || queda_financiero
        || cuanto_queda
        || (has_word("deudas") && !has_word("primero"))
        || (has_word("finanzas") && !has_word("gaste"))
    {
        return Some(CategoriaIntencion::ResumenFinanciero);
    }

    // ── 3. Registro de gasto (declarativo, no pregunta) ──────────────
    let gasto_verbos = ["gaste", "pague", "compre", "desembolse"];
    if !es_pregunta
        && (gasto_verbos.iter().any(|v| has_word(v)) || has("me costo") || has("cobre un cargo"))
    {
        return Some(CategoriaIntencion::RegistrarGasto);
    }

    // ── 4. Registro de ingreso (declarativo) ─────────────────────────
    let ingreso_verbos = [
        "recibi",
        "depositaron",
        "sueldo",
        "nomina",
        "salario",
        "cobre",
    ];
    if !es_pregunta
        && (ingreso_verbos.iter().any(|v| has_word(v))
            || has("entro dinero")
            || has("me pagaron")
            || has("me depositaron"))
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
    {
        return Some(CategoriaIntencion::AgendarPago);
    }

    None
}
/// Muestra el historial completo de pagos a un acreedor/descripción específico.
fn responder_historial_acreedor(
    keyword: &str,
    conf: f64,
    gastos: &AlmacenGastos,
    rastreador: &crate::ml::advisor::RastreadorDeudas,
) -> RespuestaAsistente {
    let encontrados = gastos.buscar_por_keyword(keyword);

    // Buscar también en las deudas registradas (pagos fijos/fijos)
    let kw_norm = sin_tildes(&keyword.to_lowercase());
    let deudas_match: Vec<&crate::ml::advisor::DeudaRastreada> = rastreador
        .deudas
        .iter()
        .filter(|d| sin_tildes(&d.nombre.to_lowercase()).contains(&kw_norm))
        .collect();

    if encontrados.is_empty() && deudas_match.is_empty() {
        let hoy = Local::now().date_naive();
        let resumen = gastos.resumen_mes(hoy.year(), hoy.month());
        return RespuestaAsistente::solo_texto(
            CategoriaIntencion::ConsultarGastos,
            conf,
            format!(
                "🔍 No encontré registros con \"{}\".\n\
                 Verifica que el nombre coincida con como lo escribiste al registrar el pago.\n\n\
                 📊 Resumen del mes {}/{}:\n  \
                 💸 Gastos:        ${:.2}\n  \
                 💰 Ingresos:      ${:.2}\n  \
                 ⚖  Balance:       ${:.2}\n  \
                 📝 Transacciones: {}",
                keyword,
                hoy.month(),
                hoy.year(),
                resumen.total_gastos,
                resumen.total_ingresos,
                resumen.balance,
                resumen.num_transacciones,
            ),
        );
    }

    // Si hay deudas/pagos fijos que coinciden, mostrar esa info
    if !deudas_match.is_empty() {
        let mut texto = format!("📌 \"{}\" en tus pagos registrados:\n\n", keyword);
        for d in &deudas_match {
            let estado = if d.activa {
                "Activa ✅"
            } else {
                "Inactiva ⏸"
            };
            let tipo = if d.obligatoria {
                "Pago fijo"
            } else {
                "Deuda/crédito"
            };
            texto.push_str(&format!(
                "  📋 {}\n     Tipo:         {}\n     Pago mensual: ${:.2}\n     Estado:       {}\n",
                d.nombre, tipo, d.pago_minimo, estado
            ));
            if !d.historial.is_empty() {
                let veces = d.historial.iter().filter(|m| m.pago > 0.0).count();
                let total: f64 = d.historial.iter().map(|m| m.pago).sum();
                texto.push_str(&format!(
                    "     Veces pagado: {}\n     Total pagado: ${:.2}\n",
                    veces, total
                ));
                // Últimos 3 meses
                let ultimos: Vec<_> = d.historial.iter().rev().take(3).collect();
                if !ultimos.is_empty() {
                    texto.push_str("     Últimos pagos:\n");
                    for m in ultimos {
                        texto.push_str(&format!("       • {} — ${:.2}\n", m.mes, m.pago));
                    }
                }
            }
        }
        // Cuando hay más de una deuda que coincide (ej. "Renta Amada" + "Celular Amada"),
        // mostrar el total combinado para que el usuario vea el monto global.
        if deudas_match.len() > 1 {
            let total_mensual: f64 = deudas_match.iter().map(|d| d.pago_minimo).sum();
            let total_historial: f64 = deudas_match
                .iter()
                .flat_map(|d| d.historial.iter())
                .map(|m| m.pago)
                .sum();
            texto.push_str(&format!(
                "\n  ─────────────────────────────────────\n\
                 💰 TOTAL a \"{}\": ${:.2}/mes  ({} conceptos)\n",
                keyword,
                total_mensual,
                deudas_match.len(),
            ));
            if total_historial > 0.0 {
                texto.push_str(&format!(
                    "  Total histórico combinado: ${:.2}\n",
                    total_historial
                ));
            }
        }
        let mut r =
            RespuestaAsistente::solo_texto(CategoriaIntencion::ConsultarGastos, conf, texto);
        r.seguimientos
            .push("Ver resumen financiero completo".to_string());
        r.seguimientos.push("¿Qué deuda pagar primero?".to_string());
        return r;
    }

    let total_gastos: Dinero = encontrados
        .iter()
        .filter(|g| g.monto.es_positivo())
        .map(|g| g.monto)
        .sum();
    let total_ingresos: Dinero = encontrados
        .iter()
        .filter(|g| g.monto.es_negativo())
        .map(|g| g.monto.abs())
        .sum();
    let veces_pagado = encontrados.iter().filter(|g| g.monto.es_positivo()).count();

    let mut texto = format!(
        "🔍 Historial de \"{}\" — {} registro(s) encontrado(s)\n\n\
         📊 Resumen:\n\
         \x20  💸 Veces pagado:   {}\n\
         \x20  💰 Total pagado:   ${:.2}\n",
        keyword,
        encontrados.len(),
        veces_pagado,
        total_gastos,
    );
    if total_ingresos.es_positivo() {
        texto.push_str(&format!("  💵 Total reembolso: ${:.2}\n", total_ingresos));
    }

    texto.push_str("\n📅 Detalle por fecha:\n");
    let meses = [
        "Ene", "Feb", "Mar", "Abr", "May", "Jun", "Jul", "Ago", "Sep", "Oct", "Nov", "Dic",
    ];
    for g in &encontrados {
        let mes_str = meses.get(g.fecha.month0() as usize).unwrap_or(&"?");
        let tipo = if g.monto.es_negativo() {
            "💵 Reembolso"
        } else {
            "💸 Pago    "
        };
        texto.push_str(&format!(
            "  {} {:02}/{}/{} — {} ${:.2}  [{}]\n",
            tipo,
            g.fecha.day(),
            mes_str,
            g.fecha.year(),
            g.descripcion,
            g.monto.abs(),
            g.id,
        ));
    }

    let mut r = RespuestaAsistente::solo_texto(CategoriaIntencion::ConsultarGastos, conf, texto);
    r.seguimientos
        .push(format!("¿Cuánto pagué en total a \"{}\"?", keyword));
    r.seguimientos
        .push("Ver resumen financiero completo".to_string());
    r
}

/// Extrae el nombre de un acreedor/empresa de la consulta del usuario.
/// Detecta patrones como "de carrington", "pagos a X", "historial de X", etc.
/// Funciona con o sin tildes.
fn extraer_nombre_acreedor(consulta: &str) -> Option<String> {
    // Normalizar para matching (sin tildes, minúsculas)
    let norm = sin_tildes(&consulta.to_lowercase());

    // Triggers: frases que indican que viene el nombre del acreedor
    let triggers = [
        "pagos de ",
        "pagos a ",
        "pague a ",
        "pago de ",
        "pago a ",
        "registros de ",
        "registro de ",
        "historial de ",
        "buscar ",
        "cuanto pague a ",
        "cuanto pago de ",
        "cuanto pague de ",
        "cuantas veces pague ",
        "cuantas veces pago ",
        "veces que pague ",
        "veces pague ",
        "tengo de ",
        "tengo a ",
        "gaste en ",
        "gaste a ",
        "cobros de ",
        "cobros a ",
        "informacion de ",
        "informacion sobre ",
        "datos de ",
        "se paga por ",
        "se paga el ",
        "se paga la ",
        "se paga de ",
        "se paga en ",
        "se gasta en ",
        "se gasta por ",
        "se cobra por ",
        "cobran por ",
        "cobran el ",
        "cobran de ",
        "cuesta el ",
        "cuesta la ",
        "vale el ",
        "vale la ",
        "pago por ",
        "pago el ",
        "pago la ",
        "pago en ",
        "pague por ",
        "pague en ",
    ];

    // Buscar por triggers (más específicos)
    for trigger in &triggers {
        if let Some(pos) = norm.find(trigger) {
            let after_norm = &norm[pos + trigger.len()..];
            let nombre = limpiar_articulos(
                after_norm
                    .split([',', '.', '?', '!'].as_ref())
                    .next()
                    .unwrap_or(after_norm)
                    .trim(),
            );
            if !nombre.is_empty() && nombre.split_whitespace().count() <= 4 {
                return Some(nombre);
            }
        }
    }

    // Fallback: "de/a/por" + cualquier palabra de >4 letras (nombre propio o empresa)
    let words: Vec<&str> = norm.split_whitespace().collect();
    let stopwords = [
        "este",
        "el mes",
        "la semana",
        "hoy",
        "ayer",
        "pago",
        "pagos",
        "gasto",
        "gastos",
        "cobro",
        "cobros",
        "mes",
        "semana",
        "dia",
        "fecha",
    ];
    for (i, w) in words.iter().enumerate() {
        if (*w == "de" || *w == "a" || *w == "al" || *w == "por" || *w == "en")
            && i + 1 < words.len()
        {
            // saltar artículo si el siguiente token es el/la/los/las
            let skip = matches!(words[i + 1], "el" | "la" | "los" | "las");
            let start = i + 1 + skip as usize;
            if start >= words.len() {
                continue;
            }
            let next = words[start];
            if next.len() > 3 {
                let nombre = words[start..]
                    .iter()
                    .take(2)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" ");
                if !stopwords.iter().any(|s| nombre.starts_with(s)) {
                    return Some(nombre);
                }
            }
        }
    }

    None
}

/// Elimina artículos definidos/indefinidos al inicio de un keyword.
fn limpiar_articulos(s: &str) -> String {
    let arts = [
        "el ", "la ", "los ", "las ", "un ", "una ", "unos ", "unas ", "al ", "del ",
    ];
    let mut result = s;
    for art in &arts {
        if let Some(stripped) = result.strip_prefix(art) {
            result = stripped.trim_start();
            break;
        }
    }
    result.to_string()
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

/// Wrapper público para que `router.rs` reutilice la extracción de fechas.
pub fn extraer_fecha_publica(s: &str) -> Option<NaiveDate> {
    extraer_fecha(s)
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
        // Frases con "pago de" y "cuentas"
        assert_eq!(
            intent_financiero("cuanto pago de kissimmee"),
            Some(CategoriaIntencion::ConsultarGastos)
        );
        assert_eq!(
            intent_financiero("cuanto pago de celular"),
            Some(CategoriaIntencion::ConsultarGastos)
        );
        assert_eq!(
            intent_financiero("cuantas cuentas tengo que pagar"),
            Some(CategoriaIntencion::ResumenFinanciero)
        );
        assert_eq!(
            intent_financiero("que cuentas tengo pendientes"),
            Some(CategoriaIntencion::ResumenFinanciero)
        );
        // Consultas con "que pagos tengo" / "pagos registrados"
        assert_eq!(
            intent_financiero("que pagos tengo registrados de carrington"),
            Some(CategoriaIntencion::ConsultarGastos)
        );
        assert_eq!(
            intent_financiero("mis pagos de este mes"),
            Some(CategoriaIntencion::ConsultarGastos)
        );
        assert_eq!(
            intent_financiero("que gastos tengo de comida"),
            Some(CategoriaIntencion::ConsultarGastos)
        );
    }
}
