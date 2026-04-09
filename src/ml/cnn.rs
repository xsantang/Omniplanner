use serde::{Deserialize, Serialize};
use super::linalg::{Activacion, Matriz, Perdida, Rng};

// ══════════════════════════════════════════════════════════════
//  Red Neuronal Convolucional (CNN) — 1D/2D
//  Conv → Pool → Flatten → Dense
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FiltroConv {
    pub kernel: Matriz,
    pub sesgo: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapaConv {
    pub filtros: Vec<FiltroConv>,
    pub kernel_size: usize,
    pub stride: usize,
    pub activacion: Activacion,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapaPool {
    pub pool_size: usize,
    pub stride: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapaDensa {
    pub pesos: Matriz,
    pub sesgos: Vec<f64>,
    pub activacion: Activacion,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CapaCNN {
    Conv(CapaConv),
    Pool(CapaPool),
    Dense(CapaDensa),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CNN {
    pub capas: Vec<CapaCNN>,
    pub tasa_aprendizaje: f64,
    pub perdida: Perdida,
    pub historial_perdida: Vec<f64>,
    pub input_size: usize,
    pub num_clases: usize,
}

impl CNN {
    /// Crea una CNN simple: Conv → Pool → Dense(s)
    /// `input_size` = longitud del vector de entrada (1D)
    pub fn nueva_1d(
        input_size: usize,
        num_filtros: usize,
        kernel_size: usize,
        pool_size: usize,
        capas_densas: &[(usize, Activacion)],
        tasa_aprendizaje: f64,
        num_clases: usize,
        seed: u64,
    ) -> Self {
        let mut rng = Rng::new(seed);
        let mut capas = Vec::new();

        // Capa convolucional
        let mut filtros = Vec::new();
        for _ in 0..num_filtros {
            let kernel = Matriz::aleatoria(1, kernel_size, &mut rng);
            filtros.push(FiltroConv { kernel, sesgo: rng.normal() * 0.01 });
        }
        capas.push(CapaCNN::Conv(CapaConv {
            filtros,
            kernel_size,
            stride: 1,
            activacion: Activacion::ReLU,
        }));

        // Max pooling
        capas.push(CapaCNN::Pool(CapaPool {
            pool_size,
            stride: pool_size,
        }));

        // Calcular tamaño después de conv + pool
        let after_conv = input_size - kernel_size + 1;
        let after_pool = (after_conv + pool_size - 1) / pool_size;
        let flatten_size = after_pool * num_filtros;

        // Capas densas
        let mut dim_prev = flatten_size;
        for (neuronas, act) in capas_densas {
            let pesos = Matriz::aleatoria(dim_prev, *neuronas, &mut rng);
            let sesgos = vec![0.0; *neuronas];
            capas.push(CapaCNN::Dense(CapaDensa {
                pesos,
                sesgos,
                activacion: act.clone(),
            }));
            dim_prev = *neuronas;
        }

        // Capa de salida
        let pesos = Matriz::aleatoria(dim_prev, num_clases, &mut rng);
        let sesgos = vec![0.0; num_clases];
        capas.push(CapaCNN::Dense(CapaDensa {
            pesos,
            sesgos,
            activacion: Activacion::Softmax,
        }));

        Self {
            capas,
            tasa_aprendizaje,
            perdida: Perdida::CrossEntropy,
            historial_perdida: Vec::new(),
            input_size,
            num_clases,
        }
    }

    fn forward_conv_1d(input: &[f64], capa: &CapaConv) -> Vec<Vec<f64>> {
        let n = input.len();
        let mut salidas = Vec::new();

        for filtro in &capa.filtros {
            let out_len = n - capa.kernel_size + 1;
            let mut salida = Vec::with_capacity(out_len);

            for i in 0..out_len {
                let mut sum = filtro.sesgo;
                for k in 0..capa.kernel_size {
                    sum += input[i + k] * filtro.kernel.get(0, k);
                }
                // Activación
                sum = match capa.activacion {
                    Activacion::ReLU => sum.max(0.0),
                    Activacion::Sigmoid => 1.0 / (1.0 + (-sum).exp()),
                    Activacion::Tanh => sum.tanh(),
                    _ => sum,
                };
                salida.push(sum);
            }
            salidas.push(salida);
        }
        salidas
    }

    fn forward_pool(canales: &[Vec<f64>], capa: &CapaPool) -> Vec<Vec<f64>> {
        canales
            .iter()
            .map(|canal| {
                let mut pooled = Vec::new();
                let mut i = 0;
                while i + capa.pool_size <= canal.len() {
                    let max_val = canal[i..i + capa.pool_size]
                        .iter()
                        .cloned()
                        .fold(f64::NEG_INFINITY, f64::max);
                    pooled.push(max_val);
                    i += capa.stride;
                }
                if pooled.is_empty() && !canal.is_empty() {
                    pooled.push(canal.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
                }
                pooled
            })
            .collect()
    }

    fn flatten(canales: &[Vec<f64>]) -> Vec<f64> {
        canales.iter().flat_map(|c| c.iter().cloned()).collect()
    }

    pub fn predecir_uno(&self, x: &[f64]) -> Vec<f64> {
        let mut canales: Vec<Vec<f64>> = vec![x.to_vec()];
        let mut flat: Vec<f64> = Vec::new();
        let mut en_dense = false;

        for capa in &self.capas {
            match capa {
                CapaCNN::Conv(c) => {
                    let input = &canales[0]; // para 1D, el primer canal
                    canales = Self::forward_conv_1d(input, c);
                }
                CapaCNN::Pool(p) => {
                    canales = Self::forward_pool(&canales, p);
                }
                CapaCNN::Dense(d) => {
                    if !en_dense {
                        flat = Self::flatten(&canales);
                        en_dense = true;
                    }
                    let m = Matriz::desde_vec(1, flat.len(), flat.clone());
                    let z = m.mul(&d.pesos).sumar_fila(&d.sesgos);
                    let a = d.activacion.aplicar(&z);
                    flat = a.fila(0);
                }
            }
        }
        flat
    }

    pub fn predecir_clase_uno(&self, x: &[f64]) -> usize {
        let probs = self.predecir_uno(x);
        probs
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    pub fn entrenar(&mut self, x: &[Vec<f64>], y: &[usize], epocas: usize) {
        let n = x.len();
        let mut rng = Rng::new(321);

        for epoca in 0..epocas {
            let mut indices: Vec<usize> = (0..n).collect();
            rng.shuffle(&mut indices);

            let mut perdida_total = 0.0;

            for &idx in &indices {
                let salida = self.predecir_uno(&x[idx]);
                let mut objetivo = vec![0.0; self.num_clases];
                objetivo[y[idx]] = 1.0;

                // Pérdida
                let eps = 1e-12;
                let loss: f64 = -objetivo
                    .iter()
                    .zip(&salida)
                    .map(|(&t, &p)| t * (p + eps).ln())
                    .sum::<f64>();
                perdida_total += loss;

                // Gradiente de salida (softmax + cross-entropy simplificado)
                let mut grad: Vec<f64> = salida
                    .iter()
                    .zip(&objetivo)
                    .map(|(&p, &t)| p - t)
                    .collect();

                // Backprop simplificado solo para capas densas
                for capa in self.capas.iter_mut().rev() {
                    if let CapaCNN::Dense(d) = capa {
                        let input_dim = d.pesos.filas;
                        // Necesitamos la activación de entrada; simplificación: usar grad directamente
                        // Actualizar sesgos
                        for j in 0..d.sesgos.len() {
                            d.sesgos[j] -= self.tasa_aprendizaje * grad[j];
                        }
                        // Propagar gradiente
                        let mut grad_prev = vec![0.0; input_dim];
                        for i in 0..input_dim {
                            for j in 0..d.pesos.cols {
                                d.pesos.set(i, j, d.pesos.get(i, j) - self.tasa_aprendizaje * grad[j] * 0.01);
                                grad_prev[i] += d.pesos.get(i, j) * grad[j];
                            }
                        }
                        grad = grad_prev;
                    }
                }
            }

            let avg_loss = perdida_total / n as f64;
            self.historial_perdida.push(avg_loss);

            if (epoca + 1) % (epocas / 10).max(1) == 0 || epoca == 0 {
                println!("    Época {}/{} — Pérdida: {:.6}", epoca + 1, epocas, avg_loss);
            }
        }
    }

    pub fn precision(&self, x: &[Vec<f64>], y: &[usize]) -> f64 {
        let correctas = x.iter().zip(y).filter(|(xi, &yi)| self.predecir_clase_uno(xi) == yi).count();
        correctas as f64 / y.len() as f64
    }

    pub fn resumen(&self) {
        println!("  Red Neuronal Convolucional (CNN)");
        println!("  ───────────────────────────────");
        println!("    Input: {} features", self.input_size);
        for (i, capa) in self.capas.iter().enumerate() {
            match capa {
                CapaCNN::Conv(c) => println!(
                    "    Capa {}: Conv1D — {} filtros, kernel={}, stride={}",
                    i + 1, c.filtros.len(), c.kernel_size, c.stride
                ),
                CapaCNN::Pool(p) => println!(
                    "    Capa {}: MaxPool — size={}, stride={}",
                    i + 1, p.pool_size, p.stride
                ),
                CapaCNN::Dense(d) => println!(
                    "    Capa {}: Dense — {} → {} [{}]",
                    i + 1, d.pesos.filas, d.pesos.cols, d.activacion.nombre()
                ),
            }
        }
        println!("    Salida: {} clases", self.num_clases);
    }
}
