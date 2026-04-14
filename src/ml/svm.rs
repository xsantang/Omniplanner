use super::linalg::{dot, vec_escalar, vec_restar, Rng};
use serde::{Deserialize, Serialize};

// ══════════════════════════════════════════════════════════════
//  Máquina de Vectores de Soporte (SVM) — clasificación binaria
//  Implementación: Descenso por gradiente con hinge loss
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SVM {
    pub pesos: Vec<f64>,
    pub sesgo: f64,
    pub c: f64, // parámetro de regularización
    pub tasa_aprendizaje: f64,
    pub historial_perdida: Vec<f64>,
    pub entrenado: bool,
}

impl SVM {
    pub fn nuevo(dimensiones: usize, c: f64, tasa_aprendizaje: f64) -> Self {
        Self {
            pesos: vec![0.0; dimensiones],
            sesgo: 0.0,
            c,
            tasa_aprendizaje,
            historial_perdida: Vec::new(),
            entrenado: false,
        }
    }

    /// Entrena el SVM. Las etiquetas deben ser +1 o -1.
    pub fn entrenar(&mut self, x: &[Vec<f64>], y: &[f64], epocas: usize) {
        let n = x.len();
        let dim = x[0].len();
        let mut rng = Rng::new(123);

        for epoca in 0..epocas {
            let mut indices: Vec<usize> = (0..n).collect();
            rng.shuffle(&mut indices);

            let mut perdida_total = 0.0;

            for &i in &indices {
                let yi = y[i];
                let score = dot(&self.pesos, &x[i]) + self.sesgo;
                let margen = yi * score;

                if margen < 1.0 {
                    // Mal clasificado o dentro del margen
                    let grad_w = vec_restar(&self.pesos, &vec_escalar(&x[i], self.c * yi));
                    self.pesos =
                        vec_restar(&self.pesos, &vec_escalar(&grad_w, self.tasa_aprendizaje));
                    self.sesgo += self.tasa_aprendizaje * self.c * yi;
                    perdida_total += 1.0 - margen;
                } else {
                    // Correctamente clasificado fuera del margen
                    for j in 0..dim {
                        self.pesos[j] *= 1.0 - self.tasa_aprendizaje;
                    }
                }
            }

            // Hinge loss + regularización
            let reg: f64 = self.pesos.iter().map(|w| w * w).sum::<f64>() * 0.5;
            let loss = reg + self.c * perdida_total / n as f64;
            self.historial_perdida.push(loss);

            if (epoca + 1) % (epocas / 10).max(1) == 0 || epoca == 0 {
                println!("    Época {}/{} — Pérdida: {:.6}", epoca + 1, epocas, loss);
            }
        }

        self.entrenado = true;
    }

    pub fn predecir_valor(&self, x: &[f64]) -> f64 {
        dot(&self.pesos, x) + self.sesgo
    }

    pub fn predecir(&self, x: &[f64]) -> i32 {
        if self.predecir_valor(x) >= 0.0 {
            1
        } else {
            -1
        }
    }

    pub fn predecir_lote(&self, x: &[Vec<f64>]) -> Vec<i32> {
        x.iter().map(|xi| self.predecir(xi)).collect()
    }

    pub fn precision(&self, x: &[Vec<f64>], y: &[f64]) -> f64 {
        let correctas = x
            .iter()
            .zip(y)
            .filter(|(xi, &yi)| self.predecir(xi) as f64 == yi)
            .count();
        correctas as f64 / y.len() as f64
    }

    pub fn resumen(&self) {
        println!("  Máquina de Vectores de Soporte (SVM)");
        println!("  ────────────────────────────────────");
        println!("    Dimensiones: {}", self.pesos.len());
        println!("    Parámetro C: {}", self.c);
        println!("    Sesgo: {:.6}", self.sesgo);
        println!(
            "    Entrenado: {}",
            if self.entrenado { "Sí" } else { "No" }
        );
    }
}

// ══════════════════════════════════════════════════════════════
//  SVM Multi-clase (One-vs-All)
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SVMMulticlase {
    pub modelos: Vec<SVM>,
    pub num_clases: usize,
}

impl SVMMulticlase {
    pub fn nuevo(dimensiones: usize, num_clases: usize, c: f64, lr: f64) -> Self {
        let modelos = (0..num_clases)
            .map(|_| SVM::nuevo(dimensiones, c, lr))
            .collect();
        Self {
            modelos,
            num_clases,
        }
    }

    pub fn entrenar(&mut self, x: &[Vec<f64>], y: &[usize], epocas: usize) {
        for clase in 0..self.num_clases {
            println!("  Entrenando clasificador para clase {} ...", clase);
            let etiquetas: Vec<f64> = y
                .iter()
                .map(|&yi| if yi == clase { 1.0 } else { -1.0 })
                .collect();
            self.modelos[clase].entrenar(x, &etiquetas, epocas);
        }
    }

    pub fn predecir(&self, x: &[f64]) -> usize {
        self.modelos
            .iter()
            .enumerate()
            .map(|(i, m)| (i, m.predecir_valor(x)))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    pub fn precision(&self, x: &[Vec<f64>], y: &[usize]) -> f64 {
        let correctas = x
            .iter()
            .zip(y)
            .filter(|(xi, &yi)| self.predecir(xi) == yi)
            .count();
        correctas as f64 / y.len() as f64
    }
}
