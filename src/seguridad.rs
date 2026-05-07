//! # seguridad — Capa de seguridad transversal de Omniplanner
//!
//! Provee:
//! - **PIN de sesión**: protege el acceso a la aplicación en cada arranque.
//! - **Cifrado de datos sensibles**: envuelve el JSON de `AppState` con
//!   AES-256-GCM + Argon2id antes de escribirlo a disco.
//! - **Doble confirmación**: helper para operaciones financieras destructivas.
//! - **Auditoría**: registro inmutable de eventos de seguridad (login, intento
//!   fallido, operación crítica, exportación de datos).
//! - **Validación de entradas**: sanitización de strings antes de procesarlos.
//! - **Límite de intentos**: bloqueo temporal tras N intentos fallidos de PIN.
//!
//! ## Arquitectura
//!
//! ```text
//!  CLI / main.rs
//!       │
//!       ▼
//!  seguridad::SesionSegura   ←  PIN + intentos + bloqueo
//!       │
//!       ▼
//!  seguridad::CifradoDatos   ←  cifrar/descifrar AppState JSON
//!       │                        (AES-256-GCM, clave ← Argon2id(PIN))
//!       ▼
//!  storage::AppState          ←  datos en memoria (sin cifrado interno)
//! ```

use chrono::{Local, NaiveDateTime};
use serde::{Deserialize, Serialize};

use crate::cripto::{
    cifrar_aes_gcm, derivar_clave_maestra, descifrar_aes_gcm, ParamsKdf, SobreAesGcm,
    AES256_KEY_LEN, ARGON2_SALT_LEN,
};

// ═══════════════════════════════════════════════════════════════════════
//  Constantes
// ═══════════════════════════════════════════════════════════════════════

/// Intentos máximos de PIN antes de bloqueo temporal.
pub const MAX_INTENTOS_PIN: u32 = 5;
/// Segundos de bloqueo tras agotar intentos.
pub const SEGUNDOS_BLOQUEO: i64 = 300; // 5 minutos
/// Longitud mínima del PIN.
pub const PIN_MIN_LEN: usize = 4;
/// Longitud máxima del PIN.
pub const PIN_MAX_LEN: usize = 32;

// ═══════════════════════════════════════════════════════════════════════
//  Errores de seguridad
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, PartialEq)]
pub enum ErrorSeguridad {
    PinIncorrecto { intentos_restantes: u32 },
    PinBloqueado { segundos_restantes: i64 },
    PinDemasiadoCorto,
    PinDemasiadoLargo,
    CifradoFallido(String),
    DescifradoFallido,
    DatosCorruptos,
    OperacionCancelada,
    EntradaInvalida(String),
}

impl std::fmt::Display for ErrorSeguridad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PinIncorrecto { intentos_restantes } => write!(
                f,
                "PIN incorrecto. Intentos restantes: {}",
                intentos_restantes
            ),
            Self::PinBloqueado { segundos_restantes } => write!(
                f,
                "Acceso bloqueado. Espera {} segundos.",
                segundos_restantes
            ),
            Self::PinDemasiadoCorto => write!(f, "PIN muy corto (mínimo {} caracteres).", PIN_MIN_LEN),
            Self::PinDemasiadoLargo => write!(f, "PIN muy largo (máximo {} caracteres).", PIN_MAX_LEN),
            Self::CifradoFallido(m) => write!(f, "Error al cifrar: {}", m),
            Self::DescifradoFallido => write!(f, "No se pudo descifrar — PIN incorrecto o datos dañados."),
            Self::DatosCorruptos => write!(f, "Datos corruptos o modificados externamente."),
            Self::OperacionCancelada => write!(f, "Operación cancelada por el usuario."),
            Self::EntradaInvalida(m) => write!(f, "Entrada inválida: {}", m),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Estado de sesión persistido (no contiene el PIN en texto plano)
// ═══════════════════════════════════════════════════════════════════════

