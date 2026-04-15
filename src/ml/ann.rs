#![allow(clippy::needless_range_loop, clippy::type_complexity)]

use super::linalg::{Activacion, Matriz, Perdida, Rng};
use super::optimizer::{
    BatchNorm, EarlyStopping, EstadoAdam, LRSchedule, LRScheduler, RegularizacionL2,
    TipoOptimizador,
};
use serde::{Deserialize, Serialize};

// ══════════════════════════════════════════════════════════════
//  Red Neuronal Artificial (ANN) — Perceptrón multicapa
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapaANN {
    pub pesos: Matriz,
    pub sesgos: Vec<f64>,
    pub activacion: Activacion,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ANN {
    pub capas: Vec<CapaANN>,
    pub tasa_aprendizaje: f64,
    pub perdida: Perdida,
    pub historial_perdida: Vec<f64>,
    pub optimizador: TipoOptimizador,
    pub lr_schedule: LRSchedule,
    pub l2: Option<RegularizacionL2>,
    pub early_stopping: Option<EarlyStopping>,
    pub batch_norms: Vec<Option<BatchNorm>>,
    #[serde(skip)]
    adam_states: Vec<EstadoAdam>,
    #[serde(skip)]
    lr_scheduler: Option<LRScheduler>,
}

#[derive(Clone, Debug)]
struct CacheForward {
    _z: Vec<Matriz>, // pre-activación
    a: Vec<Matriz>,  // post-activación
}

impl ANN {
    /// Crea una ANN con arquitectura definida por `capas_def`:
    /// cada tupla es (neuronas, activación).
    /// `entrada` = número de features de entrada.
    pub fn nueva(
        entrada: usize,
        capas_def: &[(usize, Activacion)],
        tasa_aprendizaje: f64,
        perdida: Perdida,
        seed: u64,
    ) -> Self {
        let mut rng = Rng::new(seed);
        let mut capas = Vec::new();
        let mut dim_prev = entrada;

        for (neuronas, act) in capas_def {
            let pesos = Matriz::aleatoria(dim_prev, *neuronas, &mut rng);
            let sesgos = vec![0.0; *neuronas];
            capas.push(CapaANN {
                pesos,
                sesgos,
                activacion: act.clone(),
            });
            dim_prev = *neuronas;
        }

        Self {
            capas,
            tasa_aprendizaje,
            perdida,
            historial_perdida: Vec::new(),
            optimizador: TipoOptimizador::SGD,
            lr_schedule: LRSchedule::Constante,
            l2: None,
            early_stopping: None,
            batch_norms: Vec::new(),
            adam_states: Vec::new(),
            lr_scheduler: None,
        }
    }

    /// Configura Adam como optimizador (beta1=0.9, beta2=0.999, eps=1e-8)
    pub fn con_adam(mut self) -> Self {
        self.optimizador = TipoOptimizador::Adam(0.9, 0.999, 1e-8);
        self
    }

    /// Configura un learning rate schedule
    pub fn con_lr_schedule(mut self, schedule: LRSchedule) -> Self {
        self.lr_schedule = schedule;
        self
    }

    /// Configura regularización L2 (weight decay)
    pub fn con_l2(mut self, lambda: f64) -> Self {
        self.l2 = Some(RegularizacionL2::nueva(lambda));
        self
    }

    /// Configura early stopping
    pub fn con_early_stopping(mut self, paciencia: usize, min_delta: f64) -> Self {
        self.early_stopping = Some(EarlyStopping::nuevo(paciencia, min_delta));
        self
    }

    /// Activa batch normalization en todas las capas ocultas
    pub fn con_batch_norm(mut self) -> Self {
        self.batch_norms = self
            .capas
            .iter()
            .enumerate()
            .map(|(i, c)| {
                if i < self.capas.len() - 1 {
                    Some(BatchNorm::nuevo(c.pesos.cols))
                } else {
                    None // no BN en última capa
                }
            })
            .collect();
        self
    }

    /// Inicializa estados Adam (llamar antes de entrenar si se usa Adam)
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

    fn forward(&self, x: &Matriz) -> CacheForward {
        let mut a_vec = vec![x.clone()];
        let mut z_vec = Vec::new();
        let mut actual = x.clone();

        for capa in &self.capas {
            let z = actual.mul(&capa.pesos).sumar_fila(&capa.sesgos);
            let a = capa.activacion.aplicar(&z);
            z_vec.push(z);
            actual = a.clone();
            a_vec.push(a);
        }

        CacheForward {
            _z: z_vec,
            a: a_vec,
        }
    }

    pub fn predecir(&self, x: &Matriz) -> Matriz {
        let cache = self.forward(x);
        cache.a.last().unwrap().clone()
    }

    pub fn predecir_clase(&self, x: &Matriz) -> Vec<usize> {
        self.predecir(x).argmax_por_fila()
    }

    pub fn entrenar(&mut self, x: &Matriz, y: &Matriz, epocas: usize, batch_size: usize) {
        let n = x.filas;
        let mut rng = Rng::new(42);
        self.init_adam();
        let mut scheduler = LRScheduler::nuevo(self.tasa_aprendizaje, self.lr_schedule.clone());
        let use_bn = !self.batch_norms.is_empty();

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

                // Forward con batch norm
                let mut a_vec = vec![x_batch.clone()];
                let mut z_vec = Vec::new();
                let mut bn_cache: Vec<Option<(Vec<f64>, Vec<f64>, Matriz, Matriz)>> = Vec::new();
                let mut actual = x_batch;

                for (l, capa) in self.capas.iter().enumerate() {
                    let z = actual.mul(&capa.pesos).sumar_fila(&capa.sesgos);

                    let z_bn = if use_bn {
                        if let Some(Some(bn)) = self.batch_norms.get_mut(l) {
                            let (out, mean, var, x_norm) = bn.forward_train(&z);
                            bn_cache.push(Some((mean, var, x_norm, z.clone())));
                            out
                        } else {
                            bn_cache.push(None);
                            z.clone()
                        }
                    } else {
                        bn_cache.push(None);
                        z.clone()
                    };

                    let a = capa.activacion.aplicar(&z_bn);
                    z_vec.push(z_bn);
                    actual = a.clone();
                    a_vec.push(a);
                }

                let pred = a_vec.last().unwrap();
                let mut loss = self.perdida.calcular(pred, &y_batch);

                // L2 penalty
                if let Some(ref l2) = self.l2 {
                    for capa in &self.capas {
                        loss += l2.penalty(&capa.pesos) / bs as f64;
                    }
                }

                perdida_total += loss;
                batches += 1;

                // Backpropagation
                let mut delta = pred.restar(&y_batch);

                for l in (0..self.capas.len()).rev() {
                    let a_prev = &a_vec[l];

                    let mut grad_w = a_prev.transpuesta().mul(&delta).escalar(1.0 / bs as f64);
                    let grad_b = delta
                        .suma_columnas()
                        .iter()
                        .map(|&x| x / bs as f64)
                        .collect::<Vec<f64>>();

                    // L2: sumar λ*w al gradiente
                    if let Some(ref l2) = self.l2 {
                        grad_w =
                            grad_w.sumar(&l2.grad_w(&self.capas[l].pesos).escalar(1.0 / bs as f64));
                    }

                    // Actualizar pesos según optimizador
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
                            self.capas[l].pesos = self.capas[l].pesos.restar(&grad_w.escalar(lr));
                            for j in 0..self.capas[l].sesgos.len() {
                                self.capas[l].sesgos[j] -= lr * grad_b[j];
                            }
                        }
                    }

                    // Propagar delta hacia atrás
                    if l > 0 {
                        // BatchNorm backward
                        let delta_bn = if use_bn {
                            if let Some(Some((ref mean, ref var, ref x_norm, ref z_pre))) =
                                bn_cache.get(l)
                            {
                                if let Some(Some(bn)) = self.batch_norms.get_mut(l) {
                                    bn.backward(&delta, x_norm, mean, var, z_pre, lr)
                                } else {
                                    delta.clone()
                                }
                            } else {
                                delta.clone()
                            }
                        } else {
                            delta.clone()
                        };

                        let d_act = self.capas[l - 1].activacion.derivada(&z_vec[l - 1]);
                        delta = delta_bn
                            .mul(&self.capas[l].pesos.transpuesta())
                            .hadamard(&d_act);
                    }
                }
            }

            let avg_loss = perdida_total / batches as f64;
            self.historial_perdida.push(avg_loss);
            scheduler.paso(epoca, avg_loss);

            // Early stopping
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
        self.lr_scheduler = Some(scheduler);
    }

    pub fn precision(&self, x: &Matriz, etiquetas: &[usize]) -> f64 {
        let preds = self.predecir_clase(x);
        let correctas = preds.iter().zip(etiquetas).filter(|(p, e)| p == e).count();
        correctas as f64 / etiquetas.len() as f64
    }

    pub fn resumen(&self) {
        println!("  Red Neuronal Artificial (ANN)");
        println!("  ─────────────────────────────");
        for (i, capa) in self.capas.iter().enumerate() {
            println!(
                "    Capa {}: {} → {} neuronas [{}]",
                i + 1,
                capa.pesos.filas,
                capa.pesos.cols,
                capa.activacion.nombre()
            );
        }
        let total_params: usize = self
            .capas
            .iter()
            .map(|c| c.pesos.filas * c.pesos.cols + c.sesgos.len())
            .sum();
        println!("    Total parámetros: {}", total_params);
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
