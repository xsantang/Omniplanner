#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

//! Optimizadores y utilidades de entrenamiento para redes neuronales.
//!
//! Contiene Adam, SGD, LR scheduling (4 estrategias), early stopping,
//! batch normalization, regularización L2 y resultados de k-fold CV.

use super::linalg::Matriz;
use serde::{Deserialize, Serialize};

// ══════════════════════════════════════════════════════════════
//  Optimizadores — Adam, SGD con momentum
// ══════════════════════════════════════════════════════════════

/// Configuración de regularización L2 (weight decay)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegularizacionL2 {
    pub lambda: f64, // coeficiente de regularización
}

impl RegularizacionL2 {
    pub fn nueva(lambda: f64) -> Self {
        Self { lambda }
    }

    /// Calcula el penalty L2: λ/2 * Σ w²
    pub fn penalty(&self, pesos: &Matriz) -> f64 {
        self.lambda / 2.0 * pesos.datos.iter().map(|w| w * w).sum::<f64>()
    }

    /// Gradiente adicional de L2: λ * w (sumar al gradiente de pesos)
    pub fn grad_w(&self, pesos: &Matriz) -> Matriz {
        pesos.escalar(self.lambda)
    }
}

// ══════════════════════════════════════════════════════════════
//  Early Stopping
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EarlyStopping {
    pub paciencia: usize,
    pub min_delta: f64,
    pub mejor_loss: f64,
    epocas_sin_mejora: usize,
    pub activo: bool,
}

impl EarlyStopping {
    pub fn nuevo(paciencia: usize, min_delta: f64) -> Self {
        Self {
            paciencia,
            min_delta,
            mejor_loss: f64::INFINITY,
            epocas_sin_mejora: 0,
            activo: true,
        }
    }

    /// Verifica si debe detenerse. Devuelve true si hay que parar.
    pub fn verificar(&mut self, loss: f64) -> bool {
        if !self.activo {
            return false;
        }
        if loss < self.mejor_loss - self.min_delta {
            self.mejor_loss = loss;
            self.epocas_sin_mejora = 0;
            false
        } else {
            self.epocas_sin_mejora += 1;
            self.epocas_sin_mejora >= self.paciencia
        }
    }

    pub fn epocas_sin_mejora(&self) -> usize {
        self.epocas_sin_mejora
    }
}

// ══════════════════════════════════════════════════════════════
//  Batch Normalization
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchNorm {
    pub gamma: Vec<f64>, // escala (aprendible)
    pub beta: Vec<f64>,  // desplazamiento (aprendible)
    pub running_mean: Vec<f64>,
    pub running_var: Vec<f64>,
    pub momentum: f64, // para running stats (ej: 0.1)
    pub epsilon: f64,
    pub dim: usize,
}

impl BatchNorm {
    pub fn nuevo(dim: usize) -> Self {
        Self {
            gamma: vec![1.0; dim],
            beta: vec![0.0; dim],
            running_mean: vec![0.0; dim],
            running_var: vec![1.0; dim],
            momentum: 0.1,
            epsilon: 1e-5,
            dim,
        }
    }

    /// Forward pass en modo entrenamiento.
    /// Normaliza por batch y actualiza running stats.
    /// Retorna (salida, mean_batch, var_batch, x_norm) para backprop.
    pub fn forward_train(&mut self, x: &Matriz) -> (Matriz, Vec<f64>, Vec<f64>, Matriz) {
        let bs = x.filas as f64;
        let mut mean = vec![0.0; self.dim];
        let mut var = vec![0.0; self.dim];

        // Media por columna
        for f in 0..x.filas {
            for c in 0..self.dim {
                mean[c] += x.get(f, c);
            }
        }
        for c in 0..self.dim {
            mean[c] /= bs;
        }

        // Varianza por columna
        for f in 0..x.filas {
            for c in 0..self.dim {
                let diff = x.get(f, c) - mean[c];
                var[c] += diff * diff;
            }
        }
        for c in 0..self.dim {
            var[c] /= bs;
        }

        // Normalizar
        let mut x_norm = Matriz::nueva(x.filas, self.dim);
        let mut out = Matriz::nueva(x.filas, self.dim);
        for f in 0..x.filas {
            for c in 0..self.dim {
                let xn = (x.get(f, c) - mean[c]) / (var[c] + self.epsilon).sqrt();
                x_norm.set(f, c, xn);
                out.set(f, c, self.gamma[c] * xn + self.beta[c]);
            }
        }

        // Actualizar running stats
        for c in 0..self.dim {
            self.running_mean[c] =
                (1.0 - self.momentum) * self.running_mean[c] + self.momentum * mean[c];
            self.running_var[c] =
                (1.0 - self.momentum) * self.running_var[c] + self.momentum * var[c];
        }

        (out, mean, var, x_norm)
    }

