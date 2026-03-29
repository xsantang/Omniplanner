use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
}

impl Memoria {
    pub fn new() -> Self {
        Memoria {
            recuerdos: Vec::new(),
            enlaces: Vec::new(),
            indice: HashMap::new(),
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
