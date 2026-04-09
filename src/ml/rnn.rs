use serde::{Deserialize, Serialize};
use super::linalg::{Matriz, Rng, sigmoid};

// ══════════════════════════════════════════════════════════════
//  Red Neuronal Recurrente (RNN) — con variante LSTM
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TipoRNN {
    Simple,
    LSTM,
}

/// RNN simple (Elman): h_t = tanh(W_hh * h_{t-1} + W_xh * x_t + b_h)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RNN {
    pub tipo: TipoRNN,
    pub input_size: usize,
    pub hidden_size: usize,
    pub output_size: usize,
    pub tasa_aprendizaje: f64,
    pub historial_perdida: Vec<f64>,

    // Pesos RNN simple
    pub w_xh: Matriz, // input → hidden
    pub w_hh: Matriz, // hidden → hidden
    pub b_h: Vec<f64>,

    // Pesos output
    pub w_hy: Matriz, // hidden → output
    pub b_y: Vec<f64>,

    // Pesos LSTM (si aplica)
    pub lstm: Option<LSTMPesos>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LSTMPesos {
    // Forget gate
    pub w_f: Matriz,
    pub u_f: Matriz,
    pub b_f: Vec<f64>,
    // Input gate
    pub w_i: Matriz,
    pub u_i: Matriz,
    pub b_i: Vec<f64>,
    // Cell candidate
    pub w_c: Matriz,
    pub u_c: Matriz,
    pub b_c: Vec<f64>,
    // Output gate
    pub w_o: Matriz,
    pub u_o: Matriz,
    pub b_o: Vec<f64>,
}

impl RNN {
    pub fn nueva(
        tipo: TipoRNN,
        input_size: usize,
        hidden_size: usize,
        output_size: usize,
        tasa_aprendizaje: f64,
        seed: u64,
    ) -> Self {
        let mut rng = Rng::new(seed);

        let w_xh = Matriz::aleatoria(input_size, hidden_size, &mut rng);
        let w_hh = Matriz::aleatoria(hidden_size, hidden_size, &mut rng);
        let b_h = vec![0.0; hidden_size];
        let w_hy = Matriz::aleatoria(hidden_size, output_size, &mut rng);
        let b_y = vec![0.0; output_size];

        let lstm = match tipo {
            TipoRNN::LSTM => Some(LSTMPesos {
                w_f: Matriz::aleatoria(input_size, hidden_size, &mut rng),
                u_f: Matriz::aleatoria(hidden_size, hidden_size, &mut rng),
                b_f: vec![1.0; hidden_size], // bias de forget gate inicializado a 1
                w_i: Matriz::aleatoria(input_size, hidden_size, &mut rng),
                u_i: Matriz::aleatoria(hidden_size, hidden_size, &mut rng),
                b_i: vec![0.0; hidden_size],
                w_c: Matriz::aleatoria(input_size, hidden_size, &mut rng),
                u_c: Matriz::aleatoria(hidden_size, hidden_size, &mut rng),
                b_c: vec![0.0; hidden_size],
                w_o: Matriz::aleatoria(input_size, hidden_size, &mut rng),
                u_o: Matriz::aleatoria(hidden_size, hidden_size, &mut rng),
                b_o: vec![0.0; hidden_size],
            }),
            TipoRNN::Simple => None,
        };

        Self {
            tipo,
            input_size,
            hidden_size,
            output_size,
            tasa_aprendizaje,
            historial_perdida: Vec::new(),
            w_xh,
            w_hh,
            b_h,
            w_hy,
            b_y,
            lstm,
        }
    }

    /// Forward pass para una secuencia completa.
    /// `secuencia`: Vec de vectores, uno por paso temporal.
    /// Retorna: (estados_ocultos, salidas)
    pub fn forward(&self, secuencia: &[Vec<f64>]) -> (Vec<Vec<f64>>, Vec<Vec<f64>>) {
        let mut h = vec![0.0; self.hidden_size];
        let mut c_state = vec![0.0; self.hidden_size]; // para LSTM
        let mut estados = Vec::new();
        let mut salidas = Vec::new();

        for x_t in secuencia {
            match &self.lstm {
                Some(lstm) => {
                    let (h_new, c_new) = self.lstm_step(x_t, &h, &c_state, lstm);
                    h = h_new;
                    c_state = c_new;
                }
                None => {
                    h = self.rnn_step(x_t, &h);
                }
            }

            estados.push(h.clone());

            // Salida: y = W_hy * h + b_y
            let h_mat = Matriz::desde_vec(1, self.hidden_size, h.clone());
            let y_mat = h_mat.mul(&self.w_hy).sumar_fila(&self.b_y);
            salidas.push(y_mat.fila(0));
        }

        (estados, salidas)
    }