    /// Forward en modo inferencia (usa running stats)
    pub fn forward_eval(&self, x: &Matriz) -> Matriz {
        let mut out = Matriz::nueva(x.filas, self.dim);
        for f in 0..x.filas {
            for c in 0..self.dim {
                let xn = (x.get(f, c) - self.running_mean[c])
                    / (self.running_var[c] + self.epsilon).sqrt();
                out.set(f, c, self.gamma[c] * xn + self.beta[c]);
            }
        }
        out
    }

    /// Backward pass. Recibe grad de salida, devuelve grad de entrada.
    /// También actualiza gamma y beta.
    pub fn backward(
        &mut self,
        grad_out: &Matriz,
        x_norm: &Matriz,
        mean: &[f64],
        var: &[f64],
        x: &Matriz,
        lr: f64,
    ) -> Matriz {
        let bs = grad_out.filas as f64;
        let mut grad_gamma = vec![0.0; self.dim];
        let mut grad_beta = vec![0.0; self.dim];

        for f in 0..grad_out.filas {
            for c in 0..self.dim {
                grad_gamma[c] += grad_out.get(f, c) * x_norm.get(f, c);
                grad_beta[c] += grad_out.get(f, c);
            }
        }

        // Gradiente de entrada
        let mut grad_input = Matriz::nueva(grad_out.filas, self.dim);
        for c in 0..self.dim {
            let std_inv = 1.0 / (var[c] + self.epsilon).sqrt();
            let mut dx_norm_sum = 0.0;
            let mut dx_norm_x_sum = 0.0;
            for f in 0..grad_out.filas {
                let dn = grad_out.get(f, c) * self.gamma[c];
                dx_norm_sum += dn;
                dx_norm_x_sum += dn * (x.get(f, c) - mean[c]);
            }
            for f in 0..grad_out.filas {
                let dn = grad_out.get(f, c) * self.gamma[c];
                let dx = std_inv
                    * (dn
                        - dx_norm_sum / bs
                        - (x.get(f, c) - mean[c]) * dx_norm_x_sum / (bs * (var[c] + self.epsilon)));
                grad_input.set(f, c, dx);
            }
        }

        // Actualizar gamma y beta
        for c in 0..self.dim {
            self.gamma[c] -= lr * grad_gamma[c] / bs;
            self.beta[c] -= lr * grad_beta[c] / bs;
        }

        grad_input
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TipoOptimizador {
    SGD,
    Momentum(f64),       // momentum factor
    Adam(f64, f64, f64), // beta1, beta2, epsilon
}

impl Default for TipoOptimizador {
    fn default() -> Self {
        TipoOptimizador::Adam(0.9, 0.999, 1e-8)
    }
}

/// Estado Adam para una capa: momentos para pesos y sesgos
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EstadoAdam {
    pub m_w: Matriz,   // 1er momento (media) pesos
    pub v_w: Matriz,   // 2do momento (varianza) pesos
    pub m_b: Vec<f64>, // 1er momento sesgos
    pub v_b: Vec<f64>, // 2do momento sesgos
    pub t: u64,        // paso temporal
}

impl EstadoAdam {
    pub fn nuevo(filas: usize, cols: usize) -> Self {
        Self {
            m_w: Matriz::nueva(filas, cols),
            v_w: Matriz::nueva(filas, cols),
            m_b: vec![0.0; cols],
            v_b: vec![0.0; cols],
            t: 0,
        }
    }

    /// Actualiza pesos y sesgos con Adam.
    /// Devuelve los pesos y sesgos actualizados.
    pub fn actualizar(
        &mut self,
        pesos: &Matriz,
        sesgos: &[f64],
        grad_w: &Matriz,
        grad_b: &[f64],
        lr: f64,
        beta1: f64,
        beta2: f64,
        epsilon: f64,
    ) -> (Matriz, Vec<f64>) {
        self.t += 1;
        let t = self.t as f64;

        // Actualizar momentos de pesos
        let mut nuevos_pesos = pesos.clone();
        for i in 0..pesos.filas {
            for j in 0..pesos.cols {
                let g = grad_w.get(i, j);
                let m = beta1 * self.m_w.get(i, j) + (1.0 - beta1) * g;
                let v = beta2 * self.v_w.get(i, j) + (1.0 - beta2) * g * g;
                self.m_w.set(i, j, m);
                self.v_w.set(i, j, v);

                // Corrección de sesgo
                let m_hat = m / (1.0 - beta1.powf(t));
                let v_hat = v / (1.0 - beta2.powf(t));

                nuevos_pesos.set(
                    i,
                    j,
                    pesos.get(i, j) - lr * m_hat / (v_hat.sqrt() + epsilon),
                );
            }
        }

        // Actualizar momentos de sesgos
        let mut nuevos_sesgos = sesgos.to_vec();
        for j in 0..sesgos.len() {
            let g = grad_b[j];
            let m = beta1 * self.m_b[j] + (1.0 - beta1) * g;
            let v = beta2 * self.v_b[j] + (1.0 - beta2) * g * g;
            self.m_b[j] = m;
            self.v_b[j] = v;

            let m_hat = m / (1.0 - beta1.powf(t));
            let v_hat = v / (1.0 - beta2.powf(t));

            nuevos_sesgos[j] = sesgos[j] - lr * m_hat / (v_hat.sqrt() + epsilon);
        }

        (nuevos_pesos, nuevos_sesgos)
    }
}

/// Estado Adam para parámetros 1D (filtros conv, etc.)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EstadoAdamVec {
    pub m: Vec<f64>,
    pub v: Vec<f64>,
    pub m_b: f64,
    pub v_b: f64,
    pub t: u64,
}

impl EstadoAdamVec {
    pub fn nuevo(dim: usize) -> Self {
        Self {
            m: vec![0.0; dim],
            v: vec![0.0; dim],
            m_b: 0.0,
            v_b: 0.0,
            t: 0,
        }
    }

