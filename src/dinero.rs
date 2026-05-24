//! Tipo monetario de precisión exacta para OmniPlanner.
//!
//! `Dinero` envuelve `rust_decimal::Decimal` para distinguir semánticamente
//! los montos de dinero de otros `f64` del sistema (tasas de interés,
//! ratios, pesos de ML, porcentajes). El compilador rechaza mezclarlos.
//!
//! ## Regla de uso
//! - Campos de monto, saldo, pago, cobro, costo → `Dinero`
//! - Tasas de interés, porcentajes, ratios, pesos ML → `f64` (correcto)
//!
//! ## Compatibilidad JSON
//! Con `features = ["serde-float"]`, `Decimal` serializa como número JSON
//! (`123.45`), idéntico al `f64` anterior. Los datos existentes se leen
//! sin migración manual.

use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};

/// Monto monetario de precisión exacta (sin errores de redondeo binario).
///
/// Internamente es `rust_decimal::Decimal`, que usa aritmética BCD de 96 bits.
/// Equivalente al `DECIMAL(28,10)` de SQL Server o al `NUMERIC` de PostgreSQL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct Dinero(pub Decimal);

impl Dinero {
    /// Cero monetario.
    pub const CERO: Self = Self(Decimal::ZERO);

    /// Construye desde `f64`.
    /// **Solo usar en puntos de entrada** (parsing de input del usuario,
    /// lectura de Excel). En el dominio interno, operar siempre con `Dinero`.
    pub fn desde_f64(v: f64) -> Self {
        Self(Decimal::from_f64(v).unwrap_or(Decimal::ZERO))
    }

    /// Convierte a `f64` para **boundaries de salida** (Excel, display legado).
    /// Fuera de esas fronteras, no usar.
    pub fn a_f64(self) -> f64 {
        self.0.to_f64().unwrap_or(0.0)
    }

    /// Construye desde string (e.g. `"1234.56"`). Retorna `None` si inválido.
    pub fn desde_str(s: &str) -> Option<Self> {
        let limpio = s.trim().replace(',', "");
        Decimal::from_str_exact(&limpio).ok().map(Self)
    }

    /// Aplica una tasa multiplicativa expresada como `f64` (0.0–1.0+).
    ///
    /// Uso típico: `saldo.aplicar_tasa(tasa_mensual)` para interés compuesto.
    /// La conversión f64→Decimal se hace una sola vez por operación.
    pub fn aplicar_tasa(self, tasa: f64) -> Self {
        let t = Decimal::from_f64(tasa).unwrap_or(Decimal::ZERO);
        Self(self.0 * t)
    }

    /// Redondea a 2 decimales (centavos) con HALF_UP.
    pub fn redondear(self) -> Self {
        Self(self.0.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero))
    }

    pub fn es_negativo(self) -> bool {
        self.0.is_sign_negative()
    }

    pub fn es_cero(self) -> bool {
        self.0.is_zero()
    }

    pub fn es_positivo(self) -> bool {
        !self.0.is_sign_negative() && !self.0.is_zero()
    }

    pub fn abs(self) -> Self {
        Self(self.0.abs())
    }

    /// Devuelve el máximo entre `self` y `cero`.
    pub fn max_cero(self) -> Self {
        if self.es_negativo() { Self::CERO } else { self }
    }

    /// Ratio `self / otro` como `f64`. Devuelve `0.0` si `otro` es cero.
    pub fn ratio(self, otro: Self) -> f64 {
        if otro.es_cero() {
            return 0.0;
        }
        (self.0 / otro.0).to_f64().unwrap_or(0.0)
    }
}

impl fmt::Display for Dinero {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}", self.0)
    }
}

impl Add for Dinero {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign for Dinero {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl Sub for Dinero {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

impl SubAssign for Dinero {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl Neg for Dinero {
    type Output = Self;
    fn neg(self) -> Self {
        Self(-self.0)
    }
}

impl Sum for Dinero {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::CERO, |acc, x| acc + x)
    }
}

impl PartialEq<f64> for Dinero {
    fn eq(&self, other: &f64) -> bool {
        self.a_f64() == *other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suma_exacta_sin_error_flotante() {
        // 0.1 + 0.2 en f64 da 0.30000000000000004
        let a = Dinero::desde_f64(0.1);
        let b = Dinero::desde_f64(0.2);
        let suma = (a + b).redondear();
        assert_eq!(suma.a_f64(), 0.3);
    }

    #[test]
    fn interes_compuesto_acumulado() {
        // $1000 al 2% mensual durante 12 meses
        let mut saldo = Dinero::desde_f64(1000.0);
        for _ in 0..12 {
            saldo = (saldo + saldo.aplicar_tasa(0.02)).redondear();
        }
        // Resultado esperado: ~$1268.24
        let resultado = saldo.a_f64();
        assert!((resultado - 1268.24).abs() < 0.01, "saldo={}", resultado);
    }

    #[test]
    fn desde_str_parsea_correctamente() {
        assert_eq!(Dinero::desde_str("1,234.56").unwrap().a_f64(), 1234.56);
        assert_eq!(Dinero::desde_str("0.00").unwrap(), Dinero::CERO);
        assert!(Dinero::desde_str("abc").is_none());
    }

    #[test]
    fn serde_compatible_con_json_numerico() {
        let d = Dinero::desde_f64(99.99);
        let json = serde_json::to_string(&d).unwrap();
        // Debe serializar como número, no como string
        assert!(!json.contains('"'), "serializado como string: {}", json);
        let d2: Dinero = serde_json::from_str(&json).unwrap();
        assert_eq!(d, d2);
    }
}
