//! Estado de Resultados — rendimiento financiero durante un período.
//!
//! Estructura en cascada:
//! ```text
//! (+) Ingresos Operacionales
//! (−) Costo de Ventas / Costo de Servicios
//!  =  Utilidad Bruta
//! (−) Gastos Operacionales (administración, ventas, depreciación…)
//!  =  EBIT  (Utilidad Operacional)
//! (−) Gastos Financieros (intereses de deuda)
//! (+) Ingresos Financieros (rendimiento de inversiones)
//!  =  EBT   (Utilidad antes de Impuestos)
//! (−) Impuesto a la Renta
//!  =  Utilidad Neta
//! ```
//!
//! Los ratios de rentabilidad se calculan automáticamente.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ══════════════════════════════════════════════════════════════════════════════
//  Clasificación de líneas
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClaseIngreso {
    IngresoOperacional,
    OtroIngreso,
    IngresoFinanciero,
}

impl ClaseIngreso {
    pub fn nombre(&self) -> &str {
        match self {
            ClaseIngreso::IngresoOperacional => "Ingreso Operacional",
            ClaseIngreso::OtroIngreso => "Otro Ingreso",
            ClaseIngreso::IngresoFinanciero => "Ingreso Financiero",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClaseCosto {
    CostoVentas,
    CostoServicio,
}

impl ClaseCosto {
    pub fn nombre(&self) -> &str {
        match self {
            ClaseCosto::CostoVentas => "Costo de Ventas",
            ClaseCosto::CostoServicio => "Costo de Servicio",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClaseGasto {
    GastoAdministrativo,
    GastoVentas,
    GastoDepreciacion,
    GastoAmortizacion,
    GastoFinanciero,
    Impuesto,
    OtroGasto,
}

impl ClaseGasto {
    pub fn nombre(&self) -> &str {
        match self {
            ClaseGasto::GastoAdministrativo => "Gasto Administrativo",
            ClaseGasto::GastoVentas => "Gasto de Ventas",
            ClaseGasto::GastoDepreciacion => "Depreciación",
            ClaseGasto::GastoAmortizacion => "Amortización",
            ClaseGasto::GastoFinanciero => "Gasto Financiero (intereses)",
            ClaseGasto::Impuesto => "Impuesto a la Renta",
            ClaseGasto::OtroGasto => "Otro Gasto",
        }
    }

    /// `true` si el gasto va por encima del EBIT (operacional).
    pub fn es_operacional(&self) -> bool {
        matches!(
            self,
            ClaseGasto::GastoAdministrativo
                | ClaseGasto::GastoVentas
                | ClaseGasto::GastoDepreciacion
                | ClaseGasto::GastoAmortizacion
                | ClaseGasto::OtroGasto
        )
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Partidas individuales
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartidaIngreso {
    pub id: String,
    pub clase: ClaseIngreso,
    pub descripcion: String,
    /// Siempre positivo.
    pub monto: f64,
    #[serde(default)]
    pub notas: String,
}

impl PartidaIngreso {
    pub fn nueva(clase: ClaseIngreso, descripcion: impl Into<String>, monto: f64) -> Self {
        PartidaIngreso {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            clase,
            descripcion: descripcion.into(),
            monto: monto.abs(),
            notas: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartidaCosto {
    pub id: String,
    pub clase: ClaseCosto,
    pub descripcion: String,
    /// Siempre positivo.
    pub monto: f64,
    #[serde(default)]
    pub notas: String,
}

impl PartidaCosto {
    pub fn nueva(clase: ClaseCosto, descripcion: impl Into<String>, monto: f64) -> Self {
        PartidaCosto {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            clase,
            descripcion: descripcion.into(),
            monto: monto.abs(),
            notas: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartidaGasto {
    pub id: String,
    pub clase: ClaseGasto,
    pub descripcion: String,
    /// Siempre positivo.
    pub monto: f64,
    #[serde(default)]
    pub notas: String,
}

impl PartidaGasto {
    pub fn nuevo(clase: ClaseGasto, descripcion: impl Into<String>, monto: f64) -> Self {
        PartidaGasto {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            clase,
            descripcion: descripcion.into(),
            monto: monto.abs(),
            notas: String::new(),
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Estado de Resultados
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EstadoResultados {
    pub id: String,
    pub fecha_inicio: NaiveDate,
    pub fecha_fin: NaiveDate,
    pub ingresos: Vec<PartidaIngreso>,
    pub costos: Vec<PartidaCosto>,
    pub gastos: Vec<PartidaGasto>,
    #[serde(default)]
    pub notas: String,
}

impl EstadoResultados {
    pub fn nuevo(fecha_inicio: NaiveDate, fecha_fin: NaiveDate) -> Self {
        EstadoResultados {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            fecha_inicio,
            fecha_fin,
            ingresos: Vec::new(),
            costos: Vec::new(),
            gastos: Vec::new(),
            notas: String::new(),
        }
    }

    // ── Mutación ─────────────────────────────────────────────────────────────

    pub fn agregar_ingreso(&mut self, p: PartidaIngreso) {
        self.ingresos.push(p);
    }

    pub fn agregar_costo(&mut self, p: PartidaCosto) {
        self.costos.push(p);
    }

    pub fn agregar_gasto(&mut self, p: PartidaGasto) {
        self.gastos.push(p);
    }

    // ── Subtotales en cascada ────────────────────────────────────────────────

    /// Suma de todos los ingresos operacionales.
    pub fn ingresos_operacionales(&self) -> f64 {
        self.ingresos
            .iter()
            .filter(|i| i.clase == ClaseIngreso::IngresoOperacional)
            .map(|i| i.monto)
            .sum()
    }

    /// Suma de ingresos financieros.
    pub fn ingresos_financieros(&self) -> f64 {
        self.ingresos
            .iter()
            .filter(|i| i.clase == ClaseIngreso::IngresoFinanciero)
            .map(|i| i.monto)
            .sum()
    }

    /// Suma de todos los ingresos (operacionales + otros + financieros).
    pub fn total_ingresos(&self) -> f64 {
        self.ingresos.iter().map(|i| i.monto).sum()
    }

    /// Suma del costo de ventas / servicio.
    pub fn total_costos(&self) -> f64 {
        self.costos.iter().map(|c| c.monto).sum()
    }

    /// **Utilidad Bruta** = Ingresos operacionales − Costos.
    pub fn utilidad_bruta(&self) -> f64 {
        self.ingresos_operacionales() - self.total_costos()
    }

    /// Gastos operacionales (excluye financieros e impuestos).
    pub fn gastos_operacionales(&self) -> f64 {
        self.gastos
            .iter()
            .filter(|g| g.clase.es_operacional())
            .map(|g| g.monto)
            .sum()
    }

    /// Depreciación + Amortización.
    pub fn depreciacion_amortizacion(&self) -> f64 {
        self.gastos
            .iter()
            .filter(|g| {
                g.clase == ClaseGasto::GastoDepreciacion || g.clase == ClaseGasto::GastoAmortizacion
            })
            .map(|g| g.monto)
            .sum()
    }

    /// **EBIT** (Earnings Before Interest & Taxes) = Utilidad Operacional.
    pub fn ebit(&self) -> f64 {
        self.utilidad_bruta() - self.gastos_operacionales()
    }

    /// **EBITDA** = EBIT + Depreciación + Amortización.
    pub fn ebitda(&self) -> f64 {
        self.ebit() + self.depreciacion_amortizacion()
    }

    /// Gastos financieros (intereses).
    pub fn gastos_financieros(&self) -> f64 {
        self.gastos
            .iter()
            .filter(|g| g.clase == ClaseGasto::GastoFinanciero)
            .map(|g| g.monto)
            .sum()
    }

    /// **EBT** (Earnings Before Taxes) = EBIT + ingresos financieros − gastos financieros.
    pub fn ebt(&self) -> f64 {
        self.ebit() + self.ingresos_financieros() - self.gastos_financieros()
    }

    /// Impuesto a la renta registrado.
    pub fn impuesto(&self) -> f64 {
        self.gastos
            .iter()
            .filter(|g| g.clase == ClaseGasto::Impuesto)
            .map(|g| g.monto)
            .sum()
    }

    /// **Utilidad Neta** = EBT − Impuesto.
    pub fn utilidad_neta(&self) -> f64 {
        self.ebt() - self.impuesto()
    }

    // ── Ratios de rentabilidad ────────────────────────────────────────────────

    /// Calcula los ratios de rentabilidad.
    /// `activos_totales` y `patrimonio` se obtienen del balance general correspondiente.
    pub fn ratios(&self, activos_totales: f64, patrimonio: f64) -> RatiosResultados {
        let ingresos = self.ingresos_operacionales();
        let utilidad_bruta = self.utilidad_bruta();
        let ebit = self.ebit();
        let ebitda = self.ebitda();
        let utilidad_neta = self.utilidad_neta();

        let margen_bruto = if ingresos == 0.0 {
            0.0
        } else {
            utilidad_bruta / ingresos
        };
        let margen_operacional = if ingresos == 0.0 {
            0.0
        } else {
            ebit / ingresos
        };
        let margen_ebitda = if ingresos == 0.0 {
            0.0
        } else {
            ebitda / ingresos
        };
        let margen_neto = if ingresos == 0.0 {
            0.0
        } else {
            utilidad_neta / ingresos
        };
        let roa = if activos_totales == 0.0 {
            0.0
        } else {
            utilidad_neta / activos_totales
        };
        let roe = if patrimonio == 0.0 {
            0.0
        } else {
            utilidad_neta / patrimonio
        };
        let cobertura_intereses = if self.gastos_financieros() == 0.0 {
            f64::INFINITY
        } else {
            ebit / self.gastos_financieros()
        };

        RatiosResultados {
            margen_bruto,
            margen_operacional,
            margen_ebitda,
            margen_neto,
            roa,
            roe,
            cobertura_intereses,
            ingresos_operacionales: ingresos,
            utilidad_bruta,
            ebit,
            ebitda,
            utilidad_neta,
        }
    }

    /// Vista en cascada lista para mostrar en pantalla.
    /// Devuelve: Vec<(etiqueta, valor, es_subtotal)>.
    pub fn vista_cascada(&self) -> Vec<(String, f64, bool)> {
        vec![
            (
                "(+) Ingresos Operacionales".to_string(),
                self.ingresos_operacionales(),
                false,
            ),
            (
                "(−) Costo de Ventas/Servicio".to_string(),
                self.total_costos(),
                false,
            ),
            (
                "  = Utilidad Bruta".to_string(),
                self.utilidad_bruta(),
                true,
            ),
            (
                "(−) Gastos Operacionales".to_string(),
                self.gastos_operacionales(),
                false,
            ),
            (
                "  = EBIT (Utilidad Operacional)".to_string(),
                self.ebit(),
                true,
            ),
            ("  + EBITDA".to_string(), self.ebitda(), true),
            (
                "(+) Ingresos Financieros".to_string(),
                self.ingresos_financieros(),
                false,
            ),
            (
                "(−) Gastos Financieros".to_string(),
                self.gastos_financieros(),
                false,
            ),
            ("  = EBT (Antes de Impuestos)".to_string(), self.ebt(), true),
            (
                "(−) Impuesto a la Renta".to_string(),
                self.impuesto(),
                false,
            ),
            ("  = Utilidad Neta".to_string(), self.utilidad_neta(), true),
        ]
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Ratios de Rentabilidad
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatiosResultados {
    // ── Márgenes ──────────────────────────────────────────────────────────────
    /// (Utilidad Bruta / Ingresos). Ideal varía por industria; > 30 % es bueno en servicios.
    pub margen_bruto: f64,
    /// (EBIT / Ingresos). Eficiencia operativa.
    pub margen_operacional: f64,
    /// (EBITDA / Ingresos). Capacidad de generar caja antes de cargas no caja.
    pub margen_ebitda: f64,
    /// (Utilidad Neta / Ingresos). Rentabilidad final.
    pub margen_neto: f64,

    // ── Rentabilidad sobre activos/patrimonio ─────────────────────────────────
    /// Return on Assets: Utilidad Neta / Activos Totales.
    pub roa: f64,
    /// Return on Equity: Utilidad Neta / Patrimonio.
    pub roe: f64,

    // ── Cobertura ─────────────────────────────────────────────────────────────
    /// EBIT / Gastos Financieros. Capacidad de pagar intereses. Ideal ≥ 3x.
    pub cobertura_intereses: f64,

    // ── Valores absolutos de apoyo ────────────────────────────────────────────
    pub ingresos_operacionales: f64,
    pub utilidad_bruta: f64,
    pub ebit: f64,
    pub ebitda: f64,
    pub utilidad_neta: f64,
}

impl RatiosResultados {
    /// Resumen textual con semáforo de colores en texto.
    pub fn resumen(&self) -> Vec<(String, String, &'static str)> {
        vec![
            (
                "Margen Bruto".to_string(),
                format!("{:.1} %", self.margen_bruto * 100.0),
                if self.margen_bruto >= 0.3 {
                    "✓ Bueno"
                } else if self.margen_bruto >= 0.15 {
                    "~ Moderado"
                } else {
                    "✗ Bajo"
                },
            ),
            (
                "Margen Operacional".to_string(),
                format!("{:.1} %", self.margen_operacional * 100.0),
                if self.margen_operacional >= 0.1 {
                    "✓ Bueno"
                } else if self.margen_operacional >= 0.05 {
                    "~ Moderado"
                } else {
                    "✗ Bajo"
                },
            ),
            (
                "Margen EBITDA".to_string(),
                format!("{:.1} %", self.margen_ebitda * 100.0),
                if self.margen_ebitda >= 0.15 {
                    "✓ Bueno"
                } else {
                    "~ Revisar"
                },
            ),
            (
                "Margen Neto".to_string(),
                format!("{:.1} %", self.margen_neto * 100.0),
                if self.margen_neto >= 0.1 {
                    "✓ Bueno"
                } else if self.margen_neto >= 0.05 {
                    "~ Moderado"
                } else {
                    "✗ Bajo"
                },
            ),
            (
                "ROA".to_string(),
                format!("{:.1} %", self.roa * 100.0),
                if self.roa >= 0.05 {
                    "✓ Bueno"
                } else {
                    "~ Revisar"
                },
            ),
            (
                "ROE".to_string(),
                format!("{:.1} %", self.roe * 100.0),
                if self.roe >= 0.1 {
                    "✓ Bueno"
                } else {
                    "~ Revisar"
                },
            ),
            (
                "Cobertura Intereses".to_string(),
                format!("{:.2}x", self.cobertura_intereses),
                if self.cobertura_intereses >= 3.0 {
                    "✓ Bueno"
                } else if self.cobertura_intereses >= 1.5 {
                    "~ Moderado"
                } else {
                    "✗ Crítico"
                },
            ),
        ]
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Almacén de estados de resultados
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlmacenResultados {
    pub estados: Vec<EstadoResultados>,
}

impl AlmacenResultados {
    pub fn agregar(&mut self, er: EstadoResultados) {
        self.estados.push(er);
    }

    /// El estado más reciente (por fecha de fin).
    pub fn ultimo(&self) -> Option<&EstadoResultados> {
        self.estados.iter().max_by_key(|e| e.fecha_fin)
    }

    /// Compara la utilidad neta de dos períodos.
    /// Devuelve (utilidad_a, utilidad_b, variación_absoluta, variación_%).
    pub fn comparar_utilidad(
        &self,
        fin_a: NaiveDate,
        fin_b: NaiveDate,
    ) -> Option<(f64, f64, f64, f64)> {
        let ea = self.estados.iter().find(|e| e.fecha_fin == fin_a)?;
        let eb = self.estados.iter().find(|e| e.fecha_fin == fin_b)?;
        let ua = ea.utilidad_neta();
        let ub = eb.utilidad_neta();
        let variacion = ub - ua;
        let pct = if ua == 0.0 {
            0.0
        } else {
            variacion / ua.abs() * 100.0
        };
        Some((ua, ub, variacion, pct))
    }

    /// Tendencia de ingresos operacionales (cronológica).
    pub fn tendencia_ingresos(&self) -> Vec<(NaiveDate, f64)> {
        let mut v: Vec<(NaiveDate, f64)> = self
            .estados
            .iter()
            .map(|e| (e.fecha_fin, e.ingresos_operacionales()))
            .collect();
        v.sort_by_key(|(d, _)| *d);
        v
    }
}
