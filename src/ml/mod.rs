pub mod linalg;
pub mod ann;
pub mod svm;
pub mod decision_tree;
pub mod random_forest;
pub mod dnn;
pub mod cnn;
pub mod rnn;
pub mod reinforcement;

// Re-exports para conveniencia
pub use linalg::{Activacion, Matriz, Perdida, Rng};
pub use ann::ANN;
pub use svm::{SVM, SVMMulticlase};
pub use decision_tree::ArbolDecision;
pub use random_forest::BosqueAleatorio;
pub use dnn::DNN;
pub use cnn::CNN;
pub use rnn::{RNN, TipoRNN};
pub use reinforcement::{QTable, GridWorld, MultiBandit};

use serde::{Deserialize, Serialize};

// ══════════════════════════════════════════════════════════════
//  Dataset genérico para ML
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dataset {
    pub nombre: String,
    pub features: Vec<Vec<f64>>,
    pub etiquetas: Vec<usize>,
    pub nombres_features: Vec<String>,
    pub nombres_clases: Vec<String>,
}

impl Dataset {
    pub fn nuevo(nombre: &str) -> Self {
        Self {
            nombre: nombre.to_string(),
            features: Vec::new(),
            etiquetas: Vec::new(),
            nombres_features: Vec::new(),
            nombres_clases: Vec::new(),
        }
    }

    pub fn agregar_muestra(&mut self, features: Vec<f64>, etiqueta: usize) {
        self.features.push(features);
        self.etiquetas.push(etiqueta);
    }

    pub fn num_muestras(&self) -> usize {
        self.features.len()
    }

    pub fn num_features(&self) -> usize {
        self.features.first().map(|f| f.len()).unwrap_or(0)
    }

    pub fn num_clases(&self) -> usize {
        self.etiquetas.iter().max().map(|&m| m + 1).unwrap_or(0)
    }

    /// Divide en entrenamiento/test
    pub fn dividir(&self, ratio_train: f64, seed: u64) -> (Dataset, Dataset) {
        let n = self.num_muestras();
        let mut rng = Rng::new(seed);
        let mut indices: Vec<usize> = (0..n).collect();
        rng.shuffle(&mut indices);

        let n_train = (n as f64 * ratio_train) as usize;

        let mut train = Dataset::nuevo(&format!("{}_train", self.nombre));
        train.nombres_features = self.nombres_features.clone();
        train.nombres_clases = self.nombres_clases.clone();

        let mut test = Dataset::nuevo(&format!("{}_test", self.nombre));
        test.nombres_features = self.nombres_features.clone();
        test.nombres_clases = self.nombres_clases.clone();

        for (i, &idx) in indices.iter().enumerate() {
            if i < n_train {
                train.agregar_muestra(self.features[idx].clone(), self.etiquetas[idx]);
            } else {
                test.agregar_muestra(self.features[idx].clone(), self.etiquetas[idx]);
            }
        }

        (train, test)
    }

    /// Convertir a Matriz para ANNs
    pub fn a_matriz(&self) -> Matriz {
        let filas = self.num_muestras();
        let cols = self.num_features();
        let datos: Vec<f64> = self.features.iter().flat_map(|f| f.iter().cloned()).collect();
        Matriz::desde_vec(filas, cols, datos)
    }

    /// Convertir etiquetas a one-hot
    pub fn etiquetas_one_hot(&self) -> Matriz {
        let n = self.num_muestras();
        let c = self.num_clases();
        let mut datos = vec![0.0; n * c];
        for (i, &e) in self.etiquetas.iter().enumerate() {
            datos[i * c + e] = 1.0;
        }
        Matriz::desde_vec(n, c, datos)
    }

    /// Normalizar features (min-max a [0,1])
    pub fn normalizar(&mut self) {
        if self.features.is_empty() { return; }
        let dim = self.num_features();
        for j in 0..dim {
            let min = self.features.iter().map(|f| f[j]).fold(f64::INFINITY, f64::min);
            let max = self.features.iter().map(|f| f[j]).fold(f64::NEG_INFINITY, f64::max);
            let rango = max - min;
            if rango > 1e-10 {
                for f in &mut self.features {
                    f[j] = (f[j] - min) / rango;
                }
            }
        }
    }

