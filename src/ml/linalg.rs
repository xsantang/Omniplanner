//! Álgebra lineal minimal — matrices, activaciones, funciones de pérdida y RNG.
//!
//! Provee [`Matriz`] con operaciones básicas (mul, hadamard, transpuesta),
//! 6 funciones de [`Activacion`] y 2 funciones de [`Perdida`].

use serde::{Deserialize, Serialize};
use std::fmt;

// ══════════════════════════════════════════════════════════════
//  Álgebra lineal minimal para ML — matrices y vectores
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Matriz {
    pub filas: usize,
    pub cols: usize,
    pub datos: Vec<f64>,
}

impl Matriz {
    pub fn nueva(filas: usize, cols: usize) -> Self {
        Self {
            filas,
            cols,
            datos: vec![0.0; filas * cols],
        }
    }

    pub fn desde_vec(filas: usize, cols: usize, datos: Vec<f64>) -> Self {
        assert_eq!(filas * cols, datos.len(), "Dimensiones no coinciden");
        Self { filas, cols, datos }
    }

    pub fn aleatoria(filas: usize, cols: usize, rng: &mut Rng) -> Self {
        let datos: Vec<f64> = (0..filas * cols)
            .map(|_| rng.normal() * (2.0 / (filas + cols) as f64).sqrt())
            .collect();
        Self { filas, cols, datos }
    }

    pub fn identidad(n: usize) -> Self {
        let mut m = Self::nueva(n, n);
        for i in 0..n {
            m.set(i, i, 1.0);
        }
        m
    }

    #[inline]
    pub fn get(&self, f: usize, c: usize) -> f64 {
        self.datos[f * self.cols + c]
    }

    #[inline]
    pub fn set(&mut self, f: usize, c: usize, v: f64) {
        self.datos[f * self.cols + c] = v;
    }

    pub fn fila(&self, f: usize) -> Vec<f64> {
        self.datos[f * self.cols..(f + 1) * self.cols].to_vec()
    }

    pub fn columna(&self, c: usize) -> Vec<f64> {
        (0..self.filas).map(|f| self.get(f, c)).collect()
    }

    pub fn transpuesta(&self) -> Self {
        let mut t = Self::nueva(self.cols, self.filas);
        for f in 0..self.filas {
            for c in 0..self.cols {
                t.set(c, f, self.get(f, c));
            }
        }
        t
    }

    pub fn mul(&self, otra: &Matriz) -> Self {
        assert_eq!(
            self.cols, otra.filas,
            "Dimensiones incompatibles para multiplicación"
        );
        let mut res = Self::nueva(self.filas, otra.cols);
        for i in 0..self.filas {
            for j in 0..otra.cols {
                let mut sum = 0.0;
                for k in 0..self.cols {
                    sum += self.get(i, k) * otra.get(k, j);
                }
                res.set(i, j, sum);
            }
        }
        res
    }

    pub fn mul_vec(&self, v: &[f64]) -> Vec<f64> {
        assert_eq!(self.cols, v.len());
        (0..self.filas)
            .map(|i| (0..self.cols).map(|j| self.get(i, j) * v[j]).sum())
            .collect()
    }

    pub fn sumar(&self, otra: &Matriz) -> Self {
        assert_eq!(self.filas, otra.filas);
        assert_eq!(self.cols, otra.cols);
        let datos: Vec<f64> = self
            .datos
            .iter()
            .zip(&otra.datos)
            .map(|(a, b)| a + b)
            .collect();
        Self {
            filas: self.filas,
            cols: self.cols,
            datos,
        }
    }

    pub fn restar(&self, otra: &Matriz) -> Self {
        assert_eq!(self.filas, otra.filas);
        assert_eq!(self.cols, otra.cols);
        let datos: Vec<f64> = self
            .datos
            .iter()
            .zip(&otra.datos)
            .map(|(a, b)| a - b)
            .collect();
        Self {
            filas: self.filas,
            cols: self.cols,
            datos,
        }
    }

    pub fn escalar(&self, k: f64) -> Self {
        let datos: Vec<f64> = self.datos.iter().map(|x| x * k).collect();
        Self {
            filas: self.filas,
            cols: self.cols,
            datos,
        }
    }

