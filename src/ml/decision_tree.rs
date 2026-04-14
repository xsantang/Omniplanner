use super::linalg::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ══════════════════════════════════════════════════════════════
//  Árbol de Decisión — clasificación (CART con Gini)
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodoArbol {
    Hoja {
        clase: usize,
        distribucion: Vec<f64>,
    },
    Interno {
        feature: usize,
        umbral: f64,
        izquierda: Box<NodoArbol>,
        derecha: Box<NodoArbol>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArbolDecision {
    pub raiz: Option<NodoArbol>,
    pub max_profundidad: usize,
    pub min_muestras_split: usize,
    pub num_clases: usize,
    pub num_features: usize,
    /// Si > 0, sólo considerar un subconjunto aleatorio de features (para Random Forest)
    pub max_features: usize,
}

impl ArbolDecision {
    pub fn nuevo(max_profundidad: usize, min_muestras_split: usize, num_clases: usize) -> Self {
        Self {
            raiz: None,
            max_profundidad,
            min_muestras_split,
            num_clases,
            num_features: 0,
            max_features: 0,
        }
    }

    pub fn entrenar(&mut self, x: &[Vec<f64>], y: &[usize]) {
        self.num_features = x[0].len();
        if self.max_features == 0 {
            self.max_features = self.num_features;
        }
        let mut rng = Rng::new(42);
        self.raiz = Some(self.construir(x, y, 0, &mut rng));
    }

    pub fn entrenar_con_rng(&mut self, x: &[Vec<f64>], y: &[usize], rng: &mut Rng) {
        self.num_features = x[0].len();
        if self.max_features == 0 {
            self.max_features = self.num_features;
        }
        self.raiz = Some(self.construir(x, y, 0, rng));
    }

    fn construir(
        &self,
        x: &[Vec<f64>],
        y: &[usize],
        profundidad: usize,
        rng: &mut Rng,
    ) -> NodoArbol {
        let n = y.len();

        // Distribución de clases
        let mut conteo = vec![0usize; self.num_clases];
        for &yi in y {
            conteo[yi] += 1;
        }
        let total = n as f64;
        let distribucion: Vec<f64> = conteo.iter().map(|&c| c as f64 / total).collect();

        // Condiciones hoja
        let clase_mayoritaria = conteo
            .iter()
            .enumerate()
            .max_by_key(|(_, &c)| c)
            .map(|(i, _)| i)
            .unwrap_or(0);

        if profundidad >= self.max_profundidad
            || n < self.min_muestras_split
            || conteo.iter().filter(|&&c| c > 0).count() <= 1
        {
            return NodoArbol::Hoja {
                clase: clase_mayoritaria,
                distribucion,
            };
        }

        // Seleccionar features candidatos
        let features_candidatos = if self.max_features < self.num_features {
            let mut feats: Vec<usize> = (0..self.num_features).collect();
            rng.shuffle(&mut feats);
            feats.truncate(self.max_features);
            feats
        } else {
            (0..self.num_features).collect()
        };

        // Buscar mejor split
        let mut mejor_gini = f64::MAX;
        let mut mejor_feature = 0;
        let mut mejor_umbral = 0.0;

        for &feat in &features_candidatos {
            let mut valores: Vec<f64> = x.iter().map(|xi| xi[feat]).collect();
            valores.sort_by(|a, b| a.partial_cmp(b).unwrap());
            valores.dedup();

            for i in 0..valores.len().saturating_sub(1) {
                let umbral = (valores[i] + valores[i + 1]) / 2.0;

                let (mut izq_conteo, mut der_conteo) =
                    (vec![0usize; self.num_clases], vec![0usize; self.num_clases]);
                let (mut n_izq, mut n_der) = (0usize, 0usize);

                for (j, xi) in x.iter().enumerate() {
                    if xi[feat] <= umbral {
                        izq_conteo[y[j]] += 1;
                        n_izq += 1;
                    } else {
                        der_conteo[y[j]] += 1;
                        n_der += 1;
                    }
                }

                if n_izq == 0 || n_der == 0 {
                    continue;
                }

                let gini_izq = gini_impureza(&izq_conteo, n_izq);
                let gini_der = gini_impureza(&der_conteo, n_der);
                let gini_ponderado = (n_izq as f64 * gini_izq + n_der as f64 * gini_der) / n as f64;

                if gini_ponderado < mejor_gini {
                    mejor_gini = gini_ponderado;
                    mejor_feature = feat;
                    mejor_umbral = umbral;
                }
            }
        }

        // Si no mejoró, crear hoja
        if mejor_gini >= gini_impureza(&conteo, n) - 1e-10 {
            return NodoArbol::Hoja {
                clase: clase_mayoritaria,
                distribucion,
            };
        }

        // Particionar
        let (mut x_izq, mut y_izq, mut x_der, mut y_der) = (vec![], vec![], vec![], vec![]);
        for (i, xi) in x.iter().enumerate() {
            if xi[mejor_feature] <= mejor_umbral {
                x_izq.push(xi.clone());
                y_izq.push(y[i]);
            } else {
                x_der.push(xi.clone());
                y_der.push(y[i]);
            }
        }

        NodoArbol::Interno {
            feature: mejor_feature,
            umbral: mejor_umbral,
            izquierda: Box::new(self.construir(&x_izq, &y_izq, profundidad + 1, rng)),
            derecha: Box::new(self.construir(&x_der, &y_der, profundidad + 1, rng)),
        }
    }

    pub fn predecir(&self, x: &[f64]) -> usize {
        match &self.raiz {
            Some(nodo) => predecir_nodo(nodo, x),
            None => 0,
        }
    }

    pub fn predecir_probabilidad(&self, x: &[f64]) -> Vec<f64> {
        match &self.raiz {
            Some(nodo) => probabilidad_nodo(nodo, x),
            None => vec![0.0; self.num_clases],
        }
    }

    pub fn predecir_lote(&self, x: &[Vec<f64>]) -> Vec<usize> {
        x.iter().map(|xi| self.predecir(xi)).collect()
    }

    pub fn precision(&self, x: &[Vec<f64>], y: &[usize]) -> f64 {
        let correctas = x
            .iter()
            .zip(y)
            .filter(|(xi, &yi)| self.predecir(xi) == yi)
            .count();
        correctas as f64 / y.len() as f64
    }

    pub fn profundidad(&self) -> usize {
        match &self.raiz {
            Some(nodo) => profundidad_nodo(nodo),
            None => 0,
        }
    }

    pub fn num_hojas(&self) -> usize {
        match &self.raiz {
            Some(nodo) => contar_hojas(nodo),
            None => 0,
        }
    }

    pub fn importancia_features(&self) -> HashMap<usize, usize> {
        let mut conteo = HashMap::new();
        if let Some(ref nodo) = self.raiz {
            contar_features(nodo, &mut conteo);
        }
        conteo
    }

    pub fn resumen(&self) {
        println!("  Árbol de Decisión");
        println!("  ─────────────────");
        println!("    Max profundidad: {}", self.max_profundidad);
        println!("    Profundidad real: {}", self.profundidad());
        println!("    Hojas: {}", self.num_hojas());
        println!("    Clases: {}", self.num_clases);
    }
}

fn predecir_nodo(nodo: &NodoArbol, x: &[f64]) -> usize {
    match nodo {
        NodoArbol::Hoja { clase, .. } => *clase,
        NodoArbol::Interno {
            feature,
            umbral,
            izquierda,
            derecha,
        } => {
            if x[*feature] <= *umbral {
                predecir_nodo(izquierda, x)
            } else {
                predecir_nodo(derecha, x)
            }
        }
    }
}

fn probabilidad_nodo(nodo: &NodoArbol, x: &[f64]) -> Vec<f64> {
    match nodo {
        NodoArbol::Hoja { distribucion, .. } => distribucion.clone(),
        NodoArbol::Interno {
            feature,
            umbral,
            izquierda,
            derecha,
        } => {
            if x[*feature] <= *umbral {
                probabilidad_nodo(izquierda, x)
            } else {
                probabilidad_nodo(derecha, x)
            }
        }
    }
}

fn profundidad_nodo(nodo: &NodoArbol) -> usize {
    match nodo {
        NodoArbol::Hoja { .. } => 1,
        NodoArbol::Interno {
            izquierda, derecha, ..
        } => 1 + profundidad_nodo(izquierda).max(profundidad_nodo(derecha)),
    }
}

fn contar_hojas(nodo: &NodoArbol) -> usize {
    match nodo {
        NodoArbol::Hoja { .. } => 1,
        NodoArbol::Interno {
            izquierda, derecha, ..
        } => contar_hojas(izquierda) + contar_hojas(derecha),
    }
}

fn contar_features(nodo: &NodoArbol, conteo: &mut HashMap<usize, usize>) {
    if let NodoArbol::Interno {
        feature,
        izquierda,
        derecha,
        ..
    } = nodo
    {
        *conteo.entry(*feature).or_insert(0) += 1;
        contar_features(izquierda, conteo);
        contar_features(derecha, conteo);
    }
}

fn gini_impureza(conteo: &[usize], total: usize) -> f64 {
    if total == 0 {
        return 0.0;
    }
    let t = total as f64;
    1.0 - conteo
        .iter()
        .map(|&c| {
            let p = c as f64 / t;
            p * p
        })
        .sum::<f64>()
}