/// Configuración persistida de seguridad (se guarda junto con los datos cifrados).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSeguridad {
    /// ¿Está activada la protección por PIN?
    pub pin_activo: bool,
    /// Hash Argon2id del PIN (no el PIN en texto plano).
    /// Formato: `$argon2id$v=19$...` (PHC string format).
    pub pin_hash: Option<String>,
    /// Salt usado al derivar la clave de cifrado desde el PIN.
    /// En base64. Se regenera al cambiar el PIN.
    #[serde(default)]
    pub salt_cifrado_b64: String,
    /// Parámetros Argon2id usados (para que el costo pueda cambiar en futuras versiones).
    #[serde(default)]
    pub argon2_m_cost_kib: u32,
    #[serde(default)]
    pub argon2_t_cost: u32,
    #[serde(default)]
    pub argon2_p_cost: u32,
    /// ¿Requiere confirmación doble para borrar datos financieros?
    #[serde(default = "verdadero")]
    pub confirmar_borrado_financiero: bool,
    /// ¿Requiere confirmación doble para pagos > N pesos?
    #[serde(default = "verdadero")]
    pub confirmar_pagos_grandes: bool,
    /// Umbral (en la moneda local) para pedir doble confirmación.
    #[serde(default = "umbral_pago_default")]
    pub umbral_pago_confirmacion: f64,
    /// Timestamp del último login exitoso.
    #[serde(default)]
    pub ultimo_login: Option<String>,
    /// Timestamp del último intento fallido.
    #[serde(default)]
    pub ultimo_intento_fallido: Option<String>,
    /// Contador de intentos fallidos consecutivos.
    #[serde(default)]
    pub intentos_fallidos: u32,
    /// Timestamp de inicio del bloqueo (si aplica).
    #[serde(default)]
    pub bloqueado_hasta: Option<String>,
}

fn verdadero() -> bool { true }
fn umbral_pago_default() -> f64 { 5000.0 }

impl Default for ConfigSeguridad {
    fn default() -> Self {
        let params = ParamsKdf::default();
        Self {
            pin_activo: false,
            pin_hash: None,
            salt_cifrado_b64: String::new(),
            argon2_m_cost_kib: params.m_cost_kib,
            argon2_t_cost: params.t_cost,
            argon2_p_cost: params.p_cost,
            confirmar_borrado_financiero: true,
            confirmar_pagos_grandes: true,
            umbral_pago_confirmacion: 5000.0,
            ultimo_login: None,
            ultimo_intento_fallido: None,
            intentos_fallidos: 0,
            bloqueado_hasta: None,
        }
    }
}

impl ConfigSeguridad {
    /// Devuelve los `ParamsKdf` almacenados (para coherencia al descifrar).
    pub fn params_kdf(&self) -> ParamsKdf {
        ParamsKdf {
            m_cost_kib: if self.argon2_m_cost_kib == 0 { ParamsKdf::default().m_cost_kib } else { self.argon2_m_cost_kib },
            t_cost: if self.argon2_t_cost == 0 { ParamsKdf::default().t_cost } else { self.argon2_t_cost },
            p_cost: if self.argon2_p_cost == 0 { ParamsKdf::default().p_cost } else { self.argon2_p_cost },
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Registro de auditoría
// ═══════════════════════════════════════════════════════════════════════

/// Tipo de evento de auditoría.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TipoAuditoria {
    LoginExitoso,
    LoginFallido,
    BloqueoActivado,
    PinCambiado,
    PinDesactivado,
    PinActivado,
    DatosCifrados,
    DatosDescifrados,
    OperacionCritica { descripcion: String },
    ExportacionDatos { modulo: String, formato: String },
    BorradoDatos { modulo: String },
    AccesoContrasenias,
    AccesoDatosFinancieros,
    PagoRegistrado { monto: f64 },
    DeudaModificada { nombre: String },
}

impl std::fmt::Display for TipoAuditoria {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoginExitoso => write!(f, "✅ Login exitoso"),
            Self::LoginFallido => write!(f, "❌ Intento de login fallido"),
            Self::BloqueoActivado => write!(f, "🔒 Acceso bloqueado por intentos fallidos"),
            Self::PinCambiado => write!(f, "🔑 PIN cambiado"),
            Self::PinDesactivado => write!(f, "⚠️  PIN desactivado"),
            Self::PinActivado => write!(f, "🔐 PIN activado"),
            Self::DatosCifrados => write!(f, "🔒 Datos cifrados y guardados"),
            Self::DatosDescifrados => write!(f, "🔓 Datos descifrados y cargados"),
            Self::OperacionCritica { descripcion } => write!(f, "⚡ Operación crítica: {}", descripcion),
            Self::ExportacionDatos { modulo, formato } => write!(f, "📤 Exportación: {} → {}", modulo, formato),
            Self::BorradoDatos { modulo } => write!(f, "🗑️  Borrado de datos: {}", modulo),
            Self::AccesoContrasenias => write!(f, "🔑 Acceso al gestor de contraseñas"),
            Self::AccesoDatosFinancieros => write!(f, "💰 Acceso a datos financieros"),
            Self::PagoRegistrado { monto } => write!(f, "💸 Pago registrado: ${:.2}", monto),
            Self::DeudaModificada { nombre } => write!(f, "📝 Deuda modificada: {}", nombre),
        }
    }
}

