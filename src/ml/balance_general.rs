//! Balance General — fotografía financiera en un punto del tiempo.
//!
//! Implementa la ecuación contable fundamental:
//!   **Activos = Pasivos + Patrimonio**
//!
//! # Estructura
//! ```text
//! BalanceGeneral
//! ├── activos
//! │   ├── corrientes  (efectivo, cuentas por cobrar, inventario…)
//! │   └── no_corrientes (propiedad, equipo, intangibles…)
//! ├── pasivos
//! │   ├── corrientes  (cuentas por pagar, deuda corto plazo…)
//! │   └── no_corrientes (deuda largo plazo, obligaciones…)
//! └── patrimonio
//!     (capital, utilidades retenidas, reservas…)
//! ```
//!
//! Los ratios financieros se calculan directamente desde el balance.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ══════════════════════════════════════════════════════════════════════════════
//  Partida contable
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClaseActivo {
    // Corrientes
    EfectivoEquivalente,
    CuentasPorCobrar,
    Inventario,
    GastosPrePagados,
    OtroActivoCorriente,
    // No corrientes
    PropiedadPlantaEquipo,
    ActivoIntangible,
    InversionLargoPlazo,
    OtroActivoNoCorriente,
}

impl ClaseActivo {
    pub fn es_corriente(&self) -> bool {
        matches!(
            self,
            ClaseActivo::EfectivoEquivalente
                | ClaseActivo::CuentasPorCobrar
                | ClaseActivo::Inventario
                | ClaseActivo::GastosPrePagados
                | ClaseActivo::OtroActivoCorriente
        )
    }

