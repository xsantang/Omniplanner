//! Asesor de Decisiones Prácticas para Omniplanner.
//!
//! Análisis financiero, comparación de opciones, proyecciones de ahorro,
//! presupuestos, y matriz de decisión multi-criterio — todo lo necesario
//! para tomar decisiones informadas en la vida diaria.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

// ══════════════════════════════════════════════════════════════
//  Análisis de Deudas / Tarjetas de Crédito
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnalisisDeuda {
    pub nombre: String,
    pub saldo_total: f64,
    pub tasa_interes_mensual: f64,
    pub pago_minimo: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpcionPago {
    pub nombre: String,
    pub monto_mensual: f64,
    pub meses_para_liquidar: u32,
    pub total_intereses: f64,
    pub total_pagado: f64,
    pub ahorro_vs_minimo: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilaMensual {
    pub mes: u32,
    pub pago: f64,
    pub interes: f64,
    pub abono_capital: f64,
    pub saldo_restante: f64,
    pub intereses_acumulados: f64,
}

impl AnalisisDeuda {
    pub fn nuevo(nombre: &str, saldo: f64, tasa_mensual: f64, pago_minimo: f64) -> Self {
        Self {
            nombre: nombre.to_string(),
            saldo_total: saldo,
            tasa_interes_mensual: tasa_mensual,
            pago_minimo,
        }
    }

    /// Calcula proyección para un monto de pago fijo mensual.
    pub fn calcular_opcion(&self, nombre: &str, monto_mensual: f64) -> OpcionPago {
        let (meses, total_intereses, total_pagado) = self.simular_pagos(monto_mensual);
        let (_, _, total_minimo) = self.simular_pagos(self.pago_minimo);

        OpcionPago {
            nombre: nombre.to_string(),
            monto_mensual,
            meses_para_liquidar: meses,
            total_intereses,
            total_pagado,
            ahorro_vs_minimo: total_minimo - total_pagado,
        }
    }

    pub fn simular_pagos(&self, monto: f64) -> (u32, f64, f64) {
        let mut saldo = self.saldo_total;
        let mut total_intereses = 0.0;
        let mut total_pagado = 0.0;
        let mut meses = 0u32;

        while saldo > 0.01 && meses < 600 {
            let interes = saldo * self.tasa_interes_mensual;
            total_intereses += interes;
            saldo += interes;
            let pago = monto.min(saldo);
            saldo -= pago;
            total_pagado += pago;
            meses += 1;
        }
        (meses, total_intereses, total_pagado)
    }

    /// Genera múltiples opciones de pago para comparar.
    pub fn comparar_opciones(&self, montos: &[(&str, f64)]) -> Vec<OpcionPago> {
        montos
            .iter()
            .map(|(nombre, monto)| self.calcular_opcion(nombre, *monto))
            .collect()
    }

    /// Devuelve el índice de la mejor opción (menor total pagado).
    pub fn mejor_opcion(opciones: &[OpcionPago]) -> Option<usize> {
        opciones
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.total_pagado.partial_cmp(&b.total_pagado).unwrap())
            .map(|(i, _)| i)
    }

    /// Proyección mes a mes para un monto de pago dado.
    pub fn proyeccion_mensual(&self, monto_mensual: f64, max_meses: u32) -> Vec<FilaMensual> {
        let mut filas = Vec::new();
        let mut saldo = self.saldo_total;
        let mut intereses_acum = 0.0;

        for mes in 1..=max_meses {
            if saldo <= 0.01 {
                break;
            }
            let interes = saldo * self.tasa_interes_mensual;
            intereses_acum += interes;
            saldo += interes;
            let pago = monto_mensual.min(saldo);
            let abono_capital = pago - interes;
            saldo -= pago;

            filas.push(FilaMensual {
                mes,
                pago,
                interes,
                abono_capital,
                saldo_restante: if saldo < 0.01 { 0.0 } else { saldo },
                intereses_acumulados: intereses_acum,
            });
        }
        filas
    }
}

// ══════════════════════════════════════════════════════════════
//  Corte Bancario — calcular tasa e intereses desde datos reales
// ══════════════════════════════════════════════════════════════

/// Datos del corte/estado de cuenta de una tarjeta de crédito.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorteBancario {
    pub nombre_tarjeta: String,
    pub fecha_corte: String,
    /// Saldo del corte anterior
    pub saldo_anterior: f64,
    /// Pago(s) realizado(s) en el período
    pub pago_realizado: f64,
    /// Nuevas compras/cargos en el período
    pub compras_periodo: f64,
    /// Intereses cobrados (del estado de cuenta)
    pub intereses_cobrados: f64,
    /// Otros cargos (comisiones, seguros, IVA de interés, etc.)
    pub otros_cargos: f64,
    /// Saldo al corte (nuevo saldo)
    pub saldo_al_corte: f64,
    /// Pago mínimo que indica el banco
    pub pago_minimo: f64,
    /// Pago para no generar intereses
    pub pago_no_intereses: f64,
}

/// Resultado del análisis de un corte bancario.
#[derive(Clone, Debug)]
pub struct AnalisisCorte {
    /// Tasa mensual calculada sobre el saldo que generó interés
    pub tasa_mensual_calculada: f64,
    /// Tasa anualizada
    pub tasa_anual_calculada: f64,
    /// Saldo que realmente generó interés (saldo anterior - pago)
    pub saldo_que_genero_interes: f64,
    /// Cuánto se fue a capital vs interés del pago realizado
    pub pago_a_capital: f64,
    pub pago_a_interes: f64,
    /// Porcentaje del pago que se fue a intereses
    pub pct_pago_a_interes: f64,
    /// Verificación: ¿cuadra el saldo al corte con los datos?
    pub saldo_calculado: f64,
    pub diferencia_vs_real: f64,
    /// Análisis de deuda generado para proyecciones
    pub deuda: AnalisisDeuda,
    /// Estrategia calculada para eliminar la deuda
    pub estrategia: EstrategiaDeuda,
}

/// Estrategia completa para eliminar la deuda y evitar intereses predatorios.
#[derive(Clone, Debug)]
pub struct EstrategiaDeuda {
    /// ¿Ya le están cobrando intereses?
    pub tiene_intereses: bool,
    /// ¿El pago actual cubre al menos los intereses?
    pub pago_cubre_intereses: bool,
    /// Monto exacto para cortar intereses este mes (pago_no_intereses)
    pub monto_corta_intereses: f64,
    /// Si paga solo el mínimo: cuántos meses, cuánto total, cuánto en intereses
    pub plan_minimo: PlanPago,
    /// Si sigue pagando lo mismo que pagó este corte
    pub plan_actual: PlanPago,
    /// Si paga el "no generar intereses": corta el ciclo de intereses en 1 mes
    pub plan_sin_intereses: PlanPago,
    /// Si paga el total del saldo al corte: elimina la deuda
    pub plan_total: PlanPago,
    /// Diferencia que el usuario tira a la basura pagando mínimo vs sin intereses
    pub dinero_regalado_al_banco_minimo: f64,
    /// Diferencia entre seguir con la estrategia actual vs sin intereses
    pub dinero_regalado_al_banco_actual: f64,
    /// ¿Hay intereses "residuales" pendientes (se pagan en siguiente corte)?
    pub interes_residual_estimado: f64,
}

/// Un plan de pago (simulación completa).
#[derive(Clone, Debug)]
pub struct PlanPago {
    pub nombre: String,
    pub monto_mensual: f64,
    pub meses_para_liquidar: u32,
    pub total_intereses: f64,
    pub total_pagado: f64,
}

impl CorteBancario {
    pub fn nuevo(nombre: &str) -> Self {
        Self {
            nombre_tarjeta: nombre.to_string(),
            fecha_corte: String::new(),
            saldo_anterior: 0.0,
            pago_realizado: 0.0,
            compras_periodo: 0.0,
            intereses_cobrados: 0.0,
            otros_cargos: 0.0,
            saldo_al_corte: 0.0,
            pago_minimo: 0.0,
            pago_no_intereses: 0.0,
        }
    }

    /// Analiza el corte y calcula la tasa de interés real, desglose del pago, etc.
    pub fn analizar(&self) -> AnalisisCorte {
        // El saldo que genera interés: lo que quedó debiendo después del pago
        let saldo_que_genero_interes = (self.saldo_anterior - self.pago_realizado).max(0.0);

        // Calcular tasa mensual: intereses / saldo que generó interés
        let tasa_mensual = if saldo_que_genero_interes > 0.01 {
            self.intereses_cobrados / saldo_que_genero_interes
        } else if self.saldo_anterior > 0.01 {
            self.intereses_cobrados / self.saldo_anterior
        } else {
            0.0
        };

        let tasa_anual = tasa_mensual * 12.0;

        // Cuánto del pago se fue a capital vs interés
        let pago_a_interes = self.intereses_cobrados.min(self.pago_realizado);
        let pago_a_capital = (self.pago_realizado - pago_a_interes).max(0.0);
        let pct_pago_a_interes = if self.pago_realizado > 0.01 {
            pago_a_interes / self.pago_realizado * 100.0
        } else {
            0.0
        };

        // Verificar: saldo_anterior - pago + compras + intereses + otros = saldo_al_corte
        let saldo_calculado = self.saldo_anterior - self.pago_realizado
            + self.compras_periodo
            + self.intereses_cobrados
            + self.otros_cargos;
        let diferencia = (saldo_calculado - self.saldo_al_corte).abs();

        // Generar AnalisisDeuda para proyecciones futuras
        let deuda = AnalisisDeuda::nuevo(
            &self.nombre_tarjeta,
            self.saldo_al_corte,
            tasa_mensual,
            self.pago_minimo,
        );

        // ── Construir estrategia ──
        let tiene_intereses = self.intereses_cobrados > 0.01;
        let pago_cubre_intereses = self.pago_realizado >= self.intereses_cobrados;

        // Simular cada plan
        let plan_minimo = Self::simular_plan("Pago mínimo", self.pago_minimo, &deuda);
        let plan_actual = Self::simular_plan("Estrategia actual", self.pago_realizado, &deuda);

        // "Sin intereses": pagar pago_no_intereses este mes y luego solo las compras nuevas
        // Si la persona paga pago_no_intereses, en el siguiente corte no hay interés.
        // Pero si sigue habiendo compras_periodo, necesitará seguir pagándolas.
        let monto_corta_intereses = if self.pago_no_intereses > 0.01 {
            self.pago_no_intereses
        } else {
            self.saldo_al_corte
        };
        let plan_sin_intereses = Self::simular_plan(
            "Pagar para no generar intereses",
            monto_corta_intereses,
            &deuda,
        );

        let plan_total = Self::simular_plan("Liquidar todo", self.saldo_al_corte, &deuda);

        // Interés residual: si decides pagar "sin intereses" ahora, puede que el siguiente
        // corte tenga un pequeño interés residual sobre los días entre la compra y el pago.
        let interes_residual_estimado =
            if tiene_intereses && monto_corta_intereses < self.saldo_al_corte {
                // Estimación: interés de 1 mes sobre la diferencia
                (self.saldo_al_corte - monto_corta_intereses) * tasa_mensual
            } else {
                0.0
            };

        let dinero_regalado_minimo =
            plan_minimo.total_intereses - plan_sin_intereses.total_intereses;
        let dinero_regalado_actual =
            plan_actual.total_intereses - plan_sin_intereses.total_intereses;

        let estrategia = EstrategiaDeuda {
            tiene_intereses,
            pago_cubre_intereses,
            monto_corta_intereses,
            plan_minimo,
            plan_actual,
            plan_sin_intereses,
            plan_total,
            dinero_regalado_al_banco_minimo: dinero_regalado_minimo.max(0.0),
            dinero_regalado_al_banco_actual: dinero_regalado_actual.max(0.0),
            interes_residual_estimado,
        };

        AnalisisCorte {
            tasa_mensual_calculada: tasa_mensual,
            tasa_anual_calculada: tasa_anual,
            saldo_que_genero_interes,
            pago_a_capital,
            pago_a_interes,
            pct_pago_a_interes,
            saldo_calculado,
            diferencia_vs_real: diferencia,
            deuda,
            estrategia,
        }
    }

    fn simular_plan(nombre: &str, monto: f64, deuda: &AnalisisDeuda) -> PlanPago {
        let (meses, total_int, total_pag) = deuda.simular_pagos(monto);
        PlanPago {
            nombre: nombre.to_string(),
            monto_mensual: monto,
            meses_para_liquidar: meses,
            total_intereses: total_int,
            total_pagado: total_pag,
        }
    }
}

// ══════════════════════════════════════════════════════════════
//  Presupuesto Mensual
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub enum FrecuenciaPago {
    Semanal,
    Quincenal,
    #[default]
    Mensual,
    Trimestral,
    Semestral,
    Anual,
    UnaVez,
}

impl FrecuenciaPago {
    pub fn a_mensual(&self, monto: f64) -> f64 {
        match self {
            FrecuenciaPago::Semanal => monto * 4.33,
            FrecuenciaPago::Quincenal => monto * 2.0,
            FrecuenciaPago::Mensual => monto,
            FrecuenciaPago::Trimestral => monto / 3.0,
            FrecuenciaPago::Semestral => monto / 6.0,
            FrecuenciaPago::Anual => monto / 12.0,
            FrecuenciaPago::UnaVez => monto,
        }
    }

    pub fn nombre(&self) -> &str {
        match self {
            FrecuenciaPago::Semanal => "semanal",
            FrecuenciaPago::Quincenal => "quincenal",
            FrecuenciaPago::Mensual => "mensual",
            FrecuenciaPago::Trimestral => "trimestral",
            FrecuenciaPago::Semestral => "semestral",
            FrecuenciaPago::Anual => "anual",
            FrecuenciaPago::UnaVez => "una vez",
        }
    }

