//! Presupuesto Base Cero — cada dólar tiene un destino, saldo final = $0.
//!
//! Sistema quincenal inspirado en el método "zero-based budget":
//! Ingreso total - Gastos fijos - Gastos variables - Pagos deuda - Ahorro = 0

use serde::{Deserialize, Serialize};

// ─── Categorías ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Categoria {
    Ingreso,
    GastoFijo,
    GastoVariable,
    PagoDeuda,
    Ahorro,
}

impl Categoria {
    pub fn nombre(&self) -> &str {
        match self {
            Categoria::Ingreso => "Ingreso",
            Categoria::GastoFijo => "Gasto Fijo",
            Categoria::GastoVariable => "Gasto Variable",
            Categoria::PagoDeuda => "Pago de Deuda",
            Categoria::Ahorro => "Ahorro",
        }
    }

    pub fn emoji(&self) -> &str {
        match self {
            Categoria::Ingreso => "💵",
            Categoria::GastoFijo => "🏠",
            Categoria::GastoVariable => "🛒",
            Categoria::PagoDeuda => "💳",
            Categoria::Ahorro => "🏦",
        }
    }
}

// ─── Línea individual del presupuesto ───────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineaPresupuesto {
    pub nombre: String,
    pub categoria: Categoria,
    pub monto: f64,
    pub pagado: bool,
    #[serde(default)]
    pub fecha_limite: String,
    #[serde(default)]
    pub notas: String,
    /// Saldo total de la deuda (solo para PagoDeuda)
    #[serde(default)]
    pub saldo_total_deuda: Option<f64>,
}

// ─── Presupuesto mensual completo ───────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresupuestoMensual {
    pub mes: String, // "2026-04"
    pub lineas: Vec<LineaPresupuesto>,
}

impl PresupuestoMensual {
    pub fn nuevo(mes: &str) -> Self {
        Self {
            mes: mes.to_string(),
            lineas: Vec::new(),
        }
    }

    pub fn agregar(&mut self, linea: LineaPresupuesto) {
        self.lineas.push(linea);
    }

    pub fn total_ingresos(&self) -> f64 {
        self.lineas
            .iter()
            .filter(|l| l.categoria == Categoria::Ingreso)
            .map(|l| l.monto)
            .sum()
    }

    pub fn total_gastos_fijos(&self) -> f64 {
        self.lineas
            .iter()
            .filter(|l| l.categoria == Categoria::GastoFijo)
            .map(|l| l.monto)
            .sum()
    }

    pub fn total_gastos_variables(&self) -> f64 {
        self.lineas
            .iter()
            .filter(|l| l.categoria == Categoria::GastoVariable)
            .map(|l| l.monto)
            .sum()
    }

    pub fn total_pagos_deuda(&self) -> f64 {
        self.lineas
            .iter()
            .filter(|l| l.categoria == Categoria::PagoDeuda)
            .map(|l| l.monto)
            .sum()
    }

    pub fn total_ahorro(&self) -> f64 {
        self.lineas
            .iter()
            .filter(|l| l.categoria == Categoria::Ahorro)
            .map(|l| l.monto)
            .sum()
    }

    pub fn total_egresos(&self) -> f64 {
        self.total_gastos_fijos()
            + self.total_gastos_variables()
            + self.total_pagos_deuda()
            + self.total_ahorro()
    }

    pub fn saldo(&self) -> f64 {
        self.total_ingresos() - self.total_egresos()
    }

    pub fn total_pagado(&self) -> f64 {
        self.lineas
            .iter()
            .filter(|l| l.pagado && l.categoria != Categoria::Ingreso)
            .map(|l| l.monto)
            .sum()
    }

    pub fn total_pendiente(&self) -> f64 {
        self.lineas
            .iter()
            .filter(|l| !l.pagado && l.categoria != Categoria::Ingreso)
            .map(|l| l.monto)
            .sum()
    }

    pub fn por_categoria(&self, cat: &Categoria) -> Vec<&LineaPresupuesto> {
        self.lineas.iter().filter(|l| &l.categoria == cat).collect()
    }

