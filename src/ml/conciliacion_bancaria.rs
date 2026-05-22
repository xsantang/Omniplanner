//! Conciliación Bancaria — reconciliación de saldos contables vs bancarios.
//!
//! Permite registrar cuentas bancarias, tarjetas de crédito y préstamos con
//! sus respectivas tasas; importar movimientos de extractos; y ejecutar la
//! conciliación para detectar diferencias, partidas en tránsito y errores.
//!
//! # Estructura
//! ```text
//! AlmacenConciliacion
//! ├── cuentas: Vec<CuentaBancaria>          (corriente, ahorro)
//! ├── tarjetas: Vec<TarjetaCredito>         (con tasa mensual y cupo)
//! ├── prestamos: Vec<PrestamoRegistrado>    (con tasa y tabla de amortización)
//! └── conciliaciones: Vec<ConciliacionMes>  (resultado de cada cierre mensual)
//! ```

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ══════════════════════════════════════════════════════════════════════════════
//  Tipos de cuenta
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TipoCuenta {
    CuentaCorriente,
    CuentaAhorro,
    TarjetaCredito,
    Prestamo,
    Otro(String),
}

impl TipoCuenta {
    pub fn nombre(&self) -> &str {
        match self {
            TipoCuenta::CuentaCorriente => "Cuenta Corriente",
            TipoCuenta::CuentaAhorro => "Cuenta de Ahorro",
            TipoCuenta::TarjetaCredito => "Tarjeta de Crédito",
            TipoCuenta::Prestamo => "Préstamo",
            TipoCuenta::Otro(n) => n.as_str(),
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Movimiento de extracto bancario
// ══════════════════════════════════════════════════════════════════════════════

/// Un movimiento tal como aparece en el extracto del banco.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovimientoExtracto {
    pub id: String,
    pub fecha: NaiveDate,
    pub descripcion: String,
    /// Positivo = crédito/ingreso al banco; negativo = débito/salida.
    pub monto: f64,
    /// `true` si ya fue emparejado con un movimiento contable.
    #[serde(default)]
    pub conciliado: bool,
    #[serde(default)]
    pub notas: String,
}

impl MovimientoExtracto {
    pub fn nuevo(fecha: NaiveDate, descripcion: impl Into<String>, monto: f64) -> Self {
        MovimientoExtracto {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            fecha,
            descripcion: descripcion.into(),
            monto,
            conciliado: false,
            notas: String::new(),
        }
    }
}

/// Un movimiento registrado en la contabilidad interna del usuario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovimientoContable {
    pub id: String,
    pub fecha: NaiveDate,
    pub descripcion: String,
    pub monto: f64,
    #[serde(default)]
    pub conciliado: bool,
    #[serde(default)]
    pub referencia_extracto: Option<String>,
    #[serde(default)]
    pub notas: String,
}

impl MovimientoContable {
    pub fn nuevo(fecha: NaiveDate, descripcion: impl Into<String>, monto: f64) -> Self {
        MovimientoContable {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            fecha,
            descripcion: descripcion.into(),
            monto,
            conciliado: false,
            referencia_extracto: None,
            notas: String::new(),
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Cuenta Bancaria (corriente / ahorro)
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuentaBancaria {
    pub id: String,
    pub banco: String,
    pub nombre: String,
    pub tipo: TipoCuenta,
    /// Tasa de rendimiento anual (0.0 si no aplica). Ej: 0.04 = 4 % anual.
    pub tasa_rendimiento_anual: f64,
    /// Saldo según libros contables.
    pub saldo_contable: f64,
    /// Saldo según el último extracto bancario.
    pub saldo_extracto: f64,
    pub movimientos_contables: Vec<MovimientoContable>,
    pub movimientos_extracto: Vec<MovimientoExtracto>,
    #[serde(default)]
    pub activa: bool,
}

impl CuentaBancaria {
    pub fn nueva(
        banco: impl Into<String>,
        nombre: impl Into<String>,
        tipo: TipoCuenta,
        saldo_inicial: f64,
    ) -> Self {
        CuentaBancaria {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            banco: banco.into(),
            nombre: nombre.into(),
            tipo,
            tasa_rendimiento_anual: 0.0,
            saldo_contable: saldo_inicial,
            saldo_extracto: saldo_inicial,
            movimientos_contables: Vec::new(),
            movimientos_extracto: Vec::new(),
            activa: true,
        }
    }

    /// Registra un movimiento contable y actualiza el saldo contable.
    pub fn registrar_contable(&mut self, mov: MovimientoContable) {
        self.saldo_contable += mov.monto;
        self.movimientos_contables.push(mov);
    }

    /// Importa un movimiento del extracto y actualiza el saldo del extracto.
    pub fn importar_extracto(&mut self, mov: MovimientoExtracto) {
        self.saldo_extracto += mov.monto;
        self.movimientos_extracto.push(mov);
    }

    /// Diferencia = saldo_extracto - saldo_contable.
    /// Valor 0 = conciliado correctamente.
    pub fn diferencia(&self) -> f64 {
        self.saldo_extracto - self.saldo_contable
    }

    /// Movimientos contables aún no conciliados.
    pub fn pendientes_contables(&self) -> Vec<&MovimientoContable> {
        self.movimientos_contables
            .iter()
            .filter(|m| !m.conciliado)
            .collect()
    }

    /// Movimientos del extracto aún no conciliados.
    pub fn pendientes_extracto(&self) -> Vec<&MovimientoExtracto> {
        self.movimientos_extracto
            .iter()
            .filter(|m| !m.conciliado)
            .collect()
    }

    /// Empareja un movimiento contable con uno del extracto por sus IDs.
    /// Retorna `true` si ambos existían y se marcaron como conciliados.
    pub fn emparejar(&mut self, id_contable: &str, id_extracto: &str) -> bool {
        let c = self
            .movimientos_contables
            .iter_mut()
            .find(|m| m.id == id_contable);
        let e = self
            .movimientos_extracto
            .iter_mut()
            .find(|m| m.id == id_extracto);
        match (c, e) {
            (Some(mc), Some(me)) => {
                mc.conciliado = true;
                mc.referencia_extracto = Some(me.id.clone());
                me.conciliado = true;
                true
            }
            _ => false,
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Tarjeta de Crédito
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TarjetaCredito {
    pub id: String,
    pub banco: String,
    pub nombre: String,
    /// Cupo total otorgado.
    pub cupo_total: f64,
    /// Saldo utilizado actualmente (deuda).
    pub saldo_utilizado: f64,
    /// Tasa de interés mensual efectiva. Ej: 0.0215 = 2.15 % mensual.
    pub tasa_interes_mensual: f64,
    /// Tasa anual efectiva (calculada a partir de la mensual).
    pub tasa_interes_anual: f64,
    /// Porcentaje de pago mínimo sobre el saldo. Ej: 0.05 = 5 %.
    pub porcentaje_pago_minimo: f64,
    /// Día de corte del mes (1–31).
    pub dia_corte: u8,
    /// Día de pago límite (1–31).
    pub dia_pago: u8,
    pub movimientos_contables: Vec<MovimientoContable>,
    pub movimientos_extracto: Vec<MovimientoExtracto>,
    #[serde(default)]
    pub activa: bool,
}

impl TarjetaCredito {
    pub fn nueva(
        banco: impl Into<String>,
        nombre: impl Into<String>,
        cupo_total: f64,
        tasa_mensual: f64,
    ) -> Self {
        let anual = (1.0 + tasa_mensual).powi(12) - 1.0;
        TarjetaCredito {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            banco: banco.into(),
            nombre: nombre.into(),
            cupo_total,
            saldo_utilizado: 0.0,
            tasa_interes_mensual: tasa_mensual,
            tasa_interes_anual: anual,
            porcentaje_pago_minimo: 0.05,
            dia_corte: 15,
            dia_pago: 5,
            movimientos_contables: Vec::new(),
            movimientos_extracto: Vec::new(),
            activa: true,
        }
    }

    /// Cupo disponible.
    pub fn cupo_disponible(&self) -> f64 {
        self.cupo_total - self.saldo_utilizado
    }

    /// Porcentaje de utilización del cupo (0.0–1.0).
    pub fn utilizacion(&self) -> f64 {
        if self.cupo_total == 0.0 {
            return 0.0;
        }
        self.saldo_utilizado / self.cupo_total
    }

    /// Pago mínimo del mes.
    pub fn pago_minimo(&self) -> f64 {
        self.saldo_utilizado * self.porcentaje_pago_minimo
    }

    /// Interés generado en un mes sobre el saldo actual.
    pub fn interes_mensual(&self) -> f64 {
        self.saldo_utilizado * self.tasa_interes_mensual
    }

    /// Registra un cargo (aumenta saldo).
    pub fn registrar_cargo(&mut self, mov: MovimientoContable) {
        self.saldo_utilizado += mov.monto.abs();
        self.movimientos_contables.push(mov);
    }

    /// Registra un pago (reduce saldo).
    pub fn registrar_pago(&mut self, monto: f64) {
        self.saldo_utilizado = (self.saldo_utilizado - monto).max(0.0);
    }

    /// Importa movimiento del extracto de la tarjeta.
    pub fn importar_extracto(&mut self, mov: MovimientoExtracto) {
        self.movimientos_extracto.push(mov);
    }

    /// Diferencia entre saldo registrado y saldo del extracto.
    pub fn diferencia_extracto(&self) -> f64 {
        let total_extracto: f64 = self.movimientos_extracto.iter().map(|m| m.monto).sum();
        let total_contable: f64 = self.movimientos_contables.iter().map(|m| m.monto).sum();
        total_extracto - total_contable
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Préstamo Registrado (con tabla de amortización)
// ══════════════════════════════════════════════════════════════════════════════

/// Una cuota de amortización.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuotaAmortizacion {
    pub numero: u32,
    pub fecha_vencimiento: NaiveDate,
    pub cuota_total: f64,
    pub capital: f64,
    pub interes: f64,
    pub saldo_restante: f64,
    pub pagada: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrestamoRegistrado {
    pub id: String,
    pub entidad: String,
    pub nombre: String,
    /// Capital original del préstamo.
    pub capital_original: f64,
    /// Saldo pendiente actual.
    pub saldo_pendiente: f64,
    /// Tasa de interés mensual efectiva. Ej: 0.018 = 1.8 % mensual.
    pub tasa_mensual: f64,
    /// Tasa efectiva anual (calculada).
    pub tasa_anual: f64,
    /// Número de cuotas totales.
    pub cuotas_totales: u32,
    /// Cuotas ya pagadas.
    pub cuotas_pagadas: u32,
    pub fecha_inicio: NaiveDate,
    pub tabla_amortizacion: Vec<CuotaAmortizacion>,
    #[serde(default)]
    pub activo: bool,
}

impl PrestamoRegistrado {
    /// Crea el préstamo y genera automáticamente la tabla de amortización
    /// usando el método francés (cuota fija).
    pub fn nuevo(
        entidad: impl Into<String>,
        nombre: impl Into<String>,
        capital: f64,
        tasa_mensual: f64,
        cuotas: u32,
        fecha_inicio: NaiveDate,
    ) -> Self {
        let tasa_anual = (1.0 + tasa_mensual).powi(12) - 1.0;
        let tabla = Self::generar_tabla(capital, tasa_mensual, cuotas, fecha_inicio);
        PrestamoRegistrado {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            entidad: entidad.into(),
            nombre: nombre.into(),
            capital_original: capital,
            saldo_pendiente: capital,
            tasa_mensual,
            tasa_anual,
            cuotas_totales: cuotas,
            cuotas_pagadas: 0,
            fecha_inicio,
            tabla_amortizacion: tabla,
            activo: true,
        }
    }

    /// Genera tabla de amortización francesa (cuota constante).
    fn generar_tabla(
        capital: f64,
        tasa: f64,
        cuotas: u32,
        inicio: NaiveDate,
    ) -> Vec<CuotaAmortizacion> {
        if cuotas == 0 || tasa <= 0.0 {
            return Vec::new();
        }
        // Cuota fija = C * i / (1 - (1+i)^-n)
        let cuota_fija = capital * tasa / (1.0 - (1.0 + tasa).powi(-(cuotas as i32)));
        let mut saldo = capital;
        let mut tabla = Vec::with_capacity(cuotas as usize);

        for n in 1..=cuotas {
            let interes = saldo * tasa;
            let amort = cuota_fija - interes;
            saldo = (saldo - amort).max(0.0);
            // Fecha: mes n desde inicio (month es base-1)
            let meses_offset = inicio.month() as i32 + n as i32 - 1;
            let anio = inicio.year() + (meses_offset - 1) / 12;
            let mes_real = ((meses_offset - 1) % 12) as u32 + 1;
            let fecha = NaiveDate::from_ymd_opt(anio, mes_real, inicio.day()).unwrap_or(inicio);
            tabla.push(CuotaAmortizacion {
                numero: n,
                fecha_vencimiento: fecha,
                cuota_total: cuota_fija,
                capital: amort,
                interes,
                saldo_restante: saldo,
                pagada: false,
            });
        }
        tabla
    }

    /// Marca como pagada la próxima cuota pendiente.
    pub fn registrar_pago_cuota(&mut self) -> Option<&CuotaAmortizacion> {
        if let Some(cuota) = self.tabla_amortizacion.iter_mut().find(|c| !c.pagada) {
            cuota.pagada = true;
            self.cuotas_pagadas += 1;
            self.saldo_pendiente = cuota.saldo_restante;
            // retornar referencia no es posible después de mutable borrow; devolvemos None
            // el llamador puede consultar tabla_amortizacion[cuotas_pagadas-1]
        }
        self.tabla_amortizacion
            .get((self.cuotas_pagadas as usize).saturating_sub(1))
    }

    /// Cuotas restantes por pagar.
    pub fn cuotas_restantes(&self) -> u32 {
        self.cuotas_totales - self.cuotas_pagadas
    }

    /// Total de intereses que quedan por pagar.
    pub fn intereses_futuros(&self) -> f64 {
        self.tabla_amortizacion
            .iter()
            .filter(|c| !c.pagada)
            .map(|c| c.interes)
            .sum()
    }

    /// Total pagado hasta ahora (capital + intereses).
    pub fn total_pagado(&self) -> f64 {
        self.tabla_amortizacion
            .iter()
            .filter(|c| c.pagada)
            .map(|c| c.cuota_total)
            .sum()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Resultado de conciliación mensual
// ══════════════════════════════════════════════════════════════════════════════

/// Partida en tránsito: movimiento que aparece en uno de los dos lados
/// pero no en el otro al corte del mes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartidaTransito {
    pub descripcion: String,
    pub monto: f64,
    /// "contable" si falta en extracto; "extracto" si falta en contabilidad.
    pub origen: String,
}

/// Resultado completo de la conciliación de una cuenta para un mes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConciliacionMes {
    pub id_cuenta: String,
    pub tipo_cuenta: String,
    pub anio: i32,
    pub mes: u32,
    pub saldo_contable_inicio: f64,
    pub saldo_extracto_inicio: f64,
    pub saldo_contable_fin: f64,
    pub saldo_extracto_fin: f64,
    pub partidas_en_transito: Vec<PartidaTransito>,
    /// Diferencia no explicada (errores contables o bancarios).
    pub diferencia_inexplicada: f64,
    pub conciliado: bool,
    pub fecha_cierre: NaiveDate,
}

impl ConciliacionMes {
    /// Diferencia total = saldo_extracto_fin - saldo_contable_fin.
    pub fn diferencia_total(&self) -> f64 {
        self.saldo_extracto_fin - self.saldo_contable_fin
    }

    /// Total de partidas en tránsito.
    pub fn total_transito(&self) -> f64 {
        self.partidas_en_transito.iter().map(|p| p.monto).sum()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Ratios bancarios / de endeudamiento
// ══════════════════════════════════════════════════════════════════════════════

/// Métricas derivadas del estado de todas las cuentas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatiosBancarios {
    /// Suma de saldos en cuentas corrientes y ahorro.
    pub liquidez_total: f64,
    /// Suma de deudas en tarjetas.
    pub deuda_tarjetas: f64,
    /// Suma de saldos pendientes en préstamos.
    pub deuda_prestamos: f64,
    /// Deuda total (tarjetas + préstamos).
    pub deuda_total: f64,
    /// Ratio deuda/liquidez. >1 indica que las deudas superan el efectivo disponible.
    pub ratio_deuda_liquidez: f64,
    /// Utilización promedio de tarjetas (0.0–1.0).
    pub utilizacion_promedio_tarjetas: f64,
    /// Interés mensual total que generan todas las deudas.
    pub costo_financiero_mensual: f64,
}

// ══════════════════════════════════════════════════════════════════════════════
//  Almacén principal
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlmacenConciliacion {
    pub cuentas: Vec<CuentaBancaria>,
    pub tarjetas: Vec<TarjetaCredito>,
    pub prestamos: Vec<PrestamoRegistrado>,
    pub conciliaciones: Vec<ConciliacionMes>,
}

impl AlmacenConciliacion {
    // ── Cuentas ────────────────────────────────────────────────────────────

    pub fn agregar_cuenta(&mut self, cuenta: CuentaBancaria) {
        self.cuentas.push(cuenta);
    }

    pub fn cuenta_mut(&mut self, id: &str) -> Option<&mut CuentaBancaria> {
        self.cuentas.iter_mut().find(|c| c.id == id)
    }

    pub fn cuenta(&self, id: &str) -> Option<&CuentaBancaria> {
        self.cuentas.iter().find(|c| c.id == id)
    }

    // ── Tarjetas ───────────────────────────────────────────────────────────

    pub fn agregar_tarjeta(&mut self, tarjeta: TarjetaCredito) {
        self.tarjetas.push(tarjeta);
    }

    pub fn tarjeta_mut(&mut self, id: &str) -> Option<&mut TarjetaCredito> {
        self.tarjetas.iter_mut().find(|t| t.id == id)
    }

    // ── Préstamos ──────────────────────────────────────────────────────────

    pub fn agregar_prestamo(&mut self, prestamo: PrestamoRegistrado) {
        self.prestamos.push(prestamo);
    }

    pub fn prestamo_mut(&mut self, id: &str) -> Option<&mut PrestamoRegistrado> {
        self.prestamos.iter_mut().find(|p| p.id == id)
    }

    // ── Conciliaciones ─────────────────────────────────────────────────────

    pub fn guardar_conciliacion(&mut self, c: ConciliacionMes) {
        self.conciliaciones.push(c);
    }

    /// Ejecuta la conciliación automática de una cuenta para un mes:
    /// empareja movimientos con el mismo monto y fecha, y devuelve el resultado.
    pub fn conciliar_cuenta(
        &mut self,
        id_cuenta: &str,
        anio: i32,
        mes: u32,
        fecha_cierre: NaiveDate,
    ) -> Option<ConciliacionMes> {
        let cuenta = self.cuentas.iter_mut().find(|c| c.id == id_cuenta)?;

        let saldo_cont_inicio = cuenta.saldo_contable;
        let saldo_ext_inicio = cuenta.saldo_extracto;

        // Auto-emparejar por monto+fecha exactos
        let ids_contables: Vec<String> = cuenta
            .movimientos_contables
            .iter()
            .filter(|m| !m.conciliado && m.fecha.year() == anio && m.fecha.month() == mes)
            .map(|m| m.id.clone())
            .collect();

        for id_c in &ids_contables {
            let monto_c = cuenta
                .movimientos_contables
                .iter()
                .find(|m| &m.id == id_c)
                .map(|m| (m.monto, m.fecha));
            if let Some((monto, fecha)) = monto_c {
                let id_e = cuenta
                    .movimientos_extracto
                    .iter()
                    .find(|e| !e.conciliado && (e.monto - monto).abs() < 0.01 && e.fecha == fecha)
                    .map(|e| e.id.clone());
                if let Some(ie) = id_e {
                    cuenta.emparejar(id_c, &ie);
                }
            }
        }

        // Partidas en tránsito = movimientos no conciliados del mes
        let mut partidas: Vec<PartidaTransito> = Vec::new();
        for mc in cuenta
            .movimientos_contables
            .iter()
            .filter(|m| !m.conciliado && m.fecha.year() == anio && m.fecha.month() == mes)
        {
            partidas.push(PartidaTransito {
                descripcion: mc.descripcion.clone(),
                monto: mc.monto,
                origen: "contable".to_string(),
            });
        }
        for me in cuenta
            .movimientos_extracto
            .iter()
            .filter(|m| !m.conciliado && m.fecha.year() == anio && m.fecha.month() == mes)
        {
            partidas.push(PartidaTransito {
                descripcion: me.descripcion.clone(),
                monto: me.monto,
                origen: "extracto".to_string(),
            });
        }

        let total_transito: f64 = partidas.iter().map(|p| p.monto).sum();
        let diferencia = cuenta.diferencia();
        let inexplicada = diferencia - total_transito;

        let resultado = ConciliacionMes {
            id_cuenta: id_cuenta.to_string(),
            tipo_cuenta: cuenta.tipo.nombre().to_string(),
            anio,
            mes,
            saldo_contable_inicio: saldo_cont_inicio,
            saldo_extracto_inicio: saldo_ext_inicio,
            saldo_contable_fin: cuenta.saldo_contable,
            saldo_extracto_fin: cuenta.saldo_extracto,
            partidas_en_transito: partidas,
            diferencia_inexplicada: inexplicada,
            conciliado: inexplicada.abs() < 0.01,
            fecha_cierre,
        };

        Some(resultado)
    }

    // ── Ratios ─────────────────────────────────────────────────────────────

    pub fn calcular_ratios(&self) -> RatiosBancarios {
        let liquidez_total: f64 = self
            .cuentas
            .iter()
            .filter(|c| c.activa)
            .map(|c| c.saldo_contable)
            .sum();

        let deuda_tarjetas: f64 = self
            .tarjetas
            .iter()
            .filter(|t| t.activa)
            .map(|t| t.saldo_utilizado)
            .sum();

        let deuda_prestamos: f64 = self
            .prestamos
            .iter()
            .filter(|p| p.activo)
            .map(|p| p.saldo_pendiente)
            .sum();

        let deuda_total = deuda_tarjetas + deuda_prestamos;

        let ratio_deuda_liquidez = if liquidez_total == 0.0 {
            f64::INFINITY
        } else {
            deuda_total / liquidez_total
        };

        let utilizaciones: Vec<f64> = self
            .tarjetas
            .iter()
            .filter(|t| t.activa && t.cupo_total > 0.0)
            .map(|t| t.utilizacion())
            .collect();
        let utilizacion_promedio_tarjetas = if utilizaciones.is_empty() {
            0.0
        } else {
            utilizaciones.iter().sum::<f64>() / utilizaciones.len() as f64
        };

        let costo_financiero_mensual = self
            .tarjetas
            .iter()
            .filter(|t| t.activa)
            .map(|t| t.interes_mensual())
            .sum::<f64>()
            + self
                .prestamos
                .iter()
                .filter(|p| p.activo)
                .map(|p| p.saldo_pendiente * p.tasa_mensual)
                .sum::<f64>();

        RatiosBancarios {
            liquidez_total,
            deuda_tarjetas,
            deuda_prestamos,
            deuda_total,
            ratio_deuda_liquidez,
            utilizacion_promedio_tarjetas,
            costo_financiero_mensual,
        }
    }
}
