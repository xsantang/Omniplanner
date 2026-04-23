#![allow(clippy::needless_range_loop, clippy::large_enum_variant)]

//! Machine Learning — implementaciones en Rust puro sin dependencias externas.
//!
//! Incluye redes neuronales (ANN, DNN, CNN, RNN/LSTM), clasificadores
//! (SVM, árboles, bosques), RL (Q-Learning, bandits), optimizadores
//! (Adam, LR scheduling), y utilidades (k-fold CV, datasets sintéticos).

pub mod advisor;
pub mod ann;
pub mod cnn;
pub mod decision_tree;
pub mod dnn;
pub mod linalg;
pub mod optimizer;
pub mod presupuesto_cero;
pub mod random_forest;
pub mod reinforcement;
pub mod rnn;
pub mod svm;

// Re-exports para conveniencia
pub use advisor::{
    AhorroPagoExtra, AjusteMensualLibertad, AlmacenAsesor, AnalisisDeuda, CategoriaEscenario,
    ComparacionPlanes, ComparacionRapida, CorteBancario, CriterioDecision, DecisionPago,
    DeudaRastreada, DiagnosticoGlobal, DiagnosticoMes, DiccionarioAcciones, ErrorPago, Escenario,
    EstadoDeudaUi, EstrategiaLibertad, FrecuenciaPago, ImpactoAccion, IngresoRastreado,
    MatrizDecision, MesPago, MesSimulado, MetaAhorro, Movimiento, Presupuesto, RastreadorDeudas,
    RecomendacionPagoExtra, RegistroAsesor, ResumenDeuda, SimulacionLibertad,
    SimulacionLiquidacion, TipoRegistro,
};
pub use ann::ANN;
pub use cnn::CNN;
pub use decision_tree::ArbolDecision;
pub use dnn::DNN;
pub use linalg::{Activacion, Matriz, Perdida, Rng};
pub use optimizer::{
    BatchNorm, EarlyStopping, EstadoAdam, EstadoAdamVec, LRSchedule, LRScheduler, RegularizacionL2,
    ResultadoCV, TipoOptimizador,
};
pub use presupuesto_cero::AlmacenPresupuesto;
pub use random_forest::BosqueAleatorio;
pub use reinforcement::{GridWorld, MultiBandit, QTable};
pub use rnn::{TipoRNN, RNN};
pub use svm::{SVMMulticlase, SVM};

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
        let datos: Vec<f64> = self
            .features
            .iter()
            .flat_map(|f| f.iter().cloned())
            .collect();
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
        if self.features.is_empty() {
            return;
        }
        let dim = self.num_features();
        for j in 0..dim {
            let min = self
                .features
                .iter()
                .map(|f| f[j])
                .fold(f64::INFINITY, f64::min);
            let max = self
                .features
                .iter()
                .map(|f| f[j])
                .fold(f64::NEG_INFINITY, f64::max);
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

    /// K-Fold Cross-Validation: divide el dataset en k pliegues.
    /// Devuelve un Vec de k tuplas (train, test), uno por cada fold.
    pub fn kfold(&self, k: usize, seed: u64) -> Vec<(Dataset, Dataset)> {
        assert!(k >= 2, "k debe ser >= 2");
        let n = self.num_muestras();
        assert!(k <= n, "k no puede superar el número de muestras");

        let mut rng = Rng::new(seed);
        let mut indices: Vec<usize> = (0..n).collect();
        rng.shuffle(&mut indices);

        let fold_size = n / k;
        let mut folds = Vec::with_capacity(k);

        for i in 0..k {
            let start = i * fold_size;
            let end = if i == k - 1 { n } else { start + fold_size };

            let mut train = Dataset::nuevo(&format!("{}_fold{}_train", self.nombre, i));
            train.nombres_features = self.nombres_features.clone();
            train.nombres_clases = self.nombres_clases.clone();

            let mut test = Dataset::nuevo(&format!("{}_fold{}_test", self.nombre, i));
            test.nombres_features = self.nombres_features.clone();
            test.nombres_clases = self.nombres_clases.clone();

            for (j, &idx) in indices.iter().enumerate() {
                if j >= start && j < end {
                    test.agregar_muestra(self.features[idx].clone(), self.etiquetas[idx]);
                } else {
                    train.agregar_muestra(self.features[idx].clone(), self.etiquetas[idx]);
                }
            }

            folds.push((train, test));
        }

        folds
    }

    /// K-Fold estratificado: mantiene la proporción de clases en cada fold.
    pub fn kfold_estratificado(&self, k: usize, seed: u64) -> Vec<(Dataset, Dataset)> {
        assert!(k >= 2, "k debe ser >= 2");
        let n = self.num_muestras();
        let num_clases = self.num_clases();
        assert!(k <= n, "k no puede superar el número de muestras");

        let mut rng = Rng::new(seed);

        // Agrupar índices por clase
        let mut por_clase: Vec<Vec<usize>> = vec![Vec::new(); num_clases];
        for (i, &e) in self.etiquetas.iter().enumerate() {
            por_clase[e].push(i);
        }
        // Barajar cada clase
        for grupo in &mut por_clase {
            rng.shuffle(grupo);
        }

        // Asignar cada muestra a un fold (round-robin por clase)
        let mut asignacion_fold = vec![0usize; n];
        for grupo in &por_clase {
            for (j, &idx) in grupo.iter().enumerate() {
                asignacion_fold[idx] = j % k;
            }
        }

        let mut folds = Vec::with_capacity(k);
        for fold_i in 0..k {
            let mut train = Dataset::nuevo(&format!("{}_sfold{}_train", self.nombre, fold_i));
            train.nombres_features = self.nombres_features.clone();
            train.nombres_clases = self.nombres_clases.clone();

            let mut test = Dataset::nuevo(&format!("{}_sfold{}_test", self.nombre, fold_i));
            test.nombres_features = self.nombres_features.clone();
            test.nombres_clases = self.nombres_clases.clone();

            for idx in 0..n {
                if asignacion_fold[idx] == fold_i {
                    test.agregar_muestra(self.features[idx].clone(), self.etiquetas[idx]);
                } else {
                    train.agregar_muestra(self.features[idx].clone(), self.etiquetas[idx]);
                }
            }

            folds.push((train, test));
        }

        folds
    }
}

