//! Gestión de Proveedores y Outreach — Coordinación de Cuidado.
//!
//! Cubre el ciclo completo de relación con proveedores:
//!   Directorio → Llamadas entrantes/salientes → Campañas de outreach →
//!   Seguimiento programado → Métricas de engagement.
//!
//! # Tipos principales
//! - [`Proveedor`]            — médico, grupo o instalación
//! - [`InteraccionProveedor`] — registro de llamada, email, fax, portal
//! - [`SeguimientoProveedor`] — cita de seguimiento programada
//! - [`CampanaOutreach`]      — campaña estructurada de outreach
//! - [`AlmacenProveedores`]   — almacén persistible en JSON

use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ══════════════════════════════════════════════════════════════════════════════
//  Enums
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TipoInteraccion {
    LlamadaSaliente,
    LlamadaEntrante,
    Email,
    Fax,
    Portal,
    Visita,
}

impl TipoInteraccion {
    pub fn nombre(&self) -> &str {
        match self {
            TipoInteraccion::LlamadaSaliente => "Llamada Saliente",
            TipoInteraccion::LlamadaEntrante => "Llamada Entrante",
            TipoInteraccion::Email => "Email",
            TipoInteraccion::Fax => "Fax",
            TipoInteraccion::Portal => "Portal",
            TipoInteraccion::Visita => "Visita",
        }
    }