    /// Devuelve info de deudas con saldo total (nombre, pago mensual, saldo total, meses restantes)
    pub fn info_deudas(&self) -> Vec<(&str, f64, f64, u32)> {
        self.lineas
            .iter()
            .filter(|l| l.categoria == Categoria::PagoDeuda && l.saldo_total_deuda.is_some())
            .map(|l| {
                let saldo = l.saldo_total_deuda.unwrap();
                let meses = if l.monto > 0.0 {
                    (saldo / l.monto).ceil() as u32
                } else {
                    0
                };
                (l.nombre.as_str(), l.monto, saldo, meses)
            })
            .collect()
    }

    /// Calcula el resumen del presupuesto
    pub fn resumen(&self) -> ResumenPresupuesto {
        let ingresos = self.total_ingresos();
        let fijos = self.total_gastos_fijos();
        let variables = self.total_gastos_variables();
        let deuda = self.total_pagos_deuda();
        let ahorro = self.total_ahorro();
        let egresos = fijos + variables + deuda + ahorro;
        let saldo = ingresos - egresos;
        let pagado = self.total_pagado();
        let pendiente = self.total_pendiente();

        let pct_fijos = if ingresos > 0.0 { fijos / ingresos * 100.0 } else { 0.0 };
        let pct_variables = if ingresos > 0.0 { variables / ingresos * 100.0 } else { 0.0 };
        let pct_deuda = if ingresos > 0.0 { deuda / ingresos * 100.0 } else { 0.0 };
        let pct_ahorro = if ingresos > 0.0 { ahorro / ingresos * 100.0 } else { 0.0 };

        let salud = if saldo.abs() < 0.01 {
            SaludPresupuesto::Perfecto
        } else if saldo > 0.0 {
            SaludPresupuesto::SobraDinero(saldo)
        } else {
            SaludPresupuesto::FaltaDinero(-saldo)
        };

        let mut alertas = Vec::new();

        if pct_deuda > 40.0 {
            alertas.push(format!(
                "⚠️ {:.0}% de tu ingreso va a deudas — lo ideal es <35%",
                pct_deuda
            ));
        }
        if pct_ahorro < 5.0 && ahorro > 0.0 {
            alertas.push(format!(
                "⚠️ Solo {:.0}% va a ahorro — intenta llegar al 10%",
                pct_ahorro
            ));
        } else if ahorro == 0.0 {
            alertas.push("⚠️ No tienes ahorro asignado — aunque sea $25 ayudan".into());
        }
        if let SaludPresupuesto::FaltaDinero(f) = &salud {
            alertas.push(format!(
                "🔴 Te faltan ${:.2} — necesitas recortar gastos o más ingreso",
                f
            ));
        }

        ResumenPresupuesto {
            ingresos,
            gastos_fijos: fijos,
            gastos_variables: variables,
            pagos_deuda: deuda,
            ahorro,
            egresos,
            saldo,
            pagado,
            pendiente,
            pct_fijos,
            pct_variables,
            pct_deuda,
            pct_ahorro,
            salud,
            alertas,
        }
    }
}

#[derive(Debug, Clone)]
pub enum SaludPresupuesto {
    Perfecto,
    SobraDinero(f64),
    FaltaDinero(f64),
}

#[derive(Debug, Clone)]
pub struct ResumenPresupuesto {
    pub ingresos: f64,
    pub gastos_fijos: f64,
    pub gastos_variables: f64,
    pub pagos_deuda: f64,
    pub ahorro: f64,
    pub egresos: f64,
    pub saldo: f64,
    pub pagado: f64,
    pub pendiente: f64,
    pub pct_fijos: f64,
    pub pct_variables: f64,
    pub pct_deuda: f64,
    pub pct_ahorro: f64,
    pub salud: SaludPresupuesto,
    pub alertas: Vec<String>,
}

// ─── Plantilla reutilizable ─────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlantillaPresupuesto {
    pub nombre: String,
    pub lineas: Vec<LineaPlantilla>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineaPlantilla {
    pub nombre: String,
    pub categoria: Categoria,
    pub monto_default: f64,
    pub fecha_limite: String,
    /// Saldo total de la deuda (solo para PagoDeuda)
    #[serde(default)]
    pub saldo_total_deuda: Option<f64>,
}

