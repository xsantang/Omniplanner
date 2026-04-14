use super::tokenizer::Tokenizer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ══════════════════════════════════════════════════════════════
//  Reconocimiento de Intención — reglas + patrón ML
//  Clasifica la intención del usuario a partir de su texto
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CategoriaIntencion {
    Crear,
    Consultar,
    Modificar,
    Eliminar,
    Listar,
    Ayuda,
    Saludo,
    Despedida,
    Agradecimiento,
    Afirmacion,
    Negacion,
    Sentimiento,
    Configurar,
    Buscar,
    Exportar,
    Desconocido,
}

impl CategoriaIntencion {
    pub fn nombre(&self) -> &str {
        match self {
            Self::Crear => "Crear",
            Self::Consultar => "Consultar",
            Self::Modificar => "Modificar",
            Self::Eliminar => "Eliminar",
            Self::Listar => "Listar",
            Self::Ayuda => "Ayuda",
            Self::Saludo => "Saludo",
            Self::Despedida => "Despedida",
            Self::Agradecimiento => "Agradecimiento",
            Self::Afirmacion => "Afirmación",
            Self::Negacion => "Negación",
            Self::Sentimiento => "Sentimiento",
            Self::Configurar => "Configurar",
            Self::Buscar => "Buscar",
            Self::Exportar => "Exportar",
            Self::Desconocido => "Desconocido",
        }
    }

