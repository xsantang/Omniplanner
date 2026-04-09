use serde::{Deserialize, Serialize};
use super::decision_tree::ArbolDecision;
use super::linalg::Rng;

// ══════════════════════════════════════════════════════════════
//  Bosque Aleatorio (Random Forest) — ensemble de árboles
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BosqueAleatorio {
    pub arboles: Vec<ArbolDecision>,
    pub num_arboles: usize,
    pub max_profundidad: usize,
    pub max_features: usize,
    pub num_clases: usize,
    pub entrenado: bool,
}

impl BosqueAleatorio {
    pub fn nuevo(
        num_arboles: usize,
        max_profundidad: usize,
        max_features: usize,
        num_clases: usize,
    ) -> Self {
        Self {
            arboles: Vec::new(),
            num_arboles,
            max_profundidad,
            max_features,
            num_clases,
            entrenado: false,
        }
    }

    pub fn entrenar(&mut self, x: &[Vec<f64>], y: &[usize], seed: u64) {
        let n = x.len();
        let mut rng = Rng::new(seed);

        self.arboles.clear();

        for i in 0..self.num_arboles {
            // Bootstrap sampling
            let mut x_boot = Vec::with_capacity(n);
            let mut y_boot = Vec::with_capacity(n);
            for _ in 0..n {
                let idx = rng.usize_rango(n);
                x_boot.push(x[idx].clone());
                y_boot.push(y[idx]);
            }

            let mut arbol = ArbolDecision::nuevo(
                self.max_profundidad,
                2,
                self.num_clases,
            );
            arbol.max_features = self.max_features;
            arbol.entrenar_con_rng(&x_boot, &y_boot, &mut rng);

            self.arboles.push(arbol);

            if (i + 1) % (self.num_arboles / 10).max(1) == 0 || i == 0 {
                println!("    Árbol {}/{} entrenado", i + 1, self.num_arboles);
            }
        }

        self.entrenado = true;
    }

    pub fn predecir(&self, x: &[f64]) -> usize {
        let mut votos = vec![0usize; self.num_clases];
        for arbol in &self.arboles {
            let pred = arbol.predecir(x);
            votos[pred] += 1;
        }
        votos.iter().enumerate()
            .max_by_key(|(_, &v)| v)
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    pub fn predecir_probabilidad(&self, x: &[f64]) -> Vec<f64> {
        let mut prob = vec![0.0; self.num_clases];
        for arbol in &self.arboles {
            let p = arbol.predecir_probabilidad(x);
            for (i, &pi) in p.iter().enumerate() {
                prob[i] += pi;
            }
        }
        let t = self.arboles.len() as f64;
        prob.iter_mut().for_each(|p| *p /= t);
        prob
    }

    pub fn predecir_lote(&self, x: &[Vec<f64>]) -> Vec<usize> {
        x.iter().map(|xi| self.predecir(xi)).collect()
    }

    pub fn precision(&self, x: &[Vec<f64>], y: &[usize]) -> f64 {
        let correctas = x.iter().zip(y).filter(|(xi, &yi)| self.predecir(xi) == yi).count();
        correctas as f64 / y.len() as f64
    }

    pub fn importancia_features(&self) -> Vec<(usize, usize)> {
        let mut conteo_total = std::collections::HashMap::new();
        for arbol in &self.arboles {
            for (feat, cnt) in arbol.importancia_features() {
                *conteo_total.entry(feat).or_insert(0usize) += cnt;
            }
        }
        let mut ranking: Vec<(usize, usize)> = conteo_total.into_iter().collect();
        ranking.sort_by(|a, b| b.1.cmp(&a.1));
        ranking
    }

    pub fn resumen(&self) {
        println!("  Bosque Aleatorio (Random Forest)");
        println!("  ────────────────────────────────");
        println!("    Número de árboles: {}", self.num_arboles);
        println!("    Max profundidad: {}", self.max_profundidad);
        println!("    Max features por split: {}", self.max_features);
        println!("    Clases: {}", self.num_clases);
        if self.entrenado {
            let prof_prom: f64 = self.arboles.iter().map(|a| a.profundidad() as f64).sum::<f64>()
                / self.arboles.len() as f64;
            println!("    Profundidad promedio: {:.1}", prof_prom);
        }
    }
}
