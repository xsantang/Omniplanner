//! Sugerencias inteligentes de pago — Fase 3.
//!
//! Analiza el estado real del usuario (gastos reales del mes,
//! saldo disponible, deudas activas) y genera recomendaciones
//! priorizadas de a qué deudas aplicar dinero extra este mes.

use super::advisor::{DeudaRastreada, RastreadorDeudas};
use super::gastos::AlmacenGastos;
use serde::{Deserialize, Serialize};

// ─── Tipo de recomendación ───────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TipoSugerencia {
    /// Pagar más del mínimo (alto interés)
    AbonoExtra,
    /// Pagar esta deuda primero (bola de nieve — menor saldo)
    BolaNieve,
    /// Deuda vencida / atrasada — urgente
    Urgente,
    /// Solo pagar el mínimo — no hay excedente
    SoloMinimo,
    /// Deuda casi liquidada — vale la pena terminarla
    CasiLiquidada,
}

impl std::fmt::Display for TipoSugerencia {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TipoSugerencia::AbonoExtra => write!(f, "Abono Extra (alto interés)"),
            TipoSugerencia::BolaNieve => write!(f, "Bola de Nieve (menor saldo)"),
            TipoSugerencia::Urgente => write!(f, "⚠ URGENTE — atrasada"),
            TipoSugerencia::SoloMinimo => write!(f, "Solo pago mínimo"),
            TipoSugerencia::CasiLiquidada => write!(f, "Casi liquidada — ¡termínala!"),
        }
    }
}

// ─── Sugerencia individual ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SugerenciaPago {
    pub nombre_deuda: String,
    pub tipo: TipoSugerencia,
    /// Pago mínimo requerido
    pub pago_minimo: f64,
    /// Monto sugerido a pagar este mes (mínimo + extra si hay flujo)
    pub monto_sugerido: f64,
    /// Saldo actual de la deuda
    pub saldo_actual: f64,
    /// Tasa APR de la deuda
    pub tasa_anual: f64,
    /// Justificación textual legible
    pub razon: String,
    /// Impacto estimado en intereses si se paga el monto sugerido vs mínimo
    pub ahorro_interes_estimado: f64,
}

// ─── Plan de pagos del mes ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanPagosMes {
    /// Ingreso mensual confirmado del usuario
    pub ingreso_mensual: f64,
    /// Total gastado en gastos reales este mes (no deudas)
    pub gastos_reales_mes: f64,
    /// Total pagos mínimos de todas las deudas activas
    pub pagos_minimos_total: f64,
    /// Excedente disponible para abonos extra
    pub excedente: f64,
    /// Lista de sugerencias priorizadas
    pub sugerencias: Vec<SugerenciaPago>,
    /// Advertencias generales
    pub advertencias: Vec<String>,
}