/// Entrada individual del registro de auditoría.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntradaAuditoria {
    pub id: String,
    pub timestamp: String,
    pub tipo: TipoAuditoria,
    pub descripcion_extra: Option<String>,
}

/// Registro de auditoría persistido.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RegistroAuditoria {
    pub entradas: Vec<EntradaAuditoria>,
}

impl RegistroAuditoria {
    /// Máximo de entradas a conservar (las más antiguas se eliminan).
    const MAX_ENTRADAS: usize = 500;

    /// Registra un evento de auditoría.
    pub fn registrar(&mut self, tipo: TipoAuditoria, extra: Option<&str>) {
        if self.entradas.len() >= Self::MAX_ENTRADAS {
            self.entradas.remove(0);
        }
        self.entradas.push(EntradaAuditoria {
            id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
            timestamp: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            tipo,
            descripcion_extra: extra.map(|s| s.to_string()),
        });
    }

    /// Devuelve las últimas `n` entradas (las más recientes primero).
    pub fn ultimas(&self, n: usize) -> Vec<&EntradaAuditoria> {
        self.entradas.iter().rev().take(n).collect()
    }

    /// Filtra por tipo de evento.
    pub fn por_tipo<F: Fn(&TipoAuditoria) -> bool>(&self, filtro: F) -> Vec<&EntradaAuditoria> {
        self.entradas.iter().filter(|e| filtro(&e.tipo)).collect()
    }

    /// Devuelve entradas de las últimas N horas.
    pub fn ultimas_horas(&self, horas: i64) -> Vec<&EntradaAuditoria> {
        let limite = Local::now().naive_local() - chrono::Duration::hours(horas);
        self.entradas
            .iter()
            .filter(|e| {
                NaiveDateTime::parse_from_str(&e.timestamp, "%Y-%m-%d %H:%M:%S")
                    .map(|ts| ts >= limite)
                    .unwrap_or(false)
            })
            .collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  SesionSegura — gestión de PIN + bloqueo
// ═══════════════════════════════════════════════════════════════════════

/// Maneja el ciclo de vida de la sesión: verificación de PIN, bloqueo y
/// actualización del estado de seguridad.
pub struct SesionSegura<'a> {
    pub config: &'a mut ConfigSeguridad,
    pub auditoria: &'a mut RegistroAuditoria,
}

impl<'a> SesionSegura<'a> {
    pub fn new(config: &'a mut ConfigSeguridad, auditoria: &'a mut RegistroAuditoria) -> Self {
        Self { config, auditoria }
    }

    /// Verifica si el acceso está bloqueado actualmente.
    pub fn esta_bloqueado(&self) -> bool {
        if let Some(ref hasta_str) = self.config.bloqueado_hasta {
            if let Ok(hasta) = NaiveDateTime::parse_from_str(hasta_str, "%Y-%m-%d %H:%M:%S") {
                return Local::now().naive_local() < hasta;
            }
        }
        false
    }

    /// Segundos restantes de bloqueo (0 si no está bloqueado).
    pub fn segundos_restantes_bloqueo(&self) -> i64 {
        if let Some(ref hasta_str) = self.config.bloqueado_hasta {
            if let Ok(hasta) = NaiveDateTime::parse_from_str(hasta_str, "%Y-%m-%d %H:%M:%S") {
                let restante = (hasta - Local::now().naive_local()).num_seconds();
                return restante.max(0);
            }
        }
        0
    }

