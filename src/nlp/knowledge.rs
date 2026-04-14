use super::tokenizer::Tokenizer;
use serde::{Deserialize, Serialize};

// ══════════════════════════════════════════════════════════════
//  Base de Conocimiento — almacenamiento y recuperación
//  Grafo de conceptos + búsqueda semántica
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntradaConocimiento {
    pub id: String,
    pub titulo: String,
    pub contenido: String,
    pub categoria: String,
    pub etiquetas: Vec<String>,
    pub relaciones: Vec<Relacion>,
    pub tokens: Vec<String>, // tokens preprocesados
    pub tfidf: Vec<f64>,     // vector TF-IDF precalculado
    pub consultas: usize,    // veces consultada
    pub utilidad: f64,       // score de utilidad (feedback)
    pub creado: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Relacion {
    pub destino_id: String,
    pub tipo: TipoRelacion,
    pub peso: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TipoRelacion {
    EsUn,         // "perro" es un "animal"
    TieneParte,   // "coche" tiene "motor"
    Relacionado,  // relación genérica
    Sinonimo,     // "feliz" = "contento"
    Antonimo,     // "feliz" ≠ "triste"
    Ejemplo,      // "manzana" es ejemplo de "fruta"
    Causa,        // "lluvia" causa "charco"
    Prerequisito, // "mezclar" requiere "ingredientes"
}

impl TipoRelacion {
    pub fn nombre(&self) -> &str {
        match self {
            Self::EsUn => "es_un",
            Self::TieneParte => "tiene_parte",
            Self::Relacionado => "relacionado",
            Self::Sinonimo => "sinonimo",
            Self::Antonimo => "antonimo",
            Self::Ejemplo => "ejemplo",
            Self::Causa => "causa",
            Self::Prerequisito => "prerequisito",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResultadoBusqueda {
    pub entrada: EntradaConocimiento,
    pub relevancia: f64,
    pub razon: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BaseConocimiento {
    pub entradas: Vec<EntradaConocimiento>,
    pub categorias: Vec<String>,
    pub tokenizer: Tokenizer,
    siguiente_id: usize,
}

impl BaseConocimiento {
    pub fn nueva() -> Self {
        let mut bc = Self {
            entradas: Vec::new(),
            categorias: Vec::new(),
            tokenizer: Tokenizer::new(),
            siguiente_id: 1,
        };
        bc.cargar_conocimiento_base();
        bc
    }

    fn cargar_conocimiento_base(&mut self) {
        // Conocimiento base sobre el propio sistema omniplanner
        let entradas_base: &[(&str, &str, &str, &[&str])] = &[
            ("Tareas", "Gestión de Tareas",
             "El sistema permite crear, editar, eliminar y listar tareas. Cada tarea tiene título, descripción, prioridad, etiquetas y fecha límite.",
             &["tarea", "crear", "editar", "prioridad", "pendiente"]),
            ("Agenda", "Calendario y Agenda",
             "La agenda permite programar eventos con fecha, hora de inicio y fin, y descripción. Se puede ver la agenda del día, semana o mes.",
             &["agenda", "evento", "calendario", "fecha", "horario"]),
            ("Canvas", "Canvas Visual",
             "El canvas permite crear notas visuales organizadas en un espacio de trabajo. Cada canvas puede contener tarjetas con texto.",
             &["canvas", "nota", "visual", "tarjeta", "espacio"]),
            ("Diagramas", "Diagramas de Flujo",
             "Se pueden crear diagramas de flujo con nodos y conexiones. Útil para planificar procesos y flujos de trabajo.",
             &["diagrama", "flujo", "nodo", "conexion", "proceso"]),
            ("VCS", "Control de Versiones",
             "Sistema integrado de control de versiones para rastrear cambios en tus proyectos y notas.",
             &["version", "control", "cambio", "historial", "commit"]),
            ("ML", "Machine Learning",
             "Módulo de inteligencia artificial con 8 algoritmos: ANN, SVM, Árbol de Decisión, Bosque Aleatorio, DNN, CNN, RNN y Aprendizaje por Refuerzo.",
             &["ml", "inteligencia", "artificial", "algoritmo", "modelo", "entrenar"]),
            ("NLP", "Procesamiento de Lenguaje Natural",
             "Sistema de NLP con tokenización, análisis de sentimiento, reconocimiento de intención, base de conocimiento y conversación multi-turno.",
             &["nlp", "lenguaje", "natural", "texto", "conversacion"]),
            ("Productividad", "Consejos de Productividad",
             "Para ser más productivo: divide tareas grandes en pequeñas, usa la técnica Pomodoro (25 min trabajo + 5 min descanso), prioriza con Eisenhower.",
             &["productividad", "tecnica", "pomodoro", "priorizar", "eficiencia"]),
        ];

        for &(cat, titulo, contenido, etiquetas) in entradas_base {
            self.agregar(
                titulo,
                contenido,
                cat,
                &etiquetas.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            );
        }
    }

    /// Agregar nueva entrada
    pub fn agregar(
        &mut self,
        titulo: &str,
        contenido: &str,
        categoria: &str,
        etiquetas: &[String],
    ) -> String {
        let id = format!("kb_{:04}", self.siguiente_id);
        self.siguiente_id += 1;

        let texto_completo = format!("{} {} {}", titulo, contenido, etiquetas.join(" "));
        let tokens = Tokenizer::tokenizar_limpio(&texto_completo);

        // Actualizar vocabulario del tokenizer
        self.tokenizer
            .entrenar_vocabulario(&[texto_completo.as_str()]);
        let tfidf = self.tokenizer.tfidf(&texto_completo);

        if !self.categorias.contains(&categoria.to_string()) {
            self.categorias.push(categoria.to_string());
        }

        let entrada = EntradaConocimiento {
            id: id.clone(),
            titulo: titulo.to_string(),
            contenido: contenido.to_string(),
            categoria: categoria.to_string(),
            etiquetas: etiquetas.to_vec(),
            relaciones: Vec::new(),
            tokens,
            tfidf,
            consultas: 0,
            utilidad: 0.5, // neutro inicial
            creado: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
        };

        self.entradas.push(entrada);
        id
    }

    /// Agregar relación entre dos entradas
    pub fn agregar_relacion(
        &mut self,
        origen_id: &str,
        destino_id: &str,
        tipo: TipoRelacion,
        peso: f64,
    ) {
        if let Some(entrada) = self.entradas.iter_mut().find(|e| e.id == origen_id) {
            entrada.relaciones.push(Relacion {
                destino_id: destino_id.to_string(),
                tipo,
                peso,
            });
        }
    }

    /// Buscar por texto (TF-IDF + similitud coseno)
    pub fn buscar(&mut self, consulta: &str, max_resultados: usize) -> Vec<ResultadoBusqueda> {
        let query_tfidf = self.tokenizer.tfidf(consulta);
        let query_tokens = Tokenizer::tokenizar_limpio(consulta);

        let mut resultados: Vec<(usize, f64, String)> = Vec::new();

        for (i, entrada) in self.entradas.iter().enumerate() {
            let mut score = 0.0;
            let mut razon = String::new();

            // 1. Similitud TF-IDF coseno
            let sim_tfidf = Tokenizer::similitud_coseno(&query_tfidf, &entrada.tfidf);
            if sim_tfidf > 0.01 {
                score += sim_tfidf * 0.5;
                razon.push_str(&format!("TF-IDF: {:.2} ", sim_tfidf));
            }

            // 2. Coincidencia directa de tokens
            let tokens_comunes: usize = query_tokens
                .iter()
                .filter(|qt| entrada.tokens.contains(qt))
                .count();
            if tokens_comunes > 0 {
                let ratio = tokens_comunes as f64 / query_tokens.len().max(1) as f64;
                score += ratio * 0.3;
                razon.push_str(&format!(
                    "Tokens: {}/{} ",
                    tokens_comunes,
                    query_tokens.len()
                ));
            }

            // 3. Coincidencia por etiquetas
            let etiquetas_comunes: usize = query_tokens
                .iter()
                .filter(|qt| entrada.etiquetas.iter().any(|e| e.contains(qt.as_str())))
                .count();
            if etiquetas_comunes > 0 {
                score += etiquetas_comunes as f64 * 0.15;
                razon.push_str(&format!("Etiquetas: {} ", etiquetas_comunes));
            }

            // 4. Bonus por utilidad
            score *= 0.8 + entrada.utilidad * 0.4;

            if score > 0.01 {
                resultados.push((i, score, razon));
            }
        }

        resultados.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        resultados.truncate(max_resultados);

        resultados
            .iter()
            .map(|(i, score, razon)| {
                // Incrementar contador de consultas
                let entrada = self.entradas[*i].clone();
                self.entradas[*i].consultas += 1;
                ResultadoBusqueda {
                    entrada,
                    relevancia: *score,
                    razon: razon.clone(),
                }
            })
            .collect()
    }

    /// Buscar por categoría
    pub fn buscar_por_categoria(&self, categoria: &str) -> Vec<&EntradaConocimiento> {
        self.entradas
            .iter()
            .filter(|e| e.categoria.to_lowercase() == categoria.to_lowercase())
            .collect()
    }

    /// Obtener entradas relacionadas
    pub fn obtener_relacionadas(&self, id: &str) -> Vec<(&EntradaConocimiento, &Relacion)> {
        if let Some(entrada) = self.entradas.iter().find(|e| e.id == id) {
            entrada
                .relaciones
                .iter()
                .filter_map(|rel| {
                    self.entradas
                        .iter()
                        .find(|e| e.id == rel.destino_id)
                        .map(|e| (e, rel))
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Actualizar utilidad de una entrada (feedback)
    pub fn actualizar_utilidad(&mut self, id: &str, delta: f64) {
        if let Some(entrada) = self.entradas.iter_mut().find(|e| e.id == id) {
            entrada.utilidad = (entrada.utilidad + delta).clamp(0.0, 1.0);
        }
    }

    /// Generar respuesta basada en conocimiento
    pub fn generar_respuesta(&mut self, consulta: &str) -> Option<String> {
        let resultados = self.buscar(consulta, 3);
        if resultados.is_empty() {
            return None;
        }

        let mejor = &resultados[0];
        if mejor.relevancia < 0.1 {
            return None;
        }

        let mut respuesta = mejor.entrada.contenido.clone();

        // Añadir info relacionada si hay
        let relacionadas = self.obtener_relacionadas(&mejor.entrada.id);
        if !relacionadas.is_empty() {
            respuesta.push_str("\n\nRelacionado:");
            for (rel_entrada, rel) in relacionadas.iter().take(2) {
                respuesta.push_str(&format!(
                    "\n  • {} ({}): {}",
                    rel_entrada.titulo,
                    rel.tipo.nombre(),
                    &rel_entrada.contenido[..rel_entrada.contenido.len().min(80)]
                ));
            }
        }

        Some(respuesta)
    }

    pub fn resumen(&self) {
        println!("  Base de Conocimiento");
        println!("  ────────────────────");
        println!("    Entradas: {}", self.entradas.len());
        println!("    Categorías: {}", self.categorias.len());
        let total_rel: usize = self.entradas.iter().map(|e| e.relaciones.len()).sum();
        println!("    Relaciones: {}", total_rel);
        let total_consultas: usize = self.entradas.iter().map(|e| e.consultas).sum();
        println!("    Consultas totales: {}", total_consultas);
    }

    pub fn total_entradas(&self) -> usize {
        self.entradas.len()
    }
}

impl Default for BaseConocimiento {
    fn default() -> Self {
        Self::nueva()
    }
}