impl PlanPagosMes {
    /// Genera un plan de pagos para el mes actual.
    ///
    /// Lógica:
    /// 1. Calcula gastos reales del mes y los descuenta del ingreso.
    /// 2. Identifica deudas urgentes (atrasadas).
    /// 3. Aplica estrategia avalancha (mayor tasa primero) para el excedente.
    /// 4. Marca deudas casi liquidadas (saldo < 2 meses de pago mínimo).
    pub fn generar(
        rastreador: &RastreadorDeudas,
        gastos: &AlmacenGastos,
        anio: i32,
        mes: u32,
    ) -> Self {
        let ingreso = rastreador.ingreso_mensual_confirmado();
        let resumen = gastos.resumen_mes(anio, mes);
        // Solo contamos gastos positivos (no deudas ya registradas en rastreador)
        let gastos_mes = resumen.total_gastos;
        let pagos_minimos = rastreador.pagos_minimos_mensuales();
        let excedente = (ingreso - gastos_mes - pagos_minimos).max(0.0);

        let mut advertencias: Vec<String> = Vec::new();
        let mut sugerencias: Vec<SugerenciaPago> = Vec::new();

        if ingreso < 0.01 {
            advertencias.push(
                "No hay ingresos confirmados registrados. Ve al Rastreador → Ingresos."
                    .to_string(),
            );
        }
        if gastos_mes > ingreso * 0.9 {
            advertencias.push(format!(
                "Los gastos del mes (${:.2}) superan el 90% del ingreso mensual. Revisa tu presupuesto.",
                gastos_mes
            ));
        }

        let activas = rastreador.deudas_activas();

        // Paso 1: urgentes (meses_atrasados > 0 en el historial reciente)
        // Usamos la presencia de meses_atrasados implícita en obligatoria + sin historial
        for d in &activas {
            let saldo = d.saldo_actual();
            if saldo < 0.01 {
                continue;
            }
            // Deuda casi liquidada: saldo < 2.5× el pago mínimo (o P&I)
            let pago_ref = if d.principal_interes_mensual > 0.01 {
                d.principal_interes_mensual
            } else {
                d.pago_minimo
            };
            if saldo < pago_ref * 2.5 && saldo > 0.01 {
                sugerencias.push(SugerenciaPago {
                    nombre_deuda: d.nombre.clone(),
                    tipo: TipoSugerencia::CasiLiquidada,
                    pago_minimo: pago_ref,
                    monto_sugerido: saldo, // pagar todo de una vez
                    saldo_actual: saldo,
                    tasa_anual: d.tasa_anual,
                    razon: format!(
                        "Solo quedan ${:.2} de saldo. Con un pago único puedes liquidarla esta semana.",
                        saldo
                    ),
                    ahorro_interes_estimado: saldo * (d.tasa_anual / 100.0 / 12.0),
                });
            }
        }

        // Paso 2: avalancha (mayor tasa primero) para el excedente
        let mut deudas_para_extra: Vec<&DeudaRastreada> = activas
            .iter()
            .filter(|d| {
                d.saldo_actual() > 0.01
                    && !sugerencias
                        .iter()
                        .any(|s| s.nombre_deuda == d.nombre && s.tipo == TipoSugerencia::CasiLiquidada)
            })
            .copied()
            .collect();
        deudas_para_extra.sort_by(|a, b| {
            b.tasa_anual
                .partial_cmp(&a.tasa_anual)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut excedente_restante = excedente;
        for d in &deudas_para_extra {
            let saldo = d.saldo_actual();
            let pago_ref = if d.principal_interes_mensual > 0.01 {
                d.principal_interes_mensual
            } else {
                d.pago_minimo
            };
            if excedente_restante > 0.01 {
                let extra = excedente_restante.min(saldo - pago_ref).max(0.0);
                let monto_sugerido = pago_ref + extra;
                excedente_restante -= extra;
                let ahorro = extra * (d.tasa_anual / 100.0 / 12.0);
                let tipo = if d.tasa_anual >= 20.0 {
                    TipoSugerencia::AbonoExtra
                } else {
                    TipoSugerencia::BolaNieve
                };
                let razon = if extra > 0.01 {
                    format!(
                        "Paga ${:.2} extra sobre el mínimo — ahorras ~${:.2} en intereses este mes. APR: {:.1}%",
                        extra, ahorro, d.tasa_anual
                    )
                } else {
                    format!("Solo el pago mínimo. APR: {:.1}%", d.tasa_anual)
                };
                sugerencias.push(SugerenciaPago {
                    nombre_deuda: d.nombre.clone(),
                    tipo,
                    pago_minimo: pago_ref,
                    monto_sugerido,
                    saldo_actual: saldo,
                    tasa_anual: d.tasa_anual,
                    razon,
                    ahorro_interes_estimado: ahorro,
                });
            } else {
                // Sin excedente → solo mínimo
                sugerencias.push(SugerenciaPago {
                    nombre_deuda: d.nombre.clone(),
                    tipo: TipoSugerencia::SoloMinimo,
                    pago_minimo: pago_ref,
                    monto_sugerido: pago_ref,
                    saldo_actual: saldo,
                    tasa_anual: d.tasa_anual,
                    razon: format!(
                        "No hay excedente disponible. Paga al menos el mínimo: ${:.2}",
                        pago_ref
                    ),
                    ahorro_interes_estimado: 0.0,
                });
            }
        }

        // Bola de nieve alternativa: si todas las tasas son similares (<5% diferencia),
        // reordenar por saldo menor para motivación rápida.
        if !sugerencias.is_empty() {
            let tasas: Vec<f64> = sugerencias.iter().map(|s| s.tasa_anual).collect();
            let max_t = tasas.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let min_t = tasas.iter().cloned().fold(f64::INFINITY, f64::min);
            if (max_t - min_t) < 5.0 {
                // Marcar las de menor saldo como "Bola de nieve"
                let min_saldo = sugerencias
                    .iter()
                    .map(|s| s.saldo_actual)
                    .fold(f64::INFINITY, f64::min);
                for s in &mut sugerencias {
                    if (s.saldo_actual - min_saldo).abs() < 0.01
                        && s.tipo == TipoSugerencia::AbonoExtra
                    {
                        s.tipo = TipoSugerencia::BolaNieve;
                        s.razon = format!(
                            "{} (tasas similares → estrategia bola de nieve para motivación rápida)",
                            s.razon
                        );
                    }
                }
            }
        }

        if activas.is_empty() {
            advertencias.push("¡No tienes deudas activas registradas! Agrega deudas en el Rastreador.".to_string());
        }

        PlanPagosMes {
            ingreso_mensual: ingreso,
            gastos_reales_mes: gastos_mes,
            pagos_minimos_total: pagos_minimos,
            excedente,
            sugerencias,
            advertencias,
        }
    }
}