    /// Verifica el PIN ingresado. Actualiza contadores y bloqueo.
    /// Devuelve la clave AES-256 derivada del PIN si es correcto.
    pub fn verificar_pin(&mut self, pin: &str) -> Result<[u8; AES256_KEY_LEN], ErrorSeguridad> {
        // Validar longitud antes de consultar al sistema
        if pin.len() < PIN_MIN_LEN {
            return Err(ErrorSeguridad::PinDemasiadoCorto);
        }
        if pin.len() > PIN_MAX_LEN {
            return Err(ErrorSeguridad::PinDemasiadoLargo);
        }

        // Verificar bloqueo
        if self.esta_bloqueado() {
            let restante = self.segundos_restantes_bloqueo();
            return Err(ErrorSeguridad::PinBloqueado { segundos_restantes: restante });
        }

        // Si no hay PIN configurado, derivar clave sin validar hash
        let pin_hash_guardado = match &self.config.pin_hash {
            Some(h) => h.clone(),
            None => {
                // Sin PIN configurado — derivar clave desde PIN como primera vez
                return self.derivar_clave_desde_pin(pin);
            }
        };

        // Verificar el hash Argon2id del PIN
        let ok = argon2::PasswordHash::new(&pin_hash_guardado)
            .ok()
            .and_then(|hash| {
                let argon = argon2::Argon2::default();
                argon2::PasswordVerifier::verify_password(&argon, pin.as_bytes(), &hash).ok()
            })
            .is_some();

        if ok {
            // Login exitoso — reiniciar contadores
            self.config.intentos_fallidos = 0;
            self.config.bloqueado_hasta = None;
            self.config.ultimo_login = Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
            self.auditoria.registrar(TipoAuditoria::LoginExitoso, None);
            self.derivar_clave_desde_pin(pin)
        } else {
            // Login fallido
            self.config.intentos_fallidos += 1;
            self.config.ultimo_intento_fallido =
                Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
            self.auditoria.registrar(TipoAuditoria::LoginFallido, None);

            if self.config.intentos_fallidos >= MAX_INTENTOS_PIN {
                let hasta = Local::now().naive_local()
                    + chrono::Duration::seconds(SEGUNDOS_BLOQUEO);
                self.config.bloqueado_hasta =
                    Some(hasta.format("%Y-%m-%d %H:%M:%S").to_string());
                self.auditoria.registrar(TipoAuditoria::BloqueoActivado, None);
                return Err(ErrorSeguridad::PinBloqueado {
                    segundos_restantes: SEGUNDOS_BLOQUEO,
                });
            }

            let restantes = MAX_INTENTOS_PIN - self.config.intentos_fallidos;
            Err(ErrorSeguridad::PinIncorrecto { intentos_restantes: restantes })
        }
    }

    /// Configura o cambia el PIN. Requiere el PIN actual si ya hay uno activo.
    pub fn configurar_pin(
        &mut self,
        pin_nuevo: &str,
        pin_actual: Option<&str>,
    ) -> Result<[u8; AES256_KEY_LEN], ErrorSeguridad> {
        // Validar longitud
        if pin_nuevo.len() < PIN_MIN_LEN {
            return Err(ErrorSeguridad::PinDemasiadoCorto);
        }
        if pin_nuevo.len() > PIN_MAX_LEN {
            return Err(ErrorSeguridad::PinDemasiadoLargo);
        }

        // Si ya había PIN activo, verificar el actual primero
        if self.config.pin_activo && self.config.pin_hash.is_some() {
            match pin_actual {
                Some(actual) => {
                    self.verificar_pin(actual)?;
                }
                None => return Err(ErrorSeguridad::OperacionCancelada),
            }
        }

        // Generar nuevo salt de cifrado
        let nuevo_salt = crate::cripto::bytes_aleatorios::<ARGON2_SALT_LEN>();
        self.config.salt_cifrado_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            nuevo_salt,
        );

        // Derivar y almacenar hash Argon2id del PIN (para verificación futura)
        let params = self.config.params_kdf();
        let argon = argon2::Argon2::new(
            argon2::Algorithm::Argon2id,
            argon2::Version::V0x13,
            argon2::Params::new(params.m_cost_kib, params.t_cost, params.p_cost, Some(32))
                .map_err(|e| ErrorSeguridad::CifradoFallido(e.to_string()))?,
        );
        let salt_hash = argon2::password_hash::SaltString::generate(&mut rand_core::OsRng);
        let hash = argon2::PasswordHasher::hash_password(&argon, pin_nuevo.as_bytes(), &salt_hash)
            .map_err(|e| ErrorSeguridad::CifradoFallido(e.to_string()))?
            .to_string();