    fn rnn_step(&self, x: &[f64], h_prev: &[f64]) -> Vec<f64> {
        let x_mat = Matriz::desde_vec(1, self.input_size, x.to_vec());
        let h_mat = Matriz::desde_vec(1, self.hidden_size, h_prev.to_vec());

        let xh = x_mat.mul(&self.w_xh);
        let hh = h_mat.mul(&self.w_hh);
        let pre = xh.sumar(&hh).sumar_fila(&self.b_h);
        pre.aplicar(|v| v.tanh()).fila(0)
    }

    fn lstm_step(&self, x: &[f64], h_prev: &[f64], c_prev: &[f64], lstm: &LSTMPesos) -> (Vec<f64>, Vec<f64>) {
        let x_mat = Matriz::desde_vec(1, self.input_size, x.to_vec());
        let h_mat = Matriz::desde_vec(1, self.hidden_size, h_prev.to_vec());

        // Forget gate: f_t = σ(W_f * x_t + U_f * h_{t-1} + b_f)
        let f_t = x_mat.mul(&lstm.w_f).sumar(&h_mat.mul(&lstm.u_f)).sumar_fila(&lstm.b_f).aplicar(sigmoid);
        // Input gate: i_t = σ(W_i * x_t + U_i * h_{t-1} + b_i)
        let i_t = x_mat.mul(&lstm.w_i).sumar(&h_mat.mul(&lstm.u_i)).sumar_fila(&lstm.b_i).aplicar(sigmoid);
        // Cell candidate: c̃_t = tanh(W_c * x_t + U_c * h_{t-1} + b_c)
        let c_cand = x_mat.mul(&lstm.w_c).sumar(&h_mat.mul(&lstm.u_c)).sumar_fila(&lstm.b_c).aplicar(|v| v.tanh());
        // Output gate: o_t = σ(W_o * x_t + U_o * h_{t-1} + b_o)
        let o_t = x_mat.mul(&lstm.w_o).sumar(&h_mat.mul(&lstm.u_o)).sumar_fila(&lstm.b_o).aplicar(sigmoid);

        // Cell state: c_t = f_t ⊙ c_{t-1} + i_t ⊙ c̃_t
        let c_prev_mat = Matriz::desde_vec(1, self.hidden_size, c_prev.to_vec());
        let c_new_mat = f_t.hadamard(&c_prev_mat).sumar(&i_t.hadamard(&c_cand));
        // Hidden state: h_t = o_t ⊙ tanh(c_t)
        let h_new_mat = o_t.hadamard(&c_new_mat.aplicar(|v| v.tanh()));

        (h_new_mat.fila(0), c_new_mat.fila(0))
    }

    /// Predecir la salida del último paso temporal
    pub fn predecir(&self, secuencia: &[Vec<f64>]) -> Vec<f64> {
        let (_, salidas) = self.forward(secuencia);
        salidas.last().cloned().unwrap_or_default()
    }

