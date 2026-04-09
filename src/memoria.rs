use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;

/// Un enlace entre cualquier elemento del sistema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Enlace {
    pub origen_modulo: String,    // "tarea", "evento", "diagrama", "canvas", "nota"
    pub origen_id: String,
    pub destino_modulo: String,
    pub destino_id: String,
    pub relacion: String,         // descripción libre
}

/// Una nota/recuerdo con palabras clave
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recuerdo {
    pub id: String,
    pub contenido: String,
    pub palabras_clave: Vec<String>,
    pub modulo_origen: Option<String>,
    pub item_id: Option<String>,
    pub creado: NaiveDateTime,
}

impl Recuerdo {
    pub fn new(contenido: String, palabras_clave: Vec<String>) -> Self {
        Recuerdo {
            id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
            contenido,
            palabras_clave: palabras_clave.into_iter().map(|p| p.to_lowercase()).collect(),
            modulo_origen: None,
            item_id: None,
            creado: chrono::Local::now().naive_local(),
        }
    }

    pub fn con_origen(mut self, modulo: &str, id: &str) -> Self {
        self.modulo_origen = Some(modulo.to_string());
        self.item_id = Some(id.to_string());
        self
    }
}

/// Sistema de memoria: conecta todo con todo
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Memoria {
    pub recuerdos: Vec<Recuerdo>,
    pub enlaces: Vec<Enlace>,
    /// Mapa de palabras clave → IDs de recuerdos para búsqueda rápida
    pub indice: HashMap<String, Vec<String>>,
    /// Diccionario neuronal de ideas interconectadas
    #[serde(default)]
    pub diccionario: Diccionario,
}

impl Memoria {
    pub fn new() -> Self {
        Memoria {
            recuerdos: Vec::new(),
            enlaces: Vec::new(),
            indice: HashMap::new(),
            diccionario: Diccionario::new(),
        }
    }

    pub fn agregar_recuerdo(&mut self, recuerdo: Recuerdo) {
        let id = recuerdo.id.clone();
        for palabra in &recuerdo.palabras_clave {
            self.indice
                .entry(palabra.clone())
                .or_default()
                .push(id.clone());
        }
        self.recuerdos.push(recuerdo);
    }

    pub fn enlazar(&mut self, origen_modulo: &str, origen_id: &str, destino_modulo: &str, destino_id: &str, relacion: &str) {
        self.enlaces.push(Enlace {
            origen_modulo: origen_modulo.to_string(),
            origen_id: origen_id.to_string(),
            destino_modulo: destino_modulo.to_string(),
            destino_id: destino_id.to_string(),
            relacion: relacion.to_string(),
        });
    }

    /// Buscar por palabra clave en todo el sistema
    pub fn buscar(&self, consulta: &str) -> Vec<&Recuerdo> {
        let q = consulta.to_lowercase();
        self.recuerdos
            .iter()
            .filter(|r| {
                r.palabras_clave.iter().any(|p| p.contains(&q))
                    || r.contenido.to_lowercase().contains(&q)
            })
            .collect()
    }

    /// Obtener enlaces de un item
    pub fn enlaces_de(&self, modulo: &str, id: &str) -> Vec<&Enlace> {
        self.enlaces
            .iter()
            .filter(|e| {
                (e.origen_modulo == modulo && e.origen_id == id)
                    || (e.destino_modulo == modulo && e.destino_id == id)
            })
            .collect()
    }

    /// Obtener todas las palabras clave únicas
    pub fn palabras_clave(&self) -> Vec<&String> {
        self.indice.keys().collect()
    }

    /// Eliminar un recuerdo por ID
    pub fn eliminar_recuerdo(&mut self, id: &str) -> bool {
        let len_antes = self.recuerdos.len();
        // Quitar del índice
        if let Some(r) = self.recuerdos.iter().find(|r| r.id == id) {
            for palabra in &r.palabras_clave {
                if let Some(ids) = self.indice.get_mut(palabra) {
                    ids.retain(|rid| rid != id);
                    if ids.is_empty() {
                        self.indice.remove(palabra);
                    }
                }
            }
        }
        self.recuerdos.retain(|r| r.id != id);
        self.recuerdos.len() != len_antes
    }