        self.config.pin_hash = Some(hash);
        self.config.pin_activo = true;
        self.config.intentos_fallidos = 0;
        self.config.bloqueado_hasta = None;

        self.auditoria.registrar(
            if pin_actual.is_some() { TipoAuditoria::PinCambiado } else { TipoAuditoria::PinActivado },
            None,
        );

        // Derivar clave de cifrado con el salt recién generado
        let clave_params = self.config.params_kdf();
        derivar_clave_desde_pin_con_salt(pin_nuevo, &nuevo_salt, &clave_params)
    }

    /// Desactiva el PIN (requiere el PIN actual).
    pub fn desactivar_pin(&mut self, pin_actual: &str) -> Result<(), ErrorSeguridad> {
        self.verificar_pin(pin_actual)?;
        self.config.pin_activo = false;
        self.config.pin_hash = None;
        self.config.salt_cifrado_b64 = String::new();
        self.auditoria.registrar(TipoAuditoria::PinDesactivado, None);
        Ok(())
    }

    // ── Helpers privados ─────────────────────────────────────────────

    fn derivar_clave_desde_pin(&self, pin: &str) -> Result<[u8; AES256_KEY_LEN], ErrorSeguridad> {
        let params = self.config.params_kdf();
        if self.config.salt_cifrado_b64.is_empty() {
            // Primera vez — salt aleatorio (se persistirá al guardar config)
            let salt = crate::cripto::bytes_aleatorios::<ARGON2_SALT_LEN>();
            derivar_clave_desde_pin_con_salt(pin, &salt, &params)
        } else {
            let salt_bytes = base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                &self.config.salt_cifrado_b64,
            )
            .map_err(|_| ErrorSeguridad::DatosCorruptos)?;
            let mut salt = [0u8; ARGON2_SALT_LEN];
            if salt_bytes.len() != ARGON2_SALT_LEN {
                return Err(ErrorSeguridad::DatosCorruptos);
            }
            salt.copy_from_slice(&salt_bytes);
            derivar_clave_desde_pin_con_salt(pin, &salt, &params)
        }
    }
}

/// Deriva una clave AES-256 desde `pin` + `salt` usando Argon2id.
fn derivar_clave_desde_pin_con_salt(
    pin: &str,
    salt: &[u8; ARGON2_SALT_LEN],
    params: &ParamsKdf,
) -> Result<[u8; AES256_KEY_LEN], ErrorSeguridad> {
    derivar_clave_maestra(pin.as_bytes(), Some(*salt), params)
        .map(|(clave_vec, _)| {
            let mut arr = [0u8; AES256_KEY_LEN];
            arr.copy_from_slice(&clave_vec);
            arr
        })
        .map_err(|e| ErrorSeguridad::CifradoFallido(e.to_string()))
}

// ═══════════════════════════════════════════════════════════════════════
//  CifradoDatos — envuelve el JSON de AppState con AES-256-GCM
// ═══════════════════════════════════════════════════════════════════════

/// Datos cifrados listos para persistir a disco.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatosCifrados {
    /// Versión del formato (para migraciones futuras).
    pub version: u8,
    /// Salt de Argon2id en base64 (para derivar la clave desde el PIN al cargar).
    pub salt_b64: String,
    /// El JSON de AppState cifrado con AES-256-GCM.
    pub sobre: SobreAesGcm,
    /// Config de seguridad en texto plano (necesaria para saber si hay PIN).
    pub config_seguridad: ConfigSeguridad,
    /// Registro de auditoría en texto plano (para diagnóstico).
    pub auditoria: RegistroAuditoria,
    /// Timestamp de cuando se cifró.
    pub cifrado_en: String,
}