    pub fn hadamard(&self, otra: &Matriz) -> Self {
        assert_eq!(self.filas, otra.filas);
        assert_eq!(self.cols, otra.cols);
        let datos: Vec<f64> = self
            .datos
            .iter()
            .zip(&otra.datos)
            .map(|(a, b)| a * b)
            .collect();
        Self {
            filas: self.filas,
            cols: self.cols,
            datos,
        }
    }

    pub fn aplicar<F: Fn(f64) -> f64>(&self, f: F) -> Self {
        let datos: Vec<f64> = self.datos.iter().map(|&x| f(x)).collect();
        Self {
            filas: self.filas,
            cols: self.cols,
            datos,
        }
    }

    pub fn sumar_fila(&self, fila_vec: &[f64]) -> Self {
        assert_eq!(self.cols, fila_vec.len());
        let mut res = self.clone();
        for f in 0..self.filas {
            for c in 0..self.cols {
                let v = res.get(f, c) + fila_vec[c];
                res.set(f, c, v);
            }
        }
        res
    }

    pub fn suma_columnas(&self) -> Vec<f64> {
        let mut res = vec![0.0; self.cols];
        for f in 0..self.filas {
            for c in 0..self.cols {
                res[c] += self.get(f, c);
            }
        }
        res
    }

    pub fn argmax_por_fila(&self) -> Vec<usize> {
        (0..self.filas)
            .map(|f| {
                let fila = self.fila(f);
                fila.iter()
                    .enumerate()
                    .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            })
            .collect()
    }

    pub fn max_por_region(&self, f_ini: usize, f_fin: usize, c_ini: usize, c_fin: usize) -> f64 {
        let mut mx = f64::NEG_INFINITY;
        for f in f_ini..f_fin {
            for c in c_ini..c_fin {
                let v = self.get(f, c);
                if v > mx {
                    mx = v;
                }
            }
        }
        mx
    }
}

impl fmt::Display for Matriz {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in 0..self.filas {
            let vals: Vec<String> = (0..self.cols)
                .map(|j| format!("{:8.4}", self.get(i, j)))
                .collect();
            writeln!(f, "│ {} │", vals.join(" "))?;
        }
        Ok(())
    }
}

// ══════════════════════════════════════════════════════════════
//  Funciones de activación
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Activacion {
    Sigmoid,
    Tanh,
    ReLU,
    LeakyReLU,
    Softmax,
    Lineal,
}

impl Activacion {
    pub fn aplicar(&self, m: &Matriz) -> Matriz {
        match self {
            Activacion::Sigmoid => m.aplicar(sigmoid),
            Activacion::Tanh => m.aplicar(|x| x.tanh()),
            Activacion::ReLU => m.aplicar(|x| x.max(0.0)),
            Activacion::LeakyReLU => m.aplicar(|x| if x > 0.0 { x } else { 0.01 * x }),
            Activacion::Lineal => m.clone(),
            Activacion::Softmax => softmax_matriz(m),
        }
    }

    pub fn derivada(&self, m: &Matriz) -> Matriz {
        match self {
            Activacion::Sigmoid => {
                let s = m.aplicar(sigmoid);
                s.hadamard(&s.aplicar(|x| 1.0 - x))
            }
            Activacion::Tanh => m.aplicar(|x| 1.0 - x.tanh().powi(2)),
            Activacion::ReLU => m.aplicar(|x| if x > 0.0 { 1.0 } else { 0.0 }),
            Activacion::LeakyReLU => m.aplicar(|x| if x > 0.0 { 1.0 } else { 0.01 }),
            Activacion::Lineal => Matriz::desde_vec(m.filas, m.cols, vec![1.0; m.filas * m.cols]),
            Activacion::Softmax => {
                // Diagonal del Jacobiano: s_i * (1 - s_i)
                let s = softmax_matriz(m);
                s.hadamard(&s.aplicar(|x| 1.0 - x))
            }
        }
    }

    pub fn nombre(&self) -> &str {
        match self {
            Activacion::Sigmoid => "Sigmoid",
            Activacion::Tanh => "Tanh",
            Activacion::ReLU => "ReLU",
            Activacion::LeakyReLU => "LeakyReLU",
            Activacion::Softmax => "Softmax",
            Activacion::Lineal => "Lineal",
        }
    }
}

pub fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

fn softmax_matriz(m: &Matriz) -> Matriz {
    let mut res = m.clone();
    for f in 0..m.filas {
        let fila = m.fila(f);
        let max_v = fila.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let exps: Vec<f64> = fila.iter().map(|&x| (x - max_v).exp()).collect();
        let sum: f64 = exps.iter().sum();
        for c in 0..m.cols {
            res.set(f, c, exps[c] / sum);
        }
    }
    res
}