    pub fn todas() -> Vec<CategoriaIntencion> {
        vec![
            Self::Crear,
            Self::Consultar,
            Self::Modificar,
            Self::Eliminar,
            Self::Listar,
            Self::Ayuda,
            Self::Saludo,
            Self::Despedida,
            Self::Agradecimiento,
            Self::Afirmacion,
            Self::Negacion,
            Self::Sentimiento,
            Self::Configurar,
            Self::Buscar,
            Self::Exportar,
            Self::Desconocido,
        ]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Intencion {
    pub categoria: CategoriaIntencion,
    pub confianza: f64,
    pub entidades: Vec<Entidad>,
    pub alternativas: Vec<(CategoriaIntencion, f64)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entidad {
    pub tipo: String,    // "tarea", "fecha", "proyecto", "persona", etc.
    pub valor: String,   // texto extraído
    pub posicion: usize, // posición en tokens
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClasificadorIntencion {
    /// Patrones de reglas: categoría → lista de palabras clave
    pub reglas: HashMap<String, Vec<Vec<String>>>,
    /// Pesos ML por categoría: cat → {palabra → peso}
    pub pesos_ml: HashMap<String, HashMap<String, f64>>,
    pub sesgos_ml: HashMap<String, f64>,
    pub entrenado_ml: bool,
    /// Historial para aprendizaje incremental
    pub historial: Vec<(String, CategoriaIntencion)>,
}

impl ClasificadorIntencion {
    pub fn nuevo() -> Self {
        let mut c = Self {
            reglas: HashMap::new(),
            pesos_ml: HashMap::new(),
            sesgos_ml: HashMap::new(),
            entrenado_ml: false,
            historial: Vec::new(),
        };
        c.cargar_reglas();
        c
    }

    fn cargar_reglas(&mut self) {
        let reglas_def: &[(&str, &[&[&str]])] = &[
            (
                "Crear",
                &[
                    &["crear", "tarea"],
                    &["nueva", "tarea"],
                    &["agregar"],
                    &["añadir"],
                    &["nuevo"],
                    &["nueva"],
                    &["create"],
                    &["add"],
                    &["new"],
                    &["registrar"],
                    &["agendar"],
                    &["programar"],
                ],
            ),
            (
                "Consultar",
                &[
                    &["que", "es"],
                    &["como"],
                    &["cuando"],
                    &["donde"],
                    &["cual"],
                    &["cuanto"],
                    &["ver"],
                    &["mostrar"],
                    &["show"],
                    &["what"],
                    &["how"],
                    &["when"],
                    &["where"],
                    &["estado"],
                ],
            ),
            (
                "Modificar",
                &[
                    &["editar"],
                    &["modificar"],
                    &["cambiar"],
                    &["actualizar"],
                    &["update"],
                    &["edit"],
                    &["change"],
                    &["mover"],
                    &["renombrar"],
                ],
            ),
            (
                "Eliminar",
                &[
                    &["eliminar"],
                    &["borrar"],
                    &["quitar"],
                    &["remover"],
                    &["delete"],
                    &["remove"],
                    &["limpiar"],
                ],
            ),
            (
                "Listar",
                &[
                    &["listar"],
                    &["lista"],
                    &["todas"],
                    &["todos"],
                    &["ver", "todo"],
                    &["list"],
                    &["all"],
                    &["pendientes"],
                    &["completadas"],
                ],
            ),
            (
                "Ayuda",
                &[
                    &["ayuda"],
                    &["help"],
                    &["como", "funciona"],
                    &["que", "puedo"],
                    &["opciones"],
                    &["instrucciones"],
                    &["manual"],
                ],
            ),
            (
                "Saludo",
                &[
                    &["hola"],
                    &["buenos", "dias"],
                    &["buenas", "tardes"],
                    &["buenas", "noches"],
                    &["hello"],
                    &["hi"],
                    &["hey"],
                    &["que", "tal"],
                    &["saludos"],
                ],
            ),
            (
                "Despedida",
                &[
                    &["adios"],
                    &["chao"],
                    &["hasta", "luego"],
                    &["nos", "vemos"],
                    &["bye"],
                    &["goodbye"],
                    &["salir"],
                    &["exit"],
                ],
            ),
            (
                "Agradecimiento",
                &[
                    &["gracias"],
                    &["thanks"],
                    &["thank"],
                    &["agradezco"],
                    &["genial", "gracias"],
                    &["perfecto", "gracias"],
                ],
            ),
            (
                "Afirmacion",
                &[
                    &["si"],
                    &["claro"],
                    &["por", "supuesto"],
                    &["correcto"],
                    &["yes"],
                    &["ok"],
                    &["vale"],
                    &["exacto"],
                    &["afirmativo"],
                ],
            ),
            (
                "Negacion",
                &[
                    &["no"],
                    &["negativo"],
                    &["para", "nada"],
                    &["tampoco"],
                    &["nunca"],
                    &["jamas"],
                    &["cancel"],
                ],
            ),
            (
                "Configurar",
                &[
                    &["configurar"],
                    &["config"],
                    &["ajustar"],
                    &["preferencias"],
                    &["settings"],
                    &["setup"],
                    &["personalizar"],
                ],
            ),
            (
                "Buscar",
                &[
                    &["buscar"],
                    &["encontrar"],
                    &["search"],
                    &["find"],
                    &["filtrar"],
                    &["localizar"],
                ],
            ),
            (
                "Exportar",
                &[
                    &["exportar"],
                    &["export"],
                    &["guardar", "como"],
                    &["descargar"],
                    &["download"],
                    &["backup"],
                    &["respaldo"],
                ],
            ),
        ];

        for &(cat, patrones) in reglas_def {
            let ps: Vec<Vec<String>> = patrones
                .iter()
                .map(|p| p.iter().map(|s| s.to_string()).collect())
                .collect();
            self.reglas.insert(cat.to_string(), ps);
        }
    }

    /// Clasificar intención del texto
    pub fn clasificar(&self, texto: &str) -> Intencion {
        let tokens = Tokenizer::tokenizar(texto);
        let palabras: Vec<String> = tokens.iter().map(|t| t.texto.clone()).collect();

        let mut scores: Vec<(CategoriaIntencion, f64)> = Vec::new();

        // 1. Puntuación por reglas
        for cat in CategoriaIntencion::todas() {
            let cat_nombre = cat.nombre().to_string();
            let mut score_reglas = 0.0;

            if let Some(patrones) = self.reglas.get(&cat_nombre) {
                for patron in patrones {
                    if self.patron_coincide(patron, &palabras) {
                        score_reglas += patron.len() as f64 * 0.3;
                    }
                }
            }

            // 2. Puntuación ML
            let score_ml = if self.entrenado_ml {
                let mut s = self.sesgos_ml.get(&cat_nombre).copied().unwrap_or(0.0);
                if let Some(pesos) = self.pesos_ml.get(&cat_nombre) {
                    for p in &palabras {
                        s += pesos.get(p.as_str()).copied().unwrap_or(0.0);
                    }
                }
                sigmoid(s)
            } else {
                0.0
            };

            // Combinar
            let score_final = if self.entrenado_ml {
                score_reglas * 0.5 + score_ml * 0.5
            } else {
                score_reglas
            };

            scores.push((cat, score_final));
        }

        // Ordenar por score descendente
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        let (mejor_cat, mejor_score) = scores[0].clone();
        let confianza = if mejor_score > 0.0 {
            let total: f64 = scores.iter().map(|(_, s)| s).sum();
            if total > 0.0 {
                mejor_score / total
            } else {
                0.0
            }
        } else {
            0.0
        };

        let alternativas: Vec<(CategoriaIntencion, f64)> = scores
            .iter()
            .skip(1)
            .take(3)
            .filter(|(_, s)| *s > 0.0)
            .cloned()
            .collect();

        // Extraer entidades
        let entidades = self.extraer_entidades(&palabras);

        // Si confianza muy baja, marcar como desconocido
        let (cat_final, conf_final) = if mejor_score < 0.1 {
            (CategoriaIntencion::Desconocido, 0.1)
        } else {
            (mejor_cat, confianza.clamp(0.1, 0.95))
        };

        Intencion {
            categoria: cat_final,
            confianza: conf_final,
            entidades,
            alternativas,
        }
    }

    fn patron_coincide(&self, patron: &[String], palabras: &[String]) -> bool {
        if patron.is_empty() {
            return false;
        }
        if patron.len() == 1 {
            return palabras
                .iter()
                .any(|p| p == &patron[0] || Tokenizer::stem(p) == patron[0]);
        }
        // Multi-palabra: buscar secuencia contigua o cercana
        for i in 0..palabras.len() {
            if palabras[i] == patron[0] || Tokenizer::stem(&palabras[i]) == patron[0] {
                let mut j = 1;
                let mut k = i + 1;
                while j < patron.len() && k < palabras.len() && k < i + patron.len() + 2 {
                    if palabras[k] == patron[j] || Tokenizer::stem(&palabras[k]) == patron[j] {
                        j += 1;
                    }
                    k += 1;
                }
                if j == patron.len() {
                    return true;
                }
            }
        }
        false
    }

    fn extraer_entidades(&self, palabras: &[String]) -> Vec<Entidad> {
        let mut entidades = Vec::new();

        // Detectar fechas simples (hoy, mañana, etc.)
        let palabras_fecha = [
            "hoy",
            "mañana",
            "ayer",
            "lunes",
            "martes",
            "miercoles",
            "jueves",
            "viernes",
            "sabado",
            "domingo",
            "today",
            "tomorrow",
            "yesterday",
            "semana",
            "mes",
            "año",
        ];
        for (i, p) in palabras.iter().enumerate() {
            if palabras_fecha.contains(&p.as_str()) {
                entidades.push(Entidad {
                    tipo: "fecha".to_string(),
                    valor: p.clone(),
                    posicion: i,
                });
            }
        }

        // Detectar números
        for (i, p) in palabras.iter().enumerate() {
            if p.parse::<f64>().is_ok() {
                entidades.push(Entidad {
                    tipo: "numero".to_string(),
                    valor: p.clone(),
                    posicion: i,
                });
            }
        }

        // Detectar prioridades
        let prioridades = [
            "urgente",
            "importante",
            "alta",
            "media",
            "baja",
            "critical",
            "high",
            "medium",
            "low",
        ];
        for (i, p) in palabras.iter().enumerate() {
            if prioridades.contains(&p.as_str()) {
                entidades.push(Entidad {
                    tipo: "prioridad".to_string(),
                    valor: p.clone(),
                    posicion: i,
                });
            }
        }

        entidades
    }

    /// Entrenar con datos etiquetados: (texto, categoría)
    pub fn entrenar(&mut self, datos: &[(&str, CategoriaIntencion)], epocas: usize, lr: f64) {
        // Construir vocabulario
        let mut vocabulario: Vec<String> = Vec::new();
        for (texto, _) in datos {
            let tokens = Tokenizer::tokenizar_limpio(texto);
            for t in tokens {
                if !vocabulario.contains(&t) {
                    vocabulario.push(t);
                }
            }
        }

        // Inicializar pesos por categoría
        for cat in CategoriaIntencion::todas() {
            let nombre = cat.nombre().to_string();
            let pesos: HashMap<String, f64> =
                vocabulario.iter().map(|v| (v.clone(), 0.0)).collect();
            self.pesos_ml.insert(nombre.clone(), pesos);
            self.sesgos_ml.insert(nombre, 0.0);
        }

        // Entrenamiento (one-vs-all logistic regression)
        for epoca in 0..epocas {
            let mut loss_total = 0.0;

            for &(texto, ref target) in datos {
                let tokens = Tokenizer::tokenizar_limpio(texto);

                for cat in CategoriaIntencion::todas() {
                    let nombre = cat.nombre().to_string();
                    let label = if cat == *target { 1.0 } else { 0.0 };

                    // Forward
                    let mut z = self.sesgos_ml.get(&nombre).copied().unwrap_or(0.0);
                    if let Some(pesos) = self.pesos_ml.get(&nombre) {
                        for t in &tokens {
                            z += pesos.get(t.as_str()).copied().unwrap_or(0.0);
                        }
                    }
                    let pred = sigmoid(z);
                    let error = pred - label;
                    loss_total += error * error;

                    // Backward
                    let grad = error * pred * (1.0 - pred);
                    if let Some(pesos) = self.pesos_ml.get_mut(&nombre) {
                        for t in &tokens {
                            if let Some(peso) = pesos.get_mut(t.as_str()) {
                                *peso -= lr * grad;
                            }
                        }
                    }
                    if let Some(sesgo) = self.sesgos_ml.get_mut(&nombre) {
                        *sesgo -= lr * grad;
                    }
                }
            }

            let n = datos.len() as f64 * CategoriaIntencion::todas().len() as f64;
            if (epoca + 1) % (epocas / 5).max(1) == 0 || epoca == 0 {
                println!(
                    "    Época {}/{} — MSE: {:.6}",
                    epoca + 1,
                    epocas,
                    loss_total / n
                );
            }
        }

        self.entrenado_ml = true;
    }

    /// Añadir al historial para aprendizaje futuro
    pub fn registrar(&mut self, texto: &str, categoria: CategoriaIntencion) {
        self.historial.push((texto.to_string(), categoria));
    }

    /// Consultar si la intención es ambigua
    pub fn es_ambigua(&self, intencion: &Intencion) -> bool {
        if intencion.confianza < 0.35 {
            return true;
        }
        if let Some((_, score_alt)) = intencion.alternativas.first() {
            if intencion.confianza - score_alt < 0.15 {
                return true;
            }
        }
        false
    }

    pub fn resumen(&self) {
        println!("  Clasificador de Intención");
        println!("  ─────────────────────────");
        println!("    Reglas: {} categorías", self.reglas.len());
        let total_patrones: usize = self.reglas.values().map(|v| v.len()).sum();
        println!("    Patrones: {}", total_patrones);
        println!(
            "    Modelo ML: {}",
            if self.entrenado_ml {
                "Entrenado"
            } else {
                "No entrenado"
            }
        );
        println!("    Historial: {} interacciones", self.historial.len());
    }
}

impl Default for ClasificadorIntencion {
    fn default() -> Self {
        Self::nuevo()
    }
}

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}