impl DatosCifrados {
    /// Cifra `json_estado` con la `clave` proporcionada.
    pub fn cifrar(
        json_estado: &str,
        clave: &[u8; AES256_KEY_LEN],
        salt: &[u8; ARGON2_SALT_LEN],
        config: ConfigSeguridad,
        auditoria: RegistroAuditoria,
    ) -> Result<Self, ErrorSeguridad> {
        let sobre = cifrar_aes_gcm(clave, json_estado.as_bytes())
            .map_err(|e| ErrorSeguridad::CifradoFallido(e.to_string()))?;

        Ok(Self {
            version: 1,
            salt_b64: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, salt),
            sobre,
            config_seguridad: config,
            auditoria,
            cifrado_en: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        })
    }

    /// Descifra y devuelve el JSON de AppState.
    pub fn descifrar(&self, clave: &[u8; AES256_KEY_LEN]) -> Result<String, ErrorSeguridad> {
        let bytes = descifrar_aes_gcm(clave, &self.sobre)
            .map_err(|_| ErrorSeguridad::DescifradoFallido)?;
        String::from_utf8(bytes)
            .map_err(|_| ErrorSeguridad::DatosCorruptos)
    }

    /// Serializa a JSON para guardarlo en disco.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn a_json(&self) -> Result<String, ErrorSeguridad> {
        serde_json::to_string_pretty(self)
            .map_err(|e| ErrorSeguridad::CifradoFallido(e.to_string()))
    }

    /// Deserializa desde JSON leído de disco.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn desde_json(json: &str) -> Result<Self, ErrorSeguridad> {
        serde_json::from_str(json).map_err(|_| ErrorSeguridad::DatosCorruptos)
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Doble confirmación para operaciones críticas
// ═══════════════════════════════════════════════════════════════════════

/// Nivel de confirmación requerido para una operación.
#[derive(Debug, Clone, PartialEq)]
pub enum NivelConfirmacion {
    /// Una confirmación simple (s/n).
    Simple,
    /// Requiere escribir una frase de confirmación exacta.
    FraseExacta(String),
    /// Requiere ingresar el PIN de sesión.
    Pin,
}

/// Describe una operación crítica que necesita confirmación.
#[derive(Debug, Clone)]
pub struct OperacionCritica {
    pub descripcion: String,
    pub advertencia: String,
    pub nivel: NivelConfirmacion,
}

impl OperacionCritica {
    /// Pago grande que supera el umbral configurado.
    pub fn pago_grande(monto: f64, umbral: f64) -> Self {
        Self {
            descripcion: format!("Registrar pago de ${:.2}", monto),
            advertencia: format!(
                "Este pago supera el umbral de ${:.2} configurado para doble confirmación.",
                umbral
            ),
            nivel: NivelConfirmacion::Simple,
        }
    }

    /// Borrado de datos de un módulo.
    pub fn borrar_datos(modulo: &str) -> Self {
        Self {
            descripcion: format!("Borrar todos los datos de «{}»", modulo),
            advertencia: "Esta acción es IRREVERSIBLE. Los datos se perderán permanentemente."
                .to_string(),
            nivel: NivelConfirmacion::FraseExacta(format!("borrar {}", modulo)),
        }
    }

    /// Exportación de datos sensibles.
    pub fn exportar_datos(modulo: &str, formato: &str) -> Self {
        Self {
            descripcion: format!("Exportar {} como {}", modulo, formato),
            advertencia: "Los datos exportados quedarán en texto plano en el archivo destino."
                .to_string(),
            nivel: NivelConfirmacion::Simple,
        }
    }

