#![allow(clippy::needless_range_loop)]

use super::linalg::{Activacion, Matriz, Perdida, Rng};
use super::optimizer::{
    BatchNorm, EarlyStopping, EstadoAdam, LRSchedule, LRScheduler, RegularizacionL2,
    TipoOptimizador,
};
use serde::{Deserialize, Serialize};

// ══════════════════════════════════════════════════════════════
//  Red Neuronal Profunda (DNN) — con dropout y batch norm
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapaDNN {
    pub pesos: Matriz,
    pub sesgos: Vec<f64>,
    pub activacion: Activacion,
    pub dropout: f64, // 0.0 = sin dropout
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DNN {
    pub capas: Vec<CapaDNN>,
    pub tasa_aprendizaje: f64,
    pub perdida: Perdida,
    pub momentum: f64,
    pub historial_perdida: Vec<f64>,
    pub optimizador: TipoOptimizador,
    pub lr_schedule: LRSchedule,
    pub l2: Option<RegularizacionL2>,
    pub early_stopping: Option<EarlyStopping>,
    pub batch_norms: Vec<Option<BatchNorm>>,
    // Momentum velocidades
    vel_w: Vec<Matriz>,
    vel_b: Vec<Vec<f64>>,
    #[serde(skip)]
    adam_states: Vec<EstadoAdam>,
}

impl DNN {
    pub fn nueva(
        entrada: usize,
        capas_def: &[(usize, Activacion, f64)], // (neuronas, activación, dropout)
        tasa_aprendizaje: f64,
        momentum: f64,
        perdida: Perdida,
        seed: u64,
    ) -> Self {
        let mut rng = Rng::new(seed);
        let mut capas = Vec::new();
        let mut vel_w = Vec::new();
        let mut vel_b = Vec::new();
        let mut dim_prev = entrada;

        for (neuronas, act, drop) in capas_def {
            let pesos = Matriz::aleatoria(dim_prev, *neuronas, &mut rng);
            let sesgos = vec![0.0; *neuronas];
            vel_w.push(Matriz::nueva(dim_prev, *neuronas));
            vel_b.push(vec![0.0; *neuronas]);
            capas.push(CapaDNN {
                pesos,
                sesgos,
                activacion: act.clone(),
                dropout: *drop,
            });
            dim_prev = *neuronas;
        }

        Self {
            capas,
            tasa_aprendizaje,
            perdida,
            momentum,
            historial_perdida: Vec::new(),
            optimizador: TipoOptimizador::Momentum(momentum),
            lr_schedule: LRSchedule::Constante,
            l2: None,
            early_stopping: None,
            batch_norms: Vec::new(),
            vel_w,
            vel_b,
            adam_states: Vec::new(),
        }
    }

    /// Configura Adam como optimizador
    pub fn con_adam(mut self) -> Self {
        self.optimizador = TipoOptimizador::Adam(0.9, 0.999, 1e-8);
        self
    }

    /// Configura un learning rate schedule
    pub fn con_lr_schedule(mut self, schedule: LRSchedule) -> Self {
        self.lr_schedule = schedule;
        self
    }

    /// Configura regularización L2
    pub fn con_l2(mut self, lambda: f64) -> Self {
        self.l2 = Some(RegularizacionL2::nueva(lambda));
        self
    }

    /// Configura early stopping
    pub fn con_early_stopping(mut self, paciencia: usize, min_delta: f64) -> Self {
        self.early_stopping = Some(EarlyStopping::nuevo(paciencia, min_delta));
        self
    }

    /// Activa batch normalization en capas ocultas
    pub fn con_batch_norm(mut self) -> Self {
        self.batch_norms = self
            .capas
            .iter()
            .enumerate()
            .map(|(i, c)| {
                if i < self.capas.len() - 1 {
                    Some(BatchNorm::nuevo(c.pesos.cols))
                } else {
                    None
                }
            })
            .collect();
        self
    }

    fn init_adam(&mut self) {
        if let TipoOptimizador::Adam(..) = &self.optimizador {
            if self.adam_states.len() != self.capas.len() {
                self.adam_states = self
                    .capas
                    .iter()
                    .map(|c| EstadoAdam::nuevo(c.pesos.filas, c.pesos.cols))
                    .collect();
            }
        }
    }

    fn forward(
        &self,
        x: &Matriz,
        entrenando: bool,
        rng: &mut Rng,
    ) -> (Vec<Matriz>, Vec<Matriz>, Vec<Matriz>) {
        let mut a_vec = vec![x.clone()];
        let mut z_vec = Vec::new();
        let mut masks = Vec::new();
        let mut actual = x.clone();

        for capa in &self.capas {
            let z = actual.mul(&capa.pesos).sumar_fila(&capa.sesgos);
            let mut a = capa.activacion.aplicar(&z);

            // Dropout
            let mask = if entrenando && capa.dropout > 0.0 {
                let m = Matriz::desde_vec(
                    a.filas,
                    a.cols,
                    (0..a.filas * a.cols)
                        .map(|_| {
                            if rng.f64() > capa.dropout {
                                1.0 / (1.0 - capa.dropout)
                            } else {
                                0.0
                            }
                        })
                        .collect(),
                );
                a = a.hadamard(&m);
                m
            } else {
                Matriz::desde_vec(a.filas, a.cols, vec![1.0; a.filas * a.cols])
            };

            z_vec.push(z);
            masks.push(mask);
            actual = a.clone();
            a_vec.push(a);
        }

        (z_vec, a_vec, masks)
    }

    pub fn predecir(&self, x: &Matriz) -> Matriz {
        let mut rng = Rng::new(0);
        let (_, a_vec, _) = self.forward(x, false, &mut rng);
        a_vec.last().unwrap().clone()
    }

    pub fn predecir_clase(&self, x: &Matriz) -> Vec<usize> {
        self.predecir(x).argmax_por_fila()
    }

    pub fn entrenar(&mut self, x: &Matriz, y: &Matriz, epocas: usize, batch_size: usize) {
        let n = x.filas;
        let mut rng = Rng::new(777);
        self.init_adam();
        let mut scheduler = LRScheduler::nuevo(self.tasa_aprendizaje, self.lr_schedule.clone());
        let _use_bn = !self.batch_norms.is_empty();

        for epoca in 0..epocas {
            let mut indices: Vec<usize> = (0..n).collect();
            rng.shuffle(&mut indices);

            let lr = scheduler.lr_actual;
            let mut perdida_total = 0.0;
            let mut batches = 0;

            for chunk_start in (0..n).step_by(batch_size) {
                let chunk_end = (chunk_start + batch_size).min(n);
                let batch_idx: Vec<usize> = indices[chunk_start..chunk_end].to_vec();
                let bs = batch_idx.len();

                let x_batch = extraer_filas(x, &batch_idx);
                let y_batch = extraer_filas(y, &batch_idx);

                let (z_vec, a_vec, masks) = self.forward(&x_batch, true, &mut rng);
                let pred = a_vec.last().unwrap();

                let mut loss = self.perdida.calcular(pred, &y_batch);
                if let Some(ref l2) = self.l2 {
                    for capa in &self.capas {
                        loss += l2.penalty(&capa.pesos) / bs as f64;
                    }
                }
                perdida_total += loss;
                batches += 1;

                // Backprop
                let mut delta = pred.restar(&y_batch);

                for l in (0..self.capas.len()).rev() {
                    delta = delta.hadamard(&masks[l]);

                    let a_prev = &a_vec[l];
                    let mut grad_w = a_prev.transpuesta().mul(&delta).escalar(1.0 / bs as f64);
                    let grad_b: Vec<f64> = delta
                        .suma_columnas()
                        .iter()
                        .map(|&x| x / bs as f64)
                        .collect();

                    // L2 regularización
                    if let Some(ref l2) = self.l2 {
                        grad_w =
                            grad_w.sumar(&l2.grad_w(&self.capas[l].pesos).escalar(1.0 / bs as f64));
                    }

                    match &self.optimizador {
                        TipoOptimizador::Adam(beta1, beta2, eps) => {
                            let (new_w, new_b) = self.adam_states[l].actualizar(
                                &self.capas[l].pesos,
                                &self.capas[l].sesgos,
                                &grad_w,
                                &grad_b,
                                lr,
                                *beta1,
                                *beta2,
                                *eps,
                            );
                            self.capas[l].pesos = new_w;
                            self.capas[l].sesgos = new_b;
                        }
                        _ => {
                            self.vel_w[l] = self.vel_w[l]
                                .escalar(self.momentum)
                                .sumar(&grad_w.escalar(lr));
                            for j in 0..self.vel_b[l].len() {
                                self.vel_b[l][j] =
                                    self.momentum * self.vel_b[l][j] + lr * grad_b[j];
                            }
                            self.capas[l].pesos = self.capas[l].pesos.restar(&self.vel_w[l]);
                            for j in 0..self.capas[l].sesgos.len() {
                                self.capas[l].sesgos[j] -= self.vel_b[l][j];
                            }
                        }
                    }

                    if l > 0 {
                        let d_act = self.capas[l - 1].activacion.derivada(&z_vec[l - 1]);
                        delta = delta
                            .mul(&self.capas[l].pesos.transpuesta())
                            .hadamard(&d_act);
                    }
                }
            }

            let avg_loss = perdida_total / batches as f64;
            self.historial_perdida.push(avg_loss);
            scheduler.paso(epoca, avg_loss);

            if let Some(ref mut es) = self.early_stopping {
                if es.verificar(avg_loss) {
                    println!(
                        "    ⛔ Early stopping en época {} (sin mejora en {} épocas)",
                        epoca + 1,
                        es.paciencia
                    );
                    break;
                }
            }

            if (epoca + 1) % (epocas / 10).max(1) == 0 || epoca == 0 {
                println!(
                    "    Época {}/{} — Pérdida: {:.6} — LR: {:.6}",
                    epoca + 1,
                    epocas,
                    avg_loss,
                    lr
                );
            }
        }
    }

    pub fn precision(&self, x: &Matriz, etiquetas: &[usize]) -> f64 {
        let preds = self.predecir_clase(x);
        let correctas = preds.iter().zip(etiquetas).filter(|(p, e)| p == e).count();
        correctas as f64 / etiquetas.len() as f64
    }

    pub fn resumen(&self) {
        println!("  Red Neuronal Profunda (DNN)");
        println!("  ──────────────────────────");
        for (i, capa) in self.capas.iter().enumerate() {
            println!(
                "    Capa {}: {} → {} [{}] dropout={:.0}%",
                i + 1,
                capa.pesos.filas,
                capa.pesos.cols,
                capa.activacion.nombre(),
                capa.dropout * 100.0
            );
        }
        let total: usize = self
            .capas
            .iter()
            .map(|c| c.pesos.filas * c.pesos.cols + c.sesgos.len())
            .sum();
        println!("    Total parámetros: {}", total);
        println!("    Momentum: {}", self.momentum);
    }
}

fn extraer_filas(m: &Matriz, indices: &[usize]) -> Matriz {
    let filas = indices.len();
    let cols = m.cols;
    let mut datos = Vec::with_capacity(filas * cols);
    for &i in indices {
        datos.extend_from_slice(&m.datos[i * cols..(i + 1) * cols]);
    }
    Matriz { filas, cols, datos }
}