    pub fn es_llamada(&self) -> bool {
        matches!(
            self,
            TipoInteraccion::LlamadaSaliente | TipoInteraccion::LlamadaEntrante
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ResultadoInteraccion {
    Completado,
    SinContestacion,
    Voicemail,
    Transferido,
    Rechazado,
    PendienteRespuesta,
    Reprogramado,
}

impl ResultadoInteraccion {
    pub fn nombre(&self) -> &str {
        match self {
            ResultadoInteraccion::Completado => "Completado",
            ResultadoInteraccion::SinContestacion => "Sin Contestación",
            ResultadoInteraccion::Voicemail => "Voicemail",
            ResultadoInteraccion::Transferido => "Transferido",
            ResultadoInteraccion::Rechazado => "Rechazado",
            ResultadoInteraccion::PendienteRespuesta => "Pendiente de Respuesta",
            ResultadoInteraccion::Reprogramado => "Reprogramado",
        }
    }

    pub fn exitoso(&self) -> bool {
        matches!(self, ResultadoInteraccion::Completado)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NivelEngagement {
    Nuevo,
    Activo,
    Inactivo,
    NoParticipa,
}

impl NivelEngagement {
    pub fn nombre(&self) -> &str {
        match self {
            NivelEngagement::Nuevo => "Nuevo",
            NivelEngagement::Activo => "Activo",
            NivelEngagement::Inactivo => "Inactivo",
            NivelEngagement::NoParticipa => "No Participa",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EstadoCampana {
    Planificada,
    Activa,
    Pausada,
    Completada,
    Cancelada,
}

impl EstadoCampana {
    pub fn nombre(&self) -> &str {
        match self {
            EstadoCampana::Planificada => "Planificada",
            EstadoCampana::Activa => "Activa",
            EstadoCampana::Pausada => "Pausada",
            EstadoCampana::Completada => "Completada",
            EstadoCampana::Cancelada => "Cancelada",
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Proveedor
// ══════════════════════════════════════════════════════════════════════════════

/// Proveedor de salud: médico, grupo médico, hospital, laboratorio, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proveedor {
    pub id: String,
    pub nombre: String,
    pub npi: String,
    pub especialidad: String,
    pub grupo_medico: String,
    pub telefono: String,
    pub fax: String,
    pub email: String,
    pub direccion: String,
    pub nivel_engagement: NivelEngagement,
    pub activo: bool,
    #[serde(default)]
    pub notas: String,
    pub creado: NaiveDate,
    pub ultima_interaccion: Option<NaiveDate>,
}

impl Proveedor {
    pub fn nuevo(
        nombre: impl Into<String>,
        npi: impl Into<String>,
        especialidad: impl Into<String>,
        fecha: NaiveDate,
    ) -> Self {
        Proveedor {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            nombre: nombre.into(),
            npi: npi.into(),
            especialidad: especialidad.into(),
            grupo_medico: String::new(),
            telefono: String::new(),
            fax: String::new(),
            email: String::new(),
            direccion: String::new(),
            nivel_engagement: NivelEngagement::Nuevo,
            activo: true,
            notas: String::new(),
            creado: fecha,
            ultima_interaccion: None,
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Interacción / log de llamada
// ══════════════════════════════════════════════════════════════════════════════

/// Registro de una interacción con un proveedor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteraccionProveedor {
    pub id: String,
    pub proveedor_id: String,
    pub tipo: TipoInteraccion,
    pub fecha: NaiveDate,
    pub fecha_hora: NaiveDateTime,
    pub duracion_min: u32,
    pub agente: String,
    pub resultado: ResultadoInteraccion,
    pub proposito: String,
    pub resolucion: String,
    pub seguimiento_requerido: bool,
    pub fecha_seguimiento: Option<NaiveDate>,
    /// ID del caso asociado, si aplica.
    #[serde(default)]
    pub caso_id: String,
    #[serde(default)]
    pub campana_id: String,
    #[serde(default)]
    pub notas: String,
}

impl InteraccionProveedor {
    pub fn nueva(
        proveedor_id: impl Into<String>,
        tipo: TipoInteraccion,
        agente: impl Into<String>,
        proposito: impl Into<String>,
        fecha: NaiveDate,
        fecha_hora: NaiveDateTime,
    ) -> Self {
        InteraccionProveedor {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            proveedor_id: proveedor_id.into(),
            tipo,
            fecha,
            fecha_hora,
            duracion_min: 0,
            agente: agente.into(),
            resultado: ResultadoInteraccion::PendienteRespuesta,
            proposito: proposito.into(),
            resolucion: String::new(),
            seguimiento_requerido: false,
            fecha_seguimiento: None,
            caso_id: String::new(),
            campana_id: String::new(),
            notas: String::new(),
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Seguimiento programado
// ══════════════════════════════════════════════════════════════════════════════

/// Seguimiento futuro programado con un proveedor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeguimientoProveedor {
    pub id: String,
    pub proveedor_id: String,
    pub fecha_programada: NaiveDate,
    pub motivo: String,
    pub agente_asignado: String,
    pub completado: bool,
    pub fecha_completado: Option<NaiveDate>,
    pub resultado: String,
    #[serde(default)]
    pub interaccion_id: String,
}

impl SeguimientoProveedor {
    pub fn nuevo(
        proveedor_id: impl Into<String>,
        fecha: NaiveDate,
        motivo: impl Into<String>,
        agente: impl Into<String>,
    ) -> Self {
        SeguimientoProveedor {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            proveedor_id: proveedor_id.into(),
            fecha_programada: fecha,
            motivo: motivo.into(),
            agente_asignado: agente.into(),
            completado: false,
            fecha_completado: None,
            resultado: String::new(),
            interaccion_id: String::new(),
        }
    }

    pub fn dias_restantes(&self, hoy: NaiveDate) -> i64 {
        (self.fecha_programada - hoy).num_days()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Campaña de outreach
// ══════════════════════════════════════════════════════════════════════════════

/// Métricas de una campaña.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MetricasCampana {
    pub contactados: u32,
    pub completados: u32,
    pub sin_contestacion: u32,
    pub voicemails: u32,
    pub pendientes: u32,
}

impl MetricasCampana {
    pub fn tasa_contacto(&self) -> f64 {
        let total = self.contactados;
        if total == 0 {
            return 0.0;
        }
        self.completados as f64 / total as f64 * 100.0
    }
}

/// Campaña estructurada de outreach a proveedores.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampanaOutreach {
    pub id: String,
    pub nombre: String,
    pub proposito: String,
    pub estado: EstadoCampana,
    pub fecha_inicio: NaiveDate,
    pub fecha_fin: NaiveDate,
    /// IDs de proveedores objetivo.
    pub proveedores_objetivo: Vec<String>,
    pub metricas: MetricasCampana,
    #[serde(default)]
    pub notas: String,
}

impl CampanaOutreach {
    pub fn nueva(
        nombre: impl Into<String>,
        proposito: impl Into<String>,
        inicio: NaiveDate,
        fin: NaiveDate,
    ) -> Self {
        CampanaOutreach {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            nombre: nombre.into(),
            proposito: proposito.into(),
            estado: EstadoCampana::Planificada,
            fecha_inicio: inicio,
            fecha_fin: fin,
            proveedores_objetivo: Vec::new(),
            metricas: MetricasCampana::default(),
            notas: String::new(),
        }
    }

    pub fn proveedores_pendientes<'a>(&'a self, completados: &[String]) -> Vec<&'a String> {
        self.proveedores_objetivo
            .iter()
            .filter(|id| !completados.contains(id))
            .collect()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Métricas globales de outreach
// ══════════════════════════════════════════════════════════════════════════════

/// Métricas globales de outreach y engagement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricasOutreach {
    pub total_proveedores: usize,
    pub proveedores_activos: usize,
    pub total_interacciones: usize,
    pub interacciones_exitosas: usize,
    pub tasa_contacto_pct: f64,
    pub seguimientos_pendientes: usize,
    pub campanas_activas: usize,
}

// ══════════════════════════════════════════════════════════════════════════════
//  Almacén principal
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlmacenProveedores {
    pub proveedores: Vec<Proveedor>,
    pub interacciones: Vec<InteraccionProveedor>,
    pub seguimientos: Vec<SeguimientoProveedor>,
    pub campanas: Vec<CampanaOutreach>,
}

impl AlmacenProveedores {
    // ── CRUD proveedores ──────────────────────────────────────────────────

    pub fn agregar(&mut self, p: Proveedor) {
        self.proveedores.push(p);
    }

    pub fn proveedor_mut(&mut self, id: &str) -> Option<&mut Proveedor> {
        self.proveedores
            .iter_mut()
            .find(|p| p.id == id || p.npi == id)
    }

    pub fn proveedor(&self, id: &str) -> Option<&Proveedor> {
        self.proveedores.iter().find(|p| p.id == id || p.npi == id)
    }

    pub fn buscar_por_nombre(&self, q: &str) -> Vec<&Proveedor> {
        let q = q.to_lowercase();
        self.proveedores
            .iter()
            .filter(|p| {
                p.nombre.to_lowercase().contains(&q)
                    || p.especialidad.to_lowercase().contains(&q)
                    || p.grupo_medico.to_lowercase().contains(&q)
            })
            .collect()
    }

    // ── Interacciones ────────────────────────────────────────────────────

    pub fn registrar_interaccion(&mut self, i: InteraccionProveedor) {
        // Actualizar fecha de última interacción del proveedor
        if let Some(p) = self.proveedores.iter_mut().find(|p| p.id == i.proveedor_id) {
            p.ultima_interaccion = Some(i.fecha);
            if i.resultado.exitoso() {
                p.nivel_engagement = NivelEngagement::Activo;
            }
        }
        self.interacciones.push(i);
    }

    pub fn interacciones_de(&self, proveedor_id: &str) -> Vec<&InteraccionProveedor> {
        self.interacciones
            .iter()
            .filter(|i| i.proveedor_id == proveedor_id)
            .collect()
    }

    // ── Seguimientos ─────────────────────────────────────────────────────

    pub fn agregar_seguimiento(&mut self, s: SeguimientoProveedor) {
        self.seguimientos.push(s);
    }

    pub fn seguimientos_pendientes(&self, hoy: NaiveDate) -> Vec<&SeguimientoProveedor> {
        let mut lista: Vec<&SeguimientoProveedor> = self
            .seguimientos
            .iter()
            .filter(|s| !s.completado && s.fecha_programada >= hoy)
            .collect();
        lista.sort_by_key(|s| s.fecha_programada);
        lista
    }

    pub fn seguimientos_vencidos(&self, hoy: NaiveDate) -> Vec<&SeguimientoProveedor> {
        self.seguimientos
            .iter()
            .filter(|s| !s.completado && s.fecha_programada < hoy)
            .collect()
    }

    // ── Campañas ─────────────────────────────────────────────────────────

    pub fn agregar_campana(&mut self, c: CampanaOutreach) {
        self.campanas.push(c);
    }

    pub fn campana_mut(&mut self, id: &str) -> Option<&mut CampanaOutreach> {
        self.campanas.iter_mut().find(|c| c.id == id)
    }

    pub fn campanas_activas(&self) -> Vec<&CampanaOutreach> {
        self.campanas
            .iter()
            .filter(|c| c.estado == EstadoCampana::Activa)
            .collect()
    }

    // ── Métricas ──────────────────────────────────────────────────────────

    pub fn metricas(&self, hoy: NaiveDate) -> MetricasOutreach {
        let exitosas = self
            .interacciones
            .iter()
            .filter(|i| i.resultado.exitoso())
            .count();
        let tasa = if self.interacciones.is_empty() {
            0.0
        } else {
            exitosas as f64 / self.interacciones.len() as f64 * 100.0
        };
        MetricasOutreach {
            total_proveedores: self.proveedores.len(),
            proveedores_activos: self
                .proveedores
                .iter()
                .filter(|p| p.activo && p.nivel_engagement == NivelEngagement::Activo)
                .count(),
            total_interacciones: self.interacciones.len(),
            interacciones_exitosas: exitosas,
            tasa_contacto_pct: tasa,
            seguimientos_pendientes: self.seguimientos_pendientes(hoy).len(),
            campanas_activas: self.campanas_activas().len(),
        }
    }

    /// Proveedores sin interacción en los últimos `dias` días.
    pub fn sin_contacto_reciente(&self, hoy: NaiveDate, dias: i64) -> Vec<&Proveedor> {
        let umbral = hoy - chrono::Duration::days(dias);
        self.proveedores
            .iter()
            .filter(|p| p.activo && p.ultima_interaccion.map(|d| d < umbral).unwrap_or(true))
            .collect()
    }
}