    /// Modificar una deuda existente.
    pub fn modificar_deuda(nombre: &str) -> Self {
        Self {
            descripcion: format!("Modificar deuda «{}»", nombre),
            advertencia: "Los cambios afectarán todos los cálculos y simulaciones de esta deuda."
                .to_string(),
            nivel: NivelConfirmacion::Simple,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Validación de entradas
// ═══════════════════════════════════════════════════════════════════════

/// Resultado de validar un campo de texto.
#[derive(Debug)]
pub struct ResultadoValidacion {
    pub valido: bool,
    pub errores: Vec<String>,
    pub valor_sanitizado: String,
}

impl ResultadoValidacion {
    fn ok(valor: String) -> Self {
        Self { valido: true, errores: Vec::new(), valor_sanitizado: valor }
    }
    fn error(errores: Vec<String>, valor: String) -> Self {
        Self { valido: false, errores, valor_sanitizado: valor }
    }
}

/// Valida y sanitiza un nombre de deuda/concepto.
pub fn validar_nombre(s: &str) -> ResultadoValidacion {
    let sanitizado = s.trim().to_string();
    let mut errores = Vec::new();

    if sanitizado.is_empty() {
        errores.push("El nombre no puede estar vacío.".to_string());
    }
    if sanitizado.len() > 100 {
        errores.push("El nombre no puede superar 100 caracteres.".to_string());
    }
    // Detectar caracteres de control o nulos
    if sanitizado.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
        errores.push("El nombre contiene caracteres de control no permitidos.".to_string());
    }

    if errores.is_empty() {
        ResultadoValidacion::ok(sanitizado)
    } else {
        ResultadoValidacion::error(errores, sanitizado)
    }
}

/// Valida un monto monetario.
pub fn validar_monto(valor: f64, min: f64, max: f64) -> ResultadoValidacion {
    let mut errores = Vec::new();

    if !valor.is_finite() {
        errores.push("El monto no puede ser infinito o NaN.".to_string());
    } else if valor < min {
        errores.push(format!("El monto mínimo es ${:.2}.", min));
    } else if valor > max {
        errores.push(format!("El monto máximo es ${:.2}.", max));
    }

    if errores.is_empty() {
        ResultadoValidacion::ok(format!("{:.2}", valor))
    } else {
        ResultadoValidacion::error(errores, format!("{:.2}", valor))
    }
}

/// Valida una tasa de interés mensual (0.0 – 1.0).
pub fn validar_tasa(tasa: f64) -> ResultadoValidacion {
    let mut errores = Vec::new();
    if !tasa.is_finite() {
        errores.push("La tasa no puede ser infinita o NaN.".to_string());
    } else if tasa < 0.0 {
        errores.push("La tasa no puede ser negativa.".to_string());
    } else if tasa > 1.0 {
        errores.push("La tasa mensual no puede superar 100% (1.0). ¿Ingresaste el valor en porcentaje en vez de decimal?".to_string());
    }

    if errores.is_empty() {
        ResultadoValidacion::ok(format!("{:.6}", tasa))
    } else {
        ResultadoValidacion::error(errores, format!("{:.6}", tasa))
    }
}

/// Valida un string que NO debe contener rutas de sistema o secuencias
/// de escape peligrosas (previene path traversal en nombres de archivo).
pub fn validar_nombre_archivo(s: &str) -> ResultadoValidacion {
    let sanitizado = s.trim().to_string();
    let mut errores = Vec::new();

    let prohibidos = ["..", "/", "\\", ":", "*", "?", "\"", "<", ">", "|", "\0"];
    for p in &prohibidos {
        if sanitizado.contains(p) {
            errores.push(format!("El nombre no puede contener «{}».", p));
        }
    }
    if sanitizado.is_empty() {
        errores.push("El nombre de archivo no puede estar vacío.".to_string());
    }
    if sanitizado.len() > 255 {
        errores.push("El nombre de archivo es demasiado largo.".to_string());
    }

    if errores.is_empty() {
        ResultadoValidacion::ok(sanitizado)
    } else {
        ResultadoValidacion::error(errores, sanitizado)
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Helpers de display para la CLI
// ═══════════════════════════════════════════════════════════════════════

/// Formatea el resumen de auditoría de las últimas horas para mostrarlo en CLI.
pub fn resumen_auditoria_reciente(auditoria: &RegistroAuditoria, horas: i64) -> String {
    let entradas = auditoria.ultimas_horas(horas);
    if entradas.is_empty() {
        return format!("  Sin actividad en las últimas {} horas.\n", horas);
    }
    let mut out = format!(
        "  📋 Actividad reciente (últimas {} horas) — {} eventos:\n\n",
        horas,
        entradas.len()
    );
    for e in &entradas {
        out.push_str(&format!("  {} │ {}\n", e.timestamp, e.tipo));
        if let Some(ref extra) = e.descripcion_extra {
            out.push_str(&format!("         │  {}\n", extra));
        }
    }
    out
}

/// Muestra un banner de advertencia de seguridad antes de operaciones sensibles.
pub fn banner_advertencia(advertencia: &str) -> String {
    format!(
        "\n  ⚠️  ADVERTENCIA DE SEGURIDAD\n  {}\n  {}\n  {}\n",
        "─".repeat(60),
        advertencia,
        "─".repeat(60),
    )
}