    pub fn todas() -> &'static [&'static str] {
        &[
            "Semanal",
            "Quincenal",
            "Mensual",
            "Trimestral",
            "Semestral",
            "Anual",
            "Una vez",
        ]
    }

    pub fn desde_indice(i: usize) -> Self {
        match i {
            0 => FrecuenciaPago::Semanal,
            1 => FrecuenciaPago::Quincenal,
            2 => FrecuenciaPago::Mensual,
            3 => FrecuenciaPago::Trimestral,
            4 => FrecuenciaPago::Semestral,
            5 => FrecuenciaPago::Anual,
            _ => FrecuenciaPago::UnaVez,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Movimiento {
    pub concepto: String,
    pub monto: f64,
    pub frecuencia: FrecuenciaPago,
    pub categoria: String,
    pub fijo: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetaAhorro {
    pub nombre: String,
    pub objetivo: f64,
    pub ahorrado: f64,
    pub fecha_meta: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Presupuesto {
    pub ingresos: Vec<Movimiento>,
    pub gastos: Vec<Movimiento>,
    pub metas: Vec<MetaAhorro>,
}

impl Presupuesto {
    pub fn ingreso_mensual(&self) -> f64 {
        self.ingresos
            .iter()
            .map(|m| m.frecuencia.a_mensual(m.monto))
            .sum()
    }

    pub fn gasto_mensual(&self) -> f64 {
        self.gastos
            .iter()
            .map(|g| g.frecuencia.a_mensual(g.monto))
            .sum()
    }

    pub fn balance_mensual(&self) -> f64 {
        self.ingreso_mensual() - self.gasto_mensual()
    }

    pub fn gastos_por_categoria(&self) -> Vec<(String, f64)> {
        let mut map: HashMap<String, f64> = HashMap::new();
        for g in &self.gastos {
            *map.entry(g.categoria.clone()).or_default() += g.frecuencia.a_mensual(g.monto);
        }
        let mut result: Vec<_> = map.into_iter().collect();
        result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        result
    }

    pub fn gastos_fijos_mensual(&self) -> f64 {
        self.gastos
            .iter()
            .filter(|g| g.fijo)
            .map(|g| g.frecuencia.a_mensual(g.monto))
            .sum()
    }

    pub fn gastos_variables_mensual(&self) -> f64 {
        self.gastos
            .iter()
            .filter(|g| !g.fijo)
            .map(|g| g.frecuencia.a_mensual(g.monto))
            .sum()
    }

    /// Proyecta el ahorro acumulado mes a mes.
    pub fn proyeccion_ahorro(&self, meses: u32) -> Vec<(u32, f64)> {
        let balance = self.balance_mensual();
        (1..=meses).map(|m| (m, balance * m as f64)).collect()
    }

    /// Calcula cuántos meses tomará alcanzar cada meta de ahorro.
    pub fn meses_para_metas(&self) -> Vec<(String, f64, u32)> {
        let balance = self.balance_mensual();
        if balance <= 0.0 {
            return self
                .metas
                .iter()
                .map(|m| (m.nombre.clone(), m.objetivo - m.ahorrado, 0))
                .collect();
        }
        self.metas
            .iter()
            .map(|m| {
                let faltante = (m.objetivo - m.ahorrado).max(0.0);
                let meses = (faltante / balance).ceil() as u32;
                (m.nombre.clone(), faltante, meses)
            })
            .collect()
    }
}

// ══════════════════════════════════════════════════════════════
//  Matriz de Decisión Multi-Criterio
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CriterioDecision {
    pub nombre: String,
    pub peso: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatrizDecision {
    pub titulo: String,
    pub criterios: Vec<CriterioDecision>,
    pub opciones: Vec<String>,
    /// valores\[opcion\]\[criterio\] = puntuación 0–10
    pub valores: Vec<Vec<f64>>,
    pub fecha: String,
}

impl MatrizDecision {
    pub fn nueva(titulo: &str, fecha: &str) -> Self {
        Self {
            titulo: titulo.to_string(),
            criterios: Vec::new(),
            opciones: Vec::new(),
            valores: Vec::new(),
            fecha: fecha.to_string(),
        }
    }

    pub fn agregar_criterio(&mut self, nombre: &str, peso: f64) {
        self.criterios.push(CriterioDecision {
            nombre: nombre.to_string(),
            peso: peso.clamp(0.0, 1.0),
        });
        for fila in &mut self.valores {
            fila.push(0.0);
        }
    }

    pub fn agregar_opcion(&mut self, nombre: &str) {
        self.opciones.push(nombre.to_string());
        self.valores.push(vec![0.0; self.criterios.len()]);
    }

    pub fn set_valor(&mut self, opcion: usize, criterio: usize, valor: f64) {
        if opcion < self.valores.len() && criterio < self.criterios.len() {
            self.valores[opcion][criterio] = valor.clamp(0.0, 10.0);
        }
    }

    /// Calcula puntuación ponderada para cada opción.
    pub fn puntuaciones(&self) -> Vec<(String, f64)> {
        let peso_total: f64 = self.criterios.iter().map(|c| c.peso).sum();
        if peso_total < 1e-10 {
            return self.opciones.iter().map(|o| (o.clone(), 0.0)).collect();
        }
        self.opciones
            .iter()
            .enumerate()
            .map(|(i, opcion)| {
                let score: f64 = self
                    .criterios
                    .iter()
                    .enumerate()
                    .map(|(j, c)| c.peso * self.valores[i][j])
                    .sum::<f64>()
                    / peso_total;
                (opcion.clone(), score)
            })
            .collect()
    }

    pub fn mejor_opcion(&self) -> Option<(String, f64)> {
        self.puntuaciones()
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
    }
}

// ══════════════════════════════════════════════════════════════
//  Escenarios de Decisión (historial)
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CategoriaEscenario {
    Financiera,
    Tiempo,
    Proyecto,
    Compra,
    Salud,
    Otra(String),
}

impl fmt::Display for CategoriaEscenario {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CategoriaEscenario::Financiera => write!(f, "Financiera"),
            CategoriaEscenario::Tiempo => write!(f, "Tiempo"),
            CategoriaEscenario::Proyecto => write!(f, "Proyecto"),
            CategoriaEscenario::Compra => write!(f, "Compra"),
            CategoriaEscenario::Salud => write!(f, "Salud"),
            CategoriaEscenario::Otra(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Escenario {
    pub id: String,
    pub titulo: String,
    pub descripcion: String,
    pub categoria: CategoriaEscenario,
    pub fecha: String,
    pub decision_tomada: Option<String>,
    pub resultado_real: Option<String>,
}

// ══════════════════════════════════════════════════════════════
//  Diccionario de Acciones — aprende de decisiones previas
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ImpactoAccion {
    MuyPositivo,
    Positivo,
    Neutro,
    Negativo,
    MuyNegativo,
}

impl ImpactoAccion {
    pub fn valor(&self) -> f64 {
        match self {
            ImpactoAccion::MuyPositivo => 1.0,
            ImpactoAccion::Positivo => 0.5,
            ImpactoAccion::Neutro => 0.0,
            ImpactoAccion::Negativo => -0.5,
            ImpactoAccion::MuyNegativo => -1.0,
        }
    }

    pub fn nombre(&self) -> &str {
        match self {
            ImpactoAccion::MuyPositivo => "Muy positivo",
            ImpactoAccion::Positivo => "Positivo",
            ImpactoAccion::Neutro => "Neutro",
            ImpactoAccion::Negativo => "Negativo",
            ImpactoAccion::MuyNegativo => "Muy negativo",
        }
    }

    pub fn emoji(&self) -> &str {
        match self {
            ImpactoAccion::MuyPositivo => "🌟",
            ImpactoAccion::Positivo => "✅",
            ImpactoAccion::Neutro => "➖",
            ImpactoAccion::Negativo => "⚠️",
            ImpactoAccion::MuyNegativo => "🔴",
        }
    }

    pub fn todas() -> &'static [&'static str] {
        &[
            "🌟 Muy positivo",
            "✅ Positivo",
            "➖ Neutro",
            "⚠️ Negativo",
            "🔴 Muy negativo",
        ]
    }

    pub fn desde_indice(i: usize) -> Self {
        match i {
            0 => ImpactoAccion::MuyPositivo,
            1 => ImpactoAccion::Positivo,
            2 => ImpactoAccion::Neutro,
            3 => ImpactoAccion::Negativo,
            _ => ImpactoAccion::MuyNegativo,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccionRegistrada {
    pub accion: String,
    pub categoria: String,
    pub impacto: ImpactoAccion,
    pub fecha: String,
    pub monto: Option<f64>,
    pub notas: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DiccionarioAcciones {
    pub acciones: Vec<AccionRegistrada>,
}

impl DiccionarioAcciones {
    pub fn registrar(
        &mut self,
        accion: &str,
        categoria: &str,
        impacto: ImpactoAccion,
        fecha: &str,
        monto: Option<f64>,
        notas: &str,
    ) {
        self.acciones.push(AccionRegistrada {
            accion: accion.to_string(),
            categoria: categoria.to_string(),
            impacto,
            fecha: fecha.to_string(),
            monto,
            notas: notas.to_string(),
        });
    }

    /// Resumen por categoría: (categoría, cantidad, impacto promedio, total $).
    pub fn resumen_por_categoria(&self) -> Vec<(String, usize, f64, f64)> {
        let mut map: HashMap<String, (usize, f64, f64)> = HashMap::new();
        for a in &self.acciones {
            let entry = map.entry(a.categoria.clone()).or_default();
            entry.0 += 1;
            entry.1 += a.impacto.valor();
            entry.2 += a.monto.unwrap_or(0.0);
        }
        let mut result: Vec<_> = map
            .into_iter()
            .map(|(cat, (n, sum_imp, total_m))| (cat, n, sum_imp / n as f64, total_m))
            .collect();
        result.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
        result
    }

    /// Busca acciones similares previas para sugerir basado en historial.
    pub fn buscar_similares(&self, texto: &str) -> Vec<&AccionRegistrada> {
        let texto_lower = texto.to_lowercase();
        let palabras: Vec<&str> = texto_lower.split_whitespace().collect();
        let mut coincidencias: Vec<(&AccionRegistrada, usize)> = self
            .acciones
            .iter()
            .filter_map(|a| {
                let accion_lower = a.accion.to_lowercase();
                let cat_lower = a.categoria.to_lowercase();
                let hits = palabras
                    .iter()
                    .filter(|p| accion_lower.contains(**p) || cat_lower.contains(**p))
                    .count();
                if hits > 0 {
                    Some((a, hits))
                } else {
                    None
                }
            })
            .collect();
        coincidencias.sort_by_key(|k| std::cmp::Reverse(k.1));
        coincidencias.into_iter().map(|(a, _)| a).collect()
    }
}

// ══════════════════════════════════════════════════════════════
//  Comparador rápido de dos opciones (tipo "¿pago mínimo o todo?")
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComparacionRapida {
    pub titulo: String,
    pub opcion_a: String,
    pub opcion_b: String,
    pub costo_a: f64,
    pub costo_b: f64,
    pub beneficio_a: String,
    pub beneficio_b: String,
    pub diferencia: f64,
    pub recomendacion: String,
}

impl ComparacionRapida {
    pub fn nueva(
        titulo: &str,
        opcion_a: &str,
        costo_a: f64,
        beneficio_a: &str,
        opcion_b: &str,
        costo_b: f64,
        beneficio_b: &str,
    ) -> Self {
        let diferencia = costo_a - costo_b;
        let recomendacion = if diferencia.abs() < 0.01 {
            "Ambas opciones son equivalentes en costo.".to_string()
        } else if diferencia > 0.0 {
            format!(
                "\"{}\" es más barato por ${:.2}. Recomendado.",
                opcion_b, diferencia
            )
        } else {
            format!(
                "\"{}\" es más barato por ${:.2}. Recomendado.",
                opcion_a,
                diferencia.abs()
            )
        };
        Self {
            titulo: titulo.to_string(),
            opcion_a: opcion_a.to_string(),
            opcion_b: opcion_b.to_string(),
            costo_a,
            costo_b,
            beneficio_a: beneficio_a.to_string(),
            beneficio_b: beneficio_b.to_string(),
            diferencia,
            recomendacion,
        }
    }
}

// ══════════════════════════════════════════════════════════════
//  Almacén del Asesor (persistencia)
// ══════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════
//  Rastreador de Deudas — seguimiento multi-mes con diagnóstico
// ══════════════════════════════════════════════════════════════

/// Una deuda individual rastreada mes a mes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeudaRastreada {
    pub nombre: String,
    pub tasa_anual: f64,
    pub pago_minimo: f64,
    pub activa: bool,
    pub historial: Vec<MesPago>,
    /// Pago fijo / contra entrega: no se puede fallar (renta, mortgage, carro, etc.)
    #[serde(default)]
    pub obligatoria: bool,
    /// Enganche / pago inicial único (ej: 4000 de 10000 totales). Solo informativo.
    #[serde(default)]
    pub enganche: f64,
    /// Componente mensual de escrow (seguros/impuestos). No reduce la deuda principal.
    #[serde(default)]
    pub escrow_mensual: f64,
    /// Componente mensual de principal + intereses (P&I) aplicado a la deuda.
    #[serde(default)]
    pub principal_interes_mensual: f64,
    /// La deuda ya fue originada/desembolsada. Si es false, aún no inicia pagos.
    #[serde(default = "default_true")]
    pub originada: bool,
}

fn default_true() -> bool {
    true
}

/// Registro de un mes para una deuda.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MesPago {
    pub mes: String,
    pub saldo_inicio: f64,
    pub pago: f64,
    #[serde(default)]
    pub pago_escrow: f64,
    pub nuevos_cargos: f64,
    pub intereses: f64,
    pub saldo_final: f64,
}

/// Diagnóstico de un mes: qué se pagó vs qué se debió pagar.
#[derive(Clone, Debug)]
pub struct DiagnosticoMes {
    pub deuda: String,
    pub mes: String,
    pub pago_real: f64,
    pub pago_recomendado: f64,
    pub diferencia: f64,
    pub error: ErrorPago,
    pub nota: String,
}

#[derive(Clone, Debug)]
pub enum ErrorPago {
    PagoInsuficiente,
    SiguioUsandoTarjeta,
    NoPagoNada,
    PagoCorrecto,
    PagoExcelente,
}

impl ErrorPago {
    pub fn emoji(&self) -> &str {
        match self {
            ErrorPago::PagoInsuficiente => "🟡",
            ErrorPago::SiguioUsandoTarjeta => "🔴",
            ErrorPago::NoPagoNada => "⛔",
            ErrorPago::PagoCorrecto => "✅",
            ErrorPago::PagoExcelente => "🌟",
        }
    }
    pub fn nombre(&self) -> &str {
        match self {
            ErrorPago::PagoInsuficiente => "Pago insuficiente",
            ErrorPago::SiguioUsandoTarjeta => "Siguió usando la tarjeta",
            ErrorPago::NoPagoNada => "No pagó nada",
            ErrorPago::PagoCorrecto => "Pago correcto",
            ErrorPago::PagoExcelente => "Pago excelente",
        }
    }
}

// ══════════════════════════════════════════════════════════════
//  Fase 3 — Decisión tipada de aceptación de un pago
// ══════════════════════════════════════════════════════════════

/// Resultado de evaluar un pago antes de registrarlo.
///
/// Permite que la UI distinga entre pagos aceptables, pagos que ameritan
/// un aviso, pagos que exigen doble confirmación por lo sospechosos y
/// pagos que deben bloquearse de plano (datos inválidos).
#[derive(Clone, Debug, PartialEq)]
pub enum DecisionPago {
    /// Pago válido, se puede registrar directamente.
    Aceptar,
    /// Pago válido pero con una advertencia que conviene mostrar.
    AceptarConAviso(String),
    /// Pago probablemente correcto pero sospechoso: exigir confirmación extra.
    PedirDobleConfirmacion(String),
    /// Pago inválido: no debe registrarse tal como está.
    Bloquear(String),
}

impl DecisionPago {
    /// ¿El pago puede registrarse sin más trámite?
    pub fn es_aceptado(&self) -> bool {
        matches!(self, DecisionPago::Aceptar | DecisionPago::AceptarConAviso(_))
    }

    /// ¿Se requiere que el usuario confirme explícitamente?
    pub fn requiere_confirmacion(&self) -> bool {
        matches!(self, DecisionPago::PedirDobleConfirmacion(_))
    }

    /// ¿El pago debe rechazarse?
    pub fn esta_bloqueado(&self) -> bool {
        matches!(self, DecisionPago::Bloquear(_))
    }

    /// Mensaje humano asociado (vacío si es Aceptar).
    pub fn mensaje(&self) -> &str {
        match self {
            DecisionPago::Aceptar => "",
            DecisionPago::AceptarConAviso(m)
            | DecisionPago::PedirDobleConfirmacion(m)
            | DecisionPago::Bloquear(m) => m.as_str(),
        }
    }
}

/// Estado de una deuda para mostrarla en listas / dashboards.
///
/// Versión tipada de lo que antes se calculaba con `if`s dispersos en la UI.
#[derive(Clone, Debug, PartialEq)]
pub enum EstadoDeudaUi {
    /// Saldo ≈ 0 y marcada como inactiva.
    Liquidada,
    /// Al día, sin atrasos significativos.
    AlDia,
    /// El pago mínimo no alcanza a cubrir los intereses del mes: la deuda crece sola.
    EnTrampaIntereses,
    /// Existe monto vencido acumulado (P&I o escrow) por encima del pago del mes.
    Vencida { monto_vencido: f64 },
}

impl EstadoDeudaUi {
    /// Emoji / badge corto para prefijar en listas.
    pub fn badge(&self) -> &str {
        match self {
            EstadoDeudaUi::Liquidada => "✅",
            EstadoDeudaUi::AlDia => "🟢",
            EstadoDeudaUi::EnTrampaIntereses => "🔴",
            EstadoDeudaUi::Vencida { .. } => "🟠",
        }
    }

    /// Etiqueta humana.
    pub fn etiqueta(&self) -> &str {
        match self {
            EstadoDeudaUi::Liquidada => "Liquidada",
            EstadoDeudaUi::AlDia => "Al día",
            EstadoDeudaUi::EnTrampaIntereses => "Trampa de intereses",
            EstadoDeudaUi::Vencida { .. } => "Vencida",
        }
    }
}

/// Resultado global del diagnóstico.
#[derive(Clone, Debug)]
pub struct DiagnosticoGlobal {
    pub total_pagado: f64,
    pub total_intereses_estimados: f64,
    pub total_nuevos_cargos: f64,
    pub deuda_inicial_total: f64,
    pub deuda_final_total: f64,
    pub cambio_neto: f64,
    pub meses_analizados: usize,
    pub errores: Vec<DiagnosticoMes>,
    pub resumen_por_deuda: Vec<ResumenDeuda>,
    pub recomendaciones: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ResumenDeuda {
    pub nombre: String,
    pub saldo_inicial: f64,
    pub saldo_final: f64,
    pub total_pagado: f64,
    pub total_cargos: f64,
    pub total_intereses: f64,
    pub meses: usize,
    pub tendencia: String,
}

/// Resultado de simular cuánto toma liquidar una deuda pagando un
/// monto mensual constante (sólo P&I).
#[derive(Clone, Debug, PartialEq)]
pub struct SimulacionLiquidacion {
    pub meses: u32,
    pub intereses_totales: f64,
    pub pagado_total: f64,
}

/// Comparativa de lo que se ahorra aplicando `extra` dólares extra
/// por mes sobre el pago P&I mínimo hasta liquidar la deuda.
#[derive(Clone, Debug, PartialEq)]
pub struct AhorroPagoExtra {
    pub pago_base: f64,
    pub extra: f64,
    pub meses_base: u32,
    pub meses_con_extra: u32,
    pub meses_ahorrados: u32,
    pub intereses_base: f64,
    pub intereses_con_extra: f64,
    pub intereses_ahorrados: f64,
}

impl AhorroPagoExtra {
    /// Porcentaje de intereses ahorrados respecto al escenario base.
    pub fn porcentaje_intereses_ahorrados(&self) -> f64 {
        if self.intereses_base <= 0.01 {
            0.0
        } else {
            (self.intereses_ahorrados / self.intereses_base) * 100.0
        }
    }

    /// Ahorro por dólar extra aportado (intereses ahorrados / extra total pagado).
    /// Aproximación: extra × meses_con_extra.
    pub fn eficiencia_por_dolar_extra(&self) -> f64 {
        let invertido = self.extra * self.meses_con_extra.max(1) as f64;
        if invertido <= 0.01 {
            0.0
        } else {
            self.intereses_ahorrados / invertido
        }
    }
}

/// Recomendación sobre a qué deuda conviene aplicar un pago extra.
#[derive(Clone, Debug)]
pub struct RecomendacionPagoExtra {
    pub nombre_deuda: String,
    pub ahorro: AhorroPagoExtra,
    /// Todas las alternativas ordenadas de mayor a menor ahorro de intereses.
    pub ranking: Vec<(String, AhorroPagoExtra)>,
}

impl DeudaRastreada {
    pub fn nueva(nombre: &str, tasa_anual: f64, pago_minimo: f64) -> Self {
        Self {
            nombre: nombre.to_string(),
            tasa_anual,
            pago_minimo,
            activa: true,
            historial: Vec::new(),
            obligatoria: false,
            enganche: 0.0,
            escrow_mensual: 0.0,
            principal_interes_mensual: pago_minimo,
            originada: true,
        }
    }

    /// Pago mensual que realmente ataca la deuda (principal + intereses).
    pub fn pago_pi_mensual(&self) -> f64 {
        if self.principal_interes_mensual > 0.01 {
            self.principal_interes_mensual
        } else {
            self.pago_minimo.max(0.0)
        }
    }

    /// Pago total mensual (P&I + escrow), útil para flujo de caja.
    pub fn pago_total_mensual(&self) -> f64 {
        self.pago_pi_mensual() + self.escrow_mensual.max(0.0)
    }

    pub fn tiene_escrow_configurado(&self) -> bool {
        self.escrow_mensual > 0.01
    }

    fn atraso_componentes_antes_de(&self, indice: usize) -> (f64, f64) {
        let mut atraso_pi = 0.0;
        let mut atraso_escrow = 0.0;

        for mes in self.historial.iter().take(indice) {
            // Para deudas con interés real (hipotecas, tarjetas, préstamos)
            // se acumula atraso si el pago fue menor al mínimo, independiente
            // del saldo_inicio — porque el escrow/P&I es exigible aunque el
            // saldo contable sea bajo o cero (ej: escrow puro).
            if mes.saldo_inicio < 0.01 && !self.es_pago_corriente() && self.escrow_mensual < 0.01 {
                continue;
            }
            atraso_pi = (atraso_pi + self.pago_pi_mensual() - mes.pago).max(0.0);
            atraso_escrow = (atraso_escrow + self.escrow_mensual - mes.pago_escrow).max(0.0);
        }

        (atraso_pi, atraso_escrow)
    }

    pub fn pago_exigible_componentes_en_mes(&self, indice: usize) -> (f64, f64) {
        let (atraso_pi, atraso_escrow) = self.atraso_componentes_antes_de(indice);
        let debe_mes = self
            .historial
            .get(indice)
            .map(|m| m.saldo_inicio > 0.01 || self.es_pago_corriente())
            .unwrap_or(self.saldo_actual() > 0.01 || self.es_pago_corriente() || self.activa);

        if !debe_mes {
            return (atraso_pi, atraso_escrow);
        }

        (
            atraso_pi + self.pago_pi_mensual(),
            atraso_escrow + self.escrow_mensual,
        )
    }

    pub fn pago_exigible_total_en_mes(&self, indice: usize) -> f64 {
        let (pago_pi, pago_escrow) = self.pago_exigible_componentes_en_mes(indice);
        pago_pi + pago_escrow
    }

    pub fn pago_exigible_componentes_proximo_mes(&self) -> (f64, f64) {
        self.pago_exigible_componentes_en_mes(self.historial.len())
    }

    pub fn pago_exigible_total_proximo_mes(&self) -> f64 {
        let (pago_pi, pago_escrow) = self.pago_exigible_componentes_proximo_mes();
        pago_pi + pago_escrow
    }

    pub fn deuda_vencida_componentes(&self) -> (f64, f64) {
        let (pago_pi, pago_escrow) = self.pago_exigible_componentes_proximo_mes();
        (
            (pago_pi - self.pago_pi_mensual()).max(0.0),
            (pago_escrow - self.escrow_mensual).max(0.0),
        )
    }

    pub fn deuda_vencida_total(&self) -> f64 {
        let (vencido_pi, vencido_escrow) = self.deuda_vencida_componentes();
        vencido_pi + vencido_escrow
    }

    pub fn esta_vencida(&self) -> bool {
        self.deuda_vencida_total() > 0.01
    }

    /// ¿Es un pago corriente (renta, seguro, suscripción)?
    /// Sin intereses, obligatorio, se paga completo cada mes, nunca se "liquida".
    /// NO aplica si el saldo es significativamente mayor al pago mínimo
    /// (eso indica una deuda finita que se está pagando, no un gasto recurrente).
    pub fn es_pago_corriente(&self) -> bool {
        if !self.obligatoria || self.tasa_anual >= 0.01 {
            return false;
        }
        // Si el saldo es mayor a 1.5× el pago mínimo, es una deuda real
        // (ej: Navy Federal $1396 con pago $500 → deuda, no corriente)
        // Un corriente tiene saldo ≈ pago_minimo o 0 (renta, celular, etc.)
        let saldo = self.saldo_actual();
        let pago_ref = self.pago_pi_mensual();
        if pago_ref > 0.01 && saldo > pago_ref * 1.5 {
            return false;
        }
        true
    }

    pub fn saldo_actual(&self) -> f64 {
        self.historial.last().map(|m| m.saldo_final).unwrap_or(0.0)
    }

    pub fn registrar_mes(&mut self, mes: &str, saldo_inicio: f64, pago: f64, nuevos_cargos: f64) {
        self.registrar_mes_con_escrow(mes, saldo_inicio, pago, 0.0, nuevos_cargos);
    }

    pub fn registrar_mes_con_escrow(
        &mut self,
        mes: &str,
        saldo_inicio: f64,
        pago_pi: f64,
        pago_escrow: f64,
        nuevos_cargos: f64,
    ) {
        let tasa_mensual = self.tasa_anual / 100.0 / 12.0;
        let saldo_despues_pago = (saldo_inicio - pago_pi).max(0.0);
        let intereses = saldo_despues_pago * tasa_mensual;
        let saldo_final = saldo_despues_pago + intereses + nuevos_cargos;

        self.historial.push(MesPago {
            mes: mes.to_string(),
            saldo_inicio,
            pago: pago_pi,
            pago_escrow: pago_escrow.max(0.0),
            nuevos_cargos,
            intereses,
            saldo_final: if saldo_final < 0.01 { 0.0 } else { saldo_final },
        });

        self.activa = saldo_final >= 0.01;
    }

    /// Simula qué hubiera pasado si se hubiera pagado un monto diferente.
    pub fn simular_alternativa(&self, pagos_mensuales: f64) -> Vec<MesPago> {
        let tasa_mensual = self.tasa_anual / 100.0 / 12.0;
        let mut resultado = Vec::new();
        let saldo_inicio = self
            .historial
            .first()
            .map(|m| m.saldo_inicio)
            .unwrap_or(0.0);
        let mut saldo = saldo_inicio;

        for orig in &self.historial {
            if saldo < 0.01 {
                break;
            }
            let pago = pagos_mensuales.min(saldo);
            let saldo_despues = (saldo - pago).max(0.0);
            let intereses = saldo_despues * tasa_mensual;
            // Mantener los mismos nuevos_cargos que en la realidad
            let saldo_final = saldo_despues + intereses + orig.nuevos_cargos;

            resultado.push(MesPago {
                mes: orig.mes.clone(),
                saldo_inicio: saldo,
                pago,
                pago_escrow: orig.pago_escrow,
                nuevos_cargos: orig.nuevos_cargos,
                intereses,
                saldo_final: if saldo_final < 0.01 { 0.0 } else { saldo_final },
            });
            saldo = saldo_final;
        }
        resultado
    }

    // ──────────────────────────────────────────────────────────
    //  Fase 3 — Helpers tipados (DecisionPago / EstadoDeudaUi)
    // ──────────────────────────────────────────────────────────

    /// Interés mensual estimado sobre un saldo dado.
    fn intereses_del_saldo(&self, saldo: f64) -> f64 {
        let tasa_mensual = self.tasa_anual / 100.0 / 12.0;
        (saldo.max(0.0)) * tasa_mensual
    }

    /// Simula desde el saldo actual cuántos meses e intereses tomaría liquidar
    /// la deuda pagando exactamente `pago_mensual` cada mes (P&I, sin escrow).
    /// Devuelve `None` si el pago no alcanza a cubrir intereses (deuda infinita).
    pub fn simular_liquidacion(&self, pago_mensual: f64) -> Option<SimulacionLiquidacion> {
        if pago_mensual <= 0.01 {
            return None;
        }
        let tasa_mensual = self.tasa_anual / 100.0 / 12.0;
        let mut saldo = self.saldo_actual();
        if saldo <= 0.01 {
            return Some(SimulacionLiquidacion {
                meses: 0,
                intereses_totales: 0.0,
                pagado_total: 0.0,
            });
        }
        let mut intereses_totales = 0.0;
        let mut pagado_total = 0.0;
        let mut meses = 0u32;

        while saldo > 0.01 && meses < 600 {
            let interes = saldo * tasa_mensual;
            if pago_mensual <= interes + 0.001 {
                // el pago no reduce principal ⇒ nunca liquida
                return None;
            }
            intereses_totales += interes;
            saldo += interes;
            let pago = pago_mensual.min(saldo);
            saldo -= pago;
            pagado_total += pago;
            meses += 1;
        }

        if saldo > 0.01 {
            return None;
        }

        Some(SimulacionLiquidacion {
            meses,
            intereses_totales,
            pagado_total,
        })
    }

    /// Calcula qué se ahorra (meses + intereses) por pagar `extra` dólares
    /// extra sobre el pago P&I mínimo, cada mes, hasta liquidar.
    ///
    /// Devuelve `None` si no hay saldo activo o si ni siquiera el pago
    /// con extra alcanza a cubrir intereses.
    pub fn ahorro_por_pago_extra(&self, extra: f64) -> Option<AhorroPagoExtra> {
        if extra <= 0.01 || self.saldo_actual() <= 0.01 {
            return None;
        }
        let pago_base = self.pago_pi_mensual().max(0.01);
        let base = self.simular_liquidacion(pago_base)?;
        let con_extra = self.simular_liquidacion(pago_base + extra)?;

        Some(AhorroPagoExtra {
            pago_base,
            extra,
            meses_base: base.meses,
            meses_con_extra: con_extra.meses,
            meses_ahorrados: base.meses.saturating_sub(con_extra.meses),
            intereses_base: base.intereses_totales,
            intereses_con_extra: con_extra.intereses_totales,
            intereses_ahorrados: (base.intereses_totales - con_extra.intereses_totales).max(0.0),
        })
    }

    /// Evalúa si un pago propuesto puede registrarse tal cual.
    ///
    /// Reglas:
    /// - `pago_pi` o `pago_escrow` negativos → `Bloquear`.
    /// - `saldo_inicio` negativo → `Bloquear`.
    /// - Pago total == 0 y hay saldo o pago corriente → `PedirDobleConfirmacion`.
    /// - Pago total < 50% del pago exigible → `PedirDobleConfirmacion`.
    /// - Pago total anormalmente alto (>10× exigible o > saldo + margen) → `PedirDobleConfirmacion`.
    /// - Pago P&I no cubre intereses del mes (trampa) → `AceptarConAviso`.
    /// - En cualquier otro caso → `Aceptar`.
    pub fn evaluar_pago_mes(
        &self,
        pago_pi: f64,
        pago_escrow: f64,
        saldo_inicio: f64,
    ) -> DecisionPago {
        if pago_pi.is_nan() || pago_escrow.is_nan() || saldo_inicio.is_nan() {
            return DecisionPago::Bloquear("Valores no numéricos (NaN).".to_string());
        }
        if pago_pi < 0.0 {
            return DecisionPago::Bloquear(format!(
                "El pago P&I no puede ser negativo (${:.2}).",
                pago_pi
            ));
        }
        if pago_escrow < 0.0 {
            return DecisionPago::Bloquear(format!(
                "El pago de escrow no puede ser negativo (${:.2}).",
                pago_escrow
            ));
        }
        if saldo_inicio < 0.0 {
            return DecisionPago::Bloquear(format!(
                "El saldo de inicio no puede ser negativo (${:.2}).",
                saldo_inicio
            ));
        }

        let pago_total = pago_pi + pago_escrow;
        let exigible = self.pago_exigible_total_proximo_mes();
        let debe_pagar = saldo_inicio > 0.01 || self.es_pago_corriente() || exigible > 0.01;

        if pago_total < 0.01 && debe_pagar {
            return DecisionPago::PedirDobleConfirmacion(format!(
                "Pago de $0.00 registrado habiendo ${:.2} exigible. ¿Seguro?",
                exigible
            ));
        }

        if exigible > 0.01 && pago_total + 0.01 < exigible * 0.5 {
            return DecisionPago::PedirDobleConfirmacion(format!(
                "Pago ${:.2} es menos del 50% del exigible (${:.2}).",
                pago_total, exigible
            ));
        }

        // Pago anormalmente alto: > 10× lo exigible o excede por mucho el techo razonable
        // (saldo_inicio + exigible + margen). Suele ser un dedo de más.
        let techo_razonable = saldo_inicio + exigible + 1_000.0;
        let excede_por_multiplo = exigible > 0.01 && pago_total > exigible * 10.0;
        let excede_techo = pago_total > techo_razonable && pago_total > 1_000.0;
        if excede_por_multiplo || excede_techo {
            return DecisionPago::PedirDobleConfirmacion(format!(
                "Pago ${:.2} parece excesivo (exigible ${:.2}, saldo ${:.2}). ¿Seguro?",
                pago_total, exigible, saldo_inicio
            ));
        }

        if self.tasa_anual > 0.0 && saldo_inicio > 0.01 {
            let interes = self.intereses_del_saldo(saldo_inicio);
            if interes > 1.0 && pago_pi + 0.01 < interes {
                return DecisionPago::AceptarConAviso(format!(
                    "El pago P&I ${:.2} no cubre intereses del mes ${:.2}: la deuda crecerá.",
                    pago_pi, interes
                ));
            }
        }

        DecisionPago::Aceptar
    }

    /// Clasifica el estado actual de la deuda para presentarlo en listas.
    pub fn estado_ui(&self) -> EstadoDeudaUi {
        let saldo = self.saldo_actual();
        if !self.activa && saldo < 0.01 {
            return EstadoDeudaUi::Liquidada;
        }

        let vencido = self.deuda_vencida_total();
        if vencido > 0.01 {
            return EstadoDeudaUi::Vencida {
                monto_vencido: vencido,
            };
        }

        if self.tasa_anual > 0.0 && saldo > 0.01 {
            let interes = self.intereses_del_saldo(saldo);
            if interes > 1.0 && self.pago_pi_mensual() + 0.01 < interes {
                return EstadoDeudaUi::EnTrampaIntereses;
            }
        }

        EstadoDeudaUi::AlDia
    }
}

/// Una fuente de ingreso individual.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IngresoRastreado {
    pub concepto: String,
    pub monto: f64,
    pub frecuencia: FrecuenciaPago,
    #[serde(default = "ingreso_confirmado_default")]
    pub confirmado: bool,
    #[serde(default)]
    pub taxeable: bool,
    #[serde(default)]
    pub impuesto_federal: bool,
    #[serde(default)]
    pub impuesto_estatal: bool,
    #[serde(default)]
    pub allotment_federal_pct: f64,
    #[serde(default)]
    pub allotment_estatal_pct: f64,
    #[serde(default)]
    pub retener_social_security: bool,
    #[serde(default)]
    pub retener_medicare: bool,
    #[serde(default)]
    pub permitir_allotment_cero: bool,
    #[serde(default)]
    pub es_beneficio_social_security: bool,
    #[serde(default)]
    pub beneficio_social_security_temprano: bool,
    /// Estado/territorio donde se realiza este trabajo (ej: "TX", "FL", "NY")
    #[serde(default)]
    pub estado_trabajo: String,
}

fn ingreso_confirmado_default() -> bool {
    true
}

impl IngresoRastreado {
    pub const SOCIAL_SECURITY_PCT: f64 = 6.2;
    pub const MEDICARE_PCT: f64 = 1.45;

    pub fn monto_mensual(&self) -> f64 {
        self.frecuencia.a_mensual(self.monto)
    }

    pub fn allotment_federal_pct_efectivo(&self) -> f64 {
        if self.paga_impuesto_federal() {
            self.allotment_federal_pct.max(0.0)
        } else {
            0.0
        }
    }

    pub fn allotment_estatal_pct_efectivo(&self) -> f64 {
        if self.paga_impuesto_estatal() {
            self.allotment_estatal_pct.max(0.0)
        } else {
            0.0
        }
    }

    pub fn retencion_federal_mensual(&self) -> f64 {
        self.monto_mensual() * (self.allotment_federal_pct_efectivo() / 100.0)
    }

    pub fn retencion_estatal_mensual(&self) -> f64 {
        self.monto_mensual() * (self.allotment_estatal_pct_efectivo() / 100.0)
    }

    pub fn retencion_total_mensual(&self) -> f64 {
        self.retencion_federal_mensual()
            + self.retencion_estatal_mensual()
            + self.retencion_social_security_mensual()
            + self.retencion_medicare_mensual()
    }

    pub fn monto_mensual_neto(&self) -> f64 {
        (self.monto_mensual() - self.retencion_total_mensual()).max(0.0)
    }

    pub fn retencion_social_security_mensual(&self) -> f64 {
        if self.retener_social_security {
            self.monto_mensual() * (Self::SOCIAL_SECURITY_PCT / 100.0)
        } else {
            0.0
        }
    }

    pub fn retencion_medicare_mensual(&self) -> f64 {
        if self.retener_medicare {
            self.monto_mensual() * (Self::MEDICARE_PCT / 100.0)
        } else {
            0.0
        }
    }

    pub fn paga_impuesto_federal(&self) -> bool {
        self.impuesto_federal || self.taxeable && !self.impuesto_estatal
    }

    pub fn paga_impuesto_estatal(&self) -> bool {
        self.impuesto_estatal
    }

    pub fn es_no_taxeable(&self) -> bool {
        !self.paga_impuesto_federal() && !self.paga_impuesto_estatal()
    }

    pub fn etiqueta_confirmacion(&self) -> &'static str {
        if self.confirmado {
            "confirmado"
        } else {
            "no confirmado"
        }
    }

    pub fn etiqueta_taxes(&self) -> &'static str {
        match (self.paga_impuesto_federal(), self.paga_impuesto_estatal()) {
            (true, true) => "federal + estatal",
            (true, false) => "federal",
            (false, true) => "estatal",
            (false, false) => "no taxeable",
        }
    }

    pub fn es_taxeable(&self) -> bool {
        !self.es_no_taxeable()
    }

    pub fn normalizar_impuestos_legacy(&mut self) {
        if self.taxeable && !self.impuesto_federal && !self.impuesto_estatal {
            self.impuesto_federal = true;
        }
        if !self.paga_impuesto_federal() {
            self.allotment_federal_pct = 0.0;
        }
        if !self.paga_impuesto_estatal() {
            self.allotment_estatal_pct = 0.0;
        }
    }
}

/// Lista de estados de EE.UU. sin impuesto estatal sobre ingresos.
pub const ESTADOS_SIN_IMPUESTO: &[&str] = &["AK", "FL", "NV", "SD", "TN", "TX", "WA", "WY"];

/// Rastreador global de todas las deudas.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RastreadorDeudas {
    pub deudas: Vec<DeudaRastreada>,
    /// Lista de ingresos del usuario (múltiples fuentes).
    #[serde(default)]
    pub ingresos: Vec<IngresoRastreado>,
    /// Saldo actual en banco / efectivo disponible (actualizable por el usuario).
    #[serde(default)]
    pub saldo_disponible: f64,
    /// Estado/territorio de residencia del usuario (ej: "TX", "FL", "NY")
    #[serde(default)]
    pub estado_residencia: String,
    // ── Campos legacy para compatibilidad con datos guardados ──
    #[serde(default, alias = "ingreso_quincenal")]
    ingreso: f64,
    #[serde(default = "frecuencia_ingreso_default")]
    frecuencia_ingreso: FrecuenciaPago,
}

fn frecuencia_ingreso_default() -> FrecuenciaPago {
    FrecuenciaPago::Quincenal
}

impl RastreadorDeudas {
    /// Retorna true si el estado dado no tiene impuesto sobre ingresos.
    pub fn estado_sin_impuesto(estado: &str) -> bool {
        let upper = estado.trim().to_uppercase();
        ESTADOS_SIN_IMPUESTO.contains(&upper.as_str())
    }

    /// Retorna ingresos donde estado_trabajo difiere del estado_residencia.
    pub fn ingresos_estado_dual(&self) -> Vec<&IngresoRastreado> {
        if self.estado_residencia.is_empty() {
            return vec![];
        }
        self.ingresos
            .iter()
            .filter(|ing| {
                !ing.estado_trabajo.is_empty()
                    && ing.estado_trabajo.trim().to_uppercase()
                        != self.estado_residencia.trim().to_uppercase()
            })
            .collect()
    }

    pub fn migrar_impuestos_legacy(&mut self) {
        for ingreso in &mut self.ingresos {
            ingreso.normalizar_impuestos_legacy();
        }
    }

    /// Migra el ingreso legacy (campo único) a la nueva lista, si aplica.
    pub fn migrar_ingreso_legacy(&mut self) {
        if self.ingreso > 0.0 && self.ingresos.is_empty() {
            self.ingresos.push(IngresoRastreado {
                concepto: "Ingreso principal".to_string(),
                monto: self.ingreso,
                frecuencia: self.frecuencia_ingreso.clone(),
                confirmado: true,
                taxeable: false,
                impuesto_federal: false,
                impuesto_estatal: false,
                allotment_federal_pct: 0.0,
                allotment_estatal_pct: 0.0,
                retener_social_security: false,
                retener_medicare: false,
                permitir_allotment_cero: false,
                es_beneficio_social_security: false,
                beneficio_social_security_temprano: false,
                estado_trabajo: String::new(),
            });
            self.ingreso = 0.0;
        }
        self.migrar_impuestos_legacy();
    }

    /// Total de ingresos normalizado a monto mensual.
    pub fn ingreso_mensual_total(&self) -> f64 {
        self.ingreso_mensual_confirmado()
    }

    pub fn ingreso_mensual_confirmado(&self) -> f64 {
        self.ingresos
            .iter()
            .filter(|i| i.confirmado)
            .map(|i| i.monto_mensual())
            .sum()
    }

    pub fn ingreso_mensual_confirmado_neto(&self) -> f64 {
        self.ingresos
            .iter()
            .filter(|i| i.confirmado)
            .map(|i| i.monto_mensual_neto())
            .sum()
    }

    pub fn ingreso_mensual_no_confirmado(&self) -> f64 {
        self.ingresos
            .iter()
            .filter(|i| !i.confirmado)
            .map(|i| i.monto_mensual())
            .sum()
    }

    pub fn ingreso_mensual_taxeable(&self) -> f64 {
        self.ingresos
            .iter()
            .filter(|i| i.confirmado && i.es_taxeable())
            .map(|i| i.monto_mensual())
            .sum()
    }

    pub fn ingreso_mensual_no_taxeable(&self) -> f64 {
        self.ingresos
            .iter()
            .filter(|i| i.confirmado && i.es_no_taxeable())
            .map(|i| i.monto_mensual())
            .sum()
    }

    pub fn ingreso_mensual_impuesto_federal(&self) -> f64 {
        self.ingresos
            .iter()
            .filter(|i| i.confirmado && i.paga_impuesto_federal())
            .map(|i| i.monto_mensual())
            .sum()
    }

    pub fn ingreso_mensual_impuesto_estatal(&self) -> f64 {
        self.ingresos
            .iter()
            .filter(|i| i.confirmado && i.paga_impuesto_estatal())
            .map(|i| i.monto_mensual())
            .sum()
    }

    pub fn retencion_federal_mensual_total(&self) -> f64 {
        self.ingresos
            .iter()
            .filter(|i| i.confirmado)
            .map(|i| i.retencion_federal_mensual())
            .sum()
    }

    pub fn retencion_estatal_mensual_total(&self) -> f64 {
        self.ingresos
            .iter()
            .filter(|i| i.confirmado)
            .map(|i| i.retencion_estatal_mensual())
            .sum()
    }

    pub fn retencion_total_mensual(&self) -> f64 {
        self.retencion_federal_mensual_total() + self.retencion_estatal_mensual_total()
    }

    pub fn retencion_social_security_mensual_total(&self) -> f64 {
        self.ingresos
            .iter()
            .filter(|i| i.confirmado)
            .map(|i| i.retencion_social_security_mensual())
            .sum()
    }

    pub fn retencion_medicare_mensual_total(&self) -> f64 {
        self.ingresos
            .iter()
            .filter(|i| i.confirmado)
            .map(|i| i.retencion_medicare_mensual())
            .sum()
    }

    pub fn retencion_total_mensual_completa(&self) -> f64 {
        self.retencion_federal_mensual_total()
            + self.retencion_estatal_mensual_total()
            + self.retencion_social_security_mensual_total()
            + self.retencion_medicare_mensual_total()
    }

    pub fn agregar_deuda(&mut self, deuda: DeudaRastreada) {
        self.deudas.push(deuda);
    }

    pub fn deuda_total_actual(&self) -> f64 {
        self.deudas.iter().map(|d| d.saldo_actual()).sum()
    }

    pub fn deudas_activas(&self) -> Vec<&DeudaRastreada> {
        self.deudas.iter().filter(|d| d.activa).collect()
    }

    /// Pagos mínimos mensuales totales de todas las deudas activas.
    pub fn pagos_minimos_mensuales(&self) -> f64 {
        self.deudas_activas()
            .iter()
            .map(|d| d.pago_pi_mensual())
            .sum()
    }

    /// Calcula, para cada deuda activa, cuánto se ahorraría en intereses
    /// si se le aplicaran `extra` dólares extra por mes hasta liquidar.
    /// Ordena el ranking de mayor a menor ahorro de intereses y devuelve
    /// la mejor candidata junto al ranking completo.
    pub fn mejor_destino_pago_extra(&self, extra: f64) -> Option<RecomendacionPagoExtra> {
        if extra <= 0.01 {
            return None;
        }
        let mut ranking: Vec<(String, AhorroPagoExtra)> = self
            .deudas_activas()
            .iter()
            .filter_map(|d| {
                d.ahorro_por_pago_extra(extra)
                    .map(|a| (d.nombre.clone(), a))
            })
            .collect();

        if ranking.is_empty() {
            return None;
        }

        ranking.sort_by(|a, b| {
            b.1.intereses_ahorrados
                .partial_cmp(&a.1.intereses_ahorrados)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let (nombre, ahorro) = ranking[0].clone();
        Some(RecomendacionPagoExtra {
            nombre_deuda: nombre,
            ahorro,
            ranking,
        })
    }

    /// Flujo de caja libre por mes: ingreso − pagos mínimos de deudas.
    /// Nota: no descuenta gastos del presupuesto (esos se calculan por separado).
    pub fn flujo_libre_mensual(&self) -> f64 {
        self.ingreso_mensual_total() - self.pagos_minimos_mensuales()
    }

    /// Proyecta el saldo disponible en banco/efectivo en `meses` meses,
    /// dado un flujo_extra_mensual (ingreso − gastos − pagos mínimos).
    pub fn proyectar_saldo(&self, flujo_mensual: f64, meses: u32) -> f64 {
        self.saldo_disponible + flujo_mensual * meses as f64
    }

    /// Meses aproximados para liquidar todas las deudas activas
    /// dado un monto mensual de abono `abono_mensual`.
    /// Retorna None si no es posible (abono <= intereses promedio).
    pub fn meses_para_libertad(&self, abono_mensual: f64) -> Option<u32> {
        let deuda = self.deuda_total_actual();
        if deuda < 0.01 {
            return Some(0);
        }
        if abono_mensual <= 0.01 {
            return None;
        }
        // Estimación lineal simple (conservadora, sin interés compuesto extra)
        let tasa_promedio: f64 = {
            let activas = self.deudas_activas();
            if activas.is_empty() {
                0.0
            } else {
                activas.iter().map(|d| d.tasa_anual).sum::<f64>() / activas.len() as f64
            }
        };
        let tasa_mensual = tasa_promedio / 100.0 / 12.0;
        if abono_mensual <= deuda * tasa_mensual {
            return None; // El abono no alcanza ni para cubrir intereses
        }
        // Fórmula de amortización: n = -ln(1 - (r*PV)/PMT) / ln(1+r)
        if tasa_mensual < 1e-9 {
            return Some((deuda / abono_mensual).ceil() as u32);
        }
        let n = -(1.0 - (tasa_mensual * deuda) / abono_mensual).ln() / (1.0 + tasa_mensual).ln();
        if n.is_finite() && n > 0.0 {
            Some(n.ceil() as u32)
        } else {
            None
        }
    }

    /// Diagnóstico completo: analiza todos los meses de todas las deudas.
    pub fn diagnosticar(&self) -> DiagnosticoGlobal {
        let mut errores = Vec::new();
        let mut resumen_por_deuda = Vec::new();
        let mut total_pagado = 0.0;
        let mut total_intereses = 0.0;
        let mut total_cargos = 0.0;
        let mut deuda_inicial = 0.0;
        let mut deuda_final = 0.0;
        let mut max_meses = 0usize;

        for d in &self.deudas {
            if d.historial.is_empty() {
                continue;
            }
            let si = d.historial.first().unwrap().saldo_inicio;
            let sf = d.historial.last().unwrap().saldo_final;
            let tp: f64 = d.historial.iter().map(|m| m.pago + m.pago_escrow).sum();
            let tc: f64 = d.historial.iter().map(|m| m.nuevos_cargos).sum();
            let ti: f64 = d.historial.iter().map(|m| m.intereses).sum();

            deuda_inicial += si;
            deuda_final += sf;
            total_pagado += tp;
            total_intereses += ti;
            total_cargos += tc;
            if d.historial.len() > max_meses {
                max_meses = d.historial.len();
            }

            let tendencia = if sf < si * 0.5 {
                "📉 Reduciéndose bien".to_string()
            } else if sf > si {
                "📈 ¡CRECIÓ! Acción urgente".to_string()
            } else if sf > si * 0.8 {
                "➡️ Casi estancada".to_string()
            } else {
                "📉 Bajando lento".to_string()
            };

            resumen_por_deuda.push(ResumenDeuda {
                nombre: d.nombre.clone(),
                saldo_inicial: si,
                saldo_final: sf,
                total_pagado: tp,
                total_cargos: tc,
                total_intereses: ti,
                meses: d.historial.len(),
                tendencia,
            });

            // Diagnosticar cada mes
            for mp in &d.historial {
                let tasa_mensual = d.tasa_anual / 100.0 / 12.0;
                let interes_del_saldo = mp.saldo_inicio * tasa_mensual;

                let (error, nota) = if mp.pago < 0.01 && mp.saldo_inicio > 0.01 {
                    (
                        ErrorPago::NoPagoNada,
                        if d.obligatoria {
                            format!(
                                "⛔ NO PAGÓ — PAGO FIJO OBLIGATORIO. Se acumularon ${:.2} en intereses. ¡Riesgo de perder el bien!",
                                mp.intereses
                            )
                        } else {
                            format!(
                                "No pagó nada. Se acumularon ${:.2} en intereses.",
                                mp.intereses
                            )
                        },
                    )
                } else if mp.nuevos_cargos > mp.pago && mp.saldo_inicio > 100.0 {
                    (
                        ErrorPago::SiguioUsandoTarjeta,
                        format!(
                            "Pagó ${:.2} pero cargó ${:.2} nuevos. La deuda creció ${:.2}.",
                            mp.pago,
                            mp.nuevos_cargos,
                            mp.nuevos_cargos - mp.pago
                        ),
                    )
                } else if mp.pago < interes_del_saldo && interes_del_saldo > 1.0 {
                    (
                        ErrorPago::PagoInsuficiente,
                        format!(
                            "Pago de ${:.2} no cubre intereses de ${:.2}. Deuda creciendo.",
                            mp.pago, interes_del_saldo
                        ),
                    )
                } else if mp.saldo_final < 0.01 {
                    (ErrorPago::PagoExcelente, "¡Deuda liquidada!".to_string())
                } else if mp.pago >= mp.saldo_inicio * 0.1 {
                    (
                        ErrorPago::PagoExcelente,
                        format!(
                            "Buen pago de ${:.2} ({:.0}% del saldo).",
                            mp.pago,
                            mp.pago / mp.saldo_inicio * 100.0
                        ),
                    )
                } else {
                    (
                        ErrorPago::PagoCorrecto,
                        format!("Pago aceptable de ${:.2}.", mp.pago),
                    )
                };

                // Solo registrar errores y pagos excelentes significativos
                let recomendado = (interes_del_saldo * 2.0).max(d.pago_pi_mensual());
                match &error {
                    ErrorPago::PagoCorrecto => {}
                    _ => {
                        errores.push(DiagnosticoMes {
                            deuda: d.nombre.clone(),
                            mes: mp.mes.clone(),
                            pago_real: mp.pago,
                            pago_recomendado: recomendado,
                            diferencia: mp.pago - recomendado,
                            error,
                            nota,
                        });
                    }
                }
            }
        }

        // Generar recomendaciones
        let mut recomendaciones = Vec::new();
        let cambio_neto = deuda_final - deuda_inicial;

        // Advertencia especial para deudas obligatorias con pagos fallidos
        for d in &self.deudas {
            if d.obligatoria && d.activa {
                let meses_sin_pago = d
                    .historial
                    .iter()
                    .filter(|m| m.pago < 0.01 && m.saldo_inicio > 0.01)
                    .count();
                let meses_pago_parcial = d
                    .historial
                    .iter()
                    .filter(|m| m.pago > 0.0 && m.pago < d.pago_pi_mensual() * 0.95)
                    .count();
                if meses_sin_pago > 0 {
                    recomendaciones.push(format!(
                        "🚨 '{}' es PAGO FIJO y tuvo {} mes(es) sin pago. ¡No se puede fallar — riesgo de perder el bien!",
                        d.nombre, meses_sin_pago
                    ));
                }
                if meses_pago_parcial > 0 {
                    recomendaciones.push(format!(
                        "⚠️  '{}' es PAGO FIJO y tuvo {} mes(es) con pago parcial. Debe cubrir al menos P&I (${:.2}).",
                        d.nombre, meses_pago_parcial, d.pago_pi_mensual()
                    ));
                }
            }
        }

        if cambio_neto > 0.0 {
            recomendaciones.push(format!(
                "⛔ La deuda total CRECIÓ ${:.2} en {} meses. Acción urgente necesaria.",
                cambio_neto, max_meses
            ));
        }

        // Encontrar deudas que crecieron
        for r in &resumen_por_deuda {
            if r.saldo_final > r.saldo_inicial && r.saldo_inicial > 100.0 {
                recomendaciones.push(format!(
                    "🔴 {} creció de ${:.2} a ${:.2} (+${:.2}). Problema: se siguió usando o no se pagó suficiente.",
                    r.nombre,
                    r.saldo_inicial,
                    r.saldo_final,
                    r.saldo_final - r.saldo_inicial
                ));
            }
            if r.total_cargos > r.total_pagado * 0.5 && r.total_cargos > 500.0 {
                recomendaciones.push(format!(
                    "🔴 {} tuvo ${:.2} en nuevos cargos vs ${:.2} pagados. Dejar de usar esta tarjeta.",
                    r.nombre, r.total_cargos, r.total_pagado
                ));
            }
        }

        // Sugerir orden de pago (avalancha: tasa más alta primero)
        let activas: Vec<_> = resumen_por_deuda
            .iter()
            .filter(|r| r.saldo_final > 0.01)
            .collect();
        if activas.len() > 1 {
            // Buscar la tasa de cada una
            let mut con_tasa: Vec<(&ResumenDeuda, f64)> = activas
                .iter()
                .map(|r| {
                    let tasa = self
                        .deudas
                        .iter()
                        .find(|d| d.nombre == r.nombre)
                        .map(|d| d.tasa_anual)
                        .unwrap_or(0.0);
                    (*r, tasa)
                })
                .collect();
            con_tasa.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            recomendaciones.push(
                "💡 Orden de pago recomendado (avalancha — tasa más alta primero):".to_string(),
            );
            for (i, (r, tasa)) in con_tasa.iter().enumerate() {
                recomendaciones.push(format!(
                    "   {}. {} (saldo ${:.2}, tasa {:.1}%)",
                    i + 1,
                    r.nombre,
                    r.saldo_final,
                    tasa
                ));
            }
        }

        if total_intereses > 100.0 {
            recomendaciones.push(format!(
                "💸 Se pagaron ~${:.2} en intereses en {} meses. Eso es dinero regalado al banco.",
                total_intereses, max_meses
            ));
        }

        DiagnosticoGlobal {
            total_pagado,
            total_intereses_estimados: total_intereses,
            total_nuevos_cargos: total_cargos,
            deuda_inicial_total: deuda_inicial,
            deuda_final_total: deuda_final,
            cambio_neto,
            meses_analizados: max_meses,
            errores,
            resumen_por_deuda,
            recomendaciones,
        }
    }

    /// Genera CSV del historial de una deuda.
    pub fn csv_historial_deuda(&self, nombre: &str) -> String {
        let mut s =
            String::from("\u{FEFF}Mes,Saldo Inicio,Pago,Nuevos Cargos,Intereses,Saldo Final\n");
        if let Some(d) = self.deudas.iter().find(|d| d.nombre == nombre) {
            for m in &d.historial {
                s.push_str(&format!(
                    "{},{:.2},{:.2},{:.2},{:.2},{:.2}\n",
                    m.mes, m.saldo_inicio, m.pago, m.nuevos_cargos, m.intereses, m.saldo_final
                ));
            }
        }
        s
    }

    /// Genera CSV de todas las deudas (resumen).
    pub fn csv_resumen_global(&self) -> String {
        let diag = self.diagnosticar();
        let mut s = String::from("\u{FEFF}Deuda,Saldo Inicial,Saldo Final,Total Pagado,Nuevos Cargos,Intereses Est.,Meses,Tendencia\n");
        for r in &diag.resumen_por_deuda {
            s.push_str(&format!(
                "\"{}\",{:.2},{:.2},{:.2},{:.2},{:.2},{},\"{}\"\n",
                r.nombre,
                r.saldo_inicial,
                r.saldo_final,
                r.total_pagado,
                r.total_cargos,
                r.total_intereses,
                r.meses,
                r.tendencia
            ));
        }
        s
    }

    /// Importa deudas desde un archivo CSV.
    ///
    /// Formato esperado: cuenta,mes,saldo,pago,nuevos_cargos
    /// El CSV puede tener varias cuentas mezcladas; se agrupan automáticamente.
    /// Se asume tasa_anual = 0 (se puede ajustar después).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn importar_csv(ruta: &str) -> Result<RastreadorDeudas, String> {
        let contenido =
            fs::read_to_string(ruta).map_err(|e| format!("No se pudo leer '{}': {}", ruta, e))?;

        // Quitar BOM si existe
        let contenido = contenido.trim_start_matches('\u{FEFF}');

        let mut lineas = contenido.lines();

        // Validar header
        let header = lineas.next().ok_or_else(|| "Archivo vacío".to_string())?;
        let cols: Vec<&str> = header.split(',').map(|s| s.trim()).collect();
        if cols.len() < 4
            || !cols[0].eq_ignore_ascii_case("cuenta")
            || !cols[1].eq_ignore_ascii_case("mes")
        {
            return Err(format!(
                "Header inválido. Esperado: cuenta,mes,saldo,pago[,nuevos_cargos]. Encontrado: {}",
                header
            ));
        }
        let tiene_cargos = cols.len() >= 5;

        // Agrupar por cuenta
        let mut mapa: std::collections::BTreeMap<String, Vec<(String, f64, f64, f64)>> =
            std::collections::BTreeMap::new();

        for (num_linea, linea) in lineas.enumerate() {
            let linea = linea.trim();
            if linea.is_empty() {
                continue;
            }
            let campos: Vec<&str> = linea.splitn(5, ',').collect();
            if campos.len() < 4 {
                continue;
            }
            let cuenta = campos[0].trim().trim_matches('"').to_string();
            let mes = campos[1].trim().trim_matches('"').to_string();
            let saldo: f64 = campos[2]
                .trim()
                .parse()
                .map_err(|_| format!("Línea {}: saldo inválido '{}'", num_linea + 2, campos[2]))?;
            let pago: f64 = campos[3]
                .trim()
                .parse()
                .map_err(|_| format!("Línea {}: pago inválido '{}'", num_linea + 2, campos[3]))?;
            let cargos: f64 = if tiene_cargos && campos.len() >= 5 {
                campos[4].trim().parse().unwrap_or(0.0)
            } else {
                0.0
            };

            mapa.entry(cuenta)
                .or_default()
                .push((mes, saldo, pago, cargos));
        }

        if mapa.is_empty() {
            return Err("No se encontraron datos en el CSV".to_string());
        }

        let mut rastreador = RastreadorDeudas::default();

        for (nombre, registros) in &mapa {
            let mut deuda = DeudaRastreada::nueva(nombre, 0.0, 0.0);

            for (mes, saldo, pago, cargos) in registros {
                deuda.registrar_mes(mes, *saldo, *pago, *cargos);
            }

            // Estimar pago_minimo como el mínimo pago no-cero
            let min_pago = registros
                .iter()
                .map(|(_, _, p, _)| *p)
                .filter(|p| *p > 0.01)
                .fold(f64::MAX, f64::min);
            if min_pago < f64::MAX {
                deuda.pago_minimo = min_pago;
            }

            rastreador.deudas.push(deuda);
        }

        Ok(rastreador)
    }

    /// Simula el plan completo para salir de todas las deudas.
    /// Pagos corrientes (renta, seguro — tasa 0 + obligatoria) se descuentan del presupuesto
    /// pero NO aparecen como deudas a liquidar.
    /// Estrategia: primero paga mínimos, luego sobrante va según avalancha o bola de nieve.
    pub fn simular_libertad(
        &self,
        presupuesto_mensual: f64,
        estrategia_bola_nieve: bool,
    ) -> SimulacionLibertad {
        let estrategia = if estrategia_bola_nieve {
            EstrategiaLibertad::BolaNieve
        } else {
            EstrategiaLibertad::Avalancha
        };
        self.simular_libertad_editado(presupuesto_mensual, &estrategia, &[])
    }

    /// Versión editable del simulador de libertad financiera.
    ///
    /// A diferencia de [`Self::simular_libertad`], permite:
    ///   - Escoger estrategia de reparto del sobrante (Avalancha, Bola de nieve
    ///     o Pesos relativos por deuda).
    ///   - Fijar manualmente el pago a una deuda específica en un mes concreto
    ///     via [`AjusteMensualLibertad`]; lo sobrante o faltante se re-reparte
    ///     automáticamente entre las demás deudas activas.
    ///
    /// Útil para planificación tipo hoja de cálculo: "quita $100 de Tarjeta A
    /// en el mes 3 y ponlos en Tarjeta B para nivelarla".
    pub fn simular_libertad_editado(
        &self,
        presupuesto_mensual: f64,
        estrategia: &EstrategiaLibertad,
        ajustes: &[AjusteMensualLibertad],
    ) -> SimulacionLibertad {
        let gastos_fijos: Vec<(String, f64)> = self
            .deudas
            .iter()
            .filter(|d| d.activa && d.es_pago_corriente())
            .map(|d| (d.nombre.clone(), d.pago_total_mensual()))
            .collect();
        let total_gastos_fijos: f64 = gastos_fijos.iter().map(|(_, m)| *m).sum();

        let mut deudas: Vec<DeudaSimulada> = self
            .deudas
            .iter()
            .filter(|d| d.activa && !d.es_pago_corriente() && d.saldo_actual() > 0.01)
            .map(|d| DeudaSimulada {
                nombre: d.nombre.clone(),
                saldo: d.saldo_actual(),
                tasa_anual: d.tasa_anual,
                pago_minimo: d.pago_pi_mensual(),
                liquidada_mes: None,
                obligatoria: d.obligatoria,
            })
            .collect();

        let nombre_estrategia = estrategia.nombre().to_string();
        let n = deudas.len();
        if n == 0 {
            return SimulacionLibertad {
                presupuesto_mensual,
                estrategia: nombre_estrategia,
                meses: Vec::new(),
                total_pagado: 0.0,
                total_intereses: 0.0,
                orden_liquidacion: Vec::new(),
                gastos_fijos,
                total_gastos_fijos,
            };
        }

        let presupuesto_deudas = (presupuesto_mensual - total_gastos_fijos).max(0.0);
        let mut meses_resultado: Vec<MesSimulado> = Vec::new();
        let mut total_pagado = 0.0;
        let mut total_intereses = 0.0;
        let mut orden_liquidacion: Vec<(String, usize)> = Vec::new();
        let minimos_originales: f64 = deudas.iter().map(|d| d.pago_minimo).sum();

        for mes_num in 1..=600usize {
            let vivas = deudas.iter().filter(|d| d.liquidada_mes.is_none()).count();
            if vivas == 0 {
                break;
            }

            let minimos_vivos: f64 = deudas
                .iter()
                .filter(|d| d.liquidada_mes.is_none())
                .map(|d| d.pago_minimo)
                .sum();
            let liberado = minimos_originales - minimos_vivos;

            let mut disponible = presupuesto_deudas;
            let mut pagos_mes: Vec<(String, f64)> = Vec::new();
            let mut intereses_mes: Vec<(String, f64)> = Vec::new();

            // Paso 0: aplicar ajustes manuales del usuario para este mes.
            // Los ajustes reemplazan cualquier lógica automática para esa deuda.
            let mut deudas_con_ajuste: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for ajuste in ajustes.iter().filter(|a| a.mes == mes_num) {
                let idx = match deudas
                    .iter()
                    .position(|d| d.nombre == ajuste.nombre_deuda && d.liquidada_mes.is_none())
                {
                    Some(i) => i,
                    None => continue,
                };
                let saldo = deudas[idx].saldo;
                let pago = ajuste.pago_forzado.max(0.0).min(saldo).min(disponible);
                disponible -= pago;
                pagos_mes.push((deudas[idx].nombre.clone(), pago));
                deudas_con_ajuste.insert(deudas[idx].nombre.clone());
            }

            // Paso 1: mínimos de deudas SIN ajuste, obligatorias primero.
            for obligatoria_primero in [true, false] {
                for d in deudas.iter() {
                    if d.liquidada_mes.is_some()
                        || d.obligatoria != obligatoria_primero
                        || deudas_con_ajuste.contains(&d.nombre)
                    {
                        continue;
                    }
                    let minimo = d.pago_minimo.min(d.saldo);
                    let pago = minimo.min(disponible);
                    disponible -= pago;
                    pagos_mes.push((d.nombre.clone(), pago));
                }
            }

            // Paso 2: distribuir sobrante según la estrategia.
            // Los ajustes manuales NO reciben extra automático: el usuario ya decidió.
            if disponible > 0.01 {
                let mut indices_vivas: Vec<usize> = (0..n)
                    .filter(|&i| {
                        deudas[i].liquidada_mes.is_none()
                            && !deudas_con_ajuste.contains(&deudas[i].nombre)
                    })
                    .collect();

                match estrategia {
                    EstrategiaLibertad::BolaNieve => {
                        indices_vivas.sort_by(|&a, &b| {
                            deudas[a]
                                .saldo
                                .partial_cmp(&deudas[b].saldo)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                        for &idx in &indices_vivas {
                            if disponible < 0.01 {
                                break;
                            }
                            let d = &deudas[idx];
                            let ya_pagado = pagos_mes
                                .iter()
                                .find(|(nm, _)| *nm == d.nombre)
                                .map(|(_, p)| *p)
                                .unwrap_or(0.0);
                            let max_extra = (d.saldo - ya_pagado).max(0.0);
                            let extra = max_extra.min(disponible);
                            if extra > 0.01 {
                                if let Some(entry) = pagos_mes
                                    .iter_mut()
                                    .find(|(nm, _)| *nm == deudas[idx].nombre)
                                {
                                    entry.1 += extra;
                                }
                                disponible -= extra;
                            }
                        }
                    }
                    EstrategiaLibertad::Avalancha => {
                        indices_vivas.sort_by(|&a, &b| {
                            deudas[b]
                                .tasa_anual
                                .partial_cmp(&deudas[a].tasa_anual)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                        for &idx in &indices_vivas {
                            if disponible < 0.01 {
                                break;
                            }
                            let d = &deudas[idx];
                            let ya_pagado = pagos_mes
                                .iter()
                                .find(|(nm, _)| *nm == d.nombre)
                                .map(|(_, p)| *p)
                                .unwrap_or(0.0);
                            let max_extra = (d.saldo - ya_pagado).max(0.0);
                            let extra = max_extra.min(disponible);
                            if extra > 0.01 {
                                if let Some(entry) = pagos_mes
                                    .iter_mut()
                                    .find(|(nm, _)| *nm == deudas[idx].nombre)
                                {
                                    entry.1 += extra;
                                }
                                disponible -= extra;
                            }
                        }
                    }
                    EstrategiaLibertad::Pesos(pesos) => {
                        // Reparto proporcional al peso entre deudas vivas sin ajuste.
                        // Iteramos hasta 3 rondas para re-repartir el sobrante si alguna
                        // deuda se llena antes de agotar el presupuesto.
                        for _ in 0..3 {
                            if disponible < 0.01 {
                                break;
                            }
                            let total_peso: f64 = indices_vivas
                                .iter()
                                .map(|&i| {
                                    pesos
                                        .iter()
                                        .find(|(nm, _)| *nm == deudas[i].nombre)
                                        .map(|(_, p)| *p)
                                        .unwrap_or(0.0)
                                        .max(0.0)
                                })
                                .sum();
                            if total_peso <= 1e-9 {
                                break;
                            }
                            let disponible_ronda = disponible;
                            let mut nuevos_vivas = Vec::new();
                            for &idx in &indices_vivas {
                                let peso = pesos
                                    .iter()
                                    .find(|(nm, _)| *nm == deudas[idx].nombre)
                                    .map(|(_, p)| *p)
                                    .unwrap_or(0.0)
                                    .max(0.0);
                                if peso <= 1e-9 {
                                    continue;
                                }
                                let cuota = disponible_ronda * peso / total_peso;
                                let d = &deudas[idx];
                                let ya_pagado = pagos_mes
                                    .iter()
                                    .find(|(nm, _)| *nm == d.nombre)
                                    .map(|(_, p)| *p)
                                    .unwrap_or(0.0);
                                let max_extra = (d.saldo - ya_pagado).max(0.0);
                                let extra = cuota.min(max_extra).min(disponible);
                                if extra > 0.01 {
                                    if let Some(entry) = pagos_mes
                                        .iter_mut()
                                        .find(|(nm, _)| *nm == deudas[idx].nombre)
                                    {
                                        entry.1 += extra;
                                    }
                                    disponible -= extra;
                                }
                                // Si aún cabe más, sigue en la próxima ronda.
                                if (max_extra - extra) > 0.01 {
                                    nuevos_vivas.push(idx);
                                }
                            }
                            if nuevos_vivas.len() == indices_vivas.len() {
                                // No se llenó ninguna deuda: no habrá nada que re-repartir.
                                break;
                            }
                            indices_vivas = nuevos_vivas;
                        }
                    }
                }
            }

            // Paso 3: aplicar pagos e intereses.
            let mut saldos_mes: Vec<(String, f64)> = Vec::new();
            let mut liquidadas_este_mes: Vec<String> = Vec::new();

            for d in deudas.iter_mut() {
                if d.liquidada_mes.is_some() {
                    saldos_mes.push((d.nombre.clone(), 0.0));
                    intereses_mes.push((d.nombre.clone(), 0.0));
                    continue;
                }
                let pago = pagos_mes
                    .iter()
                    .find(|(nm, _)| *nm == d.nombre)
                    .map(|(_, p)| *p)
                    .unwrap_or(0.0);
                let saldo_post_pago = (d.saldo - pago).max(0.0);
                let tasa_mensual = d.tasa_anual / 100.0 / 12.0;
                let interes = saldo_post_pago * tasa_mensual;
                d.saldo = saldo_post_pago + interes;

                total_pagado += pago;
                total_intereses += interes;
                intereses_mes.push((d.nombre.clone(), interes));

                if d.saldo < 0.01 {
                    d.saldo = 0.0;
                    d.liquidada_mes = Some(mes_num);
                    liquidadas_este_mes.push(d.nombre.clone());
                    orden_liquidacion.push((d.nombre.clone(), mes_num));
                }
                saldos_mes.push((d.nombre.clone(), d.saldo));
            }

            let deuda_total: f64 = saldos_mes.iter().map(|(_, s)| *s).sum();

            meses_resultado.push(MesSimulado {
                mes_numero: mes_num,
                saldos: saldos_mes,
                pagos: pagos_mes,
                intereses: intereses_mes,
                deuda_total,
                liquidadas_este_mes,
                presupuesto_efectivo: presupuesto_deudas,
                sobrante: disponible.max(0.0),
                liberado_de_liquidadas: liberado,
            });

            if deuda_total < 0.01 {
                break;
            }
        }

        SimulacionLibertad {
            presupuesto_mensual,
            estrategia: nombre_estrategia,
            meses: meses_resultado,
            total_pagado,
            total_intereses,
            orden_liquidacion,
            gastos_fijos,
            total_gastos_fijos,
        }
    }

    /// Compara dos planes (por ejemplo el automático y el editado) y devuelve
    /// las diferencias relevantes para la toma de decisiones.
    pub fn comparar_planes(
        base: &SimulacionLibertad,
        alternativa: &SimulacionLibertad,
    ) -> ComparacionPlanes {
        let meses_base = base.meses.len() as i32;
        let meses_alt = alternativa.meses.len() as i32;
        let max_pago_base = base
            .meses
            .iter()
            .map(|m| m.pagos.iter().map(|(_, p)| *p).sum::<f64>())
            .fold(0.0_f64, f64::max);
        let max_pago_alt = alternativa
            .meses
            .iter()
            .map(|m| m.pagos.iter().map(|(_, p)| *p).sum::<f64>())
            .fold(0.0_f64, f64::max);

        ComparacionPlanes {
            meses_base: meses_base as usize,
            meses_alternativa: meses_alt as usize,
            diferencia_meses: meses_alt - meses_base,
            intereses_base: base.total_intereses,
            intereses_alternativa: alternativa.total_intereses,
            diferencia_intereses: alternativa.total_intereses - base.total_intereses,
            max_pago_mensual_base: max_pago_base,
            max_pago_mensual_alternativa: max_pago_alt,
            orden_liquidacion_base: base.orden_liquidacion.clone(),
            orden_liquidacion_alternativa: alternativa.orden_liquidacion.clone(),
        }
    }

    /// Dado un presupuesto mensual, calcula en cuántos meses se sale de deudas.
    /// Retorna None si no se puede pagar (presupuesto < mínimos).
    pub fn meses_para_salir(&self, presupuesto: f64, bola_nieve: bool) -> Option<usize> {
        let sim = self.simular_libertad(presupuesto, bola_nieve);
        if sim.meses.is_empty() {
            return Some(0);
        }
        let ultimo = sim.meses.last()?;
        if ultimo.deuda_total < 0.01 {
            Some(sim.meses.len())
        } else {
            None // no alcanza
        }
    }

    /// Busca el aporte mínimo mensual para salir de deudas en exactamente `objetivo_meses`.
    /// Usa búsqueda binaria sobre el presupuesto. Retorna el monto redondeado al dólar.
    fn aporte_minimo_para_meses(&self, objetivo_meses: usize, bola_nieve: bool) -> Option<f64> {
        // Cotas: mínimo = suma de pagos mínimos + corrientes, máximo = deuda total
        let corrientes: f64 = self
            .deudas
            .iter()
            .filter(|d| d.activa && d.es_pago_corriente())
            .map(|d| d.pago_total_mensual())
            .sum();
        let min_deudas: f64 = self
            .deudas
            .iter()
            .filter(|d| d.activa && !d.es_pago_corriente() && d.saldo_actual() > 0.01)
            .map(|d| d.pago_pi_mensual())
            .sum();
        let deuda_total: f64 = self
            .deudas
            .iter()
            .filter(|d| d.activa && !d.es_pago_corriente())
            .map(|d| d.saldo_actual())
            .sum();

        if deuda_total < 0.01 {
            return Some(0.0);
        }

        let mut lo = min_deudas + corrientes;
        let mut hi = deuda_total + corrientes + 1.0;

        // Verificar que al menos con hi se puede en <= objetivo_meses
        match self.meses_para_salir(hi, bola_nieve) {
            Some(m) if m <= objetivo_meses => {}
            _ => {
                hi *= 2.0;
                match self.meses_para_salir(hi, bola_nieve) {
                    Some(m) if m <= objetivo_meses => {}
                    _ => return None,
                }
            }
        }

        // Búsqueda binaria: encontrar el menor presupuesto que da <= objetivo_meses
        for _ in 0..60 {
            if hi - lo < 0.50 {
                break;
            }
            let mid = (lo + hi) / 2.0;
            match self.meses_para_salir(mid, bola_nieve) {
                Some(m) if m <= objetivo_meses => hi = mid,
                _ => lo = mid,
            }
        }

        Some(hi.ceil())
    }

    /// Genera tabla de proyección: para cada número de meses (de `max_meses` hasta 1),
    /// calcula el aporte mínimo mensual necesario para salir de deudas.
    /// Retorna Vec<(meses, aporte_minimo, total_pagado, total_intereses)>.
    pub fn tabla_aporte_minimo(
        &self,
        max_meses: usize,
        min_meses: usize,
        bola_nieve: bool,
    ) -> Vec<(usize, f64, f64, f64)> {
        let mut tabla = Vec::new();
        for objetivo in (min_meses..=max_meses).rev() {
            if let Some(aporte) = self.aporte_minimo_para_meses(objetivo, bola_nieve) {
                let sim = self.simular_libertad(aporte, bola_nieve);
                let meses_reales = sim.meses.len();
                tabla.push((meses_reales, aporte, sim.total_pagado, sim.total_intereses));
            }
        }
        tabla
    }
}

/// Estado de una deuda durante la simulación.
#[derive(Clone, Debug)]
struct DeudaSimulada {
    nombre: String,
    saldo: f64,
    tasa_anual: f64,
    pago_minimo: f64,
    liquidada_mes: Option<usize>,
    /// Deuda obligatoria (hipoteca, carro, etc.) — se paga primero.
    obligatoria: bool,
}

/// Un mes de la simulación global.
#[derive(Clone, Debug)]
pub struct MesSimulado {
    pub mes_numero: usize,
    pub saldos: Vec<(String, f64)>,
    pub pagos: Vec<(String, f64)>,
    pub intereses: Vec<(String, f64)>,
    pub deuda_total: f64,
    pub liquidadas_este_mes: Vec<String>,
    /// Presupuesto efectivo para deudas este mes (descontando gastos fijos).
    pub presupuesto_efectivo: f64,
    /// Dinero sobrante que no se pudo asignar este mes.
    pub sobrante: f64,
    /// Dinero liberado de deudas ya liquidadas en meses anteriores.
    pub liberado_de_liquidadas: f64,
}

/// Resultado de la simulación completa de libertad financiera.
#[derive(Clone, Debug)]
pub struct SimulacionLibertad {
    pub presupuesto_mensual: f64,
    pub estrategia: String,
    pub meses: Vec<MesSimulado>,
    pub total_pagado: f64,
    pub total_intereses: f64,
    pub orden_liquidacion: Vec<(String, usize)>,
    /// Pagos corrientes (renta, seguro, etc.) que se restan del presupuesto cada mes.
    pub gastos_fijos: Vec<(String, f64)>,
    pub total_gastos_fijos: f64,
}

/// Estrategia de reparto del sobrante sobre los pagos mínimos.
#[derive(Clone, Debug, PartialEq)]
pub enum EstrategiaLibertad {
    /// Tasa más alta primero (ahorra más intereses).
    Avalancha,
    /// Saldo más bajo primero (motivación rápida).
    BolaNieve,
    /// Reparto proporcional a pesos por deuda (nombre → peso).
    /// Útil para "nivelar" varias deudas al mismo tiempo.
    Pesos(Vec<(String, f64)>),
}

impl EstrategiaLibertad {
    pub fn nombre(&self) -> &'static str {
        match self {
            EstrategiaLibertad::Avalancha => "Avalancha",
            EstrategiaLibertad::BolaNieve => "Bola de nieve",
            EstrategiaLibertad::Pesos(_) => "Pesos personalizados",
        }
    }

    /// Construye una estrategia `Pesos` a partir de pares `(nombre, peso)`,
    /// normalizando para que la suma sea 1.0. Si todos los pesos son 0 o
    /// negativos, devuelve `Avalancha` como fallback seguro.
    pub fn pesos_normalizados(pesos: Vec<(String, f64)>) -> Self {
        let suma: f64 = pesos.iter().map(|(_, p)| p.max(0.0)).sum();
        if suma <= 1e-9 {
            return EstrategiaLibertad::Avalancha;
        }
        let normalizados = pesos
            .into_iter()
            .map(|(n, p)| (n, (p.max(0.0)) / suma))
            .collect();
        EstrategiaLibertad::Pesos(normalizados)
    }
}

/// Ajuste manual del usuario: fijar el pago a una deuda concreta en un mes concreto.
/// El simulador lo respeta y re-reparte el resto del presupuesto entre las demás.
#[derive(Clone, Debug, PartialEq)]
pub struct AjusteMensualLibertad {
    /// Mes 1-indexado (1 = primer mes de la simulación).
    pub mes: usize,
    pub nombre_deuda: String,
    pub pago_forzado: f64,
}

impl AjusteMensualLibertad {
    pub fn nuevo(mes: usize, nombre_deuda: impl Into<String>, pago_forzado: f64) -> Self {
        Self {
            mes,
            nombre_deuda: nombre_deuda.into(),
            pago_forzado: pago_forzado.max(0.0),
        }
    }
}

/// Diferencias relevantes entre dos simulaciones de libertad financiera.
#[derive(Clone, Debug)]
pub struct ComparacionPlanes {
    pub meses_base: usize,
    pub meses_alternativa: usize,
    /// `alternativa - base`: negativo = el plan alternativo sale más rápido.
    pub diferencia_meses: i32,
    pub intereses_base: f64,
    pub intereses_alternativa: f64,
    /// `alternativa - base`: negativo = la alternativa ahorra intereses.
    pub diferencia_intereses: f64,
    pub max_pago_mensual_base: f64,
    pub max_pago_mensual_alternativa: f64,
    pub orden_liquidacion_base: Vec<(String, usize)>,
    pub orden_liquidacion_alternativa: Vec<(String, usize)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TipoRegistro {
    AnalisisDeuda {
        deuda: AnalisisDeuda,
        opciones: Vec<OpcionPago>,
        mejor_opcion: Option<String>,
    },
    CorteBancario {
        corte: CorteBancario,
        tasa_mensual: f64,
        tasa_anual: f64,
        saldo_que_genero_interes: f64,
        pago_a_capital: f64,
        pago_a_interes: f64,
        monto_corta_intereses: f64,
    },
    Comparacion(ComparacionRapida),
    MatrizDecision(MatrizDecision),
    ProyeccionAhorro {
        balance_mensual: f64,
        meses: u32,
        proyeccion: Vec<(u32, f64)>,
    },
    Accion(AccionRegistrada),
}

impl TipoRegistro {
    pub fn nombre_tipo(&self) -> &str {
        match self {
            TipoRegistro::AnalisisDeuda { .. } => "Análisis de Deuda",
            TipoRegistro::CorteBancario { .. } => "Corte Bancario",
            TipoRegistro::Comparacion(_) => "Comparación Rápida",
            TipoRegistro::MatrizDecision(_) => "Matriz de Decisión",
            TipoRegistro::ProyeccionAhorro { .. } => "Proyección de Ahorro",
            TipoRegistro::Accion(_) => "Acción Registrada",
        }
    }

    pub fn emoji(&self) -> &str {
        match self {
            TipoRegistro::AnalisisDeuda { .. } => "💳",
            TipoRegistro::CorteBancario { .. } => "🏦",
            TipoRegistro::Comparacion(_) => "⚖️",
            TipoRegistro::MatrizDecision(_) => "🧮",
            TipoRegistro::ProyeccionAhorro { .. } => "📈",
            TipoRegistro::Accion(_) => "📝",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistroAsesor {
    pub id: u64,
    pub fecha: String,
    pub hora: String,
    pub tipo_nombre: String,
    pub titulo: String,
    pub resumen: String,
    pub etiquetas: Vec<String>,
    pub datos: TipoRegistro,
}

impl RegistroAsesor {
    pub fn nuevo(
        id: u64,
        fecha: &str,
        hora: &str,
        titulo: &str,
        resumen: &str,
        etiquetas: Vec<String>,
        datos: TipoRegistro,
    ) -> Self {
        Self {
            id,
            fecha: fecha.to_string(),
            hora: hora.to_string(),
            tipo_nombre: datos.nombre_tipo().to_string(),
            titulo: titulo.to_string(),
            resumen: resumen.to_string(),
            etiquetas,
            datos,
        }
    }

    /// Línea CSV de resumen (sin header).
    pub fn csv_resumen(&self) -> String {
        format!(
            "{},{},{},\"{}\",\"{}\",\"{}\"",
            self.id,
            self.fecha,
            self.hora,
            self.tipo_nombre.replace('"', "\"\""),
            self.titulo.replace('"', "\"\""),
            self.resumen.replace('"', "\"\""),
        )
    }

    /// Detalle completo como texto legible (para imprimir / exportar).
    pub fn detalle_texto(&self) -> String {
        let mut s = String::new();
        s.push_str("══════════════════════════════════════════════════════\n");
        s.push_str(&format!(
            "  {} #{} — {} {}\n",
            self.datos.emoji(),
            self.id,
            self.tipo_nombre,
            self.fecha
        ));
        s.push_str(&format!("  {}\n", self.titulo));
        s.push_str("──────────────────────────────────────────────────────\n");

        match &self.datos {
            TipoRegistro::AnalisisDeuda {
                deuda,
                opciones,
                mejor_opcion,
            } => {
                s.push_str(&format!(
                    "  Deuda: {} — Saldo: ${:.2} — Tasa: {:.2}%/mes\n",
                    deuda.nombre,
                    deuda.saldo_total,
                    deuda.tasa_interes_mensual * 100.0
                ));
                s.push_str(&format!("  Pago mínimo: ${:.2}\n\n", deuda.pago_minimo));
                s.push_str(&format!(
                    "  {:<25} {:>8} {:>12} {:>12} {:>12}\n",
                    "Opción", "Meses", "Intereses", "Total", "Ahorro"
                ));
                s.push_str(&format!("  {}\n", "─".repeat(70)));
                for op in opciones {
                    s.push_str(&format!(
                        "  {:<25} {:>6}m   ${:>10.2}   ${:>10.2}   ${:>10.2}\n",
                        op.nombre,
                        op.meses_para_liquidar,
                        op.total_intereses,
                        op.total_pagado,
                        op.ahorro_vs_minimo
                    ));
                }
                if let Some(mejor) = mejor_opcion {
                    s.push_str(&format!("\n  ⭐ Mejor opción: {}\n", mejor));
                }
            }
            TipoRegistro::CorteBancario {
                corte,
                tasa_mensual,
                tasa_anual,
                saldo_que_genero_interes,
                pago_a_capital,
                pago_a_interes,
                monto_corta_intereses,
            } => {
                s.push_str(&format!("  Tarjeta: {}\n", corte.nombre_tarjeta));
                s.push_str(&format!("  Fecha corte: {}\n", corte.fecha_corte));
                s.push_str(&format!(
                    "  Saldo anterior: ${:.2} | Pago: ${:.2} | Compras: ${:.2}\n",
                    corte.saldo_anterior, corte.pago_realizado, corte.compras_periodo
                ));
                s.push_str(&format!(
                    "  Intereses cobrados: ${:.2} | Otros: ${:.2}\n",
                    corte.intereses_cobrados, corte.otros_cargos
                ));
                s.push_str(&format!(
                    "  Saldo al corte: ${:.2}\n\n",
                    corte.saldo_al_corte
                ));
                s.push_str(&format!(
                    "  Tasa calculada: {:.2}%/mes ({:.1}%/año)\n",
                    tasa_mensual * 100.0,
                    tasa_anual * 100.0
                ));
                s.push_str(&format!(
                    "  Saldo que generó interés: ${:.2}\n",
                    saldo_que_genero_interes
                ));
                s.push_str(&format!(
                    "  Del pago: ${:.2} a capital, ${:.2} a intereses\n",
                    pago_a_capital, pago_a_interes
                ));
                s.push_str(&format!(
                    "  Para cortar intereses: ${:.2}\n",
                    monto_corta_intereses
                ));
            }
            TipoRegistro::Comparacion(c) => {
                s.push_str(&format!("  {}\n\n", c.titulo));
                s.push_str(&format!(
                    "  A: {} — ${:.2}  ({})\n",
                    c.opcion_a, c.costo_a, c.beneficio_a
                ));
                s.push_str(&format!(
                    "  B: {} — ${:.2}  ({})\n",
                    c.opcion_b, c.costo_b, c.beneficio_b
                ));
                s.push_str(&format!("  Diferencia: ${:.2}\n", c.diferencia.abs()));
                s.push_str(&format!("  📌 {}\n", c.recomendacion));
            }
            TipoRegistro::MatrizDecision(m) => {
                s.push_str(&format!("  {} ({})\n\n", m.titulo, m.fecha));
                // Header
                s.push_str(&format!("  {:<20}", ""));
                for c in &m.criterios {
                    s.push_str(&format!(" {:>12}", format!("{}({:.1})", c.nombre, c.peso)));
                }
                s.push_str(&format!(" {:>10}\n", "TOTAL"));
                s.push_str(&format!(
                    "  {}\n",
                    "─".repeat(22 + m.criterios.len() * 13 + 11)
                ));
                let puntuaciones = m.puntuaciones();
                for (i, (opcion, score)) in puntuaciones.iter().enumerate() {
                    s.push_str(&format!("  {:<20}", opcion));
                    for j in 0..m.criterios.len() {
                        s.push_str(&format!(" {:>12.1}", m.valores[i][j]));
                    }
                    s.push_str(&format!(" {:>10.2}\n", score));
                }
                if let Some((op, sc)) = m.mejor_opcion() {
                    s.push_str(&format!("\n  ⭐ Recomendación: {} ({:.1}/10)\n", op, sc));
                }
            }
            TipoRegistro::ProyeccionAhorro {
                balance_mensual,
                meses,
                proyeccion,
            } => {
                s.push_str(&format!(
                    "  Balance mensual: ${:.2} — Proyección a {} meses\n\n",
                    balance_mensual, meses
                ));
                s.push_str(&format!("  {:<10} {:>16}\n", "Mes", "Ahorro acumulado"));
                s.push_str(&format!("  {}\n", "─".repeat(28)));
                for (mes, acum) in proyeccion {
                    s.push_str(&format!("  Mes {:<5} ${:>14.2}\n", mes, acum));
                }
            }
            TipoRegistro::Accion(a) => {
                s.push_str(&format!("  {} — {}\n", a.accion, a.categoria));
                s.push_str(&format!(
                    "  Impacto: {} {}\n",
                    a.impacto.emoji(),
                    a.impacto.nombre()
                ));
                if let Some(m) = a.monto {
                    s.push_str(&format!("  Monto: ${:.2}\n", m));
                }
                if !a.notas.is_empty() {
                    s.push_str(&format!("  Notas: {}\n", a.notas));
                }
            }
        }
        s.push_str("══════════════════════════════════════════════════════\n");
        s
    }

    /// Genera líneas CSV detalladas según el tipo de registro.
    pub fn csv_detalle(&self) -> String {
        match &self.datos {
            TipoRegistro::AnalisisDeuda {
                deuda: _,
                opciones,
                mejor_opcion: _,
            } => {
                let mut s = String::from(
                    "Opcion,Monto Mensual,Meses,Total Intereses,Total Pagado,Ahorro vs Minimo\n",
                );
                for op in opciones {
                    s.push_str(&format!(
                        "\"{}\",{:.2},{},{:.2},{:.2},{:.2}\n",
                        op.nombre.replace('"', "\"\""),
                        op.monto_mensual,
                        op.meses_para_liquidar,
                        op.total_intereses,
                        op.total_pagado,
                        op.ahorro_vs_minimo
                    ));
                }
                s
            }
            TipoRegistro::CorteBancario {
                corte,
                tasa_mensual,
                tasa_anual,
                ..
            } => {
                let mut s = String::from("Campo,Valor\n");
                s.push_str(&format!("Tarjeta,\"{}\"\n", corte.nombre_tarjeta));
                s.push_str(&format!("Fecha Corte,\"{}\"\n", corte.fecha_corte));
                s.push_str(&format!("Saldo Anterior,{:.2}\n", corte.saldo_anterior));
                s.push_str(&format!("Pago Realizado,{:.2}\n", corte.pago_realizado));
                s.push_str(&format!("Compras Periodo,{:.2}\n", corte.compras_periodo));
                s.push_str(&format!(
                    "Intereses Cobrados,{:.2}\n",
                    corte.intereses_cobrados
                ));
                s.push_str(&format!("Otros Cargos,{:.2}\n", corte.otros_cargos));
                s.push_str(&format!("Saldo al Corte,{:.2}\n", corte.saldo_al_corte));
                s.push_str(&format!("Tasa Mensual,{:.4}\n", tasa_mensual));
                s.push_str(&format!("Tasa Anual,{:.4}\n", tasa_anual));
                s
            }
            TipoRegistro::Comparacion(c) => {
                let mut s = String::from("Opcion,Costo,Beneficio\n");
                s.push_str(&format!(
                    "\"{}\",{:.2},\"{}\"\n",
                    c.opcion_a.replace('"', "\"\""),
                    c.costo_a,
                    c.beneficio_a.replace('"', "\"\"")
                ));
                s.push_str(&format!(
                    "\"{}\",{:.2},\"{}\"\n",
                    c.opcion_b.replace('"', "\"\""),
                    c.costo_b,
                    c.beneficio_b.replace('"', "\"\"")
                ));
                s
            }
            TipoRegistro::MatrizDecision(m) => {
                let mut s = String::from("Opcion");
                for c in &m.criterios {
                    s.push_str(&format!(",\"{}\" (peso {:.2})", c.nombre, c.peso));
                }
                s.push_str(",Total Ponderado\n");
                let puntuaciones = m.puntuaciones();
                for (i, (opcion, score)) in puntuaciones.iter().enumerate() {
                    s.push_str(&format!("\"{}\"", opcion.replace('"', "\"\"")));
                    for j in 0..m.criterios.len() {
                        s.push_str(&format!(",{:.1}", m.valores[i][j]));
                    }
                    s.push_str(&format!(",{:.2}\n", score));
                }
                s
            }
            TipoRegistro::ProyeccionAhorro { proyeccion, .. } => {
                let mut s = String::from("Mes,Ahorro Acumulado\n");
                for (mes, acum) in proyeccion {
                    s.push_str(&format!("{},{:.2}\n", mes, acum));
                }
                s
            }
            TipoRegistro::Accion(a) => {
                let mut s = String::from("Fecha,Accion,Categoria,Impacto,Monto,Notas\n");
                s.push_str(&format!(
                    "{},\"{}\",\"{}\",\"{}\",{},\"{}\"\n",
                    a.fecha,
                    a.accion.replace('"', "\"\""),
                    a.categoria.replace('"', "\"\""),
                    a.impacto.nombre(),
                    a.monto.map(|m| format!("{:.2}", m)).unwrap_or_default(),
                    a.notas.replace('"', "\"\""),
                ));
                s
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════
//  Almacén del Asesor (persistencia)
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AlmacenAsesor {
    pub analisis_deudas: Vec<AnalisisDeuda>,
    pub presupuesto: Presupuesto,
    pub matrices: Vec<MatrizDecision>,
    pub escenarios: Vec<Escenario>,
    pub diccionario: DiccionarioAcciones,
    pub comparaciones: Vec<ComparacionRapida>,
    #[serde(default)]
    pub registros: Vec<RegistroAsesor>,
    #[serde(default)]
    pub rastreador: RastreadorDeudas,
}

impl AlmacenAsesor {
    pub fn siguiente_id(&self) -> u64 {
        self.registros.iter().map(|r| r.id).max().unwrap_or(0) + 1
    }

    /// Directorio de exportación.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn dir_exportacion() -> PathBuf {
        let dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("omniplanner")
            .join("exports");
        fs::create_dir_all(&dir).ok();
        dir
    }

    /// Exporta TODOS los registros a un CSV resumen.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn exportar_resumen_csv(&self) -> Result<PathBuf, String> {
        let dir = Self::dir_exportacion();
        let ruta = dir.join("asesor_registros.csv");
        // BOM UTF-8 para que Excel abra bien los acentos en Windows
        let mut contenido = String::from("\u{FEFF}ID,Fecha,Hora,Tipo,Titulo,Resumen\n");
        for r in &self.registros {
            contenido.push_str(&r.csv_resumen());
            contenido.push('\n');
        }
        fs::write(&ruta, &contenido).map_err(|e| format!("Error escribiendo CSV: {}", e))?;
        Ok(ruta)
    }

    /// Exporta un registro individual a CSV detallado.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn exportar_registro_csv(&self, id: u64) -> Result<PathBuf, String> {
        let reg = self
            .registros
            .iter()
            .find(|r| r.id == id)
            .ok_or_else(|| format!("Registro #{} no encontrado", id))?;
        let dir = Self::dir_exportacion();
        let nombre_archivo = format!(
            "asesor_{}_{}.csv",
            reg.id,
            reg.titulo
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
                .collect::<String>()
                .replace(' ', "_")
                .chars()
                .take(30)
                .collect::<String>()
        );
        let ruta = dir.join(nombre_archivo);
        let mut contenido = String::from("\u{FEFF}");
        contenido.push_str(&reg.csv_detalle());
        fs::write(&ruta, &contenido).map_err(|e| format!("Error escribiendo CSV: {}", e))?;
        Ok(ruta)
    }

    /// Exporta todos los registros a un reporte de texto legible (para imprimir).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn exportar_reporte_texto(&self) -> Result<PathBuf, String> {
        let dir = Self::dir_exportacion();
        let ruta = dir.join("asesor_reporte.txt");
        let mut contenido = String::new();
        contenido.push_str("╔══════════════════════════════════════════════════════════╗\n");
        contenido.push_str("║         OMNIPLANNER — REPORTE ASESOR INTELIGENTE        ║\n");
        contenido.push_str("╚══════════════════════════════════════════════════════════╝\n");
        contenido.push_str(&format!(
            "  Total de registros: {}\n\n",
            self.registros.len()
        ));

        for r in &self.registros {
            contenido.push_str(&r.detalle_texto());
            contenido.push('\n');
        }
        fs::write(&ruta, &contenido).map_err(|e| format!("Error escribiendo reporte: {}", e))?;
        Ok(ruta)
    }

    /// Exporta un solo registro a texto.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn exportar_registro_texto(&self, id: u64) -> Result<PathBuf, String> {
        let reg = self
            .registros
            .iter()
            .find(|r| r.id == id)
            .ok_or_else(|| format!("Registro #{} no encontrado", id))?;
        let dir = Self::dir_exportacion();
        let nombre_archivo = format!(
            "asesor_{}_{}.txt",
            reg.id,
            reg.titulo
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
                .collect::<String>()
                .replace(' ', "_")
                .chars()
                .take(30)
                .collect::<String>()
        );
        let ruta = dir.join(nombre_archivo);
        fs::write(&ruta, reg.detalle_texto())
            .map_err(|e| format!("Error escribiendo reporte: {}", e))?;
        Ok(ruta)
    }

    /// Filtra registros por tipo.
    pub fn filtrar_por_tipo(&self, tipo: &str) -> Vec<&RegistroAsesor> {
        self.registros
            .iter()
            .filter(|r| r.tipo_nombre == tipo)
            .collect()
    }

    /// Filtra por etiqueta.
    pub fn filtrar_por_etiqueta(&self, etiqueta: &str) -> Vec<&RegistroAsesor> {
        let et = etiqueta.to_lowercase();
        self.registros
            .iter()
            .filter(|r| r.etiquetas.iter().any(|e| e.to_lowercase().contains(&et)))
            .collect()
    }

    /// Busca registros por texto en título o resumen.
    pub fn buscar_registros(&self, texto: &str) -> Vec<&RegistroAsesor> {
        let txt = texto.to_lowercase();
        self.registros
            .iter()
            .filter(|r| {
                r.titulo.to_lowercase().contains(&txt)
                    || r.resumen.to_lowercase().contains(&txt)
                    || r.etiquetas.iter().any(|e| e.to_lowercase().contains(&txt))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analisis_deuda_pago_total() {
        let deuda = AnalisisDeuda::nuevo("Tarjeta", 420.0, 0.03, 60.0);
        let opcion = deuda.calcular_opcion("Pago total", 420.0);
        // Con 3% mensual, mes 1: saldo 432.60, se pagan 420, queda 12.60
        // Mes 2: saldo 12.98, se pagan 12.98 → 2 meses
        assert_eq!(opcion.meses_para_liquidar, 2);
        assert!(opcion.total_intereses < 15.0);
    }

    #[test]
    fn test_analisis_deuda_pago_minimo() {
        let deuda = AnalisisDeuda::nuevo("Tarjeta", 420.0, 0.03, 60.0);
        let opcion = deuda.calcular_opcion("Pago mínimo", 60.0);
        assert!(opcion.meses_para_liquidar > 5);
        assert!(opcion.total_intereses > 10.0);
    }

    #[test]
    fn test_deuda_comparar_opciones() {
        let deuda = AnalisisDeuda::nuevo("Tarjeta", 420.0, 0.03, 60.0);
        let opciones = deuda.comparar_opciones(&[
            ("Pago mínimo ($60)", 60.0),
            ("Pago parcial ($150)", 150.0),
            ("Pago total ($420)", 420.0),
        ]);
        assert_eq!(opciones.len(), 3);
        let mejor = AnalisisDeuda::mejor_opcion(&opciones).unwrap();
        assert_eq!(mejor, 2); // pago total siempre es mejor
    }

    #[test]
    fn test_presupuesto_balance() {
        let mut pres = Presupuesto::default();
        pres.ingresos.push(Movimiento {
            concepto: "Salario".into(),
            monto: 2000.0,
            frecuencia: FrecuenciaPago::Mensual,
            categoria: "Trabajo".into(),
            fijo: true,
        });
        pres.gastos.push(Movimiento {
            concepto: "Renta".into(),
            monto: 500.0,
            frecuencia: FrecuenciaPago::Mensual,
            categoria: "Vivienda".into(),
            fijo: true,
        });
        pres.gastos.push(Movimiento {
            concepto: "Comida".into(),
            monto: 100.0,
            frecuencia: FrecuenciaPago::Semanal,
            categoria: "Alimentación".into(),
            fijo: false,
        });
        assert!((pres.ingreso_mensual() - 2000.0).abs() < 0.01);
        assert!(pres.balance_mensual() > 0.0);
        assert!(pres.balance_mensual() < 2000.0);
    }

    #[test]
    fn test_matriz_decision() {
        let mut m = MatrizDecision::nueva("¿Qué laptop comprar?", "2026-04-11");
        m.agregar_criterio("Precio", 0.4);
        m.agregar_criterio("Rendimiento", 0.3);
        m.agregar_criterio("Portabilidad", 0.3);

        m.agregar_opcion("Laptop A");
        m.agregar_opcion("Laptop B");

        m.set_valor(0, 0, 8.0); // A: buen precio
        m.set_valor(0, 1, 6.0); // A: rendimiento medio
        m.set_valor(0, 2, 7.0); // A: portátil

        m.set_valor(1, 0, 5.0); // B: cara
        m.set_valor(1, 1, 9.0); // B: alto rendimiento
        m.set_valor(1, 2, 4.0); // B: pesada

        let (mejor, _score) = m.mejor_opcion().unwrap();
        assert!(!mejor.is_empty());
    }

    #[test]
    fn test_comparacion_rapida() {
        let comp = ComparacionRapida::nueva(
            "Tarjeta",
            "Pago mínimo",
            500.0,
            "Quedan $90 en la cuenta",
            "Pago total",
            432.0,
            "Sin intereses",
        );
        assert!(comp.diferencia > 0.0);
        assert!(comp.recomendacion.contains("Pago total"));
    }

    #[test]
    fn test_corte_bancario_calcula_tasa() {
        let mut corte = CorteBancario::nuevo("Visa Test");
        corte.saldo_anterior = 1000.0;
        corte.pago_realizado = 60.0;
        corte.compras_periodo = 0.0;
        // 940 * 3% = 28.20 de interés
        corte.intereses_cobrados = 28.20;
        corte.otros_cargos = 0.0;
        corte.saldo_al_corte = 1000.0 - 60.0 + 28.20; // 968.20
        corte.pago_minimo = 60.0;
        corte.pago_no_intereses = 968.20;

        let analisis = corte.analizar();

        // Tasa mensual: 28.20 / 940 = 0.03 (3%)
        assert!((analisis.tasa_mensual_calculada - 0.03).abs() < 0.001);
        assert!((analisis.tasa_anual_calculada - 0.36).abs() < 0.01);
        assert!((analisis.saldo_que_genero_interes - 940.0).abs() < 0.01);
        // Pago: $60 - de los cuales $28.20 fueron a interés
        assert!((analisis.pago_a_interes - 28.20).abs() < 0.01);
        assert!((analisis.pago_a_capital - 31.80).abs() < 0.01);
        // Diferencia debería ser ~0 (datos consistentes)
        assert!(analisis.diferencia_vs_real < 0.01);
        // Deuda generada con el saldo nuevo
        assert!((analisis.deuda.saldo_total - 968.20).abs() < 0.01);
    }

    #[test]
    fn test_corte_bancario_pago_total_sin_interes() {
        let mut corte = CorteBancario::nuevo("Mastercard");
        corte.saldo_anterior = 500.0;
        corte.pago_realizado = 500.0;
        corte.compras_periodo = 200.0;
        corte.intereses_cobrados = 0.0;
        corte.otros_cargos = 0.0;
        corte.saldo_al_corte = 200.0;
        corte.pago_minimo = 20.0;
        corte.pago_no_intereses = 200.0;

        let analisis = corte.analizar();

        // Si pagó todo, no debería haber interés → tasa 0
        assert!((analisis.tasa_mensual_calculada - 0.0).abs() < 0.001);
        assert!((analisis.pago_a_interes - 0.0).abs() < 0.01);
        assert!((analisis.pago_a_capital - 500.0).abs() < 0.01);
    }

    // ── Fase 3: DecisionPago / EstadoDeudaUi ──

    fn deuda_tarjeta() -> DeudaRastreada {
        let mut d = DeudaRastreada::nueva("Visa", 24.0, 50.0);
        d.registrar_mes("Ene", 1_000.0, 50.0, 0.0);
        d
    }

    #[test]
    fn decision_pago_bloquea_negativos() {
        let d = deuda_tarjeta();
        assert!(matches!(
            d.evaluar_pago_mes(-1.0, 0.0, 100.0),
            DecisionPago::Bloquear(_)
        ));
        assert!(matches!(
            d.evaluar_pago_mes(10.0, -5.0, 100.0),
            DecisionPago::Bloquear(_)
        ));
        assert!(matches!(
            d.evaluar_pago_mes(10.0, 0.0, -1.0),
            DecisionPago::Bloquear(_)
        ));
    }

    #[test]
    fn decision_pago_cero_pide_confirmacion() {
        let d = deuda_tarjeta();
        let dec = d.evaluar_pago_mes(0.0, 0.0, 1_000.0);
        assert!(matches!(dec, DecisionPago::PedirDobleConfirmacion(_)));
        assert!(dec.requiere_confirmacion());
    }

    #[test]
    fn decision_pago_mitad_exigible_pide_confirmacion() {
        let d = deuda_tarjeta();
        // Exigible ~ 50, pago 10 = 20% → doble confirmación
        let dec = d.evaluar_pago_mes(10.0, 0.0, 1_000.0);
        assert!(matches!(dec, DecisionPago::PedirDobleConfirmacion(_)));
    }

    #[test]
    fn decision_pago_cubre_exigible_pero_no_intereses_avisa() {
        // Tarjeta con saldo alto y pago mínimo alto; el pago cubre exigible
        // pero no alcanza a cubrir intereses del saldo inicial.
        let mut d = DeudaRastreada::nueva("Visa", 30.0, 200.0);
        d.registrar_mes("Ene", 10_000.0, 200.0, 0.0);
        let dec = d.evaluar_pago_mes(200.0, 0.0, 10_000.0);
        assert!(matches!(dec, DecisionPago::AceptarConAviso(_)));
        assert!(dec.es_aceptado());
    }

    #[test]
    fn decision_pago_normal_acepta() {
        let mut d = DeudaRastreada::nueva("Préstamo", 6.0, 500.0);
        d.registrar_mes("Ene", 10_000.0, 500.0, 0.0);
        let dec = d.evaluar_pago_mes(500.0, 0.0, 9_500.0);
        assert_eq!(dec, DecisionPago::Aceptar);
        assert!(dec.es_aceptado());
        assert!(!dec.requiere_confirmacion());
        assert!(!dec.esta_bloqueado());
    }

    #[test]
    fn estado_ui_liquidada() {
        let mut d = DeudaRastreada::nueva("Visa", 24.0, 100.0);
        d.registrar_mes("Ene", 100.0, 100.0, 0.0);
        assert!(!d.activa);
        assert_eq!(d.estado_ui(), EstadoDeudaUi::Liquidada);
    }

    #[test]
    fn estado_ui_trampa_de_intereses() {
        let mut d = DeudaRastreada::nueva("Visa", 30.0, 20.0);
        d.registrar_mes("Ene", 10_000.0, 20.0, 0.0);
        // interés mensual ~ $250, pago_minimo $20 → trampa
        assert_eq!(d.estado_ui(), EstadoDeudaUi::EnTrampaIntereses);
    }

    #[test]
    fn estado_ui_al_dia() {
        let mut d = DeudaRastreada::nueva("Préstamo", 6.0, 500.0);
        d.registrar_mes("Ene", 10_000.0, 500.0, 0.0);
        assert_eq!(d.estado_ui(), EstadoDeudaUi::AlDia);
    }

    #[test]
    fn decision_pago_excesivo_pide_confirmacion() {
        let d = deuda_tarjeta();
        let dec = d.evaluar_pago_mes(20_000.0, 0.0, 1_000.0);
        assert!(matches!(dec, DecisionPago::PedirDobleConfirmacion(_)));
    }

    #[test]
    fn decision_pago_nan_bloquea() {
        let d = deuda_tarjeta();
        assert!(matches!(
            d.evaluar_pago_mes(f64::NAN, 0.0, 100.0),
            DecisionPago::Bloquear(_)
        ));
    }

    #[test]
    fn decision_pago_mensaje_no_vacio_cuando_no_aceptar() {
        let d = deuda_tarjeta();
        let dec = d.evaluar_pago_mes(0.0, 0.0, 1_000.0);
        assert!(!dec.mensaje().is_empty());
        assert!(DecisionPago::Aceptar.mensaje().is_empty());
    }

    #[test]
    fn estado_ui_vencida_detecta_atraso() {
        let mut d = DeudaRastreada::nueva("Hipoteca", 5.0, 500.0);
        d.obligatoria = true;
        d.registrar_mes("Ene", 100_000.0, 100.0, 0.0);
        let estado = d.estado_ui();
        assert!(matches!(estado, EstadoDeudaUi::Vencida { .. }));
        if let EstadoDeudaUi::Vencida { monto_vencido } = estado {
            assert!(monto_vencido > 300.0);
        }
    }

    #[test]
    fn estado_ui_badge_y_etiqueta() {
        assert_eq!(EstadoDeudaUi::Liquidada.badge(), "✅");
        assert_eq!(EstadoDeudaUi::AlDia.etiqueta(), "Al día");
        assert_eq!(EstadoDeudaUi::EnTrampaIntereses.badge(), "🔴");
        assert_eq!(
            EstadoDeudaUi::Vencida {
                monto_vencido: 100.0
            }
            .etiqueta(),
            "Vencida"
        );
    }

    // ──────────────────────────────────────────────────────────
    //  Tests — Análisis de ahorro por pago extra
    // ──────────────────────────────────────────────────────────

    fn deuda_con_saldo(nombre: &str, saldo: f64, tasa_anual: f64, pago_min: f64) -> DeudaRastreada {
        let mut d = DeudaRastreada::nueva(nombre, tasa_anual, pago_min);
        // Sembrar un mes de historial para que saldo_actual() devuelva el saldo deseado.
        d.historial.push(MesPago {
            mes: "2026-01".into(),
            saldo_inicio: saldo,
            pago: 0.0,
            pago_escrow: 0.0,
            nuevos_cargos: 0.0,
            intereses: 0.0,
            saldo_final: saldo,
        });
        d.activa = saldo > 0.01;
        d
    }

    #[test]
    fn simular_liquidacion_basica() {
        let d = deuda_con_saldo("Visa", 1000.0, 24.0, 100.0);
        let sim = d.simular_liquidacion(100.0).expect("debería liquidar");
        assert!(sim.meses > 0 && sim.meses < 600);
        assert!(sim.intereses_totales > 0.0);
    }

    #[test]
    fn simular_liquidacion_pago_insuficiente_no_converge() {
        // 24% anual = 2% mensual. Saldo 1000 ⇒ intereses mes 1 = 20. Pago 15 no alcanza.
        let d = deuda_con_saldo("Visa", 1000.0, 24.0, 15.0);
        assert!(d.simular_liquidacion(15.0).is_none());
    }

    #[test]
    fn ahorro_por_pago_extra_reduce_intereses_y_meses() {
        // 24% anual ⇒ 2% mensual. Saldo 3000 ⇒ intereses mes 1 = 60.
        // pago_min 100 cubre intereses y deja 40 al principal.
        let d = deuda_con_saldo("Visa", 3000.0, 24.0, 100.0);
        let a = d
            .ahorro_por_pago_extra(100.0)
            .expect("debería haber ahorro");
        assert!(a.meses_ahorrados > 0);
        assert!(a.intereses_ahorrados > 0.0);
        assert!(a.intereses_con_extra < a.intereses_base);
        assert!(a.meses_con_extra < a.meses_base);
        assert!(a.porcentaje_intereses_ahorrados() > 0.0);
    }

    #[test]
    fn ahorro_por_pago_extra_sin_saldo_devuelve_none() {
        let d = deuda_con_saldo("Visa", 0.0, 24.0, 50.0);
        assert!(d.ahorro_por_pago_extra(100.0).is_none());
    }

    #[test]
    fn ahorro_por_pago_extra_con_extra_cero_devuelve_none() {
        let d = deuda_con_saldo("Visa", 1000.0, 24.0, 100.0);
        assert!(d.ahorro_por_pago_extra(0.0).is_none());
    }

    #[test]
    fn mejor_destino_pago_extra_prioriza_mayor_ahorro() {
        let mut r = RastreadorDeudas::default();
        // Hipoteca: tasa baja, pago_min cubre bien (saldo 5000 @ 6% ⇒ interés mes1 = 25).
        r.agregar_deuda(deuda_con_saldo("Hipoteca", 5000.0, 6.0, 100.0));
        // Tarjeta: tasa alta (saldo 5000 @ 24% ⇒ interés mes1 = 100). pago_min 120 apenas reduce.
        r.agregar_deuda(deuda_con_saldo("Tarjeta", 5000.0, 24.0, 120.0));

        let rec = r
            .mejor_destino_pago_extra(100.0)
            .expect("debería recomendar");
        assert_eq!(rec.nombre_deuda, "Tarjeta");
        assert_eq!(rec.ranking.len(), 2);
        // El primer elemento del ranking coincide con la recomendación.
        assert_eq!(rec.ranking[0].0, "Tarjeta");
        // El ahorro de Tarjeta > ahorro de Hipoteca.
        assert!(rec.ranking[0].1.intereses_ahorrados > rec.ranking[1].1.intereses_ahorrados);
    }

    #[test]
    fn mejor_destino_sin_deudas_devuelve_none() {
        let r = RastreadorDeudas::default();
        assert!(r.mejor_destino_pago_extra(100.0).is_none());
    }

    #[test]
    fn eficiencia_por_dolar_extra_es_no_negativa() {
        let d = deuda_con_saldo("Visa", 2000.0, 18.0, 80.0);
        let a = d.ahorro_por_pago_extra(50.0).expect("ahorro");
        assert!(a.eficiencia_por_dolar_extra() >= 0.0);
    }

    // ──────────────────────────────────────────────────────────
    //  Tests — Planificador editable de libertad financiera
    // ──────────────────────────────────────────────────────────

    fn rastreador_con_dos_tarjetas() -> RastreadorDeudas {
        let mut r = RastreadorDeudas::default();
        r.agregar_deuda(deuda_con_saldo("Visa", 2000.0, 24.0, 60.0));
        r.agregar_deuda(deuda_con_saldo("Amex", 1000.0, 18.0, 40.0));
        r
    }

    #[test]
    fn simular_libertad_delegado_sigue_funcionando() {
        let r = rastreador_con_dos_tarjetas();
        let sim = r.simular_libertad(200.0, false);
        assert!(!sim.meses.is_empty());
        assert_eq!(sim.estrategia, "Avalancha");
        assert!(sim.total_intereses > 0.0);
    }

    #[test]
    fn estrategia_pesos_reparte_segun_peso() {
        let r = rastreador_con_dos_tarjetas();
        let estrategia = EstrategiaLibertad::Pesos(vec![
            ("Visa".into(), 3.0),
            ("Amex".into(), 1.0),
        ]);
        let sim = r.simular_libertad_editado(200.0, &estrategia, &[]);
        assert_eq!(sim.estrategia, "Pesos personalizados");
        // Primer mes: Visa debería recibir bastante más que Amex.
        let mes1 = &sim.meses[0];
        let pago_visa = mes1.pagos.iter().find(|(n, _)| n == "Visa").unwrap().1;
        let pago_amex = mes1.pagos.iter().find(|(n, _)| n == "Amex").unwrap().1;
        assert!(pago_visa > pago_amex);
    }

    #[test]
    fn pesos_normalizados_fallback_si_todos_cero() {
        let estrategia = EstrategiaLibertad::pesos_normalizados(vec![
            ("A".into(), 0.0),
            ("B".into(), 0.0),
        ]);
        assert_eq!(estrategia, EstrategiaLibertad::Avalancha);
    }

    #[test]
    fn pesos_normalizados_suma_uno() {
        let estrategia = EstrategiaLibertad::pesos_normalizados(vec![
            ("A".into(), 3.0),
            ("B".into(), 1.0),
        ]);
        match estrategia {
            EstrategiaLibertad::Pesos(p) => {
                let suma: f64 = p.iter().map(|(_, v)| v).sum();
                assert!((suma - 1.0).abs() < 1e-9);
            }
            _ => panic!("debería ser Pesos"),
        }
    }

    #[test]
    fn ajuste_manual_respeta_pago_forzado() {
        let r = rastreador_con_dos_tarjetas();
        // Forzar que en el mes 1, Visa reciba sólo 30 (menos que mínimo).
        // El resto del presupuesto debe ir a Amex.
        let ajustes = vec![AjusteMensualLibertad::nuevo(1, "Visa", 30.0)];
        let sim = r.simular_libertad_editado(
            200.0,
            &EstrategiaLibertad::Avalancha,
            &ajustes,
        );
        let mes1 = &sim.meses[0];
        let pago_visa = mes1.pagos.iter().find(|(n, _)| n == "Visa").unwrap().1;
        let pago_amex = mes1.pagos.iter().find(|(n, _)| n == "Amex").unwrap().1;
        assert!((pago_visa - 30.0).abs() < 0.01);
        // El ajuste liberó recursos que deben haberse redirigido a Amex.
        assert!(pago_amex > 40.0);
    }

    #[test]
    fn ajuste_manual_solo_afecta_su_mes() {
        let r = rastreador_con_dos_tarjetas();
        let ajustes = vec![AjusteMensualLibertad::nuevo(1, "Visa", 30.0)];
        let sim = r.simular_libertad_editado(
            200.0,
            &EstrategiaLibertad::Avalancha,
            &ajustes,
        );
        // En el mes 2 ya no hay ajuste → Visa debe recibir al menos su mínimo.
        if sim.meses.len() >= 2 {
            let mes2 = &sim.meses[1];
            let pago_visa = mes2
                .pagos
                .iter()
                .find(|(n, _)| n == "Visa")
                .map(|(_, p)| *p)
                .unwrap_or(0.0);
            // En avalancha Visa tiene tasa más alta → recibe extra, >= mínimo 60.
            assert!(pago_visa >= 60.0 - 0.01);
        }
    }

    #[test]
    fn ajuste_manual_redistribuye_cuando_sobra() {
        let r = rastreador_con_dos_tarjetas();
        // Forzar Visa a un pago ENORME acotado por saldo: el simulador debe
        // tomar como máximo el saldo y no dejar el presupuesto en negativo.
        let ajustes = vec![AjusteMensualLibertad::nuevo(1, "Visa", 10_000.0)];
        let sim = r.simular_libertad_editado(
            200.0,
            &EstrategiaLibertad::Avalancha,
            &ajustes,
        );
        let mes1 = &sim.meses[0];
        let pago_visa = mes1.pagos.iter().find(|(n, _)| n == "Visa").unwrap().1;
        // El pago a Visa está acotado por el presupuesto disponible (≤ 200).
        assert!(pago_visa <= 200.0 + 0.01);
        assert!(mes1.sobrante >= 0.0);
    }

    #[test]
    fn comparar_planes_detecta_diferencias() {
        let r = rastreador_con_dos_tarjetas();
        let base = r.simular_libertad(200.0, false);
        let alt = r.simular_libertad_editado(
            200.0,
            &EstrategiaLibertad::BolaNieve,
            &[],
        );
        let cmp = RastreadorDeudas::comparar_planes(&base, &alt);
        assert_eq!(cmp.meses_base, base.meses.len());
        assert_eq!(cmp.meses_alternativa, alt.meses.len());
        assert!((cmp.intereses_base - base.total_intereses).abs() < 1e-6);
    }

    #[test]
    fn plan_sin_deudas_devuelve_vacio() {
        let r = RastreadorDeudas::default();
        let sim = r.simular_libertad_editado(500.0, &EstrategiaLibertad::Avalancha, &[]);
        assert!(sim.meses.is_empty());
    }
}