impl PlantillaPresupuesto {
    /// Genera un presupuesto nuevo a partir de la plantilla
    pub fn generar_mes(&self, mes: &str) -> PresupuestoMensual {
        let lineas = self
            .lineas
            .iter()
            .map(|pl| LineaPresupuesto {
                nombre: pl.nombre.clone(),
                categoria: pl.categoria.clone(),
                monto: pl.monto_default,
                pagado: false,
                fecha_limite: pl.fecha_limite.clone(),
                notas: String::new(),
                saldo_total_deuda: pl.saldo_total_deuda,
            })
            .collect();

        PresupuestoMensual {
            mes: mes.to_string(),
            lineas,
        }
    }
}

// ─── Almacén persistente ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlmacenPresupuesto {
    #[serde(default)]
    pub plantilla: Option<PlantillaPresupuesto>,
    #[serde(default)]
    pub meses: Vec<PresupuestoMensual>,
}

impl AlmacenPresupuesto {
    pub fn mes_actual(&self, mes: &str) -> Option<&PresupuestoMensual> {
        self.meses.iter().find(|m| m.mes == mes)
    }

    pub fn mes_actual_mut(&mut self, mes: &str) -> Option<&mut PresupuestoMensual> {
        self.meses.iter_mut().find(|m| m.mes == mes)
    }
}

// ─── Importar desde Excel con calamine ──────────────────────

use calamine::{open_workbook, Reader, Xlsx};
use std::path::Path;

/// Resultado de importar un Excel de pagos
#[derive(Debug)]
pub struct ImportacionExcel {
    pub meses_importados: Vec<PresupuestoMensual>,
    pub errores: Vec<String>,
}

/// Intenta reconocer la categoría por el nombre de la cuenta
fn categorizar(nombre: &str) -> Categoria {
    let n = nombre.to_lowercase();
    if n.contains("sueldo") || n.contains("salary") || n.contains("income") || n.contains("army") {
        Categoria::Ingreso
    } else if n.contains("saving") || n.contains("ahorro") {
        Categoria::Ahorro
    } else if n.contains("casa") || n.contains("mortgage") || n.contains("rent")
        || n.contains("arriendo") || n.contains("carro") || n.contains("hyundai")
        || n.contains("toyota") || n.contains("motor finance")
        || n.contains("att") || n.contains("gci") || n.contains("windstream")
        || n.contains("canoochee") || n.contains("usaa") || n.contains("electric")
        || n.contains("pago de carro")
    {
        Categoria::GastoFijo
    } else if n.contains("bofa") || n.contains("discover") || n.contains("amazon")
        || n.contains("american express") || n.contains("amex")
        || n.contains("dell") || n.contains("navient") || n.contains("coma")
        || n.contains("wyndham")
    {
        Categoria::PagoDeuda
    } else if n.contains("otros") || n.contains("other") {
        Categoria::GastoVariable
    } else {
        Categoria::GastoVariable
    }
}

/// Importa un archivo Excel con el formato del usuario
/// (cada hoja = un mes, dos quincenas por hoja)
pub fn importar_excel(ruta: &Path) -> ImportacionExcel {
    let mut resultado = ImportacionExcel {
        meses_importados: Vec::new(),
        errores: Vec::new(),
    };

    let workbook: Result<Xlsx<_>, _> = open_workbook(ruta);
    let mut wb = match workbook {
        Ok(wb) => wb,
        Err(e) => {
            resultado
                .errores
                .push(format!("No se pudo abrir: {}", e));
            return resultado;
        }
    };

    let nombres: Vec<String> = wb.sheet_names().to_vec();

    for nombre_hoja in &nombres {
        let range = match wb.worksheet_range(nombre_hoja) {
            Ok(r) => r,
            Err(e) => {
                resultado
                    .errores
                    .push(format!("Error leyendo hoja '{}': {}", nombre_hoja, e));
                continue;
            }
        };

        let mut pres = PresupuestoMensual::nuevo(nombre_hoja);
        let rows: Vec<Vec<String>> = range
            .rows()
            .map(|row| {
                row.iter()
                    .map(|c| match c {
                        calamine::Data::String(s) => s.clone(),
                        calamine::Data::Float(f) => format!("{:.2}", f),
                        calamine::Data::Int(i) => i.to_string(),
                        _ => String::new(),
                    })
                    .collect()
            })
            .collect();

        // Buscar la columna del lado derecho que tiene los pagos distribuidos
        // Patrón: columna ~11-14 tiene nombre, ~columna después tiene monto
        // Buscar "Sueldo" para identificar ingreso
        for row in &rows {
            // Lado derecho: buscar pares (nombre, monto) en columnas 11+
            // El formato varía por hoja, pero típicamente:
            //   col ~11 o ~14: nombre del concepto
            //   col ~13 o ~16: monto
            let nombre_col_right = find_name_in_right_side(row, 11);
            if let Some((nombre, monto)) = nombre_col_right {
                if nombre.to_lowercase().contains("suma total")
                    || nombre.to_lowercase().contains("saldo en la cuenta")
                    || nombre.to_lowercase().contains("quincena")
                    || nombre.to_lowercase().contains("notas")
                    || nombre.is_empty()
                {
                    continue;
                }

                if monto > 0.01 {
                    let cat = categorizar(&nombre);
                    pres.agregar(LineaPresupuesto {
                        nombre,
                        categoria: cat,
                        monto,
                        pagado: true, // datos históricos
                        fecha_limite: String::new(),
                        notas: nombre_hoja.clone(),
                        saldo_total_deuda: None,
                    });
                }
            }
        }

        if !pres.lineas.is_empty() {
            resultado.meses_importados.push(pres);
        }
    }

    resultado
}