    /// Entrenar con BPTT simplificado (Backprop Through Time)
    /// `secuencias`: lista de secuencias
    /// `objetivos`: salida esperada para el último paso de cada secuencia
    pub fn entrenar(
        &mut self,
        secuencias: &[Vec<Vec<f64>>],
        objetivos: &[Vec<f64>],
        epocas: usize,
    ) {
        let n = secuencias.len();

        for epoca in 0..epocas {
            let mut perdida_total = 0.0;

            for (seq, target) in secuencias.iter().zip(objetivos) {
                let (estados, salidas) = self.forward(seq);
                let pred = salidas.last().unwrap();

                // MSE loss
                let loss: f64 = pred.iter().zip(target)
                    .map(|(p, t)| (p - t).powi(2))
                    .sum::<f64>() / pred.len() as f64;
                perdida_total += loss;

                // Gradiente de salida
                let grad_y: Vec<f64> = pred.iter().zip(target)
                    .map(|(p, t)| 2.0 * (p - t) / pred.len() as f64)
                    .collect();

                // Actualizar W_hy y b_y
                let h_last = estados.last().unwrap();
                for i in 0..self.hidden_size {
                    for j in 0..self.output_size {
                        let g = grad_y[j] * h_last[i];
                        let w = self.w_hy.get(i, j);
                        self.w_hy.set(i, j, w - self.tasa_aprendizaje * g);
                    }
                }
                for j in 0..self.output_size {
                    self.b_y[j] -= self.tasa_aprendizaje * grad_y[j];
                }

                // BPTT truncado (últimos 5 pasos máximo)
                let grad_h_mat = Matriz::desde_vec(1, self.output_size, grad_y);
                let mut grad_h = grad_h_mat.mul(&self.w_hy.transpuesta()).fila(0);

                let max_bptt = seq.len().min(5);
                let t_start = seq.len().saturating_sub(max_bptt);

                for t in (t_start..seq.len()).rev() {
                    let h_t = &estados[t];
                    let x_t = &seq[t];

                    // dtanh = (1 - h_t^2) * grad_h (para RNN simple)
                    let dtanh: Vec<f64> = h_t.iter().zip(&grad_h)
                        .map(|(h, g)| (1.0 - h * h) * g)
                        .collect();

                    // Actualizar W_xh
                    for i in 0..self.input_size {
                        for j in 0..self.hidden_size {
                            let g = dtanh[j] * x_t[i];
                            let w = self.w_xh.get(i, j);
                            self.w_xh.set(i, j, w - self.tasa_aprendizaje * g.clamp(-1.0, 1.0));
                        }
                    }

                    // Actualizar W_hh
                    let h_prev = if t > 0 { &estados[t - 1] } else { &vec![0.0; self.hidden_size] };
                    for i in 0..self.hidden_size {
                        for j in 0..self.hidden_size {
                            let g = dtanh[j] * h_prev[i];
                            let w = self.w_hh.get(i, j);
                            self.w_hh.set(i, j, w - self.tasa_aprendizaje * g.clamp(-1.0, 1.0));
                        }
                    }

                    // Actualizar b_h
                    for j in 0..self.hidden_size {
                        self.b_h[j] -= self.tasa_aprendizaje * dtanh[j].clamp(-1.0, 1.0);
                    }

                    // Propagar hacia atrás en el tiempo
                    let dtanh_mat = Matriz::desde_vec(1, self.hidden_size, dtanh);
                    grad_h = dtanh_mat.mul(&self.w_hh.transpuesta()).fila(0);
                }
            }

            let avg_loss = perdida_total / n as f64;
            self.historial_perdida.push(avg_loss);

            if (epoca + 1) % (epocas / 10).max(1) == 0 || epoca == 0 {
                println!("    Época {}/{} — Pérdida: {:.6}", epoca + 1, epocas, avg_loss);
            }
        }
    }

    pub fn resumen(&self) {
        let tipo_str = match self.tipo {
            TipoRNN::Simple => "RNN Simple (Elman)",
            TipoRNN::LSTM => "LSTM",
        };
        println!("  Red Neuronal Recurrente ({})", tipo_str);
        println!("  ──────────────────────────────");
        println!("    Input: {}", self.input_size);
        println!("    Hidden: {}", self.hidden_size);
        println!("    Output: {}", self.output_size);

        let params = match &self.lstm {
            Some(_) => {
                4 * (self.input_size * self.hidden_size + self.hidden_size * self.hidden_size + self.hidden_size)
                    + self.hidden_size * self.output_size + self.output_size
            }
            None => {
                self.input_size * self.hidden_size
                    + self.hidden_size * self.hidden_size
                    + self.hidden_size
                    + self.hidden_size * self.output_size
                    + self.output_size
            }
        };
        println!("    Total parámetros: {}", params);
    }
}
