use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use super::linalg::{argmax, Rng};

// ══════════════════════════════════════════════════════════════
//  Aprendizaje por Refuerzo — Q-Learning tabular
//  + Deep Q-Learning simplificado
// ══════════════════════════════════════════════════════════════

/// Estado discreto representado como string (para Q-table)
pub type Estado = String;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QTable {
    pub tabla: HashMap<String, Vec<f64>>, // estado → Q-values por acción
    pub num_acciones: usize,
    pub alpha: f64,       // tasa de aprendizaje
    pub gamma: f64,       // factor de descuento
    pub epsilon: f64,     // exploración ε-greedy
    pub epsilon_min: f64,
    pub epsilon_decay: f64,
    pub episodios_entrenados: usize,
    pub historial_recompensas: Vec<f64>,
}

impl QTable {
    pub fn nueva(num_acciones: usize, alpha: f64, gamma: f64, epsilon: f64) -> Self {
        Self {
            tabla: HashMap::new(),
            num_acciones,
            alpha,
            gamma,
            epsilon,
            epsilon_min: 0.01,
            epsilon_decay: 0.995,
            episodios_entrenados: 0,
            historial_recompensas: Vec::new(),
        }
    }

    fn obtener_q(&self, estado: &str) -> Vec<f64> {
        self.tabla
            .get(estado)
            .cloned()
            .unwrap_or_else(|| vec![0.0; self.num_acciones])
    }

    fn asegurar_estado(&mut self, estado: &str) {
        if !self.tabla.contains_key(estado) {
            self.tabla.insert(estado.to_string(), vec![0.0; self.num_acciones]);
        }
    }

    pub fn elegir_accion(&self, estado: &str, rng: &mut Rng) -> usize {
        if rng.f64() < self.epsilon {
            // Exploración
            rng.usize_rango(self.num_acciones)
        } else {
            // Explotación
            let q = self.obtener_q(estado);
            argmax(&q)
        }
    }

    pub fn actualizar(
        &mut self,
        estado: &str,
        accion: usize,
        recompensa: f64,
        siguiente_estado: &str,
        terminal: bool,
    ) {
        self.asegurar_estado(estado);
        self.asegurar_estado(siguiente_estado);

        let q_actual = self.tabla[estado][accion];
        let q_max_siguiente = if terminal {
            0.0
        } else {
            let qs = self.obtener_q(siguiente_estado);
            qs.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
        };

        let q_nuevo = q_actual + self.alpha * (recompensa + self.gamma * q_max_siguiente - q_actual);

        self.tabla.get_mut(estado).unwrap()[accion] = q_nuevo;
    }

    pub fn decay_epsilon(&mut self) {
        self.epsilon = (self.epsilon * self.epsilon_decay).max(self.epsilon_min);
    }

    pub fn mejor_accion(&self, estado: &str) -> usize {
        argmax(&self.obtener_q(estado))
    }

    pub fn resumen(&self) {
        println!("  Q-Learning");
        println!("  ──────────");
        println!("    Estados conocidos: {}", self.tabla.len());
        println!("    Acciones: {}", self.num_acciones);
        println!("    α (aprendizaje): {}", self.alpha);
        println!("    γ (descuento): {}", self.gamma);
        println!("    ε (exploración): {:.4}", self.epsilon);
        println!("    Episodios entrenados: {}", self.episodios_entrenados);
    }
}

// ══════════════════════════════════════════════════════════════
//  Entorno de ejemplo: GridWorld
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GridWorld {
    pub filas: usize,
    pub cols: usize,
    pub pos_agente: (usize, usize),
    pub pos_meta: (usize, usize),
    pub obstaculos: Vec<(usize, usize)>,
    pub recompensa_meta: f64,
    pub recompensa_paso: f64,
    pub recompensa_obstaculo: f64,
}

impl GridWorld {
    pub fn nuevo(filas: usize, cols: usize, meta: (usize, usize)) -> Self {
        Self {
            filas,
            cols,
            pos_agente: (0, 0),
            pos_meta: meta,
            obstaculos: Vec::new(),
            recompensa_meta: 10.0,
            recompensa_paso: -0.1,
            recompensa_obstaculo: -5.0,
        }
    }

    pub fn con_obstaculos(mut self, obs: Vec<(usize, usize)>) -> Self {
        self.obstaculos = obs;
        self
    }

    pub fn reset(&mut self) -> Estado {
        self.pos_agente = (0, 0);
        self.estado_str()
    }

    pub fn estado_str(&self) -> Estado {
        format!("{},{}", self.pos_agente.0, self.pos_agente.1)
    }