/// Busca un par (nombre, monto) en el lado derecho de una fila
fn find_name_in_right_side(row: &[String], start_col: usize) -> Option<(String, f64)> {
    if row.len() <= start_col {
        return None;
    }

    // Buscar el primer string no vacío desde start_col
    let mut nombre = String::new();
    let mut monto = 0.0f64;

    for i in start_col..row.len() {
        let cell = row[i].trim();
        if cell.is_empty() {
            continue;
        }
        // ¿Es un número?
        if let Ok(val) = cell.replace(',', "").parse::<f64>() {
            if nombre.is_empty() {
                continue; // número sin nombre, skip
            }
            monto = val;
            return Some((nombre, monto));
        } else if nombre.is_empty() {
            nombre = cell.to_string();
        }
    }

    if !nombre.is_empty() && monto > 0.01 {
        Some((nombre, monto))
    } else {
        None
    }
}

/// Genera una plantilla a partir de los datos importados (promedia los montos)
pub fn generar_plantilla_desde_importacion(
    meses: &[PresupuestoMensual],
) -> PlantillaPresupuesto {
    use std::collections::HashMap;

    // Agrupar por nombre y categoría, promediar montos
    let mut cuentas: HashMap<String, (Categoria, Vec<f64>)> = HashMap::new();

    for mes in meses {
        for linea in &mes.lineas {
            let key = linea.nombre.to_lowercase();
            let entry = cuentas
                .entry(key)
                .or_insert_with(|| (linea.categoria.clone(), Vec::new()));
            entry.1.push(linea.monto);
        }
    }

    let mut lineas: Vec<LineaPlantilla> = cuentas
        .into_iter()
        .map(|(nombre_lower, (cat, montos))| {
            let promedio = montos.iter().sum::<f64>() / montos.len() as f64;
            // Capitalizar nombre
            let nombre = nombre_lower
                .split_whitespace()
                .map(|w| {
                    let mut c = w.chars();
                    match c.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");

            LineaPlantilla {
                nombre,
                categoria: cat,
                monto_default: (promedio * 100.0).round() / 100.0,
                fecha_limite: String::new(),
                saldo_total_deuda: None,
            }
        })
        .collect();

    // Ordenar: Ingresos primero, luego fijos, variables, deuda, ahorro
    lineas.sort_by(|a, b| {
        let orden = |c: &Categoria| -> u8 {
            match c {
                Categoria::Ingreso => 0,
                Categoria::GastoFijo => 1,
                Categoria::GastoVariable => 2,
                Categoria::PagoDeuda => 3,
                Categoria::Ahorro => 4,
            }
        };
        orden(&a.categoria)
            .cmp(&orden(&b.categoria))
            .then(a.nombre.cmp(&b.nombre))
    });

    PlantillaPresupuesto {
        nombre: "Plantilla desde Excel".to_string(),
        lineas,
    }
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_presupuesto_base_cero() {
        let mut pres = PresupuestoMensual::nuevo("2026-04");

        pres.agregar(LineaPresupuesto {
            nombre: "Sueldo".into(),
            categoria: Categoria::Ingreso,
            monto: 3500.0,
            pagado: false,
            fecha_limite: String::new(),
            notas: String::new(),
        });
        pres.agregar(LineaPresupuesto {
            nombre: "Casa".into(),
            categoria: Categoria::GastoFijo,
            monto: 1500.0,
            pagado: false,
            fecha_limite: "1".into(),
            notas: String::new(),
        });
        pres.agregar(LineaPresupuesto {
            nombre: "Carro".into(),
            categoria: Categoria::GastoFijo,
            monto: 750.0,
            pagado: false,
            fecha_limite: "15".into(),
            notas: String::new(),
        });
        pres.agregar(LineaPresupuesto {
            nombre: "BOFA".into(),
            categoria: Categoria::PagoDeuda,
            monto: 300.0,
            pagado: false,
            fecha_limite: String::new(),
            notas: String::new(),
        });
        pres.agregar(LineaPresupuesto {
            nombre: "Comida".into(),
            categoria: Categoria::GastoVariable,
            monto: 400.0,
            pagado: false,
            fecha_limite: String::new(),
            notas: String::new(),
        });
        pres.agregar(LineaPresupuesto {
            nombre: "Savings".into(),
            categoria: Categoria::Ahorro,
            monto: 50.0,
            pagado: false,
            fecha_limite: String::new(),
            notas: String::new(),
        });

        // 3500 - 1500 - 750 - 300 - 400 - 50 = 500
        assert_eq!(pres.total_ingresos(), 3500.0);
        assert_eq!(pres.total_egresos(), 3000.0);
        assert!((pres.saldo() - 500.0).abs() < 0.01);
    }

    #[test]
    fn test_presupuesto_perfecto() {
        let mut pres = PresupuestoMensual::nuevo("2026-04");
        pres.agregar(LineaPresupuesto {
            nombre: "Sueldo".into(),
            categoria: Categoria::Ingreso,
            monto: 1000.0,
            pagado: false,
            fecha_limite: String::new(),
            notas: String::new(),
        });
        pres.agregar(LineaPresupuesto {
            nombre: "Renta".into(),
            categoria: Categoria::GastoFijo,
            monto: 800.0,
            pagado: false,
            fecha_limite: String::new(),
            notas: String::new(),
        });
        pres.agregar(LineaPresupuesto {
            nombre: "Ahorro".into(),
            categoria: Categoria::Ahorro,
            monto: 200.0,
            pagado: false,
            fecha_limite: String::new(),
            notas: String::new(),
        });

        let resumen = pres.resumen();
        assert!(resumen.saldo.abs() < 0.01);
        assert!(matches!(resumen.salud, SaludPresupuesto::Perfecto));
    }

    #[test]
    fn test_categorizar() {
        assert_eq!(categorizar("Sueldo de Xavier"), Categoria::Ingreso);
        assert_eq!(categorizar("BOFA Xavico"), Categoria::PagoDeuda);
        assert_eq!(categorizar("Pago de carro"), Categoria::GastoFijo);
        assert_eq!(categorizar("Savings Jennifer"), Categoria::Ahorro);
        assert_eq!(categorizar("ATT"), Categoria::GastoFijo);
        assert_eq!(categorizar("Discover"), Categoria::PagoDeuda);
    }

    #[test]
    fn test_plantilla_genera_mes() {
        let plantilla = PlantillaPresupuesto {
            nombre: "Test".into(),
            lineas: vec![
                LineaPlantilla {
                    nombre: "Sueldo".into(),
                    categoria: Categoria::Ingreso,
                    monto_default: 3000.0,
                    fecha_limite: String::new(),
                },
                LineaPlantilla {
                    nombre: "Renta".into(),
                    categoria: Categoria::GastoFijo,
                    monto_default: 1500.0,
                    fecha_limite: "1".into(),
                },
            ],
        };

        let mes = plantilla.generar_mes("2026-05");
        assert_eq!(mes.mes, "2026-05");
        assert_eq!(mes.lineas.len(), 2);
        assert!(!mes.lineas[0].pagado);
        assert_eq!(mes.lineas[0].monto, 3000.0);
    }
}