    pub fn nombre(&self) -> &str {
        match self {
            ClaseActivo::EfectivoEquivalente => "Efectivo y Equivalentes",
            ClaseActivo::CuentasPorCobrar => "Cuentas por Cobrar",
            ClaseActivo::Inventario => "Inventario",
            ClaseActivo::GastosPrePagados => "Gastos Prepagados",
            ClaseActivo::OtroActivoCorriente => "Otros Activos Corrientes",
            ClaseActivo::PropiedadPlantaEquipo => "Propiedad, Planta y Equipo (neto)",
            ClaseActivo::ActivoIntangible => "Activos Intangibles",
            ClaseActivo::InversionLargoPlazo => "Inversiones a Largo Plazo",
            ClaseActivo::OtroActivoNoCorriente => "Otros Activos No Corrientes",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClasePasivo {
    // Corrientes
    CuentasPorPagar,
    DeudaCortoplazo,
    ImpuestosPorPagar,
    OtroPasivoCorriente,
    // No corrientes
    DeudaLargoPlazo,
    ObligacionesLaborales,
    OtroPasivoNoCorriente,
}

impl ClasePasivo {
    pub fn es_corriente(&self) -> bool {
        matches!(
            self,
            ClasePasivo::CuentasPorPagar
                | ClasePasivo::DeudaCortoplazo
                | ClasePasivo::ImpuestosPorPagar
                | ClasePasivo::OtroPasivoCorriente
        )
    }

    pub fn nombre(&self) -> &str {
        match self {
            ClasePasivo::CuentasPorPagar => "Cuentas por Pagar",
            ClasePasivo::DeudaCortoplazo => "Deuda a Corto Plazo",
            ClasePasivo::ImpuestosPorPagar => "Impuestos por Pagar",
            ClasePasivo::OtroPasivoCorriente => "Otros Pasivos Corrientes",
            ClasePasivo::DeudaLargoPlazo => "Deuda a Largo Plazo",
            ClasePasivo::ObligacionesLaborales => "Obligaciones Laborales",
            ClasePasivo::OtroPasivoNoCorriente => "Otros Pasivos No Corrientes",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClasePatrimonio {
    CapitalSocial,
    UtilidadesRetenidas,
    ReservaLegal,
    OtroPatrimonio,
}

impl ClasePatrimonio {
    pub fn nombre(&self) -> &str {
        match self {
            ClasePatrimonio::CapitalSocial => "Capital Social",
            ClasePatrimonio::UtilidadesRetenidas => "Utilidades Retenidas",
            ClasePatrimonio::ReservaLegal => "Reserva Legal",
            ClasePatrimonio::OtroPatrimonio => "Otro Patrimonio",
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Partidas individuales
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartidaActivo {
    pub id: String,
    pub clase: ClaseActivo,
    pub descripcion: String,
    pub monto: f64,
    #[serde(default)]
    pub notas: String,
}

impl PartidaActivo {
    pub fn nueva(clase: ClaseActivo, descripcion: impl Into<String>, monto: f64) -> Self {
        PartidaActivo {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            clase,
            descripcion: descripcion.into(),
            monto,
            notas: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartidaPasivo {
    pub id: String,
    pub clase: ClasePasivo,
    pub descripcion: String,
    pub monto: f64,
    #[serde(default)]
    pub notas: String,
}

impl PartidaPasivo {
    pub fn nueva(clase: ClasePasivo, descripcion: impl Into<String>, monto: f64) -> Self {
        PartidaPasivo {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            clase,
            descripcion: descripcion.into(),
            monto,
            notas: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartidaPatrimonio {
    pub id: String,
    pub clase: ClasePatrimonio,
    pub descripcion: String,
    pub monto: f64,
    #[serde(default)]
    pub notas: String,
}

impl PartidaPatrimonio {
    pub fn nueva(clase: ClasePatrimonio, descripcion: impl Into<String>, monto: f64) -> Self {
        PartidaPatrimonio {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            clase,
            descripcion: descripcion.into(),
            monto,
            notas: String::new(),
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Balance General
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceGeneral {
    pub id: String,
    /// Fecha de corte ("al día de...").
    pub fecha_corte: NaiveDate,
    pub activos: Vec<PartidaActivo>,
    pub pasivos: Vec<PartidaPasivo>,
    pub patrimonio: Vec<PartidaPatrimonio>,
    #[serde(default)]
    pub notas: String,
}

impl BalanceGeneral {
    pub fn nuevo(fecha_corte: NaiveDate) -> Self {
        BalanceGeneral {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            fecha_corte,
            activos: Vec::new(),
            pasivos: Vec::new(),
            patrimonio: Vec::new(),
            notas: String::new(),
        }
    }

    // ── Totales ─────────────────────────────────────────────────────────────

    pub fn total_activos(&self) -> f64 {
        self.activos.iter().map(|a| a.monto).sum()
    }

    pub fn total_activos_corrientes(&self) -> f64 {
        self.activos
            .iter()
            .filter(|a| a.clase.es_corriente())
            .map(|a| a.monto)
            .sum()
    }

    pub fn total_activos_no_corrientes(&self) -> f64 {
        self.activos
            .iter()
            .filter(|a| !a.clase.es_corriente())
            .map(|a| a.monto)
            .sum()
    }

    pub fn total_pasivos(&self) -> f64 {
        self.pasivos.iter().map(|p| p.monto).sum()
    }

    pub fn total_pasivos_corrientes(&self) -> f64 {
        self.pasivos
            .iter()
            .filter(|p| p.clase.es_corriente())
            .map(|p| p.monto)
            .sum()
    }

    pub fn total_pasivos_no_corrientes(&self) -> f64 {
        self.pasivos
            .iter()
            .filter(|p| !p.clase.es_corriente())
            .map(|p| p.monto)
            .sum()
    }

    pub fn total_patrimonio(&self) -> f64 {
        self.patrimonio.iter().map(|p| p.monto).sum()
    }

    /// Verifica la ecuación fundamental: Activos = Pasivos + Patrimonio.
    /// Devuelve la diferencia (debería ser ~0).
    pub fn verificar_ecuacion(&self) -> f64 {
        self.total_activos() - (self.total_pasivos() + self.total_patrimonio())
    }

    pub fn ecuacion_cuadra(&self) -> bool {
        self.verificar_ecuacion().abs() < 0.01
    }

    // ── Ratios del Balance ───────────────────────────────────────────────────

    pub fn ratios(&self) -> RatiosBalance {
        let ac = self.total_activos_corrientes();
        let pc = self.total_pasivos_corrientes();
        let at = self.total_activos();
        let pt = self.total_patrimonio();
        let pas = self.total_pasivos();

        // Inventario para prueba ácida
        let inventario: f64 = self
            .activos
            .iter()
            .filter(|a| a.clase == ClaseActivo::Inventario)
            .map(|a| a.monto)
            .sum();

        // Efectivo para ratio de caja
        let efectivo: f64 = self
            .activos
            .iter()
            .filter(|a| a.clase == ClaseActivo::EfectivoEquivalente)
            .map(|a| a.monto)
            .sum();

        RatiosBalance {
            // Liquidez
            razon_corriente: if pc == 0.0 { f64::INFINITY } else { ac / pc },
            prueba_acida: if pc == 0.0 {
                f64::INFINITY
            } else {
                (ac - inventario) / pc
            },
            razon_caja: if pc == 0.0 {
                f64::INFINITY
            } else {
                efectivo / pc
            },
            // Solvencia / endeudamiento
            ratio_endeudamiento: if at == 0.0 { 0.0 } else { pas / at },
            ratio_deuda_patrimonio: if pt == 0.0 { f64::INFINITY } else { pas / pt },
            apalancamiento_financiero: if pt == 0.0 { f64::INFINITY } else { at / pt },
            // Valores absolutos útiles
            capital_trabajo: ac - pc,
            activos_totales: at,
            pasivos_totales: pas,
            patrimonio_total: pt,
        }
    }

    // ── Mutación ────────────────────────────────────────────────────────────

    pub fn agregar_activo(&mut self, p: PartidaActivo) {
        self.activos.push(p);
    }

    pub fn agregar_pasivo(&mut self, p: PartidaPasivo) {
        self.pasivos.push(p);
    }

    pub fn agregar_patrimonio(&mut self, p: PartidaPatrimonio) {
        self.patrimonio.push(p);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Ratios del Balance General
// ══════════════════════════════════════════════════════════════════════════════

/// Ratios financieros derivados exclusivamente del balance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatiosBalance {
    // ── Liquidez ──────────────────────────────────────────────────────────
    /// Activo corriente / Pasivo corriente. Ideal ≥ 1.5.
    pub razon_corriente: f64,
    /// (Activo corriente − Inventario) / Pasivo corriente. Ideal ≥ 1.0.
    pub prueba_acida: f64,
    /// Efectivo / Pasivo corriente. Mide capacidad inmediata de pago.
    pub razon_caja: f64,

    // ── Solvencia / endeudamiento ─────────────────────────────────────────
    /// Pasivos totales / Activos totales. Ideal < 0.5 (menos del 50 %).
    pub ratio_endeudamiento: f64,
    /// Pasivos totales / Patrimonio. Mide la dependencia de deuda vs capital propio.
    pub ratio_deuda_patrimonio: f64,
    /// Activos totales / Patrimonio (multiplicador de capital).
    pub apalancamiento_financiero: f64,

    // ── Valores absolutos ─────────────────────────────────────────────────
    /// Activo corriente − Pasivo corriente.
    pub capital_trabajo: f64,
    pub activos_totales: f64,
    pub pasivos_totales: f64,
    pub patrimonio_total: f64,
}

impl RatiosBalance {
    /// Devuelve un resumen textual legible de los ratios.
    pub fn resumen(&self) -> Vec<(String, String, &'static str)> {
        vec![
            (
                "Razón Corriente".to_string(),
                format!("{:.2}", self.razon_corriente),
                if self.razon_corriente >= 1.5 {
                    "✓ Bueno"
                } else if self.razon_corriente >= 1.0 {
                    "~ Aceptable"
                } else {
                    "✗ Crítico"
                },
            ),
            (
                "Prueba Ácida".to_string(),
                format!("{:.2}", self.prueba_acida),
                if self.prueba_acida >= 1.0 {
                    "✓ Bueno"
                } else {
                    "✗ Bajo"
                },
            ),
            (
                "Razón de Caja".to_string(),
                format!("{:.2}", self.razon_caja),
                if self.razon_caja >= 0.5 {
                    "✓ Bueno"
                } else {
                    "~ Revisar"
                },
            ),
            (
                "Ratio Endeudamiento".to_string(),
                format!("{:.1} %", self.ratio_endeudamiento * 100.0),
                if self.ratio_endeudamiento <= 0.5 {
                    "✓ Bueno"
                } else if self.ratio_endeudamiento <= 0.7 {
                    "~ Moderado"
                } else {
                    "✗ Alto"
                },
            ),
            (
                "Deuda / Patrimonio".to_string(),
                format!("{:.2}x", self.ratio_deuda_patrimonio),
                if self.ratio_deuda_patrimonio <= 1.0 {
                    "✓ Bueno"
                } else if self.ratio_deuda_patrimonio <= 2.0 {
                    "~ Moderado"
                } else {
                    "✗ Alto"
                },
            ),
            (
                "Apalancamiento".to_string(),
                format!("{:.2}x", self.apalancamiento_financiero),
                "",
            ),
            (
                "Capital de Trabajo".to_string(),
                format!("{:.2}", self.capital_trabajo),
                if self.capital_trabajo >= 0.0 {
                    "✓ Positivo"
                } else {
                    "✗ Negativo"
                },
            ),
        ]
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Almacén de balances
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlmacenBalances {
    pub balances: Vec<BalanceGeneral>,
}

impl AlmacenBalances {
    pub fn agregar(&mut self, b: BalanceGeneral) {
        self.balances.push(b);
    }

    /// Balance más reciente.
    pub fn ultimo(&self) -> Option<&BalanceGeneral> {
        self.balances.iter().max_by_key(|b| b.fecha_corte)
    }

    /// Comparar activos totales entre dos fechas de corte.
    /// Devuelve (fecha_a, fecha_b, variación_absoluta, variación_%).
    pub fn comparar_activos(
        &self,
        fecha_a: NaiveDate,
        fecha_b: NaiveDate,
    ) -> Option<(f64, f64, f64, f64)> {
        let ba = self.balances.iter().find(|b| b.fecha_corte == fecha_a)?;
        let bb = self.balances.iter().find(|b| b.fecha_corte == fecha_b)?;
        let ta = ba.total_activos();
        let tb = bb.total_activos();
        let variacion = tb - ta;
        let porcentaje = if ta == 0.0 {
            0.0
        } else {
            variacion / ta * 100.0
        };
        Some((ta, tb, variacion, porcentaje))
    }
}
