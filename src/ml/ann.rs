use serde::{Deserialize, Serialize};
use super::linalg::{Activacion, Matriz, Perdida, Rng};

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
}

#[derive(Clone, Debug)]
struct CacheForward {
    z: Vec<Matriz>, // pre-activación
    a: Vec<Matriz>, // post-activación
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

        CacheForward { z: z_vec, a: a_vec }
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

        for epoca in 0..epocas {
            let mut indices: Vec<usize> = (0..n).collect();
            rng.shuffle(&mut indices);

            let mut perdida_total = 0.0;
            let mut batches = 0;

            for chunk_start in (0..n).step_by(batch_size) {
                let chunk_end = (chunk_start + batch_size).min(n);
                let batch_idx: Vec<usize> = indices[chunk_start..chunk_end].to_vec();
                let bs = batch_idx.len();

                // Extraer batch
                let x_batch = extraer_filas(x, &batch_idx);
                let y_batch = extraer_filas(y, &batch_idx);

                // Forward
                let cache = self.forward(&x_batch);
                let pred = cache.a.last().unwrap();

                perdida_total += self.perdida.calcular(pred, &y_batch);
                batches += 1;

                // Backpropagation
                let mut delta = pred.restar(&y_batch); // para softmax+CE o sigmoid+MSE simplificado

                for l in (0..self.capas.len()).rev() {
                    let a_prev = &cache.a[l];

                    // Gradientes
                    let grad_w = a_prev.transpuesta().mul(&delta).escalar(1.0 / bs as f64);
                    let grad_b = delta.suma_columnas().iter().map(|&x| x / bs as f64).collect::<Vec<f64>>();

                    // Actualizar pesos
                    self.capas[l].pesos = self.capas[l]
                        .pesos
                        .restar(&grad_w.escalar(self.tasa_aprendizaje));
                    for j in 0..self.capas[l].sesgos.len() {
                        self.capas[l].sesgos[j] -= self.tasa_aprendizaje * grad_b[j];
                    }

                    // Propagar delta hacia atrás
                    if l > 0 {
                        let d_act = self.capas[l - 1].activacion.derivada(&cache.z[l - 1]);
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
        let total_params: usize = self.capas.iter()
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
