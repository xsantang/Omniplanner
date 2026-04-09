use serde::{Deserialize, Serialize};
use super::linalg::{Activacion, Matriz, Perdida, Rng};

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
    // Momentum velocidades
    vel_w: Vec<Matriz>,
    vel_b: Vec<Vec<f64>>,
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
            vel_w,
            vel_b,
        }
    }

    fn forward(&self, x: &Matriz, entrenando: bool, rng: &mut Rng) -> (Vec<Matriz>, Vec<Matriz>, Vec<Matriz>) {
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
                        .map(|_| if rng.f64() > capa.dropout { 1.0 / (1.0 - capa.dropout) } else { 0.0 })
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

        for epoca in 0..epocas {
            let mut indices: Vec<usize> = (0..n).collect();
            rng.shuffle(&mut indices);

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

                perdida_total += self.perdida.calcular(pred, &y_batch);
                batches += 1;

                // Backprop
                let mut delta = pred.restar(&y_batch);

                for l in (0..self.capas.len()).rev() {
                    // Aplicar máscara de dropout
                    delta = delta.hadamard(&masks[l]);

                    let a_prev = &a_vec[l];
                    let grad_w = a_prev.transpuesta().mul(&delta).escalar(1.0 / bs as f64);
                    let grad_b: Vec<f64> = delta.suma_columnas().iter().map(|&x| x / bs as f64).collect();

                    // Momentum SGD
                    self.vel_w[l] = self.vel_w[l]
                        .escalar(self.momentum)
                        .sumar(&grad_w.escalar(self.tasa_aprendizaje));
                    for j in 0..self.vel_b[l].len() {
                        self.vel_b[l][j] = self.momentum * self.vel_b[l][j]
                            + self.tasa_aprendizaje * grad_b[j];
                    }

                    self.capas[l].pesos = self.capas[l].pesos.restar(&self.vel_w[l]);
                    for j in 0..self.capas[l].sesgos.len() {
                        self.capas[l].sesgos[j] -= self.vel_b[l][j];
                    }

                    if l > 0 {
                        let d_act = self.capas[l - 1].activacion.derivada(&z_vec[l - 1]);
                        delta = delta.mul(&self.capas[l].pesos.transpuesta()).hadamard(&d_act);
                    }
                }
            }

            let avg_loss = perdida_total / batches as f64;
            self.historial_perdida.push(avg_loss);

            if (epoca + 1) % (epocas / 10).max(1) == 0 || epoca == 0 {
                println!("    Época {}/{} — Pérdida: {:.6}", epoca + 1, epocas, avg_loss);
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
        let total: usize = self.capas.iter()
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