// ══════════════════════════════════════════════════════════════
//  Funciones de pérdida
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Perdida {
    MSE,
    CrossEntropy,
}

impl Perdida {
    pub fn calcular(&self, prediccion: &Matriz, objetivo: &Matriz) -> f64 {
        match self {
            Perdida::MSE => {
                let diff = prediccion.restar(objetivo);
                diff.datos.iter().map(|x| x * x).sum::<f64>() / (diff.datos.len() as f64)
            }
            Perdida::CrossEntropy => {
                let eps = 1e-12;
                let n = prediccion.filas as f64;
                -prediccion
                    .datos
                    .iter()
                    .zip(&objetivo.datos)
                    .map(|(&p, &y)| y * (p + eps).ln() + (1.0 - y) * (1.0 - p + eps).ln())
                    .sum::<f64>()
                    / n
            }
        }
    }

    pub fn gradiente(&self, prediccion: &Matriz, objetivo: &Matriz) -> Matriz {
        match self {
            Perdida::MSE => prediccion
                .restar(objetivo)
                .escalar(2.0 / prediccion.filas as f64),
            Perdida::CrossEntropy => {
                let eps = 1e-12;
                let n = prediccion.filas as f64;
                let datos: Vec<f64> = prediccion
                    .datos
                    .iter()
                    .zip(&objetivo.datos)
                    .map(|(&p, &y)| (p - y) / ((p + eps) * (1.0 - p + eps) * n))
                    .collect();
                Matriz {
                    filas: prediccion.filas,
                    cols: prediccion.cols,
                    datos,
                }
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════
//  RNG simple (xoshiro256**)  — sin dependencias externas
// ══════════════════════════════════════════════════════════════

/// Generador pseudoaleatorio xoshiro256** para ML.
///
/// **NOTA DE SEGURIDAD:** Este RNG NO es criptográficamente seguro.
/// Solo debe usarse para inicialización de pesos, shuffle de datasets
/// y operaciones de entrenamiento. Para criptografía usar `rand::rngs::OsRng`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        let mut s = [
            seed,
            seed.wrapping_mul(6364136223846793005).wrapping_add(1),
            seed.wrapping_mul(1442695040888963407).wrapping_add(3),
            seed.wrapping_mul(3935559000370003845).wrapping_add(7),
        ];
        if s.iter().all(|&x| x == 0) {
            s[0] = 1;
        }
        let mut rng = Self { s };
        for _ in 0..20 {
            rng.next_u64();
        }
        rng
    }

    pub fn next_u64(&mut self) -> u64 {
        let result = (self.s[1].wrapping_mul(5)).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn rango(&mut self, min: f64, max: f64) -> f64 {
        min + self.f64() * (max - min)
    }

    pub fn usize_rango(&mut self, max: usize) -> usize {
        (self.next_u64() as usize) % max
    }

    /// Box-Muller para distribución normal estándar
    pub fn normal(&mut self) -> f64 {
        let u1 = self.f64().max(1e-15);
        let u2 = self.f64();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }

    pub fn shuffle(&mut self, indices: &mut [usize]) {
        for i in (1..indices.len()).rev() {
            let j = self.usize_rango(i + 1);
            indices.swap(i, j);
        }
    }
}

// ══════════════════════════════════════════════════════════════
//  Utilidades comunes
// ══════════════════════════════════════════════════════════════

pub fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

pub fn vec_sumar(a: &[f64], b: &[f64]) -> Vec<f64> {
    a.iter().zip(b).map(|(x, y)| x + y).collect()
}

pub fn vec_restar(a: &[f64], b: &[f64]) -> Vec<f64> {
    a.iter().zip(b).map(|(x, y)| x - y).collect()
}

pub fn vec_escalar(a: &[f64], k: f64) -> Vec<f64> {
    a.iter().map(|x| x * k).collect()
}

pub fn norma(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

pub fn one_hot(clase: usize, total: usize) -> Vec<f64> {
    let mut v = vec![0.0; total];
    if clase < total {
        v[clase] = 1.0;
    }
    v
}

pub fn argmax(v: &[f64]) -> usize {
    v.iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(i, _)| i)
        .unwrap_or(0)
}
