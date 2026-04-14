use super::linalg::{Activacion, Matriz, Perdida, Rng};
use super::optimizer::{
    EarlyStopping, EstadoAdam, EstadoAdamVec, LRSchedule, LRScheduler, RegularizacionL2,
    TipoOptimizador,
};
use serde::{Deserialize, Serialize};

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

#[derive(Clone)]
enum CacheCapaCNN {
    Conv {
        input: Vec<f64>,
        pre_act: Vec<Vec<f64>>,
    },
    Pool {
        input_channels: Vec<Vec<f64>>,
        max_indices: Vec<Vec<usize>>,
    },
    Dense {
        input: Vec<f64>,
        pre_act: Vec<f64>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CNN {
    pub capas: Vec<CapaCNN>,
    pub tasa_aprendizaje: f64,
    pub perdida: Perdida,
    pub historial_perdida: Vec<f64>,
    pub input_size: usize,
    pub num_clases: usize,
    pub optimizador: TipoOptimizador,
    pub lr_schedule: LRSchedule,
    pub l2: Option<RegularizacionL2>,
    pub early_stopping: Option<EarlyStopping>,
    #[serde(skip)]
    adam_dense: Vec<EstadoAdam>,
    #[serde(skip)]
    adam_conv: Vec<Vec<EstadoAdamVec>>, // por capa conv, por filtro
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
            filtros.push(FiltroConv {
                kernel,
                sesgo: rng.normal() * 0.01,
            });
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
        let after_pool = if after_conv >= pool_size {
            (after_conv - pool_size) / pool_size + 1
        } else {
            1 // fallback: al menos un valor por pooling
        };
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
            optimizador: TipoOptimizador::SGD,
            lr_schedule: LRSchedule::Constante,
            l2: None,
            early_stopping: None,
            adam_dense: Vec::new(),
            adam_conv: Vec::new(),
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

    /// Inicializa estados Adam para todas las capas
    fn init_adam(&mut self) {
        if let TipoOptimizador::Adam(..) = &self.optimizador {
            if self.adam_dense.is_empty() && self.adam_conv.is_empty() {
                for capa in &self.capas {
                    match capa {
                        CapaCNN::Dense(d) => {
                            self.adam_dense
                                .push(EstadoAdam::nuevo(d.pesos.filas, d.pesos.cols));
                            // placeholder para mantener alineación conv
                            self.adam_conv.push(Vec::new());
                        }
                        CapaCNN::Conv(c) => {
                            // placeholder para mantener alineación dense
                            self.adam_dense.push(EstadoAdam::nuevo(0, 0));
                            let filtro_states: Vec<EstadoAdamVec> = c
                                .filtros
                                .iter()
                                .map(|f| EstadoAdamVec::nuevo(f.kernel.cols))
                                .collect();
                            self.adam_conv.push(filtro_states);
                        }
                        CapaCNN::Pool(_) => {
                            self.adam_dense.push(EstadoAdam::nuevo(0, 0));
                            self.adam_conv.push(Vec::new());
                        }
                    }
                }
            }
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

    /// Forward pass guardando intermedios para backpropagation completa
    fn forward_cache(&self, x: &[f64]) -> (Vec<f64>, Vec<CacheCapaCNN>) {
        let mut caches: Vec<CacheCapaCNN> = Vec::new();
        let mut canales: Vec<Vec<f64>> = vec![x.to_vec()];
        let mut flat: Vec<f64> = Vec::new();
        let mut en_dense = false;

        for capa in &self.capas {
            match capa {
                CapaCNN::Conv(c) => {
                    let input = canales[0].clone();
                    let n = input.len();
                    let out_len = if n >= c.kernel_size {
                        n - c.kernel_size + 1
                    } else {
                        0
                    };
                    let mut pre_act_all = Vec::new();
                    let mut post_act_all = Vec::new();

                    for filtro in &c.filtros {
                        let mut pre = Vec::with_capacity(out_len);
                        let mut post = Vec::with_capacity(out_len);
                        for i in 0..out_len {
                            let mut sum = filtro.sesgo;
                            for k in 0..c.kernel_size {
                                sum += input[i + k] * filtro.kernel.get(0, k);
                            }
                            pre.push(sum);
                            let activated = match c.activacion {
                                Activacion::ReLU => sum.max(0.0),
                                Activacion::Sigmoid => 1.0 / (1.0 + (-sum).exp()),
                                Activacion::Tanh => sum.tanh(),
                                Activacion::LeakyReLU => {
                                    if sum > 0.0 {
                                        sum
                                    } else {
                                        0.01 * sum
                                    }
                                }
                                _ => sum,
                            };
                            post.push(activated);
                        }
                        pre_act_all.push(pre);
                        post_act_all.push(post);
                    }

                    caches.push(CacheCapaCNN::Conv {
                        input,
                        pre_act: pre_act_all,
                    });
                    canales = post_act_all;
                }
                CapaCNN::Pool(p) => {
                    let mut pooled = Vec::new();
                    let mut indices = Vec::new();

                    for canal in &canales {
                        let mut p_out = Vec::new();
                        let mut p_idx = Vec::new();
                        let mut i = 0;
                        while i + p.pool_size <= canal.len() {
                            let slice = &canal[i..i + p.pool_size];
                            let (max_pos, &max_val) = slice
                                .iter()
                                .enumerate()
                                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                                .unwrap();
                            p_out.push(max_val);
                            p_idx.push(i + max_pos);
                            i += p.stride;
                        }
                        if p_out.is_empty() && !canal.is_empty() {
                            let (max_pos, &max_val) = canal
                                .iter()
                                .enumerate()
                                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                                .unwrap();
                            p_out.push(max_val);
                            p_idx.push(max_pos);
                        }
                        pooled.push(p_out);
                        indices.push(p_idx);
                    }

                    caches.push(CacheCapaCNN::Pool {
                        input_channels: canales.clone(),
                        max_indices: indices,
                    });
                    canales = pooled;
                }
                CapaCNN::Dense(d) => {
                    if !en_dense {
                        flat = Self::flatten(&canales);
                        en_dense = true;
                    }
                    let input = flat.clone();
                    let m = d.pesos.cols;
                    let mut z = vec![0.0; m];
                    for j in 0..m {
                        z[j] = d.sesgos[j];
                        for ii in 0..input.len() {
                            z[j] += input[ii] * d.pesos.get(ii, j);
                        }
                    }
                    caches.push(CacheCapaCNN::Dense {
                        input,
                        pre_act: z.clone(),
                    });
                    let z_mat = Matriz::desde_vec(1, m, z);
                    let a_mat = d.activacion.aplicar(&z_mat);
                    flat = a_mat.fila(0);
                }
            }
        }

        (flat, caches)
    }

    pub fn entrenar(&mut self, x: &[Vec<f64>], y: &[usize], epocas: usize) {
        let n = x.len();
        let mut rng = Rng::new(321);
        let n_capas = self.capas.len();
        self.init_adam();
        let mut scheduler = LRScheduler::nuevo(self.tasa_aprendizaje, self.lr_schedule.clone());

        for epoca in 0..epocas {
            let mut indices: Vec<usize> = (0..n).collect();
            rng.shuffle(&mut indices);

            let lr = scheduler.lr_actual;
            let mut perdida_total = 0.0;

            for &idx in &indices {
                let (salida, caches) = self.forward_cache(&x[idx]);

                // Pérdida cross-entropy
                let mut objetivo = vec![0.0; self.num_clases];
                objetivo[y[idx]] = 1.0;
                let eps = 1e-12;
                let mut loss: f64 = -objetivo
                    .iter()
                    .zip(&salida)
                    .map(|(&t, &p)| t * (p + eps).ln())
                    .sum::<f64>();

                // L2 penalty en capas densas
                if let Some(ref l2) = self.l2 {
                    for capa in &self.capas {
                        if let CapaCNN::Dense(d) = capa {
                            loss += l2.penalty(&d.pesos);
                        }
                    }
                }
                perdida_total += loss;

                // Gradiente inicial: softmax + cross-entropy → p - t
                let mut grad: Vec<f64> =
                    salida.iter().zip(&objetivo).map(|(&p, &t)| p - t).collect();

                let use_adam = matches!(&self.optimizador, TipoOptimizador::Adam(..));

                // Backpropagation completa por todas las capas
                for layer_idx in (0..n_capas).rev() {
                    let cache = caches[layer_idx].clone();
                    match (&mut self.capas[layer_idx], &cache) {
                        (CapaCNN::Dense(d), CacheCapaCNN::Dense { input, pre_act }) => {
                            // Para capas ocultas, aplicar derivada de activación
                            let dz: Vec<f64> = if layer_idx == n_capas - 1 {
                                grad.clone() // softmax+CE: grad ya es dL/dz
                            } else {
                                let mut dz_v = Vec::with_capacity(grad.len());
                                for j in 0..grad.len() {
                                    let z = pre_act[j];
                                    let deriv = match d.activacion {
                                        Activacion::ReLU => {
                                            if z > 0.0 {
                                                1.0
                                            } else {
                                                0.0
                                            }
                                        }
                                        Activacion::Sigmoid => {
                                            let s = 1.0 / (1.0 + (-z).exp());
                                            s * (1.0 - s)
                                        }
                                        Activacion::Tanh => 1.0 - z.tanh().powi(2),
                                        Activacion::LeakyReLU => {
                                            if z > 0.0 {
                                                1.0
                                            } else {
                                                0.01
                                            }
                                        }
                                        _ => 1.0,
                                    };
                                    dz_v.push(grad[j] * deriv);
                                }
                                dz_v
                            };

                            // Propagar gradiente ANTES de actualizar pesos
                            let n_in = input.len();
                            let mut grad_prev = vec![0.0; n_in];
                            for i in 0..n_in {
                                for j in 0..d.pesos.cols {
                                    grad_prev[i] += d.pesos.get(i, j) * dz[j];
                                }
                            }

                            // Actualizar pesos según optimizador
                            if use_adam {
                                if let TipoOptimizador::Adam(beta1, beta2, eps) = &self.optimizador
                                {
                                    let mut grad_w = Matriz::nueva(n_in, d.pesos.cols);
                                    for i in 0..n_in {
                                        for j in 0..d.pesos.cols {
                                            let mut g = input[i] * dz[j];
                                            if let Some(ref l2) = self.l2 {
                                                g += l2.lambda * d.pesos.get(i, j);
                                            }
                                            grad_w.set(i, j, g);
                                        }
                                    }
                                    let (new_w, new_b) = self.adam_dense[layer_idx].actualizar(
                                        &d.pesos, &d.sesgos, &grad_w, &dz, lr, *beta1, *beta2, *eps,
                                    );
                                    d.pesos = new_w;
                                    d.sesgos = new_b;
                                }
                            } else {
                                for i in 0..n_in {
                                    for j in 0..d.pesos.cols {
                                        let mut g = input[i] * dz[j];
                                        if let Some(ref l2) = self.l2 {
                                            g += l2.lambda * d.pesos.get(i, j);
                                        }
                                        d.pesos.set(i, j, d.pesos.get(i, j) - lr * g);
                                    }
                                }
                                for j in 0..d.sesgos.len() {
                                    d.sesgos[j] -= lr * dz[j];
                                }
                            }

                            grad = grad_prev;
                        }

                        (
                            CapaCNN::Pool(_),
                            CacheCapaCNN::Pool {
                                input_channels,
                                max_indices,
                            },
                        ) => {
                            // Unflatten grad y rutear a posiciones del máximo
                            let mut offset = 0;
                            let mut d_input_chs: Vec<Vec<f64>> = Vec::new();

                            for (ch, ch_indices) in max_indices.iter().enumerate() {
                                let ch_len = input_channels[ch].len();
                                let mut d_ch = vec![0.0; ch_len];
                                for (p, &max_idx) in ch_indices.iter().enumerate() {
                                    if offset + p < grad.len() {
                                        d_ch[max_idx] += grad[offset + p];
                                    }
                                }
                                offset += ch_indices.len();
                                d_input_chs.push(d_ch);
                            }

                            grad = d_input_chs
                                .into_iter()
                                .flat_map(|v| v.into_iter())
                                .collect();
                        }

                        (CapaCNN::Conv(c), CacheCapaCNN::Conv { input, pre_act }) => {
                            let n_filtros = c.filtros.len();
                            let out_len = if !pre_act.is_empty() {
                                pre_act[0].len()
                            } else {
                                0
                            };

                            // Separar grad por filtro
                            let mut grad_per_f: Vec<Vec<f64>> = Vec::new();
                            for f in 0..n_filtros {
                                let start = f * out_len;
                                let end = start + out_len;
                                grad_per_f.push(if end <= grad.len() {
                                    grad[start..end].to_vec()
                                } else {
                                    vec![0.0; out_len]
                                });
                            }

                            // Aplicar derivada de activación
                            for f in 0..n_filtros {
                                for i in 0..out_len {
                                    let z = pre_act[f][i];
                                    let deriv = match c.activacion {
                                        Activacion::ReLU => {
                                            if z > 0.0 {
                                                1.0
                                            } else {
                                                0.0
                                            }
                                        }
                                        Activacion::Sigmoid => {
                                            let s = 1.0 / (1.0 + (-z).exp());
                                            s * (1.0 - s)
                                        }
                                        Activacion::Tanh => 1.0 - z.tanh().powi(2),
                                        Activacion::LeakyReLU => {
                                            if z > 0.0 {
                                                1.0
                                            } else {
                                                0.01
                                            }
                                        }
                                        _ => 1.0,
                                    };
                                    grad_per_f[f][i] *= deriv;
                                }
                            }

                            // Actualizar filtros y sesgos
                            for f in 0..n_filtros {
                                let mut kernel_grad = vec![0.0; c.kernel_size];
                                for k in 0..c.kernel_size {
                                    for i in 0..out_len {
                                        kernel_grad[k] += grad_per_f[f][i] * input[i + k];
                                    }
                                }
                                let g_bias: f64 = grad_per_f[f].iter().sum();

                                if use_adam {
                                    if let TipoOptimizador::Adam(beta1, beta2, eps) =
                                        &self.optimizador
                                    {
                                        let kernel_data: Vec<f64> = (0..c.kernel_size)
                                            .map(|k| c.filtros[f].kernel.get(0, k))
                                            .collect();
                                        let (new_k, new_b) = self.adam_conv[layer_idx][f]
                                            .actualizar(
                                                &kernel_data,
                                                c.filtros[f].sesgo,
                                                &kernel_grad,
                                                g_bias,
                                                lr,
                                                *beta1,
                                                *beta2,
                                                *eps,
                                            );
                                        for k in 0..c.kernel_size {
                                            c.filtros[f].kernel.set(0, k, new_k[k]);
                                        }
                                        c.filtros[f].sesgo = new_b;
                                    }
                                } else {
                                    for k in 0..c.kernel_size {
                                        let old = c.filtros[f].kernel.get(0, k);
                                        c.filtros[f].kernel.set(0, k, old - lr * kernel_grad[k]);
                                    }
                                    c.filtros[f].sesgo -= lr * g_bias;
                                }
                            }
                        }

                        _ => {}
                    }
                }
            }

            let avg_loss = perdida_total / n as f64;
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

    pub fn precision(&self, x: &[Vec<f64>], y: &[usize]) -> f64 {
        let correctas = x
            .iter()
            .zip(y)
            .filter(|(xi, &yi)| self.predecir_clase_uno(xi) == yi)
            .count();
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
                    i + 1,
                    c.filtros.len(),
                    c.kernel_size,
                    c.stride
                ),
                CapaCNN::Pool(p) => println!(
                    "    Capa {}: MaxPool — size={}, stride={}",
                    i + 1,
                    p.pool_size,
                    p.stride
                ),
                CapaCNN::Dense(d) => println!(
                    "    Capa {}: Dense — {} → {} [{}]",
                    i + 1,
                    d.pesos.filas,
                    d.pesos.cols,
                    d.activacion.nombre()
                ),
            }
        }
        println!("    Salida: {} clases", self.num_clases);
    }
}