    pub fn resumen(&self) {
        println!("  Dataset: {}", self.nombre);
        println!("  ─────────────────");
        println!("    Muestras: {}", self.num_muestras());
        println!("    Features: {}", self.num_features());
        println!("    Clases: {}", self.num_clases());
        if !self.nombres_clases.is_empty() {
            for (i, nombre) in self.nombres_clases.iter().enumerate() {
                let count = self.etiquetas.iter().filter(|&&e| e == i).count();
                println!("      Clase {} ({}): {} muestras", i, nombre, count);
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════
//  Datasets de ejemplo generados proceduralmente
// ══════════════════════════════════════════════════════════════

pub fn dataset_iris_sintetico(seed: u64) -> Dataset {
    let mut rng = Rng::new(seed);
    let mut ds = Dataset::nuevo("Iris Sintético");
    ds.nombres_features = vec![
        "sépalo largo".into(), "sépalo ancho".into(),
        "pétalo largo".into(), "pétalo ancho".into(),
    ];
    ds.nombres_clases = vec!["Setosa".into(), "Versicolor".into(), "Virginica".into()];

    // Clase 0: Setosa (features pequeños)
    for _ in 0..50 {
        ds.agregar_muestra(vec![
            5.0 + rng.normal() * 0.35,
            3.4 + rng.normal() * 0.38,
            1.5 + rng.normal() * 0.17,
            0.2 + rng.normal() * 0.10,
        ], 0);
    }
    // Clase 1: Versicolor
    for _ in 0..50 {
        ds.agregar_muestra(vec![
            5.9 + rng.normal() * 0.52,
            2.8 + rng.normal() * 0.31,
            4.3 + rng.normal() * 0.47,
            1.3 + rng.normal() * 0.20,
        ], 1);
    }
    // Clase 2: Virginica
    for _ in 0..50 {
        ds.agregar_muestra(vec![
            6.6 + rng.normal() * 0.64,
            3.0 + rng.normal() * 0.32,
            5.6 + rng.normal() * 0.55,
            2.0 + rng.normal() * 0.27,
        ], 2);
    }

    ds
}

pub fn dataset_xor(seed: u64) -> Dataset {
    let mut rng = Rng::new(seed);
    let mut ds = Dataset::nuevo("XOR");
    ds.nombres_features = vec!["x".into(), "y".into()];
    ds.nombres_clases = vec!["0".into(), "1".into()];

    for _ in 0..100 {
        let x = rng.rango(-1.0, 1.0);
        let y = rng.rango(-1.0, 1.0);
        let clase = if (x > 0.0) ^ (y > 0.0) { 1 } else { 0 };
        ds.agregar_muestra(vec![x, y], clase);
    }

    ds
}

pub fn dataset_circulos(seed: u64) -> Dataset {
    let mut rng = Rng::new(seed);
    let mut ds = Dataset::nuevo("Círculos concéntricos");
    ds.nombres_features = vec!["x".into(), "y".into()];
    ds.nombres_clases = vec!["interior".into(), "exterior".into()];

    for _ in 0..200 {
        let angulo = rng.rango(0.0, 2.0 * std::f64::consts::PI);
        let ruido = rng.normal() * 0.1;

        if rng.f64() < 0.5 {
            let r = 0.5 + ruido;
            ds.agregar_muestra(vec![r * angulo.cos(), r * angulo.sin()], 0);
        } else {
            let r = 1.5 + ruido;
            ds.agregar_muestra(vec![r * angulo.cos(), r * angulo.sin()], 1);
        }
    }

    ds
}

pub fn dataset_secuencia_temporal(seed: u64) -> (Vec<Vec<Vec<f64>>>, Vec<Vec<f64>>) {
    let mut rng = Rng::new(seed);
    let mut secuencias = Vec::new();
    let mut objetivos = Vec::new();

    for _ in 0..100 {
        let base = rng.rango(0.0, 5.0);
        let mut seq = Vec::new();
        for t in 0..10 {
            let val = base + (t as f64) * 0.5 + rng.normal() * 0.1;
            seq.push(vec![val]);
        }
        let target = vec![base + 10.0 * 0.5]; // predicción del siguiente valor
        secuencias.push(seq);
        objetivos.push(target);
    }

    (secuencias, objetivos)
}

// ══════════════════════════════════════════════════════════════
//  Contenedor de modelos entrenados (para persistencia)
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModeloML {
    pub id: String,
    pub nombre: String,
    pub tipo: TipoModelo,
    pub creado: String,
    pub precision_train: Option<f64>,
    pub precision_test: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TipoModelo {
    ANN(ANN),
    SVM(SVM),
    SVMMulti(SVMMulticlase),
    ArbolDecision(ArbolDecision),
    BosqueAleatorio(BosqueAleatorio),
    DNN(DNN),
    CNN(CNN),
    RNN(RNN),
    QLearning(QTable),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AlmacenML {
    pub modelos: Vec<ModeloML>,
    pub datasets: Vec<Dataset>,
}

impl AlmacenML {
    pub fn agregar_modelo(&mut self, modelo: ModeloML) {
        self.modelos.push(modelo);
    }

    pub fn agregar_dataset(&mut self, dataset: Dataset) {
        self.datasets.push(dataset);
    }
}