    pub fn actualizar(
        &mut self,
        params: &[f64],
        sesgo: f64,
        grad: &[f64],
        grad_bias: f64,
        lr: f64,
        beta1: f64,
        beta2: f64,
        epsilon: f64,
    ) -> (Vec<f64>, f64) {
        self.t += 1;
        let t = self.t as f64;

        let mut nuevos = params.to_vec();
        for i in 0..params.len() {
            let g = grad[i];
            self.m[i] = beta1 * self.m[i] + (1.0 - beta1) * g;
            self.v[i] = beta2 * self.v[i] + (1.0 - beta2) * g * g;

            let m_hat = self.m[i] / (1.0 - beta1.powf(t));
            let v_hat = self.v[i] / (1.0 - beta2.powf(t));

            nuevos[i] = params[i] - lr * m_hat / (v_hat.sqrt() + epsilon);
        }

        // Sesgo
        self.m_b = beta1 * self.m_b + (1.0 - beta1) * grad_bias;
        self.v_b = beta2 * self.v_b + (1.0 - beta2) * grad_bias * grad_bias;
        let m_hat_b = self.m_b / (1.0 - beta1.powf(t));
        let v_hat_b = self.v_b / (1.0 - beta2.powf(t));
        let nuevo_sesgo = sesgo - lr * m_hat_b / (v_hat_b.sqrt() + epsilon);

        (nuevos, nuevo_sesgo)
    }
}

// ══════════════════════════════════════════════════════════════
//  Learning Rate Scheduling
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub enum LRSchedule {
    /// Tasa constante (sin scheduling)
    #[default]
    Constante,
    /// Step decay: lr *= factor cada `step_size` épocas
    StepDecay { step_size: usize, factor: f64 },
    /// Cosine annealing: lr oscila entre lr_max y lr_min
    CosineAnnealing { lr_min: f64, t_max: usize },
    /// Reduce al estancarse: reduce lr si la pérdida no baja en `paciencia` épocas
    ReduceAlEstancarse {
        factor: f64,
        paciencia: usize,
        lr_min: f64,
    },
}

// ══════════════════════════════════════════════════════════════
//  K-Fold Cross-Validation — Resultado
// ══════════════════════════════════════════════════════════════

/// Resultado de una validación cruzada k-fold
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResultadoCV {
    pub k: usize,
    pub precisiones_train: Vec<f64>,
    pub precisiones_test: Vec<f64>,
    pub perdidas_train: Vec<f64>,
}

impl ResultadoCV {
    pub fn nuevo(k: usize) -> Self {
        Self {
            k,
            precisiones_train: Vec::with_capacity(k),
            precisiones_test: Vec::with_capacity(k),
            perdidas_train: Vec::with_capacity(k),
        }
    }

