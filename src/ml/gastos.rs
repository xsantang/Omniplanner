//! Registro de gastos reales — tracking de transacciones individuales.
//!
//! Complementa al [`super::presupuesto_cero`] que solo maneja presupuesto
//! planificado. Este módulo registra cada gasto/ingreso real con fecha,
//! monto, categoría y notas para análisis y reconciliación.

use super::presupuesto_cero::Categoria;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dinero::Dinero;

// ─── Transacción individual ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GastoReal {
    pub id: String,
    pub fecha: NaiveDate,
    pub descripcion: String,
    pub categoria: Categoria,
    /// Monto positivo = gasto/egreso; negativo = ingreso/reembolso
    pub monto: Dinero,
    /// Nombre de la línea de presupuesto a la que pertenece (opcional)
    #[serde(default)]
    pub linea_presupuesto: Option<String>,
    /// Etiqueta libre (ej: "efectivo", "tarjeta", "transferencia")
    #[serde(default)]
    pub metodo_pago: String,
    #[serde(default)]
    pub notas: String,
}

impl GastoReal {
    pub fn nuevo(
        fecha: NaiveDate,
        descripcion: impl Into<String>,
        categoria: Categoria,
        monto: f64,
    ) -> Self {
        GastoReal {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            fecha,
            descripcion: descripcion.into(),
            categoria,
            monto: Dinero::desde_f64(monto),
            linea_presupuesto: None,
            metodo_pago: String::new(),
            notas: String::new(),
        }
    }

    pub fn es_ingreso(&self) -> bool {
        self.monto.es_negativo()
    }

    pub fn monto_abs(&self) -> Dinero {
        self.monto.abs()
    }
}

// ─── Almacén principal ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlmacenGastos {
    pub transacciones: Vec<GastoReal>,
}

impl AlmacenGastos {
    /// Agregar una transacción
    pub fn agregar(&mut self, g: GastoReal) {
        self.transacciones.push(g);
    }

    /// Eliminar por ID
    pub fn eliminar(&mut self, id: &str) -> bool {
        let antes = self.transacciones.len();
        self.transacciones.retain(|g| g.id != id);
        self.transacciones.len() < antes
    }

    /// Total de gastos (positivos) en un rango de fechas
    pub fn total_gastos_rango(&self, desde: NaiveDate, hasta: NaiveDate) -> Dinero {
        self.transacciones
            .iter()
            .filter(|g| g.fecha >= desde && g.fecha <= hasta && g.monto.es_positivo())
            .map(|g| g.monto)
            .sum()
    }

    /// Total de ingresos (negativos → abs) en un rango de fechas
    pub fn total_ingresos_rango(&self, desde: NaiveDate, hasta: NaiveDate) -> Dinero {
        self.transacciones
            .iter()
            .filter(|g| g.fecha >= desde && g.fecha <= hasta && g.monto.es_negativo())
            .map(|g| g.monto.abs())
            .sum()
    }

    /// Gastos agrupados por categoría en un rango
    pub fn por_categoria(&self, desde: NaiveDate, hasta: NaiveDate) -> Vec<(Categoria, Dinero)> {
        let mut mapa: Vec<(Categoria, Dinero)> = Vec::new();
        for g in self
            .transacciones
            .iter()
            .filter(|g| g.fecha >= desde && g.fecha <= hasta && g.monto.es_positivo())
        {
            if let Some(entry) = mapa.iter_mut().find(|(c, _)| c == &g.categoria) {
                entry.1 += g.monto;
            } else {
                mapa.push((g.categoria.clone(), g.monto));
            }
        }
        // Ordenar de mayor a menor gasto (Dinero: Ord, sin unwrap)
        mapa.sort_by(|a, b| b.1.cmp(&a.1));
        mapa
    }

    /// Transacciones del mes actual (año-mes)
    pub fn del_mes(&self, anio: i32, mes: u32) -> Vec<&GastoReal> {
        self.transacciones
            .iter()
            .filter(|g| g.fecha.year() == anio && g.fecha.month() == mes)
            .collect()
    }

    /// Busca transacciones cuya descripción contenga la palabra clave (case-insensitive, sin tildes).
    /// Devuelve referencias ordenadas de más reciente a más antigua.
    pub fn buscar_por_keyword(&self, keyword: &str) -> Vec<&GastoReal> {
        let kw = sin_tildes_gastos(&keyword.to_lowercase());
        let mut resultado: Vec<&GastoReal> = self
            .transacciones
            .iter()
            .filter(|g| sin_tildes_gastos(&g.descripcion.to_lowercase()).contains(&kw))
            .collect();
        resultado.sort_by_key(|g| std::cmp::Reverse(g.fecha));
        resultado
    }

    /// Resumen textual de un mes: total gasto, total ingreso, balance
    pub fn resumen_mes(&self, anio: i32, mes: u32) -> ResumenMes {
        let transacciones = self.del_mes(anio, mes);
        let total_gastos: Dinero = transacciones
            .iter()
            .filter(|g| g.monto.es_positivo())
            .map(|g| g.monto)
            .sum();
        let total_ingresos: Dinero = transacciones
            .iter()
            .filter(|g| g.monto.es_negativo())
            .map(|g| g.monto.abs())
            .sum();
        ResumenMes {
            anio,
            mes,
            total_gastos,
            total_ingresos,
            balance: total_ingresos - total_gastos,
            num_transacciones: transacciones.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResumenMes {
    pub anio: i32,
    pub mes: u32,
    pub total_gastos: Dinero,
    pub total_ingresos: Dinero,
    pub balance: Dinero,
    pub num_transacciones: usize,
}

// Necesario para NaiveDate::year()/month() sin ambigüedad
use chrono::Datelike;

/// Elimina tildes para búsquedas tolerantes a acentuación.
fn sin_tildes_gastos(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' => 'a',
            'é' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'ï' => 'i',
            'ó' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            'ñ' => 'n',
            other => other,
        })
        .collect()
}