    /// Eliminar una palabra clave de un recuerdo específico
    pub fn quitar_palabra_de_recuerdo(&mut self, recuerdo_id: &str, palabra: &str) -> bool {
        let p = palabra.to_lowercase();
        let mut encontrada = false;
        if let Some(r) = self.recuerdos.iter_mut().find(|r| r.id == recuerdo_id) {
            let antes = r.palabras_clave.len();
            r.palabras_clave.retain(|pc| pc != &p);
            encontrada = r.palabras_clave.len() != antes;
        }
        // Actualizar índice
        if encontrada {
            if let Some(ids) = self.indice.get_mut(&p) {
                ids.retain(|rid| rid != recuerdo_id);
                if ids.is_empty() {
                    self.indice.remove(&p);
                }
            }
        }
        encontrada
    }

    /// Eliminar una palabra clave globalmente (de todos los recuerdos)
    pub fn eliminar_palabra_global(&mut self, palabra: &str) -> usize {
        let p = palabra.to_lowercase();
        let mut count = 0;
        for r in &mut self.recuerdos {
            let antes = r.palabras_clave.len();
            r.palabras_clave.retain(|pc| pc != &p);
            if r.palabras_clave.len() != antes {
                count += 1;
            }
        }
        self.indice.remove(&p);
        count
    }

    /// Agregar palabra clave a un recuerdo existente
    pub fn agregar_palabra_a_recuerdo(&mut self, recuerdo_id: &str, palabra: &str) -> bool {
        let p = palabra.to_lowercase();
        if let Some(r) = self.recuerdos.iter_mut().find(|r| r.id == recuerdo_id) {
            if !r.palabras_clave.contains(&p) {
                r.palabras_clave.push(p.clone());
                self.indice.entry(p).or_default().push(recuerdo_id.to_string());
                return true;
            }
        }
        false
    }

    /// Editar el contenido de un recuerdo
    pub fn editar_contenido(&mut self, recuerdo_id: &str, nuevo_contenido: String) -> bool {
        if let Some(r) = self.recuerdos.iter_mut().find(|r| r.id == recuerdo_id) {
            r.contenido = nuevo_contenido;
            return true;
        }
        false
    }

    /// Obtener recuerdos por palabra clave exacta
    pub fn recuerdos_con_palabra(&self, palabra: &str) -> Vec<&Recuerdo> {
        let p = palabra.to_lowercase();
        self.recuerdos
            .iter()
            .filter(|r| r.palabras_clave.contains(&p))
            .collect()
    }
}

// ══════════════════════════════════════════════════════════════
//  Diccionario Neuronal de Ideas
// ══════════════════════════════════════════════════════════════

/// Una conexión neuronal entre dos ideas/palabras clave
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConexionIdea {
    pub palabra_a: String,
    pub palabra_b: String,
    pub fuerza: u32,        // se incrementa cada vez que co-ocurren
    pub contexto: Vec<String>, // breves notas de por qué se conectaron
}

/// Diccionario neuronal: mapa de ideas interconectadas que crece con el uso
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Diccionario {
    /// Todas las conexiones entre pares de palabras
    pub conexiones: Vec<ConexionIdea>,
    /// Mapa rápido: palabra → índices de conexiones donde aparece
    pub grafo: HashMap<String, Vec<usize>>,
    /// Historial: qué tareas/items generaron qué ideas
    pub historial: Vec<EntradaDiccionario>,
}

/// Registro de cuándo y por qué se agregó una idea
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntradaDiccionario {
    pub palabras: Vec<String>,
    pub modulo: String,       // "tarea", "evento", etc.
    pub item_id: String,
    pub item_titulo: String,
    pub nota: String,         // contexto libre del usuario
    pub creado: NaiveDateTime,
}

impl Diccionario {
    pub fn new() -> Self {
        Diccionario {
            conexiones: Vec::new(),
            grafo: HashMap::new(),
            historial: Vec::new(),
        }
    }