    pub fn agregar_fold(&mut self, prec_train: f64, prec_test: f64, loss_train: f64) {
        self.precisiones_train.push(prec_train);
        self.precisiones_test.push(prec_test);
        self.perdidas_train.push(loss_train);
    }

    pub fn media_test(&self) -> f64 {
        let n = self.precisiones_test.len() as f64;
        if n == 0.0 {
            return 0.0;
        }
        self.precisiones_test.iter().sum::<f64>() / n
    }

    pub fn media_train(&self) -> f64 {
        let n = self.precisiones_train.len() as f64;
        if n == 0.0 {
            return 0.0;
        }
        self.precisiones_train.iter().sum::<f64>() / n
    }

    pub fn desviacion_test(&self) -> f64 {
        let media = self.media_test();
        let n = self.precisiones_test.len() as f64;
        if n <= 1.0 {
            return 0.0;
        }
        let var = self
            .precisiones_test
            .iter()
            .map(|x| (x - media).powi(2))
            .sum::<f64>()
            / (n - 1.0);
        var.sqrt()
    }

    pub fn resumen(&self) -> String {
        format!(
            "CV {}-fold: test={:.1}% ±{:.1}%  train={:.1}%  [{}]",
            self.k,
            self.media_test() * 100.0,
            self.desviacion_test() * 100.0,
            self.media_train() * 100.0,
            self.precisiones_test
                .iter()
                .map(|p| format!("{:.1}%", p * 100.0))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LRScheduler {
    pub schedule: LRSchedule,
    pub lr_inicial: f64,
    pub lr_actual: f64,
    best_loss: f64,
    epocas_sin_mejora: usize,
}

impl LRScheduler {
    pub fn nuevo(lr_inicial: f64, schedule: LRSchedule) -> Self {
        Self {
            schedule,
            lr_inicial,
            lr_actual: lr_inicial,
            best_loss: f64::INFINITY,
            epocas_sin_mejora: 0,
        }
    }

    /// Llamar al final de cada época con la época actual (0-indexed) y la pérdida
    pub fn paso(&mut self, epoca: usize, loss: f64) -> f64 {
        match &self.schedule {
            LRSchedule::Constante => {}
            LRSchedule::StepDecay { step_size, factor } => {
                if (epoca + 1).is_multiple_of(*step_size) {
                    self.lr_actual *= factor;
                }
            }
            LRSchedule::CosineAnnealing { lr_min, t_max } => {
                let t_max = *t_max as f64;
                let t = epoca as f64;
                self.lr_actual = lr_min
                    + (self.lr_inicial - lr_min) * (1.0 + (std::f64::consts::PI * t / t_max).cos())
                        / 2.0;
            }
            LRSchedule::ReduceAlEstancarse {
                factor,
                paciencia,
                lr_min,
            } => {
                if loss < self.best_loss - 1e-6 {
                    self.best_loss = loss;
                    self.epocas_sin_mejora = 0;
                } else {
                    self.epocas_sin_mejora += 1;
                    if self.epocas_sin_mejora >= *paciencia {
                        self.lr_actual = (self.lr_actual * factor).max(*lr_min);
                        self.epocas_sin_mejora = 0;
                    }
                }
            }
        }
        self.lr_actual
    }
}
