use super::tokenizer::Tokenizer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ══════════════════════════════════════════════════════════════
//  Análisis de Sentimiento — léxico + ML
//  Detecta polaridad (positivo/negativo/neutro) e intensidad
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Polaridad {
    MuyPositivo,
    Positivo,
    Neutro,
    Negativo,
    MuyNegativo,
}

impl Polaridad {
    pub fn emoji(&self) -> &str {
        match self {
            Polaridad::MuyPositivo => "😄",
            Polaridad::Positivo => "🙂",
            Polaridad::Neutro => "😐",
            Polaridad::Negativo => "😟",
            Polaridad::MuyNegativo => "😡",
        }
    }

    pub fn nombre(&self) -> &str {
        match self {
            Polaridad::MuyPositivo => "Muy positivo",
            Polaridad::Positivo => "Positivo",
            Polaridad::Neutro => "Neutro",
            Polaridad::Negativo => "Negativo",
            Polaridad::MuyNegativo => "Muy negativo",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResultadoSentimiento {
    pub polaridad: Polaridad,
    pub score: f64,                         // -1.0 a 1.0
    pub confianza: f64,                     // 0.0 a 1.0
    pub emociones: HashMap<String, f64>,    // emoción → intensidad
    pub palabras_clave: Vec<(String, f64)>, // palabra → contribución
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnalizadorSentimiento {
    pub lexico_es: HashMap<String, f64>,
    pub lexico_en: HashMap<String, f64>,
    pub intensificadores: HashMap<String, f64>,
    pub negadores: Vec<String>,
    pub emociones_lexico: HashMap<String, Vec<(String, f64)>>, // emoción → [(palabra, peso)]
    /// Modelo ML: pesos aprendidos por palabra
    pub pesos_ml: HashMap<String, f64>,
    pub sesgo_ml: f64,
    pub entrenado_ml: bool,
}

impl AnalizadorSentimiento {
    pub fn nuevo() -> Self {
        let mut s = Self {
            lexico_es: HashMap::new(),
            lexico_en: HashMap::new(),
            intensificadores: HashMap::new(),
            negadores: Vec::new(),
            emociones_lexico: HashMap::new(),
            pesos_ml: HashMap::new(),
            sesgo_ml: 0.0,
            entrenado_ml: false,
        };
        s.cargar_lexicos();
        s
    }

    fn cargar_lexicos(&mut self) {
        // ── Léxico español ──
        let positivas_es: &[(&str, f64)] = &[
            ("bueno", 0.6),
            ("bien", 0.5),
            ("excelente", 0.9),
            ("genial", 0.8),
            ("increible", 0.85),
            ("maravilloso", 0.9),
            ("fantastico", 0.85),
            ("perfecto", 0.95),
            ("hermoso", 0.7),
            ("bonito", 0.6),
            ("lindo", 0.6),
            ("amor", 0.8),
            ("feliz", 0.8),
            ("alegre", 0.7),
            ("contento", 0.7),
            ("divertido", 0.6),
            ("gracioso", 0.5),
            ("agradable", 0.6),
            ("mejor", 0.7),
            ("facil", 0.4),
            ("rapido", 0.3),
            ("eficiente", 0.6),
            ("util", 0.5),
            ("importante", 0.4),
            ("exitoso", 0.8),
            ("logro", 0.7),
            ("avance", 0.5),
            ("progreso", 0.6),
            ("completado", 0.5),
            ("terminado", 0.4),
            ("gracias", 0.6),
            ("amable", 0.6),
            ("brillante", 0.7),
            ("productivo", 0.6),
            ("inspirador", 0.7),
            ("motivado", 0.7),
            ("entusiasmo", 0.7),
            ("esperanza", 0.6),
            ("confianza", 0.6),
            ("satisfecho", 0.7),
            ("orgulloso", 0.7),
            ("tranquilo", 0.5),
            ("seguro", 0.5),
            ("libre", 0.5),
            ("victoria", 0.8),
            ("celebrar", 0.7),
            ("fiesta", 0.5),
            ("regalo", 0.5),
            ("encanta", 0.8),
            ("gusta", 0.5),
            ("amo", 0.8),
            ("super", 0.6),
            ("wow", 0.7),
            ("bravo", 0.7),
            ("viva", 0.6),
        ];
        let negativas_es: &[(&str, f64)] = &[
            ("malo", -0.6),
            ("mal", -0.5),
            ("terrible", -0.9),
            ("horrible", -0.85),
            ("peor", -0.7),
            ("pesimo", -0.9),
            ("feo", -0.5),
            ("triste", -0.7),
            ("deprimido", -0.8),
            ("enojado", -0.7),
            ("furioso", -0.9),
            ("frustrado", -0.7),
            ("aburrido", -0.4),
            ("cansado", -0.4),
            ("dolor", -0.6),
            ("sufrir", -0.7),
            ("llorar", -0.6),
            ("miedo", -0.6),
            ("odio", -0.9),
            ("odiar", -0.85),
            ("asco", -0.7),
            ("repugnante", -0.8),
            ("dificil", -0.4),
            ("imposible", -0.7),
            ("problema", -0.5),
            ("error", -0.5),
            ("fallo", -0.6),
            ("fracaso", -0.8),
            ("perdida", -0.7),
            ("lento", -0.3),
            ("complicado", -0.4),
            ("confuso", -0.5),
            ("estresado", -0.6),
            ("ansiedad", -0.7),
            ("preocupado", -0.5),
            ("solo", -0.4),
            ("perdido", -0.5),
            ("atrasado", -0.4),
            ("cancelado", -0.5),
            ("rechazado", -0.6),
            ("ignorado", -0.5),
            ("inutil", -0.7),
            ("basura", -0.8),
            ("desastre", -0.8),
            ("caos", -0.6),
            ("crisis", -0.6),
            ("urgente", -0.3),
            ("nunca", -0.3),
            ("jamas", -0.4),
            ("nada", -0.3),
        ];

        for &(p, v) in positivas_es {
            self.lexico_es.insert(p.to_string(), v);
        }
        for &(p, v) in negativas_es {
            self.lexico_es.insert(p.to_string(), v);
        }

        // ── Léxico inglés ──
        let positivas_en: &[(&str, f64)] = &[
            ("good", 0.6),
            ("great", 0.8),
            ("excellent", 0.9),
            ("amazing", 0.85),
            ("wonderful", 0.9),
            ("fantastic", 0.85),
            ("perfect", 0.95),
            ("beautiful", 0.7),
            ("love", 0.8),
            ("happy", 0.8),
            ("glad", 0.6),
            ("awesome", 0.85),
            ("best", 0.8),
            ("brilliant", 0.8),
            ("easy", 0.4),
            ("fast", 0.3),
            ("efficient", 0.6),
            ("useful", 0.5),
            ("success", 0.8),
            ("win", 0.7),
            ("complete", 0.5),
            ("done", 0.4),
            ("thanks", 0.6),
            ("nice", 0.5),
            ("cool", 0.5),
            ("fun", 0.6),
            ("enjoy", 0.6),
            ("like", 0.4),
            ("excited", 0.7),
            ("proud", 0.7),
        ];
        let negativas_en: &[(&str, f64)] = &[
            ("bad", -0.6),
            ("terrible", -0.9),
            ("horrible", -0.85),
            ("awful", -0.8),
            ("worst", -0.8),
            ("ugly", -0.5),
            ("sad", -0.7),
            ("angry", -0.7),
            ("hate", -0.9),
            ("boring", -0.4),
            ("tired", -0.4),
            ("pain", -0.6),
            ("fail", -0.6),
            ("failure", -0.8),
            ("loss", -0.6),
            ("slow", -0.3),
            ("difficult", -0.4),
            ("impossible", -0.7),
            ("problem", -0.5),
            ("error", -0.5),
            ("bug", -0.4),
            ("broken", -0.6),
            ("wrong", -0.5),
            ("stress", -0.6),
            ("anxiety", -0.7),
            ("worried", -0.5),
            ("never", -0.3),
            ("nothing", -0.3),
            ("useless", -0.7),
        ];

        for &(p, v) in positivas_en {
            self.lexico_en.insert(p.to_string(), v);
        }
        for &(p, v) in negativas_en {
            self.lexico_en.insert(p.to_string(), v);
        }

        // ── Intensificadores ──
        let intensif: &[(&str, f64)] = &[
            ("muy", 1.5),
            ("mucho", 1.4),
            ("bastante", 1.3),
            ("demasiado", 1.5),
            ("super", 1.6),
            ("extremadamente", 1.8),
            ("increiblemente", 1.7),
            ("totalmente", 1.5),
            ("completamente", 1.5),
            ("absolutamente", 1.6),
            ("really", 1.5),
            ("very", 1.5),
            ("extremely", 1.8),
            ("totally", 1.5),
            ("so", 1.3),
            ("too", 1.3),
            ("quite", 1.2),
            ("absolutely", 1.6),
            // Atenuadores
            ("poco", 0.5),
            ("algo", 0.6),
            ("ligeramente", 0.4),
            ("apenas", 0.3),
            ("slightly", 0.4),
            ("somewhat", 0.5),
            ("barely", 0.3),
            ("little", 0.5),
        ];
        for &(p, v) in intensif {
            self.intensificadores.insert(p.to_string(), v);
        }

        // ── Negadores ──
        self.negadores = vec![
            "no".into(),
            "ni".into(),
            "nunca".into(),
            "jamas".into(),
            "tampoco".into(),
            "sin".into(),
            "not".into(),
            "never".into(),
            "neither".into(),
            "nor".into(),
            "don't".into(),
            "doesn't".into(),
            "didn't".into(),
            "won't".into(),
        ];

        // ── Emociones ──
        let emociones: &[(&str, &[(&str, f64)])] = &[
            (
                "alegria",
                &[
                    ("feliz", 0.9),
                    ("contento", 0.8),
                    ("alegre", 0.9),
                    ("divertido", 0.7),
                    ("risa", 0.8),
                    ("celebrar", 0.7),
                    ("happy", 0.9),
                    ("joy", 0.9),
                ],
            ),
            (
                "tristeza",
                &[
                    ("triste", 0.9),
                    ("llorar", 0.8),
                    ("deprimido", 0.9),
                    ("melancolico", 0.7),
                    ("solo", 0.6),
                    ("perdida", 0.7),
                    ("sad", 0.9),
                    ("cry", 0.8),
                ],
            ),
            (
                "enojo",
                &[
                    ("enojado", 0.9),
                    ("furioso", 0.95),
                    ("rabia", 0.9),
                    ("odio", 0.85),
                    ("frustrado", 0.7),
                    ("angry", 0.9),
                    ("furious", 0.95),
                    ("hate", 0.85),
                ],
            ),
            (
                "miedo",
                &[
                    ("miedo", 0.9),
                    ("asustado", 0.8),
                    ("terror", 0.95),
                    ("panico", 0.9),
                    ("ansioso", 0.7),
                    ("afraid", 0.9),
                    ("scared", 0.8),
                    ("fear", 0.9),
                ],
            ),
            (
                "sorpresa",
                &[
                    ("sorpresa", 0.8),
                    ("asombro", 0.7),
                    ("increible", 0.6),
                    ("wow", 0.8),
                    ("surprise", 0.8),
                    ("amazing", 0.6),
                    ("shocked", 0.8),
                ],
            ),
            (
                "confianza",
                &[
                    ("seguro", 0.7),
                    ("confianza", 0.8),
                    ("capaz", 0.6),
                    ("fuerte", 0.6),
                    ("confident", 0.8),
                    ("trust", 0.7),
                    ("strong", 0.6),
                ],
            ),
        ];
        for &(emocion, palabras) in emociones {
            self.emociones_lexico.insert(
                emocion.to_string(),
                palabras.iter().map(|&(p, v)| (p.to_string(), v)).collect(),
            );
        }
    }

    /// Análisis principal: combina léxico + contexto + ML
    pub fn analizar(&self, texto: &str) -> ResultadoSentimiento {
        let tokens = Tokenizer::tokenizar(texto);
        let palabras: Vec<String> = tokens.iter().map(|t| t.texto.clone()).collect();

        let mut score_total = 0.0;
        let mut contribuciones: Vec<(String, f64)> = Vec::new();
        let mut palabras_con_score = 0;

        for (i, palabra) in palabras.iter().enumerate() {
            // Buscar en ambos léxicos
            let mut score_palabra = 0.0;
            if let Some(&v) = self.lexico_es.get(palabra.as_str()) {
                score_palabra = v;
            } else if let Some(&v) = self.lexico_en.get(palabra.as_str()) {
                score_palabra = v;
            }

            if score_palabra.abs() < 0.01 {
                // Intentar con stem
                let stemmed = Tokenizer::stem(palabra);
                if let Some(&v) = self.lexico_es.get(stemmed.as_str()) {
                    score_palabra = v;
                } else if let Some(&v) = self.lexico_en.get(stemmed.as_str()) {
                    score_palabra = v;
                }
            }

            if score_palabra.abs() > 0.01 {
                // Verificar negadores en las 3 palabras anteriores
                let negado = (1..=3).any(|k| {
                    if i >= k {
                        self.negadores.contains(&palabras[i - k])
                    } else {
                        false
                    }
                });
                if negado {
                    score_palabra *= -0.8; // invertir pero atenuar un poco
                }

                // Verificar intensificadores en las 2 palabras anteriores
                for k in 1..=2 {
                    if i >= k {
                        if let Some(&mult) = self.intensificadores.get(palabras[i - k].as_str()) {
                            score_palabra *= mult;
                        }
                    }
                }

                contribuciones.push((palabra.clone(), score_palabra));
                score_total += score_palabra;
                palabras_con_score += 1;
            }
        }

        // Score ML si está entrenado
        if self.entrenado_ml {
            let mut score_ml = self.sesgo_ml;
            for palabra in &palabras {
                if let Some(&peso) = self.pesos_ml.get(palabra.as_str()) {
                    score_ml += peso;
                }
            }
            // Combinar léxico (60%) + ML (40%)
            let score_ml_norm = score_ml.tanh(); // normalizar a [-1, 1]
            if palabras_con_score > 0 {
                score_total = score_total * 0.6 + score_ml_norm * palabras_con_score as f64 * 0.4;
            } else {
                score_total = score_ml_norm;
                palabras_con_score = 1;
            }
        }

        // Normalizar score
        let score_norm = if palabras_con_score > 0 {
            (score_total / palabras_con_score as f64).clamp(-1.0, 1.0)
        } else {
            0.0
        };

        // Determinar polaridad
        let polaridad = match score_norm {
            s if s > 0.5 => Polaridad::MuyPositivo,
            s if s > 0.1 => Polaridad::Positivo,
            s if s > -0.1 => Polaridad::Neutro,
            s if s > -0.5 => Polaridad::Negativo,
            _ => Polaridad::MuyNegativo,
        };

        // Confianza basada en cantidad de señales
        let confianza = if palabras_con_score == 0 {
            0.2 // muy baja si no encontró nada
        } else {
            (0.3 + 0.1 * palabras_con_score as f64).min(0.95)
        };

        // Detectar emociones
        let emociones = self.detectar_emociones(&palabras);

        contribuciones.sort_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).unwrap());
        contribuciones.truncate(5);

        ResultadoSentimiento {
            polaridad,
            score: score_norm,
            confianza,
            emociones,
            palabras_clave: contribuciones,
        }
    }

    fn detectar_emociones(&self, palabras: &[String]) -> HashMap<String, f64> {
        let mut emociones = HashMap::new();

        for (emocion, lista) in &self.emociones_lexico {
            let mut score = 0.0;
            let mut hits = 0;

            for palabra in palabras {
                for (p, peso) in lista {
                    if palabra == p || Tokenizer::stem(palabra) == Tokenizer::stem(p) {
                        score += peso;
                        hits += 1;
                    }
                }
            }

            if hits > 0 {
                emociones.insert(emocion.clone(), (score / hits as f64).min(1.0));
            }
        }

        emociones
    }

    /// Entrenar modelo ML con datos etiquetados (texto, score -1 a 1)
    pub fn entrenar_ml(&mut self, datos: &[(&str, f64)], epocas: usize, lr: f64) {
        // Construir vocabulario de entrenamiento
        for (texto, _) in datos {
            let tokens = Tokenizer::tokenizar_limpio(texto);
            for t in tokens {
                self.pesos_ml.entry(t).or_insert(0.0);
            }
        }

        for epoca in 0..epocas {
            let mut loss_total = 0.0;

            for &(texto, target) in datos {
                let tokens = Tokenizer::tokenizar_limpio(texto);

                // Forward
                let mut pred = self.sesgo_ml;
                for t in &tokens {
                    pred += self.pesos_ml.get(t.as_str()).copied().unwrap_or(0.0);
                }
                pred = pred.tanh();

                // Loss
                let error = pred - target;
                loss_total += error * error;

                // Gradiente (MSE + tanh derivative)
                let dtanh = 1.0 - pred * pred;
                let grad = 2.0 * error * dtanh;

                // Actualizar pesos
                for t in &tokens {
                    if let Some(peso) = self.pesos_ml.get_mut(t.as_str()) {
                        *peso -= lr * grad;
                    }
                }
                self.sesgo_ml -= lr * grad;
            }

            let avg_loss = loss_total / datos.len() as f64;
            if (epoca + 1) % (epocas / 5).max(1) == 0 || epoca == 0 {
                println!("    Época {}/{} — MSE: {:.6}", epoca + 1, epocas, avg_loss);
            }
        }

        self.entrenado_ml = true;
    }

    pub fn resumen(&self) {
        println!("  Analizador de Sentimiento");
        println!("  ─────────────────────────");
        println!("    Léxico español: {} palabras", self.lexico_es.len());
        println!("    Léxico inglés: {} palabras", self.lexico_en.len());
        println!("    Intensificadores: {}", self.intensificadores.len());
        println!("    Negadores: {}", self.negadores.len());
        println!("    Emociones: {}", self.emociones_lexico.len());
        println!(
            "    Modelo ML: {}",
            if self.entrenado_ml {
                "Entrenado"
            } else {
                "No entrenado"
            }
        );
        if self.entrenado_ml {
            println!("    Palabras ML: {}", self.pesos_ml.len());
        }
    }
}

impl Default for AnalizadorSentimiento {
    fn default() -> Self {
        Self::nuevo()
    }
}