// ══════════════════════════════════════════════════════════════
//  Datasets de ejemplo generados proceduralmente
// ══════════════════════════════════════════════════════════════

pub fn dataset_iris_sintetico(seed: u64) -> Dataset {
    let mut rng = Rng::new(seed);
    let mut ds = Dataset::nuevo("Iris Sintético");
    ds.nombres_features = vec![
        "sépalo largo".into(),
        "sépalo ancho".into(),
        "pétalo largo".into(),
        "pétalo ancho".into(),
    ];
    ds.nombres_clases = vec!["Setosa".into(), "Versicolor".into(), "Virginica".into()];

    // Clase 0: Setosa (features pequeños)
    for _ in 0..50 {
        ds.agregar_muestra(
            vec![
                5.0 + rng.normal() * 0.35,
                3.4 + rng.normal() * 0.38,
                1.5 + rng.normal() * 0.17,
                0.2 + rng.normal() * 0.10,
            ],
            0,
        );
    }
    // Clase 1: Versicolor
    for _ in 0..50 {
        ds.agregar_muestra(
            vec![
                5.9 + rng.normal() * 0.52,
                2.8 + rng.normal() * 0.31,
                4.3 + rng.normal() * 0.47,
                1.3 + rng.normal() * 0.20,
            ],
            1,
        );
    }
    // Clase 2: Virginica
    for _ in 0..50 {
        ds.agregar_muestra(
            vec![
                6.6 + rng.normal() * 0.64,
                3.0 + rng.normal() * 0.32,
                5.6 + rng.normal() * 0.55,
                2.0 + rng.normal() * 0.27,
            ],
            2,
        );
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── 1. ANN — Red Neuronal Artificial ──
    #[test]
    fn test_ann_aprende_xor() {
        let mut ds = dataset_xor(42);
        ds.normalizar();
        let x = ds.a_matriz();
        let y = ds.etiquetas_one_hot();

        let capas = vec![(8, Activacion::ReLU), (2, Activacion::Softmax)];
        let mut ann = ANN::nueva(2, &capas, 0.05, Perdida::CrossEntropy, 42);
        ann.entrenar(&x, &y, 200, 16);

        let precision = ann.precision(&x, &ds.etiquetas);
        println!("  ANN precisión en XOR: {:.1}%", precision * 100.0);
        assert!(
            precision > 0.70,
            "ANN debería superar 70% en XOR, obtuvo {:.1}%",
            precision * 100.0
        );
    }

    // ── 2. SVM — Máquina de Vectores de Soporte ──
    #[test]
    fn test_svm_clasifica_binario() {
        let mut rng = Rng::new(42);
        let mut x = Vec::new();
        let mut y = Vec::new();
        for _ in 0..100 {
            let v = vec![rng.normal() + 2.0, rng.normal() + 2.0];
            x.push(v);
            y.push(1.0);
        }
        for _ in 0..100 {
            let v = vec![rng.normal() - 2.0, rng.normal() - 2.0];
            x.push(v);
            y.push(-1.0);
        }

        let mut svm = SVM::nuevo(2, 1.0, 0.001);
        svm.entrenar(&x, &y, 100);

        let precision = svm.precision(&x, &y);
        println!("  SVM precisión binaria: {:.1}%", precision * 100.0);
        assert!(
            precision > 0.90,
            "SVM debería superar 90%, obtuvo {:.1}%",
            precision * 100.0
        );
        assert!(svm.entrenado);
    }

    // ── 3. Árbol de Decisión ──
    #[test]
    fn test_arbol_decision_iris() {
        let mut ds = dataset_iris_sintetico(42);
        ds.normalizar();
        let (train, test) = ds.dividir(0.8, 42);

        let mut arbol = ArbolDecision::nuevo(8, 2, 3);
        arbol.entrenar(&train.features, &train.etiquetas);

        let prec_train = arbol.precision(&train.features, &train.etiquetas);
        let prec_test = arbol.precision(&test.features, &test.etiquetas);

        println!(
            "  Árbol — train: {:.1}%  test: {:.1}%  prof: {}  hojas: {}",
            prec_train * 100.0,
            prec_test * 100.0,
            arbol.profundidad(),
            arbol.num_hojas()
        );

        assert!(prec_train > 0.85, "Árbol train > 85%");
        assert!(prec_test > 0.60, "Árbol test > 60%");
        assert!(arbol.profundidad() > 0);
        assert!(arbol.num_hojas() > 1);
    }

    // ── 4. Bosque Aleatorio ──
    #[test]
    fn test_bosque_aleatorio_iris() {
        let mut ds = dataset_iris_sintetico(42);
        ds.normalizar();
        let (train, test) = ds.dividir(0.8, 42);

        let mut bosque = BosqueAleatorio::nuevo(20, 8, 2, 3);
        bosque.entrenar(&train.features, &train.etiquetas, 42);

        let prec_train = bosque.precision(&train.features, &train.etiquetas);
        let prec_test = bosque.precision(&test.features, &test.etiquetas);

        println!(
            "  Bosque — train: {:.1}%  test: {:.1}%",
            prec_train * 100.0,
            prec_test * 100.0
        );

        assert!(prec_train > 0.85, "Bosque train > 85%");
        assert!(prec_test > 0.60, "Bosque test > 60%");
        assert!(bosque.entrenado);

        let imp = bosque.importancia_features();
        assert!(!imp.is_empty(), "Debería tener importancia de features");
    }

    // ── 5. DNN — Red Neuronal Profunda ──
    #[test]
    fn test_dnn_con_dropout() {
        let mut ds = dataset_iris_sintetico(42);
        ds.normalizar();
        let x = ds.a_matriz();
        let y = ds.etiquetas_one_hot();

        let capas = vec![
            (16, Activacion::ReLU, 0.1),
            (8, Activacion::ReLU, 0.1),
            (3, Activacion::Softmax, 0.0),
        ];
        let mut dnn = DNN::nueva(4, &capas, 0.01, 0.9, Perdida::CrossEntropy, 42);
        dnn.entrenar(&x, &y, 150, 16);

        let precision = dnn.precision(&x, &ds.etiquetas);
        println!("  DNN precisión Iris: {:.1}%", precision * 100.0);
        assert!(precision > 0.60, "DNN debería superar 60% en Iris");
        assert!(!dnn.historial_perdida.is_empty());
    }

    // ── 6. CNN — Red Convolucional ──
    #[test]
    fn test_cnn_1d() {
        let mut ds = dataset_iris_sintetico(42);
        ds.normalizar();
        let (train, _test) = ds.dividir(0.8, 42);

        let mut cnn = CNN::nueva_1d(
            4, // input_size (4 features de iris)
            4, // filtros
            2, // kernel
            2, // pool
            &[(8, Activacion::ReLU)],
            0.01,
            3, // clases
            42,
        );
        cnn.entrenar(&train.features, &train.etiquetas, 50);

        let prec = cnn.precision(&train.features, &train.etiquetas);
        println!("  CNN precisión train: {:.1}%", prec * 100.0);
        // CNN 1D con tan pocas features es limitada, solo verificar que no crashea y aprende algo
        assert!(prec > 0.33, "CNN debería superar azar (33%)");
        assert!(!cnn.historial_perdida.is_empty());
    }

    // ── 7. RNN — Red Recurrente ──
    #[test]
    fn test_rnn_secuencia() {
        let (seqs, objs) = dataset_secuencia_temporal(42);
        let n = 50;
        let seq_train = &seqs[..n];
        let obj_train = &objs[..n];
        let seq_test = &seqs[n..n + 10];
        let obj_test = &objs[n..n + 10];

        let mut rnn = RNN::nueva(TipoRNN::Simple, 1, 8, 1, 0.005, 42);
        rnn.entrenar(seq_train, obj_train, 80);

        // Medir MSE en test
        let mut mse = 0.0;
        for (s, o) in seq_test.iter().zip(obj_test) {
            let pred = rnn.predecir(s);
            mse += (pred[0] - o[0]).powi(2);
        }
        mse /= seq_test.len() as f64;

        println!("  RNN MSE test: {:.4}", mse);
        assert!(!rnn.historial_perdida.is_empty());
        // La pérdida debería decrecer
        let primera = rnn.historial_perdida.first().unwrap();
        let ultima = rnn.historial_perdida.last().unwrap();
        println!("  RNN pérdida: {:.4} → {:.4}", primera, ultima);
        assert!(ultima < primera, "La pérdida de RNN debería decrecer");
    }

    // ── 7b. LSTM ──
    #[test]
    fn test_lstm_secuencia() {
        let (seqs, objs) = dataset_secuencia_temporal(99);
        let seq_train = &seqs[..50];
        let obj_train = &objs[..50];

        let mut lstm = RNN::nueva(TipoRNN::LSTM, 1, 8, 1, 0.005, 99);
        lstm.entrenar(seq_train, obj_train, 50);

        let pred = lstm.predecir(&seqs[80]);
        println!(
            "  LSTM objetivo: {:.3}  predicción: {:.3}",
            objs[80][0], pred[0]
        );
        assert!(!lstm.historial_perdida.is_empty());
    }

    // ── 8. Q-Learning (Reinforcement Learning) ──
    #[test]
    fn test_qlearning_gridworld() {
        let mut grid = GridWorld::nuevo(4, 4, (3, 3)).con_obstaculos(vec![(1, 1), (2, 2)]);
        let mut q = QTable::nueva(4, 0.1, 0.99, 1.0);

        grid.entrenar_agente(&mut q, 2000, 50);

        // Verificar que aprendió algo
        assert!(q.tabla.len() > 5, "Debería conocer varios estados");
        assert!(q.episodios_entrenados == 2000);
        assert!(q.epsilon < 1.0, "Epsilon debería haber decaído");

        // La recompensa promedio debería mejorar
        let primeras: f64 = q.historial_recompensas[..100].iter().sum::<f64>() / 100.0;
        let ultimas: f64 = q.historial_recompensas[1900..].iter().sum::<f64>() / 100.0;
        println!("  Q-Learning recompensa: {:.2} → {:.2}", primeras, ultimas);
        assert!(
            ultimas > primeras,
            "El agente debería mejorar con el tiempo"
        );

        // Verificar que la mejor acción desde (0,0) tiene sentido (debería ir → o ↓)
        let accion = q.mejor_accion("0,0");
        assert!(accion == 1 || accion == 3, "Desde (0,0) debería ir ↓ o →");
    }

    // ── 8b. Multi-Armed Bandit ──
    #[test]
    fn test_multi_bandit() {
        let probs = vec![0.2, 0.5, 0.8, 0.1, 0.3];
        let mut bandit = MultiBandit::nuevo(probs);
        let historial = bandit.entrenar_epsilon_greedy(5000, 0.1);

        assert_eq!(bandit.mejor_brazo(), 2, "Brazo con prob 0.8 es el mejor");
        // El brazo más tirado debería ser el 2
        let mas_tirado = bandit
            .conteos
            .iter()
            .enumerate()
            .max_by_key(|(_, &c)| c)
            .map(|(i, _)| i)
            .unwrap();
        println!(
            "  Bandit: mejor brazo real=2, más tirado={}, tiradas={:?}",
            mas_tirado, bandit.conteos
        );
        assert_eq!(mas_tirado, 2, "El agente debería explotar el brazo 2");
        assert!(
            historial.last().unwrap() > &0.5,
            "Recompensa promedio > 0.5"
        );
    }

    // ── Dataset y utilidades ──
    #[test]
    fn test_dataset_operaciones() {
        let ds = dataset_iris_sintetico(42);
        assert_eq!(ds.num_muestras(), 150);
        assert_eq!(ds.num_features(), 4);
        assert_eq!(ds.num_clases(), 3);

        let (train, test) = ds.dividir(0.8, 42);
        assert_eq!(train.num_muestras(), 120);
        assert_eq!(test.num_muestras(), 30);

        let mat = train.a_matriz();
        assert_eq!(mat.filas, 120);
        assert_eq!(mat.cols, 4);

        let oh = train.etiquetas_one_hot();
        assert_eq!(oh.filas, 120);
        assert_eq!(oh.cols, 3);
    }

    #[test]
    fn test_matriz_operaciones() {
        let a = Matriz::desde_vec(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let b = Matriz::desde_vec(3, 2, vec![7.0, 8.0, 9.0, 10.0, 11.0, 12.0]);
        let c = a.mul(&b);
        assert_eq!(c.filas, 2);
        assert_eq!(c.cols, 2);
        assert_eq!(c.get(0, 0), 58.0); // 1*7 + 2*9 + 3*11
        assert_eq!(c.get(0, 1), 64.0); // 1*8 + 2*10 + 3*12

        let id = Matriz::identidad(3);
        assert_eq!(id.get(0, 0), 1.0);
        assert_eq!(id.get(0, 1), 0.0);

        let t = a.transpuesta();
        assert_eq!(t.filas, 3);
        assert_eq!(t.cols, 2);
        assert_eq!(t.get(0, 0), 1.0);
        assert_eq!(t.get(0, 1), 4.0);
    }

    // ── K-Fold Cross-Validation ──
    #[test]
    fn test_kfold_basico() {
        let ds = dataset_iris_sintetico(42);
        let folds = ds.kfold(5, 42);

        assert_eq!(folds.len(), 5);
        // Cada fold: ~120 train + ~30 test = 150 total
        for (i, (train, test)) in folds.iter().enumerate() {
            assert_eq!(
                train.num_muestras() + test.num_muestras(),
                150,
                "Fold {} no suma 150 muestras",
                i
            );
            assert!(
                test.num_muestras() >= 29,
                "Fold {} test muy pequeño: {}",
                i,
                test.num_muestras()
            );
            assert!(
                train.num_muestras() >= 119,
                "Fold {} train muy pequeño: {}",
                i,
                train.num_muestras()
            );
        }
    }

    #[test]
    fn test_kfold_estratificado() {
        let ds = dataset_iris_sintetico(42);
        let folds = ds.kfold_estratificado(5, 42);

        assert_eq!(folds.len(), 5);
        for (i, (train, test)) in folds.iter().enumerate() {
            assert_eq!(
                train.num_muestras() + test.num_muestras(),
                150,
                "Sfold {} no suma 150",
                i
            );
            // Cada fold test debe tener clases representadas
            let clases_test = test.num_clases();
            assert!(
                clases_test >= 2,
                "Sfold {} test tiene pocas clases: {}",
                i,
                clases_test
            );
        }
    }

    #[test]
    fn test_kfold_cv_con_bosque() {
        let mut ds = dataset_iris_sintetico(42);
        ds.normalizar();
        let folds = ds.kfold_estratificado(5, 42);

        let mut cv = ResultadoCV::nuevo(5);
        for (train, test) in &folds {
            let mut bosque = BosqueAleatorio::nuevo(10, 6, 2, 3);
            bosque.entrenar(&train.features, &train.etiquetas, 42);

            let prec_train = bosque.precision(&train.features, &train.etiquetas);
            let prec_test = bosque.precision(&test.features, &test.etiquetas);
            cv.agregar_fold(prec_train, prec_test, 0.0);
        }

        let resumen = cv.resumen();
        println!("  {}", resumen);

        assert!(
            cv.media_test() > 0.60,
            "CV media test debe superar 60%, got {:.1}%",
            cv.media_test() * 100.0
        );
        assert!(
            cv.desviacion_test() < 0.30,
            "CV desviación test debe ser < 30%"
        );
        assert_eq!(cv.precisiones_test.len(), 5);
    }

    #[test]
    fn test_kfold_cv_con_ann() {
        let mut ds = dataset_iris_sintetico(42);
        ds.normalizar();
        let folds = ds.kfold(3, 42);

        let mut cv = ResultadoCV::nuevo(3);
        for (train, test) in &folds {
            let x = train.a_matriz();
            let y = train.etiquetas_one_hot();
            let capas = vec![(8, Activacion::ReLU), (3, Activacion::Softmax)];
            let mut ann = ANN::nueva(4, &capas, 0.05, Perdida::CrossEntropy, 42);
            ann.entrenar(&x, &y, 100, 16);

            let prec_train = ann.precision(&x, &train.etiquetas);
            let x_test = test.a_matriz();
            let prec_test = ann.precision(&x_test, &test.etiquetas);
            let loss = *ann.historial_perdida.last().unwrap_or(&0.0);
            cv.agregar_fold(prec_train, prec_test, loss);
        }

        let resumen = cv.resumen();
        println!("  {}", resumen);

        assert!(cv.media_test() > 0.50, "ANN CV media test > 50%");
        assert_eq!(cv.precisiones_test.len(), 3);
    }
}
