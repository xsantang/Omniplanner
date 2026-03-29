use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::tasks::TaskManager;
use crate::agenda::Agenda;
use crate::canvas::Canvas;
use crate::diagrams::Diagrama;
use crate::vcs::DataVcs;
use crate::mapper::Mapper;
use crate::memoria::Memoria;
use crate::sync::SyncConfig;

/// Estado completo de la aplicación (persistible)
#[derive(Debug, Serialize, Deserialize)]
pub struct AppState {
    pub tasks: TaskManager,
    pub agenda: Agenda,
    pub canvases: Vec<Canvas>,
    pub diagramas: Vec<Diagrama>,
    pub vcs: DataVcs,
    pub mapper: Mapper,
    #[serde(default)]
    pub memoria: Memoria,
    #[serde(default)]
    pub sync: SyncConfig,
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            tasks: TaskManager::new(),
            agenda: Agenda::new(),
            canvases: Vec::new(),
            diagramas: Vec::new(),
            vcs: DataVcs::new(),
            mapper: Mapper::new(),
            memoria: Memoria::new(),
            sync: SyncConfig::default(),
        }
    }

    pub fn ruta_datos() -> PathBuf {
        let dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("omniplanner");
        fs::create_dir_all(&dir).ok();
        dir.join("data.json")
    }

    pub fn guardar(&self) -> Result<(), String> {
        let ruta = Self::ruta_datos();
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Error serializando: {}", e))?;
        fs::write(&ruta, json)
            .map_err(|e| format!("Error escribiendo {}: {}", ruta.display(), e))?;
        Ok(())
    }

    pub fn cargar() -> Result<Self, String> {
        let ruta = Self::ruta_datos();
        if !ruta.exists() {
            return Ok(Self::new());
        }
        let contenido = fs::read_to_string(&ruta)
            .map_err(|e| format!("Error leyendo {}: {}", ruta.display(), e))?;
        serde_json::from_str(&contenido)
            .map_err(|e| format!("Error deserializando: {}", e))
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carga_datos_existentes() {
        // Verifica que los datos del formato anterior (sin campo memoria) se cargan bien
        match AppState::cargar() {
            Ok(s) => {
                println!("Datos cargados: {} tareas, {} eventos, {} diagramas, {} recuerdos",
                    s.tasks.tareas.len(), s.agenda.eventos.len(),
                    s.diagramas.len(), s.memoria.recuerdos.len());
            }
            Err(e) => panic!("Error cargando datos: {}", e),
        }
    }

    #[test]
    fn json_sin_memoria_deserializa() {
        let json = r#"{
            "tasks": {"tareas": []},
            "agenda": {"eventos": [], "horarios_escritura": []},
            "canvases": [],
            "diagramas": [],
            "vcs": {"snapshots": [], "rama_actual": "main", "ramas": [{"nombre": "main", "snapshot_ids": []}]},
            "mapper": {"esquemas": []}
        }"#;
        let state: AppState = serde_json::from_str(json).unwrap();
        assert!(state.memoria.recuerdos.is_empty());
        assert!(state.memoria.enlaces.is_empty());
    }
}
