#![allow(clippy::needless_range_loop)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ══════════════════════════════════════════════════════════════
//  Tokenizer — procesamiento de texto base para NLP
//  Tokenización, normalización, stopwords, stemming, n-grams,
//  TF-IDF, bag-of-words
// ══════════════════════════════════════════════════════════════

/// Stopwords en español
const STOPWORDS_ES: &[&str] = &[
    "a", "al", "algo", "algunas", "algunos", "ante", "antes", "como", "con", "contra", "cual",
    "cuando", "de", "del", "desde", "donde", "durante", "e", "el", "ella", "ellas", "ellos", "en",
    "entre", "era", "esa", "esas", "ese", "eso", "esos", "esta", "estaba", "estado", "estar",
    "estas", "este", "esto", "estos", "fue", "ha", "hacer", "hasta", "hay", "la", "las", "le",
    "les", "lo", "los", "mas", "más", "me", "mi", "mí", "mientras", "muy", "nada", "ni", "no",
    "nos", "nosotros", "nuestro", "o", "otra", "otras", "otro", "otros", "para", "pero", "por",
    "que", "qué", "quien", "se", "ser", "si", "sí", "sin", "sino", "sobre", "somos", "son", "soy",
    "su", "sus", "también", "te", "tengo", "ti", "tiene", "tienen", "todo", "todos", "tu", "tú",
    "tus", "un", "una", "unas", "uno", "unos", "usted", "ustedes", "va", "vamos", "ya", "yo",
    // inglés básico
    "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had", "do",
    "does", "did", "will", "would", "shall", "should", "may", "might", "must", "can", "could", "i",
    "you", "he", "she", "it", "we", "they", "me", "him", "her", "us", "them", "my", "your", "his",
    "its", "our", "their", "this", "that", "these", "those", "am", "in", "on", "at", "to", "for",
    "of", "with", "by", "from", "as", "into", "through", "during", "before", "after", "and", "but",
    "or", "nor", "not", "so", "if",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Token {
    pub texto: String,
    pub original: String,
    pub posicion: usize,
    pub es_stopword: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Tokenizer {
    pub vocabulario: HashMap<String, usize>, // palabra → índice
    pub idf: HashMap<String, f64>,           // inverse document frequency
    pub doc_count: usize,
    pub custom_stopwords: Vec<String>,
}

impl Tokenizer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Normaliza texto: minúsculas, quita acentos parciales, limpia puntuación
    pub fn normalizar(texto: &str) -> String {
        texto
            .to_lowercase()
            .replace('á', "a")
            .replace('é', "e")
            .replace('í', "i")
            .replace('ó', "o")
            .replace(['ú', 'ü'], "u")
            .replace('ñ', "n")
    }

    /// Tokeniza texto en palabras limpias
    pub fn tokenizar(texto: &str) -> Vec<Token> {
        let normalizado = Self::normalizar(texto);
        normalizado
            .split(|c: char| !c.is_alphanumeric() && c != '\'')
            .enumerate()
            .filter(|(_, w)| !w.is_empty())
            .map(|(i, w)| {
                let palabra = w.trim_matches('\'').to_string();
                let es_stop = STOPWORDS_ES.contains(&palabra.as_str());
                Token {
                    texto: palabra,
                    original: w.to_string(),
                    posicion: i,
                    es_stopword: es_stop,
                }
            })
            .collect()
    }

    /// Tokeniza y filtra stopwords
    pub fn tokenizar_limpio(texto: &str) -> Vec<String> {
        Self::tokenizar(texto)
            .into_iter()
            .filter(|t| !t.es_stopword && t.texto.len() > 1)
            .map(|t| t.texto)
            .collect()
    }

    /// Stemming simplificado para español (sufijos comunes)
    pub fn stem(palabra: &str) -> String {
        let w = palabra.to_lowercase();
        let sufijos = &[
            "amiento", "imiento", "idades", "mente", "acion", "ición", "ando", "endo", "iendo",
            "ador", "edor", "ción", "sion", "idad", "ible", "able", "ente", "ante", "ando", "endo",
            "ados", "idos", "adas", "idas", "emos", "amos", "aba", "ado", "ido", "ada", "ida",
            "ara", "era", "ará", "erá", "ría", "ión", "ar", "er", "ir", "as", "es", "os", "an",
            "en",
        ];
        for suf in sufijos {
            if w.len() > suf.len() + 2 && w.ends_with(suf) {
                return w[..w.len() - suf.len()].to_string();
            }
        }
        w
    }

    /// Genera n-grams de palabras
    pub fn ngrams(palabras: &[String], n: usize) -> Vec<String> {
        if palabras.len() < n {
            return vec![];
        }
        (0..=palabras.len() - n)
            .map(|i| palabras[i..i + n].join(" "))
            .collect()
    }

    /// Genera character n-grams (para similitud)
    pub fn char_ngrams(palabra: &str, n: usize) -> Vec<String> {
        let chars: Vec<char> = palabra.chars().collect();
        if chars.len() < n {
            return vec![palabra.to_string()];
        }
        (0..=chars.len() - n)
            .map(|i| chars[i..i + n].iter().collect())
            .collect()
    }

    /// Bag of Words: vector de frecuencias
    pub fn bag_of_words(&self, texto: &str) -> Vec<f64> {
        let tokens = Self::tokenizar_limpio(texto);
        let mut bow = vec![0.0; self.vocabulario.len()];
        for t in &tokens {
            if let Some(&idx) = self.vocabulario.get(t) {
                bow[idx] += 1.0;
            }
        }
        bow
    }

    /// TF-IDF vector
    pub fn tfidf(&self, texto: &str) -> Vec<f64> {
        let tokens = Self::tokenizar_limpio(texto);
        let total = tokens.len() as f64;
        let mut vec = vec![0.0; self.vocabulario.len()];

        if total == 0.0 {
            return vec;
        }

        // Contar frecuencias
        let mut freqs: HashMap<&str, f64> = HashMap::new();
        for t in &tokens {
            *freqs.entry(t.as_str()).or_insert(0.0) += 1.0;
        }

        for (palabra, &freq) in &freqs {
            if let Some(&idx) = self.vocabulario.get(*palabra) {
                let tf = freq / total;
                let idf = self.idf.get(*palabra).copied().unwrap_or(1.0);
                vec[idx] = tf * idf;
            }
        }
        vec
    }

    /// Entrenar vocabulario y IDF desde un corpus
    pub fn entrenar_vocabulario(&mut self, documentos: &[&str]) {
        self.doc_count = documentos.len();
        let mut doc_freq: HashMap<String, usize> = HashMap::new();

        for doc in documentos {
            let tokens = Self::tokenizar_limpio(doc);
            let unicos: std::collections::HashSet<String> = tokens.into_iter().collect();
            for palabra in unicos {
                if !self.vocabulario.contains_key(&palabra) {
                    let idx = self.vocabulario.len();
                    self.vocabulario.insert(palabra.clone(), idx);
                }
                *doc_freq.entry(palabra).or_insert(0) += 1;
            }
        }

        // Calcular IDF
        let n = self.doc_count as f64;
        for (palabra, &df) in &doc_freq {
            self.idf
                .insert(palabra.clone(), (n / (1.0 + df as f64)).ln() + 1.0);
        }
    }

    /// Similitud coseno entre dos vectores
    pub fn similitud_coseno(a: &[f64], b: &[f64]) -> f64 {
        let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
        let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm_a < 1e-10 || norm_b < 1e-10 {
            return 0.0;
        }
        dot / (norm_a * norm_b)
    }

    /// Distancia de Levenshtein (edit distance)
    pub fn levenshtein(a: &str, b: &str) -> usize {
        let a: Vec<char> = a.chars().collect();
        let b: Vec<char> = b.chars().collect();
        let (m, n) = (a.len(), b.len());
        let mut dp = vec![vec![0usize; n + 1]; m + 1];

        for i in 0..=m {
            dp[i][0] = i;
        }
        for j in 0..=n {
            dp[0][j] = j;
        }

        for i in 1..=m {
            for j in 1..=n {
                let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
                dp[i][j] = (dp[i - 1][j] + 1)
                    .min(dp[i][j - 1] + 1)
                    .min(dp[i - 1][j - 1] + cost);
            }
        }
        dp[m][n]
    }

    /// Similitud Jaccard entre dos conjuntos de tokens
    pub fn jaccard(a: &[String], b: &[String]) -> f64 {
        let set_a: std::collections::HashSet<&str> = a.iter().map(|s| s.as_str()).collect();
        let set_b: std::collections::HashSet<&str> = b.iter().map(|s| s.as_str()).collect();
        let interseccion = set_a.intersection(&set_b).count() as f64;
        let union = set_a.union(&set_b).count() as f64;
        if union == 0.0 {
            0.0
        } else {
            interseccion / union
        }
    }

    /// Extraer palabras clave (las más frecuentes no-stopword)
    pub fn palabras_clave(texto: &str, top_n: usize) -> Vec<(String, usize)> {
        let tokens = Self::tokenizar_limpio(texto);
        let mut freq: HashMap<String, usize> = HashMap::new();
        for t in tokens {
            *freq.entry(t).or_insert(0) += 1;
        }
        let mut ranking: Vec<(String, usize)> = freq.into_iter().collect();
        ranking.sort_by_key(|k| std::cmp::Reverse(k.1));
        ranking.truncate(top_n);
        ranking
    }

    pub fn vocab_size(&self) -> usize {
        self.vocabulario.len()
    }
}

// ══════════════════════════════════════════════════════════════
//  Word Embeddings simplificados (Word2Vec-like skip-gram)
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WordEmbeddings {
    pub dimension: usize,
    pub embeddings: HashMap<String, Vec<f64>>,
    pub vocab_index: HashMap<String, usize>,
}

impl WordEmbeddings {
    pub fn nuevo(dimension: usize) -> Self {
        Self {
            dimension,
            embeddings: HashMap::new(),
            vocab_index: HashMap::new(),
        }
    }

    /// Entrenar embeddings con skip-gram simplificado
    pub fn entrenar(&mut self, corpus: &[&str], ventana: usize, epocas: usize, lr: f64) {
        use crate::ml::Rng;
        let mut rng = Rng::new(42);

        // Construir vocabulario
        let mut vocab: Vec<String> = Vec::new();
        let mut todas_palabras: Vec<Vec<String>> = Vec::new();

        for doc in corpus {
            let tokens = Tokenizer::tokenizar_limpio(doc);
            for t in &tokens {
                if !self.vocab_index.contains_key(t) {
                    self.vocab_index.insert(t.clone(), vocab.len());
                    vocab.push(t.clone());
                }
            }
            todas_palabras.push(tokens);
        }

        // Inicializar embeddings aleatorios
        for palabra in &vocab {
            let emb: Vec<f64> = (0..self.dimension).map(|_| rng.rango(-0.5, 0.5)).collect();
            self.embeddings.insert(palabra.clone(), emb);
        }

        // Matrices de pesos (input y output)
        let v = vocab.len();
        let d = self.dimension;
        let mut w_in: Vec<Vec<f64>> = (0..v)
            .map(|_| (0..d).map(|_| rng.rango(-0.5, 0.5)).collect())
            .collect();
        let mut w_out: Vec<Vec<f64>> = (0..v)
            .map(|_| (0..d).map(|_| rng.rango(-0.5, 0.5)).collect())
            .collect();

        for epoca in 0..epocas {
            let mut loss_total = 0.0;
            let mut pares = 0;

            for doc_tokens in &todas_palabras {
                for (i, palabra) in doc_tokens.iter().enumerate() {
                    let Some(&idx_w) = self.vocab_index.get(palabra) else {
                        continue;
                    };

                    let start = i.saturating_sub(ventana);
                    let end = (i + ventana + 1).min(doc_tokens.len());

                    for j in start..end {
                        if j == i {
                            continue;
                        }
                        let Some(&idx_c) = self.vocab_index.get(&doc_tokens[j]) else {
                            continue;
                        };

                        // Dot product
                        let dot: f64 = (0..d).map(|k| w_in[idx_w][k] * w_out[idx_c][k]).sum();
                        let sigmoid = 1.0 / (1.0 + (-dot).exp());
                        let error = sigmoid - 1.0; // positive pair, target = 1

                        loss_total += -(sigmoid.max(1e-10)).ln();
                        pares += 1;

                        // Gradientes
                        for k in 0..d {
                            let g_in = error * w_out[idx_c][k];
                            let g_out = error * w_in[idx_w][k];
                            w_in[idx_w][k] -= lr * g_in;
                            w_out[idx_c][k] -= lr * g_out;
                        }

                        // Negative sampling (2 negativos aleatorios)
                        for _ in 0..2 {
                            let neg = rng.usize_rango(v);
                            if neg == idx_c {
                                continue;
                            }
                            let dot_neg: f64 = (0..d).map(|k| w_in[idx_w][k] * w_out[neg][k]).sum();
                            let sig_neg = 1.0 / (1.0 + (-dot_neg).exp());
                            let err_neg = sig_neg; // negative, target = 0

                            for k in 0..d {
                                let g_in = err_neg * w_out[neg][k];
                                let g_out = err_neg * w_in[idx_w][k];
                                w_in[idx_w][k] -= lr * g_in;
                                w_out[neg][k] -= lr * g_out;
                            }
                        }
                    }
                }
            }

            if (epoca + 1) % (epocas / 5).max(1) == 0 || epoca == 0 {
                let avg = if pares > 0 {
                    loss_total / pares as f64
                } else {
                    0.0
                };
                println!("    Época {}/{} — Loss: {:.4}", epoca + 1, epocas, avg);
            }
        }

        // Guardar embeddings finales
        for (palabra, &idx) in &self.vocab_index {
            self.embeddings.insert(palabra.clone(), w_in[idx].clone());
        }
    }

    pub fn vector(&self, palabra: &str) -> Option<&Vec<f64>> {
        let norm = Tokenizer::normalizar(palabra);
        self.embeddings.get(&norm)
    }

    /// Vector promedio de una frase
    pub fn vector_frase(&self, texto: &str) -> Vec<f64> {
        let tokens = Tokenizer::tokenizar_limpio(texto);
        let mut sum = vec![0.0; self.dimension];
        let mut count = 0;

        for t in &tokens {
            if let Some(v) = self.embeddings.get(t) {
                for (i, &val) in v.iter().enumerate() {
                    sum[i] += val;
                }
                count += 1;
            }
        }

        if count > 0 {
            sum.iter_mut().for_each(|x| *x /= count as f64);
        }
        sum
    }

    /// Palabras más similares
    pub fn mas_similares(&self, palabra: &str, top_n: usize) -> Vec<(String, f64)> {
        let Some(vec_p) = self.vector(palabra) else {
            return vec![];
        };

        let mut sims: Vec<(String, f64)> = self
            .embeddings
            .iter()
            .filter(|(k, _)| k.as_str() != Tokenizer::normalizar(palabra))
            .map(|(k, v)| {
                let sim = Tokenizer::similitud_coseno(vec_p, v);
                (k.clone(), sim)
            })
            .collect();

        sims.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        sims.truncate(top_n);
        sims
    }

    pub fn vocab_size(&self) -> usize {
        self.embeddings.len()
    }
}
