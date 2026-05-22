//! Submenu del rastreador de deudas y funciones relacionadas.
//!
//! Extraido de `main.rs` como parte de la Fase 2 del plan de mejoramiento.
#![allow(
    clippy::to_string_in_format_args,
    clippy::print_literal,
    clippy::nonminimal_bool,
    clippy::double_ended_iterator_last,
    clippy::redundant_locals,
    clippy::unnecessary_sort_by,
    clippy::collapsible_match,
    clippy::useless_format
)]

use chrono::{Datelike, Local};
use colored::Colorize;

use omniplanner::ml::{
    AjusteMensualLibertad, BorradorPlanLibertad, DecisionPago, DeudaRastreada, EstrategiaLibertad,
    FrecuenciaPago, IngresoRastreado, RastreadorDeudas, SimulacionLibertad,
};
use omniplanner::storage::AppState;
use rust_xlsxwriter::{Format, FormatAlign, FormatBorder, Workbook};

use crate::{
    calcular_distribucion_flujo, confirmar, formatear_plazo_meses, limpiar, menu, pausa, pedir_f64,
    pedir_texto, pedir_texto_opcional, separador, PoliticaFlujo, TermConfirm,
};

pub fn menu_asesor_rastreador(state: &mut AppState) {
    // One-time prompt por sesión: si hay deudas reales sin día de corte
    // definido, ofrecer al usuario llenarlas. Si ya respondió (incluso
    // saltando), no se vuelve a preguntar hasta el próximo arranque.
    use std::sync::atomic::{AtomicBool, Ordering};
    static YA_PREGUNTADO: AtomicBool = AtomicBool::new(false);
    if !YA_PREGUNTADO.swap(true, Ordering::Relaxed) {
        prompt_inicial_dias_corte(state);
    }

    loop {
        limpiar();
        println!(
            "{}",
            "╔══════════════════════════════════════════════════════════════╗".cyan()
        );
        println!(
            "{}",
            "║  🔎 R A S T R E A D O R   D E   D E U D A S              ║"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "║  Seguimiento multi-cuenta, diagnóstico y simulación       ║".cyan()
        );
        println!(
            "{}",
            "╚══════════════════════════════════════════════════════════════╝".cyan()
        );
        println!();

        let rast = &state.asesor.rastreador;
        if rast.deudas.is_empty() {
            println!("  📌 No hay deudas registradas en el rastreador.");
            println!("  💡 Agrega tus deudas con saldo, tasa y pagos mensuales.");
        } else {
            let deudas_vencidas = rast
                .deudas
                .iter()
                .filter(|d| d.activa && !d.es_pago_corriente() && d.esta_vencida())
                .count();
            println!("  📊 Estado actual del portafolio de deudas:");
            println!();
            println!(
                "  {:<4} {:<22} {:>12} {:>8} {:>16} {:>12} {:>8}",
                "", "Cuenta", "Saldo", "Tasa%", "Pago mes", "Vencida", "Meses"
            );
            println!("  {}", "─".repeat(92));
            for d in &rast.deudas {
                let status = if d.activa { "" } else { " ✅" };
                let estado = d.estado_ui();
                let badge = estado.badge();
                if d.es_pago_corriente() {
                    let pago_display = if matches!(d.frecuencia, FrecuenciaPago::Mensual) {
                        format!("${:.2}/mes", d.pago_total_mensual())
                    } else {
                        format!(
                            "${:.2}/{} (~${:.2}/m)",
                            d.pago_minimo,
                            d.frecuencia.nombre(),
                            d.pago_total_mensual()
                        )
                    };
                    println!(
                        "  {:<4} {:<22} {:>12} {:>8} {:>16} {:>12} {:>6} 🔒",
                        badge,
                        if d.nombre.len() > 22 {
                            format!("{}…", &d.nombre[..21])
                        } else {
                            d.nombre.clone()
                        },
                        "corriente",
                        "0.0%",
                        pago_display,
                        "-",
                        d.historial.len()
                    );
                } else {
                    let tipo = if d.obligatoria { " 🔒" } else { "" };
                    let tasa_display = format!("{:.1}%", d.tasa_anual);
                    let tiene_escrow = d.tiene_escrow_configurado();
                    let pago_display = if tiene_escrow {
                        format!("${:.2}", d.pago_total_mensual())
                    } else {
                        format!("${:.2}", d.pago_pi_mensual())
                    };
                    let enganche_tag = if d.enganche > 0.01 {
                        format!(" [enganche ${:.0}]", d.enganche)
                    } else {
                        String::new()
                    };
                    let vencida_display = if d.esta_vencida() {
                        format!("${:.2} ⚠", d.deuda_vencida_total())
                            .yellow()
                            .bold()
                            .to_string()
                    } else {
                        "-".dimmed().to_string()
                    };
                    println!(
                        "  {:<4} {:<22} {:>12} {:>8} {:>16} {:>12} {:>6}{}{}{}",
                        badge,
                        if d.nombre.len() > 22 {
                            format!("{}…", &d.nombre[..21])
                        } else {
                            d.nombre.clone()
                        },
                        format!("${:.2}", d.saldo_actual()),
                        tasa_display,
                        pago_display,
                        vencida_display,
                        d.historial.len(),
                        status,
                        tipo,
                        enganche_tag
                    );
                    if tiene_escrow {
                        println!(
                            "       {}",
                            format!(
                                "↳ P&I ${:.2} + Escrow ${:.2} (seguro/impuestos)",
                                d.pago_pi_mensual(),
                                d.escrow_mensual
                            )
                            .dimmed()
                        );
                    }
                }
            }
            println!("  {}", "─".repeat(92));
            let total = rast.deuda_total_actual();
            let activas = rast.deudas_activas().len();
            println!(
                "  Total: {}  ({} activas de {})",
                format!("${:.2}", total).red().bold(),
                activas,
                rast.deudas.len()
            );
            if deudas_vencidas > 0 {
                println!(
                    "  {} {} deuda(s) vencida(s). Abre la vista dedicada para priorizarlas.",
                    "⚠️".yellow().bold(),
                    deudas_vencidas
                );
            }
            if !rast.ingresos.is_empty() {
                println!("  {}", "Ingresos:".green().bold());
                for ing in &rast.ingresos {
                    println!(
                        "    • {} — {} ({}) [{} | {}]",
                        ing.concepto,
                        format!("${:.2}", ing.monto).green(),
                        ing.frecuencia.nombre(),
                        ing.etiqueta_confirmacion(),
                        ing.etiqueta_taxes()
                    );
                }
                println!(
                    "    Total mensual confirmado: {}",
                    format!("${:.2}", rast.ingreso_mensual_confirmado())
                        .green()
                        .bold()
                );
                let ingreso_no_confirmado = rast.ingreso_mensual_no_confirmado();
                if ingreso_no_confirmado > 0.01 {
                    println!(
                        "    No confirmado (no entra en proyección): {}",
                        format!("${:.2}", ingreso_no_confirmado).yellow()
                    );
                }
            }
        }
        println!();

        let saldo_banco = state.asesor.rastreador.saldo_disponible;
        let saldo_tag = if saldo_banco > 0.01 {
            format!(" (actual: {})", format!("${:.2}", saldo_banco).green())
        } else {
            " (no registrado)".dimmed().to_string()
        };
        let opcion_saldo = format!("💰  Actualizar saldo en banco/efectivo{}", saldo_tag);

        // Indicador de plan activo en el menú
        let plan_tag = if let Some(b) = &state.asesor.borrador_plan {
            let sim_len = state
                .asesor
                .rastreador
                .simular_libertad_editado(b.presupuesto, &b.estrategia, &b.ajustes)
                .meses
                .len();
            let inicio = b
                .mes_inicio
                .as_deref()
                .unwrap_or(&b.actualizado_en[..7.min(b.actualizado_en.len())]);
            let hoy_ym = chrono::Local::now().format("%Y-%m").to_string();
            let mes_actual = {
                let parse = |s: &str| -> i32 {
                    let mut it = s.splitn(2, '-');
                    let y: i32 = it.next().and_then(|p| p.parse().ok()).unwrap_or(0);
                    let m: i32 = it.next().and_then(|p| p.parse().ok()).unwrap_or(0);
                    y * 12 + m
                };
                ((parse(&hoy_ym) - parse(inicio)).max(0) as usize) + 1
            };
            format!(" 📍 mes {}/{}", mes_actual.min(sim_len), sim_len)
        } else {
            " (sin plan guardado)".dimmed().to_string()
        };
        let opcion_seguimiento = format!(
            "📍  Seguimiento del plan — ¿estás en el camino?{}",
            plan_tag
        );

        let opciones: Vec<&str> = vec![
            "➕  Agregar nueva deuda",
            "📅  Registrar mes de pago (a una deuda)",
            "🗓️   Programar pago futuro",
            "🔍  Revisar deuda individual (análisis predatorio + pagos sugeridos)",
            "🧮  Seleccionar deudas → calcular total / simular liquidar",
            "�  Consolidación de deudas (¿me conviene este préstamo?)",
            "�📊  Diagnóstico completo (errores + recomendaciones)",
            "📈  Simulación: ¿qué hubiera pasado si...?",
            "🗺️   Simulación: camino a la libertad financiera",
            &opcion_seguimiento,
            "🧮  Proyección de pagos y liquidez",
            "📋  Tabla de aporte mínimo (¿cuánto necesito para salir en X meses?)",
            "🚨  Período de atraso / mora  (calcular lo que debes al reanudar)",
            "🔴  Ver deudas vencidas (priorizar atrasos)",
            "✏️   Editar pago de un mes",
            "⚙️   Ajustar tasa de interés",
            "💵  Configurar ingresos",
            &opcion_saldo,
            "📥  Exportar CSV del rastreador",
            "📂  Importar desde CSV (Excel convertido)",
            "🔧  Gestionar deudas (activar/desactivar, obligatoria)",
            "🔗  Vincular deudas (cuotas espejo: hipoteca ↔ escrow, etc.)",
            "🗑️   Eliminar una deuda",
            "📓  Bitácora del sistema (paper trail completo)",
            "📦  Importar / Exportar pagos (CSV, MD, JSON, Excel, SQL)",
            "🔙  Volver",
        ];

        match menu("¿Qué hacer?", &opciones) {
            Some(0) => rastreador_agregar_deuda(state),
            Some(1) => rastreador_registrar_mes(state),
            Some(2) => rastreador_programar_pago(state),
            Some(3) => rastreador_revisar_deuda_individual(state),
            Some(4) => rastreador_seleccionar_calcular(state),
            Some(5) => rastreador_consolidar_deudas(state),
            Some(6) => rastreador_diagnostico(state),
            Some(7) => rastreador_simulacion(state),
            Some(8) => rastreador_simulacion_libertad(state),
            Some(9) => rastreador_seguimiento_plan(state),
            Some(10) => rastreador_proyeccion_pagos_liquidez(state),
            Some(11) => rastreador_tabla_aporte_minimo(state),
            Some(12) => rastreador_calcular_mora(state),
            Some(13) => rastreador_ver_deudas_vencidas(state),
            Some(14) => rastreador_editar_pago(state),
            Some(15) => rastreador_ajustar_tasa(state),
            Some(16) => rastreador_ingreso(state),
            Some(17) => rastreador_actualizar_saldo(state),
            Some(18) => rastreador_exportar(state),
            Some(19) => rastreador_importar_csv(state),
            Some(20) => rastreador_gestionar_deudas(state),
            Some(21) => rastreador_gestionar_vinculos(state),
            Some(22) => rastreador_eliminar(state),
            Some(23) => rastreador_bitacora(state),
            Some(24) => crate::cli::io_modulos::menu_io_pagos(state),
            _ => return,
        }
    }
}

/// Submenú para crear/eliminar vínculos entre deudas.
/// Un vínculo significa: cuando se duplican (o multiplican) las cuotas de la
/// principal en un mes, la dependiente recibe el mismo número de cuotas de su
/// propia mensualidad. Útil para hipoteca + escrow account, leasing + seguro, etc.
pub fn rastreador_gestionar_vinculos(state: &mut AppState) {
    use omniplanner::ml::VinculoDeudas;
    loop {
        limpiar();
        separador("🔗 VÍNCULOS ENTRE DEUDAS");
        let rast = &state.asesor.rastreador;
        if rast.vinculos.is_empty() {
            println!("  📌 No hay vínculos definidos todavía.");
            println!(
                "  💡 Ejemplo: vincula 'Carrington Mortgage' (principal) con 'Escrow account'"
            );
            println!("     para que cuando dupliques la hipoteca, también se duplique el escrow.");
        } else {
            println!("  Vínculos activos:");
            for (i, v) in rast.vinculos.iter().enumerate() {
                let factor_txt = if (v.factor - 1.0).abs() < 0.001 {
                    "1 a 1".to_string()
                } else {
                    format!("factor {}", v.factor)
                };
                println!(
                    "    {}. {} → {}  ({})",
                    i + 1,
                    v.principal,
                    v.dependiente,
                    factor_txt
                );
            }
        }
        println!();

        let opciones = &["➕  Agregar vínculo", "🗑️   Eliminar vínculo", "🔙  Volver"];
        match menu("¿Qué deseas hacer?", opciones) {
            Some(0) => {
                let nombres: Vec<String> = rast.deudas.iter().map(|d| d.nombre.clone()).collect();
                if nombres.len() < 2 {
                    println!("  Necesitas al menos 2 deudas registradas.");
                    pausa();
                    continue;
                }
                let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
                let p = match menu("Deuda PRINCIPAL (la que decides multiplicar)", &refs) {
                    Some(i) => nombres[i].clone(),
                    None => continue,
                };
                let d = match menu("Deuda DEPENDIENTE (la que sigue a la principal)", &refs) {
                    Some(i) => nombres[i].clone(),
                    None => continue,
                };
                if p == d {
                    println!("  La principal y la dependiente deben ser distintas.");
                    pausa();
                    continue;
                }
                let factor = pedir_f64(
                    "Factor de cuotas (1 = misma cantidad, 0.5 = mitad, 2 = doble)",
                    1.0,
                );
                if factor <= 0.0 {
                    println!("  Factor inválido.");
                    pausa();
                    continue;
                }
                state.asesor.rastreador.vinculos.push(VinculoDeudas {
                    principal: p.clone(),
                    dependiente: d.clone(),
                    factor,
                });
                println!("  ✓ Vínculo creado: {} → {} (factor {}).", p, d, factor);
                pausa();
            }
            Some(1) => {
                if state.asesor.rastreador.vinculos.is_empty() {
                    println!("  No hay vínculos.");
                    pausa();
                    continue;
                }
                let labels: Vec<String> = state
                    .asesor
                    .rastreador
                    .vinculos
                    .iter()
                    .map(|v| format!("{} → {} (factor {})", v.principal, v.dependiente, v.factor))
                    .collect();
                let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
                if let Some(idx) = menu("¿Cuál eliminar?", &refs) {
                    state.asesor.rastreador.vinculos.remove(idx);
                    println!("  ✓ Vínculo eliminado.");
                    pausa();
                }
            }
            _ => return,
        }
    }
}

pub fn rastreador_registrar_mes(state: &mut AppState) {
    if state.asesor.rastreador.deudas.is_empty() {
        println!("  Sin deudas registradas.");
        pausa();
        return;
    }

    let nombres: Vec<String> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .map(|d| {
            format!(
                "{} — ${:.2}{}",
                d.nombre,
                d.saldo_actual(),
                if d.activa { "" } else { " ✅ (pagada)" }
            )
        })
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    if let Some(idx) = menu("¿A cuál deuda registrar pago?", &refs) {
        let d = &state.asesor.rastreador.deudas[idx];
        let es_corriente = d.es_pago_corriente();
        let saldo_act = d.saldo_actual();
        let pago_min = d.pago_total_mensual();
        let (pago_exigible_pi, pago_exigible_escrow) = d.pago_exigible_componentes_proximo_mes();
        let pago_exigible_total = d.pago_exigible_total_proximo_mes();

        // Mes sugerido: si la deuda tiene día de corte, depende de si hoy es
        // antes o después del corte. Si no, mes corriente.
        let mes_sugerido = d.mes_de_pago_para(Local::now().date_naive());
        let prompt_mes = if let Some(corte) = d.dia_corte {
            format!(
                "Mes en que se realizó el pago (YYYY-MM, vacío={} — corte día {})",
                mes_sugerido, corte
            )
        } else {
            format!(
                "Mes en que se realizó el pago (YYYY-MM, vacío={})",
                mes_sugerido
            )
        };
        let mes = pedir_texto_opcional(&prompt_mes);
        let mes = if mes.is_empty() { mes_sugerido } else { mes };

        let mut id_pago_bus_opt: Option<String> = None;

        if es_corriente {
            // Pago corriente: el saldo siempre es el monto fijo, se paga completo
            if !matches!(d.frecuencia, FrecuenciaPago::Mensual) {
                println!(
                    "  ℹ️  Pago {}: ${:.2} total (equiv. ${:.2}/mes).",
                    d.frecuencia.nombre(),
                    d.pago_minimo,
                    d.pago_total_mensual()
                );
                println!("  Puedes registrar el monto completo del período o el parcial mensual.");
                println!();
            }
            let pago = match crate::pedir_f64_opt(
                &format!(
                    "Pago realizado (${:.2} exigible)",
                    pago_exigible_total.max(pago_min)
                ),
                pago_exigible_total.max(pago_min),
            ) {
                Some(v) => v,
                None => {
                    println!("  Cancelado.");
                    pausa();
                    return;
                }
            };
            state.asesor.rastreador.deudas[idx].registrar_mes(&mes, pago_min, pago, 0.0);
            println!();
            if (pago - pago_min).abs() < 0.01 {
                println!("  ✅ {} — Pago corriente ${:.2} registrado ✓", mes, pago);
            } else {
                println!(
                    "  ⚠️ {} — Pagaste ${:.2} de ${:.2} (faltaron ${:.2})",
                    mes,
                    pago,
                    pago_min,
                    (pago_min - pago).max(0.0)
                );
            }
        } else {
            if pago_exigible_total > pago_min + 0.01 {
                println!(
                    "  ⚠️ Hay atraso acumulado. Pago exigible para este mes: ${:.2}",
                    pago_exigible_total
                );
            }
            let saldo_inicio = match crate::pedir_f64_opt(
                &format!("Saldo al inicio (${:.2} sugerido)", saldo_act),
                saldo_act,
            ) {
                Some(v) => v,
                None => {
                    println!("  Cancelado.");
                    pausa();
                    return;
                }
            };
            let pago_pi_ref = d.pago_pi_mensual();
            let tiene_escrow = d.tiene_escrow_configurado();
            let escrow_ref = d.escrow_mensual;

            // ── Paso 1: ¿qué meses cubre este pago? ─────────────────────────
            // Preguntamos PRIMERO los meses cubiertos. Eso permite calcular
            // automáticamente el monto sugerido como N × cuota mensual y
            // garantiza que MesPago.meses_cubiertos quede siempre lleno
            // (la simulación de libertad financiera depende de ese dato).
            println!();
            println!(
                "  {} ¿Qué meses cubre este pago? Separa con coma para varios.",
                "📅".cyan()
            );
            println!(
                "  Ej: '{0}' (un mes), '{0},2026-06' (dos meses), Enter = solo {0}.",
                mes
            );
            let cubiertos_raw =
                pedir_texto_opcional(&format!("Meses cubiertos por este pago (vacío={})", mes));
            let meses_cubiertos: Vec<String> = if cubiertos_raw.is_empty() {
                // Auto-copia desde `mes`; si contiene comas (ej. "2026-03, 2026-04"), separa
                if mes.contains(',') {
                    mes.split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                } else {
                    vec![mes.clone()]
                }
            } else {
                let mut v: Vec<String> = cubiertos_raw
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if v.is_empty() {
                    v.push(mes.clone());
                }
                v
            };
            let n_meses = meses_cubiertos.len().max(1) as f64;

            let pi_sugerido = (pago_exigible_pi.max(pago_pi_ref)) * n_meses;
            let escrow_sugerido = (pago_exigible_escrow.max(escrow_ref)) * n_meses;

            let pago = if tiene_escrow {
                match crate::pedir_f64_opt(
                    &format!(
                        "Pago P&I realizado (${:.2} exigible, default={}×${:.2})",
                        pi_sugerido, n_meses as u32, pago_pi_ref
                    ),
                    pago_pi_ref * n_meses,
                ) {
                    Some(v) => v,
                    None => {
                        println!("  Cancelado.");
                        pausa();
                        return;
                    }
                }
            } else {
                match crate::pedir_f64_opt(
                    &format!(
                        "Pago realizado (${:.2} exigible, default={}×${:.2})",
                        pi_sugerido, n_meses as u32, pago_pi_ref
                    ),
                    pago_pi_ref * n_meses,
                ) {
                    Some(v) => v,
                    None => {
                        println!("  Cancelado.");
                        pausa();
                        return;
                    }
                }
            };
            let pago_escrow = if tiene_escrow {
                match crate::pedir_f64_opt(
                    &format!(
                        "Pago Escrow realizado (${:.2} exigible, default={}×${:.2})",
                        escrow_sugerido, n_meses as u32, escrow_ref
                    ),
                    escrow_ref * n_meses,
                ) {
                    Some(v) => v,
                    None => {
                        println!("  Cancelado.");
                        pausa();
                        return;
                    }
                }
            } else {
                0.0
            };
            let cargos = pedir_f64("Nuevos cargos/compras ($)", 0.0);

            let nota = pedir_texto_opcional("Nota (vacío=ninguna)");
            // Análisis de ahorro: si el usuario paga más que el exigible, mostrar
            // cuánto ahorra en esta deuda y si otra deuda le daría mayor ahorro
            // con ese mismo extra mensual aplicado como política.
            let extra_sobre_exigible = (pago - pago_exigible_pi.max(pago_pi_ref)).max(0.0);
            if extra_sobre_exigible > 10.0 {
                mostrar_analisis_ahorro_pago_extra(
                    &state.asesor.rastreador,
                    idx,
                    extra_sobre_exigible,
                );
            }

            let deuda_ref = &state.asesor.rastreador.deudas[idx];
            match deuda_ref.evaluar_pago_mes(pago, pago_escrow, saldo_inicio) {
                DecisionPago::Bloquear(msg) => {
                    println!();
                    println!("  {} Pago inválido: {}", "⛔".red(), msg);
                    println!("  No se registró nada. Corrige los valores.");
                    pausa();
                    return;
                }
                DecisionPago::PedirDobleConfirmacion(msg) => {
                    println!();
                    println!("  {}  {}", "⚠️".yellow(), msg.yellow());
                    if !confirmar("¿Registrar de todos modos?", false) {
                        println!("  Cancelado.");
                        pausa();
                        return;
                    }
                }
                DecisionPago::AceptarConAviso(msg) => {
                    println!();
                    println!("  {}  {}", "⚠️".yellow(), msg);
                }
                DecisionPago::Aceptar => {}
            }

            let meses_cubiertos_clon = meses_cubiertos.clone();
            let nota_clon = nota.clone();
            state.asesor.rastreador.deudas[idx].registrar_mes_completo(
                &mes,
                saldo_inicio,
                pago,
                pago_escrow,
                cargos,
                meses_cubiertos,
                nota,
            );

            let nuevo_saldo = state.asesor.rastreador.deudas[idx].saldo_actual();
            let nombre_deuda_evt = state.asesor.rastreador.deudas[idx].nombre.clone();
            // ── Emitir evento al bus central ────────────────────────────────
            id_pago_bus_opt = Some({
                use omniplanner::eventos::{
                    EstadoEvento, EventoSistema, Modulo, Referencia, TipoEvento,
                };
                let etiq_meses = if meses_cubiertos_clon.is_empty() {
                    mes.clone()
                } else {
                    meses_cubiertos_clon.join(" + ")
                };
                let mut ev = EventoSistema::nuevo(
                    Modulo::Rastreador,
                    TipoEvento::PagoRealizado,
                    format!("Pago {} ({})", nombre_deuda_evt, etiq_meses),
                )
                .con_monto(pago + pago_escrow)
                .con_contraparte(nombre_deuda_evt.clone())
                .con_estado(EstadoEvento::Realizado)
                .con_referencia(Referencia::nueva(
                    "rastreador",
                    "deuda",
                    &nombre_deuda_evt,
                    &nombre_deuda_evt,
                ))
                .con_etiqueta("pago")
                .con_etiqueta(mes.clone());
                if !nota_clon.is_empty() {
                    ev = ev.con_nota(nota_clon);
                }
                state.bus.emitir(ev)
            });
            println!();
            if nuevo_saldo < saldo_act {
                println!(
                    "  ✅ {} — Saldo: ${:.2} → ${:.2} (bajó ${:.2})",
                    mes,
                    saldo_act,
                    nuevo_saldo,
                    saldo_act - nuevo_saldo
                );
            } else {
                println!(
                    "  ⚠️ {} — Saldo: ${:.2} → ${:.2} (subió ${:.2})",
                    mes,
                    saldo_act,
                    nuevo_saldo,
                    nuevo_saldo - saldo_act
                );
            }
            if pago_escrow > 0.01 {
                println!(
                    "  🧾 Escrow registrado: ${:.2} (no se aplica al saldo de deuda)",
                    pago_escrow
                );
            }
        }

        // ── Sincronizar hacia presupuesto ───────────────────────────────────
        let nombre_deuda = state.asesor.rastreador.deudas[idx].nombre.clone();
        let monto_total = state.asesor.rastreador.deudas[idx]
            .historial
            .iter()
            .find(|m| m.mes == mes)
            .map(|m| m.pago + m.pago_escrow)
            .unwrap_or(0.0);
        if let Some(mes_fmt) = crate::mes_a_yyyy_mm(&mes) {
            if let Some(id_pres) = crate::sincronizar_presupuesto_desde_rastreador(
                state,
                &nombre_deuda,
                &mes_fmt,
                monto_total,
            ) {
                if let Some(id_pago) = &id_pago_bus_opt {
                    state.bus.relacionar_eventos(id_pago, &id_pres);
                }
            }
        }

        pausa();
    }
}

// ══════════════════════════════════════════════════════════════
//  Helpers de entrada flexible de fechas (YYYY-MM)
// ══════════════════════════════════════════════════════════════

fn nombre_mes_a_numero(s: &str) -> Option<u32> {
    match s.to_lowercase().trim() {
        "1" | "01" | "ene" | "enero" | "jan" | "january" => Some(1),
        "2" | "02" | "feb" | "febrero" | "february" => Some(2),
        "3" | "03" | "mar" | "marzo" | "march" => Some(3),
        "4" | "04" | "abr" | "abril" | "apr" | "april" => Some(4),
        "5" | "05" | "may" | "mayo" => Some(5),
        "6" | "06" | "jun" | "junio" | "june" => Some(6),
        "7" | "07" | "jul" | "julio" | "july" => Some(7),
        "8" | "08" | "ago" | "agosto" | "aug" | "august" => Some(8),
        "9" | "09" | "sep" | "sept" | "septiembre" | "september" => Some(9),
        "10" | "oct" | "octubre" | "october" => Some(10),
        "11" | "nov" | "noviembre" | "november" => Some(11),
        "12" | "dic" | "diciembre" | "dec" | "december" => Some(12),
        _ => None,
    }
}

fn nombre_mes_espanol(m: u32) -> &'static str {
    match m {
        1 => "enero",
        2 => "febrero",
        3 => "marzo",
        4 => "abril",
        5 => "mayo",
        6 => "junio",
        7 => "julio",
        8 => "agosto",
        9 => "septiembre",
        10 => "octubre",
        11 => "noviembre",
        12 => "diciembre",
        _ => "?",
    }
}

fn anio_corto_a_largo(yy: i32) -> i32 {
    if yy <= 30 {
        2000 + yy
    } else {
        1900 + yy
    }
}

fn push_candidato_mes(results: &mut Vec<(String, String)>, y: i32, m: u32) {
    if (1..=12).contains(&m) {
        let key = format!("{:04}-{:02}", y, m);
        if !results.iter().any(|(k, _)| k == &key) {
            results.push((key, format!("{} de {:04}", nombre_mes_espanol(m), y)));
        }
    }
}

/// Avanza (o retrocede) una cadena "YYYY-MM" en `delta` meses calendario.
fn avanzar_mes(yyyy_mm: &str, delta: i32) -> String {
    let mut it = yyyy_mm.splitn(2, '-');
    let y: i32 = it.next().unwrap_or("2026").trim().parse().unwrap_or(2026);
    let m: i32 = it.next().unwrap_or("01").trim().parse().unwrap_or(1);
    let total = y * 12 + (m - 1) + delta;
    let ny = total / 12;
    let nm = total % 12 + 1;
    format!("{:04}-{:02}", ny, nm)
}

/// Intenta parsear texto libre como uno o más candidatos YYYY-MM.
/// Devuelve lista de `(yyyy_mm, descripcion_humana)`.
fn candidatos_mes(raw: &str, anio_actual: i32) -> Vec<(String, String)> {
    let s = raw.trim();
    if s.is_empty() {
        return vec![];
    }

    let mut results: Vec<(String, String)> = Vec::new();

    let sl = s.to_lowercase();

    // 1. YYYY-MM exacto
    if sl.len() == 7 && sl.as_bytes().get(4) == Some(&b'-') {
        if let (Ok(y), Ok(m)) = (sl[..4].parse::<i32>(), sl[5..].parse::<u32>()) {
            if (1..=12).contains(&m) {
                push_candidato_mes(&mut results, y, m);
                return results;
            }
        }
    }

    // 2. Con separador: "06/2026", "junio 2026", "06-2026", "jun-26"
    for sep in &['/', '-', '.', ' '] {
        if let Some(pos) = sl.find(*sep) {
            let a = sl[..pos].trim();
            let b = sl[pos + 1..].trim();
            if a.is_empty() || b.is_empty() {
                continue;
            }
            if let Some(m) = nombre_mes_a_numero(a) {
                if let Ok(y) = b.parse::<i32>() {
                    match b.len() {
                        4 => push_candidato_mes(&mut results, y, m),
                        2 => push_candidato_mes(&mut results, anio_corto_a_largo(y), m),
                        _ => {}
                    }
                }
            }
            if let (Ok(y), Some(m)) = (a.parse::<i32>(), nombre_mes_a_numero(b)) {
                if a.len() == 4 {
                    push_candidato_mes(&mut results, y, m);
                }
            }
            break;
        }
    }
    if !results.is_empty() {
        return results;
    }

    // 3. Solo texto: "junio" o "junio 2026"
    let partes: Vec<&str> = sl.split_whitespace().collect();
    match partes.as_slice() {
        [mes_s, anio_s] => {
            if let Some(m) = nombre_mes_a_numero(mes_s) {
                if let Ok(y) = anio_s.parse::<i32>() {
                    match anio_s.len() {
                        4 => push_candidato_mes(&mut results, y, m),
                        2 => push_candidato_mes(&mut results, anio_corto_a_largo(y), m),
                        _ => {}
                    }
                }
            }
        }
        [mes_s] => {
            if let Some(m) = nombre_mes_a_numero(mes_s) {
                push_candidato_mes(&mut results, anio_actual, m);
            }
        }
        _ => {}
    }
    if !results.is_empty() {
        return results;
    }

    // 4. Letra+dígito concatenado: "jun26", "junio2026"
    let alpha: String = sl.chars().filter(|c| c.is_alphabetic()).collect();
    let digs_concat: String = sl.chars().filter(|c| c.is_ascii_digit()).collect();
    if !alpha.is_empty() && !digs_concat.is_empty() && alpha.len() + digs_concat.len() == sl.len() {
        if let Some(m) = nombre_mes_a_numero(&alpha) {
            if let Ok(y) = digs_concat.parse::<i32>() {
                match digs_concat.len() {
                    4 => push_candidato_mes(&mut results, y, m),
                    2 => push_candidato_mes(&mut results, anio_corto_a_largo(y), m),
                    _ => {}
                }
            }
        }
    }
    if !results.is_empty() {
        return results;
    }

    // 5. Solo dígitos
    let digits: String = sl.chars().filter(|c| c.is_ascii_digit()).collect();
    match digits.len() {
        1 | 2 => {
            if let Ok(m) = digits.parse::<u32>() {
                push_candidato_mes(&mut results, anio_actual, m);
            }
        }
        4 => {
            // MMYY
            let mm = digits[..2].parse::<u32>().unwrap_or(0);
            let yy_m = digits[2..].parse::<i32>().unwrap_or(0);
            if (1..=12).contains(&mm) {
                push_candidato_mes(&mut results, anio_corto_a_largo(yy_m), mm);
            }
            // YYMM
            let yy_y = digits[..2].parse::<i32>().unwrap_or(0);
            let mm2 = digits[2..].parse::<u32>().unwrap_or(0);
            if (1..=12).contains(&mm2) {
                push_candidato_mes(&mut results, anio_corto_a_largo(yy_y), mm2);
            }
        }
        6 => {
            // YYYYMM
            if let (Ok(y), Ok(m)) = (digits[..4].parse::<i32>(), digits[4..].parse::<u32>()) {
                if (1..=12).contains(&m) {
                    push_candidato_mes(&mut results, y, m);
                }
            }
            // MMYYYY
            if let (Ok(m), Ok(y)) = (digits[..2].parse::<u32>(), digits[2..].parse::<i32>()) {
                if (1..=12).contains(&m) {
                    push_candidato_mes(&mut results, y, m);
                }
            }
        }
        _ => {}
    }

    results
}

/// Pide un mes en formato libre. Muestra la interpretación y pide confirmación.
/// Si el usuario escribe vacío, retorna `default` sin preguntar.
/// `require_input`: si true y `default` está vacío, no acepta entrada vacía.
fn pedir_mes_flexible(prompt: &str, default: &str, require_input: bool) -> String {
    let anio_actual = chrono::Local::now().year();
    loop {
        let raw = pedir_texto_opcional(prompt);
        if raw.trim().is_empty() {
            if !default.is_empty() {
                println!("  {} Conservando: {}", "↩".dimmed(), default.cyan());
                return default.to_string();
            }
            if !require_input {
                return String::new();
            }
            println!(
                "  ⚠️  Se requiere una fecha. Intenta con: junio, jun 2026, 06, 062026, 2026-06."
            );
            continue;
        }

        let candidatos = candidatos_mes(&raw, anio_actual);
        match candidatos.len() {
            0 => {
                println!(
                    "  {} No entendí «{}». Prueba: junio, jun 2026, 06, 062026, 2026-06.",
                    "⚠️".yellow(),
                    raw.trim()
                );
            }
            1 => {
                let (ym, desc) = &candidatos[0];
                println!("  → {} ({})", ym.cyan().bold(), desc.dimmed());
                if confirmar("¿Es la fecha correcta?", true) {
                    return ym.clone();
                }
            }
            _ => {
                println!("  «{}» tiene varias interpretaciones posibles:", raw.trim());
                let opts: Vec<String> = candidatos
                    .iter()
                    .map(|(ym, desc)| format!("{} — {}", ym, desc))
                    .collect();
                let refs: Vec<&str> = opts.iter().map(|s| s.as_str()).collect();
                if let Some(idx) = menu("¿Cuál es la fecha correcta?", &refs) {
                    return candidatos[idx].0.clone();
                }
            }
        }
    }
}

/// Pide una lista de meses en formato libre separados por coma.
/// Cada token se parsea individualmente con confirmación.
/// Si el usuario escribe vacío, retorna `default`.
fn pedir_meses_flexibles(prompt: &str, default: &[String]) -> Vec<String> {
    let anio_actual = chrono::Local::now().year();
    loop {
        let raw = pedir_texto_opcional(prompt);
        if raw.trim().is_empty() {
            if !default.is_empty() {
                println!(
                    "  {} Conservando: {}",
                    "↩".dimmed(),
                    default.join(", ").cyan()
                );
                return default.to_vec();
            }
            return vec![];
        }

        let tokens: Vec<&str> = raw
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        let mut resueltos: Vec<String> = Vec::new();
        let mut error = false;

        for token in &tokens {
            let cands = candidatos_mes(token, anio_actual);
            match cands.len() {
                0 => {
                    println!(
                        "  {} No entendí «{}». Intenta de nuevo.",
                        "⚠️".yellow(),
                        token
                    );
                    error = true;
                    break;
                }
                1 => {
                    resueltos.push(cands[0].0.clone());
                }
                _ => {
                    println!("  «{}» tiene varias interpretaciones:", token);
                    let opts: Vec<String> = cands
                        .iter()
                        .map(|(ym, desc)| format!("{} — {}", ym, desc))
                        .collect();
                    let refs: Vec<&str> = opts.iter().map(|s| s.as_str()).collect();
                    if let Some(idx) = menu("¿Cuál es?", &refs) {
                        resueltos.push(cands[idx].0.clone());
                    } else {
                        error = true;
                        break;
                    }
                }
            }
        }

        if error {
            continue;
        }

        println!("  → Meses: {}", resueltos.join(", ").cyan().bold());
        if confirmar("¿Son correctos?", true) {
            return resueltos;
        }
    }
}

// ══════════════════════════════════════════════════════════════
//  Programar pago futuro — plan de pagos pendientes
// ══════════════════════════════════════════════════════════════

pub fn rastreador_programar_pago(state: &mut AppState) {
    loop {
        limpiar();
        separador("🗓️  PLAN DE PAGOS FUTUROS");
        println!();

        // ── Construir vista unificada: programados (azul) + sin programar (naranja) ──
        let programados = &state.asesor.rastreador.pagos_programados;

        // Deudas que YA tienen pago programado
        let nombres_prog: std::collections::HashSet<String> =
            programados.iter().map(|p| p.nombre_deuda.clone()).collect();

        // Deudas activas que NO tienen pago programado
        let sin_programar: Vec<(usize, &omniplanner::ml::DeudaRastreada)> = state
            .asesor
            .rastreador
            .deudas
            .iter()
            .enumerate()
            .filter(|(_, d)| d.activa && !nombres_prog.contains(&d.nombre))
            .collect();

        let hay_algo = !programados.is_empty() || !sin_programar.is_empty();

        // ── Fondos disponibles este mes ─────────────────────────────────────
        {
            let mes_hoy = Local::now().format("%Y-%m").to_string();
            let rast = &state.asesor.rastreador;
            let saldo_banco = rast.saldo_disponible;
            let mut total_fondos = 0.0f64;

            println!("  {}", "💰 Fondos disponibles este mes:".green().bold());
            for ing in rast.ingresos.iter().filter(|i| i.confirmado) {
                if ing.frecuencia == FrecuenciaPago::UnaVez {
                    let aplica = match &ing.mes_aplicable {
                        Some(m) => *m == mes_hoy,
                        None => true,
                    };
                    if !aplica {
                        continue;
                    }
                    println!(
                        "     • {} — {} (una vez)",
                        ing.concepto,
                        format!("${:.2}", ing.monto).green()
                    );
                    total_fondos += ing.monto;
                } else {
                    let mensual = ing.monto_mensual();
                    println!(
                        "     • {} — {} ({})",
                        ing.concepto,
                        format!("${:.2}", mensual).green(),
                        ing.frecuencia.nombre()
                    );
                    total_fondos += mensual;
                }
            }
            if saldo_banco > 0.01 {
                println!(
                    "     • {} — {}",
                    "Colchón (saldo anterior)".dimmed(),
                    format!("${:.2}", saldo_banco).green()
                );
                total_fondos += saldo_banco;
            }
            println!(
                "   {} {}",
                "Total disponible:".green(),
                format!("${:.2}", total_fondos).green().bold()
            );
            println!();
        }

        if !hay_algo {
            println!("  {} No hay deudas activas registradas.", "ℹ️".cyan());
        } else {
            // Encabezado
            println!(
                "  {:<24} {:>10} {:>9}  {:>10}  {:<20}  {}",
                "Deuda".bold(),
                "P&I".bold(),
                "Escrow".bold(),
                "Mínimo".bold(),
                "Meses".bold(),
                "Pagar en".bold()
            );
            println!("  {}", "─".repeat(88));

            let mut n = 1usize;

            // ── GRUPO 1: ya programados → azul ──────────────────────────────
            for p in programados.iter() {
                let nombre_raw = if p.nombre_deuda.len() > 24 {
                    format!("{}…", &p.nombre_deuda[..23])
                } else {
                    p.nombre_deuda.clone()
                };
                let nombre = nombre_raw.blue().to_string();
                let escrow_str = if p.monto_escrow > 0.01 {
                    format!("${:.2}", p.monto_escrow).blue().to_string()
                } else {
                    "—".dimmed().to_string()
                };
                // Mínimo sugerido = mensual × número de meses cubiertos
                let n_meses_min = p.meses_cubiertos.len().max(1) as f64;
                let minimo = state
                    .asesor
                    .rastreador
                    .deudas
                    .iter()
                    .find(|d| d.nombre == p.nombre_deuda)
                    .map(|d| (d.pago_pi_mensual() + d.escrow_mensual) * n_meses_min)
                    .unwrap_or(p.monto_pi + p.monto_escrow);
                println!(
                    "  {:>2}. {:<24} {:>10} {:>9}  {:>10}  {:<20}  {}",
                    n,
                    nombre,
                    format!("${:.2}", p.monto_pi).blue(),
                    escrow_str,
                    format!("${:.2}", minimo).truecolor(220, 60, 60),
                    p.etiqueta_meses().blue().to_string(),
                    p.fecha_pago_prevista.blue().to_string()
                );
                if !p.nota.is_empty() {
                    println!("       {} {}", "📝", p.nota.dimmed());
                }
                n += 1;
            }

            // ── GRUPO 2: sin programar → naranja ────────────────────────────
            if !sin_programar.is_empty() {
                if !programados.is_empty() {
                    println!("  {}", "┄".repeat(88).truecolor(150, 100, 50));
                }
                for (_, d) in &sin_programar {
                    let nombre_raw = if d.nombre.len() > 24 {
                        format!("{}…", &d.nombre[..23])
                    } else {
                        d.nombre.clone()
                    };
                    let nombre = nombre_raw.truecolor(220, 140, 40).to_string();
                    let pi_ref = d.pago_pi_mensual();
                    let esc_ref = d.escrow_mensual;
                    let escrow_str = if esc_ref > 0.01 {
                        format!("${:.2}", esc_ref)
                            .truecolor(220, 140, 40)
                            .to_string()
                    } else {
                        "—".dimmed().to_string()
                    };
                    let minimo = pi_ref + esc_ref;
                    println!(
                        "  {:>2}. {:<24} {:>10} {:>9}  {:>10}  {:<20}  {}",
                        n,
                        nombre,
                        format!("${:.2}", pi_ref).truecolor(220, 140, 40),
                        escrow_str,
                        format!("${:.2}", minimo).truecolor(220, 60, 60),
                        "— sin programar —".truecolor(150, 100, 40),
                        "⏳ pendiente".truecolor(180, 120, 40)
                    );
                    n += 1;
                }
            }

            println!();
            let total_pi: f64 = programados.iter().map(|p| p.monto_pi).sum();
            let total_esc: f64 = programados.iter().map(|p| p.monto_escrow).sum();
            let total_min_pend: f64 = sin_programar
                .iter()
                .map(|(_, d)| d.pago_pi_mensual() + d.escrow_mensual)
                .sum();

            // Comprometido: desglose línea a línea por pago programado
            println!("  {}", "Comprometido programado:".blue().bold());
            for p in programados.iter() {
                let nombre_corto = if p.nombre_deuda.len() > 28 {
                    format!("{}…", &p.nombre_deuda[..27])
                } else {
                    p.nombre_deuda.clone()
                };
                let n_meses = p.meses_cubiertos.len().max(1);
                let meses_tag = if n_meses > 1 {
                    format!("  ({} meses)", n_meses)
                } else {
                    String::new()
                };
                if p.monto_escrow > 0.01 {
                    println!(
                        "    {:<28}  {} + {} escrow = {}{}",
                        nombre_corto,
                        format!("${:.2}", p.monto_pi).blue(),
                        format!("${:.2}", p.monto_escrow).blue(),
                        format!("${:.2}", p.monto_pi + p.monto_escrow).blue().bold(),
                        meses_tag.dimmed()
                    );
                } else {
                    println!(
                        "    {:<28}  {}{}",
                        nombre_corto,
                        format!("${:.2}", p.monto_pi).blue(),
                        meses_tag.dimmed()
                    );
                }
            }
            println!("    {}", "─".repeat(54).dimmed());
            println!(
                "    {:<28}  {} + {} escrow = {}",
                "Total programado:",
                format!("${:.2}", total_pi).blue(),
                format!("${:.2}", total_esc).blue(),
                format!("${:.2}", total_pi + total_esc).blue().bold()
            );
            if total_min_pend > 0.01 {
                println!(
                    "  Mínimo pendiente sin programar: {}",
                    format!("${:.2}", total_min_pend)
                        .truecolor(220, 60, 60)
                        .bold()
                );
                println!(
                    "  {}  Total estimado del mes: {}",
                    "→".dimmed(),
                    format!("${:.2}", total_pi + total_esc + total_min_pend)
                        .yellow()
                        .bold()
                );
            }

            // ── Flujo paso a paso ──────────────────────────────────────────
            {
                let mes_hoy = Local::now().format("%Y-%m").to_string();
                let rast = &state.asesor.rastreador;
                let saldo_banco = rast.saldo_disponible;
                let mut total_fondos = 0.0f64;
                for ing in rast.ingresos.iter().filter(|i| i.confirmado) {
                    if ing.frecuencia == FrecuenciaPago::UnaVez {
                        let aplica = match &ing.mes_aplicable {
                            Some(m) => *m == mes_hoy,
                            None => true,
                        };
                        if aplica {
                            total_fondos += ing.monto;
                        }
                    } else {
                        total_fondos += ing.monto_mensual();
                    }
                }
                if saldo_banco > 0.01 {
                    total_fondos += saldo_banco;
                }

                if total_fondos > 0.01 {
                    println!();
                    println!(
                        "  {}",
                        "── Flujo del mes ─────────────────────────────────────────"
                            .truecolor(100, 140, 200)
                    );
                    let mut saldo = total_fondos;
                    println!(
                        "  {:42} {}",
                        "Inicio:",
                        format!("${:.2}", saldo).green().bold()
                    );

                    for p in programados.iter() {
                        let costo = p.monto_pi + p.monto_escrow;
                        saldo -= costo;
                        let nombre_corto = if p.nombre_deuda.len() > 27 {
                            format!("{}…", &p.nombre_deuda[..26])
                        } else {
                            p.nombre_deuda.clone()
                        };
                        let saldo_str = if saldo >= 0.0 {
                            format!("${:.2}", saldo).green().to_string()
                        } else {
                            format!("-${:.2}", saldo.abs())
                                .truecolor(220, 60, 60)
                                .to_string()
                        };
                        println!(
                            "  (-) {:<27}  {:>10}   → {}",
                            nombre_corto,
                            format!("${:.2}", costo).blue(),
                            saldo_str
                        );
                    }

                    let saldo_tras_prog = saldo;
                    let color_prog = if saldo_tras_prog >= 0.0 {
                        format!("${:.2}", saldo_tras_prog)
                            .green()
                            .bold()
                            .to_string()
                    } else {
                        format!("-${:.2}", saldo_tras_prog.abs())
                            .truecolor(220, 60, 60)
                            .bold()
                            .to_string()
                    };
                    println!(
                        "  {:42} {} tras programados",
                        "─────────────────────────────────────────", color_prog
                    );

                    if total_min_pend > 0.01 {
                        let saldo_min = saldo - total_min_pend;
                        let color_min = if saldo_min >= 0.0 {
                            format!("${:.2}", saldo_min).green().to_string()
                        } else {
                            format!("-${:.2}", saldo_min.abs())
                                .truecolor(220, 60, 60)
                                .to_string()
                        };
                        println!(
                            "  Si paga mínimos pendientes ({}):{:>8}   → {}",
                            format!("${:.2}", total_min_pend).truecolor(220, 60, 60),
                            "",
                            color_min
                        );
                    }
                }
            }
        } // fin else hay_algo

        println!();
        let opciones = &[
            "➕  Agregar pago programado",
            "✅  Convertir a pago registrado (ya lo pagué)",
            "✏️   Editar pago programado",
            "🗑️   Eliminar programado",
            "🔙  Volver",
        ];
        match menu("Acción:", opciones) {
            Some(0) => {
                // Agregar programado
                if state.asesor.rastreador.deudas.is_empty() {
                    println!("  Sin deudas registradas.");
                    pausa();
                    continue;
                }
                let nombres: Vec<String> = state
                    .asesor
                    .rastreador
                    .deudas
                    .iter()
                    .map(|d| d.nombre.clone())
                    .collect();
                let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
                let Some(idx) = menu("¿Para cuál deuda?", &refs) else {
                    continue;
                };

                let d = &state.asesor.rastreador.deudas[idx];
                let tiene_escrow = d.tiene_escrow_configurado();
                let pago_pi_ref = d.pago_pi_mensual();
                let escrow_ref = d.escrow_mensual;

                println!();
                println!(
                    "  Deuda: {}  |  P&I/mes: ${:.2}{}",
                    d.nombre.cyan().bold(),
                    pago_pi_ref,
                    if tiene_escrow {
                        format!("  |  Escrow/mes: ${:.2}", escrow_ref)
                    } else {
                        String::new()
                    }
                );
                println!();
                let monto_pi = pedir_f64(
                    &format!("Monto P&I a pagar (${:.2} = 1 mes)", pago_pi_ref),
                    pago_pi_ref,
                );
                let monto_escrow = if tiene_escrow {
                    pedir_f64(
                        &format!("Monto Escrow a pagar (${:.2} = 1 mes)", escrow_ref),
                        escrow_ref,
                    )
                } else {
                    0.0
                };

                println!();
                println!("  Meses que cubre este pago (Enter=ninguno).");
                println!("  Acepta: junio, jun 2026, 06, 062026, 2026-06, separados por coma.");
                let meses_cubiertos = pedir_meses_flexibles("Meses cubiertos", &[]);

                let hoy_sig = {
                    let h = chrono::Local::now();
                    let (y, m) = if h.month() == 12 {
                        (h.year() + 1, 1)
                    } else {
                        (h.year(), h.month() + 1)
                    };
                    format!("{}-{:02}", y, m)
                };
                let fecha_pago = pedir_mes_flexible(
                    &format!("¿En qué mes harás el pago? (Enter={})", hoy_sig),
                    &hoy_sig,
                    false,
                );
                let nota = pedir_texto_opcional("Nota (vacío=ninguna)");

                // ── Frecuencia de pago ──────────────────────────────────
                println!();
                let (intervalo, n_total): (usize, usize) = {
                    let opciones_frec = &[
                        "Pago único (sin repetir)",
                        "Mensual (cada mes)",
                        "Bimestral (cada 2 meses)",
                        "Trimestral (cada 3 meses)",
                        "Personalizado (cada N meses)",
                    ];
                    let iv = match menu("Frecuencia de pago:", opciones_frec) {
                        Some(0) | None => 0usize,
                        Some(1) => 1,
                        Some(2) => 2,
                        Some(3) => 3,
                        Some(4) => (pedir_f64("Cada ¿cuántos meses?", 2.0) as usize).max(1),
                        _ => 0,
                    };
                    if iv == 0 {
                        (0, 1)
                    } else {
                        // Sugerido hasta fin de año
                        let mut fp_it = fecha_pago.splitn(2, '-');
                        let _fp_y: i32 = fp_it.next().unwrap_or("2026").parse().unwrap_or(2026);
                        let fp_m: i32 = fp_it.next().unwrap_or("01").parse().unwrap_or(1);
                        let hasta_dic = (((12 - fp_m).max(0) as f64) / iv as f64).ceil() as usize;
                        let hasta_dic = hasta_dic.max(1);

                        // Estimado hasta extinción de la deuda
                        let deuda_ref = &state.asesor.rastreador.deudas[idx];
                        let saldo = deuda_ref
                            .historial
                            .last()
                            .map(|m| m.saldo_final)
                            .unwrap_or(0.0);
                        let tasa_m = deuda_ref.tasa_anual / 12.0 / 100.0;
                        let pago_m = if monto_pi > 0.01 {
                            monto_pi
                        } else {
                            deuda_ref.pago_minimo
                        };
                        let meses_ext = if saldo <= 0.01 || pago_m <= 0.01 {
                            hasta_dic * iv
                        } else if tasa_m < 0.0001 {
                            (saldo / pago_m).ceil() as usize
                        } else {
                            let ratio = 1.0 - tasa_m * saldo / pago_m;
                            if ratio <= 0.0 {
                                0 // pago insuficiente
                            } else {
                                (-ratio.ln() / (1.0 + tasa_m).ln()).ceil() as usize
                            }
                        };
                        let pagos_ext = if meses_ext == 0 {
                            0 // indica que el pago no cubre intereses
                        } else {
                            ((meses_ext as f64) / (iv as f64)).ceil() as usize
                        };

                        // Menú "¿hasta cuándo?"
                        println!();
                        let etiq_ext = if pagos_ext == 0 {
                            format!(
                                "Hasta extinción de la deuda  \
                                 (⚠️  pago P&I ${:.2} no cubre intereses)",
                                monto_pi
                            )
                        } else {
                            format!(
                                "Hasta extinción de la deuda  \
                                 (~{} pagos / ~{} meses)",
                                pagos_ext,
                                pagos_ext * iv
                            )
                        };
                        let opc_fin = [
                            "Número específico de veces",
                            &format!("Hasta fin de año  ({} pagos)", hasta_dic),
                            etiq_ext.as_str(),
                        ];
                        let nt = match menu("¿Hasta cuándo repetir?", &opc_fin) {
                            Some(0) | None => {
                                let n = pedir_f64(
                                    &format!(
                                        "¿Cuántas veces en total? \
                                         (fin de año ≈ {})",
                                        hasta_dic
                                    ),
                                    hasta_dic as f64,
                                ) as usize;
                                n.max(1)
                            }
                            Some(1) => hasta_dic,
                            Some(2) => {
                                if pagos_ext == 0 {
                                    println!(
                                        "  ⚠️  El pago no cubre los intereses; \
                                         usa un monto mayor."
                                    );
                                    hasta_dic
                                } else {
                                    pagos_ext
                                }
                            }
                            _ => 1,
                        };
                        (iv, nt.max(1))
                    }
                };

                // ── Advertencia para lotes grandes ──────────────────────
                {
                    let meses_tot = n_total * intervalo.max(1);
                    if meses_tot > 24 {
                        println!();
                        println!(
                            "  {}  Esto generará {} pagos ({} meses ≈ {} años).",
                            "⚠️".yellow(),
                            n_total,
                            meses_tot,
                            meses_tot / 12
                        );
                        if meses_tot > 60 {
                            println!(
                                "     Son más de 5 años — la tabla no cabrá en \
                                 pantalla. Considera un ciclo más corto."
                            );
                        }
                        if !confirmar("¿Continuar de todas formas?", false) {
                            println!("  Cancelado.");
                            pausa();
                            continue;
                        }
                    }
                }

                // ── Generar lote de pagos ───────────────────────────────
                let mut lote: Vec<(String, Vec<String>, String)> = (0..n_total)
                    .map(|i| {
                        let offset = if intervalo == 0 {
                            0i32
                        } else {
                            (i * intervalo) as i32
                        };
                        let fecha_i = avanzar_mes(&fecha_pago, offset);
                        let meses_i: Vec<String> = meses_cubiertos
                            .iter()
                            .map(|m| avanzar_mes(m, offset))
                            .collect();
                        let nota_i = if n_total > 1 && i > 0 {
                            if nota.is_empty() {
                                format!("#{}/{}", i + 1, n_total)
                            } else {
                                format!("{} #{}/{}", nota, i + 1, n_total)
                            }
                        } else {
                            nota.clone()
                        };
                        (fecha_i, meses_i, nota_i)
                    })
                    .collect();

                // ── Revisar / editar / confirmar ────────────────────────
                let mut confirmado = false;
                loop {
                    separador("─");
                    let frec_str = match intervalo {
                        0 => "pago único".to_string(),
                        1 => "mensual".to_string(),
                        2 => "bimestral".to_string(),
                        3 => "trimestral".to_string(),
                        n => format!("cada {} meses", n),
                    };
                    if n_total == 1 {
                        println!("  ── Resumen del pago programado ──────────────────");
                    } else {
                        println!(
                            "  ── {} pagos  [frecuencia: {}] ───────────────────",
                            n_total, frec_str
                        );
                    }
                    println!(
                        "  Deuda:     {}",
                        state.asesor.rastreador.deudas[idx].nombre.cyan().bold()
                    );
                    println!("  Monto P&I: ${:.2}", monto_pi);
                    if monto_escrow > 0.01 {
                        println!("  Escrow:    ${:.2}", monto_escrow);
                    }
                    println!();
                    for (num, (fecha_i, meses_i, nota_i)) in lote.iter().enumerate() {
                        let meses_str = if meses_i.is_empty() {
                            String::new()
                        } else {
                            format!("  (cubre: {})", meses_i.join(", "))
                        };
                        let nota_str = if nota_i.is_empty() {
                            String::new()
                        } else {
                            format!("  [{}]", nota_i)
                        };
                        if n_total == 1 {
                            println!("  Fecha pago:  {}", fecha_i.cyan());
                            if !meses_i.is_empty() {
                                println!("  Meses cubr:  {}", meses_i.join(", "));
                            }
                            if !nota_i.is_empty() {
                                println!("  Nota:        {}", nota_i);
                            }
                        } else {
                            println!(
                                "    {:>2}. {}{}{}",
                                num + 1,
                                fecha_i.cyan(),
                                meses_str.dimmed(),
                                nota_str.dimmed()
                            );
                        }
                    }
                    println!();
                    let opc_rev: &[&str] = if n_total == 1 {
                        &["Confirmar", "Editar este pago", "Cancelar"]
                    } else {
                        &[
                            "Confirmar todos",
                            "Editar un pago del lote",
                            "Cortar lote (eliminar pagos desde el #N en adelante)",
                            "Cancelar",
                        ]
                    };
                    match menu("¿Qué deseas hacer?", opc_rev) {
                        Some(0) => {
                            confirmado = true;
                            break;
                        }
                        Some(1) => {
                            let num_editar = if n_total == 1 {
                                1usize
                            } else {
                                let n = pedir_f64(
                                    &format!("¿Qué número editar? (1-{})", lote.len()),
                                    1.0,
                                ) as usize;
                                n.clamp(1, lote.len())
                            };
                            if let Some(entry) = lote.get_mut(num_editar - 1) {
                                println!(
                                    "  Editando pago #{}: fecha={}, nota='{}'",
                                    num_editar, entry.0, entry.2
                                );
                                let nueva_fecha = pedir_mes_flexible(
                                    &format!("Nueva fecha (Enter = {})", entry.0),
                                    &entry.0,
                                    false,
                                );
                                if !nueva_fecha.is_empty() && nueva_fecha != entry.0 {
                                    entry.0 = nueva_fecha;
                                }
                                let nueva_nota = pedir_texto_opcional(&format!(
                                    "Nueva nota (Enter = sin cambio, actual: '{}')",
                                    entry.2
                                ));
                                if !nueva_nota.is_empty() {
                                    entry.2 = nueva_nota;
                                }
                            }
                        }
                        Some(2) if n_total > 1 => {
                            // Cortar lote desde el pago #N en adelante
                            println!();
                            println!(
                                "  El lote tiene {} pagos ({} → {}).",
                                lote.len(),
                                lote.first()
                                    .map(|e| e.0.as_str())
                                    .unwrap_or("?")
                                    .cyan()
                                    .to_string(),
                                lote.last()
                                    .map(|e| e.0.as_str())
                                    .unwrap_or("?")
                                    .cyan()
                                    .to_string()
                            );
                            let desde = pedir_f64(
                                "Mantener solo hasta el pago # (los siguientes se eliminan):",
                                lote.len() as f64,
                            ) as usize;
                            let desde = desde.clamp(1, lote.len());
                            let eliminados = lote.len() - desde;
                            if eliminados > 0 {
                                lote.truncate(desde);
                                println!(
                                    "  {} Se eliminaron {} pagos. Quedan {}.",
                                    "✂️".yellow(),
                                    eliminados,
                                    lote.len()
                                );
                            } else {
                                println!("  Sin cambios (no se eliminó ningún pago).");
                            }
                        }
                        _ => break, // cancelado (Some(3) o None cuando n_total==1 es Some(2))
                    }
                }
                if !confirmado {
                    println!("  Cancelado.");
                    pausa();
                    continue;
                }

                let nombre_deuda = state.asesor.rastreador.deudas[idx].nombre.clone();
                let fecha_primera = lote.first().map(|(f, _, _)| f.clone()).unwrap_or_default();
                let fecha_ultima = lote.last().map(|(f, _, _)| f.clone()).unwrap_or_default();

                for (fecha_i, meses_i, nota_i) in lote {
                    let meses_clon = meses_i.clone();
                    let nota_clon = nota_i.clone();
                    state.asesor.rastreador.pagos_programados.push(
                        omniplanner::ml::PagoProgramado {
                            nombre_deuda: nombre_deuda.clone(),
                            monto_pi,
                            monto_escrow,
                            meses_cubiertos: meses_i,
                            fecha_pago_prevista: fecha_i.clone(),
                            nota: nota_i,
                        },
                    );
                    // ── Emitir evento ───────────────────────────────────────
                    {
                        use omniplanner::eventos::{
                            EstadoEvento, EventoSistema, Modulo, Referencia, TipoEvento,
                        };
                        let fecha_evt = chrono::NaiveDate::parse_from_str(
                            &format!("{}-01", fecha_i),
                            "%Y-%m-%d",
                        )
                        .unwrap_or_else(|_| chrono::Local::now().date_naive());
                        let etiq_meses = if meses_clon.is_empty() {
                            fecha_i.clone()
                        } else {
                            meses_clon.join(" + ")
                        };
                        let mut ev = EventoSistema::nuevo(
                            Modulo::Rastreador,
                            TipoEvento::PagoProgramado,
                            format!("Programado: {} ({})", nombre_deuda, etiq_meses),
                        )
                        .con_fecha(fecha_evt)
                        .con_monto(monto_pi + monto_escrow)
                        .con_contraparte(nombre_deuda.clone())
                        .con_estado(EstadoEvento::Pendiente)
                        .con_referencia(Referencia::nueva(
                            "rastreador",
                            "deuda",
                            &nombre_deuda,
                            &nombre_deuda,
                        ))
                        .con_etiqueta("programado");
                        if !nota_clon.is_empty() {
                            ev = ev.con_nota(nota_clon);
                        }
                        state.bus.emitir(ev);
                    }
                }
                if n_total == 1 {
                    println!(
                        "  {} Pago programado para {}.",
                        "✅".green(),
                        fecha_primera.cyan().bold()
                    );
                } else {
                    println!(
                        "  {} {} pagos programados ({} → {}).",
                        "✅".green(),
                        n_total,
                        fecha_primera.cyan().bold(),
                        fecha_ultima.cyan().bold()
                    );
                }
                pausa();
            }
            Some(1) => {
                // Convertir programado → registrado
                if state.asesor.rastreador.pagos_programados.is_empty() {
                    println!("  No hay pagos programados.");
                    pausa();
                    continue;
                }
                let etiquetas: Vec<String> = state
                    .asesor
                    .rastreador
                    .pagos_programados
                    .iter()
                    .map(|p| {
                        format!(
                            "{} — {} — {} P&I+Escrow: ${:.2}",
                            p.nombre_deuda,
                            p.etiqueta_meses(),
                            p.fecha_pago_prevista,
                            p.monto_total()
                        )
                    })
                    .collect();
                let refs: Vec<&str> = etiquetas.iter().map(|s| s.as_str()).collect();
                let Some(pidx) = menu("¿Cuál convertir a pago real?", &refs) else {
                    continue;
                };

                let prog = state.asesor.rastreador.pagos_programados[pidx].clone();
                // Buscar la deuda
                let didx = state
                    .asesor
                    .rastreador
                    .deudas
                    .iter()
                    .position(|d| d.nombre == prog.nombre_deuda);
                let Some(didx) = didx else {
                    println!(
                        "  {} Deuda '{}' no encontrada.",
                        "⛔".red(),
                        prog.nombre_deuda
                    );
                    pausa();
                    continue;
                };

                let saldo_act = state.asesor.rastreador.deudas[didx].saldo_actual();
                let mes_registro = pedir_texto_opcional("Mes de registro (YYYY-MM, vacío=hoy)");
                let mes_registro = if mes_registro.is_empty() {
                    chrono::Local::now().format("%Y-%m").to_string()
                } else {
                    mes_registro
                };

                let saldo_inicio = pedir_f64(
                    &format!("Saldo al inicio (${:.2} sugerido)", saldo_act),
                    saldo_act,
                );

                state.asesor.rastreador.deudas[didx].registrar_mes_completo(
                    &mes_registro,
                    saldo_inicio,
                    prog.monto_pi,
                    prog.monto_escrow,
                    0.0,
                    prog.meses_cubiertos.clone(),
                    prog.nota.clone(),
                );
                // ── Emitir evento PagoRealizado (conversión programado→real) ──
                let id_pago_bus = {
                    use omniplanner::eventos::{
                        EstadoEvento, EventoSistema, Modulo, Referencia, TipoEvento,
                    };
                    let etiq_meses = if prog.meses_cubiertos.is_empty() {
                        mes_registro.clone()
                    } else {
                        prog.meses_cubiertos.join(" + ")
                    };
                    let mut ev = EventoSistema::nuevo(
                        Modulo::Rastreador,
                        TipoEvento::PagoRealizado,
                        format!("Pago {} ({})", prog.nombre_deuda, etiq_meses),
                    )
                    .con_monto(prog.monto_pi + prog.monto_escrow)
                    .con_contraparte(prog.nombre_deuda.clone())
                    .con_estado(EstadoEvento::Realizado)
                    .con_referencia(Referencia::nueva(
                        "rastreador",
                        "deuda",
                        &prog.nombre_deuda,
                        &prog.nombre_deuda,
                    ))
                    .con_etiqueta("pago")
                    .con_etiqueta("desde-programado")
                    .con_etiqueta(mes_registro.clone());
                    if !prog.nota.is_empty() {
                        ev = ev.con_nota(prog.nota.clone());
                    }
                    state.bus.emitir(ev)
                };
                // Sincronizar presupuesto
                if let Some(mes_fmt) = crate::mes_a_yyyy_mm(&mes_registro) {
                    if let Some(id_pres) = crate::sincronizar_presupuesto_desde_rastreador(
                        state,
                        &prog.nombre_deuda,
                        &mes_fmt,
                        prog.monto_total(),
                    ) {
                        state.bus.relacionar_eventos(&id_pago_bus, &id_pres);
                    }
                }
                let nuevo_saldo = state.asesor.rastreador.deudas[didx].saldo_actual();
                println!(
                    "  {} Registrado. Saldo: ${:.2} → ${:.2}",
                    "✅".green(),
                    saldo_act,
                    nuevo_saldo
                );
                state.asesor.rastreador.pagos_programados.remove(pidx);
                println!("  {} Programado eliminado de la lista.", "🗑️".dimmed());
                pausa();
            }
            Some(2) => {
                // Editar programado
                if state.asesor.rastreador.pagos_programados.is_empty() {
                    println!("  No hay pagos programados.");
                    pausa();
                    continue;
                }
                let etiquetas: Vec<String> = state
                    .asesor
                    .rastreador
                    .pagos_programados
                    .iter()
                    .map(|p| {
                        format!(
                            "{} — {} — ${:.2}",
                            p.nombre_deuda,
                            p.fecha_pago_prevista,
                            p.monto_total()
                        )
                    })
                    .collect();
                let refs: Vec<&str> = etiquetas.iter().map(|s| s.as_str()).collect();
                let Some(pidx) = menu("¿Cuál editar?", &refs) else {
                    continue;
                };

                let prog = state.asesor.rastreador.pagos_programados[pidx].clone();

                // ¿La deuda tiene escrow?
                let tiene_escrow = state
                    .asesor
                    .rastreador
                    .deudas
                    .iter()
                    .find(|d| d.nombre == prog.nombre_deuda)
                    .map(|d| d.tiene_escrow_configurado())
                    .unwrap_or(false);

                println!();
                println!(
                    "  Editando pago programado — {}",
                    prog.nombre_deuda.cyan().bold()
                );
                println!("    Fecha:          {}", prog.fecha_pago_prevista);
                println!("    Monto P&I:      ${:.2}", prog.monto_pi);
                if prog.monto_escrow > 0.01 {
                    println!("    Monto Escrow:   ${:.2}", prog.monto_escrow);
                }
                if !prog.meses_cubiertos.is_empty() {
                    println!("    Meses cubiertos: {}", prog.meses_cubiertos.join(", "));
                }
                if !prog.nota.is_empty() {
                    println!("    Nota:           {}", prog.nota);
                }
                println!();
                println!(
                    "  {} Deja vacío (Enter) para mantener el valor actual.",
                    "ℹ".cyan()
                );
                println!();

                // ── Monto P&I ────────────────────────────────────────────
                let nuevo_pi = pedir_f64(
                    &format!("Monto P&I (actual ${:.2})", prog.monto_pi),
                    prog.monto_pi,
                );

                // ── Monto Escrow ─────────────────────────────────────────
                let nuevo_escrow = if tiene_escrow || prog.monto_escrow > 0.01 {
                    pedir_f64(
                        &format!("Monto Escrow (actual ${:.2})", prog.monto_escrow),
                        prog.monto_escrow,
                    )
                } else {
                    0.0
                };

                // ── Meses cubiertos ──────────────────────────────────────
                println!();
                println!("  Acepta: junio, jun 2026, 06, 062026, 2026-06, separados por coma.");
                let nuevos_meses = pedir_meses_flexibles(
                    &format!(
                        "Meses cubiertos (actual: \"{}\")",
                        prog.meses_cubiertos.join(",")
                    ),
                    &prog.meses_cubiertos,
                );

                // ── Fecha de pago prevista ───────────────────────────────
                println!();
                let nueva_fecha = pedir_mes_flexible(
                    &format!(
                        "Mes en que harás el pago (actual: {})",
                        prog.fecha_pago_prevista
                    ),
                    &prog.fecha_pago_prevista,
                    true,
                );

                // ── Nota ─────────────────────────────────────────────────
                let nueva_nota = {
                    let input = pedir_texto_opcional(&format!("Nota (actual: \"{}\")", prog.nota));
                    if input.trim().is_empty() {
                        prog.nota.clone()
                    } else {
                        input
                    }
                };

                // ── Aplicar cambios ──────────────────────────────────────
                let p = &mut state.asesor.rastreador.pagos_programados[pidx];
                p.monto_pi = nuevo_pi;
                p.monto_escrow = nuevo_escrow;
                p.meses_cubiertos = nuevos_meses;
                p.fecha_pago_prevista = nueva_fecha.clone();
                p.nota = nueva_nota;

                println!();
                println!(
                    "  {} Pago programado actualizado para {}.",
                    "✓".green(),
                    nueva_fecha.cyan().bold()
                );
                pausa();
            }
            Some(3) => {
                // Eliminar programado
                if state.asesor.rastreador.pagos_programados.is_empty() {
                    println!("  No hay pagos programados.");
                    pausa();
                    continue;
                }
                let etiquetas: Vec<String> = state
                    .asesor
                    .rastreador
                    .pagos_programados
                    .iter()
                    .map(|p| {
                        format!(
                            "{} — {} — {}",
                            p.nombre_deuda,
                            p.etiqueta_meses(),
                            p.fecha_pago_prevista
                        )
                    })
                    .collect();
                let refs: Vec<&str> = etiquetas.iter().map(|s| s.as_str()).collect();
                if let Some(pidx) = menu("¿Cuál eliminar?", &refs) {
                    if confirmar("¿Eliminar este pago programado?", false) {
                        state.asesor.rastreador.pagos_programados.remove(pidx);
                        println!("  {} Eliminado.", "✅".green());
                    }
                    pausa();
                }
            }
            _ => return,
        }
        if let Err(e) = state.guardar() {
            println!("  {} Error guardando: {}", "⛔".red(), e);
        }
    }
}

pub fn rastreador_actualizar_saldo(state: &mut AppState) {
    println!();

    let nuevo_str = pedir_texto_opcional("Nuevo saldo disponible ($)");
    match nuevo_str.replace(',', ".").parse::<f64>() {
        Ok(v) if v >= 0.0 => {
            state.asesor.rastreador.saldo_disponible = v;
            println!();
            println!(
                "  {} Saldo actualizado: {}",
                "✓".green(),
                format!("${:.2}", v).green().bold()
            );

            // Proyección rápida al registrar
            let ingreso = state.asesor.rastreador.ingreso_mensual_total();
            let gastos = state.asesor.presupuesto.gasto_mensual();
            let pagos_min = state.asesor.rastreador.pagos_minimos_mensuales();
            let flujo = ingreso - gastos - pagos_min;
            if ingreso > 0.01 {
                println!();
                println!(
                    "  {} Proyección con flujo libre ${:.2}/mes:",
                    "📈".cyan(),
                    flujo
                );
                for meses in [1u32, 3, 6, 12] {
                    let proyectado = v + flujo * meses as f64;
                    let tag = if proyectado >= 0.0 {
                        format!("${:.2}", proyectado).green().to_string()
                    } else {
                        format!("-${:.2}", proyectado.abs()).red().to_string()
                    };
                    println!("     En {:>2} mes(es): {}", meses, tag);
                }
            }
        }
        Ok(_) => println!("  {} El saldo no puede ser negativo.", "⚠️".yellow()),
        Err(_) => println!("  {} Valor inválido, no se actualizó.", "⚠️".yellow()),
    }

    pausa();
}

// ── Rastreador de deudas multi-cuenta con diagnóstico ──

pub fn rastreador_ver_deudas_vencidas(state: &mut AppState) {
    let vencidas: Vec<(usize, &DeudaRastreada)> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .enumerate()
        .filter(|(_, d)| d.activa && !d.es_pago_corriente() && d.esta_vencida())
        .collect();

    if vencidas.is_empty() {
        println!("  No hay deudas vencidas ahora mismo.");
        pausa();
        return;
    }

    loop {
        limpiar();
        separador("🚨 DEUDAS VENCIDAS");

        println!(
            "  {:<3} {:<24} {:>12} {:>12} {:>12} {:>12}",
            "#", "Cuenta", "Saldo", "Pago mes", "Vencida", "Exigible"
        );
        println!("  {}", "─".repeat(86));

        let mut opciones = Vec::new();
        for (pos, (_, deuda)) in vencidas.iter().enumerate() {
            let vencida = deuda.deuda_vencida_total();
            let exigible = deuda.pago_exigible_total_proximo_mes();
            println!(
                "  {:<3} {:<24} {:>12} {:>12} {:>12} {:>12}",
                format!("{}.", pos + 1),
                if deuda.nombre.len() > 24 {
                    format!("{}…", &deuda.nombre[..23])
                } else {
                    deuda.nombre.clone()
                },
                format!("${:.2}", deuda.saldo_actual()),
                format!("${:.2}", deuda.pago_total_mensual()),
                format!("${:.2}", vencida).yellow(),
                format!("${:.2}", exigible).red().bold()
            );
            opciones.push(format!(
                "{} — vencida ${:.2} | exigible ${:.2}",
                deuda.nombre, vencida, exigible
            ));
        }
        println!("  {}", "─".repeat(86));
        println!("  💡 Exigible = pago del mes + atraso acumulado. Eso es lo que debes cubrir para ponerte al día.");
        println!("  💡 Vencida = atraso puro. Esa columna muestra lo que ya dejaste atrás y no quieres volver a repetir.");

        opciones.push("🔙  Volver".to_string());
        let refs: Vec<&str> = opciones.iter().map(|s| s.as_str()).collect();

        match menu("¿Qué deuda vencida quieres revisar?", &refs) {
            Some(sel) if sel < vencidas.len() => {
                let (_, deuda) = vencidas[sel];
                let (vencida_pi, vencida_escrow) = deuda.deuda_vencida_componentes();
                let (exigible_pi, exigible_escrow) = deuda.pago_exigible_componentes_proximo_mes();
                println!();
                println!("  📌 {}", deuda.nombre.bold());
                println!("    Saldo actual: ${:.2}", deuda.saldo_actual());
                println!(
                    "    Pago normal del mes: ${:.2}",
                    deuda.pago_total_mensual()
                );
                println!("    Deuda vencida: ${:.2}", deuda.deuda_vencida_total());
                println!(
                    "    Pago exigible para ponerte al día: ${:.2}",
                    deuda.pago_exigible_total_proximo_mes()
                );
                if deuda.tiene_escrow_configurado() {
                    println!("    P&I vencido: ${:.2}", vencida_pi);
                    println!("    Escrow vencido: ${:.2}", vencida_escrow);
                    println!("    P&I exigible: ${:.2}", exigible_pi);
                    println!("    Escrow exigible: ${:.2}", exigible_escrow);
                }
                println!();
                println!("  ⚠️ Si esta columna crece, significa que faltó dinero, planificación o prioridad ese mes.");
                println!("  ⚠️ La meta es bajar primero la parte vencida para no seguir arrastrando atraso.");
                pausa();
            }
            _ => return,
        }
    }
}

pub fn rastreador_revisar_deuda_individual(state: &AppState) {
    let deudas_con_interes: Vec<(usize, &omniplanner::ml::advisor::DeudaRastreada)> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .enumerate()
        .filter(|(_, d)| d.activa && !d.es_pago_corriente() && d.saldo_actual() > 0.01)
        .collect();

    if deudas_con_interes.is_empty() {
        println!("  No hay deudas activas para revisar.");
        pausa();
        return;
    }

    loop {
        limpiar();
        println!(
            "{}",
            "╔══════════════════════════════════════════════════════════════╗".cyan()
        );
        println!(
            "{}",
            "║  🔍 REVISIÓN INDIVIDUAL DE DEUDAS                         ║"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "║  Selecciona una deuda para ver análisis detallado          ║".cyan()
        );
        println!(
            "{}",
            "╚══════════════════════════════════════════════════════════════╝".cyan()
        );
        println!();

        // Resumen rápido con indicadores
        println!(
            "  {:<22} {:>11} {:>7} {:>9} {:>10} {:>10} {:>10} Estado",
            "Cuenta", "Saldo", "Tasa%", "Int/mes", "Pago mín", "Sugerido", "A capital"
        );
        println!("  {}", "─".repeat(100));

        let mut opciones_menu: Vec<String> = Vec::new();
        let mut total_sugerido: f64 = 0.0;
        let mes_hoy = chrono::Local::now().format("%Y-%m").to_string();
        let mes_hoy_alt = chrono::Local::now().format("%b %Y").to_string();
        for (_, d) in deudas_con_interes.iter() {
            let saldo = d.saldo_actual();
            let tasa_mensual = d.tasa_anual / 100.0 / 12.0;
            let interes_mensual = saldo * tasa_mensual;
            let es_predatoria = d.pago_minimo < interes_mensual && d.tasa_anual > 0.01;

            // Regla: pagar el DOBLE del mínimo o al menos +75%, lo que sea mayor
            let pago_sugerido = if d.tasa_anual >= 20.0 {
                (d.pago_minimo * 2.0)
                    .max(d.pago_minimo * 1.75)
                    .max(interes_mensual * 2.0)
            } else if d.tasa_anual > 0.01 {
                d.pago_minimo * 1.75
            } else {
                d.pago_minimo
            };
            total_sugerido += pago_sugerido;

            let a_capital_min = d.pago_minimo - interes_mensual;
            let _a_capital_sug = pago_sugerido - interes_mensual;

            let pago_este_mes = d
                .historial
                .iter()
                .find(|m| m.mes == mes_hoy || m.mes == mes_hoy_alt)
                .map(|m| m.pago)
                .unwrap_or(0.0);
            let es_mensual_frecuencia = matches!(
                d.frecuencia,
                FrecuenciaPago::Mensual | FrecuenciaPago::Quincenal | FrecuenciaPago::Semanal
            );

            let estado = if pago_este_mes < 0.01 && es_mensual_frecuencia {
                // No pagado este mes — siempre rojo salvo que ya sea CRECE
                if es_predatoria {
                    "⛔ CRECE".red().bold().to_string()
                } else {
                    "🔴 Sin pagar".red().bold().to_string()
                }
            } else if es_predatoria {
                "⛔ CRECE".red().bold().to_string()
            } else if d.tasa_anual >= 20.0 {
                "⚠️  PREDATORIA".yellow().bold().to_string()
            } else if interes_mensual > 0.01 && a_capital_min < interes_mensual * 0.3 {
                "⚠️  Lenta".yellow().to_string()
            } else if d.tasa_anual < 0.01 {
                "✅ Sin int.".green().to_string()
            } else {
                "✅ Bajando".green().to_string()
            };

            let nombre_corto = if d.nombre.len() > 21 {
                format!("{}…", &d.nombre[..20])
            } else {
                d.nombre.clone()
            };

            let capital_str = if a_capital_min < 0.0 {
                format!("-${:.0}", a_capital_min.abs()).red().to_string()
            } else {
                format!("${:.0}", a_capital_min).to_string()
            };

            println!(
                "  {:<22} {:>11} {:>6.1}% {:>9} {:>10} {:>10} {:>10} {}",
                nombre_corto,
                format!("${:.2}", saldo),
                d.tasa_anual,
                format!("${:.0}", interes_mensual),
                format!("${:.0}", d.pago_minimo),
                format!("${:.0}", pago_sugerido).green(),
                capital_str,
                estado
            );

            let tag = if es_predatoria {
                " ⛔ CRECE"
            } else if pago_este_mes < 0.01 && es_mensual_frecuencia {
                " 🔴"
            } else if d.tasa_anual >= 20.0 {
                " ⚠️"
            } else {
                ""
            };
            opciones_menu.push(format!("{}  ${:.2}{}", d.nombre, saldo, tag));
        }
        println!("  {}", "─".repeat(100));

        // Totales
        let total_saldo: f64 = deudas_con_interes
            .iter()
            .map(|(_, d)| d.saldo_actual())
            .sum();
        let total_interes: f64 = deudas_con_interes
            .iter()
            .map(|(_, d)| d.saldo_actual() * d.tasa_anual / 100.0 / 12.0)
            .sum();
        let total_minimos: f64 = deudas_con_interes.iter().map(|(_, d)| d.pago_minimo).sum();

        println!(
            "  {:<22} {:>11} {:>7} {:>9} {:>10} {:>10}",
            "TOTALES",
            format!("${:.2}", total_saldo).red().bold(),
            "",
            format!("${:.0}", total_interes).red(),
            format!("${:.0}", total_minimos).yellow(),
            format!("${:.0}", total_sugerido).green().bold()
        );
        println!();

        // Warning box siempre visible
        println!(
            "  {}",
            "┌──────────────────────────────────────────────────────────────────┐".yellow()
        );
        println!(
            "  {} ⚠️  REGLA DE ORO: Pagar SIEMPRE el DOBLE del mínimo o +75%{}  {}",
            "│".yellow(),
            " ".repeat(5),
            "│".yellow()
        );
        println!(
            "  {} El pago mínimo es una TRAMPA — solo alimenta intereses{}     {}",
            "│".yellow(),
            " ".repeat(5),
            "│".yellow()
        );
        println!(
            "  {}",
            "├──────────────────────────────────────────────────────────────────┤".yellow()
        );
        // Show each card's minimum as warning
        for (_, d) in &deudas_con_interes {
            if d.tasa_anual >= 20.0 {
                let int_m = d.saldo_actual() * d.tasa_anual / 100.0 / 12.0;
                let sug = (d.pago_minimo * 2.0)
                    .max(d.pago_minimo * 1.75)
                    .max(int_m * 2.0);
                let crece = if d.pago_minimo < int_m {
                    " ⛔ CRECE"
                } else {
                    ""
                };
                println!(
                    "  {} {:<20} mín: ${:<8.0} → sugerido: ${:<8.0} (int: ${:.0}/mes){}{}",
                    "│".yellow(),
                    d.nombre,
                    d.pago_minimo,
                    sug,
                    int_m,
                    crece,
                    format!("{:>width$}│", "", width = 1).yellow()
                );
            }
        }
        println!(
            "  {}",
            "└──────────────────────────────────────────────────────────────────┘".yellow()
        );

        if total_interes > total_minimos * 0.4 {
            println!();
            println!(
                "  🚨 De los ${:.0} en pagos mínimos, ${:.0} ({:.0}%) se va SOLO a intereses.",
                total_minimos,
                total_interes,
                (total_interes / total_minimos) * 100.0
            );
            println!(
                "     Pagando los sugeridos (${:.0}/mes), más dinero iría a reducir la deuda.",
                total_sugerido
            );
        }
        println!();

        // ── LEYENDA ──────────────────────────────────────────────────────────
        println!("  {}", "─".repeat(72));
        println!("  {} {}", "LEYENDA — columna Estado:".bold(), "");
        println!(
            "   {}  {}   {}  {}   {}  {}   {}  {}   {}  {}",
            "✅ Bajando".green(),
            "= pagado, saldo reduce",
            "✅ Sin int.".green(),
            "= sin tasa, pagado",
            "⚡ Parcial".yellow(),
            "= pago menor al plan",
            "⚠️  PREDATORIA".yellow(),
            "= tasa ≥20%, riesgo alto",
            "⛔ CRECE".red().bold(),
            "= interés > pago mínimo",
        );
        println!(
            "   {}  {}   {}  {}",
            "🔴 Sin pagar".red().bold(),
            "= no hay pago registrado este mes",
            "🟠 naranja".yellow(),
            "= advertencia, revisar",
        );
        println!("  {}", "─".repeat(72));
        println!();

        opciones_menu.push("🔙  Volver".to_string());
        let opciones_ref: Vec<&str> = opciones_menu.iter().map(|s| s.as_str()).collect();

        match menu("¿Qué deuda deseas revisar?", &opciones_ref) {
            Some(i) if i < deudas_con_interes.len() => {
                let (_, deuda) = deudas_con_interes[i];
                let mes_hoy_pres = chrono::Local::now().format("%Y-%m").to_string();
                let datos_pres = state
                    .presupuesto
                    .mes_actual(&mes_hoy_pres)
                    .and_then(|p| {
                        let dn = deuda.nombre.to_lowercase();
                        p.lineas.iter().find(|l| {
                            let ln = l.nombre.to_lowercase();
                            ln.contains(&dn) || dn.contains(&ln)
                        })
                    })
                    .map(|l| (l.monto, l.monto_pagado_real));
                mostrar_analisis_deuda_individual(deuda, datos_pres);
            }
            _ => return,
        }
    }
}

pub fn mostrar_analisis_deuda_individual(
    d: &omniplanner::ml::advisor::DeudaRastreada,
    datos_presupuesto: Option<(f64, f64)>,
) {
    let saldo = d.saldo_actual();
    let tasa_mensual = d.tasa_anual / 100.0 / 12.0;
    let interes_mensual = saldo * tasa_mensual;
    let pago_base = d.pago_pi_mensual();
    let es_predatoria = pago_base < interes_mensual && d.tasa_anual > 0.01;
    let pago_para_empatar = interes_mensual * 1.005;
    // Regla de oro: doble del mínimo o +75%, lo que sea mayor; nunca menos que 2x el interés
    let pago_sugerido = if d.tasa_anual >= 20.0 {
        (pago_base * 2.0)
            .max(pago_base * 1.75)
            .max(interes_mensual * 2.0)
    } else if d.tasa_anual > 0.01 {
        pago_base * 1.75
    } else {
        pago_base
    };

    loop {
        limpiar();

        // ── Encabezado ──
        if es_predatoria {
            println!(
                "{}",
                "╔══════════════════════════════════════════════════════════════╗".red()
            );
            println!(
                "{}",
                format!("║  ⛔ DEUDA PREDATORIA: {:<38}║", d.nombre)
                    .red()
                    .bold()
            );
            println!(
                "{}",
                "║  El pago mínimo NO cubre los intereses — la deuda CRECE    ║".red()
            );
            println!(
                "{}",
                "╚══════════════════════════════════════════════════════════════╝".red()
            );
        } else if d.tasa_anual >= 20.0 {
            println!(
                "{}",
                "╔══════════════════════════════════════════════════════════════╗".yellow()
            );
            println!(
                "{}",
                format!("║  ⚠️  TASA PREDATORIA: {:<37}║", d.nombre)
                    .yellow()
                    .bold()
            );
            println!(
                "{}",
                "║  Tasa ≥20% — cada mes que pase es dinero regalado al banco  ║".yellow()
            );
            println!(
                "{}",
                "╚══════════════════════════════════════════════════════════════╝".yellow()
            );
        } else {
            println!(
                "{}",
                "╔══════════════════════════════════════════════════════════════╗".cyan()
            );
            println!(
                "{}",
                format!("║  🔍 ANÁLISIS: {:<45}║", d.nombre).cyan().bold()
            );
            println!(
                "{}",
                "╚══════════════════════════════════════════════════════════════╝".cyan()
            );
        }

        // ── WARNING: Pago mínimo siempre visible ──
        println!();
        println!(
            "  {}",
            "┌──────────────────────────────────────────────────────────────┐".yellow()
        );
        println!(
            "  {}  ⚠️  PAGO MÍNIMO:  {}    ←  esto es lo que pide el banco{}",
            "│".yellow(),
            format!("${:.2}", pago_base).red().bold(),
            format!("{:>width$}│", "", width = 3).yellow()
        );
        println!(
            "  {}  💰 PAGO SUGERIDO: {}    ←  mínimo para avanzar de verdad{}",
            "│".yellow(),
            format!("${:.2}", pago_sugerido).green().bold(),
            format!("{:>width$}│", "", width = 1).yellow()
        );
        if es_predatoria {
            println!(
                "  {}  🛑 PARA EMPATAR:  {}    ←  solo para que DEJE de crecer{}",
                "│".yellow(),
                format!("${:.2}", pago_para_empatar).yellow().bold(),
                format!("{:>width$}│", "", width = 1).yellow()
            );
        }
        println!(
            "  {}",
            "└──────────────────────────────────────────────────────────────┘".yellow()
        );

        // ── Situación de pago: Exigible / Disponible / Realizable ──
        {
            let exigible = d.pago_exigible_total_proximo_mes();
            let vencido = d.deuda_vencida_total();
            let normal = d.pago_total_mensual();
            let (pres_asignado, pres_pagado) = datos_presupuesto.unwrap_or((0.0, 0.0));
            let disponible = pres_asignado;
            let realizable = pres_pagado;

            println!();
            println!("  {}", "📌 SITUACIÓN DE PAGO ESTE MES".bold());
            println!("  {}", "─".repeat(68));
            println!(
                "  {:<14} {}  ←  lo que el banco exige cobrar HOY",
                "Exigible:".bold(),
                format!("${:.2}", exigible).red().bold()
            );
            if vencido > 0.01 {
                let (venc_pi, venc_esc) = d.deuda_vencida_componentes();
                if d.tiene_escrow_configurado() {
                    println!(
                        "  {:<14}   cuota normal ${:.2}  +  P&I vencido ${:.2}  +  escrow vencido ${:.2}",
                        "", normal, venc_pi, venc_esc
                    );
                } else {
                    println!(
                        "  {:<14}   cuota normal ${:.2}  +  atraso vencido ${:.2}",
                        "", normal, vencido
                    );
                }
            } else {
                println!("  {:<14}   cuota normal del mes, sin atrasos", "");
            }

            if disponible > 0.01 {
                let cubre = disponible >= exigible * 0.95;
                let disponible_str = format!("${:.2}", disponible);
                let disponible_col = if cubre {
                    disponible_str.green()
                } else {
                    disponible_str.yellow()
                };
                println!(
                    "  {:<14} {}  ←  lo asignado en Presupuesto Base Cero este mes",
                    "Disponible:".bold(),
                    disponible_col
                );
                if realizable > 0.01 {
                    let real_str = format!("${:.2}", realizable);
                    let real_col = if realizable >= exigible * 0.95 {
                        real_str.green()
                    } else {
                        real_str.yellow()
                    };
                    println!(
                        "  {:<14} {}  ←  lo que ya marcaste como pagado en presupuesto",
                        "Realizable:".bold(),
                        real_col
                    );
                } else {
                    println!(
                        "  {:<14} {}  ←  aún no marcado como pagado en presupuesto",
                        "Realizable:".bold(),
                        "$0.00 pendiente".dimmed()
                    );
                }
                let brecha = exigible - disponible;
                if brecha > 0.01 {
                    println!();
                    println!(
                        "  ⚠️  {} Hay una brecha de {} entre lo exigible y lo disponible.",
                        "ATENCIÓN:".yellow().bold(),
                        format!("${:.2}", brecha).red().bold()
                    );
                } else if cubre {
                    println!();
                    println!("  ✅ El presupuesto cubre el exigible de este mes.");
                }
            } else {
                println!(
                    "  {:<14} {}  ←  no encontrado en Presupuesto Base Cero",
                    "Disponible:".bold(),
                    "sin datos".dimmed()
                );
                println!("  {:<14} {}", "Realizable:".bold(), "sin datos".dimmed());
            }

            if vencido > 0.01 {
                let meses_atraso = (vencido / normal).round() as u32;
                println!();
                println!(
                    "  🔴 Tienes {} de deuda vencida (~{} mes{} de atraso).",
                    format!("${:.2}", vencido).red().bold(),
                    meses_atraso,
                    if meses_atraso == 1 { "" } else { "es" }
                );
                println!(
                    "     Para ponerte al corriente este mes: {}  (cuota normal: ${:.2})",
                    format!("${:.2}", exigible).red().bold(),
                    normal
                );
                println!(
                    "     Después de ponerte al corriente: pago normal de ${:.2}/mes.",
                    normal
                );
            }
            println!("  {}", "─".repeat(68));
        }

        // ── Sección 1: Radiografía ──
        println!();
        println!("  📋 RADIOGRAFÍA DE LA DEUDA");
        println!("  {}", "─".repeat(60));
        println!(
            "  Saldo actual:           {}",
            format!("${:.2}", saldo).red().bold()
        );
        println!(
            "  Tasa anual:             {}  (todas las tarjetas al 30% son predatorias)",
            format!("{:.1}%", d.tasa_anual).red()
        );
        println!("  Tasa mensual:           {:.2}%", tasa_mensual * 100.0);
        println!(
            "  Intereses que genera:   {} cada mes",
            format!("${:.2}", interes_mensual).red().bold()
        );
        println!(
            "  Intereses al año:       {} — dinero regalado al banco",
            format!("${:.2}", interes_mensual * 12.0).red()
        );
        println!(
            "  Pago mínimo del banco:  {} ← NO pagues solo esto",
            format!("${:.2}", pago_base).yellow()
        );
        if d.tiene_escrow_configurado() {
            println!(
                "  Escrow mensual:         {} (hazard insurance/impuestos)",
                format!("${:.2}", d.escrow_mensual).cyan()
            );
            println!(
                "  Pago total mensual:     {}  (P&I + escrow)",
                format!("${:.2}", d.pago_total_mensual()).cyan().bold()
            );
        }
        println!(
            "  Pago sugerido (×2/+75%):{}  ← MÍNIMO recomendado",
            format!("${:.2}", pago_sugerido).green().bold()
        );

        if es_predatoria {
            let deficit = interes_mensual - d.pago_minimo;
            println!();
            println!(
                "  ⛔ ALERTA CRÍTICA: Pagando el mínimo de ${:.2}, la deuda SUBE ${:.2}/mes",
                pago_base, deficit
            );
            println!(
                "    → En 12 meses habrás pagado ${:.2} y la deuda habrá SUBIDO",
                pago_base * 12.0
            );
            println!(
                "    → Necesitas pagar al menos {} para que deje de crecer",
                format!("${:.2}", pago_para_empatar).yellow().bold()
            );
            println!(
                "    → Con el sugerido de {} empezarías a reducirla de verdad",
                format!("${:.2}", pago_sugerido).green().bold()
            );
        } else if d.tasa_anual > 0.01 {
            let a_capital_min = pago_base - interes_mensual;
            let a_capital_sug = pago_sugerido - interes_mensual;
            let pct_interes = (interes_mensual / pago_base) * 100.0;
            println!();
            println!("  Pagando el mínimo de ${:.2}:", pago_base);
            println!(
                "    → ${:.2} ({:.0}%) se va a intereses (dinero regalado al banco)",
                interes_mensual, pct_interes
            );
            println!(
                "    → ${:.2} ({:.0}%) reduce tu deuda realmente",
                a_capital_min,
                100.0 - pct_interes
            );
            println!();
            println!(
                "  Pagando el sugerido de {}:",
                format!("${:.2}", pago_sugerido).green()
            );
            println!(
                "    → ${:.2} iría a capital — {:.1}× más rápido que con el mínimo",
                a_capital_sug,
                if a_capital_min > 0.01 {
                    a_capital_sug / a_capital_min
                } else {
                    0.0
                }
            );
        }

        // ── Sección 2: Tabla comparativa de pagos ──
        println!();
        println!("  💰 COMPARACIÓN DE PAGOS — ¿Cuánto debería pagar?");
        println!("  {}", "─".repeat(60));

        // Generar opciones: mínimo, empatar, sugerido, doble, triple, por meses
        let mut montos: Vec<(String, f64)> = Vec::new();

        montos.push(("⛔ Pago mínimo (trampa)".to_string(), pago_base));

        if es_predatoria {
            montos.push((
                "🛑 Para detener crecimiento".to_string(),
                pago_para_empatar.ceil(),
            ));
        }

        // Pago sugerido (+75% / doble)
        montos.push(("💰 SUGERIDO (×2 / +75%)".to_string(), pago_sugerido));

        // Calcular montos estratégicos
        let opciones_monto = [("Triple del mínimo", d.pago_minimo * 3.0)];
        for (nombre, monto) in &opciones_monto {
            if *monto > pago_sugerido + 10.0
                && !montos.iter().any(|(_, m)| (*m - *monto).abs() < 10.0)
            {
                montos.push((nombre.to_string(), *monto));
            }
        }

        // Pago para salir en X meses (búsqueda simple)
        for target_meses in [12u32, 24, 36, 48] {
            let pago_necesario = calcular_pago_para_meses(saldo, tasa_mensual, target_meses);
            if pago_necesario > d.pago_minimo
                && pago_necesario < saldo
                && !montos
                    .iter()
                    .any(|(_, m)| (*m - pago_necesario).abs() < 10.0)
            {
                montos.push((
                    format!("Liquidar en {} meses", target_meses),
                    pago_necesario,
                ));
            }
        }

        // Pago total (liquidar ya)
        montos.push(("Pago total (liquidar ya)".to_string(), saldo));

        // Ordenar por monto
        montos.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        // Tabla
        println!(
            "  ┌──────────────────────────────┬──────────┬────────┬──────────────┬──────────────┬─────────────┐"
        );
        println!(
            "  │ {:<28} │ {:>8} │ {:>6} │ {:>12} │ {:>12} │ {:>11} │",
            "Estrategia", "Pago/mes", "Meses", "Intereses", "Total pagado", "Costo extra"
        );
        println!(
            "  ├──────────────────────────────┼──────────┼────────┼──────────────┼──────────────┼─────────────┤"
        );

        let mut resultados: Vec<(String, f64, u32, f64, f64)> = Vec::new();
        for (nombre, monto) in &montos {
            let (meses, total_int, total_pag) = simular_pagos_simple(saldo, tasa_mensual, *monto);
            resultados.push((nombre.clone(), *monto, meses, total_int, total_pag));
        }

        let costo_minimo = resultados
            .last()
            .map(|(_, _, _, _, tp)| *tp)
            .unwrap_or(saldo);

        for (nombre, monto, meses, total_int, total_pag) in &resultados {
            let costo_extra = total_pag - costo_minimo;
            let meses_str = if *meses >= 600 {
                "∞".to_string()
            } else {
                format!("{}", meses)
            };

            let nombre_corto = if nombre.len() > 28 {
                format!("{}…", &nombre[..27])
            } else {
                nombre.clone()
            };

            // Indicador visual
            let indicador = if *meses >= 600 {
                " ⛔"
            } else if *meses > 60 {
                " ⚠️ "
            } else if *meses <= 24 {
                " ✅"
            } else {
                ""
            };

            println!(
                "  │ {:<28} │ {:>8} │ {:>5}{} │ {:>12} │ {:>12} │ {:>11} │",
                nombre_corto,
                format!("${:.0}", monto),
                meses_str,
                if indicador.is_empty() { " " } else { indicador },
                format!("${:.2}", total_int),
                format!("${:.2}", total_pag),
                if costo_extra > 0.5 {
                    format!("+${:.0}", costo_extra)
                } else {
                    "—".to_string()
                }
            );
        }

        println!(
            "  └──────────────────────────────┴──────────┴────────┴──────────────┴──────────────┴─────────────┘"
        );

        println!();
        println!("  💡 \"Costo extra\" = cuánto más pagas en total vs liquidar de inmediato.");
        println!("     Cada dólar en esa columna es dinero regalado al banco.");

        // ── Sección 3: Historial ──
        if !d.historial.is_empty() {
            println!();
            println!("  📅 HISTORIAL DE PAGOS REGISTRADOS");
            println!("  {}", "─".repeat(60));
            println!(
                "  {:<12} {:>12} {:>10} {:>10} {:>10} {:>12}",
                "Mes", "Saldo ini.", "Pago", "Interés", "Cargos", "Saldo fin."
            );
            println!("  {}", "─".repeat(68));
            for m in &d.historial {
                println!(
                    "  {:<12} {:>12} {:>10} {:>10} {:>10} {:>12}",
                    m.mes,
                    format!("${:.2}", m.saldo_inicio),
                    format!("${:.2}", m.pago),
                    format!("${:.2}", m.intereses),
                    format!("${:.2}", m.nuevos_cargos),
                    format!("${:.2}", m.saldo_final)
                );
            }
            println!("  {}", "─".repeat(68));
            let total_pagado: f64 = d.historial.iter().map(|m| m.pago).sum();
            let total_interes: f64 = d.historial.iter().map(|m| m.intereses).sum();
            println!(
                "  Total pagado: {}  |  Total en intereses: {}  |  Eficiencia: {:.0}%",
                format!("${:.2}", total_pagado).green(),
                format!("${:.2}", total_interes).red(),
                if total_pagado > 0.01 {
                    ((total_pagado - total_interes) / total_pagado) * 100.0
                } else {
                    0.0
                }
            );
        }

        // ── Sub-menú ──
        println!();
        let sub_opciones = &[
            "📊  Ver proyección mes a mes con un monto específico",
            "⭐  Ver proyección con el pago SUGERIDO",
            "🔙  Volver al listado de deudas",
        ];

        match menu("¿Qué deseas hacer?", sub_opciones) {
            Some(0) => {
                let monto = pedir_f64("Monto de pago mensual a proyectar ($)", pago_sugerido);
                let max_m = pedir_f64("¿Cuántos meses proyectar? (máx)", 60.0) as u32;
                mostrar_proyeccion_individual(d, monto, max_m);
            }
            Some(1) => {
                mostrar_proyeccion_individual(d, pago_sugerido, 60);
            }
            _ => return,
        }
    }
}

/// Calcula el pago mensual fijo necesario para liquidar una deuda en X meses.
pub fn calcular_pago_para_meses(saldo: f64, tasa_mensual: f64, meses: u32) -> f64 {
    if tasa_mensual < 0.0001 {
        return saldo / meses as f64;
    }
    // Fórmula de amortización: P = S * [r(1+r)^n] / [(1+r)^n - 1]
    let r = tasa_mensual;
    let n = meses as f64;
    let factor = r * (1.0 + r).powf(n);
    let denom = (1.0 + r).powf(n) - 1.0;
    if denom.abs() < 0.0001 {
        return saldo / meses as f64;
    }
    (saldo * factor / denom).ceil()
}

/// Simula pagos fijos mensuales y devuelve (meses, total_intereses, total_pagado).
pub fn simular_pagos_simple(saldo_inicial: f64, tasa_mensual: f64, monto: f64) -> (u32, f64, f64) {
    let mut saldo = saldo_inicial;
    let mut total_int = 0.0;
    let mut total_pag = 0.0;
    let mut meses = 0u32;

    while saldo > 0.01 && meses < 600 {
        let interes = saldo * tasa_mensual;
        total_int += interes;
        saldo += interes;
        let pago = monto.min(saldo);
        saldo -= pago;
        total_pag += pago;
        meses += 1;
    }
    (meses, total_int, total_pag)
}

/// Muestra proyección mes a mes para una deuda con un monto de pago dado.
pub fn mostrar_proyeccion_individual(
    d: &omniplanner::ml::advisor::DeudaRastreada,
    monto: f64,
    max_meses: u32,
) {
    let saldo_ini = d.saldo_actual();
    let tasa_mensual = d.tasa_anual / 100.0 / 12.0;
    let interes_mes1 = saldo_ini * tasa_mensual;

    limpiar();
    separador(&format!(
        "📊 PROYECCIÓN: {} — pagando ${:.2}/mes",
        d.nombre, monto
    ));

    if monto <= interes_mes1 && d.tasa_anual > 0.01 {
        println!();
        println!(
            "  ⛔ Con ${:.2}/mes NO cubres los intereses de ${:.2}/mes.",
            monto, interes_mes1
        );
        println!("  La deuda crecerá indefinidamente. Necesitas pagar más.");
        println!();
    }

    println!();
    println!(
        "  {:<5} {:>12} {:>10} {:>12} {:>12} {:>14}",
        "Mes", "Saldo", "Pago", "→ Interés", "→ Capital", "Int. acum."
    );
    println!("  {}", "─".repeat(70));

    let mut saldo = saldo_ini;
    let mut int_acum = 0.0;

    for mes in 1..=max_meses {
        if saldo < 0.01 {
            break;
        }
        let interes = saldo * tasa_mensual;
        int_acum += interes;
        saldo += interes;
        let pago = monto.min(saldo);
        let a_capital = pago - interes;
        saldo -= pago;
        if saldo < 0.01 {
            saldo = 0.0;
        }

        // Colorear: rojo si a_capital negativo, verde si positivo
        let capital_str = if a_capital < 0.0 {
            format!("-${:.2}", a_capital.abs()).red().to_string()
        } else {
            format!("${:.2}", a_capital).green().to_string()
        };

        println!(
            "  {:<5} {:>12} {:>10} {:>12} {:>12} {:>14}",
            mes,
            format!("${:.2}", saldo),
            format!("${:.2}", pago),
            format!("${:.2}", interes),
            capital_str,
            format!("${:.2}", int_acum)
        );

        if saldo < 0.01 {
            println!();
            println!(
                "  🎉 ¡Deuda liquidada en {} meses! Total intereses: ${:.2}",
                mes, int_acum
            );
            break;
        }
    }

    if saldo > 0.01 {
        println!("  {}", "─".repeat(70));
        println!(
            "  Después de {} meses: Saldo restante ${:.2}  |  Intereses pagados: ${:.2}",
            max_meses, saldo, int_acum
        );
    }

    println!();
    pausa();
}

/// Calcula mora e intereses acumulados por un período de no-pago,
/// y muestra cuánto debe pagar el usuario para reanudar normalmente.
pub fn rastreador_calcular_mora(state: &mut AppState) {
    limpiar();
    separador("⚠️  PERÍODO DE ATRASO / MORA");

    if state.asesor.rastreador.deudas.is_empty() {
        println!("  No hay deudas registradas.");
        pausa();
        return;
    }

    // ── Seleccionar deuda ─────────────────────────────────────
    let etiquetas: Vec<String> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .map(|d| {
            let saldo = d.saldo_actual();
            format!(
                "{} — saldo: ${:.2}  tasa: {:.1}%  mín: ${:.2}",
                d.nombre, saldo, d.tasa_anual, d.pago_minimo
            )
        })
        .collect();
    let refs: Vec<&str> = etiquetas.iter().map(String::as_str).collect();
    let idx = match menu("Selecciona la deuda en atraso:", &refs) {
        Some(i) => i,
        None => return,
    };
    let nombre_deuda = state.asesor.rastreador.deudas[idx].nombre.clone();

    // ── Período de no-pago ────────────────────────────────────
    println!();
    println!("  Ingresa el rango de meses que NO se pagaron.");
    let hoy = chrono::Local::now();
    let default_ini = format!("{}-01", chrono::Local::now().format("%Y-%m"));
    let _ = default_ini;
    let default_ini = format!(
        "{:04}-{:02}",
        hoy.year(),
        hoy.month().saturating_sub(2).max(1)
    );
    let default_fin = format!("{:04}-{:02}", hoy.year(), hoy.month());

    let mes_ini = pedir_mes_flexible(
        "Primer mes sin pagar (YYYY-MM o 'mayo 2026'):",
        &default_ini,
        true,
    );
    if mes_ini.is_empty() {
        return;
    }
    let mes_fin = pedir_mes_flexible(
        "Último mes sin pagar (YYYY-MM o 'julio 2026'):",
        &default_fin,
        true,
    );
    if mes_fin.is_empty() {
        return;
    }

    // ── Tasa de mora ──────────────────────────────────────────
    println!();
    println!("  La mora se calcula como % mensual sobre el pago mínimo vencido.");
    println!("  (Típico: 5%–15% anual. Revisa tu contrato.)");
    let tasa_mora = pedir_f64("Tasa de mora anual % (Enter = 12.0):", 12.0);

    // ── Calcular ──────────────────────────────────────────────
    let resumen =
        state.asesor.rastreador.deudas[idx].calcular_atraso(&mes_ini, &mes_fin, tasa_mora);

    if resumen.meses_sin_pagar == 0 {
        println!("  No se encontraron meses en ese rango.");
        pausa();
        return;
    }

    // ── Mostrar resultado ─────────────────────────────────────
    limpiar();
    separador("📊 RESUMEN DE MORA");
    println!("  Deuda:   {}", nombre_deuda.cyan().bold());
    println!(
        "  Período: {} → {}  ({} meses sin pagar)",
        mes_ini.cyan(),
        mes_fin.cyan(),
        resumen.meses_sin_pagar
    );
    println!(
        "  Saldo al inicio del atraso: {}",
        format!("${:.2}", resumen.saldo_al_inicio).yellow()
    );
    println!(
        "  Saldo al cierre del atraso: {}  (creció por intereses capitalizados)",
        format!("${:.2}", resumen.saldo_al_final).red().bold()
    );
    println!();
    println!("  ── Desglose mes a mes ───────────────────────────────");
    println!(
        "  {:<9}  {:>12}  {:>10}  {:>8}  {:>12}",
        "Mes", "Saldo inicio", "Intereses", "Mora", "Saldo fin"
    );
    println!("  {}", "─".repeat(60));
    for det in &resumen.detalle {
        println!(
            "  {:<9}  {:>12}  {:>10}  {:>8}  {:>12}",
            det.mes,
            format!("${:.2}", det.saldo_inicio),
            format!("${:.2}", det.intereses).yellow().to_string(),
            format!("${:.2}", det.mora).red().to_string(),
            format!("${:.2}", det.saldo_fin)
        );
    }
    println!("  {}", "─".repeat(60));
    println!(
        "  {:>40}  {:>10}  {:>8}",
        "TOTALES:",
        format!("${:.2}", resumen.total_intereses_extra)
            .yellow()
            .to_string(),
        format!("${:.2}", resumen.total_mora).red().to_string()
    );
    println!();
    println!("  ── Lo que debes pagar para reanudar ─────────────────");
    println!(
        "    Mínimos acumulados ({} × mín):  {}",
        resumen.meses_sin_pagar,
        format!("${:.2}", resumen.total_minimos_acumulados).yellow()
    );
    println!(
        "    Mora acumulada:                {}",
        format!("${:.2}", resumen.total_mora).red()
    );
    let pago_mes_actual = state.asesor.rastreador.deudas[idx]
        .pago_minimo
        .max(state.asesor.rastreador.deudas[idx].principal_interes_mensual);
    println!(
        "    Cuota regular del mes actual:  {}",
        format!("${:.2}", pago_mes_actual).green()
    );
    println!("    {}", "─".repeat(46));
    println!(
        "    TOTAL A PAGAR PARA REANUDAR:   {}",
        format!("${:.2}", resumen.pago_para_reanudar).green().bold()
    );
    println!();
    println!(
        "  ⚠️  El saldo de la deuda quedará en {} después del pago de reanudación.",
        format!("${:.2}", resumen.saldo_al_final).red().bold()
    );
    println!("     Ese saldo continuará amortizándose con los pagos regulares siguientes.");
    println!();

    // ── Opción: crear pago programado de reanudación ──────────
    let opciones_post = &[
        "Programar el pago de reanudación como pago futuro",
        "Solo consultar (no crear nada)",
    ];
    if let Some(0) = menu("¿Qué deseas hacer con este cálculo?", opciones_post) {
        let mes_reanudar = {
            // Mes siguiente al fin del atraso
            let mut it = mes_fin.splitn(2, '-');
            let y: i32 = it.next().unwrap_or("2026").parse().unwrap_or(2026);
            let m: i32 = it.next().unwrap_or("01").parse().unwrap_or(1);
            let total = y * 12 + m; // mes_fin en absoluto, +1
            let ny = total / 12;
            let nm = total % 12 + 1;
            format!("{:04}-{:02}", ny, nm)
        };
        let nota_reanudacion = format!(
            "Reanudación tras atraso {}-{}: mín×{}+mora",
            mes_ini, mes_fin, resumen.meses_sin_pagar
        );
        state
            .asesor
            .rastreador
            .pagos_programados
            .push(omniplanner::ml::PagoProgramado {
                nombre_deuda: nombre_deuda.clone(),
                monto_pi: resumen.pago_para_reanudar,
                monto_escrow: 0.0,
                meses_cubiertos: resumen.detalle.iter().map(|d| d.mes.clone()).collect(),
                fecha_pago_prevista: mes_reanudar.clone(),
                nota: nota_reanudacion,
            });
        // Emitir evento
        {
            use omniplanner::eventos::{
                EstadoEvento, EventoSistema, Modulo, Referencia, TipoEvento,
            };
            let fecha_evt =
                chrono::NaiveDate::parse_from_str(&format!("{}-01", mes_reanudar), "%Y-%m-%d")
                    .unwrap_or_else(|_| chrono::Local::now().date_naive());
            let ev = EventoSistema::nuevo(
                Modulo::Rastreador,
                TipoEvento::PagoProgramado,
                format!("Reanudación mora: {} ({})", nombre_deuda, mes_reanudar),
            )
            .con_fecha(fecha_evt)
            .con_monto(resumen.pago_para_reanudar)
            .con_contraparte(nombre_deuda.clone())
            .con_estado(EstadoEvento::Pendiente)
            .con_referencia(Referencia::nueva(
                "rastreador",
                "deuda",
                &nombre_deuda,
                &nombre_deuda,
            ))
            .con_etiqueta("mora")
            .con_nota(format!(
                "Incluye {} meses de atraso + mora ${:.2}",
                resumen.meses_sin_pagar, resumen.total_mora
            ));
            state.bus.emitir(ev);
        }
        let _ = state.guardar();
        println!(
            "  {} Pago de reanudación programado para {}  (${:.2}).",
            "✅".green(),
            mes_reanudar.cyan().bold(),
            resumen.pago_para_reanudar
        );
    }
    pausa();
}

pub fn rastreador_agregar_deuda(state: &mut AppState) {
    limpiar();
    separador("➕ AGREGAR DEUDA AL RASTREADOR");

    let nombre = match pedir_texto("Nombre de la cuenta (ej: Discover, BOFA, Renta, Seguro)") {
        Some(n) => n,
        None => return,
    };

    // Preguntar tipo PRIMERO — el flujo cambia según la respuesta
    let tipos_deuda = &[
        "💳  Tarjeta de crédito / línea de crédito",
        "🏠  Préstamo con interés compuesto (mortgage, carro, préstamo personal)",
        "🔒  Pago corriente / fijo (renta, seguro, suscripción — sin intereses, se paga completo)",
    ];
    let tipo = match menu("Tipo de deuda", tipos_deuda) {
        Some(t) => t,
        _ => return,
    };

    let (saldo, tasa, pago_min, es_obligatoria, escrow_mensual, pago_pi_configurado);
    let mut inicia_con_primera_cuota = false;
    let mut frecuencia_corriente = FrecuenciaPago::Mensual;

    match tipo {
        2 => {
            // Pago corriente: renta, seguro, suscripción — tasa 0, pago = monto completo
            es_obligatoria = true;
            tasa = 0.0;

            let freq_opciones = &["Mensual", "Trimestral", "Semestral", "Anual"];
            let freq_idx = match menu("Frecuencia del pago", freq_opciones) {
                Some(i) => i,
                None => return,
            };
            frecuencia_corriente = match freq_idx {
                0 => FrecuenciaPago::Mensual,
                1 => FrecuenciaPago::Trimestral,
                2 => FrecuenciaPago::Semestral,
                _ => FrecuenciaPago::Anual,
            };
            let label_monto = match freq_idx {
                0 => "Monto mensual fijo ($)",
                1 => "Monto trimestral ($)",
                2 => "Monto semestral ($)",
                _ => "Monto anual ($)",
            };
            pago_min = pedir_f64(label_monto, 0.0);
            saldo = pago_min;
            escrow_mensual = 0.0;
            pago_pi_configurado = 0.0;

            let equiv_mensual = frecuencia_corriente.a_mensual(pago_min);
            println!();
            if matches!(frecuencia_corriente, FrecuenciaPago::Mensual) {
                println!(
                    "    🔒 Pago corriente: ${:.2}/mes — sin intereses, se paga en su totalidad.",
                    pago_min
                );
            } else {
                println!(
                    "    🔒 Pago corriente {}: ${:.2} (~${:.2}/mes equivalente) — sin intereses.",
                    frecuencia_corriente.nombre(),
                    pago_min,
                    equiv_mensual
                );
            }
        }
        1 => {
            // Préstamo con interés compuesto — obligatoria, con tasa fija
            es_obligatoria = true;
            saldo = pedir_f64("Saldo actual del préstamo ($)", 0.0);
            tasa = pedir_f64("Tasa de interés ANUAL fija (%) (ej: 6.5)", 0.0);
            let separar_componentes = TermConfirm::new()
                .with_prompt("  ¿Deseas separar P&I y Escrow para este pago mensual?")
                .default(true)
                .interact()
                .unwrap_or(true);
            if separar_componentes {
                let pago_pi = pedir_f64("Pago mensual P&I ($)", 0.0);
                let escrow = pedir_f64(
                    "Pago mensual Escrow ($ - hazard insurance / impuestos)",
                    0.0,
                );
                pago_min = pago_pi;
                pago_pi_configurado = pago_pi;
                escrow_mensual = escrow.max(0.0);
                println!(
                    "    🧾 Configurado: P&I ${:.2} + Escrow ${:.2} = Total ${:.2}/mes",
                    pago_pi,
                    escrow_mensual,
                    pago_pi + escrow_mensual
                );
            } else {
                pago_min = pedir_f64("Pago mensual aplicado a deuda (P&I) ($)", 0.0);
                pago_pi_configurado = pago_min;
                escrow_mensual = 0.0;
            }
            inicia_con_primera_cuota = TermConfirm::new()
                .with_prompt(
                    "  ¿Esta deuda todavía no se ha generado y solo existirá tras la primera cuota?",
                )
                .default(false)
                .interact()
                .unwrap_or(false);
            if inicia_con_primera_cuota {
                println!("    ⏳ Esta deuda quedará pendiente hasta registrar la primera cuota.");
            }

            println!(
                "    🔒 Préstamo fijo al {:.1}% — interés compuesto, no varía.",
                tasa
            );
        }
        _ => {
            // Tarjeta de crédito — no obligatoria, con tasa y pago mínimo
            es_obligatoria = false;
            saldo = pedir_f64("Saldo actual ($)", 0.0);
            tasa = pedir_f64("Tasa de interés ANUAL (%) (ej: 24.99)", 0.0);
            pago_min = pedir_f64("Pago mínimo mensual ($)", 25.0);
            escrow_mensual = 0.0;
            pago_pi_configurado = pago_min;
        }
    }

    let mut deuda = DeudaRastreada::nueva(&nombre, tasa, pago_min);
    deuda.obligatoria = es_obligatoria;
    deuda.escrow_mensual = escrow_mensual;
    deuda.principal_interes_mensual = if pago_pi_configurado > 0.01 {
        pago_pi_configurado
    } else {
        pago_min
    };
    // Aplicar frecuencia para pagos corrientes no-mensuales (anual, semestral, etc.)
    if tipo == 2 {
        deuda.frecuencia = frecuencia_corriente.clone();
    }

    // Enganche (solo para deudas con saldo, no para pagos corrientes)
    let enganche = if tipo != 2 && saldo > 0.0 {
        let tiene_enganche = TermConfirm::new()
            .with_prompt("  ¿Hubo un enganche o pago inicial único?")
            .default(false)
            .interact()
            .unwrap_or(false);
        if tiene_enganche {
            let eng = pedir_f64("  Monto del enganche/pago inicial ($)", 0.0);
            if eng > 0.0 && eng < saldo {
                println!(
                    "    💰 Enganche de ${:.2} — saldo pendiente: ${:.2}",
                    eng,
                    saldo - eng
                );
            }
            eng
        } else {
            0.0
        }
    } else {
        0.0
    };
    deuda.enganche = enganche;
    let saldo_efectivo = (saldo - enganche).max(0.0);
    // Persistimos el principal declarado para que `saldo_actual()` lo refleje
    // aun cuando el historial esté vacío (caso típico: mortgage pendiente de originarse).
    deuda.saldo_inicial = saldo_efectivo;
    if tipo != 2 {
        if tipo == 1 && inicia_con_primera_cuota {
            deuda.activa = false;
            deuda.originada = false;
            println!();
            println!(
                "  ⏳ '{}' quedará pendiente: la deuda se originará cuando registres la primera cuota.",
                nombre
            );
        } else {
            let cargar_hist = TermConfirm::new()
                .with_prompt("  ¿Quieres cargar meses anteriores de pago?")
                .default(false)
                .interact()
                .unwrap_or(false);

            if cargar_hist {
                println!();
                println!("  📅 Ingresa los datos mes por mes (vacío para terminar).");
                let mut saldo_actual = saldo_efectivo;

                loop {
                    let mes = pedir_texto_opcional(&format!(
                        "Mes {} (ej: Ene 2021, vacío=terminar)",
                        deuda.historial.len() + 1
                    ));
                    if mes.is_empty() {
                        break;
                    }

                    let saldo_inicio = pedir_f64(
                        &format!("  Saldo al inicio del mes (${:.2} sugerido)", saldo_actual),
                        saldo_actual,
                    );
                    let pago = if deuda.tiene_escrow_configurado() {
                        pedir_f64(
                            &format!(
                                "  Pago P&I realizado (${:.2} sugerido)",
                                deuda.pago_pi_mensual()
                            ),
                            deuda.pago_pi_mensual(),
                        )
                    } else {
                        pedir_f64("  Pago realizado ($)", 0.0)
                    };
                    let pago_escrow = if deuda.tiene_escrow_configurado() {
                        pedir_f64(
                            &format!(
                                "  Pago Escrow realizado (${:.2} sugerido)",
                                deuda.escrow_mensual
                            ),
                            deuda.escrow_mensual,
                        )
                    } else {
                        0.0
                    };
                    let cargos = pedir_f64("  Nuevos cargos/compras ($)", 0.0);

                    deuda.registrar_mes_con_escrow(&mes, saldo_inicio, pago, pago_escrow, cargos);
                    saldo_actual = deuda.saldo_actual();

                    println!(
                        "    {} {} — Saldo final: ${:.2}",
                        "✓".green(),
                        mes,
                        saldo_actual
                    );
                }
            } else {
                let hoy = Local::now().format("%b %Y").to_string();
                deuda.registrar_mes(&hoy, saldo_efectivo, 0.0, 0.0);
            }
        }
    } else {
        // Pago corriente: registrar un mes con su monto como saldo
        let hoy = Local::now().format("%b %Y").to_string();
        deuda.registrar_mes(&hoy, saldo_efectivo, 0.0, 0.0);
    }

    println!();
    let sufijo = if tipo == 2 {
        if matches!(frecuencia_corriente, FrecuenciaPago::Mensual) {
            "/mes (pago corriente)".to_string()
        } else {
            format!(
                "/{} (~${:.2}/mes equivalente)",
                frecuencia_corriente.nombre(),
                frecuencia_corriente.a_mensual(deuda.pago_minimo)
            )
        }
    } else {
        String::new()
    };
    if tipo == 1 && inicia_con_primera_cuota {
        println!(
            "  {} '{}' agregada — pendiente de originarse hasta la primera cuota",
            "✓".green(),
            nombre
        );
    } else {
        println!(
            "  {} '{}' agregada — ${:.2}{}",
            "✓".green(),
            nombre,
            deuda.saldo_actual(),
            sufijo
        );
    }

    // Día de corte (1-31): opcional pero útil para auto-rellenar el mes al
    // registrar pagos y para alertas de cortes próximos.
    if !deuda.es_pago_corriente() || matches!(deuda.frecuencia, FrecuenciaPago::Mensual) {
        println!();
        println!("  📅 Día del mes en que vence/cierra el corte (1-31, 0 = sin definir)");
        let dia = pedir_f64("Día de corte", 0.0) as i64;
        if (1..=31).contains(&dia) {
            deuda.dia_corte = Some(dia as u32);
        }
    }

    state.asesor.rastreador.agregar_deuda(deuda);
    pausa();
}

pub fn rastreador_diagnostico(state: &mut AppState) {
    if state.asesor.rastreador.deudas.is_empty() {
        println!("  Sin deudas registradas.");
        pausa();
        return;
    }

    limpiar();
    let diag = state.asesor.rastreador.diagnosticar();
    let mes_hoy = chrono::Local::now().format("%Y-%m").to_string();

    separador("📊 DIAGNÓSTICO DE DEUDAS — VISTA COMPLETA");

    // ── RESUMEN FINANCIERO ───────────────────────────────────────────────────
    println!();
    let cambio_str = if diag.cambio_neto > 0.0 {
        format!("+${:.2}", diag.cambio_neto)
            .red()
            .bold()
            .to_string()
    } else if diag.cambio_neto < 0.0 {
        format!("-${:.2}", diag.cambio_neto.abs())
            .green()
            .bold()
            .to_string()
    } else {
        "Sin cambio".dimmed().to_string()
    };

    println!(
        "  {}  {}  {}  {}  {}  {}",
        format!("📅 {} meses", diag.meses_analizados).cyan(),
        format!(
            "💰 Deuda: ${:.0} → ${:.0}",
            diag.deuda_inicial_total, diag.deuda_final_total
        ),
        format!("Δ {}", cambio_str),
        format!("✅ Pagado: ${:.0}", diag.total_pagado).green(),
        format!("💸 Intereses: ${:.0}", diag.total_intereses_estimados).yellow(),
        format!("🆕 Nuevos cargos: ${:.0}", diag.total_nuevos_cargos),
    );
    println!();

    // ── TABLA PRINCIPAL ──────────────────────────────────────────────────────
    println!(
        "  {:<24} {:>8} {:>8} {:>9} {:>9} {:>10}  {}",
        "Cuenta".bold(),
        "Tasa%".bold(),
        "Saldo".bold(),
        "Plan/mes".bold(),
        "Ult.pago".bold(),
        "Pagado".bold(),
        "Estado".bold()
    );
    println!("  {}", "─".repeat(92));

    let mut deudas_sin_pago_mes: Vec<String> = Vec::new();

    for d in &state.asesor.rastreador.deudas {
        if !d.activa {
            continue;
        }

        let r = diag.resumen_por_deuda.iter().find(|r| r.nombre == d.nombre);
        let saldo_actual = d.saldo_actual();
        let pago_plan = d.pago_pi_mensual();

        // Último pago registrado
        let (ult_mes, ult_monto) = d
            .historial
            .iter()
            .filter(|m| m.pago > 0.01)
            .last()
            .map(|m| (m.mes.as_str(), m.pago))
            .unwrap_or(("—", 0.0));

        // Pago en el mes actual
        let pago_este_mes = d
            .historial
            .iter()
            .find(|m| m.mes == mes_hoy)
            .map(|m| m.pago)
            .unwrap_or(0.0);

        // Total pagado (del resumen)
        let total_pagado = r.map(|r| r.total_pagado).unwrap_or(0.0);

        // Determinar estado visual
        let es_corriente = d.tasa_anual < 0.01 && d.obligatoria;
        let estado = if !matches!(
            d.frecuencia,
            FrecuenciaPago::Mensual | FrecuenciaPago::Quincenal | FrecuenciaPago::Semanal
        ) {
            // No mensual: OK si tiene algún pago en el ciclo
            let tiene_pago_ciclo = !d
                .historial
                .iter()
                .filter(|m| m.pago > 0.01)
                .last()
                .is_none();
            if tiene_pago_ciclo {
                "✅ Al día".green().to_string()
            } else {
                "⚠️  Sin pago".yellow().to_string()
            }
        } else if pago_este_mes > 0.01 {
            if pago_este_mes >= pago_plan * 0.95 {
                "✅ Pagado".green().to_string()
            } else {
                format!("⚡ Parcial ${:.0}", pago_este_mes)
                    .yellow()
                    .to_string()
            }
        } else if es_corriente {
            deudas_sin_pago_mes.push(d.nombre.clone());
            "⏳ Pendiente".yellow().to_string()
        } else {
            deudas_sin_pago_mes.push(d.nombre.clone());
            "🔴 Sin pago".red().to_string()
        };

        let frec_label = match d.frecuencia {
            FrecuenciaPago::Mensual => "",
            FrecuenciaPago::Semanal => "/sem",
            FrecuenciaPago::Quincenal => "/qna",
            FrecuenciaPago::Trimestral => "/trim",
            FrecuenciaPago::Semestral => "/sem",
            FrecuenciaPago::Anual => "/año",
            FrecuenciaPago::UnaVez => "/vez",
        };

        let nombre_display = if d.nombre.len() > 23 {
            format!("{}…", &d.nombre[..22])
        } else {
            d.nombre.clone()
        };

        let saldo_str = if es_corriente {
            "corriente".dimmed().to_string()
        } else {
            format!("${:.0}", saldo_actual)
        };

        println!(
            "  {:<24} {:>7.1}% {:>8} {:>8}{} {:>9} {:>10}  {}",
            nombre_display,
            d.tasa_anual,
            saldo_str,
            format!("${:.0}", pago_plan),
            frec_label,
            if ult_monto > 0.01 {
                format!("{} ${:.0}", ult_mes, ult_monto)
            } else {
                "—".to_string()
            },
            format!("${:.0}", total_pagado),
            estado
        );
    }
    println!("  {}", "─".repeat(92));

    // Total
    let total_saldo: f64 = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .filter(|d| d.activa && d.tasa_anual > 0.01)
        .map(|d| d.saldo_actual())
        .sum();
    let total_plan: f64 = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .filter(|d| d.activa)
        .map(|d| d.pago_pi_mensual())
        .sum();
    println!(
        "  {:<24} {:>8} {:>8} {:>9}",
        "TOTAL activas".bold(),
        "",
        format!("${:.0}", total_saldo).yellow().bold(),
        format!("${:.0}/mes", total_plan).cyan().bold(),
    );
    println!();

    // ── ALERTAS ──────────────────────────────────────────────────────────────
    let errores_graves: Vec<_> = diag
        .errores
        .iter()
        .filter(|e| {
            matches!(
                e.error,
                omniplanner::ml::advisor::ErrorPago::NoPagoNada
                    | omniplanner::ml::advisor::ErrorPago::SiguioUsandoTarjeta
                    | omniplanner::ml::advisor::ErrorPago::PagoInsuficiente
            )
        })
        // Solo errores recientes (últimos 3 meses por deuda)
        .collect();

    // Agrupar errores por deuda mostrando solo el más reciente
    let mut deudas_con_error: std::collections::HashMap<
        &str,
        &omniplanner::ml::advisor::DiagnosticoMes,
    > = std::collections::HashMap::new();
    for e in &errores_graves {
        deudas_con_error
            .entry(e.deuda.as_str())
            .and_modify(|prev| {
                if e.mes > prev.mes {
                    *prev = e;
                }
            })
            .or_insert(e);
    }

    if !deudas_con_error.is_empty() {
        println!("  {} ALERTAS ({}):", "⚠️".yellow(), deudas_con_error.len());
        println!();
        let mut alertas: Vec<_> = deudas_con_error.values().collect();
        alertas.sort_by(|a, b| b.mes.cmp(&a.mes));
        for e in alertas {
            println!(
                "    {} {} — {} ({})",
                e.error.emoji(),
                e.deuda.bold(),
                e.nota,
                e.mes.dimmed()
            );
        }
        println!();
    }

    // ── DEUDAS SIN PAGO ESTE MES ─────────────────────────────────────────────
    let pendientes_mes: Vec<_> = deudas_sin_pago_mes
        .iter()
        .filter(|n| {
            state
                .asesor
                .rastreador
                .deudas
                .iter()
                .find(|d| d.nombre == **n)
                .map(|d| {
                    matches!(
                        d.frecuencia,
                        FrecuenciaPago::Mensual
                            | FrecuenciaPago::Quincenal
                            | FrecuenciaPago::Semanal
                    )
                })
                .unwrap_or(false)
        })
        .collect();

    if !pendientes_mes.is_empty() {
        println!(
            "  {} Sin pago registrado este mes ({}):",
            "📅".cyan(),
            mes_hoy
        );
        for n in &pendientes_mes {
            let d = state
                .asesor
                .rastreador
                .deudas
                .iter()
                .find(|d| d.nombre == **n)
                .unwrap();
            println!("    • {} — Plan: ${:.2}/mes", n.bold(), d.pago_pi_mensual());
        }
        println!(
            "  {} Para registrarlos usa: Registrar mes de pago",
            "ℹ️".cyan()
        );
        println!();
    }

    // ── RECOMENDACIONES CLAVE ─────────────────────────────────────────────────
    // Solo las más importantes, sin repetir
    let mut recs_mostradas = 0;
    let recs_filtradas: Vec<_> = diag
        .recomendaciones
        .iter()
        .filter(|r| !r.contains("Orden de pago") && !r.starts_with("   "))
        .take(5)
        .collect();

    if !recs_filtradas.is_empty() {
        println!("  {} RECOMENDACIONES:", "💡".yellow());
        println!();
        for rec in recs_filtradas {
            println!("    {}", rec);
            recs_mostradas += 1;
        }
        println!();
    }

    // Orden de avalancha compacto
    let activas_con_tasa: Vec<_> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .filter(|d| d.activa && d.tasa_anual > 0.01 && d.saldo_actual() > 0.01)
        .collect();
    if activas_con_tasa.len() > 1 {
        let mut orden: Vec<_> = activas_con_tasa.iter().collect();
        orden.sort_by(|a, b| b.tasa_anual.partial_cmp(&a.tasa_anual).unwrap());
        println!(
            "  {} ORDEN AVALANCHA (ataca la tasa más alta primero):",
            "🎯".cyan()
        );
        for (i, d) in orden.iter().enumerate() {
            let saldo = d.saldo_actual();
            println!(
                "    {}. {:<26} {:.1}%  ${:.0}",
                i + 1,
                d.nombre,
                d.tasa_anual,
                saldo
            );
        }
        println!();
    }

    let _ = recs_mostradas; // suprimir warning

    // ── Menú de acción rápida ────────────────────────────────────────────────
    println!();
    let acciones = &[
        "📝  Editar un pago en el Rastreador (sincroniza al Presupuesto)",
        "📅  Registrar pago de este mes (sincroniza al Presupuesto)",
        "🔙  Volver",
    ];
    match menu("¿Qué hacer?", acciones) {
        Some(0) => rastreador_editar_pago(state),
        Some(1) => rastreador_registrar_mes(state),
        _ => {}
    }
}

pub fn rastreador_simulacion(state: &AppState) {
    if state.asesor.rastreador.deudas.is_empty() {
        println!("  Sin deudas registradas.");
        pausa();
        return;
    }

    let nombres: Vec<String> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .map(|d| format!("{} — ${:.2}", d.nombre, d.saldo_actual()))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    if let Some(idx) = menu("¿Simular cuál deuda?", &refs) {
        let d = &state.asesor.rastreador.deudas[idx];
        if d.historial.is_empty() {
            println!("  Esta deuda no tiene historial aún.");
            pausa();
            return;
        }

        limpiar();
        separador(&format!("📈 SIMULACIÓN: {}", d.nombre));

        println!("  🔄 Real vs Alternativa");
        println!();

        let pago_alt = pedir_f64(
            "¿Cuánto hubieras querido pagar por mes? ($)",
            d.pago_minimo * 2.0,
        );

        let alt = d.simular_alternativa(pago_alt);

        // Mostrar tabla comparativa
        println!();
        println!(
            "  {:<10} {:>12} {:>10} {:>12} {:>10}",
            "Mes", "Real", "Pago.R", "Alternativa", "Pago.A"
        );
        println!("  {}", "─".repeat(60));

        let max_filas = d.historial.len().max(alt.len());
        for i in 0..max_filas {
            let real = d.historial.get(i);
            let sim = alt.get(i);
            println!(
                "  {:<10} {:>12} {:>10} {:>12} {:>10}",
                real.map(|m| m.mes.as_str()).unwrap_or("-"),
                real.map(|m| format!("${:.2}", m.saldo_final))
                    .unwrap_or_default(),
                real.map(|m| format!("${:.2}", m.pago)).unwrap_or_default(),
                sim.map(|m| format!("${:.2}", m.saldo_final))
                    .unwrap_or_default(),
                sim.map(|m| format!("${:.2}", m.pago)).unwrap_or_default(),
            );
        }
        println!("  {}", "─".repeat(60));

        let real_final = d.historial.last().map(|m| m.saldo_final).unwrap_or(0.0);
        let alt_final = alt.last().map(|m| m.saldo_final).unwrap_or(0.0);
        let real_pagado: f64 = d.historial.iter().map(|m| m.pago).sum();
        let alt_pagado: f64 = alt.iter().map(|m| m.pago).sum();

        println!();
        println!(
            "  Saldo final REAL:        {}",
            format!("${:.2}", real_final).red()
        );
        println!(
            "  Saldo final ALTERNATIVO: {}",
            if alt_final < real_final {
                format!("${:.2}", alt_final).green().to_string()
            } else {
                format!("${:.2}", alt_final).red().to_string()
            }
        );
        println!(
            "  Diferencia:              {}",
            format!("${:.2} menos", (real_final - alt_final).max(0.0)).green()
        );
        println!();
        println!(
            "  Total pagado REAL: ${:.2}  |  ALTERNATIVO: ${:.2}",
            real_pagado, alt_pagado
        );

        pausa();
    }
}

pub fn rastreador_simulacion_libertad(state: &mut AppState) {
    let deudas_reales: Vec<&DeudaRastreada> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .filter(|d| d.activa && !d.es_pago_corriente() && d.saldo_actual() > 0.01)
        .collect();

    let pagos_corrientes: Vec<&DeudaRastreada> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .filter(|d| d.activa && d.es_pago_corriente())
        .collect();

    if deudas_reales.is_empty() {
        println!("  No hay deudas activas (con saldo) para simular.");
        if !pagos_corrientes.is_empty() {
            println!(
                "  (Tienes {} pago(s) corriente(s) pero esos no se liquidan.)",
                pagos_corrientes.len()
            );
        }
        pausa();
        return;
    }

    limpiar();
    separador("🗺️  SIMULACIÓN: CAMINO A LA LIBERTAD FINANCIERA");

    let deuda_total: f64 = deudas_reales.iter().map(|d| d.saldo_actual()).sum();
    let ingreso_mensual = state.asesor.rastreador.ingreso_mensual_total();
    let minimos_deudas: f64 = deudas_reales.iter().map(|d| d.pago_minimo).sum();
    let total_corrientes: f64 = pagos_corrientes.iter().map(|d| d.pago_minimo).sum();

    // Mostrar pagos corrientes (gastos fijos)
    if !pagos_corrientes.is_empty() {
        println!();
        println!("  🔒 Pagos corrientes (se restan del presupuesto):");
        for d in &pagos_corrientes {
            println!(
                "     • {} — {}/mes",
                d.nombre,
                format!("${:.2}", d.pago_minimo).yellow()
            );
        }
        println!(
            "     Total gastos fijos: {}",
            format!("${:.2}", total_corrientes).yellow()
        );
    }

    // Mostrar deudas reales
    println!();
    println!("  📋 Deudas a liquidar: {}", deudas_reales.len());
    for d in &deudas_reales {
        let tag = if d.obligatoria { " 🔒" } else { "" };
        let pago_str = if d.tiene_escrow_configurado() {
            format!(
                "${:.2} (P&I ${:.2} + Escrow ${:.2})",
                d.pago_total_mensual(),
                d.pago_pi_mensual(),
                d.escrow_mensual
            )
        } else {
            format!("${:.2}", d.pago_pi_mensual())
        };
        println!(
            "     • {} — Saldo: {} | Pago: {} | Tasa: {:.1}%{}",
            d.nombre,
            format!("${:.2}", d.saldo_actual()).red(),
            pago_str,
            d.tasa_anual,
            tag,
        );
    }
    println!();
    println!(
        "  Deuda total:         {}",
        format!("${:.2}", deuda_total).red()
    );
    println!(
        "  Ingreso mensual:     {}",
        format!("${:.2}", ingreso_mensual).green()
    );
    if total_corrientes > 0.0 {
        println!(
            "  Gastos fijos:       -{}",
            format!("${:.2}", total_corrientes).yellow()
        );
        println!(
            "  Disponible p/deudas: {}",
            format!("${:.2}", (ingreso_mensual - total_corrientes).max(0.0)).cyan()
        );
    }
    println!(
        "  Pago mínimo deudas:  {}",
        format!("${:.2}", minimos_deudas).yellow()
    );
    println!();

    // Elegir estrategia
    let estrategias = &[
        "❄️  Avalancha (paga primero la tasa más alta — ahorra más en intereses)",
        "⛄ Bola de nieve (paga primero el saldo más bajo — victorias rápidas)",
    ];
    let bola_nieve = match menu("¿Qué estrategia usar?", estrategias) {
        Some(1) => true,
        Some(0) => false,
        _ => return,
    };

    // Monto mensual (incluye gastos fijos + deudas)
    let minimo_necesario = minimos_deudas + total_corrientes;
    let sugerido = if ingreso_mensual > minimo_necesario * 1.5 {
        minimo_necesario * 1.5
    } else {
        minimo_necesario
    };
    let presupuesto = pedir_f64(
        "¿Cuánto puedes destinar al mes en TOTAL? (deudas + gastos fijos) ($)",
        sugerido,
    );

    let politica = PoliticaFlujo::camino_libertad();
    let dist_plan = calcular_distribucion_flujo(
        presupuesto,
        minimos_deudas,
        total_corrientes,
        true,
        &politica,
    );
    let presupuesto_comprometido = dist_plan.comprometido_objetivo;

    if presupuesto < minimo_necesario {
        println!();
        println!(
            "  ⚠️ El presupuesto (${:.2}) es menor que lo necesario (${:.2} fijos + ${:.2} mínimos).",
            presupuesto,
            total_corrientes,
            minimos_deudas
        );
        println!("  No se podrán cubrir todos los pagos.");
        println!();
    }

    println!(
        "  Política: {} comprometido al plan. Comprometido: {} | Jugable: {}",
        format!("{:.0}%", dist_plan.ratio_comprometido_aplicado * 100.0)
            .yellow()
            .bold(),
        format!("${:.2}", presupuesto_comprometido).yellow().bold(),
        format!("${:.2}", dist_plan.flujo_jugable).green()
    );
    println!(
        "    Nivel endeudamiento actual: {:.0}% (umbral {:.0}%)",
        dist_plan.nivel_endeudamiento * 100.0,
        politica.umbral_endeudamiento * 100.0
    );
    println!(
        "    Reparto jugable → Variable {} | Ahorro {} | Colocación {}",
        format!("${:.2}", dist_plan.bolsa_variable).cyan(),
        format!("${:.2}", dist_plan.bolsa_ahorro).green(),
        format!("${:.2}", dist_plan.bolsa_colocacion).yellow()
    );
    println!();

    // Informar al usuario qué pagos ya empujados al historial y qué pagos
    // programados a futuro se incorporarán automáticamente a la simulación.
    let nombres_deudas_reales: std::collections::HashSet<String> =
        deudas_reales.iter().map(|d| d.nombre.clone()).collect();
    let mes_actual_etq = chrono::Local::now().format("%Y-%m").to_string();
    let pagos_mes_aplicados: Vec<&DeudaRastreada> = deudas_reales
        .iter()
        .filter(|d| {
            d.historial
                .iter()
                .any(|m| m.mes == mes_actual_etq && m.pago > 0.01)
        })
        .copied()
        .collect();
    let pagos_programados_relevantes: Vec<&omniplanner::ml::PagoProgramado> = state
        .asesor
        .rastreador
        .pagos_programados
        .iter()
        .filter(|pp| nombres_deudas_reales.contains(&pp.nombre_deuda))
        .collect();
    if !pagos_mes_aplicados.is_empty() || !pagos_programados_relevantes.is_empty() {
        println!(
            "  {}",
            "🧮 La simulación parte del estado actual:".cyan().bold()
        );
        if !pagos_mes_aplicados.is_empty() {
            println!(
                "     • Paso 1: pagos de {} ya aplicados a {} deuda(s) ({}).",
                mes_actual_etq,
                pagos_mes_aplicados.len(),
                pagos_mes_aplicados
                    .iter()
                    .map(|d| d.nombre.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        if !pagos_programados_relevantes.is_empty() {
            println!(
                "     • Paso 2: {} pago(s) programado(s) a futuro inyectado(s) en la proyección:",
                pagos_programados_relevantes.len()
            );
            for pp in &pagos_programados_relevantes {
                println!(
                    "        - {} → ${:.2} en {} (cubre {})",
                    pp.nombre_deuda,
                    pp.monto_pi,
                    pp.fecha_pago_prevista,
                    pp.etiqueta_meses()
                );
            }
        }
        println!();
    }

    let sim = state
        .asesor
        .rastreador
        .simular_libertad(presupuesto_comprometido, bola_nieve);

    if sim.meses.is_empty() {
        println!("  No hay nada que simular.");
        pausa();
        return;
    }

    let sim = sim;

    limpiar();
    separador(&format!(
        "📊 PLAN DE LIBERTAD — {} | ${:.2}/mes",
        sim.estrategia, sim.presupuesto_mensual
    ));

    // Mostrar gastos fijos descontados
    if !sim.gastos_fijos.is_empty() {
        println!();
        println!(
            "  🔒 Gastos fijos descontados: {} ({}/mes)",
            sim.gastos_fijos
                .iter()
                .map(|(n, m)| format!("{} ${:.0}", n, m))
                .collect::<Vec<_>>()
                .join(", "),
            format!("${:.2}", sim.total_gastos_fijos).yellow()
        );
        println!(
            "  💰 Presupuesto efectivo para deudas: {}/mes",
            format!("${:.2}", sim.presupuesto_mensual - sim.total_gastos_fijos).green()
        );
    }

    // Nombres de deudas
    let nombres: Vec<String> = if let Some(primer_mes) = sim.meses.first() {
        primer_mes.saldos.iter().map(|(n, _)| n.clone()).collect()
    } else {
        Vec::new()
    };

    // ═══════════════════════════════════════════════════════════
    // TABLA DE AMORTIZACIÓN DETALLADA — mes a mes, deuda por deuda
    // ═══════════════════════════════════════════════════════════
    println!();
    println!(
        "  {}",
        "══════════════════════════════════════════════════════════════════════".cyan()
    );
    println!(
        "  {}",
        "  TABLA DE AMORTIZACIÓN — Distribución de pagos mes a mes"
            .cyan()
            .bold()
    );
    println!(
        "  {}",
        "══════════════════════════════════════════════════════════════════════".cyan()
    );

    for mes in &sim.meses {
        println!();
        // Header del mes
        let pago_total_mes: f64 = mes.pagos.iter().map(|(_, p)| *p).sum();
        let interes_total_mes: f64 = mes.intereses.iter().map(|(_, i)| *i).sum();

        println!(
            "  ┌─── {} ──────────────────────────────────────────────┐",
            format!("MES {} — {}", mes.mes_numero, mes_corto(&mes.mes_yyyy_mm)).bold()
        );

        // Línea 1: Presupuesto efectivo con detalle de liberados
        if mes.liberado_de_liquidadas > 0.01 {
            println!(
                "  │  Presupuesto: {} (base ${:.2} + {} liberados)",
                format!("${:.2}", mes.presupuesto_efectivo).green().bold(),
                mes.presupuesto_efectivo - mes.liberado_de_liquidadas,
                format!("${:.2}", mes.liberado_de_liquidadas).green(),
            );
        } else {
            println!(
                "  │  Presupuesto: {}",
                format!("${:.2}", mes.presupuesto_efectivo),
            );
        }

        // Línea 2: Pagos, intereses, deuda restante, sobrante
        println!(
            "  │  Pagos: {}  │  Intereses: {}  │  Deuda restante: {}{}",
            format!("${:.2}", pago_total_mes).green(),
            format!("${:.2}", interes_total_mes).red(),
            if mes.deuda_total < 0.01 {
                "$0.00".green().bold().to_string()
            } else {
                format!("${:.2}", mes.deuda_total)
            },
            if mes.sobrante > 0.01 {
                format!(
                    "  │  Sin asignar: {}",
                    format!("${:.2}", mes.sobrante).yellow()
                )
            } else {
                String::new()
            }
        );
        println!("  ├──────────────────────┬────────────┬────────────┬──────────────┤");
        println!(
            "  │ {:<20} │ {:>10} │ {:>10} │ {:>12} │",
            "Deuda", "Pago", "Interés", "Saldo"
        );
        println!("  ├──────────────────────┼────────────┼────────────┼──────────────┤");

        for (nombre, saldo) in &mes.saldos {
            let pago = mes
                .pagos
                .iter()
                .find(|(n, _)| n == nombre)
                .map(|(_, p)| *p)
                .unwrap_or(0.0);
            let interes = mes
                .intereses
                .iter()
                .find(|(n, _)| n == nombre)
                .map(|(_, i)| *i)
                .unwrap_or(0.0);

            let nombre_corto = if nombre.len() > 20 {
                format!("{}…", &nombre[..19])
            } else {
                nombre.clone()
            };

            if *saldo < 0.01 && pago < 0.01 {
                // Ya liquidada en un mes anterior
                println!(
                    "  │ {:<20} │ {:>10} │ {:>10} │ {:>12} │",
                    nombre_corto, "—", "—", "✅ $0.00"
                );
            } else if mes.liquidadas_este_mes.contains(nombre) {
                // Se liquidó ESTE mes
                println!(
                    "  │ {} │ {} │ {} │ {} │",
                    format!("{:<20}", nombre_corto).green().bold(),
                    format!("{:>10}", format!("${:.2}", pago)).green().bold(),
                    if interes > 0.01 {
                        format!("{:>10}", format!("${:.2}", interes))
                            .red()
                            .to_string()
                    } else {
                        format!("{:>10}", "$0.00")
                    },
                    format!("{:>12}", "🎉 $0.00").green().bold()
                );
            } else {
                // Deuda activa con pago
                let pago_str = if pago > 0.01 {
                    format!("${:.2}", pago)
                } else {
                    "$0.00".to_string()
                };
                let interes_str = if interes > 0.01 {
                    format!("${:.2}", interes)
                } else {
                    "$0.00".to_string()
                };
                println!(
                    "  │ {:<20} │ {:>10} │ {} │ {:>12} │",
                    nombre_corto,
                    pago_str,
                    if interes > 0.01 {
                        format!("{:>10}", interes_str).red().to_string()
                    } else {
                        format!("{:>10}", interes_str)
                    },
                    format!("${:.2}", saldo)
                );
            }
        }

        println!("  └──────────────────────┴────────────┴────────────┴──────────────┘");

        // Evento de liquidación
        if !mes.liquidadas_este_mes.is_empty() {
            for nombre in &mes.liquidadas_este_mes {
                let pago_final = mes
                    .pagos
                    .iter()
                    .find(|(n, _)| n == nombre)
                    .map(|(_, p)| *p)
                    .unwrap_or(0.0);
                println!(
                    "  {}",
                    format!(
                        "  🎉 ¡{} LIQUIDADA! → ${:.2}/mes liberados para las demás deudas.",
                        nombre.to_uppercase(),
                        pago_final
                    )
                    .green()
                    .bold()
                );
            }
        }
    }

    // ═══════════════════════════════════════════════════════════
    // RESUMEN FINAL
    // ═══════════════════════════════════════════════════════════
    println!();
    let total_meses = sim.meses.len();
    let anios = total_meses / 12;
    let meses_rest = total_meses % 12;
    let tiempo = if anios > 0 && meses_rest > 0 {
        format!("{} año(s) y {} mes(es)", anios, meses_rest)
    } else if anios > 0 {
        format!("{} año(s)", anios)
    } else {
        format!("{} mes(es)", meses_rest)
    };

    println!(
        "  {}",
        "══════════════════════════════════════════════════════════════════════".yellow()
    );
    println!(
        "  {}",
        "  👑  ¡LIBERTAD FINANCIERA ALCANZADA!  👑".green().bold()
    );
    println!(
        "  {}",
        "══════════════════════════════════════════════════════════════════════".yellow()
    );
    println!();
    println!("  ⏱️  Tiempo total:        {}", tiempo.green().bold());
    println!(
        "  💰 Total pagado:        {}",
        format!("${:.2}", sim.total_pagado).cyan()
    );
    println!(
        "  📈 Total en intereses:  {}",
        format!("${:.2}", sim.total_intereses).red()
    );
    println!(
        "  💵 Capital real pagado: {}",
        format!("${:.2}", sim.total_pagado - sim.total_intereses).green()
    );

    // Resumen por deuda: total pagado e intereses por cada una
    println!();
    println!("  {}", "  📋 RESUMEN POR DEUDA".cyan().bold());
    println!("  ┌──────────────────────┬────────────┬────────────┬────────────┬──────────┐");
    println!(
        "  │ {:<20} │ {:>10} │ {:>10} │ {:>10} │ {:>8} │",
        "Deuda", "Pagado", "Intereses", "Capital", "Mes liq."
    );
    println!("  ├──────────────────────┼────────────┼────────────┼────────────┼──────────┤");
    for nombre in &nombres {
        let total_pago_deuda: f64 = sim
            .meses
            .iter()
            .flat_map(|m| m.pagos.iter())
            .filter(|(n, _)| n == nombre)
            .map(|(_, p)| *p)
            .sum();
        let total_int_deuda: f64 = sim
            .meses
            .iter()
            .flat_map(|m| m.intereses.iter())
            .filter(|(n, _)| n == nombre)
            .map(|(_, i)| *i)
            .sum();
        let mes_liq = sim
            .orden_liquidacion
            .iter()
            .find(|(n, _)| n == nombre)
            .map(|(_, m)| format!("{}", m))
            .unwrap_or_else(|| "—".to_string());
        let nombre_corto = if nombre.len() > 20 {
            format!("{}…", &nombre[..19])
        } else {
            nombre.clone()
        };
        println!(
            "  │ {:<20} │ {:>10} │ {} │ {:>10} │ {:>8} │",
            nombre_corto,
            format!("${:.2}", total_pago_deuda),
            format!("{:>10}", format!("${:.2}", total_int_deuda)).red(),
            format!("${:.2}", total_pago_deuda - total_int_deuda),
            mes_liq
        );
    }
    println!("  └──────────────────────┴────────────┴────────────┴────────────┴──────────┘");

    // Orden de liquidación
    println!();
    println!("  {}", "  🗺️  ORDEN DE LIQUIDACIÓN".cyan().bold());
    for (i, (nombre, mes)) in sim.orden_liquidacion.iter().enumerate() {
        let emoji = if i == sim.orden_liquidacion.len() - 1 {
            "👑"
        } else {
            "✅"
        };
        let meses_txt = if *mes == 1 {
            "1 mes".to_string()
        } else {
            format!("{} meses", mes)
        };
        println!(
            "     {} {}. {} — liquidada en {} (mes {})",
            emoji,
            i + 1,
            nombre,
            meses_txt,
            mes
        );
    }
    println!();

    // Editor del plan: permite al usuario ajustar estrategia, mover recursos
    // entre deudas en meses específicos o fijar pagos, como en una hoja de cálculo.
    // El editor maneja internamente la exportación a Excel y la persistencia
    // del borrador — no hay pérdidas accidentales por salidas silenciosas.
    let ofrecer_editor = state.asesor.borrador_plan.is_some()
        || TermConfirm::new()
            .with_prompt("¿Deseas editar y planificar este plan? (mover recursos, cambiar estrategia, trabajar mes por mes)")
            .default(false)
            .interact()
            .unwrap_or(false);
    if ofrecer_editor {
        let _ = editor_plan_libertad(state, sim, presupuesto_comprometido, &nombres);
    } else if TermConfirm::new()
        .with_prompt("¿Deseas exportar este reporte a Excel tal cual?")
        .default(false)
        .interact()
        .unwrap_or(false)
    {
        match exportar_simulacion_excel(&sim, &nombres) {
            Ok(ruta) => {
                println!();
                println!("  ✅ Reporte exportado a: {}", ruta.green().bold());
                println!("  Puedes abrirlo en Excel e imprimirlo.");
            }
            Err(e) => {
                println!();
                println!("  ❌ Error al exportar: {}", e);
            }
        }
    }

    pausa();
}

pub fn rastreador_proyeccion_pagos_liquidez(state: &AppState) {
    let deudas_activas: Vec<&DeudaRastreada> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .filter(|d| d.activa && (d.es_pago_corriente() || d.saldo_actual() > 0.01))
        .collect();

    if deudas_activas.is_empty() {
        println!("  No hay pagos activos para proyectar.");
        pausa();
        return;
    }

    limpiar();
    separador("🧮 PROYECCIÓN DE PAGOS Y LIQUIDEZ");

    let ingreso_mensual_bruto = state.asesor.rastreador.ingreso_mensual_confirmado();
    let retencion_mensual = state.asesor.rastreador.retencion_total_mensual_completa();
    let ingreso_mensual = state.asesor.rastreador.ingreso_mensual_confirmado_neto();
    let saldo_banco = state.asesor.rastreador.saldo_disponible;
    let ingreso_no_confirmado = state.asesor.rastreador.ingreso_mensual_no_confirmado();

    println!("  Esta proyección asume disciplina mínima:");
    println!("    • Mes 1: cubrir todo lo exigible (incluye vencidos)");
    println!("    • Meses siguientes: cubrir al menos el pago del mes");
    println!("    • No agregar cargos nuevos ni atrasos adicionales");
    println!();
    println!(
        "  Saldo disponible actual: {}",
        format!("${:.2}", saldo_banco).green()
    );
    println!(
        "  Ingreso mensual bruto confirmado: {}",
        format!("${:.2}", ingreso_mensual_bruto).green()
    );
    println!(
        "  Retención/allotment mensual estimado: {}",
        format!("${:.2}", retencion_mensual).yellow()
    );
    println!(
        "  Ingreso mensual neto disponible: {}",
        format!("${:.2}", ingreso_mensual).green().bold()
    );
    if ingreso_no_confirmado > 0.01 {
        println!(
            "  Ingreso no confirmado: {} (no se usa en esta proyección)",
            format!("${:.2}", ingreso_no_confirmado).yellow()
        );
    }
    println!();

    let meses = pedir_f64("¿Cuántos meses proyectar?", 6.0) as usize;
    if meses == 0 {
        println!("  Debe ser al menos 1 mes.");
        pausa();
        return;
    }

    println!();
    println!(
        "  {:<8} {:>12} {:>12} {:>12} {:>12}",
        "Mes", "Ingreso", "Pago req.", "Liquidez", "Estado"
    );
    println!("  {}", "─".repeat(66));

    let mut liquidez = saldo_banco;
    let mut primer_mes_requerido = 0.0;
    let mut mes_critico: Option<(usize, f64)> = None;

    for mes_idx in 0..meses {
        let ingreso = ingreso_mensual;
        let pago_requerido: f64 = deudas_activas
            .iter()
            .map(|d| {
                if mes_idx == 0 {
                    d.pago_exigible_total_proximo_mes()
                        .max(d.pago_total_mensual())
                } else {
                    d.pago_total_mensual()
                }
            })
            .sum();

        if mes_idx == 0 {
            primer_mes_requerido = pago_requerido;
        }

        liquidez += ingreso - pago_requerido;
        let estado = if liquidez >= 0.0 {
            "OK".green().bold().to_string()
        } else {
            let faltante = liquidez.abs();
            if mes_critico.is_none() {
                mes_critico = Some((mes_idx + 1, faltante));
            }
            format!("FALTA ${:.2}", faltante).red().bold().to_string()
        };

        println!(
            "  {:<8} {:>12} {:>12} {:>12} {:>12}",
            format!("Mes {}", mes_idx + 1),
            format!("${:.2}", ingreso),
            format!("${:.2}", pago_requerido).yellow(),
            if liquidez >= 0.0 {
                format!("${:.2}", liquidez).green().to_string()
            } else {
                format!("-${:.2}", liquidez.abs()).red().to_string()
            },
            estado
        );
    }

    println!("  {}", "─".repeat(66));
    println!();
    println!(
        "  Requerido para el próximo mes: {}",
        format!("${:.2}", primer_mes_requerido).yellow().bold()
    );

    let pagos_vencidos: Vec<&DeudaRastreada> = deudas_activas
        .iter()
        .copied()
        .filter(|d| d.esta_vencida())
        .collect();
    if !pagos_vencidos.is_empty() {
        println!("  Deudas vencidas que empujan el requerido del mes 1:");
        for deuda in pagos_vencidos {
            println!(
                "    • {} — vencida {} | exigible {}",
                deuda.nombre,
                format!("${:.2}", deuda.deuda_vencida_total()).red(),
                format!("${:.2}", deuda.pago_exigible_total_proximo_mes()).yellow()
            );
        }
    }

    println!();
    match mes_critico {
        Some((mes_num, faltante)) => {
            println!(
                "  ⚠️ Quedarías ilíquido en el mes {}. Te faltarían {} para no quedar mal.",
                mes_num,
                format!("${:.2}", faltante).red().bold()
            );
            println!("  Esto significa que necesitas una de estas acciones antes de ese mes:");
            println!("    • subir ingreso");
            println!("    • recortar gasto no esencial");
            println!("    • bajar otra obligación");
            println!("    • planificar el atraso antes de que vuelva a crecer");
        }
        None => {
            println!(
                "  ✅ Con el saldo actual y el ingreso mensual proyectado, sí alcanzas a cubrir los pagos mostrados sin quedar ilíquido."
            );
        }
    }

    pausa();
}

pub fn rastreador_tabla_aporte_minimo(state: &AppState) {
    let deudas_reales: Vec<&DeudaRastreada> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .filter(|d| d.activa && !d.es_pago_corriente() && d.saldo_actual() > 0.01)
        .collect();

    if deudas_reales.is_empty() {
        println!("  No hay deudas activas para proyectar.");
        pausa();
        return;
    }

    limpiar();
    separador("📊 TABLA DE APORTE MÍNIMO MENSUAL — ¿Cuánto necesitas para salir de deudas?");

    let deuda_total: f64 = deudas_reales.iter().map(|d| d.saldo_actual()).sum();
    let ingreso_mensual = state.asesor.rastreador.ingreso_mensual_total();
    let minimos: f64 = deudas_reales.iter().map(|d| d.pago_minimo).sum();

    println!();
    println!(
        "  Deuda total:     {}",
        format!("${:.2}", deuda_total).red().bold()
    );
    println!(
        "  Ingreso mensual: {}",
        format!("${:.2}", ingreso_mensual).green()
    );
    println!("  Mínimos deudas:  {}", format!("${:.2}", minimos).yellow());
    println!();

    // Elegir estrategia
    let estrategias = &[
        "❄️  Avalancha (tasa más alta primero)",
        "⛄ Bola de nieve (saldo más bajo primero)",
    ];
    let bola_nieve = match menu("¿Qué estrategia usar?", estrategias) {
        Some(1) => true,
        Some(0) => false,
        _ => return,
    };

    // Calcular máximo de meses con pagos mínimos
    let max_meses_default = match state.asesor.rastreador.meses_para_salir(
        minimos
            + state
                .asesor
                .rastreador
                .deudas
                .iter()
                .filter(|d| d.activa && d.es_pago_corriente())
                .map(|d| d.pago_minimo)
                .sum::<f64>(),
        bola_nieve,
    ) {
        Some(m) if m > 0 => m.min(120),
        _ => 60,
    };

    let max_meses = pedir_f64(
        "¿Hasta cuántos meses mostrar? (máx referencia con pago mínimo)",
        max_meses_default as f64,
    ) as usize;

    let min_meses = pedir_f64("¿Desde cuántos meses? (mínimo agresivo)", 1.0) as usize;

    if min_meses > max_meses || min_meses == 0 {
        println!("  Rango inválido.");
        pausa();
        return;
    }

    println!();
    println!("  ⏳ Calculando proyecciones... (esto puede tomar unos segundos)");
    println!();

    let tabla = state
        .asesor
        .rastreador
        .tabla_aporte_minimo(max_meses, min_meses, bola_nieve);

    if tabla.is_empty() {
        println!("  No se pudo calcular ninguna proyección.");
        pausa();
        return;
    }

    limpiar();
    let nombre_est = if bola_nieve {
        "Bola de nieve"
    } else {
        "Avalancha"
    };
    separador(&format!(
        "📊 TABLA DE APORTE MÍNIMO — {} | Deuda: ${:.2}",
        nombre_est, deuda_total
    ));

    println!();
    println!("  💡 Esta tabla muestra cuánto necesitas aportar como mínimo cada mes");
    println!("     para salir de deudas en el número de meses indicado.");
    println!("     Úsala como referencia para saber cuánto debes ganar o destinar.");
    println!();

    // Encabezados de la tabla
    println!(
        "  ┌──────────┬──────────────────┬──────────────────┬──────────────────┬────────────────┐"
    );
    println!(
        "  │ {:>8} │ {:>16} │ {:>16} │ {:>16} │ {:>14} │",
        "Meses", "Aporte/mes", "Total pagado", "Intereses", "Ahorro vs max"
    );
    println!(
        "  ├──────────┼──────────────────┼──────────────────┼──────────────────┼────────────────┤"
    );

    // El mayor total pagado (más meses = más intereses) para calcular ahorro
    let max_total = tabla.first().map(|(_, _, tp, _)| *tp).unwrap_or(0.0);

    let mut prev_aporte = 0.0f64;
    for (meses, aporte, total_pagado, total_intereses) in &tabla {
        let ahorro = max_total - total_pagado;
        let delta = if prev_aporte > 0.01 {
            aporte - prev_aporte
        } else {
            0.0
        };
        let delta_str = if delta.abs() > 0.01 {
            format!(" (+${:.0})", delta)
        } else {
            String::new()
        };

        // Colorear según accesibilidad
        let aporte_str = format!("${:.2}", aporte);
        let aporte_display = if ingreso_mensual > 0.01 && *aporte <= ingreso_mensual {
            format!("{:>16}", aporte_str).green().to_string()
        } else if ingreso_mensual > 0.01 && *aporte <= ingreso_mensual * 1.2 {
            format!("{:>16}", aporte_str).yellow().to_string()
        } else {
            format!("{:>16}", aporte_str).red().to_string()
        };

        println!(
            "  │ {:>6}m  │ {} │ {:>16} │ {:>16} │ {:>14} │",
            meses,
            aporte_display,
            format!("${:.2}", total_pagado),
            format!("${:.2}", total_intereses),
            if ahorro > 0.01 {
                format!("${:.2}", ahorro)
            } else {
                "—".to_string()
            }
        );

        if !delta_str.is_empty() {
            println!(
                "  │          │ {:>16} │                  │                  │                │",
                delta_str
            );
        }

        prev_aporte = *aporte;
    }
    println!(
        "  └──────────┴──────────────────┴──────────────────┴──────────────────┴────────────────┘"
    );

    // Resumen
    println!();
    if let Some((meses_max, aporte_min, _, int_max)) = tabla.first() {
        if let Some((meses_min, aporte_max, _, int_min)) = tabla.last() {
            println!(
                "  📌 Con {} puedes salir en {}m (máximo interés: {})",
                format!("${:.2}/mes", aporte_min).yellow(),
                meses_max,
                format!("${:.2}", int_max).red()
            );
            println!(
                "  🚀 Con {} sales en solo {}m (interés: {})",
                format!("${:.2}/mes", aporte_max).green().bold(),
                meses_min,
                format!("${:.2}", int_min).red()
            );
            let ahorro_total = int_max - int_min;
            if ahorro_total > 0.01 {
                println!(
                    "  💰 Diferencia en intereses: {} — ¡eso te ahorras pagando más rápido!",
                    format!("${:.2}", ahorro_total).green().bold()
                );
            }
        }
    }

    // Indicar qué es viable con ingreso actual
    if ingreso_mensual > 0.01 {
        println!();
        let viables: Vec<_> = tabla
            .iter()
            .filter(|(_, aporte, _, _)| *aporte <= ingreso_mensual)
            .collect();
        if let Some((meses_rapido, aporte_rapido, _, _)) = viables.last() {
            println!(
                "  ✅ Con tu ingreso actual ({}) lo más rápido viable es {}m aportando {}",
                format!("${:.2}", ingreso_mensual).green(),
                meses_rapido,
                format!("${:.2}/mes", aporte_rapido).green().bold()
            );
        } else {
            println!(
                "  ⚠️  Tu ingreso actual ({}) no alcanza para ninguna opción.",
                format!("${:.2}", ingreso_mensual).red()
            );
            if let Some((_, aporte_min, _, _)) = tabla.first() {
                println!(
                    "     Necesitas al menos {} para el plan más lento.",
                    format!("${:.2}/mes", aporte_min).yellow()
                );
            }
        }
    }

    println!();
    pausa();
}

pub fn exportar_simulacion_excel(
    sim: &SimulacionLibertad,
    nombres: &[String],
) -> Result<String, String> {
    let carpeta = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("exports");
    std::fs::create_dir_all(&carpeta).map_err(|e| format!("No se pudo crear carpeta: {}", e))?;

    let fecha = chrono::Local::now().format("%Y-%m-%d_%H%M%S");
    let archivo = carpeta.join(format!("simulacion_deudas_{}.xlsx", fecha));

    let mut wb = Workbook::new();

    // ── Formatos ──
    let fmt_titulo = Format::new()
        .set_bold()
        .set_font_size(14)
        .set_align(FormatAlign::Center);
    let fmt_header = Format::new()
        .set_bold()
        .set_font_size(11)
        .set_border(FormatBorder::Thin)
        .set_background_color("4472C4")
        .set_font_color("FFFFFF")
        .set_align(FormatAlign::Center);
    let fmt_dinero = Format::new()
        .set_num_format("$#,##0.00")
        .set_border(FormatBorder::Thin);
    let fmt_dinero_rojo = Format::new()
        .set_num_format("$#,##0.00")
        .set_border(FormatBorder::Thin)
        .set_font_color("FF0000");
    let fmt_dinero_verde = Format::new()
        .set_num_format("$#,##0.00")
        .set_border(FormatBorder::Thin)
        .set_font_color("008000");
    let fmt_celda = Format::new()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center);
    let fmt_celda_izq = Format::new().set_border(FormatBorder::Thin);
    let fmt_evento = Format::new().set_bold().set_font_color("008000");
    let fmt_descubierto = Format::new()
        .set_bold()
        .set_font_color("FFFFFF")
        .set_background_color("C00000")
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center);
    let fmt_seccion = Format::new()
        .set_bold()
        .set_font_size(12)
        .set_background_color("D9E2F3");

    // ════════════════════════════════════════════
    //  HOJA 1: Amortización mes a mes
    // ════════════════════════════════════════════
    let ws = wb.add_worksheet();
    ws.set_name("Amortización").map_err(|e| e.to_string())?;

    // Título
    ws.merge_range(0, 0, 0, 4, "", &fmt_titulo)
        .map_err(|e| e.to_string())?;
    ws.write_string_with_format(
        0,
        0,
        format!(
            "Plan de Libertad Financiera — {} | ${:.2}/mes",
            sim.estrategia, sim.presupuesto_mensual
        ),
        &fmt_titulo,
    )
    .map_err(|e| e.to_string())?;

    // Info general
    let mut row = 2u32;
    ws.write_string(row, 0, "Presupuesto mensual:")
        .map_err(|e| e.to_string())?;
    ws.write_number_with_format(row, 1, sim.presupuesto_mensual, &fmt_dinero)
        .map_err(|e| e.to_string())?;
    row += 1;
    ws.write_string(row, 0, "Gastos fijos:")
        .map_err(|e| e.to_string())?;
    ws.write_number_with_format(row, 1, sim.total_gastos_fijos, &fmt_dinero)
        .map_err(|e| e.to_string())?;
    if !sim.gastos_fijos.is_empty() {
        let detalle: String = sim
            .gastos_fijos
            .iter()
            .map(|(n, m)| format!("{} ${:.2}", n, m))
            .collect::<Vec<_>>()
            .join(", ");
        ws.write_string(row, 2, &detalle)
            .map_err(|e| e.to_string())?;
    }
    row += 1;
    ws.write_string(row, 0, "Disponible para deudas:")
        .map_err(|e| e.to_string())?;
    ws.write_number_with_format(
        row,
        1,
        sim.presupuesto_mensual - sim.total_gastos_fijos,
        &fmt_dinero_verde,
    )
    .map_err(|e| e.to_string())?;
    row += 2;

    // Tabla de amortización
    for mes in &sim.meses {
        ws.merge_range(row, 0, row, 4, "", &fmt_seccion)
            .map_err(|e| e.to_string())?;
        let pago_total: f64 = mes.pagos.iter().map(|(_, p)| *p).sum();
        let int_total: f64 = mes.intereses.iter().map(|(_, i)| *i).sum();
        ws.write_string_with_format(
            row,
            0,
            format!(
                "MES {}  |  Pagos: ${:.2}  |  Intereses: ${:.2}  |  Deuda restante: ${:.2}",
                mes.mes_numero, pago_total, int_total, mes.deuda_total
            ),
            &fmt_seccion,
        )
        .map_err(|e| e.to_string())?;
        row += 1;

        ws.write_string_with_format(row, 0, "Deuda", &fmt_header)
            .map_err(|e| e.to_string())?;
        ws.write_string_with_format(row, 1, "Pago", &fmt_header)
            .map_err(|e| e.to_string())?;
        ws.write_string_with_format(row, 2, "Interés", &fmt_header)
            .map_err(|e| e.to_string())?;
        ws.write_string_with_format(row, 3, "Saldo", &fmt_header)
            .map_err(|e| e.to_string())?;
        ws.write_string_with_format(row, 4, "Evento", &fmt_header)
            .map_err(|e| e.to_string())?;
        row += 1;

        for (nombre, saldo) in &mes.saldos {
            let pago = mes
                .pagos
                .iter()
                .find(|(n, _)| n == nombre)
                .map(|(_, p)| *p)
                .unwrap_or(0.0);
            let interes = mes
                .intereses
                .iter()
                .find(|(n, _)| n == nombre)
                .map(|(_, i)| *i)
                .unwrap_or(0.0);

            ws.write_string_with_format(row, 0, nombre, &fmt_celda_izq)
                .map_err(|e| e.to_string())?;
            ws.write_number_with_format(row, 1, pago, &fmt_dinero_verde)
                .map_err(|e| e.to_string())?;
            ws.write_number_with_format(
                row,
                2,
                interes,
                if interes > 0.01 {
                    &fmt_dinero_rojo
                } else {
                    &fmt_dinero
                },
            )
            .map_err(|e| e.to_string())?;
            ws.write_number_with_format(row, 3, *saldo, &fmt_dinero)
                .map_err(|e| e.to_string())?;

            if mes.liquidadas_este_mes.contains(nombre) {
                ws.write_string_with_format(row, 4, "LIQUIDADA", &fmt_evento)
                    .map_err(|e| e.to_string())?;
            } else if *saldo < 0.01 && pago < 0.01 {
                ws.write_string_with_format(row, 4, "ya liquidada", &fmt_celda)
                    .map_err(|e| e.to_string())?;
            } else if mes.deudas_descubiertas.iter().any(|n| n == nombre) {
                // Pago recibido por debajo del mínimo → esta deuda crece por intereses
                let etiqueta = if pago < 0.01 && interes > 0.01 {
                    "⚠ SIN PAGO — CRECE"
                } else {
                    "⚠ PAGO < MÍNIMO"
                };
                ws.write_string_with_format(row, 4, etiqueta, &fmt_descubierto)
                    .map_err(|e| e.to_string())?;
            }
            row += 1;
        }
        row += 1;
    }

    ws.set_column_width(0, 22).map_err(|e| e.to_string())?;
    ws.set_column_width(1, 14).map_err(|e| e.to_string())?;
    ws.set_column_width(2, 14).map_err(|e| e.to_string())?;
    ws.set_column_width(3, 14).map_err(|e| e.to_string())?;
    ws.set_column_width(4, 14).map_err(|e| e.to_string())?;

    // ════════════════════════════════════════════
    //  HOJA 2: Resumen
    // ════════════════════════════════════════════
    let ws2 = wb.add_worksheet();
    ws2.set_name("Resumen").map_err(|e| e.to_string())?;

    ws2.merge_range(0, 0, 0, 4, "", &fmt_titulo)
        .map_err(|e| e.to_string())?;
    ws2.write_string_with_format(0, 0, "Resumen — Plan de Libertad Financiera", &fmt_titulo)
        .map_err(|e| e.to_string())?;

    let mut r = 2u32;
    ws2.write_string(r, 0, "Estrategia:")
        .map_err(|e| e.to_string())?;
    ws2.write_string(r, 1, &sim.estrategia)
        .map_err(|e| e.to_string())?;
    r += 1;
    ws2.write_string(r, 0, "Meses totales:")
        .map_err(|e| e.to_string())?;
    ws2.write_number_with_format(r, 1, sim.meses.len() as f64, &fmt_celda)
        .map_err(|e| e.to_string())?;
    r += 1;
    ws2.write_string(r, 0, "Total pagado:")
        .map_err(|e| e.to_string())?;
    ws2.write_number_with_format(r, 1, sim.total_pagado, &fmt_dinero)
        .map_err(|e| e.to_string())?;
    r += 1;
    ws2.write_string(r, 0, "Total intereses:")
        .map_err(|e| e.to_string())?;
    ws2.write_number_with_format(r, 1, sim.total_intereses, &fmt_dinero_rojo)
        .map_err(|e| e.to_string())?;
    r += 1;
    ws2.write_string(r, 0, "Capital pagado:")
        .map_err(|e| e.to_string())?;
    ws2.write_number_with_format(
        r,
        1,
        sim.total_pagado - sim.total_intereses,
        &fmt_dinero_verde,
    )
    .map_err(|e| e.to_string())?;
    r += 2;

    // Sección de alertas: mínimos no cubiertos
    if sim.meses_con_descubierto > 0 {
        ws2.merge_range(r, 0, r, 4, "", &fmt_descubierto)
            .map_err(|e| e.to_string())?;
        ws2.write_string_with_format(
            r,
            0,
            "⚠ ALERTA — EL PRESUPUESTO NO CUBRE TODOS LOS PAGOS MÍNIMOS",
            &fmt_descubierto,
        )
        .map_err(|e| e.to_string())?;
        r += 1;
        ws2.write_string(r, 0, "Meses con descubierto:")
            .map_err(|e| e.to_string())?;
        ws2.write_number_with_format(r, 1, sim.meses_con_descubierto as f64, &fmt_celda)
            .map_err(|e| e.to_string())?;
        r += 1;
        ws2.write_string(r, 0, "Total mínimos no cubiertos:")
            .map_err(|e| e.to_string())?;
        ws2.write_number_with_format(r, 1, sim.minimos_no_cubiertos_total, &fmt_dinero_rojo)
            .map_err(|e| e.to_string())?;
        r += 1;
        // Detalle: deudas que más veces quedaron descubiertas
        let mut conteo: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for m in &sim.meses {
            for n in &m.deudas_descubiertas {
                *conteo.entry(n.clone()).or_insert(0) += 1;
            }
        }
        let mut listado: Vec<(String, usize)> = conteo.into_iter().collect();
        listado.sort_by(|a, b| b.1.cmp(&a.1));
        if !listado.is_empty() {
            ws2.write_string(r, 0, "Deudas descubiertas (meses):")
                .map_err(|e| e.to_string())?;
            let detalle: String = listado
                .iter()
                .map(|(n, c)| format!("{} ({})", n, c))
                .collect::<Vec<_>>()
                .join(", ");
            ws2.write_string(r, 1, &detalle)
                .map_err(|e| e.to_string())?;
            r += 1;
        }
        ws2.write_string(
            r,
            0,
            "Causa: el presupuesto mensual o los pagos forzados dejan a estas deudas por debajo de su mínimo.",
        )
        .map_err(|e| e.to_string())?;
        r += 1;
        ws2.write_string(
            r,
            0,
            "Efecto: esas deudas CRECEN por intereses compuestos → 'no hay ahorro posible'.",
        )
        .map_err(|e| e.to_string())?;
        r += 2;
    }

    ws2.write_string_with_format(r, 0, "Deuda", &fmt_header)
        .map_err(|e| e.to_string())?;
    ws2.write_string_with_format(r, 1, "Total pagado", &fmt_header)
        .map_err(|e| e.to_string())?;
    ws2.write_string_with_format(r, 2, "Intereses", &fmt_header)
        .map_err(|e| e.to_string())?;
    ws2.write_string_with_format(r, 3, "Capital", &fmt_header)
        .map_err(|e| e.to_string())?;
    ws2.write_string_with_format(r, 4, "Mes liquidación", &fmt_header)
        .map_err(|e| e.to_string())?;
    r += 1;

    for nombre in nombres {
        let total_pago: f64 = sim
            .meses
            .iter()
            .flat_map(|m| m.pagos.iter())
            .filter(|(n, _)| n == nombre)
            .map(|(_, p)| *p)
            .sum();
        let total_int: f64 = sim
            .meses
            .iter()
            .flat_map(|m| m.intereses.iter())
            .filter(|(n, _)| n == nombre)
            .map(|(_, i)| *i)
            .sum();
        let mes_liq = sim
            .orden_liquidacion
            .iter()
            .find(|(n, _)| n == nombre)
            .map(|(_, m)| *m as f64);

        ws2.write_string_with_format(r, 0, nombre, &fmt_celda_izq)
            .map_err(|e| e.to_string())?;
        ws2.write_number_with_format(r, 1, total_pago, &fmt_dinero)
            .map_err(|e| e.to_string())?;
        ws2.write_number_with_format(r, 2, total_int, &fmt_dinero_rojo)
            .map_err(|e| e.to_string())?;
        ws2.write_number_with_format(r, 3, total_pago - total_int, &fmt_dinero_verde)
            .map_err(|e| e.to_string())?;
        if let Some(m) = mes_liq {
            ws2.write_number_with_format(r, 4, m, &fmt_celda)
                .map_err(|e| e.to_string())?;
        } else {
            ws2.write_string_with_format(r, 4, "—", &fmt_celda)
                .map_err(|e| e.to_string())?;
        }
        r += 1;
    }

    r += 1;
    ws2.write_string_with_format(r, 0, "Orden de liquidación", &fmt_seccion)
        .map_err(|e| e.to_string())?;
    r += 1;
    for (i, (nombre, mes)) in sim.orden_liquidacion.iter().enumerate() {
        ws2.write_string_with_format(r, 0, format!("{}. {}", i + 1, nombre), &fmt_celda_izq)
            .map_err(|e| e.to_string())?;
        ws2.write_string(r, 1, format!("Mes {}", mes))
            .map_err(|e| e.to_string())?;
        r += 1;
    }

    ws2.set_column_width(0, 22).map_err(|e| e.to_string())?;
    ws2.set_column_width(1, 16).map_err(|e| e.to_string())?;
    ws2.set_column_width(2, 14).map_err(|e| e.to_string())?;
    ws2.set_column_width(3, 14).map_err(|e| e.to_string())?;
    ws2.set_column_width(4, 18).map_err(|e| e.to_string())?;

    // Guardar
    wb.save(&archivo)
        .map_err(|e| format!("Error guardando Excel: {}", e))?;

    Ok(archivo.to_string_lossy().to_string())
}

pub fn rastreador_editar_pago(state: &mut AppState) {
    if state.asesor.rastreador.deudas.is_empty() {
        println!("  Sin deudas registradas.");
        pausa();
        return;
    }

    let nombres: Vec<String> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .map(|d| format!("{} ({} meses)", d.nombre, d.historial.len()))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    if let Some(idx) = menu("¿Editar cuál deuda?", &refs) {
        let d = &state.asesor.rastreador.deudas[idx];
        if d.historial.is_empty() {
            println!("  No hay meses registrados.");
            pausa();
            return;
        }

        let meses: Vec<String> = d
            .historial
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let exigible = d.pago_exigible_total_en_mes(i);
                format!(
                    "{} — Saldo: ${:.2}, Pago: ${:.2}, Exigible: ${:.2}, Cargos: ${:.2}",
                    m.mes,
                    m.saldo_inicio,
                    m.pago + m.pago_escrow,
                    exigible,
                    m.nuevos_cargos
                )
            })
            .collect();
        let refs_m: Vec<&str> = meses.iter().map(|s| s.as_str()).collect();

        if let Some(midx) = menu("¿Cuál mes editar?", &refs_m) {
            let actual = &d.historial[midx];
            let (pago_exigible_pi, pago_exigible_escrow) = d.pago_exigible_componentes_en_mes(midx);
            let pago_exigible_total = d.pago_exigible_total_en_mes(midx);
            let tiene_escrow = d.tiene_escrow_configurado();

            println!();
            println!("  Datos actuales — {}", actual.mes.cyan().bold());
            println!("    Fecha (mes):            {}", actual.mes);
            println!("    Saldo inicio:           ${:.2}", actual.saldo_inicio);
            println!("    Pago P&I:               ${:.2}", actual.pago);
            if tiene_escrow {
                println!("    Pago Escrow:            ${:.2}", actual.pago_escrow);
            }
            println!(
                "    Pago total:             ${:.2}",
                actual.pago + actual.pago_escrow
            );
            println!("    Pago exigible acumulado:${:.2}", pago_exigible_total);
            println!("    Nuevos cargos:          ${:.2}", actual.nuevos_cargos);
            if !actual.nota.is_empty() {
                println!("    Nota:                   {}", actual.nota);
            }
            println!();
            println!(
                "  {} Deja vacío (Enter) cualquier campo para mantener el valor actual.",
                "ℹ".cyan()
            );
            println!();

            // ── Fecha (mes) ──────────────────────────────────────────────────
            let mes_actual = actual.mes.clone();
            println!(
                "  {}",
                "── Fecha ─────────────────────────────────────────".dimmed()
            );
            println!("  Acepta: junio, jun 2026, 06, 062026, 2026-06. Enter = conservar actual.");
            let nueva_fecha = pedir_mes_flexible(
                &format!("Fecha del mes (actual: {})", mes_actual),
                &mes_actual,
                true,
            );

            // ── Saldo de inicio ──────────────────────────────────────────────
            println!();
            println!(
                "  {}",
                "── Saldo de inicio ───────────────────────────────".dimmed()
            );
            let nuevo_saldo_inicio = pedir_f64(
                &format!("Saldo de inicio (actual ${:.2})", actual.saldo_inicio),
                actual.saldo_inicio,
            );

            // ── Pagos ────────────────────────────────────────────────────────
            println!();
            println!(
                "  {}",
                "── Montos pagados ────────────────────────────────".dimmed()
            );
            let nuevo_pago = pedir_f64(
                &format!(
                    "Pago P&I (actual ${:.2}, exigible ${:.2})",
                    actual.pago, pago_exigible_pi
                ),
                actual.pago,
            );
            let nuevo_pago_escrow = if tiene_escrow {
                pedir_f64(
                    &format!(
                        "Pago Escrow (actual ${:.2}, exigible ${:.2})",
                        actual.pago_escrow, pago_exigible_escrow
                    ),
                    actual.pago_escrow,
                )
            } else {
                0.0
            };
            let nuevos_cargos = pedir_f64(
                &format!("Nuevos cargos (actual ${:.2})", actual.nuevos_cargos),
                actual.nuevos_cargos,
            );

            // ── Nota libre ───────────────────────────────────────────────────
            println!();
            println!(
                "  {}",
                "── Nota ──────────────────────────────────────────".dimmed()
            );
            let nota_actual = actual.nota.clone();
            let nueva_nota = {
                let input = pedir_texto_opcional(&format!("Nota (actual: \"{}\")", nota_actual));
                if input.trim().is_empty() {
                    nota_actual
                } else {
                    input
                }
            };

            // ── Recalcular ───────────────────────────────────────────────────
            let tasa_anual = state.asesor.rastreador.deudas[idx].tasa_anual;
            let saldo_inicio = nuevo_saldo_inicio;

            // Actualizar este mes
            let tasa_mensual = tasa_anual / 100.0 / 12.0;
            let saldo_despues = (saldo_inicio - nuevo_pago).max(0.0);
            let intereses = saldo_despues * tasa_mensual;
            let saldo_final = saldo_despues + intereses + nuevos_cargos;

            state.asesor.rastreador.deudas[idx].historial[midx].mes = nueva_fecha.clone();
            state.asesor.rastreador.deudas[idx].historial[midx].saldo_inicio = saldo_inicio;
            state.asesor.rastreador.deudas[idx].historial[midx].pago = nuevo_pago;
            state.asesor.rastreador.deudas[idx].historial[midx].pago_escrow = nuevo_pago_escrow;
            state.asesor.rastreador.deudas[idx].historial[midx].nuevos_cargos = nuevos_cargos;
            state.asesor.rastreador.deudas[idx].historial[midx].intereses = intereses;
            state.asesor.rastreador.deudas[idx].historial[midx].saldo_final =
                if saldo_final < 0.01 { 0.0 } else { saldo_final };
            state.asesor.rastreador.deudas[idx].historial[midx].nota = nueva_nota;

            // Recalcular meses siguientes
            let mut saldo = if saldo_final < 0.01 { 0.0 } else { saldo_final };
            let len = state.asesor.rastreador.deudas[idx].historial.len();
            for i in (midx + 1)..len {
                state.asesor.rastreador.deudas[idx].historial[i].saldo_inicio = saldo;
                let pago_i = state.asesor.rastreador.deudas[idx].historial[i].pago;
                let cargos_i = state.asesor.rastreador.deudas[idx].historial[i].nuevos_cargos;
                let sd = (saldo - pago_i).max(0.0);
                let int_i = sd * tasa_mensual;
                let sf = sd + int_i + cargos_i;
                state.asesor.rastreador.deudas[idx].historial[i].intereses = int_i;
                state.asesor.rastreador.deudas[idx].historial[i].saldo_final =
                    if sf < 0.01 { 0.0 } else { sf };
                saldo = if sf < 0.01 { 0.0 } else { sf };
            }

            println!();
            println!(
                "  {} Pago actualizado para el mes {}. Saldos recalculados. Nuevo saldo final: ${:.2}",
                "✓".green(),
                nueva_fecha.cyan().bold(),
                state.asesor.rastreador.deudas[idx].saldo_actual()
            );
            if nuevo_pago + nuevo_pago_escrow + 0.01 < pago_exigible_total {
                println!(
                    "  ⚠️ Ese mes sigue con atraso: faltan ${:.2} para cubrir el exigible acumulado.",
                    (pago_exigible_total - (nuevo_pago + nuevo_pago_escrow)).max(0.0)
                );
            }

            // ── Sincronizar hacia presupuesto ───────────────────────────────
            let nombre_deuda = state.asesor.rastreador.deudas[idx].nombre.clone();
            let mes_editado = state.asesor.rastreador.deudas[idx].historial[midx]
                .mes
                .clone();
            let monto_total = nuevo_pago + nuevo_pago_escrow;
            if let Some(mes_fmt) = crate::mes_a_yyyy_mm(&mes_editado) {
                crate::sincronizar_presupuesto_desde_rastreador(
                    state,
                    &nombre_deuda,
                    &mes_fmt,
                    monto_total,
                );
            }

            pausa();
        }
    }
}

pub fn rastreador_ajustar_tasa(state: &mut AppState) {
    if state.asesor.rastreador.deudas.is_empty() {
        println!("  Sin deudas registradas.");
        pausa();
        return;
    }

    let nombres: Vec<String> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .map(|d| format!("{} — tasa actual: {:.1}% anual", d.nombre, d.tasa_anual))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    if let Some(idx) = menu("¿A cuál deuda ajustar la tasa?", &refs) {
        let nombre = state.asesor.rastreador.deudas[idx].nombre.clone();
        let actual = state.asesor.rastreador.deudas[idx].tasa_anual;
        println!();
        println!(
            "  {} — Tasa actual: {:.2}% anual ({:.2}% mensual)",
            nombre,
            actual,
            actual / 12.0
        );
        let nueva = pedir_f64("Nueva tasa anual (%) (ej: 24.99)", actual);
        state.asesor.rastreador.deudas[idx].tasa_anual = nueva;
        println!(
            "  {} Tasa de '{}' actualizada a {:.2}%",
            "✓".green(),
            nombre,
            nueva
        );
    }
    pausa();
}

pub fn rastreador_ingreso(state: &mut AppState) {
    state.asesor.rastreador.migrar_ingreso_legacy();
    loop {
        limpiar();
        separador("💵 INGRESOS");

        let rast = &state.asesor.rastreador;
        // Estado de residencia global
        if rast.estado_residencia.is_empty() {
            println!(
                "  {} Estado de residencia no configurado. Agrégalo en 'Configurar estado de residencia'.",
                "ℹ".cyan()
            );
        } else {
            println!(
                "  Estado de residencia: {}{}",
                rast.estado_residencia.to_uppercase().cyan().bold(),
                if omniplanner::ml::advisor::RastreadorDeudas::estado_sin_impuesto(
                    &rast.estado_residencia
                ) {
                    format!(" {} sin impuesto estatal sobre ingresos", "(✓)".green())
                } else {
                    String::new()
                }
            );
        }
        println!();
        if rast.ingresos.is_empty() {
            println!("  No hay ingresos registrados.");
        } else {
            for (i, ing) in rast.ingresos.iter().enumerate() {
                let estado_trabajo_txt = if !ing.estado_trabajo.is_empty() {
                    let mismo = rast.estado_residencia.trim().to_uppercase()
                        == ing.estado_trabajo.trim().to_uppercase();
                    if mismo {
                        format!(" [trabajo: {}]", ing.estado_trabajo.to_uppercase().cyan())
                    } else {
                        format!(
                            " [trabajo: {} {} estado dual]",
                            ing.estado_trabajo.to_uppercase().yellow().bold(),
                            "⚠".yellow()
                        )
                    }
                } else {
                    String::new()
                };
                println!(
                    "  {}. {} — {} bruto ({}) [{} | {}]{}",
                    i + 1,
                    ing.concepto,
                    format!("${:.2}", ing.monto).green(),
                    ing.frecuencia.nombre(),
                    ing.etiqueta_confirmacion(),
                    ing.etiqueta_taxes(),
                    estado_trabajo_txt
                );
                println!(
                    "      neto {} | fed {} | est {} | SS {} | Medicare {}",
                    format!("${:.2}", ing.monto_mensual_neto()).green(),
                    format!("${:.2}", ing.retencion_federal_mensual()).magenta(),
                    format!("${:.2}", ing.retencion_estatal_mensual()).yellow(),
                    format!("${:.2}", ing.retencion_social_security_mensual()).cyan(),
                    format!("${:.2}", ing.retencion_medicare_mensual()).blue()
                );
            }
            println!();
            println!(
                "  Total mensual confirmado bruto: {}",
                format!("${:.2}", rast.ingreso_mensual_confirmado())
                    .green()
                    .bold()
            );
            println!(
                "  Retención mensual estimada: {} (fed {} | est {} | SS {} | Med {})",
                format!("${:.2}", rast.retencion_total_mensual_completa())
                    .yellow()
                    .bold(),
                format!("${:.2}", rast.retencion_federal_mensual_total()).magenta(),
                format!("${:.2}", rast.retencion_estatal_mensual_total()).yellow(),
                format!("${:.2}", rast.retencion_social_security_mensual_total()).cyan(),
                format!("${:.2}", rast.retencion_medicare_mensual_total()).blue()
            );
            println!(
                "  Total mensual confirmado neto: {}",
                format!("${:.2}", rast.ingreso_mensual_confirmado_neto())
                    .green()
                    .bold()
            );
            println!(
                "  Total mensual no confirmado: {}",
                format!("${:.2}", rast.ingreso_mensual_no_confirmado()).yellow()
            );
            println!(
                "  Confirmado no taxeable: {} | Con federal: {} | Con estatal: {}",
                format!("${:.2}", rast.ingreso_mensual_no_taxeable()).cyan(),
                format!("${:.2}", rast.ingreso_mensual_impuesto_federal()).magenta(),
                format!("${:.2}", rast.ingreso_mensual_impuesto_estatal()).yellow()
            );

            let ingresos_allotment_pendiente: Vec<&IngresoRastreado> = rast
                .ingresos
                .iter()
                .filter(|ing| {
                    !ing.permitir_allotment_cero
                        && ((ing.paga_impuesto_federal()
                            && ing.allotment_federal_pct_efectivo() <= 0.0)
                            || (ing.paga_impuesto_estatal()
                                && ing.allotment_estatal_pct_efectivo() <= 0.0))
                })
                .collect();
            if !ingresos_allotment_pendiente.is_empty() {
                println!();
                println!(
                    "  {} Completa el allotment para evitar deuda fiscal futura:",
                    "⚠".yellow().bold()
                );
                for ing in ingresos_allotment_pendiente {
                    let mut faltantes: Vec<&str> = Vec::new();
                    if ing.paga_impuesto_federal() && ing.allotment_federal_pct_efectivo() <= 0.0 {
                        faltantes.push("federal");
                    }
                    if ing.paga_impuesto_estatal() && ing.allotment_estatal_pct_efectivo() <= 0.0 {
                        faltantes.push("estatal");
                    }
                    println!(
                        "    - {}: allotment pendiente ({})",
                        ing.concepto,
                        faltantes.join(" + ")
                    );
                }
                println!(
                    "    Edita el ingreso y agrega % de allotment. Este aviso desaparece automáticamente al capturarlo."
                );
            }

            // Alerta de estado dual (trabaja en un estado, vive en otro)
            let duales = rast.ingresos_estado_dual();
            if !duales.is_empty() {
                println!();
                println!(
                    "  {} Tienes ingresos en estado diferente a tu residencia:",
                    "⚠".yellow().bold()
                );
                for ing in &duales {
                    println!(
                        "    - {}: trabaja en {} | reside en {}",
                        ing.concepto,
                        ing.estado_trabajo.to_uppercase().yellow().bold(),
                        rast.estado_residencia.to_uppercase().cyan()
                    );
                }
                println!(
                    "    Podrías tener obligación de declarar en ambos estados. Consulta las reglas de crédito por impuestos pagados al otro estado."
                );
            }

            let ingresos_ss_temprano: Vec<&IngresoRastreado> = rast
                .ingresos
                .iter()
                .filter(|ing| ing.beneficio_social_security_temprano)
                .collect();
            if !ingresos_ss_temprano.is_empty() {
                let ingreso_laboral_anual: f64 = rast
                    .ingresos
                    .iter()
                    .filter(|ing| ing.confirmado && !ing.es_beneficio_social_security)
                    .map(|ing| ing.monto_mensual() * 12.0)
                    .sum();
                println!();
                println!(
                    "  {} Beneficio de Social Security antes de edad plena detectado:",
                    "⚠".yellow().bold()
                );
                for ing in ingresos_ss_temprano {
                    println!("    - {}", ing.concepto);
                }
                println!(
                    "    Ingreso laboral anual estimado (sin beneficios SS): {}",
                    format!("${:.2}", ingreso_laboral_anual).yellow().bold()
                );
                println!(
                    "    Mantén ingreso anual bajo y valida el límite SSA/IRS vigente del año para evitar deuda fiscal."
                );
            }
        }
        println!();

        let opciones = &[
            "➕  Agregar ingreso",
            "✏️   Editar ingreso",
            "🗑️   Eliminar ingreso",
            "🧮  Calcular aporte mínimo de allotment",
            "🏠  Configurar estado de residencia",
            "🔙  Volver",
        ];
        match menu("¿Qué hacer?", opciones) {
            Some(0) => rastreador_agregar_ingreso(state),
            Some(1) => rastreador_editar_ingreso(state),
            Some(2) => rastreador_eliminar_ingreso(state),
            Some(3) => rastreador_calcular_aporte_minimo_allotment(state),
            Some(4) => rastreador_configurar_estado_residencia(state),
            _ => return,
        }
    }
}

pub fn rastreador_configurar_estado_residencia(state: &mut AppState) {
    limpiar();
    separador("🏠 ESTADO DE RESIDENCIA");
    let actual = &state.asesor.rastreador.estado_residencia;
    if actual.is_empty() {
        println!("  Sin estado configurado.");
    } else {
        let sin_impuesto = omniplanner::ml::advisor::RastreadorDeudas::estado_sin_impuesto(actual);
        println!(
            "  Estado actual: {}{}",
            actual.to_uppercase().cyan().bold(),
            if sin_impuesto {
                "  (sin impuesto estatal sobre ingresos)"
            } else {
                ""
            }
        );
    }
    println!();
    println!("  Ingresa las siglas de tu estado (ej: TX, FL, NY, CA, PR).");
    println!("  Estados sin impuesto estatal: AK, FL, NV, SD, TN, TX, WA, WY");
    let nuevo = match pedir_texto("Estado de residencia (siglas, vacío=cancelar)") {
        Some(s) if !s.trim().is_empty() => s.trim().to_uppercase(),
        _ => {
            println!("  Cancelado.");
            pausa();
            return;
        }
    };
    let sin_impuesto = omniplanner::ml::advisor::RastreadorDeudas::estado_sin_impuesto(&nuevo);
    state.asesor.rastreador.estado_residencia = nuevo.clone();
    println!(
        "  {} Estado de residencia actualizado a: {}{}",
        "✓".green(),
        nuevo.cyan().bold(),
        if sin_impuesto {
            "  — sin impuesto estatal, no necesitas allotment estatal.".to_string()
        } else {
            String::new()
        }
    );
    pausa();
}

pub fn rastreador_calcular_aporte_minimo_allotment(state: &AppState) {
    limpiar();
    separador("🧮 APORTE MÍNIMO DE ALLOTMENT");

    let ingresos_taxeables_confirmados: Vec<&IngresoRastreado> = state
        .asesor
        .rastreador
        .ingresos
        .iter()
        .filter(|ing| {
            ing.confirmado && (ing.paga_impuesto_federal() || ing.paga_impuesto_estatal())
        })
        .collect();

    if ingresos_taxeables_confirmados.is_empty() {
        println!(
            "  {} Actualmente no tienes ingresos taxeables confirmados.",
            "ℹ".cyan()
        );
        println!(
            "  Cuando agregues un ingreso de empleo con impuesto federal/estatal, aquí verás el cálculo mínimo recomendado."
        );
        pausa();
        return;
    }

    println!("  Este cálculo te da un piso de contribución para no quedarte corto.");
    println!("  Ajusta los porcentajes objetivo según tu estrategia anual.");
    println!();

    let federal_obj_pct = pedir_f64("% objetivo mínimo federal", 12.0).max(0.0);
    let estatal_obj_pct = pedir_f64("% objetivo mínimo estatal", 5.0).max(0.0);

    let mut base_federal_mensual = 0.0;
    let mut base_estatal_mensual = 0.0;
    let mut actual_federal_mensual = 0.0;
    let mut actual_estatal_mensual = 0.0;

    for ing in ingresos_taxeables_confirmados {
        let mensual = ing.monto_mensual();
        if ing.paga_impuesto_federal() {
            base_federal_mensual += mensual;
            actual_federal_mensual += ing.retencion_federal_mensual();
        }
        if ing.paga_impuesto_estatal() {
            base_estatal_mensual += mensual;
            actual_estatal_mensual += ing.retencion_estatal_mensual();
        }
    }

    let objetivo_federal_mensual = base_federal_mensual * (federal_obj_pct / 100.0);
    let objetivo_estatal_mensual = base_estatal_mensual * (estatal_obj_pct / 100.0);
    let objetivo_total_mensual = objetivo_federal_mensual + objetivo_estatal_mensual;
    let actual_total_mensual = actual_federal_mensual + actual_estatal_mensual;
    let brecha_mensual = (objetivo_total_mensual - actual_total_mensual).max(0.0);

    println!();
    println!(
        "  Base mensual taxeable federal: {}",
        format!("${:.2}", base_federal_mensual).magenta()
    );
    println!(
        "  Base mensual taxeable estatal: {}",
        format!("${:.2}", base_estatal_mensual).yellow()
    );
    println!();
    println!(
        "  Objetivo mínimo mensual: {} (fed {} + est {})",
        format!("${:.2}", objetivo_total_mensual).yellow().bold(),
        format!("${:.2}", objetivo_federal_mensual).magenta(),
        format!("${:.2}", objetivo_estatal_mensual).yellow()
    );
    println!(
        "  Retención actual mensual: {} (fed {} + est {})",
        format!("${:.2}", actual_total_mensual).green().bold(),
        format!("${:.2}", actual_federal_mensual).magenta(),
        format!("${:.2}", actual_estatal_mensual).yellow()
    );
    if brecha_mensual > 0.01 {
        println!(
            "  {} Te faltan al menos {} por mes para alcanzar el mínimo objetivo.",
            "⚠".yellow().bold(),
            format!("${:.2}", brecha_mensual).yellow().bold()
        );
    } else {
        println!(
            "  {} Tu retención actual ya cubre o supera el mínimo objetivo.",
            "✓".green().bold()
        );
    }
    println!(
        "  Objetivo mínimo anual: {}",
        format!("${:.2}", objetivo_total_mensual * 12.0)
            .yellow()
            .bold()
    );

    pausa();
}

pub fn pedir_frecuencia(prompt: &str) -> Option<FrecuenciaPago> {
    let frecuencias = &[
        "Semanal",
        "Quincenal",
        "Mensual",
        "Trimestral",
        "Semestral",
        "Anual",
        "Una sola vez (pago único)",
    ];
    match menu(prompt, frecuencias) {
        Some(0) => Some(FrecuenciaPago::Semanal),
        Some(1) => Some(FrecuenciaPago::Quincenal),
        Some(2) => Some(FrecuenciaPago::Mensual),
        Some(3) => Some(FrecuenciaPago::Trimestral),
        Some(4) => Some(FrecuenciaPago::Semestral),
        Some(5) => Some(FrecuenciaPago::Anual),
        Some(6) => Some(FrecuenciaPago::UnaVez),
        _ => None,
    }
}

pub fn rastreador_agregar_ingreso(state: &mut AppState) {
    let concepto = match pedir_texto("Concepto (ej: Sueldo empresa X, Freelance, Renta)") {
        Some(c) => c,
        None => return,
    };
    let freq = match pedir_frecuencia("¿Cada cuánto recibes este ingreso?") {
        Some(f) => f,
        None => return,
    };
    let monto = pedir_f64("Monto ($)", 0.0);
    if monto <= 0.0 {
        println!("  {} El monto debe ser mayor a 0.", "✗".red());
        pausa();
        return;
    }
    let confirmado = TermConfirm::new()
        .with_prompt("  ¿Este ingreso ya existe y está confirmado?")
        .default(true)
        .interact()
        .unwrap_or(true);
    let impuesto_federal = TermConfirm::new()
        .with_prompt("  ¿Este ingreso paga impuesto federal?")
        .default(false)
        .interact()
        .unwrap_or(false);
    let impuesto_estatal = TermConfirm::new()
        .with_prompt("  ¿Este ingreso paga impuesto estatal?")
        .default(false)
        .interact()
        .unwrap_or(false);
    let allotment_federal_pct = if impuesto_federal {
        pedir_f64("  % de allotment/retención federal", 0.0)
    } else {
        0.0
    };
    let allotment_estatal_pct = if impuesto_estatal {
        pedir_f64("  % de allotment/retención estatal", 0.0)
    } else {
        0.0
    };
    // Estado de trabajo
    let estado_residencia = state.asesor.rastreador.estado_residencia.clone();
    println!();
    if !estado_residencia.is_empty() {
        println!(
            "  Estado de residencia: {}",
            estado_residencia.to_uppercase().cyan()
        );
    }
    let estado_trabajo = match pedir_texto(
        "  Estado donde realizas este trabajo (siglas, vacío=mismo que residencia)",
    ) {
        Some(s) if !s.trim().is_empty() => s.trim().to_uppercase(),
        _ => estado_residencia.clone(),
    };
    if !estado_trabajo.is_empty()
        && !estado_residencia.is_empty()
        && estado_trabajo != estado_residencia.to_uppercase()
    {
        println!(
            "  {} Trabajo en {} pero resides en {}. Podrías tener obligación fiscal en ambos estados.",
            "⚠".yellow().bold(),
            estado_trabajo.yellow().bold(),
            estado_residencia.to_uppercase().cyan()
        );
    }
    let retener_social_security = TermConfirm::new()
        .with_prompt("  ¿Retener Social Security en este ingreso?")
        .default(false)
        .interact()
        .unwrap_or(false);
    let retener_medicare = TermConfirm::new()
        .with_prompt("  ¿Retener Medicare en este ingreso?")
        .default(false)
        .interact()
        .unwrap_or(false);
    let es_beneficio_social_security = TermConfirm::new()
        .with_prompt("  ¿Este ingreso corresponde a beneficios de Social Security?")
        .default(false)
        .interact()
        .unwrap_or(false);
    let beneficio_social_security_temprano = if es_beneficio_social_security {
        TermConfirm::new()
            .with_prompt("  ¿Recibes este beneficio antes de la edad plena de jubilación?")
            .default(false)
            .interact()
            .unwrap_or(false)
    } else {
        false
    };
    let permitir_allotment_cero = if (impuesto_federal && allotment_federal_pct <= 0.0)
        || (impuesto_estatal && allotment_estatal_pct <= 0.0)
    {
        TermConfirm::new()
            .with_prompt(
                "  ¿Confirmas dejar este ingreso con 0% de impuestos/allotment de forma intencional?",
            )
            .default(false)
            .interact()
            .unwrap_or(false)
    } else {
        false
    };
    state.asesor.rastreador.ingresos.push(IngresoRastreado {
        concepto: concepto.clone(),
        monto,
        frecuencia: freq.clone(),
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
    println!(
        "  {} Ingreso agregado: {} — ${:.2} ({}) [{} | {}]",
        "✓".green(),
        concepto,
        monto,
        freq.nombre(),
        if confirmado {
            "confirmado"
        } else {
            "no confirmado"
        },
        if impuesto_federal || impuesto_estatal {
            if impuesto_federal && impuesto_estatal {
                "federal + estatal"
            } else if impuesto_federal {
                "federal"
            } else {
                "estatal"
            }
        } else {
            "no taxeable"
        }
    );
    if impuesto_federal || impuesto_estatal {
        println!(
            "    Retención estimada: fed {:.2}% | est {:.2}%",
            allotment_federal_pct, allotment_estatal_pct
        );
    }
    if retener_social_security || retener_medicare {
        println!(
            "    Payroll taxes: SS {} | Medicare {}",
            if retener_social_security { "sí" } else { "no" },
            if retener_medicare { "sí" } else { "no" }
        );
    }
    if permitir_allotment_cero {
        println!(
            "    {} 0% de impuestos/allotment marcado como decisión intencional.",
            "⚠".yellow()
        );
    }
    if beneficio_social_security_temprano {
        println!(
            "    {} Beneficio SS temprano: mantén ingreso anual bajo para evitar deuda con IRS.",
            "⚠".yellow()
        );
    }
    pausa();
}

pub fn rastreador_editar_ingreso(state: &mut AppState) {
    if state.asesor.rastreador.ingresos.is_empty() {
        println!("  No hay ingresos para editar.");
        pausa();
        return;
    }
    let nombres: Vec<String> = state
        .asesor
        .rastreador
        .ingresos
        .iter()
        .enumerate()
        .map(|(i, ing)| {
            format!(
                "{}. {} — ${:.2} ({}) [{} | {}]",
                i + 1,
                ing.concepto,
                ing.monto,
                ing.frecuencia.nombre(),
                ing.etiqueta_confirmacion(),
                ing.etiqueta_taxes()
            )
        })
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
    let idx = match menu("¿Cuál editar?", &refs) {
        Some(i) => i,
        None => return,
    };

    let ing = &state.asesor.rastreador.ingresos[idx];
    let concepto_actual = ing.concepto.clone();
    let monto_actual = ing.monto;

    let nuevo_concepto = pedir_texto_opcional(&format!(
        "Concepto (actual: {}, vacío=mantener)",
        concepto_actual
    ));
    let freq = pedir_frecuencia("Nueva frecuencia (Esc=mantener)");
    let nuevo_monto = pedir_f64("Nuevo monto ($)", monto_actual);
    let confirmado = TermConfirm::new()
        .with_prompt(format!(
            "  ¿Ingreso confirmado? (actual: {})",
            if ing.confirmado { "sí" } else { "no" }
        ))
        .default(ing.confirmado)
        .interact()
        .unwrap_or(ing.confirmado);
    let impuesto_federal = TermConfirm::new()
        .with_prompt(format!(
            "  ¿Paga impuesto federal? (actual: {})",
            if ing.paga_impuesto_federal() {
                "sí"
            } else {
                "no"
            }
        ))
        .default(ing.paga_impuesto_federal())
        .interact()
        .unwrap_or(ing.paga_impuesto_federal());
    let impuesto_estatal = TermConfirm::new()
        .with_prompt(format!(
            "  ¿Paga impuesto estatal? (actual: {})",
            if ing.paga_impuesto_estatal() {
                "sí"
            } else {
                "no"
            }
        ))
        .default(ing.paga_impuesto_estatal())
        .interact()
        .unwrap_or(ing.paga_impuesto_estatal());
    let allotment_federal_pct = if impuesto_federal {
        pedir_f64(
            &format!(
                "  % de allotment federal (actual {:.2}%)",
                ing.allotment_federal_pct_efectivo()
            ),
            ing.allotment_federal_pct_efectivo(),
        )
    } else {
        0.0
    };
    let allotment_estatal_pct = if impuesto_estatal {
        pedir_f64(
            &format!(
                "  % de allotment estatal (actual {:.2}%)",
                ing.allotment_estatal_pct_efectivo()
            ),
            ing.allotment_estatal_pct_efectivo(),
        )
    } else {
        0.0
    };
    let retener_social_security = TermConfirm::new()
        .with_prompt(format!(
            "  ¿Retener Social Security? (actual: {})",
            if ing.retener_social_security {
                "sí"
            } else {
                "no"
            }
        ))
        .default(ing.retener_social_security)
        .interact()
        .unwrap_or(ing.retener_social_security);
    let retener_medicare = TermConfirm::new()
        .with_prompt(format!(
            "  ¿Retener Medicare? (actual: {})",
            if ing.retener_medicare { "sí" } else { "no" }
        ))
        .default(ing.retener_medicare)
        .interact()
        .unwrap_or(ing.retener_medicare);
    let es_beneficio_social_security = TermConfirm::new()
        .with_prompt(format!(
            "  ¿Es beneficio de Social Security? (actual: {})",
            if ing.es_beneficio_social_security {
                "sí"
            } else {
                "no"
            }
        ))
        .default(ing.es_beneficio_social_security)
        .interact()
        .unwrap_or(ing.es_beneficio_social_security);
    let beneficio_social_security_temprano = if es_beneficio_social_security {
        TermConfirm::new()
            .with_prompt(format!(
                "  ¿Beneficio antes de edad plena? (actual: {})",
                if ing.beneficio_social_security_temprano {
                    "sí"
                } else {
                    "no"
                }
            ))
            .default(ing.beneficio_social_security_temprano)
            .interact()
            .unwrap_or(ing.beneficio_social_security_temprano)
    } else {
        false
    };
    let permitir_allotment_cero = if (impuesto_federal && allotment_federal_pct <= 0.0)
        || (impuesto_estatal && allotment_estatal_pct <= 0.0)
    {
        TermConfirm::new()
            .with_prompt(format!(
                "  ¿Mantener 0% de impuestos/allotment intencionalmente? (actual: {})",
                if ing.permitir_allotment_cero {
                    "sí"
                } else {
                    "no"
                }
            ))
            .default(ing.permitir_allotment_cero)
            .interact()
            .unwrap_or(ing.permitir_allotment_cero)
    } else {
        false
    };
    // Estado de trabajo
    let estado_residencia = state.asesor.rastreador.estado_residencia.clone();
    let actual_estado_trabajo = state.asesor.rastreador.ingresos[idx].estado_trabajo.clone();
    let prompt_estado = if actual_estado_trabajo.is_empty() {
        format!(
            "  Estado donde realizas este trabajo (vacío=mismo que residencia {})",
            if estado_residencia.is_empty() {
                "no configurado".to_string()
            } else {
                estado_residencia.to_uppercase()
            }
        )
    } else {
        format!(
            "  Estado de trabajo (actual: {}, vacío=mantener)",
            actual_estado_trabajo.to_uppercase()
        )
    };
    let estado_trabajo = match pedir_texto(&prompt_estado) {
        Some(s) if !s.trim().is_empty() => s.trim().to_uppercase(),
        _ => actual_estado_trabajo.clone(),
    };
    if !estado_trabajo.is_empty()
        && !estado_residencia.is_empty()
        && estado_trabajo != estado_residencia.trim().to_uppercase()
    {
        println!(
            "  {} Trabajo en {} pero resides en {}. Podrías tener obligación fiscal en ambos estados.",
            "⚠".yellow().bold(),
            estado_trabajo.as_str().yellow().bold(),
            estado_residencia.to_uppercase().cyan()
        );
    }

    let ing = &mut state.asesor.rastreador.ingresos[idx];
    if !nuevo_concepto.is_empty() {
        ing.concepto = nuevo_concepto;
    }
    if let Some(f) = freq {
        ing.frecuencia = f;
    }
    ing.monto = nuevo_monto;
    ing.confirmado = confirmado;
    ing.impuesto_federal = impuesto_federal;
    ing.impuesto_estatal = impuesto_estatal;
    ing.taxeable = impuesto_federal || impuesto_estatal;
    ing.allotment_federal_pct = allotment_federal_pct;
    ing.allotment_estatal_pct = allotment_estatal_pct;
    ing.retener_social_security = retener_social_security;
    ing.retener_medicare = retener_medicare;
    ing.permitir_allotment_cero = permitir_allotment_cero;
    ing.es_beneficio_social_security = es_beneficio_social_security;
    ing.beneficio_social_security_temprano = beneficio_social_security_temprano;
    ing.estado_trabajo = estado_trabajo;
    println!(
        "  {} Ingreso actualizado: {} — ${:.2} ({}) [{} | {}]",
        "✓".green(),
        ing.concepto,
        ing.monto,
        ing.frecuencia.nombre(),
        ing.etiqueta_confirmacion(),
        ing.etiqueta_taxes()
    );
    if ing.es_taxeable() {
        println!(
            "    Retención estimada: fed {:.2}% | est {:.2}% | SS {} | Medicare {} | neto mensual {}",
            ing.allotment_federal_pct_efectivo(),
            ing.allotment_estatal_pct_efectivo(),
            if ing.retener_social_security { "sí" } else { "no" },
            if ing.retener_medicare { "sí" } else { "no" },
            format!("${:.2}", ing.monto_mensual_neto()).green()
        );
    }
    if ing.permitir_allotment_cero {
        println!(
            "    {} 0% de impuestos/allotment quedó registrado como decisión intencional.",
            "⚠".yellow()
        );
    }
    if ing.beneficio_social_security_temprano {
        println!(
            "    {} Beneficio SS temprano activo: mantén ingreso anual bajo para evitar deuda con IRS.",
            "⚠".yellow()
        );
    }
    pausa();
}

pub fn rastreador_eliminar_ingreso(state: &mut AppState) {
    if state.asesor.rastreador.ingresos.is_empty() {
        println!("  No hay ingresos para eliminar.");
        pausa();
        return;
    }
    let nombres: Vec<String> = state
        .asesor
        .rastreador
        .ingresos
        .iter()
        .enumerate()
        .map(|(i, ing)| {
            format!(
                "{}. {} — ${:.2} ({})",
                i + 1,
                ing.concepto,
                ing.monto,
                ing.frecuencia.nombre()
            )
        })
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
    let idx = match menu("¿Cuál eliminar?", &refs) {
        Some(i) => i,
        None => return,
    };
    let eliminado = state.asesor.rastreador.ingresos.remove(idx);
    println!(
        "  {} Ingreso '{}' eliminado.",
        "✓".green(),
        eliminado.concepto
    );
    pausa();
}

// ══════════════════════════════════════════════════════════════════════════════
//  Selección múltiple de deudas → calcular total / simular liquidar
// ══════════════════════════════════════════════════════════════════════════════

pub fn rastreador_seleccionar_calcular(state: &mut AppState) {
    limpiar();
    separador("🧮 SELECCIONAR DEUDAS — CALCULAR / LIQUIDAR");

    let deudas = &state.asesor.rastreador.deudas;
    if deudas.is_empty() {
        println!("  Sin deudas registradas.");
        pausa();
        return;
    }

    // Construir etiquetas con saldo y pago mensual
    let etiquetas: Vec<String> = deudas
        .iter()
        .map(|d| {
            let saldo = d.saldo_actual();
            let pago = d.pago_total_mensual();
            let estado = if d.activa { "" } else { " [liquidada]" };
            format!(
                "{:<25}  Saldo: {:>11}  Pago/mes: {:>9}{}",
                if d.nombre.len() > 24 {
                    format!("{}…", &d.nombre[..23])
                } else {
                    d.nombre.clone()
                },
                format!("${:.2}", saldo),
                format!("${:.2}", pago),
                estado
            )
        })
        .collect();

    println!();
    println!("  Usa ESPACIO para marcar/desmarcar, Enter para confirmar.");
    println!("  Puedes seleccionar las deudas que tienes dinero para liquidar");
    println!("  y ver el total exacto que necesitarías.");
    println!();

    // Multi-selección
    let seleccionados: Vec<usize> = if crate::es_termux() {
        // En Termux no hay dialoguer interactivo — entrada por números
        for (i, etq) in etiquetas.iter().enumerate() {
            println!("  {}. {}", i + 1, etq);
        }
        println!();
        match pedir_texto("Ingresa los números separados por coma (ej: 1,3,5)") {
            Some(entrada) => entrada
                .split(',')
                .filter_map(|s| s.trim().parse::<usize>().ok())
                .filter(|&n| n >= 1 && n <= deudas.len())
                .map(|n| n - 1)
                .collect(),
            None => return,
        }
    } else {
        use dialoguer::MultiSelect;
        let refs: Vec<&str> = etiquetas.iter().map(|s| s.as_str()).collect();
        match MultiSelect::new()
            .with_prompt("  Selecciona las deudas")
            .items(&refs)
            .interact_opt()
            .unwrap_or(None)
        {
            Some(sel) => sel,
            None => return,
        }
    };

    if seleccionados.is_empty() {
        println!();
        println!("  ℹ️  No seleccionaste ninguna deuda.");
        pausa();
        return;
    }

    // ── Calcular totales de la selección ──────────────────────────────────
    limpiar();
    separador("🧮 RESUMEN DE DEUDAS SELECCIONADAS");

    let mes_hoy = chrono::Local::now().format("%Y-%m").to_string();

    let mut total_saldo: f64 = 0.0;
    let mut total_pago_mensual: f64 = 0.0;
    let mut total_interes_mensual: f64 = 0.0;

    println!(
        "  {:<26} {:>12} {:>10} {:>10}",
        "Cuenta", "Saldo actual", "Int/mes", "Pago/mes"
    );
    println!("  {}", "─".repeat(64));

    for &idx in &seleccionados {
        let d = &deudas[idx];
        let saldo = d.saldo_actual();
        let tasa_mensual = d.tasa_anual / 100.0 / 12.0;
        let interes_mes = saldo * tasa_mensual;
        let pago_mes = d.pago_total_mensual();

        total_saldo += saldo;
        total_pago_mensual += pago_mes;
        total_interes_mensual += interes_mes;

        let nombre_corto = if d.nombre.len() > 25 {
            format!("{}…", &d.nombre[..24])
        } else {
            d.nombre.clone()
        };
        println!(
            "  {:<26} {:>12} {:>10} {:>10}",
            nombre_corto,
            format!("${:.2}", saldo),
            format!("${:.2}", interes_mes),
            format!("${:.2}", pago_mes),
        );
    }

    println!("  {}", "─".repeat(64));
    println!(
        "  {:<26} {:>12} {:>10} {:>10}",
        format!("TOTAL ({} deudas)", seleccionados.len()),
        format!("${:.2}", total_saldo).green().bold().to_string(),
        format!("${:.2}", total_interes_mensual)
            .yellow()
            .to_string(),
        format!("${:.2}", total_pago_mensual).cyan().to_string(),
    );
    println!();

    // Cuántos meses para liquidar al ritmo actual
    if total_pago_mensual > total_interes_mensual + 0.01 {
        let meses_est = (total_saldo / (total_pago_mensual - total_interes_mensual)).ceil() as u64;
        println!(
            "  ⏱️  Al ritmo actual (~${:.2}/mes netos a capital), quedan ~{} meses ({:.1} años)",
            total_pago_mensual - total_interes_mensual,
            meses_est,
            meses_est as f64 / 12.0
        );
    }

    // Ahorro de intereses si se liquidan hoy
    if total_pago_mensual > total_interes_mensual + 0.01 {
        let meses_est = (total_saldo / (total_pago_mensual - total_interes_mensual)).ceil();
        let intereses_totales = total_interes_mensual * meses_est;
        println!(
            "  💰 Si liquidas hoy: ahorras ~{} en intereses futuros",
            format!("${:.2}", intereses_totales).green().bold()
        );
    }

    println!();
    println!(
        "  📌 Para liquidar estas {} deuda(s) necesitas tener disponibles:",
        seleccionados.len()
    );
    println!("     {}", format!("${:.2}", total_saldo).green().bold());
    println!();

    // ── Opciones de acción ────────────────────────────────────────────────
    let opciones_accion = &[
        "📄  Exportar esta selección como CSV",
        "✅  Registrar pago completo (liquidar) de las deudas seleccionadas",
        "🔙  Solo ver y volver",
    ];

    match menu("¿Qué deseas hacer?", opciones_accion) {
        Some(0) => {
            // Exportar CSV de la selección
            let mut csv = String::new();
            csv.push_str(
                "Nombre,Saldo_actual,Tasa_anual_%,Pago_mensual,Interes_mensual,A_capital_mensual\n",
            );
            for &idx in &seleccionados {
                let d = &state.asesor.rastreador.deudas[idx];
                let saldo = d.saldo_actual();
                let tasa_mensual = d.tasa_anual / 100.0 / 12.0;
                let interes_mes = saldo * tasa_mensual;
                let pago_mes = d.pago_total_mensual();
                let a_capital = (pago_mes - interes_mes).max(0.0);
                csv.push_str(&format!(
                    "{},{:.2},{:.4},{:.2},{:.2},{:.2}\n",
                    d.nombre.replace(',', ";"),
                    saldo,
                    d.tasa_anual,
                    pago_mes,
                    interes_mes,
                    a_capital,
                ));
            }
            // Línea de totales
            csv.push_str(&format!(
                "TOTAL,{:.2},,{:.2},{:.2},{:.2}\n",
                total_saldo,
                total_pago_mensual,
                total_interes_mensual,
                (total_pago_mensual - total_interes_mensual).max(0.0),
            ));

            let dir = omniplanner::ml::advisor::AlmacenAsesor::dir_exportacion();
            let ruta = dir.join("seleccion_deudas.csv");
            match std::fs::write(&ruta, &csv) {
                Ok(()) => println!(
                    "\n  ✅ CSV exportado: {}",
                    ruta.display().to_string().green()
                ),
                Err(e) => println!("  ❌ Error al exportar: {}", e),
            }
            pausa();
        }
        Some(1) => {
            // Liquidar: registrar pago igual al saldo actual para cada deuda seleccionada
            println!();
            println!(
                "  ⚠️  Esto registrará un pago por el SALDO COMPLETO en el mes {}.",
                mes_hoy.cyan()
            );
            println!("  Las deudas quedarán marcadas como liquidadas (saldo = 0).");
            println!();
            if confirmar("¿Confirmas liquidar las deudas seleccionadas?", false) {
                let mut liquidadas = 0usize;
                for &idx in &seleccionados {
                    let saldo = state.asesor.rastreador.deudas[idx].saldo_actual();
                    if saldo < 0.01 {
                        continue; // ya liquidada
                    }
                    // Registrar pago por el saldo completo con escrow 0
                    state.asesor.rastreador.deudas[idx]
                        .registrar_mes_con_escrow(&mes_hoy, saldo, saldo, 0.0, 0.0);
                    state.asesor.rastreador.deudas[idx].activa = false;
                    liquidadas += 1;
                }
                // Persistir
                let _ = state.guardar();
                println!();
                println!(
                    "  ✅ {} deuda(s) marcadas como liquidadas.",
                    liquidadas.to_string().green().bold()
                );
                println!(
                    "  💰 Total liquidado: {}",
                    format!("${:.2}", total_saldo).green().bold()
                );
            }
            pausa();
        }
        _ => {}
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Consolidación de deudas — ¿me conviene este préstamo?
//
//  Colores de las filas:
//    🔵 cyan   = deuda seleccionada AUTOMÁTICAMENTE por el sistema
//    🟡 yellow = deuda movida MANUALMENTE por el usuario (override)
//    🟢 green  = se va (cubierta por el préstamo)   ← solapados con ↑
//    🔴 red    = se queda (no cubierta)
// ══════════════════════════════════════════════════════════════════════════════

pub fn rastreador_consolidar_deudas(state: &mut AppState) {
    limpiar();
    println!(
        "{}",
        "╔══════════════════════════════════════════════════════════════════╗".cyan()
    );
    println!(
        "{}",
        "║  🔄 CONSOLIDACIÓN DE DEUDAS                                    ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║  Evalúa si un préstamo de consolidación te conviene            ║".cyan()
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════════════╝".cyan()
    );
    println!();

    let deudas_activas: Vec<(usize, &DeudaRastreada)> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .enumerate()
        .filter(|(_, d)| d.activa && d.saldo_actual() > 0.01)
        .collect();

    if deudas_activas.is_empty() {
        println!("  Sin deudas activas registradas.");
        pausa();
        return;
    }

    // ── PASO 1: Datos del préstamo ofrecido ───────────────────────────────
    separador("📋 PASO 1 — Datos del préstamo de consolidación");
    println!("  (El banco o cooperativa te ofrece un préstamo para saldar deudas.)");
    println!();

    let monto_prestamo = pedir_f64("Monto del préstamo que te ofrecen ($)", 0.0);
    if monto_prestamo < 0.01 {
        return;
    }
    let tasa_anual_prest = pedir_f64("Tasa anual del préstamo (APR %, ej: 5.99)", 0.0);
    let meses_prest = pedir_f64("Plazo en meses (ej: 60 para 5 años)", 60.0) as u32;
    if meses_prest == 0 {
        return;
    }

    // Calcular pago mensual con fórmula de amortización
    let tasa_mensual_prest = tasa_anual_prest / 100.0 / 12.0;
    let pago_mensual_calc = if tasa_mensual_prest < 0.0001 {
        monto_prestamo / meses_prest as f64
    } else {
        let r = tasa_mensual_prest;
        let n = meses_prest as f64;
        let factor = r * (1.0 + r).powf(n) / ((1.0 + r).powf(n) - 1.0);
        monto_prestamo * factor
    };
    let total_a_pagar_prest = pago_mensual_calc * meses_prest as f64;
    let interes_total_prest = total_a_pagar_prest - monto_prestamo;

    println!();
    println!(
        "  💡 Pago mensual calculado: {}",
        format!("${:.2}", pago_mensual_calc).cyan().bold()
    );
    println!(
        "     Total a pagar en {} meses: {}  (intereses: {})",
        meses_prest,
        format!("${:.2}", total_a_pagar_prest).yellow(),
        format!("${:.2}", interes_total_prest).red()
    );

    // El usuario puede confirmar o corregir con la cifra que el banco le cotizó
    let pago_mensual_real = {
        println!();
        let cotizado = pedir_f64(
            "Pago mensual que te cotizó el banco (Enter = usar el calculado)",
            pago_mensual_calc,
        );
        if (cotizado - pago_mensual_calc).abs() > 1.0 {
            println!(
                "  ℹ️  Diferencia vs calculado: ${:.2} — se usará la cifra cotizada.",
                (cotizado - pago_mensual_calc).abs()
            );
        }
        cotizado
    };
    let total_real = pago_mensual_real * meses_prest as f64;
    let interes_real = total_real - monto_prestamo;

    // ── PASO 2: Selección automática por estrategia ───────────────────────
    limpiar();
    separador("🤖 PASO 2 — Selección automática de deudas");
    println!(
        "  El préstamo es de {}. El sistema elige qué deudas cubrir.",
        format!("${:.2}", monto_prestamo).cyan().bold()
    );
    println!();

    let estrategias = &[
        "🏆  Avalancha — paga primero las de mayor tasa (ahorra más intereses)",
        "⛄  Bola de nieve — elimina primero las más pequeñas (más cuentas cerradas)",
        "💰  Maximizar monto — llenar al máximo el préstamo (cubre más saldo)",
        "✋  Sin auto-selección — yo elijo manualmente todas",
    ];
    let estrategia_idx = match menu("Estrategia de selección automática", estrategias) {
        Some(i) => i,
        None => return,
    };

    // Ordenar índices según estrategia
    let mut orden_auto: Vec<usize> = (0..deudas_activas.len()).collect();
    match estrategia_idx {
        0 => {
            // Avalancha: mayor tasa primero
            orden_auto.sort_by(|&a, &b| {
                deudas_activas[b]
                    .1
                    .tasa_anual
                    .partial_cmp(&deudas_activas[a].1.tasa_anual)
                    .unwrap()
            });
        }
        1 => {
            // Bola de nieve: menor saldo primero
            orden_auto.sort_by(|&a, &b| {
                deudas_activas[a]
                    .1
                    .saldo_actual()
                    .partial_cmp(&deudas_activas[b].1.saldo_actual())
                    .unwrap()
            });
        }
        2 => {
            // Maximizar: mayor saldo primero (llenar el préstamo)
            orden_auto.sort_by(|&a, &b| {
                deudas_activas[b]
                    .1
                    .saldo_actual()
                    .partial_cmp(&deudas_activas[a].1.saldo_actual())
                    .unwrap()
            });
        }
        _ => {
            orden_auto.clear(); // sin auto-selección
        }
    }

    // Seleccionar automáticamente hasta cubrir el monto del préstamo
    // auto_sel[i] = true si el sistema la marcó, false si no
    let mut auto_sel = vec![false; deudas_activas.len()];
    let mut monto_cubierto = 0.0f64;
    for &i in &orden_auto {
        let saldo = deudas_activas[i].1.saldo_actual();
        if monto_cubierto + saldo <= monto_prestamo + 0.01 {
            auto_sel[i] = true;
            monto_cubierto += saldo;
        }
    }

    // ── PASO 3: Mostrar tabla y permitir ajuste manual ────────────────────
    // manual_override[i]: Some(true) = user forzó incluir, Some(false) = user forzó excluir, None = sigue auto
    let mut manual_override: Vec<Option<bool>> = vec![None; deudas_activas.len()];

    loop {
        limpiar();
        separador("🎨 PASO 3 — Revisar y ajustar selección");

        // Estado final de cada deuda (va o se queda)
        let va: Vec<bool> = (0..deudas_activas.len())
            .map(|i| manual_override[i].unwrap_or(auto_sel[i]))
            .collect();

        let saldo_cubierto: f64 = deudas_activas
            .iter()
            .enumerate()
            .filter(|(i, _)| va[*i])
            .map(|(_, (_, d))| d.saldo_actual())
            .sum();

        let saldo_restante: f64 = deudas_activas
            .iter()
            .enumerate()
            .filter(|(i, _)| !va[*i])
            .map(|(_, (_, d))| d.saldo_actual())
            .sum();

        // Encabezado de tabla
        println!("  LEYENDA:");
        println!(
            "   {}  = sistema elige  |  {}  = tú lo ajustaste  |  {}  = se va (verde)  |  {}  = se queda (rojo)",
            "[AUTO]".cyan().bold(),
            "[MANUAL]".yellow().bold(),
            "●".green().bold(),
            "●".red().bold()
        );
        println!();
        println!(
            "  {:<3} {:<24} {:>10} {:>7} {:>10} {:>10}  {}",
            "#", "Cuenta", "Saldo", "Tasa%", "Pago/mes", "Int/mes", "Estado"
        );
        println!("  {}", "─".repeat(85));

        let mut opciones_toggle: Vec<String> = Vec::new();

        for (i, (_, d)) in deudas_activas.iter().enumerate() {
            let saldo = d.saldo_actual();
            let int_mes = saldo * d.tasa_anual / 100.0 / 12.0;
            let esta_incluida = va[i];
            let es_auto = manual_override[i].is_none() && auto_sel[i];
            let es_manual = manual_override[i].is_some();

            let badge = if es_manual {
                "[MANUAL]".yellow().bold().to_string()
            } else if es_auto {
                "[AUTO]  ".cyan().bold().to_string()
            } else {
                "        ".to_string()
            };

            let nombre_corto = if d.nombre.len() > 23 {
                format!("{}…", &d.nombre[..22])
            } else {
                d.nombre.clone()
            };

            let estado_str = if esta_incluida {
                "🟢 SE VA".green().bold().to_string()
            } else {
                "🔴 QUEDA".red().bold().to_string()
            };

            // La línea completa coloreada según si va o queda
            let linea = format!(
                "  {:>2}. {:<24} {:>10} {:>6.1}% {:>10} {:>10}  {}  {}",
                i + 1,
                nombre_corto,
                format!("${:.2}", saldo),
                d.tasa_anual,
                format!("${:.2}", d.pago_total_mensual()),
                format!("${:.2}", int_mes),
                badge,
                estado_str
            );
            if esta_incluida {
                println!("{}", linea.green());
            } else {
                println!("{}", linea.red());
            }

            let tag = if esta_incluida {
                "✅ SE VA"
            } else {
                "❌ QUEDA"
            };
            opciones_toggle.push(format!(
                "{}  {} — ${:.2}  ({:.1}%)",
                tag, d.nombre, saldo, d.tasa_anual
            ));
        }

        println!("  {}", "─".repeat(85));

        // Resumen de uso del préstamo
        let excede = saldo_cubierto > monto_prestamo + 0.01;
        let uso_pct = (saldo_cubierto / monto_prestamo * 100.0).min(999.0);
        let disponible_restante = (monto_prestamo - saldo_cubierto).max(0.0);

        if excede {
            println!(
                "  ⚠️  Saldo cubierto {} EXCEDE el préstamo de {} — ajusta la selección.",
                format!("${:.2}", saldo_cubierto).red().bold(),
                format!("${:.2}", monto_prestamo).cyan()
            );
        } else {
            println!(
                "  💳 Uso del préstamo: {} / {} ({:.1}%)   Disponible: {}",
                format!("${:.2}", saldo_cubierto).green().bold(),
                format!("${:.2}", monto_prestamo).cyan(),
                uso_pct,
                format!("${:.2}", disponible_restante).yellow()
            );
        }
        println!(
            "  Deuda que queda fuera: {}",
            format!("${:.2}", saldo_restante).red()
        );
        println!();

        // Acciones disponibles
        let accion_opciones = &[
            "🔀  Mover una deuda (verde↔rojo)",
            "📊  Ver análisis comparativo completo",
            "✅  Aceptar y agregar préstamo al rastreador",
            "🔙  Cancelar",
        ];

        match menu("¿Qué deseas hacer?", accion_opciones) {
            Some(0) => {
                // Toggle de una deuda
                opciones_toggle.push("🔙 Cancelar".to_string());
                let refs: Vec<&str> = opciones_toggle.iter().map(|s| s.as_str()).collect();
                if let Some(idx) = menu("¿Cuál deuda mover?", &refs) {
                    if idx < deudas_activas.len() {
                        let actual = va[idx];
                        // Si iba a ir → forzar que se quede; si se quedaba → forzar que vaya
                        let nuevo_estado = !actual;
                        // Verificar que al incluirla no exceda el monto
                        if nuevo_estado {
                            let nuevo_total: f64 = deudas_activas
                                .iter()
                                .enumerate()
                                .filter(|(i2, _)| {
                                    if *i2 == idx {
                                        true
                                    } else {
                                        manual_override[*i2].unwrap_or(auto_sel[*i2])
                                    }
                                })
                                .map(|(_, (_, d))| d.saldo_actual())
                                .sum();
                            if nuevo_total > monto_prestamo + 0.01 {
                                println!();
                                println!(
                                    "  ⚠️  Agregar '{}' excedería el préstamo en ${}.",
                                    deudas_activas[idx].1.nombre,
                                    format!("{:.2}", nuevo_total - monto_prestamo).red()
                                );
                                println!("  Para incluirla debes excluir otra deuda primero.");
                                pausa();
                                continue;
                            }
                        }
                        manual_override[idx] = if Some(nuevo_estado) == Some(auto_sel[idx])
                            && manual_override[idx].is_some()
                        {
                            None // volver al estado automático
                        } else {
                            Some(nuevo_estado)
                        };
                    }
                }
            }
            Some(1) => {
                // Análisis comparativo completo
                mostrar_analisis_consolidacion(
                    &deudas_activas,
                    &va,
                    monto_prestamo,
                    pago_mensual_real,
                    meses_prest,
                    tasa_anual_prest,
                    total_real,
                    interes_real,
                );
                pausa();
            }
            Some(2) => {
                // Aceptar: agregar el préstamo al rastreador y marcar las cubiertas como pagadas
                let saldo_final: f64 = deudas_activas
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| va[*i])
                    .map(|(_, (_, d))| d.saldo_actual())
                    .sum();

                if saldo_final > monto_prestamo + 0.01 {
                    println!();
                    println!(
                        "  ⚠️  La selección excede el monto del préstamo. Ajusta antes de aceptar."
                    );
                    pausa();
                    continue;
                }

                // Nombre del préstamo
                let nombre_prest =
                    pedir_texto("Nombre del préstamo de consolidación (ej: Personal Loan BOFA)")
                        .unwrap_or_else(|| "Préstamo Consolidación".to_string());

                let mes_hoy = chrono::Local::now().format("%Y-%m").to_string();

                // Recolectar los datos necesarios ANTES de mutar (evitar borrow conflict)
                let a_liquidar: Vec<(usize, f64, String)> = deudas_activas
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| va[*i])
                    .map(|(_, (idx_real, d))| (*idx_real, d.saldo_actual(), d.nombre.clone()))
                    .filter(|(_, saldo, _)| *saldo >= 0.01)
                    .collect();

                // Registrar pago de liquidación en las deudas cubiertas
                for (idx_real, saldo, nombre) in &a_liquidar {
                    state.asesor.rastreador.deudas[*idx_real]
                        .registrar_mes_con_escrow(&mes_hoy, *saldo, *saldo, 0.0, 0.0);
                    state.asesor.rastreador.deudas[*idx_real].activa = false;
                    println!(
                        "  {} '{}' marcada como liquidada (${:.2})",
                        "✓".green(),
                        nombre,
                        saldo
                    );
                }

                // Agregar el nuevo préstamo de consolidación
                let mut nueva_deuda =
                    DeudaRastreada::nueva(&nombre_prest, tasa_anual_prest, pago_mensual_real);
                nueva_deuda.obligatoria = true;
                nueva_deuda.principal_interes_mensual = pago_mensual_real;
                nueva_deuda.saldo_inicial = monto_prestamo;
                nueva_deuda.registrar_mes(&mes_hoy, monto_prestamo, 0.0, 0.0);

                state.asesor.rastreador.agregar_deuda(nueva_deuda);
                let _ = state.guardar();

                println!();
                println!(
                    "  {} Préstamo '{}' agregado al rastreador.",
                    "✅".green(),
                    nombre_prest.cyan().bold()
                );
                println!(
                    "     ${:.2} al {:.2}% APR — ${:.2}/mes × {} meses",
                    monto_prestamo, tasa_anual_prest, pago_mensual_real, meses_prest
                );
                pausa();
                return;
            }
            _ => return,
        }
    }
}

/// Muestra el análisis financiero completo de la consolidación.
#[allow(clippy::too_many_arguments)]
fn mostrar_analisis_consolidacion(
    deudas_activas: &[(usize, &DeudaRastreada)],
    va: &[bool],
    monto_prestamo: f64,
    pago_mensual_prest: f64,
    meses_prest: u32,
    tasa_anual_prest: f64,
    total_real: f64,
    interes_real: f64,
) {
    let _ = total_real; // presente en firma para contexto futuro
    limpiar();
    println!(
        "{}",
        "╔══════════════════════════════════════════════════════════════════╗".cyan()
    );
    println!(
        "{}",
        "║  📊 ANÁLISIS COMPARATIVO — ¿Conviene consolidar?               ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════════════╝".cyan()
    );
    println!();

    // ── Deudas que SE VAN ─────────────────────────────────────────────────
    println!(
        "  {}",
        "🟢 DEUDAS QUE SE LIQUIDARÍAN (cubiertas por el préstamo):"
            .green()
            .bold()
    );
    println!(
        "  {:<24} {:>10} {:>7} {:>10} {:>10}",
        "Cuenta", "Saldo", "Tasa%", "Pago/mes", "Int/mes"
    );
    println!("  {}", "─".repeat(67));

    let mut saldo_cubierto = 0.0f64;
    let mut pago_mensual_actual_cubierto = 0.0f64;
    let mut interes_mensual_cubierto = 0.0f64;
    let mut interes_total_actual_cubierto = 0.0f64;
    let mut meses_max_cubierto = 0u32;

    for (i, (_, d)) in deudas_activas.iter().enumerate() {
        if !va[i] {
            continue;
        }
        let saldo = d.saldo_actual();
        let tasa_m = d.tasa_anual / 100.0 / 12.0;
        let int_mes = saldo * tasa_m;
        let pago_mes = d.pago_total_mensual();
        let (meses_d, int_total_d, _) = simular_pagos_simple(saldo, tasa_m, pago_mes);

        saldo_cubierto += saldo;
        pago_mensual_actual_cubierto += pago_mes;
        interes_mensual_cubierto += int_mes;
        interes_total_actual_cubierto += int_total_d;
        meses_max_cubierto = meses_max_cubierto.max(meses_d);

        println!(
            "  {:<24} {:>10} {:>6.1}% {:>10} {:>10}",
            if d.nombre.len() > 23 {
                format!("{}…", &d.nombre[..22])
            } else {
                d.nombre.clone()
            },
            format!("${:.2}", saldo).green(),
            d.tasa_anual,
            format!("${:.2}", pago_mes),
            format!("${:.2}", int_mes).red()
        );
    }
    println!("  {}", "─".repeat(67));
    println!(
        "  {:<24} {:>10} {:>7} {:>10} {:>10}",
        "SUBTOTAL",
        format!("${:.2}", saldo_cubierto).green().bold(),
        "",
        format!("${:.2}", pago_mensual_actual_cubierto)
            .yellow()
            .bold(),
        format!("${:.2}", interes_mensual_cubierto).red()
    );
    println!();

    // ── Deudas que SE QUEDAN ──────────────────────────────────────────────
    let deudas_quedan: Vec<_> = deudas_activas
        .iter()
        .enumerate()
        .filter(|(i, _)| !va[*i])
        .collect();

    if !deudas_quedan.is_empty() {
        println!(
            "  {}",
            "🔴 DEUDAS QUE PERMANECEN (no cubiertas):".red().bold()
        );
        println!(
            "  {:<24} {:>10} {:>7} {:>10}",
            "Cuenta", "Saldo", "Tasa%", "Pago/mes"
        );
        println!("  {}", "─".repeat(55));
        let mut saldo_queda = 0.0f64;
        let mut pago_queda = 0.0f64;
        for (_, (_, d)) in &deudas_quedan {
            let saldo = d.saldo_actual();
            saldo_queda += saldo;
            pago_queda += d.pago_total_mensual();
            println!(
                "  {:<24} {:>10} {:>6.1}% {:>10}",
                if d.nombre.len() > 23 {
                    format!("{}…", &d.nombre[..22])
                } else {
                    d.nombre.clone()
                },
                format!("${:.2}", saldo).red(),
                d.tasa_anual,
                format!("${:.2}", d.pago_total_mensual())
            );
        }
        println!("  {}", "─".repeat(55));
        println!(
            "  {:<24} {:>10} {:>7} {:>10}",
            "SUBTOTAL QUEDA",
            format!("${:.2}", saldo_queda).red().bold(),
            "",
            format!("${:.2}", pago_queda).yellow()
        );
        println!();
    }

    // ── Comparativa SIN vs CON consolidación ─────────────────────────────
    println!("  {}", "═".repeat(70));
    println!(
        "  {}",
        "⚖️  COMPARATIVA: SIN consolidación  vs  CON consolidación".bold()
    );
    println!("  {}", "═".repeat(70));
    println!(
        "  {:<38} {:>14} {:>14}",
        "", "SIN consolid.", "CON consolid."
    );
    println!("  {}", "─".repeat(70));

    let tasa_prom_actual = if saldo_cubierto > 0.01 {
        deudas_activas
            .iter()
            .enumerate()
            .filter(|(i, _)| va[*i])
            .map(|(_, (_, d))| d.tasa_anual * d.saldo_actual())
            .sum::<f64>()
            / saldo_cubierto
    } else {
        0.0
    };

    println!(
        "  {:<38} {:>14} {:>14}",
        "Tasa promedio ponderada:",
        format!("{:.2}%", tasa_prom_actual).red(),
        format!("{:.2}%", tasa_anual_prest).green()
    );
    println!(
        "  {:<38} {:>14} {:>14}",
        "Saldo total cubierto:",
        format!("${:.2}", saldo_cubierto),
        format!("${:.2}", monto_prestamo)
    );
    println!(
        "  {:<38} {:>14} {:>14}",
        "Pago mensual (deudas cubiertas):",
        format!("${:.2}", pago_mensual_actual_cubierto).yellow(),
        format!("${:.2}", pago_mensual_prest).cyan().bold()
    );

    let flujo_liberado = pago_mensual_actual_cubierto - pago_mensual_prest;
    let flujo_str = if flujo_liberado >= 0.0 {
        format!("+${:.2}", flujo_liberado)
            .green()
            .bold()
            .to_string()
    } else {
        format!("-${:.2}", flujo_liberado.abs())
            .red()
            .bold()
            .to_string()
    };
    println!(
        "  {:<38} {:>14} {:>14}",
        "Flujo mensual liberado/extra:", "", flujo_str
    );
    println!(
        "  {:<38} {:>14} {:>14}",
        "Meses para liquidar (deudas cub.):",
        format!("{}", meses_max_cubierto).yellow(),
        format!("{}", meses_prest).cyan()
    );
    println!(
        "  {:<38} {:>14} {:>14}",
        "Total intereses pagados:",
        format!("${:.2}", interes_total_actual_cubierto)
            .red()
            .bold(),
        format!("${:.2}", interes_real).yellow()
    );
    println!("  {}", "─".repeat(70));

    let ahorro_intereses = interes_total_actual_cubierto - interes_real;
    let (ahorro_str, veredicto) = if ahorro_intereses > 0.0 {
        (
            format!("+${:.2} ahorras en intereses", ahorro_intereses)
                .green()
                .bold()
                .to_string(),
            true,
        )
    } else {
        (
            format!("-${:.2} pagas más en intereses", ahorro_intereses.abs())
                .red()
                .bold()
                .to_string(),
            false,
        )
    };
    println!(
        "  {:<38} {:>29}",
        "Diferencia neta en intereses:", ahorro_str
    );
    println!();

    // ── Veredicto final ───────────────────────────────────────────────────
    println!("  {}", "═".repeat(70));
    if veredicto && flujo_liberado >= 0.0 {
        println!("  {}", "✅ CONVIENE CONSOLIDAR".green().bold());
        println!(
            "     Ahorras {} en intereses totales y liberas {} /mes en flujo.",
            format!("${:.2}", ahorro_intereses).green().bold(),
            format!("${:.2}", flujo_liberado).green()
        );
    } else if veredicto && flujo_liberado < 0.0 {
        println!(
            "  {}",
            "⚠️  CONVIENE EN INTERESES, PERO PAGO MENSUAL SUBE"
                .yellow()
                .bold()
        );
        println!(
            "     Ahorras {} en intereses pero pagas {} más por mes.",
            format!("${:.2}", ahorro_intereses).green(),
            format!("${:.2}", flujo_liberado.abs()).red()
        );
        println!(
            "     Asegúrate de que tu presupuesto aguante ${:.2}/mes extra.",
            flujo_liberado.abs()
        );
    } else if !veredicto && flujo_liberado >= 0.0 {
        println!(
            "  {}",
            "⚠️  LIBERA FLUJO PERO PAGAS MÁS INTERESES EN TOTAL"
                .yellow()
                .bold()
        );
        println!(
            "     Pagas {} más en intereses pero tu pago mensual baja ${:.2}.",
            format!("${:.2}", ahorro_intereses.abs()).red(),
            flujo_liberado
        );
        println!("     Considera solo si el flujo mensual es tu prioridad urgente.");
    } else {
        println!("  {}", "❌ NO CONVIENE CONSOLIDAR".red().bold());
        println!(
            "     Pagarías {} más en intereses Y tu pago mensual sube.",
            format!("${:.2}", ahorro_intereses.abs()).red().bold()
        );
        println!("     Busca una tasa más baja o un plazo más corto.");
    }
    println!("  {}", "═".repeat(70));
    println!();

    // Tip: qué hacer con el dinero liberado
    if flujo_liberado > 10.0 {
        println!(
            "  💡 Con ${:.2}/mes liberados, si los aplicas a deudas que quedan:",
            flujo_liberado
        );
        for (_, (_, d)) in &deudas_quedan {
            let saldo = d.saldo_actual();
            let tasa_m = d.tasa_anual / 100.0 / 12.0;
            let pago_extra = d.pago_total_mensual() + flujo_liberado;
            let (meses_sin, _, _) = simular_pagos_simple(saldo, tasa_m, d.pago_total_mensual());
            let (meses_con, _, _) = simular_pagos_simple(saldo, tasa_m, pago_extra);
            if meses_sin > meses_con {
                println!(
                    "     '{}': {} → {} meses (−{} meses)",
                    d.nombre,
                    meses_sin,
                    meses_con,
                    meses_sin - meses_con
                );
            }
        }
        println!();
    }
}

pub fn rastreador_exportar(state: &AppState) {
    if state.asesor.rastreador.deudas.is_empty() {
        println!("  Sin deudas registradas.");
        pausa();
        return;
    }

    let opciones = &[
        "📊  Exportar resumen global (todas las deudas)",
        "📋  Exportar historial de una deuda",
        "🔙  Cancelar",
    ];

    match menu("¿Qué exportar?", opciones) {
        Some(0) => {
            let csv = state.asesor.rastreador.csv_resumen_global();
            let dir = omniplanner::ml::advisor::AlmacenAsesor::dir_exportacion();
            let ruta = dir.join("rastreador_resumen.csv");
            match std::fs::write(&ruta, &csv) {
                Ok(()) => {
                    println!();
                    println!("  ✅ CSV exportado: {}", ruta.display().to_string().green());
                }
                Err(e) => println!("  {} Error: {}", "✗".red(), e),
            }
            pausa();
        }
        Some(1) => {
            let nombres: Vec<String> = state
                .asesor
                .rastreador
                .deudas
                .iter()
                .map(|d| format!("{} ({} meses)", d.nombre, d.historial.len()))
                .collect();
            let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

            if let Some(idx) = menu("¿Cuál deuda exportar?", &refs) {
                let nombre = &state.asesor.rastreador.deudas[idx].nombre;
                let csv = state.asesor.rastreador.csv_historial_deuda(nombre);
                let dir = omniplanner::ml::advisor::AlmacenAsesor::dir_exportacion();
                let archivo = format!(
                    "rastreador_{}.csv",
                    nombre
                        .chars()
                        .filter(|c| c.is_alphanumeric() || *c == ' ')
                        .collect::<String>()
                        .replace(' ', "_")
                );
                let ruta = dir.join(archivo);
                match std::fs::write(&ruta, &csv) {
                    Ok(()) => {
                        println!();
                        println!("  ✅ CSV exportado: {}", ruta.display().to_string().green());
                    }
                    Err(e) => println!("  {} Error: {}", "✗".red(), e),
                }
                pausa();
            }
        }
        _ => {}
    }
}

pub fn rastreador_importar_csv(state: &mut AppState) {
    limpiar();
    separador("📂 IMPORTAR DEUDAS");

    println!("  📋 Arrastra tu archivo Excel (.xlsx) o CSV aquí:");
    println!("  💡 También puedes escribir la ruta manualmente.");
    println!();

    let ruta = match pedir_texto("Ruta del archivo (arrastra aquí)") {
        Some(r) => {
            // Limpiar formato de arrastrar en Windows: & 'ruta' → ruta
            let limpio = r.trim();
            let limpio = limpio.strip_prefix("& ").unwrap_or(limpio);
            let limpio = limpio.trim_matches('\'').trim_matches('"').trim();
            limpio.to_string()
        }
        None => return,
    };

    // Si es Excel, convertir automáticamente con Python
    let csv_path =
        if ruta.to_lowercase().ends_with(".xlsx") || ruta.to_lowercase().ends_with(".xls") {
            println!();
            println!("  🔄 Detectado archivo Excel. Convirtiendo a CSV...");

            // Ruta temporal para el CSV generado
            let csv_temp = std::env::temp_dir().join("omniplanner_import.csv");

            // Buscar el script de conversión
            let script = if let Ok(exe) = std::env::current_exe() {
                let base = exe
                    .parent()
                    .and_then(|p| p.parent())
                    .and_then(|p| p.parent())
                    .unwrap_or_else(|| std::path::Path::new("."));
                let s = base.join("tools").join("excel_a_csv.py");
                if s.exists() {
                    s
                } else {
                    std::path::PathBuf::from("tools").join("excel_a_csv.py")
                }
            } else {
                std::path::PathBuf::from("tools").join("excel_a_csv.py")
            };

            // Intentar varias ubicaciones del script
            let script_path = if script.exists() {
                script
            } else {
                // Intentar relativo al directorio de trabajo
                let cwd_script = std::path::PathBuf::from("tools").join("excel_a_csv.py");
                if cwd_script.exists() {
                    cwd_script
                } else {
                    // Ruta absoluta del proyecto
                    std::path::PathBuf::from(
                        r"C:\Users\elxav\proyectos\omniplanner\tools\excel_a_csv.py",
                    )
                }
            };

            if !script_path.exists() {
                println!(
                    "  {} No se encontró el script de conversión: {}",
                    "✗".red(),
                    script_path.display()
                );
                println!("  Asegúrate de que existe: tools/excel_a_csv.py");
                pausa();
                return;
            }

            let resultado = std::process::Command::new("python")
                .arg(&script_path)
                .arg(&ruta)
                .arg(csv_temp.to_str().unwrap_or("omniplanner_import.csv"))
                .output();

            match resultado {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);

                    if !stdout.is_empty() {
                        for line in stdout.lines() {
                            println!("    {}", line);
                        }
                    }

                    if !output.status.success() {
                        println!("  {} Error al convertir Excel:", "✗".red());
                        if !stderr.is_empty() {
                            for line in stderr.lines() {
                                println!("    {}", line);
                            }
                        }
                        pausa();
                        return;
                    }

                    if !csv_temp.exists() {
                        println!("  {} No se generó el archivo CSV.", "✗".red());
                        pausa();
                        return;
                    }

                    println!("  ✅ Conversión exitosa.");
                    csv_temp.to_string_lossy().to_string()
                }
                Err(e) => {
                    println!("  {} No se pudo ejecutar Python: {}", "✗".red(), e);
                    println!("  Asegúrate de tener Python instalado con: pip install openpyxl");
                    pausa();
                    return;
                }
            }
        } else {
            ruta
        };

    println!();

    match omniplanner::ml::advisor::RastreadorDeudas::importar_csv(&csv_path) {
        Ok(importado) => {
            let n_deudas = importado.deudas.len();
            let n_meses: usize = importado.deudas.iter().map(|d| d.historial.len()).sum();

            println!();
            println!(
                "  ✅ Importación exitosa: {} cuentas, {} registros",
                n_deudas, n_meses
            );
            println!();

            // Mostrar resumen de lo importado
            for d in &importado.deudas {
                let si = d.historial.first().map(|m| m.saldo_inicio).unwrap_or(0.0);
                let sf = d.saldo_actual();
                let tendencia = if sf > si + 100.0 {
                    "📈 Creció".red().to_string()
                } else if sf < si * 0.5 {
                    "📉 Bajó mucho".green().to_string()
                } else {
                    "➡️ Estable".to_string()
                };
                println!(
                    "    {:<20} ${:>10.2} → ${:>10.2}  ({} meses) {}",
                    d.nombre,
                    si,
                    sf,
                    d.historial.len(),
                    tendencia
                );
            }
            println!();

            if !state.asesor.rastreador.deudas.is_empty() {
                let opciones_merge = &[
                    "🔄  Reemplazar todo (borrar datos actuales)",
                    "➕  Agregar a las existentes (merge)",
                    "❌  Cancelar",
                ];
                match menu(
                    "Ya tienes deudas en el rastreador. ¿Qué hacer?",
                    opciones_merge,
                ) {
                    Some(0) => {
                        state.asesor.rastreador = importado;
                        println!("  {} Datos reemplazados.", "✓".green());
                    }
                    Some(1) => {
                        for d in importado.deudas {
                            // Si ya existe una deuda con el mismo nombre, reemplazarla
                            if let Some(pos) = state
                                .asesor
                                .rastreador
                                .deudas
                                .iter()
                                .position(|x| x.nombre == d.nombre)
                            {
                                state.asesor.rastreador.deudas[pos] = d;
                            } else {
                                state.asesor.rastreador.deudas.push(d);
                            }
                        }
                        println!("  {} Datos combinados.", "✓".green());
                    }
                    _ => {
                        println!("  Importación cancelada.");
                    }
                }
            } else {
                state.asesor.rastreador = importado;
                println!("  {} Listo. Ahora puedes ver el diagnóstico.", "✓".green());
            }

            println!();
            println!("  💡 Tip: Ajusta las tasas de interés de cada cuenta");
            println!("    para un diagnóstico más preciso.");
        }
        Err(e) => {
            println!();
            println!("  {} Error: {}", "✗".red(), e);
        }
    }
    pausa();
}

pub fn rastreador_gestionar_deudas(state: &mut AppState) {
    loop {
        limpiar();
        separador("🔀 GESTIONAR DEUDAS");

        if state.asesor.rastreador.deudas.is_empty() {
            println!("  Sin deudas registradas.");
            pausa();
            return;
        }

        // Mostrar tabla con estado actual
        println!(
            "  {:<4} {:<25} {:>10} {:>8} {:>10}  Estado",
            "#", "Deuda", "Saldo", "Tasa%", "Pago mín"
        );
        println!("  {}", "─".repeat(78));

        for (i, d) in state.asesor.rastreador.deudas.iter().enumerate() {
            let estado = if !d.activa {
                "⏸️  INACTIVA".to_string()
            } else if d.es_pago_corriente() {
                "🔒 Corriente".to_string()
            } else if d.obligatoria {
                "🔒 Obligatoria".to_string()
            } else {
                "📋 Normal".to_string()
            };

            let nombre_corto = if d.nombre.len() > 24 {
                format!("{}…", &d.nombre[..23])
            } else {
                d.nombre.clone()
            };

            let saldo_str = if d.es_pago_corriente() {
                "corriente".to_string()
            } else {
                format!("${:.2}", d.saldo_actual())
            };

            println!(
                "  {:<4} {:<25} {:>10} {:>7}% {:>10}  {}",
                format!("{}.", i + 1),
                nombre_corto,
                saldo_str,
                format!("{:.1}", d.tasa_anual),
                format!("${:.2}", d.pago_total_mensual()),
                estado
            );
        }
        println!("  {}", "─".repeat(78));
        println!();

        let acciones = &[
            "⏸️   Activar / Desactivar una deuda (excluir de simulación)",
            "🔒  Cambiar a Obligatoria / Normal (prioridad de pago)",
            "🔙  Volver",
        ];

        match menu("¿Qué quieres hacer?", acciones) {
            Some(0) => {
                // Toggle activa/inactiva
                let nombres: Vec<String> = state
                    .asesor
                    .rastreador
                    .deudas
                    .iter()
                    .enumerate()
                    .map(|(i, d)| {
                        let estado = if d.activa { "ACTIVA" } else { "INACTIVA" };
                        format!(
                            "{}. {} — ${:.2} [{}]",
                            i + 1,
                            d.nombre,
                            d.saldo_actual(),
                            estado
                        )
                    })
                    .collect();
                let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

                if let Some(idx) = menu("¿Cuál deuda cambiar?", &refs) {
                    let d = &mut state.asesor.rastreador.deudas[idx];
                    let nuevo_estado = !d.activa;
                    let accion = if nuevo_estado {
                        "ACTIVADA ✅"
                    } else {
                        "DESACTIVADA ⏸️"
                    };
                    d.activa = nuevo_estado;
                    println!();
                    println!("  {} '{}' ahora está {}", "✓".green(), d.nombre, accion);
                    if !nuevo_estado {
                        println!(
                            "  {}",
                            "  → No aparecerá en simulaciones ni diagnósticos.".dimmed()
                        );
                    }
                    state.guardar().ok();
                    pausa();
                }
            }
            Some(1) => {
                // Toggle obligatoria/normal
                let deudas_no_corrientes: Vec<(usize, String)> = state
                    .asesor
                    .rastreador
                    .deudas
                    .iter()
                    .enumerate()
                    .filter(|(_, d)| !d.es_pago_corriente())
                    .map(|(i, d)| {
                        let tipo = if d.obligatoria {
                            "🔒 OBLIGATORIA"
                        } else {
                            "📋 Normal"
                        };
                        (
                            i,
                            format!("{} — ${:.2} [{}]", d.nombre, d.saldo_actual(), tipo),
                        )
                    })
                    .collect();

                if deudas_no_corrientes.is_empty() {
                    println!("  No hay deudas editables (solo pagos corrientes).");
                    pausa();
                    continue;
                }

                let labels: Vec<&str> = deudas_no_corrientes
                    .iter()
                    .map(|(_, s)| s.as_str())
                    .collect();

                if let Some(sel) = menu("¿Cuál deuda cambiar?", &labels) {
                    let real_idx = deudas_no_corrientes[sel].0;
                    let d = &mut state.asesor.rastreador.deudas[real_idx];
                    let nueva_prioridad = !d.obligatoria;
                    let accion = if nueva_prioridad {
                        "OBLIGATORIA 🔒 (se paga primero en simulación)"
                    } else {
                        "NORMAL 📋 (participa en avalancha/bola de nieve)"
                    };
                    d.obligatoria = nueva_prioridad;
                    println!();
                    println!("  {} '{}' ahora es {}", "✓".green(), d.nombre, accion);
                    state.guardar().ok();
                    pausa();
                }
            }
            _ => return,
        }
    }
}

pub fn rastreador_eliminar(state: &mut AppState) {
    if state.asesor.rastreador.deudas.is_empty() {
        println!("  Sin deudas registradas.");
        pausa();
        return;
    }

    let nombres: Vec<String> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .map(|d| format!("{} — ${:.2}", d.nombre, d.saldo_actual()))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    if let Some(idx) = menu("¿Cuál deuda eliminar?", &refs) {
        let nombre = state.asesor.rastreador.deudas[idx].nombre.clone();
        if TermConfirm::new()
            .with_prompt(format!(
                "  ¿Eliminar '{}'? Se perderá todo el historial.",
                nombre
            ))
            .default(false)
            .interact()
            .unwrap_or(false)
        {
            state.asesor.rastreador.deudas.remove(idx);
            println!("  {} '{}' eliminada", "✓".green(), nombre);
        }
    }
    pausa();
}

/// Muestra un análisis de ahorro (meses e intereses) al aplicar `extra`
/// dólares extra por mes a una deuda, y compara contra el resto de deudas
/// activas para sugerir si ese extra rendiría más en otra deuda.
fn mostrar_analisis_ahorro_pago_extra(rastreador: &RastreadorDeudas, idx: usize, extra: f64) {
    let deuda = match rastreador.deudas.get(idx) {
        Some(d) => d,
        None => return,
    };
    let ahorro_actual = match deuda.ahorro_por_pago_extra(extra) {
        Some(a) => a,
        None => return,
    };

    println!();
    println!(
        "  {} Análisis de pago extra (+${:.2}/mes)",
        "💡".cyan(),
        extra
    );
    println!(
        "    · En '{}': liquidas en {} meses en vez de {} ({} meses antes)",
        deuda.nombre.cyan(),
        ahorro_actual.meses_con_extra,
        ahorro_actual.meses_base,
        ahorro_actual.meses_ahorrados
    );
    println!(
        "    · Ahorro en intereses: ${:.2} ({:.1}%)",
        ahorro_actual.intereses_ahorrados,
        ahorro_actual.porcentaje_intereses_ahorrados()
    );

    if let Some(rec) = rastreador.mejor_destino_pago_extra(extra) {
        if rec.nombre_deuda != deuda.nombre
            && rec.ahorro.intereses_ahorrados > ahorro_actual.intereses_ahorrados + 1.0
        {
            let diff = rec.ahorro.intereses_ahorrados - ahorro_actual.intereses_ahorrados;
            println!();
            println!(
                "  {} Mejor opción: aplicar ese ${:.2}/mes a '{}'",
                "🎯".yellow(),
                extra,
                rec.nombre_deuda.yellow()
            );
            println!(
                "    · Ahí ahorrarías ${:.2} en intereses ({:.1}%) — ${:.2} más que en '{}'",
                rec.ahorro.intereses_ahorrados,
                rec.ahorro.porcentaje_intereses_ahorrados(),
                diff,
                deuda.nombre
            );
            println!(
                "    · Y liquidarías esa deuda {} meses antes",
                rec.ahorro.meses_ahorrados
            );
        }

        if rec.ranking.len() > 1 {
            println!();
            println!("  Ranking de ahorro con +${:.2}/mes:", extra);
            for (i, (nombre, a)) in rec.ranking.iter().take(3).enumerate() {
                let nombre_corto = if nombre.len() > 24 {
                    &nombre[..24]
                } else {
                    nombre.as_str()
                };
                println!(
                    "    {}. {:<24} ahorra ${:>8.2} ({:>5.1}%) · {} meses antes",
                    i + 1,
                    nombre_corto,
                    a.intereses_ahorrados,
                    a.porcentaje_intereses_ahorrados(),
                    a.meses_ahorrados
                );
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Editor del plan de libertad financiera (estilo Excel)
// ═══════════════════════════════════════════════════════════════════

/// Resultado del editor: el usuario decide explícitamente cómo salir.
pub(crate) enum SalidaEditorPlan {
    /// Exportó a Excel (el borrador se limpia).
    Exportado,
    /// Guardó borrador para continuar después (persiste en `AlmacenAsesor`).
    BorradorGuardado,
    /// Descartó el plan explícitamente (borrador limpiado).
    Descartado,
    /// Salió sin cambios o sin tocar borrador.
    SinCambios,
}

/// Permite al usuario construir el plan mes a mes, sin perder trabajo:
///   - Cada edición se recalcula en vivo, sin destruir ajustes previos.
///   - Vista enfocada mes por mes para trabajar incrementalmente.
///   - Borrador persistente: al volver a entrar se reanuda el trabajo.
///   - La simulación sólo "se cierra" cuando el usuario exporta a Excel
///     o descarta el borrador explícitamente.
fn editor_plan_libertad(
    state: &mut AppState,
    base: SimulacionLibertad,
    presupuesto: f64,
    nombres_deudas: &[String],
) -> SalidaEditorPlan {
    // ── Recuperar borrador si existe y el presupuesto coincide ──
    let (mut estrategia, mut ajustes, ediciones_previas) = match state.asesor.borrador_plan.as_ref()
    {
        Some(b) if (b.presupuesto - presupuesto).abs() < 0.01 => {
            println!();
            println!(
                "  {} Borrador previo detectado: {} ajuste(s), última edición {} (#{} ediciones).",
                "📂".cyan().bold(),
                b.ajustes.len(),
                b.actualizado_en,
                b.ediciones
            );
            let reanudar = TermConfirm::new()
                .with_prompt("  ¿Reanudar el borrador?")
                .default(true)
                .interact()
                .unwrap_or(true);
            if reanudar {
                (b.estrategia.clone(), b.ajustes.clone(), b.ediciones)
            } else {
                let descartar = TermConfirm::new()
                    .with_prompt("  ¿Eliminar el borrador guardado?")
                    .default(false)
                    .interact()
                    .unwrap_or(false);
                if descartar {
                    state.asesor.borrador_plan = None;
                }
                let est_inicial = if base.estrategia == "Bola de nieve" {
                    EstrategiaLibertad::BolaNieve
                } else {
                    EstrategiaLibertad::Avalancha
                };
                (est_inicial, Vec::new(), 0)
            }
        }
        _ => {
            let est_inicial = if base.estrategia == "Bola de nieve" {
                EstrategiaLibertad::BolaNieve
            } else {
                EstrategiaLibertad::Avalancha
            };
            (est_inicial, Vec::new(), 0)
        }
    };

    let base_snapshot = base.clone();
    let rastreador = state.asesor.rastreador.clone();
    let mut sim = rastreador.simular_libertad_editado(presupuesto, &estrategia, &ajustes);
    let mut ediciones: u32 = ediciones_previas;
    let mut dirty = false; // hay cambios no guardados respecto al borrador persistido

    // Helper cerrado: guarda borrador cuando se marca dirty.
    let fecha_ahora = || chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();

    loop {
        limpiar();
        separador("✏️  EDITOR DEL PLAN DE LIBERTAD");
        println!();
        println!(
            "  Estrategia: {} | Presupuesto para deudas: ${:.2}/mes",
            estrategia.nombre().cyan().bold(),
            presupuesto
        );
        println!(
            "  Meses hasta libertad: {} | Total intereses: ${:.2}",
            sim.meses.len().to_string().yellow().bold(),
            sim.total_intereses
        );
        if !ajustes.is_empty() {
            println!(
                "  Ajustes manuales activos: {} | Ediciones: {}",
                ajustes.len().to_string().magenta().bold(),
                ediciones.to_string().magenta()
            );
        }
        if sim.meses_con_descubierto > 0 {
            println!(
                "  {} {} mes(es) con mínimos descubiertos (falta ${:.2}).",
                "⚠️".red().bold(),
                sim.meses_con_descubierto,
                sim.minimos_no_cubiertos_total
            );
        }
        let estado_borrador = if dirty {
            "🟡 cambios sin guardar".yellow().to_string()
        } else if ediciones > 0 {
            "🟢 borrador al día".green().to_string()
        } else {
            "sin cambios".dimmed().to_string()
        };
        println!("  Estado: {}", estado_borrador);
        println!();

        let opcion = menu(
            "¿Qué deseas hacer?",
            &[
                "📋 Ver tabla mes × deuda (como Excel)",
                "🎯 Enfocar un mes específico (trabajo fino)",
                "🔀 Cambiar estrategia (Avalancha / Bola de nieve / Pesos)",
                "↔️  Mover recursos entre deudas en un mes",
                "📌 Fijar pago a una deuda en un mes",
                "⏩ Acumular cuotas en un mes (con cobertura sugerida)",
                "💉 Inyectar pagos programados al plan",
                "🧹 Quitar todos los ajustes manuales",
                "🆚 Comparar contra plan automático original",
                "💾 Guardar borrador y salir (se reanuda luego)",
                "📤 EXPORTAR a Excel (cierra el plan)",
                "💰 Registrar ingreso extra en un mes",
                "🗑️  Descartar borrador y salir",
            ],
        );

        match opcion {
            Some(0) => mostrar_tabla_plan_libertad(&sim),
            Some(1) => {
                if mes_focus(
                    &rastreador,
                    &mut sim,
                    &mut ajustes,
                    &estrategia,
                    presupuesto,
                ) {
                    ediciones += 1;
                    dirty = true;
                }
            }
            Some(2) => {
                if let Some(nueva) = elegir_estrategia(&rastreador) {
                    estrategia = nueva;
                    sim = rastreador.simular_libertad_editado(presupuesto, &estrategia, &ajustes);
                    ediciones += 1;
                    dirty = true;
                }
            }
            Some(3) => {
                if mover_recursos_entre_deudas(&sim, &mut ajustes, None) {
                    sim = rastreador.simular_libertad_editado(presupuesto, &estrategia, &ajustes);
                    ediciones += 1;
                    dirty = true;
                }
            }
            Some(4) => {
                if fijar_pago_en_mes(&rastreador, &sim, &mut ajustes, None) {
                    sim = rastreador.simular_libertad_editado(presupuesto, &estrategia, &ajustes);
                    ediciones += 1;
                    dirty = true;
                }
            }
            Some(5) => {
                if acumular_pagos_deuda(&rastreador, &sim, &mut ajustes, None) {
                    sim = rastreador.simular_libertad_editado(presupuesto, &estrategia, &ajustes);
                    ediciones += 1;
                    dirty = true;
                }
            }
            Some(6) => {
                // ─── INYECTAR PAGOS PROGRAMADOS AL PLAN ──────────────────────
                // Flujo:
                //   1. Leer pagos_programados válidos para deudas activas
                //   2. Comparar con lo que el plan ya asigna → calcular DIFF
                //   3. Mostrar DIFF al usuario y confirmar
                //   4. Aplicar solo los cambios reales, re-simular
                //   5. Mostrar resumen de redistribución y ofrecer ajuste manual
                let fn_fecha_valida = |s: &str| -> bool {
                    let p: Vec<&str> = s.splitn(2, '-').collect();
                    p.len() == 2
                        && p[0].len() == 4
                        && p[0].chars().all(|c| c.is_ascii_digit())
                        && p[1].parse::<u32>().is_ok_and(|m| (1..=12).contains(&m))
                };

                // Helper: pago que el plan actual asigna a (deuda, YYYY-MM)
                let pago_en_plan = |deuda: &str, yyyy_mm: &str| -> f64 {
                    sim.meses
                        .iter()
                        .find(|m| m.mes_yyyy_mm == yyyy_mm)
                        .and_then(|m| m.pagos.iter().find(|(d, _)| d == deuda))
                        .map(|(_, p)| *p)
                        .unwrap_or(0.0)
                };

                let programados: Vec<&omniplanner::ml::PagoProgramado> = rastreador
                    .pagos_programados
                    .iter()
                    .filter(|pp| {
                        rastreador.deudas.iter().any(|d| {
                            d.nombre == pp.nombre_deuda
                                && d.activa
                                && !d.es_pago_corriente()
                                && d.saldo_actual() > 0.01
                        })
                    })
                    .collect();

                if programados.is_empty() {
                    println!();
                    println!(
                        "  {} No hay pagos programados para deudas activas.",
                        "ℹ️".cyan()
                    );
                    pausa();
                    continue;
                }

                // Advertir fechas inválidas
                let invalidos: Vec<_> = programados
                    .iter()
                    .filter(|pp| !fn_fecha_valida(&pp.fecha_pago_prevista))
                    .copied()
                    .collect();
                if !invalidos.is_empty() {
                    println!();
                    println!(
                        "  {} {} pago(s) con fecha inválida (serán ignorados):",
                        "⚠️".yellow().bold(),
                        invalidos.len()
                    );
                    for pp in &invalidos {
                        println!(
                            "    • {} → \"{}\"  (elimina y recrea con formato YYYY-MM)",
                            pp.nombre_deuda.yellow(),
                            pp.fecha_pago_prevista
                        );
                    }
                    println!();
                }

                // ── PASO 2: Calcular DIFF ─────────────────────────────────────
                // (yyyy_mm, deuda, antes, después, motivo)
                let mut diffs: Vec<(String, String, f64, f64, &'static str)> = Vec::new();

                for pp in &programados {
                    if !fn_fecha_valida(&pp.fecha_pago_prevista) {
                        continue;
                    }
                    if pp.meses_cubiertos.is_empty() {
                        // Sin meses_cubiertos → pago del presupuesto, fijar monto
                        let antes = pago_en_plan(&pp.nombre_deuda, &pp.fecha_pago_prevista);
                        let despues = pp.monto_pi.max(0.0);
                        if (antes - despues).abs() > 0.01 {
                            diffs.push((
                                pp.fecha_pago_prevista.clone(),
                                pp.nombre_deuda.clone(),
                                antes,
                                despues,
                                "pago fijado (presupuesto)",
                            ));
                        }
                    } else {
                        // Con meses_cubiertos → pre-pago de ahorros, todos los meses → $0
                        let mut todos = pp.meses_cubiertos.clone();
                        if !todos.contains(&pp.fecha_pago_prevista) {
                            todos.push(pp.fecha_pago_prevista.clone());
                        }
                        for mes in &todos {
                            if !fn_fecha_valida(mes) {
                                continue;
                            }
                            let antes = pago_en_plan(&pp.nombre_deuda, mes);
                            if antes > 0.01 {
                                diffs.push((
                                    mes.clone(),
                                    pp.nombre_deuda.clone(),
                                    antes,
                                    0.0,
                                    "cubierto por ahorros → $0",
                                ));
                            }
                        }
                    }
                }

                println!();
                if diffs.is_empty() {
                    println!(
                        "  {} El plan ya refleja todos los pagos programados. Sin cambios.",
                        "ℹ️".cyan()
                    );
                    pausa();
                    continue;
                }

                // ── PASO 3: Mostrar DIFF y confirmar ─────────────────────────
                println!(
                    "  {} cambio(s) detectado(s) respecto al plan actual:",
                    diffs.len()
                );
                println!();
                println!(
                    "  {:<12} {:<32} {:>9}  {:>9}  {}",
                    "Mes".bold(),
                    "Deuda".bold(),
                    "Plan hoy".bold(),
                    "Nuevo".bold(),
                    "Motivo".bold()
                );
                println!("  {}", "─".repeat(78));
                for (yyyy_mm, deuda, antes, despues, motivo) in &diffs {
                    let cols = format!("{:>9.2}  {:>9.2}", antes, despues);
                    let cols_col = if *despues < *antes {
                        cols.yellow().to_string()
                    } else {
                        cols.green().to_string()
                    };
                    println!(
                        "  {:<12} {:<32} {}  {}",
                        mes_corto(yyyy_mm),
                        truncar(deuda, 32),
                        cols_col,
                        motivo
                    );
                }
                println!();

                if !confirmar("¿Aplicar estos cambios al plan?", true) {
                    println!("  Cancelado.");
                    pausa();
                    continue;
                }

                // ── PASO 4: Aplicar solo los diffs ───────────────────────────
                let snapshot_antes: Vec<(String, Vec<(String, f64)>)> = sim
                    .meses
                    .iter()
                    .map(|m| (m.mes_yyyy_mm.clone(), m.pagos.clone()))
                    .collect();

                for (yyyy_mm, deuda, _, nuevo_monto, _) in &diffs {
                    ajustes.retain(|a| !(a.mes == *yyyy_mm && a.nombre_deuda == *deuda));
                    ajustes.push(omniplanner::ml::AjusteMensualLibertad::nuevo(
                        yyyy_mm.clone(),
                        deuda.clone(),
                        *nuevo_monto,
                    ));
                }

                sim = rastreador.simular_libertad_editado(presupuesto, &estrategia, &ajustes);
                ediciones += 1;
                dirty = true;

                // ── PASO 5: Resumen de redistribución ────────────────────────
                println!();
                println!("  {} Plan actualizado.", "✅".green().bold());
                println!();

                let meses_afectados: std::collections::BTreeSet<String> =
                    diffs.iter().map(|(m, _, _, _, _)| m.clone()).collect();

                for yyyy_mm in &meses_afectados {
                    let pagos_antes = snapshot_antes
                        .iter()
                        .find(|(m, _)| m == yyyy_mm)
                        .map(|(_, p)| p.as_slice())
                        .unwrap_or(&[]);
                    let mes_nuevo = sim.meses.iter().find(|m| &m.mes_yyyy_mm == yyyy_mm);
                    if let Some(mn) = mes_nuevo {
                        let hay_cambio_redistribucion = mn.pagos.iter().any(|(d, p_nuevo)| {
                            let p_viejo = pagos_antes
                                .iter()
                                .find(|(x, _)| x == d)
                                .map(|(_, v)| *v)
                                .unwrap_or(0.0);
                            (p_viejo - p_nuevo).abs() > 0.01
                        });
                        if hay_cambio_redistribucion {
                            println!("  {} {}:", "📅".cyan(), mes_corto(yyyy_mm).bold());
                            for (deuda, p_nuevo) in &mn.pagos {
                                let p_viejo = pagos_antes
                                    .iter()
                                    .find(|(x, _)| x == deuda)
                                    .map(|(_, v)| *v)
                                    .unwrap_or(0.0);
                                if (p_viejo - p_nuevo).abs() > 0.01 {
                                    let delta = p_nuevo - p_viejo;
                                    let signo = if delta > 0.0 { "+" } else { "" };
                                    println!(
                                        "    {:<32} {:>8.2} → {:>8.2}  ({}{:.2})",
                                        truncar(deuda, 32),
                                        p_viejo,
                                        p_nuevo,
                                        signo,
                                        delta
                                    );
                                }
                            }
                            if mn.sobrante > 0.01 {
                                println!("    {} Sin asignar: ${:.2}", "⚠️".yellow(), mn.sobrante);
                            }
                        }
                    }
                }

                println!();
                if confirmar(
                    "¿Quieres ajustar cuotas en algún mes afectado? (Fijar pago / Mover recursos)",
                    false,
                ) {
                    println!();
                    println!("  → Vuelve al menú y usa 'Fijar pago' o 'Mover recursos'.");
                }

                pausa();
            }
            Some(7) => {
                if !ajustes.is_empty() && confirmar("¿Eliminar todos los ajustes manuales?", false)
                {
                    ajustes.clear();
                    sim = rastreador.simular_libertad_editado(presupuesto, &estrategia, &ajustes);
                    ediciones += 1;
                    dirty = true;
                    println!("  Ajustes eliminados.");
                    pausa();
                }
            }
            Some(8) => mostrar_comparacion_planes(&base_snapshot, &sim),
            Some(9) => {
                // Guardar borrador y salir
                // Preservar mes_inicio si ya existía (no resetear el origen del plan)
                let mes_inicio_actual = state
                    .asesor
                    .borrador_plan
                    .as_ref()
                    .and_then(|b| b.mes_inicio.clone())
                    .or_else(|| Some(chrono::Local::now().format("%Y-%m").to_string()));
                state.asesor.borrador_plan = Some(BorradorPlanLibertad {
                    presupuesto,
                    estrategia: estrategia.clone(),
                    ajustes: ajustes.clone(),
                    actualizado_en: fecha_ahora(),
                    ediciones,
                    mes_inicio: mes_inicio_actual,
                });
                println!();
                println!(
                    "  {} Borrador guardado ({} ajuste(s), {} ediciones). Se reanudará al volver a abrir el plan.",
                    "💾".green().bold(),
                    ajustes.len(),
                    ediciones
                );
                pausa();
                return SalidaEditorPlan::BorradorGuardado;
            }
            Some(10) => {
                // Exportar y cerrar
                match exportar_simulacion_excel(&sim, nombres_deudas) {
                    Ok(ruta) => {
                        state.asesor.borrador_plan = None;
                        println!();
                        println!("  ✅ Reporte exportado a: {}", ruta.green().bold());
                        println!("  El borrador ha sido cerrado (plan finalizado).");
                        // Ofrecer abrir el archivo automáticamente.
                        if confirmar("¿Abrir el archivo Excel ahora?", true) {
                            let _ = open::that(&ruta);
                        }
                        pausa();
                        return SalidaEditorPlan::Exportado;
                    }
                    Err(e) => {
                        println!();
                        println!("  ❌ Error al exportar: {}", e);
                        println!(
                            "  {} Tus ediciones NO se perdieron — sigues en el editor.",
                            "ℹ️".cyan()
                        );
                        pausa();
                    }
                }
            }
            Some(11) => {
                // ─── REGISTRAR / VER INGRESOS EXTRA POR MES ───────────────
                registrar_ingreso_extra_mes(
                    state,
                    &rastreador,
                    &mut sim,
                    presupuesto,
                    &estrategia,
                    &ajustes,
                );
            }
            Some(12) => {
                if dirty || !ajustes.is_empty() {
                    println!();
                    println!(
                        "  {} Descartar elimina {} ajuste(s) y {} edición(es) — ESTO NO SE PUEDE DESHACER.",
                        "⚠️".red().bold(),
                        ajustes.len(),
                        ediciones
                    );
                    let confirmar1 = TermConfirm::new()
                        .with_prompt("  ¿Descartar definitivamente?")
                        .default(false)
                        .interact()
                        .unwrap_or(false);
                    if !confirmar1 {
                        continue;
                    }
                }
                state.asesor.borrador_plan = None;
                println!(
                    "  {} Plan descartado. No hay ajustes pendientes.",
                    "🗑️".red()
                );
                pausa();
                return SalidaEditorPlan::Descartado;
            }
            None => {
                // ESC/cancelación: NO sale silencioso si hay trabajo. Fuerza decisión.
                if !dirty && ediciones == 0 && ajustes.is_empty() {
                    return SalidaEditorPlan::SinCambios;
                }
                println!();
                println!(
                    "  {} Hay {} ajuste(s) y {} edición(es) en curso.",
                    "⚠️".yellow().bold(),
                    ajustes.len(),
                    ediciones
                );
                println!(
                    "  {} Elige explícitamente: 💾 Guardar, 📤 Exportar o 🗑️ Descartar.",
                    "→".cyan()
                );
                pausa();
            }
            _ => {}
        }
    }
}

/// Permite registrar, ver y borrar ingresos extra puntuales por mes.
/// Se usa dentro del editor del plan para reflejar depósitos/bonos puntuales.
fn registrar_ingreso_extra_mes(
    state: &mut AppState,
    _rastreador: &RastreadorDeudas,
    sim: &mut SimulacionLibertad,
    presupuesto: f64,
    estrategia: &EstrategiaLibertad,
    ajustes: &[AjusteMensualLibertad],
) {
    limpiar();
    separador("💰 INGRESOS EXTRA POR MES");
    println!();

    let extras = &state.asesor.rastreador.ingresos_extra;
    if extras.is_empty() {
        println!("  (No hay ingresos extra registrados)");
    } else {
        println!(
            "  {:<12} {:>12} {}",
            "Mes".bold(),
            "Monto".bold(),
            "Concepto".bold()
        );
        println!("  {}", "─".repeat(50));
        for (i, e) in extras.iter().enumerate() {
            println!(
                "  {}. {:<10} {:>12.2}  {}",
                i + 1,
                e.mes,
                e.monto,
                if e.concepto.is_empty() {
                    "—"
                } else {
                    &e.concepto
                }
            );
        }
    }
    println!();

    let opcion = menu(
        "¿Qué deseas hacer?",
        &[
            "➕ Agregar ingreso extra en un mes",
            "🗑️  Eliminar un ingreso extra",
            "↩️  Volver",
        ],
    );

    match opcion {
        Some(0) => {
            println!();
            let mes_raw = match pedir_texto("Mes (YYYY-MM, ej: 2026-05)") {
                Some(s) => s,
                None => return,
            };
            // Validar formato YYYY-MM
            let partes: Vec<&str> = mes_raw.splitn(2, '-').collect();
            let ok = partes.len() == 2
                && partes[0].len() == 4
                && partes[0].parse::<i32>().is_ok()
                && partes[1]
                    .parse::<u32>()
                    .is_ok_and(|m| (1..=12).contains(&m));
            if !ok {
                println!("  ⚠️  Formato inválido. Usa YYYY-MM.");
                pausa();
                return;
            }
            let monto = pedir_f64("Monto extra ($)", 0.0);
            if monto <= 0.0 {
                println!("  ⚠️  El monto debe ser mayor a $0.");
                pausa();
                return;
            }
            let concepto = pedir_texto_opcional("Concepto (vacío = sin descripción)");
            state
                .asesor
                .rastreador
                .ingresos_extra
                .push(omniplanner::ml::IngresoExtraMes {
                    mes: mes_raw.clone(),
                    monto,
                    concepto,
                });
            *sim =
                state
                    .asesor
                    .rastreador
                    .simular_libertad_editado(presupuesto, estrategia, ajustes);
            println!("  ✓ ${:.2} extra registrado para {}.", monto, mes_raw);
            pausa();
        }
        Some(1) => {
            let extras = &state.asesor.rastreador.ingresos_extra;
            if extras.is_empty() {
                println!("  (No hay ingresos extra que eliminar)");
                pausa();
                return;
            }
            let etiquetas: Vec<String> = extras
                .iter()
                .map(|e| format!("{} — ${:.2}  {}", e.mes, e.monto, e.concepto))
                .collect();
            let etiquetas_ref: Vec<&str> = etiquetas.iter().map(|s| s.as_str()).collect();
            if let Some(idx) = menu("Selecciona el ingreso extra a eliminar", &etiquetas_ref) {
                state.asesor.rastreador.ingresos_extra.remove(idx);
                *sim = state.asesor.rastreador.simular_libertad_editado(
                    presupuesto,
                    estrategia,
                    ajustes,
                );
                println!("  ✓ Ingreso extra eliminado.");
                pausa();
            }
        }
        _ => {}
    }
}

fn elegir_estrategia(rastreador: &RastreadorDeudas) -> Option<EstrategiaLibertad> {
    let opcion = menu(
        "Estrategia de reparto del sobrante",
        &[
            "Avalancha (tasa más alta primero — ahorra más)",
            "Bola de nieve (saldo más bajo primero — motivación)",
            "Pesos personalizados (nivelar varias deudas a la vez)",
            "Cancelar",
        ],
    );
    match opcion {
        Some(0) => Some(EstrategiaLibertad::Avalancha),
        Some(1) => Some(EstrategiaLibertad::BolaNieve),
        Some(2) => {
            let activas: Vec<&DeudaRastreada> = rastreador
                .deudas
                .iter()
                .filter(|d| d.activa && !d.es_pago_corriente() && d.saldo_actual() > 0.01)
                .collect();
            if activas.is_empty() {
                println!("  No hay deudas activas.");
                pausa();
                return None;
            }
            println!();
            println!("  Asigna un peso a cada deuda (ej. 3 y 1 → 75%/25%):");
            let mut pesos: Vec<(String, f64)> = Vec::new();
            for d in &activas {
                let peso = pedir_f64(
                    &format!(
                        "  Peso para '{}' (saldo ${:.2})",
                        d.nombre,
                        d.saldo_actual()
                    ),
                    1.0,
                );
                pesos.push((d.nombre.clone(), peso));
            }
            Some(EstrategiaLibertad::pesos_normalizados(pesos))
        }
        _ => None,
    }
}

fn mover_recursos_entre_deudas(
    sim: &SimulacionLibertad,
    ajustes: &mut Vec<AjusteMensualLibertad>,
    mes_pre: Option<usize>,
) -> bool {
    if sim.meses.is_empty() {
        return false;
    }
    let max_mes = sim.meses.len();
    let mes = match mes_pre {
        Some(m) if m >= 1 && m <= max_mes => m,
        Some(_) => {
            println!("  Mes fuera de rango.");
            pausa();
            return false;
        }
        None => {
            let m = pedir_f64(&format!("Mes (1-{})", max_mes), 1.0) as usize;
            if m < 1 || m > max_mes {
                println!("  Mes fuera de rango.");
                pausa();
                return false;
            }
            m
        }
    };
    let mes_data = &sim.meses[mes - 1];
    let mes_yyyy = mes_data.mes_yyyy_mm.clone();
    println!();
    println!("  Pagos en el mes {} ({}):", mes, mes_yyyy);
    let mut nombres: Vec<String> = Vec::new();
    for (i, (nombre, pago)) in mes_data.pagos.iter().enumerate() {
        println!("    {}. {:<20} ${:.2}", i + 1, nombre, pago);
        nombres.push(nombre.clone());
    }
    if nombres.len() < 2 {
        println!("  Se necesitan al menos 2 deudas para mover recursos.");
        pausa();
        return false;
    }
    let origen_idx = pedir_f64("Número de la deuda ORIGEN (de dónde quitar)", 1.0) as usize;
    let destino_idx = pedir_f64("Número de la deuda DESTINO (hacia dónde mover)", 2.0) as usize;
    if origen_idx < 1
        || origen_idx > nombres.len()
        || destino_idx < 1
        || destino_idx > nombres.len()
        || origen_idx == destino_idx
    {
        println!("  Selección inválida.");
        pausa();
        return false;
    }
    let origen = &nombres[origen_idx - 1];
    let destino = &nombres[destino_idx - 1];
    let pago_origen = mes_data
        .pagos
        .iter()
        .find(|(n, _)| n == origen)
        .map(|(_, p)| *p)
        .unwrap_or(0.0);
    let pago_destino = mes_data
        .pagos
        .iter()
        .find(|(n, _)| n == destino)
        .map(|(_, p)| *p)
        .unwrap_or(0.0);
    let monto = pedir_f64(
        &format!(
            "Monto a mover de '{}' (${:.2}) a '{}' (${:.2})",
            origen, pago_origen, destino, pago_destino
        ),
        0.0,
    );
    if monto <= 0.0 || monto > pago_origen + 0.01 {
        println!("  Monto inválido (debe ser > 0 y ≤ pago origen).");
        pausa();
        return false;
    }
    // Traducir a dos ajustes: fijar origen = pago-monto, fijar destino = pago+monto.
    reemplazar_ajuste(ajustes, &mes_yyyy, origen, (pago_origen - monto).max(0.0));
    reemplazar_ajuste(ajustes, &mes_yyyy, destino, pago_destino + monto);
    println!(
        "  ✓ Movidos ${:.2} de '{}' → '{}' en el mes {} ({}).",
        monto, origen, destino, mes, mes_yyyy
    );
    pausa();
    true
}

fn fijar_pago_en_mes(
    rastreador: &RastreadorDeudas,
    sim: &SimulacionLibertad,
    ajustes: &mut Vec<AjusteMensualLibertad>,
    mes_pre: Option<usize>,
) -> bool {
    if sim.meses.is_empty() {
        return false;
    }
    let max_mes = sim.meses.len();
    let mes = match mes_pre {
        Some(m) if m >= 1 && m <= max_mes => m,
        Some(_) => {
            println!("  Mes fuera de rango.");
            pausa();
            return false;
        }
        None => {
            let m = pedir_f64(&format!("Mes (1-{})", max_mes), 1.0) as usize;
            if m < 1 || m > max_mes {
                println!("  Mes fuera de rango.");
                pausa();
                return false;
            }
            m
        }
    };
    let mes_data = &sim.meses[mes - 1];
    let mes_yyyy = mes_data.mes_yyyy_mm.clone();
    println!();
    println!("  Pagos actuales en el mes {} ({}):", mes, mes_yyyy);
    let mut nombres: Vec<String> = Vec::new();
    for (i, (nombre, pago)) in mes_data.pagos.iter().enumerate() {
        println!("    {}. {:<20} ${:.2}", i + 1, nombre, pago);
        nombres.push(nombre.clone());
    }
    let idx = pedir_f64("Número de la deuda a fijar", 1.0) as usize;
    if idx < 1 || idx > nombres.len() {
        println!("  Selección inválida.");
        pausa();
        return false;
    }
    let nombre = &nombres[idx - 1];
    let pago_actual = mes_data
        .pagos
        .iter()
        .find(|(n, _)| n == nombre)
        .map(|(_, p)| *p)
        .unwrap_or(0.0);
    let nuevo = pedir_f64(
        &format!("Nuevo pago para '{}' (actual ${:.2})", nombre, pago_actual),
        pago_actual,
    );
    if nuevo < 0.0 {
        println!("  Monto inválido.");
        pausa();
        return false;
    }

    // ── Aviso: ¿este pago queda por debajo del mínimo?
    if let Some(deuda) = rastreador.deudas.iter().find(|d| d.nombre == *nombre) {
        let minimo = deuda.pago_pi_mensual();
        let saldo = mes_data
            .saldos
            .iter()
            .find(|(n, _)| n == nombre)
            .map(|(_, s)| *s)
            .unwrap_or(deuda.saldo_actual());
        let tasa_mes = deuda.tasa_anual / 100.0 / 12.0;
        if nuevo + 0.01 < minimo && saldo > 0.01 {
            let interes_mes = saldo * tasa_mes;
            let crecimiento_mes = (interes_mes - nuevo).max(0.0);
            println!();
            println!(
                "  ⚠️  Este pago (${:.2}) es MENOR al mínimo (${:.2}) de '{}'.",
                nuevo, minimo, nombre
            );
            println!(
                "     Tasa anual {:.2}% → interés mensual ≈ ${:.2} sobre saldo ${:.2}.",
                deuda.tasa_anual, interes_mes, saldo
            );
            if crecimiento_mes > 0.01 {
                println!(
                    "     Con este pago, el saldo CRECE ~${:.2}/mes (~${:.2}/año).",
                    crecimiento_mes,
                    crecimiento_mes * 12.0
                );
                println!(
                    "     💡 Por eso 'no hay ahorro': el dinero redirigido paga intereses de esta deuda en negativo."
                );
            } else {
                println!(
                    "     El saldo baja lentamente (${:.2}/mes) — por debajo del mínimo pactado.",
                    nuevo - interes_mes
                );
            }
            if !confirmar("¿Aplicar este pago igualmente?", false) {
                println!("  Cambio cancelado.");
                pausa();
                return false;
            }
        }
    }

    reemplazar_ajuste(ajustes, &mes_yyyy, nombre, nuevo);
    println!(
        "  ✓ Fijado '{}' = ${:.2} en el mes {} ({}).",
        nombre, nuevo, mes, mes_yyyy
    );
    pausa();
    true
}

/// Trunca un string a `max` caracteres añadiendo '…' si excede.
fn truncar(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        let cortado: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{}…", cortado)
    } else {
        s.to_string()
    }
}

/// Convierte "YYYY-MM" en etiqueta corta para mostrar en tablas: "may'26".
fn mes_corto(yyyy_mm: &str) -> String {
    let mut it = yyyy_mm.split('-');
    let y: i32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let m: u32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let nombre = match m {
        1 => "ene",
        2 => "feb",
        3 => "mar",
        4 => "abr",
        5 => "may",
        6 => "jun",
        7 => "jul",
        8 => "ago",
        9 => "sep",
        10 => "oct",
        11 => "nov",
        12 => "dic",
        _ => "???",
    };
    format!("{}'{}", nombre, y % 100)
}

/// Acumular varias cuotas de UNA deuda en un mes (ej. pagar 2 meses de hipoteca juntos).
///
/// Calcula el dinero adicional necesario, mira el sobrante del mes y, si hay déficit,
/// PROPONE recortar otras deudas no obligatorias hasta su mínimo (ordenadas por menor
/// tasa primero para minimizar el costo de intereses). Si la deuda es recurrente
/// (obligatoria o pago corriente), ofrece poner $0 en los meses adelantados para no
/// cobrarla doble.
fn acumular_pagos_deuda(
    rastreador: &RastreadorDeudas,
    sim: &SimulacionLibertad,
    ajustes: &mut Vec<AjusteMensualLibertad>,
    mes_pre: Option<usize>,
) -> bool {
    if sim.meses.is_empty() {
        return false;
    }
    let max_mes = sim.meses.len();
    let mes = match mes_pre {
        Some(m) if m >= 1 && m <= max_mes => m,
        Some(_) => {
            println!("  Mes fuera de rango.");
            pausa();
            return false;
        }
        None => {
            let m = pedir_f64(
                &format!("Mes destino para acumular cuotas (1-{})", max_mes),
                1.0,
            ) as usize;
            if m < 1 || m > max_mes {
                println!("  Mes fuera de rango.");
                pausa();
                return false;
            }
            m
        }
    };

    let mes_data = &sim.meses[mes - 1];
    let mes_yyyy = mes_data.mes_yyyy_mm.clone();
    println!();
    println!("  Pagos programados en el mes {} ({}):", mes, mes_yyyy);
    let mut nombres: Vec<String> = Vec::new();
    for (i, (nombre, pago)) in mes_data.pagos.iter().enumerate() {
        println!("    {}. {:<24} ${:.2}", i + 1, nombre, pago);
        nombres.push(nombre.clone());
    }
    if nombres.is_empty() {
        println!("  No hay pagos programados este mes.");
        pausa();
        return false;
    }
    let idx = pedir_f64("Número de la deuda donde acumular cuotas", 1.0) as usize;
    if idx < 1 || idx > nombres.len() {
        println!("  Selección inválida.");
        pausa();
        return false;
    }
    let nombre = nombres[idx - 1].clone();
    let pago_actual = mes_data
        .pagos
        .iter()
        .find(|(n, _)| n == &nombre)
        .map(|(_, p)| *p)
        .unwrap_or(0.0);

    let cuota = pedir_f64(
        &format!(
            "Monto de UNA cuota normal de '{}' (def ${:.2})",
            nombre, pago_actual
        ),
        pago_actual,
    );
    if cuota <= 0.0 {
        println!("  Cuota inválida.");
        pausa();
        return false;
    }
    let n_extra = pedir_f64(
        "¿Cuántas cuotas EXTRA acumular en este mes? (1 = pagar doble, 2 = triple…)",
        1.0,
    ) as usize;
    if n_extra < 1 {
        println!("  Cantidad inválida.");
        pausa();
        return false;
    }

    let extra = cuota * n_extra as f64;
    let nuevo_pago = cuota * (n_extra as f64 + 1.0);
    let delta = (nuevo_pago - pago_actual).max(0.0);
    let sobrante = mes_data.sobrante.max(0.0);
    let deficit = (delta - sobrante).max(0.0);

    println!();
    println!("  ▸ Cuota normal:         ${:.2}", cuota);
    println!(
        "  ▸ Cuotas a acumular:    {} extra (total {} cuotas en este mes)",
        n_extra,
        n_extra + 1
    );
    println!(
        "  ▸ Pago objetivo del mes: ${:.2}  (actual ${:.2})",
        nuevo_pago, pago_actual
    );
    println!("  ▸ Cuotas extra ($):     ${:.2}", extra);
    println!("  ▸ Diferencia vs actual: ${:.2}", delta);
    println!("  ▸ Sobrante del mes:     ${:.2}", sobrante);
    if deficit > 0.01 {
        println!(
            "  ▸ Déficit a cubrir:     {}",
            format!("${:.2}", deficit).red().bold()
        );
    } else {
        println!(
            "  ▸ Déficit a cubrir:     {}",
            "ninguno (alcanza con el sobrante)".green()
        );
    }
    println!();

    // ── Construir candidatos para recortar (no obligatorios, no la propia deuda)
    struct Candidato {
        nombre: String,
        pago_actual: f64,
        minimo: f64,
        saldo: f64,
        tasa_anual: f64,
        recorte_sobre_minimo: f64, // margen "seguro" (pago - mínimo)
        recorte_bajo_minimo: f64,  // adicional disponible (mínimo, pago a $0)
    }
    let mut candidatos: Vec<Candidato> = Vec::new();
    for (n, pago) in &mes_data.pagos {
        if n == &nombre {
            continue;
        }
        if let Some(d) = rastreador.deudas.iter().find(|d| &d.nombre == n) {
            if d.obligatoria || d.es_pago_corriente() {
                continue;
            }
            let minimo = d.pago_pi_mensual();
            let saldo = mes_data
                .saldos
                .iter()
                .find(|(nm, _)| nm == n)
                .map(|(_, s)| *s)
                .unwrap_or(d.saldo_actual());
            if saldo <= 0.01 {
                continue;
            }
            let recorte_sobre_minimo = (*pago - minimo).max(0.0);
            let recorte_bajo_minimo = (*pago - recorte_sobre_minimo).max(0.0);
            candidatos.push(Candidato {
                nombre: n.clone(),
                pago_actual: *pago,
                minimo,
                saldo,
                tasa_anual: d.tasa_anual,
                recorte_sobre_minimo,
                recorte_bajo_minimo,
            });
        }
    }
    // Ordena por menor tasa primero (recortar primero las deudas baratas).
    candidatos.sort_by(|a, b| {
        a.tasa_anual
            .partial_cmp(&b.tasa_anual)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut plan_recortes: Vec<(String, f64, f64)> = Vec::new(); // (deuda, pago_orig, recorte)
    let mut por_cubrir = deficit;
    let mut bajo_minimo_aplicado = false;
    let mut costo_total_intereses_extra = 0.0;

    if por_cubrir > 0.01 {
        if candidatos.is_empty() {
            println!(
                "  {} No hay deudas no obligatorias de donde recortar este mes.",
                "⚠️".red().bold()
            );
            println!(
                "     Reduce el monto de cuota o las cuotas extra, o aumenta el ingreso del mes."
            );
            pausa();
            return false;
        }

        // Capacidad total y estrategia
        let cap_sobre_minimo: f64 = candidatos.iter().map(|c| c.recorte_sobre_minimo).sum();
        let necesita_bajo_minimo = por_cubrir > cap_sobre_minimo + 0.01;

        if necesita_bajo_minimo {
            println!(
                "  {} Las demás deudas ya están en el mínimo (sobrante = $0).",
                "ℹ️".cyan()
            );
            println!(
                "     Para cubrir ${:.2} hay que recortar POR DEBAJO del mínimo en algunas.",
                por_cubrir
            );
            println!("     Eso hará que su saldo crezca temporalmente por intereses no cubiertos.");
            if !confirmar(
                "¿Permitir recortes bajo el mínimo? (Si dices NO, se cancela)",
                true,
            ) {
                println!("  Cancelado.");
                pausa();
                return false;
            }
            bajo_minimo_aplicado = true;
        }

        println!();
        println!("  💡 Propuesta automática (recorta primero las deudas con MENOR tasa):");
        println!(
            "     {:<24} {:>10} → {:>10}  {:>10}  {:>10}  {:>14}",
            "Deuda", "Pago", "Nuevo", "Recorte", "Mínimo", "Costo intereses"
        );

        // Pasada 1: recortar dentro del margen seguro (pago > mínimo).
        for c in &candidatos {
            if por_cubrir <= 0.01 {
                break;
            }
            let recorte = c.recorte_sobre_minimo.min(por_cubrir);
            if recorte <= 0.01 {
                continue;
            }
            let costo_int = recorte * (c.tasa_anual / 100.0 / 12.0);
            costo_total_intereses_extra += costo_int;
            println!(
                "     {:<24} {:>10} → {:>10}  {:>10}  {:>10}  {:>14}",
                truncar(&c.nombre, 24),
                format!("${:.2}", c.pago_actual),
                format!("${:.2}", c.pago_actual - recorte),
                format!("${:.2}", recorte),
                format!("${:.2}", c.minimo),
                format!("~${:.2}", costo_int),
            );
            plan_recortes.push((c.nombre.clone(), c.pago_actual, recorte));
            por_cubrir -= recorte;
        }

        // Pasada 2 (si aún hay déficit): recortar bajo el mínimo, también de menor tasa primero.
        if por_cubrir > 0.01 {
            for c in &candidatos {
                if por_cubrir <= 0.01 {
                    break;
                }
                let ya_recortado = plan_recortes
                    .iter()
                    .find(|(n, _, _)| n == &c.nombre)
                    .map(|(_, _, r)| *r)
                    .unwrap_or(0.0);
                let extra = (c.recorte_bajo_minimo).min(por_cubrir);
                if extra <= 0.01 {
                    continue;
                }
                let total_recorte = ya_recortado + extra;
                let costo_int_bajo = c.saldo * (c.tasa_anual / 100.0 / 12.0);
                costo_total_intereses_extra += costo_int_bajo;
                println!(
                    "     {:<24} {:>10} → {:>10}  {:>10}  {:>10}  {:>14} {}",
                    truncar(&c.nombre, 24),
                    format!("${:.2}", c.pago_actual - ya_recortado),
                    format!("${:.2}", (c.pago_actual - total_recorte).max(0.0)),
                    format!("${:.2}", extra),
                    format!("${:.2}", c.minimo),
                    format!("~${:.2}", costo_int_bajo),
                    "🔻 bajo mínimo".yellow(),
                );
                if let Some(idx) = plan_recortes.iter().position(|(n, _, _)| n == &c.nombre) {
                    plan_recortes[idx].2 = total_recorte;
                } else {
                    plan_recortes.push((c.nombre.clone(), c.pago_actual, extra));
                }
                por_cubrir -= extra;
            }
        }
        if por_cubrir > 0.01 {
            println!();
            println!(
                "  {} No alcanza con recortar TODAS las deudas a $0.",
                "⚠️".red().bold()
            );
            println!(
                "     Faltan ${:.2}. Sugerencias: bajar la cuota, reducir cuotas extra,",
                por_cubrir
            );
            println!("     o aumentar el ingreso/presupuesto del mes.");
            pausa();
            return false;
        }
        println!();
        println!(
            "  Total recortado: {} (cubre el déficit de ${:.2})",
            format!("${:.2}", deficit).green().bold(),
            deficit
        );
        if bajo_minimo_aplicado {
            println!(
                "  {} Costo estimado de intereses extra ese mes: ~${:.2}",
                "💸".yellow(),
                costo_total_intereses_extra
            );
            println!(
                "     (los saldos de las deudas recortadas crecerán por los intereses no cubiertos)"
            );
        }
    }

    println!();
    println!(
        "  ✦ Resultado: '{}' pagará ${:.2} en el mes {} (en lugar de ${:.2}).",
        nombre, nuevo_pago, mes, pago_actual
    );

    // ── Vínculos: dependientes que siguen a esta deuda principal.
    let dependientes: Vec<(String, f64, f64)> = rastreador
        .vinculos
        .iter()
        .filter(|v| v.principal == nombre)
        .filter_map(|v| {
            let pago_normal_dep = mes_data
                .pagos
                .iter()
                .find(|(n, _)| n == &v.dependiente)
                .map(|(_, p)| *p)
                .or_else(|| {
                    rastreador
                        .deudas
                        .iter()
                        .find(|d| d.nombre == v.dependiente)
                        .map(|d| d.pago_total_mensual())
                });
            pago_normal_dep.map(|p| (v.dependiente.clone(), p, v.factor))
        })
        .collect();
    if !dependientes.is_empty() {
        println!();
        println!("  🔗 Deudas vinculadas que también recibirán cuotas extra:");
        for (dep, pago_dep, factor) in &dependientes {
            let cuotas_dep_extra = (n_extra as f64 * factor).round() as usize;
            let pago_dep_objetivo = *pago_dep + pago_dep * cuotas_dep_extra as f64;
            println!(
                "     · {:<22} ${:.2} → ${:.2}  (+{} cuota[s])",
                dep, pago_dep, pago_dep_objetivo, cuotas_dep_extra
            );
        }
    }

    if !confirmar("¿Aplicar este plan?", true) {
        println!("  Cancelado.");
        pausa();
        return false;
    }

    // Aplicar fijación principal
    reemplazar_ajuste(ajustes, &mes_yyyy, &nombre, nuevo_pago);
    // Aplicar recortes
    for (n, pago_orig, recorte) in &plan_recortes {
        reemplazar_ajuste(ajustes, &mes_yyyy, n, (*pago_orig - *recorte).max(0.0));
    }
    // Aplicar dependientes (vinculadas)
    let mut dependientes_recurrentes: Vec<(String, usize)> = Vec::new();
    for (dep, pago_dep, factor) in &dependientes {
        let cuotas_dep_extra = (n_extra as f64 * factor).round() as usize;
        let pago_dep_objetivo = *pago_dep + pago_dep * cuotas_dep_extra as f64;
        reemplazar_ajuste(ajustes, &mes_yyyy, dep, pago_dep_objetivo);
        let es_recurrente_dep = rastreador
            .deudas
            .iter()
            .find(|d| &d.nombre == dep)
            .map(|d| d.obligatoria || d.es_pago_corriente())
            .unwrap_or(false);
        if es_recurrente_dep && cuotas_dep_extra > 0 {
            dependientes_recurrentes.push((dep.clone(), cuotas_dep_extra));
        }
    }

    // Si la deuda es recurrente (obligatoria o pago corriente), ofrecer fijar $0
    // en los próximos n_extra meses para no doble-cobrarla.
    let es_recurrente = rastreador
        .deudas
        .iter()
        .find(|d| d.nombre == nombre)
        .map(|d| d.obligatoria || d.es_pago_corriente())
        .unwrap_or(false);
    if es_recurrente {
        let primero = mes + 1;
        let hasta = (mes + n_extra).min(max_mes);
        if primero <= hasta {
            println!();
            println!(
                "  ℹ️  '{}' es pago recurrente. Si pagas {} cuota(s) adelantada(s),",
                nombre, n_extra
            );
            println!(
                "     normalmente NO debes pagarla los próximos {} mes(es) ({} a {}).",
                n_extra, primero, hasta
            );
            if confirmar(
                &format!(
                    "¿Fijar pago = $0 a '{}' en los meses {}–{}?",
                    nombre, primero, hasta
                ),
                true,
            ) {
                for m in primero..=hasta {
                    if let Some(md) = sim.meses.get(m - 1) {
                        let yyyy = md.mes_yyyy_mm.clone();
                        reemplazar_ajuste(ajustes, &yyyy, &nombre, 0.0);
                    }
                }
                println!("  ✓ Pagos puestos en $0 para meses {}–{}.", primero, hasta);
                // Aplicar también a dependientes recurrentes según sus propias cuotas extra.
                for (dep, cuotas_extra_dep) in &dependientes_recurrentes {
                    let primero_d = mes + 1;
                    let hasta_d = (mes + cuotas_extra_dep).min(max_mes);
                    if primero_d <= hasta_d {
                        for m in primero_d..=hasta_d {
                            if let Some(md) = sim.meses.get(m - 1) {
                                let yyyy = md.mes_yyyy_mm.clone();
                                reemplazar_ajuste(ajustes, &yyyy, dep, 0.0);
                            }
                        }
                        println!(
                            "  ✓ Vinculada '{}' también en $0 para meses {}–{}.",
                            dep, primero_d, hasta_d
                        );
                    }
                }
            }
        }
    }

    println!();
    println!(
        "  ✅ Plan aplicado: 1 fijación + {} recorte(s). Recalculando simulación.",
        plan_recortes.len()
    );
    pausa();
    true
}

/// Vista enfocada de un solo mes dentro del editor.
/// Permite iterar sobre cambios dentro del mes elegido y movernos libremente
/// a meses anteriores/siguientes sin perder el trabajo acumulado en `ajustes`.
/// Devuelve `true` si hubo al menos una edición real.
fn mes_focus(
    rastreador: &RastreadorDeudas,
    sim: &mut SimulacionLibertad,
    ajustes: &mut Vec<AjusteMensualLibertad>,
    estrategia: &EstrategiaLibertad,
    presupuesto: f64,
) -> bool {
    if sim.meses.is_empty() {
        println!("  No hay meses para enfocar.");
        pausa();
        return false;
    }
    let max_mes = sim.meses.len();
    let mes_inicial = pedir_f64(&format!("¿Qué mes enfocar? (1-{})", max_mes), 1.0) as usize;
    if mes_inicial < 1 || mes_inicial > max_mes {
        println!("  Mes fuera de rango.");
        pausa();
        return false;
    }
    let mut mes_idx = mes_inicial;
    let mut hubo_cambio = false;

    loop {
        limpiar();
        let mes_cal = sim
            .meses
            .get(mes_idx - 1)
            .map(|m| mes_corto(&m.mes_yyyy_mm))
            .unwrap_or_default();
        separador(&format!(
            "🎯 ENFOQUE MES {} ({}) / {}",
            mes_idx,
            mes_cal,
            sim.meses.len()
        ));
        let mes_data = match sim.meses.get(mes_idx - 1) {
            Some(m) => m,
            None => {
                println!("  El mes {} ya no existe en la simulación.", mes_idx);
                pausa();
                return hubo_cambio;
            }
        };
        let pago_total: f64 = mes_data.pagos.iter().map(|(_, p)| *p).sum();
        let int_total: f64 = mes_data.intereses.iter().map(|(_, i)| *i).sum();
        println!(
            "  Pagos: {} | Intereses: {} | Deuda restante: {}",
            format!("${:.2}", pago_total).green(),
            format!("${:.2}", int_total).red(),
            format!("${:.2}", mes_data.deuda_total).yellow()
        );
        if mes_data.minimos_no_cubiertos > 0.01 {
            println!(
                "  {} Mínimos NO cubiertos este mes: {} ({} deuda[s] descubierta[s])",
                "⚠️".red().bold(),
                format!("${:.2}", mes_data.minimos_no_cubiertos)
                    .red()
                    .bold(),
                mes_data.deudas_descubiertas.len()
            );
        }
        println!();
        println!(
            "  {:<24} {:>12} {:>12} {:>14}",
            "Deuda".bold(),
            "Pago".bold(),
            "Interés".bold(),
            "Saldo final".bold()
        );
        let mes_yyyy = mes_data.mes_yyyy_mm.clone();
        for (nombre, saldo) in &mes_data.saldos {
            let pago = mes_data
                .pagos
                .iter()
                .find(|(n, _)| n == nombre)
                .map(|(_, p)| *p)
                .unwrap_or(0.0);
            let interes = mes_data
                .intereses
                .iter()
                .find(|(n, _)| n == nombre)
                .map(|(_, i)| *i)
                .unwrap_or(0.0);
            let fijado = ajustes
                .iter()
                .any(|a| a.mes == mes_yyyy && a.nombre_deuda == *nombre);
            let marca = if mes_data.deudas_descubiertas.iter().any(|n| n == nombre) {
                " 🔴"
            } else if fijado {
                " 📌"
            } else {
                ""
            };
            let n_corto = if nombre.len() > 22 {
                format!("{}…", &nombre[..21])
            } else {
                nombre.clone()
            };
            println!(
                "  {:<24} {:>12} {:>12} {:>14}{}",
                n_corto,
                format!("${:.2}", pago),
                format!("${:.2}", interes),
                format!("${:.2}", saldo),
                marca
            );
        }
        let ajustes_mes: Vec<&AjusteMensualLibertad> =
            ajustes.iter().filter(|a| a.mes == mes_yyyy).collect();
        if !ajustes_mes.is_empty() {
            println!();
            println!("  📌 Pagos fijados en este mes:");
            for a in ajustes_mes {
                println!("     · {} → ${:.2}", a.nombre_deuda, a.pago_forzado);
            }
        }

        println!();
        let opcion = menu(
            "Acciones sobre este mes",
            &[
                "📌 Fijar pago a una deuda aquí",
                "↔️  Mover recursos entre deudas de este mes",
                "⏩ Acumular cuotas en este mes (con cobertura sugerida)",
                "🧹 Quitar TODOS los ajustes de este mes",
                "◀️  Mes anterior",
                "▶️  Mes siguiente",
                "🔢 Saltar a otro mes",
                "↩️  Volver al editor principal",
            ],
        );

        match opcion {
            Some(0) => {
                if fijar_pago_en_mes(rastreador, sim, ajustes, Some(mes_idx)) {
                    *sim = rastreador.simular_libertad_editado(presupuesto, estrategia, ajustes);
                    hubo_cambio = true;
                }
            }
            Some(1) => {
                if mover_recursos_entre_deudas(sim, ajustes, Some(mes_idx)) {
                    *sim = rastreador.simular_libertad_editado(presupuesto, estrategia, ajustes);
                    hubo_cambio = true;
                }
            }
            Some(2) => {
                if acumular_pagos_deuda(rastreador, sim, ajustes, Some(mes_idx)) {
                    *sim = rastreador.simular_libertad_editado(presupuesto, estrategia, ajustes);
                    hubo_cambio = true;
                }
            }
            Some(3) => {
                let cuantos = ajustes.iter().filter(|a| a.mes == mes_yyyy).count();
                if cuantos == 0 {
                    println!("  (No hay ajustes en el mes {} ({}).)", mes_idx, mes_yyyy);
                    pausa();
                } else if confirmar(
                    &format!(
                        "¿Eliminar {} ajuste(s) del mes {} ({})?",
                        cuantos, mes_idx, mes_yyyy
                    ),
                    false,
                ) {
                    ajustes.retain(|a| a.mes != mes_yyyy);
                    *sim = rastreador.simular_libertad_editado(presupuesto, estrategia, ajustes);
                    hubo_cambio = true;
                    println!(
                        "  ✓ {} ajuste(s) eliminados del mes {} ({}).",
                        cuantos, mes_idx, mes_yyyy
                    );
                    pausa();
                }
            }
            Some(4) => {
                if mes_idx > 1 {
                    mes_idx -= 1;
                } else {
                    println!("  Ya estás en el primer mes.");
                    pausa();
                }
            }
            Some(5) => {
                if mes_idx < sim.meses.len() {
                    mes_idx += 1;
                } else {
                    println!("  Ya estás en el último mes.");
                    pausa();
                }
            }
            Some(6) => {
                let nuevo = pedir_f64(
                    &format!("Mes a enfocar (1-{})", sim.meses.len()),
                    mes_idx as f64,
                ) as usize;
                if nuevo >= 1 && nuevo <= sim.meses.len() {
                    mes_idx = nuevo;
                } else {
                    println!("  Mes fuera de rango.");
                    pausa();
                }
            }
            Some(7) | None => return hubo_cambio,
            _ => {}
        }
    }
}

fn reemplazar_ajuste(ajustes: &mut Vec<AjusteMensualLibertad>, mes: &str, nombre: &str, pago: f64) {
    ajustes.retain(|a| !(a.mes == mes && a.nombre_deuda == nombre));
    ajustes.push(AjusteMensualLibertad::nuevo(mes, nombre, pago));
}

fn mostrar_tabla_plan_libertad(sim: &SimulacionLibertad) {
    if sim.meses.is_empty() {
        limpiar();
        separador("📊 TABLA DEL PLAN (mes × deuda)");
        println!("  (Sin meses)");
        pausa();
        return;
    }

    let nombres: Vec<String> = sim.meses[0].saldos.iter().map(|(n, _)| n.clone()).collect();
    let total_meses = sim.meses.len();
    const PAGINA: usize = 24;
    let mut inicio: usize = 0;

    loop {
        limpiar();
        separador(&format!(
            "📊 TABLA DEL PLAN — meses {}–{} de {}",
            inicio + 1,
            (inicio + PAGINA).min(total_meses),
            total_meses
        ));

        // Cabecera
        print!("  {:<12}", "Mes".bold());
        for n in &nombres {
            let corto = if n.len() > 10 { &n[..10] } else { n.as_str() };
            print!(" {:>11}", corto.bold());
        }
        print!(" {:>10}", "Total".bold());
        println!();
        let ancho = 12 + nombres.len() * 12 + 11;
        println!("  {}", "─".repeat(ancho.min(180)));

        let fin = (inicio + PAGINA).min(total_meses);
        for mes in &sim.meses[inicio..fin] {
            let etiq = format!("{} {}", mes.mes_numero, mes_corto(&mes.mes_yyyy_mm));
            print!("  {:<12}", etiq);
            let mut total = 0.0;
            for nombre in &nombres {
                let pago = mes
                    .pagos
                    .iter()
                    .find(|(n, _)| n == nombre)
                    .map(|(_, p)| *p)
                    .unwrap_or(0.0);
                total += pago;
                if pago < 0.01 {
                    print!(" {:>11}", "-".dimmed());
                } else {
                    print!(" {:>11.2}", pago);
                }
            }
            // Colorear Total según si excede el presupuesto efectivo del mes.
            // sobrante < 0 → el usuario necesita fondos extra (ahorros/escrow).
            let total_fmt = format!("{:>10.2}", total);
            if mes.sobrante < -0.01 {
                print!(" {}", total_fmt.red());
            } else if total > mes.presupuesto_efectivo * 0.97 {
                print!(" {}", total_fmt.yellow());
            } else {
                print!(" {}", total_fmt);
            }
            if !mes.liquidadas_este_mes.is_empty() {
                print!("  {} {}", "✅".green(), mes.liquidadas_este_mes.join(", "));
            }
            println!();
        }
        println!();
        println!(
            "  {}",
            format!("Mostrando {}–{} de {} meses.", inicio + 1, fin, total_meses).dimmed()
        );

        if total_meses <= PAGINA {
            pausa();
            return;
        }

        let mut opciones: Vec<&str> = Vec::new();
        if fin < total_meses {
            opciones.push("▶️  Página siguiente");
        }
        if inicio > 0 {
            opciones.push("◀️  Página anterior");
        }
        opciones.push("🔢 Saltar a un mes");
        opciones.push("↩️  Volver");

        match menu("Navegación", &opciones) {
            Some(i) => {
                let etiqueta = opciones[i];
                if etiqueta.contains("siguiente") {
                    inicio = (inicio + PAGINA).min(total_meses.saturating_sub(1));
                } else if etiqueta.contains("anterior") {
                    inicio = inicio.saturating_sub(PAGINA);
                } else if etiqueta.contains("Saltar") {
                    let m = pedir_f64(
                        &format!("Mes inicial (1-{})", total_meses),
                        (inicio + 1) as f64,
                    ) as usize;
                    if m >= 1 && m <= total_meses {
                        inicio = m - 1;
                    }
                } else {
                    return;
                }
            }
            None => return,
        }
    }
}

fn mostrar_comparacion_planes(base: &SimulacionLibertad, alt: &SimulacionLibertad) {
    limpiar();
    separador("⚖️  COMPARACIÓN: PLAN ORIGINAL vs PLAN EDITADO");
    let cmp = RastreadorDeudas::comparar_planes(base, alt);
    println!();
    println!(
        "  {:<28} {:>15} {:>15} {:>15}",
        "Métrica".bold(),
        "Original".bold(),
        "Editado".bold(),
        "Diferencia".bold()
    );
    println!("  {}", "─".repeat(75));
    let diff_meses_txt = if cmp.diferencia_meses == 0 {
        "igual".to_string()
    } else if cmp.diferencia_meses < 0 {
        format!("{} meses antes", -cmp.diferencia_meses)
    } else {
        format!("{} meses después", cmp.diferencia_meses)
    };
    println!(
        "  {:<28} {:>15} {:>15} {:>15}",
        "Meses hasta libertad", cmp.meses_base, cmp.meses_alternativa, diff_meses_txt
    );
    let diff_int = cmp.diferencia_intereses;
    let diff_int_txt = if diff_int.abs() < 0.01 {
        "igual".to_string()
    } else if diff_int < 0.0 {
        format!("-${:.2} (ahorras)", -diff_int)
    } else {
        format!("+${:.2} (pagas más)", diff_int)
    };
    println!(
        "  {:<28} {:>15} {:>15} {:>15}",
        "Intereses totales",
        format!("${:.2}", cmp.intereses_base),
        format!("${:.2}", cmp.intereses_alternativa),
        diff_int_txt
    );
    let diff_max = cmp.max_pago_mensual_alternativa - cmp.max_pago_mensual_base;
    println!(
        "  {:<28} {:>15} {:>15} {:>15}",
        "Mayor pago mensual",
        format!("${:.2}", cmp.max_pago_mensual_base),
        format!("${:.2}", cmp.max_pago_mensual_alternativa),
        format!(
            "{}${:.2}",
            if diff_max >= 0.0 { "+" } else { "-" },
            diff_max.abs()
        )
    );
    println!();

    // Consejo final
    if cmp.diferencia_meses < 0 {
        println!(
            "  💡 {}",
            format!(
                "El plan editado sale {} meses antes.",
                -cmp.diferencia_meses
            )
            .green()
            .bold()
        );
    } else if cmp.diferencia_meses == 0 && diff_int < -0.01 {
        println!(
            "  💡 {}",
            format!("Mismos meses pero ahorras ${:.2} en intereses.", -diff_int)
                .green()
                .bold()
        );
    } else if cmp.diferencia_meses == 0 && diff_max.abs() < 1.0 && diff_int.abs() < 1.0 {
        println!(
            "  💡 {}",
            "Mismo resultado con pagos redistribuidos — útil para nivelar meses difíciles.".cyan()
        );
    } else if cmp.diferencia_meses > 0 || diff_int > 1.0 {
        println!(
            "  ⚠️  {}",
            "El plan editado es menos eficiente que el original.".yellow()
        );
    }
    println!();
    pausa();
}

// ══════════════════════════════════════════════════════════════
//  Seguimiento del plan — ¿Estás en el camino este mes?
// ══════════════════════════════════════════════════════════════

pub fn rastreador_seguimiento_plan(state: &AppState) {
    let borrador = match state.asesor.borrador_plan.as_ref() {
        Some(b) => b.clone(),
        None => {
            limpiar();
            separador("📍 SEGUIMIENTO DEL PLAN");
            println!();
            println!("  {} No hay plan guardado todavía.", "ℹ️".cyan());
            println!();
            println!("  Para activar el seguimiento:");
            println!("  1. Ve a 'Simulacion: camino a la libertad financiera'");
            println!("  2. Elige tu estrategia y presupuesto");
            println!("  3. Entra al editor y usa 'Guardar borrador'");
            println!();
            println!("  Una vez guardado, esta pantalla comparará cada mes");
            println!("  lo que el plan dice vs lo que realmente pagaste.");
            pausa();
            return;
        }
    };

    let hoy = chrono::Local::now();
    let mes_hoy = hoy.format("%Y-%m").to_string();

    let parse_ym = |s: &str| -> Option<(i32, i32)> {
        let mut it = s.splitn(2, '-');
        let y: i32 = it.next()?.parse().ok()?;
        let m: i32 = it.next()?.parse().ok()?;
        Some((y, m))
    };

    let mes_inicio_str = match &borrador.mes_inicio {
        Some(m) => m.clone(),
        None => borrador
            .actualizado_en
            .get(..7)
            .unwrap_or(&mes_hoy)
            .to_string(),
    };

    let idx_simulacion: usize = match (parse_ym(&mes_hoy), parse_ym(&mes_inicio_str)) {
        (Some((ay, am)), Some((by, bm))) => {
            let diff = (ay * 12 + am) - (by * 12 + bm);
            if diff < 0 {
                0
            } else {
                diff as usize
            }
        }
        _ => 0,
    };

    let sim = state.asesor.rastreador.simular_libertad_editado(
        borrador.presupuesto,
        &borrador.estrategia,
        &borrador.ajustes,
    );
    let total_meses = sim.meses.len();

    limpiar();
    separador("📍 SEGUIMIENTO DEL PLAN — ¿Estás en el camino?");
    println!();

    println!(
        "  Plan activo: {} | ${:.2}/mes para deudas",
        borrador.estrategia.nombre().cyan().bold(),
        borrador.presupuesto
    );
    println!(
        "  Inicio del plan: {}  |  Hoy: {}",
        mes_inicio_str.yellow(),
        mes_hoy.green()
    );

    if total_meses == 0 || total_meses >= 600 {
        println!(
            "  {} El plan no converge — revisa el presupuesto.",
            "🔴".red()
        );
        pausa();
        return;
    }

    println!(
        "  Libertad financiera en: {} ({} meses totales)",
        formatear_plazo_meses(total_meses).yellow().bold(),
        total_meses
    );

    if idx_simulacion >= total_meses {
        println!();
        println!(
            "  {} ¡{}! Según el plan, ya deberías haber liquidado todas tus deudas.",
            "🏆".green().bold(),
            "LIBERTAD FINANCIERA ALCANZADA".green().bold()
        );
        println!("  Verifica en el Rastreador que todos los saldos sean $0.");
        pausa();
        return;
    }

    let mes_plan = &sim.meses[idx_simulacion];
    println!(
        "  Mes del plan: {} de {} — {} restantes",
        (idx_simulacion + 1).to_string().yellow().bold(),
        total_meses,
        formatear_plazo_meses(total_meses.saturating_sub(idx_simulacion + 1))
    );

    if !sim.gastos_fijos.is_empty() {
        println!();
        println!(
            "  🔒 Gastos fijos descontados: {} ({}/mes)",
            sim.gastos_fijos
                .iter()
                .map(|(n, m)| format!("{} ${:.0}", n, m))
                .collect::<Vec<_>>()
                .join(", "),
            format!("${:.2}", sim.total_gastos_fijos).yellow()
        );
    }

    println!();
    println!(
        "  {}",
        "══════════════════════════════════════════════════════════════════".cyan()
    );
    println!(
        "  {}",
        "  COMPARATIVA: PLAN DEL MES vs LO QUE REALMENTE PAGASTE"
            .cyan()
            .bold()
    );
    println!(
        "  {}",
        "══════════════════════════════════════════════════════════════════".cyan()
    );
    println!();
    println!(
        "  {:<22} {:>11} {:>11} {:>11}  {}",
        "Deuda".bold(),
        "Plan $".bold(),
        "Real $".bold(),
        "Δ".bold(),
        "Estado".bold()
    );
    println!("  {}", "─".repeat(70));

    // Calcular días restantes en el mes actual para distinguir PENDIENTE de ATRASADO
    let dia_actual = hoy.day() as i64;
    let dias_en_mes = {
        let mes = hoy.month();
        let anio = hoy.year();
        let inicio_sig = if mes == 12 {
            chrono::NaiveDate::from_ymd_opt(anio + 1, 1, 1)
        } else {
            chrono::NaiveDate::from_ymd_opt(anio, mes + 1, 1)
        }
        .unwrap_or(hoy.date_naive());
        let inicio_actual =
            chrono::NaiveDate::from_ymd_opt(anio, mes, 1).unwrap_or(hoy.date_naive());
        (inicio_sig - inicio_actual).num_days()
    };
    let dias_restantes = (dias_en_mes - dia_actual).max(0);

    let mut total_plan = 0.0f64;
    let mut total_real = 0.0f64;
    let mut deudas_atrasadas = 0usize;
    let mut deudas_pendientes_mes = 0usize; // sin registrar, pero mes en curso

    for (nombre, _saldo) in &mes_plan.saldos {
        let pago_plan = mes_plan
            .pagos
            .iter()
            .find(|(n, _)| n == nombre)
            .map(|(_, p)| *p)
            .unwrap_or(0.0);

        let pago_real = state
            .asesor
            .rastreador
            .deudas
            .iter()
            .find(|d| d.nombre == *nombre)
            .and_then(|d| d.historial.iter().find(|m| m.mes == mes_hoy))
            .map(|m| m.pago)
            .unwrap_or(0.0);

        let delta = pago_real - pago_plan;
        let estado = if pago_real < 0.01 && pago_plan > 0.01 {
            deudas_pendientes_mes += 1;
            "⏳ PENDIENTE".yellow().to_string()
        } else if delta < -1.0 {
            deudas_atrasadas += 1;
            "⚠️  BAJO".yellow().to_string()
        } else if delta > 1.0 {
            "🚀 EXTRA".green().to_string()
        } else {
            "✅ OK".green().to_string()
        };

        let delta_str = if delta.abs() < 0.50 {
            "    —".dimmed().to_string()
        } else if delta > 0.0 {
            format!("+${:.0}", delta).green().to_string()
        } else {
            format!("-${:.0}", delta.abs()).red().to_string()
        };

        let real_str = if pago_real > 0.01 {
            format!("${:.2}", pago_real)
        } else {
            "—".dimmed().to_string()
        };

        println!(
            "  {:<22} {:>11} {:>11} {:>11}  {}",
            if nombre.len() > 22 {
                format!("{}…", &nombre[..21])
            } else {
                nombre.clone()
            },
            format!("${:.2}", pago_plan),
            real_str,
            delta_str,
            estado
        );

        total_plan += pago_plan;
        total_real += pago_real;
    }

    println!("  {}", "─".repeat(70));
    let delta_total = total_real - total_plan;
    let delta_total_str = if delta_total.abs() < 0.50 {
        "    $0.00".dimmed().to_string()
    } else if delta_total > 0.0 {
        format!("+${:.2}", delta_total).green().bold().to_string()
    } else {
        format!("-${:.2}", delta_total.abs())
            .red()
            .bold()
            .to_string()
    };
    println!(
        "  {:<22} {:>11} {:>11} {:>11}",
        "TOTAL".bold(),
        format!("${:.2}", total_plan).yellow().bold(),
        format!("${:.2}", total_real).green().bold(),
        delta_total_str
    );

    // ── Sección gastos fijos (renta + escrow): comparativa plan vs real ──────
    if !sim.gastos_fijos.is_empty() {
        println!();
        println!(
            "  {}",
            "── Gastos fijos del plan ──────────────────────────────────────".dimmed()
        );
        let mut hay_algun_gasto = false;
        for (nombre_gasto, monto_plan) in &sim.gastos_fijos {
            // Buscar el nombre base (sin " — Escrow") en el rastreador
            let nombre_base = nombre_gasto.replace(" — Escrow", "");
            let es_escrow = nombre_gasto.contains("— Escrow");

            let real_gasto = state
                .asesor
                .rastreador
                .deudas
                .iter()
                .find(|d| d.nombre == nombre_base)
                .and_then(|d| d.historial.iter().find(|m| m.mes == mes_hoy))
                .map(|m| if es_escrow { m.pago_escrow } else { m.pago })
                .unwrap_or(0.0);

            let gasto_estado = if real_gasto < 0.01 {
                "⏳ PENDIENTE".yellow().to_string()
            } else if real_gasto >= *monto_plan - 0.50 {
                "✅ OK".green().to_string()
            } else {
                "⚠️  BAJO".yellow().to_string()
            };

            let real_gasto_str = if real_gasto > 0.01 {
                format!("${:.2}", real_gasto)
            } else {
                "—".dimmed().to_string()
            };

            let nombre_corto = if nombre_gasto.len() > 22 {
                format!("{}…", &nombre_gasto[..21])
            } else {
                nombre_gasto.clone()
            };
            println!(
                "  {:<22} {:>11} {:>11} {:>11}  {}",
                nombre_corto.dimmed(),
                format!("${:.2}", monto_plan).dimmed(),
                real_gasto_str,
                "".dimmed(),
                gasto_estado
            );
            hay_algun_gasto = true;
        }
        if !hay_algun_gasto {
            println!("  (ninguno registrado)");
        }
    }

    // Nota: deudas donde el pago registrado supera significativamente el plan P&I
    // Puede indicar que el usuario incluyó escrow o pagó doble sin darse cuenta.
    {
        let mut notas_extra: Vec<String> = Vec::new();
        for (nombre, _saldo) in &mes_plan.saldos {
            let pago_plan = mes_plan
                .pagos
                .iter()
                .find(|(n, _)| n == nombre)
                .map(|(_, p)| *p)
                .unwrap_or(0.0);
            if pago_plan < 0.01 {
                continue;
            }
            let deuda_opt = state
                .asesor
                .rastreador
                .deudas
                .iter()
                .find(|d| d.nombre == *nombre);
            let pago_real = deuda_opt
                .and_then(|d| d.historial.iter().find(|m| m.mes == mes_hoy))
                .map(|m| m.pago)
                .unwrap_or(0.0);
            if pago_real > pago_plan * 1.4 {
                let exceso = pago_real - pago_plan;
                let tiene_escrow = deuda_opt
                    .map(|d| d.tiene_escrow_configurado())
                    .unwrap_or(false);
                let escrow = deuda_opt.map(|d| d.escrow_mensual).unwrap_or(0.0);
                if tiene_escrow && (exceso - escrow).abs() < escrow * 0.3 {
                    notas_extra.push(format!(
                        "  ⚠️  {}: registraste ${:.2} P&I (plan ${:.2}). El exceso (~${:.0}) ≈ escrow.\n     Verifica si registraste el escrow (${:.2}) como P&I por error.",
                        if nombre.len() > 20 { format!("{}…", &nombre[..19]) } else { nombre.clone() },
                        pago_real, pago_plan, exceso, escrow
                    ));
                } else {
                    notas_extra.push(format!(
                        "  ℹ️  {}: registraste ${:.2} P&I (plan ${:.2}). Exceso: +${:.0}.",
                        if nombre.len() > 20 {
                            format!("{}…", &nombre[..19])
                        } else {
                            nombre.clone()
                        },
                        pago_real,
                        pago_plan,
                        exceso
                    ));
                }
            }
        }
        if !notas_extra.is_empty() {
            println!(
                "  {}",
                "── Notas sobre pagos registrados ──────────────────────────────".dimmed()
            );
            for nota in &notas_extra {
                println!("{}", nota);
            }
            println!();
        }
    }

    println!(
        "  {}",
        "── VEREDICTO ──────────────────────────────────────────────────".cyan()
    );
    println!();

    if deudas_atrasadas == 0 && deudas_pendientes_mes == 0 && total_real >= total_plan - 0.50 {
        println!(
            "  {} {} ¡Vas perfectamente según el plan!",
            "🟢".green(),
            "EN CAMINO".green().bold()
        );
        if delta_total > 1.0 {
            let sim_acc = state.asesor.rastreador.simular_libertad_editado(
                borrador.presupuesto + delta_total,
                &borrador.estrategia,
                &borrador.ajustes,
            );
            let meses_ganados = total_meses.saturating_sub(sim_acc.meses.len());
            println!(
                "  {} Pagaste {} de más — ¡excelente!",
                "💪".green(),
                format!("${:.2}", delta_total).green().bold()
            );
            if meses_ganados > 0 {
                println!(
                    "  {} Si mantienes este ritmo, llegas {} antes.",
                    "🚀".cyan(),
                    formatear_plazo_meses(meses_ganados).cyan().bold()
                );
            }
        }
    } else if deudas_atrasadas == 0 && deudas_pendientes_mes > 0 {
        // Mes en curso: hay pendientes pero ningún pago por debajo del plan
        println!(
            "  {} {} — {} deuda(s) sin registrar pago todavía.",
            "🟡",
            "MES EN CURSO".yellow().bold(),
            deudas_pendientes_mes
        );
        println!();
        let faltante = (total_plan - total_real).max(0.0);
        if faltante > 0.50 {
            println!(
                "  {} Queda registrar ~{} más para completar el plan.",
                "💡".yellow(),
                format!("${:.2}", faltante).yellow().bold()
            );
        }
        if dias_restantes > 0 {
            println!(
                "  {} Tienes {} día(s) restantes en {} para completar los pagos.",
                "📅".cyan(),
                dias_restantes.to_string().cyan().bold(),
                mes_hoy.cyan()
            );
        }
        if total_real > 0.01 {
            println!(
                "  {} Registrado hasta ahora: {} de {}.",
                "✅".green(),
                format!("${:.2}", total_real).green(),
                format!("${:.2}", total_plan).yellow()
            );
        }
    } else {
        println!(
            "  {} {} Hay pagos por debajo del plan.",
            "🔴".red(),
            "FUERA DEL PLAN".red().bold()
        );
        println!();
        if total_real < 0.01 {
            println!(
                "  {} No has registrado pagos reales para {} todavía.",
                "ℹ️".cyan(),
                mes_hoy
            );
            println!("  Usa 'Registrar mes de pago' para completar el seguimiento.");
        } else {
            let faltante = (total_plan - total_real).max(0.0);
            if faltante > 0.50 {
                println!(
                    "  {} Faltan {} para completar el plan de este mes.",
                    "💡".yellow(),
                    format!("${:.2}", faltante).yellow().bold()
                );
            }
        }
    }

    // Prioridad: deuda de mayor tasa con pago pendiente este mes
    let prioridad = mes_plan
        .pagos
        .iter()
        .filter(|(_, p)| *p > 0.01)
        .max_by(|a, b| {
            let ta = state
                .asesor
                .rastreador
                .deudas
                .iter()
                .find(|d| d.nombre == a.0)
                .map(|d| d.tasa_anual)
                .unwrap_or(0.0);
            let tb = state
                .asesor
                .rastreador
                .deudas
                .iter()
                .find(|d| d.nombre == b.0)
                .map(|d| d.tasa_anual)
                .unwrap_or(0.0);
            ta.partial_cmp(&tb).unwrap_or(std::cmp::Ordering::Equal)
        });
    if let Some((nombre_pri, _)) = prioridad {
        let tasa_pri = state
            .asesor
            .rastreador
            .deudas
            .iter()
            .find(|d| d.nombre == *nombre_pri)
            .map(|d| d.tasa_anual)
            .unwrap_or(0.0);
        if tasa_pri > 5.0 {
            println!();
            println!(
                "  {} Enfoca cualquier dólar extra en: {} ({:.1}% anual).",
                "🎯".cyan(),
                nombre_pri.cyan().bold(),
                tasa_pri
            );
        }
    }

    // Deudas que se liquidan este mes según el plan
    if !mes_plan.liquidadas_este_mes.is_empty() {
        println!();
        println!(
            "  {} Según el plan, {} se liquida(n) este mes:",
            "🏆".green().bold(),
            mes_plan.liquidadas_este_mes.len()
        );
        for nombre in &mes_plan.liquidadas_este_mes {
            let liberado = state
                .asesor
                .rastreador
                .deudas
                .iter()
                .find(|d| d.nombre == *nombre)
                .map(|d| d.pago_pi_mensual())
                .unwrap_or(0.0);
            println!(
                "     {} {} → libera {}/mes para acelerar las demás",
                "✅".green(),
                nombre.green().bold(),
                format!("${:.2}", liberado).cyan()
            );
        }
    }

    // ══════════════════════════════════════════════════════════════
    //  BALANCE REAL DEL MES — cuánto comprometiste vs lo disponible
    // ══════════════════════════════════════════════════════════════
    println!();
    println!(
        "  {}",
        "── BALANCE REAL DEL MES ───────────────────────────────────────".cyan()
    );
    println!();

    println!(
        "  {:<35} {:>11}",
        "Presupuesto mensual:",
        format!("${:.2}", borrador.presupuesto).yellow().bold()
    );

    let mut total_comprometido = 0.0f64;
    // Colecta alertas de escrow no registrado para mostrar al final
    let mut alertas_escrow: Vec<String> = Vec::new();

    // ── Gastos fijos del plan (renta, escrow, etc.) ──────────────
    if !sim.gastos_fijos.is_empty() {
        println!("  {:<35}", "  Gastos fijos del plan:".dimmed());
        for (nombre_gasto, monto_plan) in &sim.gastos_fijos {
            let nombre_base = nombre_gasto.replace(" — Escrow", "");
            let es_escrow = nombre_gasto.contains("— Escrow");
            let deuda_opt = state
                .asesor
                .rastreador
                .deudas
                .iter()
                .find(|d| d.nombre == nombre_base);
            let hist_mes = deuda_opt.and_then(|d| d.historial.iter().find(|h| h.mes == mes_hoy));
            let real_gasto = hist_mes
                .map(|h| if es_escrow { h.pago_escrow } else { h.pago })
                .unwrap_or(0.0);

            // Usar el real si es mayor al plan (pago doble/catch-up), si no el plan
            let monto_mostrar = real_gasto.max(*monto_plan);

            let label = if nombre_gasto.len() > 30 {
                format!("    {}…", &nombre_gasto[..29])
            } else {
                format!("    {}", nombre_gasto)
            };
            let (estado_gasto, monto_display) = if es_escrow
                && real_gasto < 0.01
                && deuda_opt.map(|d| d.escrow_mensual > 0.01).unwrap_or(false)
            {
                // Escrow configurado pero NO registrado este mes
                let escrow_cfg = deuda_opt.map(|d| d.escrow_mensual).unwrap_or(0.0);
                alertas_escrow.push(format!(
                    "  {} El escrow de {} está registrado como $0.00 en {}.\n     Escrow configurado: ${:.2}/mes. Usa '✏️  Editar pago' para corregirlo.",
                    "⛔".red(),
                    nombre_base,
                    mes_hoy,
                    escrow_cfg
                ));
                (
                    "❌ FALTA REGISTRAR".red().bold().to_string(),
                    format!("-${:.2}", monto_plan).red().to_string(),
                )
            } else if real_gasto < 0.01 {
                (
                    "⏳".to_string(),
                    format!("-${:.2}", monto_plan).red().to_string(),
                )
            } else if real_gasto > *monto_plan + 0.50 {
                // Pago real mayor al plan (doble pago, catch-up)
                (
                    format!("✅ (real: ${:.2})", real_gasto).green().to_string(),
                    format!("-${:.2}", real_gasto).red().to_string(),
                )
            } else {
                (
                    "✅".green().to_string(),
                    format!("-${:.2}", monto_plan).red().to_string(),
                )
            };

            println!("  {:<33} {:>11}  {}", label, monto_display, estado_gasto);
            total_comprometido += monto_mostrar;
        }
    }

    // ── Pagos de deudas del plan ──────────────────────────────────
    let pagos_plan_registrados: f64 = mes_plan.pagos.iter().map(|(_, p)| *p).sum();
    if pagos_plan_registrados > 0.01 {
        let total_real_deudas_plan: f64 = mes_plan
            .pagos
            .iter()
            .filter_map(|(nombre, _)| {
                state
                    .asesor
                    .rastreador
                    .deudas
                    .iter()
                    .find(|d| d.nombre == *nombre)
                    .and_then(|d| d.historial.iter().find(|h| h.mes == mes_hoy))
                    .map(|h| h.pago)
            })
            .sum();
        println!(
            "  {:<35} {:>11}",
            "  Deudas del plan (planificado):",
            format!("-${:.2}", pagos_plan_registrados).red()
        );
        if (total_real_deudas_plan - pagos_plan_registrados).abs() > 0.50 {
            println!(
                "  {:<35} {:>11}  {}",
                "    (registrado hasta ahora):",
                format!("-${:.2}", total_real_deudas_plan).dimmed(),
                "parcial".dimmed()
            );
        }
        total_comprometido += pagos_plan_registrados;
    }

    // ── Pagos fuera del plan: deudas inactivas con pago este mes ──
    let pagos_fuera_plan: Vec<(String, f64, &str)> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .filter(|d| !d.activa)
        .filter_map(|d| {
            let total: f64 = d
                .historial
                .iter()
                .filter(|h| h.mes == mes_hoy)
                .map(|h| h.pago + h.pago_escrow)
                .sum();
            if total > 0.01 {
                let frec = match d.frecuencia {
                    FrecuenciaPago::Anual => "anual",
                    FrecuenciaPago::Semestral => "semestral",
                    FrecuenciaPago::Trimestral => "trimestral",
                    _ => "especial",
                };
                Some((d.nombre.clone(), total, frec))
            } else {
                None
            }
        })
        .collect();

    for (nombre, monto, frec) in &pagos_fuera_plan {
        let display = if nombre.len() > 27 {
            format!("{}…", &nombre[..26])
        } else {
            nombre.clone()
        };
        println!(
            "  {:<35} {:>11}  {}",
            format!("  {} ({}):", display, frec),
            format!("-${:.2}", monto).red().bold(),
            "⚠️  fuera del plan".yellow()
        );
        total_comprometido += monto;
    }

    println!("  {}", "─".repeat(49));
    let disponible_hoy = borrador.presupuesto - total_comprometido;
    println!(
        "  {:<35} {:>11}",
        "Total comprometido:",
        format!("-${:.2}", total_comprometido).red().bold()
    );
    println!(
        "  {:<35} {:>11}",
        "Disponible para pagos pendientes:",
        if disponible_hoy >= 0.0 {
            format!("${:.2}", disponible_hoy).green().bold().to_string()
        } else {
            format!("-${:.2}", disponible_hoy.abs())
                .red()
                .bold()
                .to_string()
        }
    );

    // Mostrar alertas de escrow no registrado
    if !alertas_escrow.is_empty() {
        println!();
        println!(
            "  {}",
            "── Errores de registro detectados ─────────────────────────────".red()
        );
        for alerta in &alertas_escrow {
            println!("{}", alerta);
        }
    }
    // Cuánto falta pagar del plan este mes
    let pendiente_plan_hoy: f64 = mes_plan
        .pagos
        .iter()
        .filter(|(nombre, pago_plan)| {
            *pago_plan > 0.01
                && state
                    .asesor
                    .rastreador
                    .deudas
                    .iter()
                    .find(|d| d.nombre == *nombre)
                    .and_then(|d| d.historial.iter().find(|h| h.mes == mes_hoy))
                    .map(|h| h.pago)
                    .unwrap_or(0.0)
                    < 0.01
        })
        .map(|(_, p)| *p)
        .sum();

    if pendiente_plan_hoy > 0.50 {
        println!();
        if disponible_hoy >= pendiente_plan_hoy - 0.50 {
            println!(
                "  {} Tienes {} disponibles para los {} restantes del plan. ¡Alcanza!",
                "✅".green(),
                format!("${:.2}", disponible_hoy).green().bold(),
                format!("${:.2}", pendiente_plan_hoy).yellow()
            );
        } else {
            let brecha = (pendiente_plan_hoy - disponible_hoy.max(0.0)).max(0.0);
            println!(
                "  {} Aún faltan {} del plan pero solo hay {} disponibles.",
                "⚠️".yellow(),
                format!("${:.2}", pendiente_plan_hoy).yellow().bold(),
                if disponible_hoy >= 0.0 {
                    format!("${:.2}", disponible_hoy).red().to_string()
                } else {
                    format!("-${:.2}", disponible_hoy.abs())
                        .red()
                        .bold()
                        .to_string()
                }
            );
            println!(
                "  {} Brecha: {} — paga primero las deudas de mayor tasa.",
                "💡",
                format!("${:.2}", brecha).red().bold()
            );
        }
    }

    // ══════════════════════════════════════════════════════════════
    //  PROYECCIÓN MES SIGUIENTE
    // ══════════════════════════════════════════════════════════════
    let (ay2, am2) = parse_ym(&mes_hoy).unwrap_or((2026, 5));
    let (ny, nm) = if am2 == 12 {
        (ay2 + 1, 1)
    } else {
        (ay2, am2 + 1)
    };
    let mes_siguiente = format!("{}-{:02}", ny, nm);

    println!();
    println!(
        "  {}",
        "── PROYECCIÓN MES SIGUIENTE ───────────────────────────────────".cyan()
    );
    println!("  Mes: {}", mes_siguiente.cyan().bold());
    println!();

    // Pagos periódicos que no se repiten el mes siguiente
    let mut alivio_siguiente = 0.0f64;
    let mut notas_alivio: Vec<String> = Vec::new();

    for d in state.asesor.rastreador.deudas.iter() {
        if matches!(d.frecuencia, FrecuenciaPago::Mensual) {
            continue;
        }
        let pagado_este_mes: f64 = d
            .historial
            .iter()
            .filter(|h| h.mes == mes_hoy)
            .map(|h| h.pago + h.pago_escrow)
            .sum();
        if pagado_este_mes < 0.01 {
            continue;
        }
        let meses_hasta_proximo = match &d.frecuencia {
            FrecuenciaPago::Anual => 12usize,
            FrecuenciaPago::Semestral => 6,
            FrecuenciaPago::Trimestral => 3,
            _ => 1,
        };
        if meses_hasta_proximo > 1 {
            alivio_siguiente += pagado_este_mes;
            let display = if d.nombre.len() > 22 {
                format!("{}…", &d.nombre[..21])
            } else {
                d.nombre.clone()
            };
            notas_alivio.push(format!(
                "  {} {} ({}): {} → no toca en {} (cada {} meses)",
                "✂️",
                display,
                d.frecuencia.nombre(),
                format!("${:.2}", pagado_este_mes).yellow(),
                mes_siguiente,
                meses_hasta_proximo
            ));
        }
    }

    if notas_alivio.is_empty() {
        println!("  Sin pagos periódicos extra que desaparezcan el mes que viene.");
    } else {
        for linea in &notas_alivio {
            println!("{}", linea);
        }
        println!();
        println!(
            "  {} {} de ALIVIO en {} vs {} (pagos no mensuales que no se repiten).",
            "🟢",
            format!("${:.2}", alivio_siguiente).green().bold(),
            mes_siguiente.cyan().bold(),
            mes_hoy.cyan()
        );
    }

    // Plan del mes siguiente
    if idx_simulacion + 1 < total_meses {
        let mes_siguiente_plan = &sim.meses[idx_simulacion + 1];
        let total_plan_sig: f64 = mes_siguiente_plan.pagos.iter().map(|(_, p)| *p).sum();
        println!();
        println!(
            "  {} El plan de {} necesita {} para deudas + {} gastos fijos.",
            "📅".cyan(),
            mes_siguiente.cyan(),
            format!("${:.2}", total_plan_sig).yellow().bold(),
            format!("${:.2}", sim.total_gastos_fijos).yellow()
        );
        let comprometido_sig = total_plan_sig + sim.total_gastos_fijos;
        let libre_sig = borrador.presupuesto - comprometido_sig;
        println!(
            "  {} Después de cumplir el plan quedará aprox. {}.",
            if libre_sig >= 0.0 { "💰" } else { "🔴" },
            if libre_sig >= 0.0 {
                format!("${:.2} disponibles", libre_sig).green().to_string()
            } else {
                format!("${:.2} de déficit", libre_sig.abs())
                    .red()
                    .bold()
                    .to_string()
            }
        );
    }

    // ══════════════════════════════════════════════════════════════
    //  PAGOS PROGRAMADOS — compromisos futuros planificados
    // ══════════════════════════════════════════════════════════════
    let programados = &state.asesor.rastreador.pagos_programados;
    if !programados.is_empty() {
        println!();
        println!(
            "  {}",
            "── PAGOS PROGRAMADOS ──────────────────────────────────────────".cyan()
        );
        println!();
        println!(
            "  {:<22} {:>10} {:>10}  {:<22}  {}",
            "Deuda".bold(),
            "P&I".bold(),
            "Escrow".bold(),
            "Meses cubiertos".bold(),
            "Pagar en".bold()
        );
        println!("  {}", "─".repeat(82));
        for p in programados {
            let nombre = if p.nombre_deuda.len() > 22 {
                format!("{}…", &p.nombre_deuda[..21])
            } else {
                p.nombre_deuda.clone()
            };
            let escrow_str = if p.monto_escrow > 0.01 {
                format!("${:.2}", p.monto_escrow)
            } else {
                "—".dimmed().to_string()
            };
            let fecha_tag = if p.fecha_pago_prevista <= mes_hoy {
                p.fecha_pago_prevista.red().bold().to_string()
            } else {
                p.fecha_pago_prevista.cyan().to_string()
            };
            println!(
                "  {:<22} {:>10} {:>10}  {:<22}  {}",
                nombre,
                format!("${:.2}", p.monto_pi).yellow(),
                escrow_str,
                p.etiqueta_meses(),
                fecha_tag
            );
            if !p.nota.is_empty() {
                println!("     {} {}", "📝", p.nota.dimmed());
            }
        }
        let total_prog: f64 = programados.iter().map(|p| p.monto_total()).sum();
        println!("  {}", "─".repeat(82));
        println!(
            "  {:<48}  {}",
            "Total comprometido en pagos programados:".dimmed(),
            format!("${:.2}", total_prog).yellow().bold()
        );
        // Pagos cuya fecha ya llegó (vencidos)
        let vencidos: Vec<_> = programados
            .iter()
            .filter(|p| p.fecha_pago_prevista <= mes_hoy)
            .collect();
        if !vencidos.is_empty() {
            println!();
            println!(
                "  {} {} pago(s) programados con fecha {} o anterior — ¡usa 'Convertir a pago real'!",
                "⚠️".yellow(),
                vencidos.len().to_string().yellow().bold(),
                mes_hoy.yellow()
            );
        }
    }

    println!();
    pausa();
}

/// Bitácora del sistema — vista unificada del bus de eventos (paper trail).
pub fn rastreador_bitacora(state: &mut AppState) {
    loop {
        limpiar();
        println!("{}", "📰 Bitácora del sistema (paper trail)".bold().cyan());
        separador("Eventos");
        let total = state.bus.total();
        let pendientes = state.bus.pendientes().len();
        let vencidos = state.bus.vencidos().len();
        let hoy = state.bus.de_hoy().len();
        println!(
            "  Total: {}   Hoy: {}   Pendientes: {}   Vencidos: {}",
            total.to_string().bold(),
            hoy.to_string().green(),
            pendientes.to_string().yellow(),
            vencidos.to_string().red()
        );
        println!();

        let opciones = [
            "📅  Eventos de hoy",
            "⏭️   Próximos 20 eventos",
            "⚠️   Pendientes",
            "🚨  Vencidos",
            "🗂️   Todos (últimos 30)",
            "🔍  Buscar por texto",
            "🏷️   Filtrar por módulo",
            "🏷️   Filtrar por etiqueta",
            "🔎  Ver detalle de un evento (y relacionados)",
            "📤  Importar / Exportar (CSV / MD / JSON / Excel / SQL)",
            "🔙  Volver",
        ];

        match menu("¿Qué ver?", &opciones) {
            Some(0) => {
                println!("{}", "── Eventos de hoy ──".bold());
                let evs = state.bus.de_hoy();
                bitacora_render(&evs);
                println!();
                pausa();
            }
            Some(1) => {
                println!("{}", "── Próximos 20 ──".bold());
                let evs = state.bus.proximos(20);
                bitacora_render(&evs);
                println!();
                pausa();
            }
            Some(2) => {
                println!("{}", "── Pendientes ──".bold());
                let evs = state.bus.pendientes();
                bitacora_render(&evs);
                println!();
                pausa();
            }
            Some(3) => {
                println!("{}", "── Vencidos ──".bold());
                let evs = state.bus.vencidos();
                bitacora_render(&evs);
                println!();
                pausa();
            }
            Some(4) => {
                use omniplanner::eventos::EventoSistema;
                println!("{}", "── Últimos 30 ──".bold());
                let mut todos: Vec<&EventoSistema> = state.bus.todos().iter().collect();
                todos.sort_by(|a, b| b.creado.cmp(&a.creado));
                let recortado: Vec<&EventoSistema> = todos.into_iter().take(30).collect();
                bitacora_render(&recortado);
                println!();
                pausa();
            }
            Some(5) => bitacora_buscar_texto(state),
            Some(6) => bitacora_filtrar_modulo(state),
            Some(7) => bitacora_filtrar_etiqueta(state),
            Some(8) => bitacora_ver_detalle(state),
            Some(9) => crate::cli::io_modulos::menu_io_bitacora(state),
            _ => return,
        }
    }
}

/// Renderiza una lista de eventos con formato uniforme.
fn bitacora_render(evs: &[&omniplanner::eventos::EventoSistema]) {
    use omniplanner::eventos::EstadoEvento;
    if evs.is_empty() {
        println!("  {} (vacío)", "·".dimmed());
        return;
    }
    for ev in evs {
        let modulo_etq = format!("[{}]", ev.origen);
        let estado_col = match ev.estado {
            EstadoEvento::Realizado => "✓".green().to_string(),
            EstadoEvento::Pendiente => "⏳".yellow().to_string(),
            EstadoEvento::EnCurso => "▶".cyan().to_string(),
            EstadoEvento::Cancelado => "✗".dimmed().to_string(),
            EstadoEvento::Fallido => "✗".red().to_string(),
        };
        let monto_str = ev.monto.map(|m| format!(" ${:.2}", m)).unwrap_or_default();
        let fecha_str = ev.fecha.format("%Y-%m-%d").to_string();
        println!(
            "  {} {} {} {}{}",
            estado_col,
            fecha_str.dimmed(),
            modulo_etq.cyan(),
            ev.titulo,
            monto_str.bold()
        );
        if !ev.eventos_relacionados.is_empty() {
            println!(
                "       🔗 {} evento(s) relacionado(s)",
                ev.eventos_relacionados.len()
            );
        }
        if !ev.notas.is_empty() {
            let nota_join = ev.notas.join(" / ");
            println!("       · {}", nota_join.dimmed());
        }
    }
}

/// Búsqueda libre en título, contraparte, descripción y notas.
fn bitacora_buscar_texto(state: &AppState) {
    use omniplanner::eventos::EventoSistema;
    println!();
    let q = match pedir_texto("Texto a buscar (mín. 2 caracteres)") {
        Some(s) if s.trim().len() >= 2 => s.trim().to_lowercase(),
        _ => {
            println!("  {} Búsqueda cancelada.", "ℹ️".cyan());
            pausa();
            return;
        }
    };
    let resultados: Vec<&EventoSistema> = state
        .bus
        .todos()
        .iter()
        .filter(|e| {
            e.titulo.to_lowercase().contains(&q)
                || e.descripcion.to_lowercase().contains(&q)
                || e.contraparte.to_lowercase().contains(&q)
                || e.notas.iter().any(|n| n.to_lowercase().contains(&q))
                || e.etiquetas.iter().any(|t| t.to_lowercase().contains(&q))
        })
        .collect();
    println!();
    println!(
        "{} ({} coincidencia(s))",
        format!("── Resultados: \"{}\" ──", q).bold(),
        resultados.len()
    );
    bitacora_render(&resultados);
    println!();
    pausa();
}

/// Filtro por módulo emisor.
fn bitacora_filtrar_modulo(state: &AppState) {
    use omniplanner::eventos::Modulo;
    let modulos = [
        Modulo::Rastreador,
        Modulo::Presupuesto,
        Modulo::Agenda,
        Modulo::Tareas,
        Modulo::Memoria,
        Modulo::Cartera,
        Modulo::Contactos,
        Modulo::Facturas,
        Modulo::Sync,
        Modulo::Sistema,
    ];
    let etiquetas: Vec<String> = modulos.iter().map(|m| m.to_string()).collect();
    let refs: Vec<&str> = etiquetas.iter().map(|s| s.as_str()).collect();
    let i = match menu("¿Qué módulo?", &refs) {
        Some(i) => i,
        None => return,
    };
    let evs = state.bus.por_modulo(&modulos[i]);
    println!();
    println!(
        "{}",
        format!("── Módulo: {} ({} eventos) ──", modulos[i], evs.len()).bold()
    );
    bitacora_render(&evs);
    println!();
    pausa();
}

/// Filtro por etiqueta exacta.
fn bitacora_filtrar_etiqueta(state: &AppState) {
    use omniplanner::eventos::EventoSistema;
    // Recolectar etiquetas únicas con conteo
    let mut tally: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for ev in state.bus.todos() {
        for t in &ev.etiquetas {
            *tally.entry(t.clone()).or_insert(0) += 1;
        }
    }
    if tally.is_empty() {
        println!("  {} No hay etiquetas registradas todavía.", "ℹ️".cyan());
        pausa();
        return;
    }
    let etiquetas: Vec<String> = tally
        .iter()
        .map(|(t, c)| format!("{} ({})", t, c))
        .collect();
    let claves: Vec<String> = tally.keys().cloned().collect();
    let refs: Vec<&str> = etiquetas.iter().map(|s| s.as_str()).collect();
    let i = match menu("¿Qué etiqueta?", &refs) {
        Some(i) => i,
        None => return,
    };
    let etq = &claves[i];
    let evs: Vec<&EventoSistema> = state
        .bus
        .todos()
        .iter()
        .filter(|e| e.etiquetas.iter().any(|t| t == etq))
        .collect();
    println!();
    println!(
        "{}",
        format!("── Etiqueta: {} ({} eventos) ──", etq, evs.len()).bold()
    );
    bitacora_render(&evs);
    println!();
    pausa();
}

/// Vista detallada de un evento, navegando a sus relacionados.
fn bitacora_ver_detalle(state: &AppState) {
    use omniplanner::eventos::EventoSistema;
    if state.bus.total() == 0 {
        println!("  {} La bitácora está vacía.", "ℹ️".cyan());
        pausa();
        return;
    }
    // Mostrar últimos 30 ordenados por fecha de creación descendente para elegir
    let mut lista: Vec<&EventoSistema> = state.bus.todos().iter().collect();
    lista.sort_by(|a, b| b.creado.cmp(&a.creado));
    lista.truncate(30);
    let etiquetas: Vec<String> = lista
        .iter()
        .map(|e| {
            format!(
                "[{}] {} — {} {}",
                e.fecha.format("%Y-%m-%d"),
                e.origen,
                e.titulo,
                e.monto.map(|m| format!("${:.2}", m)).unwrap_or_default()
            )
        })
        .collect();
    let refs: Vec<&str> = etiquetas.iter().map(|s| s.as_str()).collect();
    let i = match menu("¿Qué evento?", &refs) {
        Some(i) => i,
        None => return,
    };
    let mut id_actual = lista[i].id.clone();
    drop(lista);
    drop(etiquetas);

    loop {
        let ev = match state.bus.buscar(&id_actual) {
            Some(e) => e,
            None => return,
        };
        limpiar();
        println!("{}", "🔎 Detalle del evento".bold().cyan());
        separador("Detalle");
        println!("  ID:           {}", ev.id.dimmed());
        println!("  Módulo:       {}", ev.origen.to_string().cyan());
        println!("  Tipo:         {}", ev.tipo);
        println!("  Estado:       {}", ev.estado);
        println!("  Título:       {}", ev.titulo.bold());
        if !ev.descripcion.is_empty() {
            println!("  Descripción:  {}", ev.descripcion);
        }
        println!(
            "  Fecha:        {}",
            ev.fecha.format("%Y-%m-%d").to_string().yellow()
        );
        println!(
            "  Creado:       {}",
            ev.creado.format("%Y-%m-%d %H:%M").to_string().dimmed()
        );
        if let Some(m) = ev.monto {
            println!("  Monto:        {}", format!("${:.2}", m).bold().green());
        }
        if !ev.contraparte.is_empty() {
            println!("  Contraparte:  {}", ev.contraparte);
        }
        if !ev.etiquetas.is_empty() {
            println!("  Etiquetas:    {}", ev.etiquetas.join(", ").yellow());
        }
        if !ev.referencias.is_empty() {
            println!("  Referencias:");
            for r in &ev.referencias {
                println!("    · {}/{}/{} → {}", r.modulo, r.tipo, r.id, r.etiqueta);
            }
        }
        if !ev.adjuntos.is_empty() {
            println!("  Adjuntos:");
            for a in &ev.adjuntos {
                println!("    · [{}] {} → {}", a.tipo, a.nombre, a.ruta);
            }
        }
        if !ev.notas.is_empty() {
            println!("  Notas:");
            for n in &ev.notas {
                println!("    · {}", n);
            }
        }
        println!();

        // Listar relacionados si los hay
        if ev.eventos_relacionados.is_empty() {
            println!("  {} Sin eventos relacionados.", "·".dimmed());
            println!();
            pausa();
            return;
        }

        println!(
            "  🔗 {} evento(s) relacionado(s):",
            ev.eventos_relacionados.len()
        );
        let ids: Vec<String> = ev.eventos_relacionados.clone();
        let mut etqs: Vec<String> = Vec::new();
        for rid in &ids {
            if let Some(rev) = state.bus.buscar(rid) {
                etqs.push(format!(
                    "[{}] {} — {}",
                    rev.fecha.format("%Y-%m-%d"),
                    rev.origen,
                    rev.titulo
                ));
            } else {
                etqs.push(format!("(huérfano: {})", rid));
            }
        }
        let mut opcs: Vec<&str> = etqs.iter().map(|s| s.as_str()).collect();
        opcs.push("🔙 Volver");
        let sel = match menu("¿Navegar a cuál?", &opcs) {
            Some(i) if i < ids.len() => i,
            _ => return,
        };
        id_actual = ids[sel].clone();
    }
}

/// Prompt one-time por sesión: para cada deuda real sin `dia_corte` definido,
/// ofrecer al usuario establecer el día (1-31). Si el usuario decide saltar
/// la primera, se aborta el resto del bucle (no es invasivo).
fn prompt_inicial_dias_corte(state: &mut AppState) {
    let pendientes: Vec<String> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .filter(|d| {
            d.activa
                && d.dia_corte.is_none()
                && (!d.es_pago_corriente() || matches!(d.frecuencia, FrecuenciaPago::Mensual))
        })
        .map(|d| d.nombre.clone())
        .collect();

    if pendientes.is_empty() {
        return;
    }

    limpiar();
    separador("📅 DÍAS DE CORTE — CONFIGURACIÓN INICIAL");
    println!();
    println!(
        "  Tienes {} deuda(s) sin día de corte definido.",
        pendientes.len().to_string().yellow().bold()
    );
    println!("  El día de corte (1-31) es la fecha del mes en que vence o cierra el ciclo.");
    println!("  Sirve para auto-rellenar el mes al registrar pagos y para alertas.");
    println!();
    if !confirmar("¿Quieres definirlos ahora?", true) {
        println!("  Puedes editarlos luego al agregar/registrar pagos.");
        pausa();
        return;
    }

    for nombre in pendientes {
        println!();
        println!("  • {}", nombre.cyan().bold());
        let dia = pedir_f64("Día de corte (1-31, 0 = saltar)", 0.0) as i64;
        if (1..=31).contains(&dia) {
            if let Some(d) = state
                .asesor
                .rastreador
                .deudas
                .iter_mut()
                .find(|d| d.nombre == nombre)
            {
                d.dia_corte = Some(dia as u32);
                println!("    ✓ Día {} registrado.", dia);
            }
        } else {
            println!("    (saltada)");
        }
    }
    println!();
    pausa();
}
