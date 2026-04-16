//! Persistencia del estado de la aplicación a disco (JSON).
//!
//! [`AppState`] contiene todos los módulos y se serializa/deserializa
//! automáticamente desde `~/.omniplanner/data.json`.
//!
//! Incluye auto-sync con GitHub Gist y backups locales rotativos.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::agenda::Agenda;
use crate::canvas::Canvas;
use crate::contrasenias::AlmacenContrasenias;
use crate::diagrams::Diagrama;
use crate::mapper::Mapper;
use crate::memoria::Memoria;
use crate::ml::{AlmacenAsesor, AlmacenML, AlmacenPresupuesto};
use crate::nlp::AlmacenNLP;
use crate::sync::SyncConfig;
use crate::tasks::TaskManager;
use crate::vcs::DataVcs;

/// Máximo de backups locales a mantener
const MAX_BACKUPS: usize = 5;

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
    #[serde(default)]
    pub ml: AlmacenML,
    #[serde(default)]
    pub nlp: AlmacenNLP,
    #[serde(default)]
    pub asesor: AlmacenAsesor,
    #[serde(default)]
    pub presupuesto: AlmacenPresupuesto,
    #[serde(default)]
    pub contrasenias: AlmacenContrasenias,
    /// Timestamp de última modificación (epoch secs)
    #[serde(default)]
    pub ultima_modificacion: i64,
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
            ml: AlmacenML::default(),
            nlp: AlmacenNLP::default(),
            asesor: AlmacenAsesor::default(),
            presupuesto: AlmacenPresupuesto::default(),
            contrasenias: AlmacenContrasenias::default(),
            ultima_modificacion: 0,
        }
    }

    pub fn ruta_datos() -> PathBuf {
        let dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("omniplanner");
        fs::create_dir_all(&dir).ok();
        dir.join("data.json")
    }

    /// Rota backups locales: data.json.1, data.json.2, ... data.json.{MAX_BACKUPS}
    fn rotar_backups() {
        let ruta = Self::ruta_datos();
        if !ruta.exists() {
            return;
        }
        // Eliminar el backup más viejo
        let mas_viejo = format!("{}.{}", ruta.display(), MAX_BACKUPS);
        fs::remove_file(&mas_viejo).ok();
        // Rotar: .4 → .5, .3 → .4, ... .1 → .2
        for i in (1..MAX_BACKUPS).rev() {
            let de = format!("{}.{}", ruta.display(), i);
            let a = format!("{}.{}", ruta.display(), i + 1);
            fs::rename(&de, &a).ok();
        }
        // Copiar actual como .1
        let backup_1 = format!("{}.1", ruta.display());
        fs::copy(&ruta, &backup_1).ok();
    }

    pub fn guardar(&mut self) -> Result<(), String> {
        let ruta = Self::ruta_datos();

        // Rotar backups antes de escribir
        Self::rotar_backups();

        // Actualizar timestamp
        self.ultima_modificacion = chrono::Utc::now().timestamp();

        let json =
            serde_json::to_string_pretty(self).map_err(|e| format!("Error serializando: {}", e))?;
        fs::write(&ruta, &json)
            .map_err(|e| format!("Error escribiendo {}: {}", ruta.display(), e))?;

        // Auto-sync a Gist (silencioso, no bloquea en error)
        if self.sync.auto_sync && self.sync.gist_configurado() {
            match crate::sync::gist::gist_push(&self.sync, &json) {
                Ok(gist_id) => {
                    if self.sync.gist_id.is_empty() || self.sync.gist_id != gist_id {
                        self.sync.gist_id = gist_id;
                        // Re-guardar para persistir el nuevo gist_id
                        let json2 = serde_json::to_string_pretty(self).unwrap_or_default();
                        fs::write(&ruta, json2).ok();
                    }
                }
                Err(_e) => {
                    // Sync falló silenciosamente — datos locales están a salvo
                    // Se reintentará en el próximo guardar()
                }
            }
        }

        Ok(())
    }

    pub fn cargar() -> Result<Self, String> {
        let ruta = Self::ruta_datos();
        if !ruta.exists() {
            return Ok(Self::new());
        }

        // Intentar cargar local
        let mut state = match fs::read_to_string(&ruta) {
            Ok(contenido) => match serde_json::from_str::<Self>(&contenido) {
                Ok(s) => s,
                Err(e) => {
                    // JSON corrupto — intentar restaurar desde backup
                    eprintln!("  ⚠ data.json corrupto ({}), buscando backup...", e);
                    Self::restaurar_desde_backup()?
                }
            },
            Err(e) => {
                eprintln!("  ⚠ Error leyendo data.json ({}), buscando backup...", e);
                Self::restaurar_desde_backup()?
            }
        };

        // Auto-pull desde Gist si está configurado
        if state.sync.auto_sync && state.sync.gist_configurado() && !state.sync.gist_id.is_empty() {
            if let Ok(contenido_remoto) = crate::sync::gist::gist_pull(&state.sync) {
                if let Ok(remoto) = serde_json::from_str::<Self>(&contenido_remoto) {
                    if remoto.ultima_modificacion > state.ultima_modificacion {
                        // El remoto es más nuevo — actualizar datos pero preservar config local
                        let sync_local = state.sync.clone();
                        state = remoto;
                        state.sync.gist_token = sync_local.gist_token;
                        state.sync.gist_id = sync_local.gist_id;
                        state.sync.auto_sync = sync_local.auto_sync;
                        state.sync.google_client_id = sync_local.google_client_id;
                        state.sync.google_client_secret = sync_local.google_client_secret;
                        state.sync.google_access_token = sync_local.google_access_token;
                        state.sync.google_refresh_token = sync_local.google_refresh_token;
                        state.sync.drive_file_id = sync_local.drive_file_id;
                        state.sync.smtp_server = sync_local.smtp_server;
                        state.sync.smtp_port = sync_local.smtp_port;
                        state.sync.smtp_usuario = sync_local.smtp_usuario;
                        state.sync.smtp_password = sync_local.smtp_password;
                        state.sync.email_remitente = sync_local.email_remitente;
                        state.sync.email_destinatario = sync_local.email_destinatario;
                        // Guardar la versión más nueva localmente
                        let json = serde_json::to_string_pretty(&state).unwrap_or_default();
                        fs::write(&ruta, json).ok();
                        eprintln!("  ☁ Datos actualizados desde la nube.");
                    }
                }
            }
        }

        Ok(state)
    }

    /// Intenta restaurar desde backups locales (.1, .2, ...) o desde Gist
    fn restaurar_desde_backup() -> Result<Self, String> {
        let ruta = Self::ruta_datos();

        // Intentar backups locales
        for i in 1..=MAX_BACKUPS {
            let backup = format!("{}.{}", ruta.display(), i);
            if let Ok(contenido) = fs::read_to_string(&backup) {
                if let Ok(state) = serde_json::from_str::<Self>(&contenido) {
                    eprintln!("  ✓ Restaurado desde backup local #{}", i);
                    // Reescribir data.json con el backup bueno
                    fs::write(&ruta, &contenido).ok();
                    return Ok(state);
                }
            }
        }

        // Último recurso: restaurar desde Gist
        // Crear un state temporal para leer sync config del backup más reciente
        eprintln!("  ⚠ Sin backups locales válidos. Intentando restaurar desde Gist...");

        // Intentar leer al menos el token de algún backup parcial
        for i in 1..=MAX_BACKUPS {
            let backup = format!("{}.{}", ruta.display(), i);
            if let Ok(contenido) = fs::read_to_string(&backup) {
                // Intentar extraer al menos el token del JSON parcialmente corrupto
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&contenido) {
                    let token = val["sync"]["gist_token"].as_str().unwrap_or("");
                    let gist_id = val["sync"]["gist_id"].as_str().unwrap_or("");
                    if !token.is_empty() && !gist_id.is_empty() {
                        let cfg = SyncConfig {
                            gist_token: token.to_string(),
                            gist_id: gist_id.to_string(),
                            ..SyncConfig::default()
                        };
                        if let Ok(contenido_remoto) = crate::sync::gist::gist_pull(&cfg) {
                            if let Ok(state) = serde_json::from_str::<Self>(&contenido_remoto) {
                                eprintln!("  ✓ Restaurado desde GitHub Gist.");
                                fs::write(&ruta, &contenido_remoto).ok();
                                return Ok(state);
                            }
                        }
                    }
                }
            }
        }

        Err("No se pudo restaurar datos. Archivo corrupto y sin backups disponibles.".to_string())
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
                println!(
                    "Datos cargados: {} tareas, {} eventos, {} diagramas, {} recuerdos",
                    s.tasks.tareas.len(),
                    s.agenda.eventos.len(),
                    s.diagramas.len(),
                    s.memoria.recuerdos.len()
                );
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