    /// Acciones: 0=arriba, 1=abajo, 2=izquierda, 3=derecha
    pub fn step(&mut self, accion: usize) -> (Estado, f64, bool) {
        let (f, c) = self.pos_agente;
        let nueva_pos = match accion {
            0 => (f.saturating_sub(1), c),
            1 => ((f + 1).min(self.filas - 1), c),
            2 => (f, c.saturating_sub(1)),
            3 => (f, (c + 1).min(self.cols - 1)),
            _ => (f, c),
        };

        self.pos_agente = nueva_pos;

        if self.pos_agente == self.pos_meta {
            (self.estado_str(), self.recompensa_meta, true)
        } else if self.obstaculos.contains(&self.pos_agente) {
            (self.estado_str(), self.recompensa_obstaculo, false)
        } else {
            (self.estado_str(), self.recompensa_paso, false)
        }
    }

    pub fn entrenar_agente(&mut self, q: &mut QTable, episodios: usize, max_pasos: usize) {
        let mut rng = Rng::new(42);

        for ep in 0..episodios {
            let mut estado = self.reset();
            let mut recompensa_total = 0.0;

            for _ in 0..max_pasos {
                let accion = q.elegir_accion(&estado, &mut rng);
                let (sig_estado, recompensa, terminal) = self.step(accion);
                q.actualizar(&estado, accion, recompensa, &sig_estado, terminal);
                recompensa_total += recompensa;
                estado = sig_estado;
                if terminal { break; }
            }

            q.decay_epsilon();
            q.historial_recompensas.push(recompensa_total);
            q.episodios_entrenados += 1;

            if (ep + 1) % (episodios / 10).max(1) == 0 || ep == 0 {
                let avg: f64 = q.historial_recompensas
                    [q.historial_recompensas.len().saturating_sub(100)..]
                    .iter()
                    .sum::<f64>()
                    / q.historial_recompensas.len().min(100) as f64;
                println!(
                    "    Episodio {}/{} — Recompensa promedio(100): {:.2} — ε: {:.4}",
                    ep + 1, episodios, avg, q.epsilon
                );
            }
        }
    }

    pub fn mostrar_politica(&self, q: &QTable) {
        let flechas = ["↑", "↓", "←", "→"];
        println!();
        for f in 0..self.filas {
            let mut linea = String::new();
            for c in 0..self.cols {
                if (f, c) == self.pos_meta {
                    linea.push_str(" ★ ");
                } else if self.obstaculos.contains(&(f, c)) {
                    linea.push_str(" ▓ ");
                } else {
                    let estado = format!("{},{}", f, c);
                    let accion = q.mejor_accion(&estado);
                    linea.push_str(&format!(" {} ", flechas[accion]));
                }
            }
            println!("    {}", linea);
        }
        println!();
    }
}

// ══════════════════════════════════════════════════════════════
//  Entorno de ejemplo: Bandit Multi-brazo
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MultiBandit {
    pub probabilidades: Vec<f64>,
    pub recompensas_acumuladas: Vec<f64>,
    pub conteos: Vec<usize>,
}

impl MultiBandit {
    pub fn nuevo(probabilidades: Vec<f64>) -> Self {
        let n = probabilidades.len();
        Self {
            probabilidades,
            recompensas_acumuladas: vec![0.0; n],
            conteos: vec![0; n],
        }
    }

    pub fn pull(&mut self, brazo: usize, rng: &mut Rng) -> f64 {
        let recompensa = if rng.f64() < self.probabilidades[brazo] { 1.0 } else { 0.0 };
        self.conteos[brazo] += 1;
        self.recompensas_acumuladas[brazo] += recompensa;
        recompensa
    }

    pub fn mejor_brazo(&self) -> usize {
        self.probabilidades
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    pub fn entrenar_epsilon_greedy(&mut self, episodios: usize, epsilon: f64) -> Vec<f64> {
        let mut rng = Rng::new(42);
        let n = self.probabilidades.len();
        let mut valores_estimados = vec![0.0f64; n];
        let mut historial = Vec::new();
        let mut recompensa_total = 0.0;

        for ep in 0..episodios {
            let brazo = if rng.f64() < epsilon {
                rng.usize_rango(n)
            } else {
                valores_estimados
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            };

            let recompensa = self.pull(brazo, &mut rng);
            recompensa_total += recompensa;

            // Actualización incremental
            let n_pulls = self.conteos[brazo] as f64;
            valores_estimados[brazo] += (recompensa - valores_estimados[brazo]) / n_pulls;

            historial.push(recompensa_total / (ep + 1) as f64);
        }

        historial
    }
}