    /// Registrar un conjunto de palabras como conectadas entre sí (grafo completo entre ellas)
    pub fn conectar_palabras(&mut self, palabras: &[String], contexto: &str) {
        let palabras_lc: Vec<String> = palabras.iter().map(|p| p.to_lowercase()).collect();
        for i in 0..palabras_lc.len() {
            for j in (i + 1)..palabras_lc.len() {
                let a = &palabras_lc[i];
                let b = &palabras_lc[j];
                self.reforzar_o_crear(a, b, contexto);
            }
        }
    }

    /// Reforzar conexión existente o crear nueva, con contexto
    fn reforzar_o_crear(&mut self, a: &str, b: &str, contexto: &str) {
        let idx = self.reforzar_o_crear_idx(a, b);
        if !contexto.is_empty() && !self.conexiones[idx].contexto.contains(&contexto.to_string()) {
            self.conexiones[idx].contexto.push(contexto.to_string());
        }
    }

    fn reforzar_o_crear_idx(&mut self, a: &str, b: &str) -> usize {
        if let Some(idx) = self.conexiones.iter().position(|c| {
            (c.palabra_a == a && c.palabra_b == b) || (c.palabra_a == b && c.palabra_b == a)
        }) {
            self.conexiones[idx].fuerza += 1;
            idx
        } else {
            let idx = self.conexiones.len();
            self.conexiones.push(ConexionIdea {
                palabra_a: a.to_string(),
                palabra_b: b.to_string(),
                fuerza: 1,
                contexto: Vec::new(),
            });
            self.grafo.entry(a.to_string()).or_default().push(idx);
            self.grafo.entry(b.to_string()).or_default().push(idx);
            idx
        }
    }

    /// Registrar una entrada de historial
    pub fn registrar(&mut self, modulo: &str, item_id: &str, titulo: &str, palabras: &[String], nota: &str) {
        self.historial.push(EntradaDiccionario {
            palabras: palabras.to_vec(),
            modulo: modulo.to_string(),
            item_id: item_id.to_string(),
            item_titulo: titulo.to_string(),
            nota: nota.to_string(),
            creado: chrono::Local::now().naive_local(),
        });
        // Conectar todas las palabras entre sí
        self.conectar_palabras(palabras, nota);
    }

    /// Obtener ideas relacionadas a una palabra (vecinos en el grafo)
    pub fn ideas_relacionadas(&self, palabra: &str) -> Vec<(&str, u32)> {
        let p = palabra.to_lowercase();
        let mut resultado: Vec<(&str, u32)> = Vec::new();
        if let Some(indices) = self.grafo.get(&p) {
            for &idx in indices {
                let c = &self.conexiones[idx];
                let otra = if c.palabra_a == p { &c.palabra_b } else { &c.palabra_a };
                resultado.push((otra.as_str(), c.fuerza));
            }
        }
        resultado.sort_by(|a, b| b.1.cmp(&a.1)); // más fuertes primero
        resultado
    }

    /// Sugerir conexiones: dado un set de palabras, buscar qué otras ideas se relacionan
    pub fn sugerir(&self, palabras: &[String]) -> Vec<(String, u32)> {
        let input_set: HashSet<String> = palabras.iter().map(|p| p.to_lowercase()).collect();
        let mut scores: HashMap<String, u32> = HashMap::new();
        for p in &input_set {
            for (rel, fuerza) in self.ideas_relacionadas(p) {
                if !input_set.contains(rel) {
                    *scores.entry(rel.to_string()).or_default() += fuerza;
                }
            }
        }
        let mut resultado: Vec<(String, u32)> = scores.into_iter().collect();
        resultado.sort_by(|a, b| b.1.cmp(&a.1));
        resultado
    }

    /// Todas las palabras únicas en el diccionario
    pub fn todas_las_ideas(&self) -> Vec<&String> {
        self.grafo.keys().collect()
    }

    /// Historial de un módulo específico
    pub fn historial_modulo(&self, modulo: &str) -> Vec<&EntradaDiccionario> {
        self.historial.iter().filter(|e| e.modulo == modulo).collect()
    }
}
