//! Sincronización con servicios externos.
//!
//! Submódulos: Google Calendar, Google Drive, GitHub Gist, Email SMTP
//! y servidor web local para dashboard.

#[cfg(feature = "desktop")]
pub mod calendario;
#[cfg(feature = "desktop")]
pub mod correo;
#[cfg(feature = "desktop")]
pub mod drive;
#[cfg(feature = "desktop")]
pub mod gist;
pub mod servidor;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuración de sincronización (calendario y email)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    // Google Calendar OAuth2
    pub google_client_id: String,
    pub google_client_secret: String,
    pub google_access_token: Option<String>,
    pub google_refresh_token: Option<String>,
    pub google_calendar_id: String,

    // Email SMTP
    pub smtp_server: String,
    pub smtp_port: u16,
    pub smtp_usuario: String,
    pub smtp_password: String,
    pub email_remitente: String,
    pub email_destinatario: String,

    // GitHub Gist sync (método principal de sincronización)
    #[serde(default)]
    pub gist_token: String,
    #[serde(default)]
    pub gist_id: String,

    // Google Drive sync (deprecado — usar Gist)
    #[serde(default)]
    pub drive_file_id: String,

    // Mapeo de IDs locales → Google Calendar event IDs
    pub mapa_eventos: HashMap<String, String>,
    pub mapa_tareas: HashMap<String, String>,

    // Preferencias
    pub auto_sync: bool,
    pub notificar_follow_ups: bool,
    pub resumen_diario: bool,
}

impl Default for SyncConfig {
    fn default() -> Self {
        SyncConfig {
            google_client_id: String::new(),
            google_client_secret: String::new(),
            google_access_token: None,
            google_refresh_token: None,
            google_calendar_id: "primary".to_string(),

            smtp_server: String::new(),
            smtp_port: 587,
            smtp_usuario: String::new(),
            smtp_password: String::new(),
            email_remitente: String::new(),
            email_destinatario: String::new(),

            mapa_eventos: HashMap::new(),
            mapa_tareas: HashMap::new(),

            gist_token: String::new(),
            gist_id: String::new(),

            drive_file_id: String::new(),

            auto_sync: false,
            notificar_follow_ups: false,
            resumen_diario: false,
        }
    }
}

impl SyncConfig {
    pub fn google_configurado(&self) -> bool {
        !self.google_client_id.is_empty() && !self.google_client_secret.is_empty()
    }

    pub fn email_configurado(&self) -> bool {
        !self.smtp_server.is_empty()
            && !self.smtp_usuario.is_empty()
            && !self.email_remitente.is_empty()
            && !self.email_destinatario.is_empty()
    }

    pub fn google_autenticado(&self) -> bool {
        self.google_access_token.is_some()
    }

    pub fn gist_configurado(&self) -> bool {
        !self.gist_token.is_empty()
    }

    pub fn drive_configurado(&self) -> bool {
        !self.drive_file_id.is_empty()
    }
}
