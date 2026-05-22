//! Gestión de Propuestas de Planes de Salud — herramienta de trabajo diario.
//!
//! Cubre el ciclo completo de una propuesta:
//!   Solicitud (Salesforce) → Kick-off → Desarrollo por secciones →
//!   Revisiones / SMEs → Proofreading → Entrega al vendedor.
//!
//! # Tipos principales
//! - [`Propuesta`]          — propuesta completa con secciones, timeline y estado
//! - [`SeccionPropuesta`]   — pricing, stop-loss, red, gestión de cuidado, etc.
//! - [`HitoTimeline`]       — jalón de la línea de tiempo
//! - [`ContactoSME`]        — experto en la materia
//! - [`SolicitudSME`]       — pregunta enviada a un SME
//! - [`ReunionProyecto`]    — reunión con acta y puntos de acción
//! - [`SolicitudRevision`]  — solicitud de revisión de borrador
//! - [`EscalacionProblema`] — problema escalado hacia arriba
//! - [`RegistroSalesforce`] — entrada de log al CRM
//! - [`AlmacenPropuestas`]  — almacén completo, persistible en JSON

use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ══════════════════════════════════════════════════════════════════════════════
//  Enums de estado
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EstadoPropuesta {
    Recibida,
    KickOffPendiente,
    EnDesarrollo,
    EnRevisionInterna,
    EnRevisionVendedor,
    Proofreading,
    ListaParaEnvio,
    Enviada,
    Ganada,
    Perdida,
    Cancelada,
}

impl EstadoPropuesta {
    pub fn nombre(&self) -> &str {
        match self {
            EstadoPropuesta::Recibida => "Recibida",
            EstadoPropuesta::KickOffPendiente => "Kick-Off Pendiente",
            EstadoPropuesta::EnDesarrollo => "En Desarrollo",
            EstadoPropuesta::EnRevisionInterna => "En Revisión Interna",
            EstadoPropuesta::EnRevisionVendedor => "En Revisión del Vendedor",
            EstadoPropuesta::Proofreading => "Proofreading / Edición",
            EstadoPropuesta::ListaParaEnvio => "Lista para Envío",
            EstadoPropuesta::Enviada => "Enviada",
            EstadoPropuesta::Ganada => "Ganada",
            EstadoPropuesta::Perdida => "Perdida",
            EstadoPropuesta::Cancelada => "Cancelada",
        }
    }

    pub fn es_activa(&self) -> bool {
        !matches!(
            self,
            EstadoPropuesta::Ganada | EstadoPropuesta::Perdida | EstadoPropuesta::Cancelada
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TipoSeccion {
    Pricing,
    StopLoss,
    Red,                 // Network
    GestionCuidado,      // Care Management
    EngagementPoblacion, // Population Engagement
    EstrategiaVentas,
    ResumenEjecutivo,
    Administrativa,
    Otra(String),
}

impl TipoSeccion {
    pub fn nombre(&self) -> &str {
        match self {
            TipoSeccion::Pricing => "Pricing",
            TipoSeccion::StopLoss => "Stop Loss",
            TipoSeccion::Red => "Red / Network",
            TipoSeccion::GestionCuidado => "Gestión de Cuidado",
            TipoSeccion::EngagementPoblacion => "Engagement de Población",
            TipoSeccion::EstrategiaVentas => "Estrategia de Ventas",
            TipoSeccion::ResumenEjecutivo => "Resumen Ejecutivo",
            TipoSeccion::Administrativa => "Administrativa",
            TipoSeccion::Otra(n) => n.as_str(),
        }
    }

    pub fn enfoque_externo(&self) -> bool {
        matches!(
            self,
            TipoSeccion::ResumenEjecutivo | TipoSeccion::EngagementPoblacion | TipoSeccion::Red
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EstadoSeccion {
    Pendiente,
    EnProceso,
    Borrador,
    EnRevision,
    Aprobada,
    Entregada,
}

impl EstadoSeccion {
    pub fn nombre(&self) -> &str {
        match self {
            EstadoSeccion::Pendiente => "Pendiente",
            EstadoSeccion::EnProceso => "En Proceso",
            EstadoSeccion::Borrador => "Borrador",
            EstadoSeccion::EnRevision => "En Revisión",
            EstadoSeccion::Aprobada => "Aprobada",
            EstadoSeccion::Entregada => "Entregada",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EstadoSolicitudSME {
    Pendiente,
    Enviada,
    EnProceso,
    Respondida,
    Cancelada,
}

impl EstadoSolicitudSME {
    pub fn nombre(&self) -> &str {
        match self {
            EstadoSolicitudSME::Pendiente => "Pendiente",
            EstadoSolicitudSME::Enviada => "Enviada al SME",
            EstadoSolicitudSME::EnProceso => "En Proceso",
            EstadoSolicitudSME::Respondida => "Respondida",
            EstadoSolicitudSME::Cancelada => "Cancelada",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TipoReunion {
    KickOff,
    Estrategia,
    Estatus,
    RevisionBorrador,
    Cierre,
    Escalacion,
    Otra(String),
}

impl TipoReunion {
    pub fn nombre(&self) -> &str {
        match self {
            TipoReunion::KickOff => "Kick-Off",
            TipoReunion::Estrategia => "Estrategia",
            TipoReunion::Estatus => "Estatus",
            TipoReunion::RevisionBorrador => "Revisión de Borrador",
            TipoReunion::Cierre => "Cierre",
            TipoReunion::Escalacion => "Escalación",
            TipoReunion::Otra(n) => n.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NivelEscalacion {
    Supervisor,
    Gerente,
    Director,
    VP,
}

impl NivelEscalacion {
    pub fn nombre(&self) -> &str {
        match self {
            NivelEscalacion::Supervisor => "Supervisor",
            NivelEscalacion::Gerente => "Gerente",
            NivelEscalacion::Director => "Director",
            NivelEscalacion::VP => "VP",
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Estructuras principales
// ══════════════════════════════════════════════════════════════════════════════

/// Jalón en la línea de tiempo de la propuesta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitoTimeline {
    pub id: String,
    pub descripcion: String,
    pub fecha_objetivo: NaiveDate,
    pub fecha_real: Option<NaiveDate>,
    pub completado: bool,
    #[serde(default)]
    pub notas: String,
}

impl HitoTimeline {
    pub fn nuevo(descripcion: impl Into<String>, fecha: NaiveDate) -> Self {
        HitoTimeline {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            descripcion: descripcion.into(),
            fecha_objetivo: fecha,
            fecha_real: None,
            completado: false,
            notas: String::new(),
        }
    }

    pub fn dias_restantes(&self, hoy: NaiveDate) -> i64 {
        (self.fecha_objetivo - hoy).num_days()
    }
}

/// Sección de la propuesta (pricing, stop-loss, red, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeccionPropuesta {
    pub id: String,
    pub tipo: TipoSeccion,
    pub descripcion: String,
    pub responsable: String,
    pub estado: EstadoSeccion,
    pub estrategia_ventas_presente: bool,
    pub estrategia_ventas_consistente: bool,
    #[serde(default)]
    pub notas: String,
    #[serde(default)]
    pub referencias: Vec<String>,
    #[serde(default)]
    pub analisis_solicitados: Vec<String>,
}

impl SeccionPropuesta {
    pub fn nueva(
        tipo: TipoSeccion,
        descripcion: impl Into<String>,
        responsable: impl Into<String>,
    ) -> Self {
        SeccionPropuesta {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            tipo,
            descripcion: descripcion.into(),
            responsable: responsable.into(),
            estado: EstadoSeccion::Pendiente,
            estrategia_ventas_presente: false,
            estrategia_ventas_consistente: false,
            notas: String::new(),
            referencias: Vec::new(),
            analisis_solicitados: Vec::new(),
        }
    }
}

/// Punto de acción de una reunión.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PuntoAccion {
    pub id: String,
    pub descripcion: String,
    pub responsable: String,
    pub fecha_limite: NaiveDate,
    pub completado: bool,
}

impl PuntoAccion {
    pub fn nuevo(
        descripcion: impl Into<String>,
        responsable: impl Into<String>,
        fecha_limite: NaiveDate,
    ) -> Self {
        PuntoAccion {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            descripcion: descripcion.into(),
            responsable: responsable.into(),
            fecha_limite,
            completado: false,
        }
    }
}

/// Reunión de proyecto (kick-off, estrategia, estatus, etc.) con acta y puntos de acción.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReunionProyecto {
    pub id: String,
    pub propuesta_id: String,
    pub tipo: TipoReunion,
    pub titulo: String,
    pub fecha: NaiveDate,
    pub participantes: Vec<String>,
    pub agenda: Vec<String>,
    pub acta_resumen: String,
    pub puntos_accion: Vec<PuntoAccion>,
    pub recap_enviado: bool,
    pub creado: NaiveDate,
}

impl ReunionProyecto {
    pub fn nueva(
        propuesta_id: impl Into<String>,
        tipo: TipoReunion,
        titulo: impl Into<String>,
        fecha: NaiveDate,
    ) -> Self {
        ReunionProyecto {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            propuesta_id: propuesta_id.into(),
            tipo,
            titulo: titulo.into(),
            fecha,
            participantes: Vec::new(),
            agenda: Vec::new(),
            acta_resumen: String::new(),
            puntos_accion: Vec::new(),
            recap_enviado: false,
            creado: fecha,
        }
    }

    /// Puntos de acción pendientes.
    pub fn acciones_pendientes(&self) -> Vec<&PuntoAccion> {
        self.puntos_accion
            .iter()
            .filter(|p| !p.completado)
            .collect()
    }
}

/// Experto en la materia (Subject Matter Expert).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactoSME {
    pub id: String,
    pub nombre: String,
    pub area_especialidad: String,
    pub email: String,
    pub telefono: String,
    pub empresa: String,
    #[serde(default)]
    pub notas: String,
    #[serde(default)]
    pub activo: bool,
}

impl ContactoSME {
    pub fn nuevo(
        nombre: impl Into<String>,
        area: impl Into<String>,
        email: impl Into<String>,
    ) -> Self {
        ContactoSME {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            nombre: nombre.into(),
            area_especialidad: area.into(),
            email: email.into(),
            telefono: String::new(),
            empresa: String::new(),
            notas: String::new(),
            activo: true,
        }
    }
}

/// Solicitud de información/respuesta a un SME para una propuesta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolicitudSME {
    pub id: String,
    pub propuesta_id: String,
    pub sme_id: String,
    pub pregunta: String,
    pub contexto: String,
    pub estado: EstadoSolicitudSME,
    pub fecha_solicitud: NaiveDate,
    pub fecha_limite: NaiveDate,
    pub respuesta: String,
    pub fecha_respuesta: Option<NaiveDate>,
    #[serde(default)]
    pub seccion_destino: String,
}

impl SolicitudSME {
    pub fn nueva(
        propuesta_id: impl Into<String>,
        sme_id: impl Into<String>,
        pregunta: impl Into<String>,
        fecha_solicitud: NaiveDate,
        fecha_limite: NaiveDate,
    ) -> Self {
        SolicitudSME {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            propuesta_id: propuesta_id.into(),
            sme_id: sme_id.into(),
            pregunta: pregunta.into(),
            contexto: String::new(),
            estado: EstadoSolicitudSME::Pendiente,
            fecha_solicitud,
            fecha_limite,
            respuesta: String::new(),
            fecha_respuesta: None,
            seccion_destino: String::new(),
        }
    }

    pub fn dias_para_vencer(&self, hoy: NaiveDate) -> i64 {
        (self.fecha_limite - hoy).num_days()
    }
}

/// Solicitud de revisión de borrador (al vendedor o editor estratégico).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TipoRevisor {
    Vendedor,
    EditorEstrategico,
    Interno,
    Otro(String),
}

impl TipoRevisor {
    pub fn nombre(&self) -> &str {
        match self {
            TipoRevisor::Vendedor => "Vendedor",
            TipoRevisor::EditorEstrategico => "Editor Estratégico",
            TipoRevisor::Interno => "Revisión Interna",
            TipoRevisor::Otro(n) => n.as_str(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolicitudRevision {
    pub id: String,
    pub propuesta_id: String,
    pub tipo_revisor: TipoRevisor,
    pub nombre_revisor: String,
    pub seccion_ids: Vec<String>,
    pub fecha_solicitud: NaiveDate,
    pub fecha_limite: NaiveDate,
    pub comentarios: String,
    pub completada: bool,
    pub fecha_completada: Option<NaiveDate>,
}

impl SolicitudRevision {
    pub fn nueva(
        propuesta_id: impl Into<String>,
        tipo_revisor: TipoRevisor,
        nombre_revisor: impl Into<String>,
        fecha_solicitud: NaiveDate,
        fecha_limite: NaiveDate,
    ) -> Self {
        SolicitudRevision {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            propuesta_id: propuesta_id.into(),
            tipo_revisor,
            nombre_revisor: nombre_revisor.into(),
            seccion_ids: Vec::new(),
            fecha_solicitud,
            fecha_limite,
            comentarios: String::new(),
            completada: false,
            fecha_completada: None,
        }
    }
}

/// Problema escalado hacia la cadena de mando.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalacionProblema {
    pub id: String,
    pub propuesta_id: String,
    pub descripcion: String,
    pub nivel: NivelEscalacion,
    pub escalado_a: String,
    pub fecha: NaiveDate,
    pub resolucion: String,
    pub resuelto: bool,
    pub fecha_resolucion: Option<NaiveDate>,
}

impl EscalacionProblema {
    pub fn nueva(
        propuesta_id: impl Into<String>,
        descripcion: impl Into<String>,
        nivel: NivelEscalacion,
        escalado_a: impl Into<String>,
        fecha: NaiveDate,
    ) -> Self {
        EscalacionProblema {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            propuesta_id: propuesta_id.into(),
            descripcion: descripcion.into(),
            nivel,
            escalado_a: escalado_a.into(),
            fecha,
            resolucion: String::new(),
            resuelto: false,
            fecha_resolucion: None,
        }
    }
}

/// Entrada en el log de Salesforce (CRM interno).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistroSalesforce {
    pub id: String,
    pub propuesta_id: String,
    pub oportunidad_sf: String,
    pub accion: String,
    pub fecha: NaiveDate,
    pub fecha_hora: NaiveDateTime,
    pub usuario: String,
    #[serde(default)]
    pub notas: String,
}

// ══════════════════════════════════════════════════════════════════════════════
//  Propuesta principal
// ══════════════════════════════════════════════════════════════════════════════

/// Verificación de que la estrategia de ventas está presente y consistente
/// en toda la propuesta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificacionEstrategia {
    pub estrategia_definida: bool,
    pub comunicada_al_equipo: bool,
    /// Número de secciones que deben tener estrategia vs cuántas la tienen
    pub secciones_con_estrategia: u32,
    pub secciones_total: u32,
    pub porcentaje_consistencia: f64,
    pub observaciones: String,
}

impl VerificacionEstrategia {
    pub fn calcular(secciones: &[SeccionPropuesta]) -> Self {
        let total = secciones.len() as u32;
        let con_estrategia = secciones
            .iter()
            .filter(|s| s.estrategia_ventas_presente)
            .count() as u32;
        let consistente = secciones
            .iter()
            .filter(|s| s.estrategia_ventas_consistente)
            .count() as u32;
        let pct = if total == 0 {
            100.0
        } else {
            consistente as f64 / total as f64 * 100.0
        };
        VerificacionEstrategia {
            estrategia_definida: con_estrategia > 0,
            comunicada_al_equipo: false,
            secciones_con_estrategia: con_estrategia,
            secciones_total: total,
            porcentaje_consistencia: pct,
            observaciones: String::new(),
        }
    }
}

/// Propuesta completa.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Propuesta {
    pub id: String,
    pub nombre: String,
    pub cliente: String,
    pub vendedor: String,
    pub estado: EstadoPropuesta,
    pub fecha_recibida: NaiveDate,
    pub fecha_vencimiento: NaiveDate,
    pub fecha_entregada: Option<NaiveDate>,
    pub id_salesforce: String,
    pub secciones: Vec<SeccionPropuesta>,
    pub timeline: Vec<HitoTimeline>,
    pub reuniones: Vec<ReunionProyecto>,
    pub solicitudes_sme: Vec<SolicitudSME>,
    pub revisiones: Vec<SolicitudRevision>,
    pub escalaciones: Vec<EscalacionProblema>,
    pub log_salesforce: Vec<RegistroSalesforce>,
    pub estrategia_ventas: String,
    #[serde(default)]
    pub notas: String,
    pub creado: NaiveDate,
}

impl Propuesta {
    pub fn nueva(
        nombre: impl Into<String>,
        cliente: impl Into<String>,
        vendedor: impl Into<String>,
        fecha_recibida: NaiveDate,
        fecha_vencimiento: NaiveDate,
    ) -> Self {
        Propuesta {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            nombre: nombre.into(),
            cliente: cliente.into(),
            vendedor: vendedor.into(),
            estado: EstadoPropuesta::Recibida,
            fecha_recibida,
            fecha_vencimiento,
            fecha_entregada: None,
            id_salesforce: String::new(),
            secciones: Vec::new(),
            timeline: Vec::new(),
            reuniones: Vec::new(),
            solicitudes_sme: Vec::new(),
            revisiones: Vec::new(),
            escalaciones: Vec::new(),
            log_salesforce: Vec::new(),
            estrategia_ventas: String::new(),
            notas: String::new(),
            creado: fecha_recibida,
        }
    }

    /// Días restantes para el vencimiento.
    pub fn dias_restantes(&self, hoy: NaiveDate) -> i64 {
        (self.fecha_vencimiento - hoy).num_days()
    }

    /// Verifica consistencia de estrategia de ventas en todas las secciones.
    pub fn verificar_estrategia(&self) -> VerificacionEstrategia {
        VerificacionEstrategia::calcular(&self.secciones)
    }

    /// Progreso general (% de secciones aprobadas o entregadas).
    pub fn progreso_pct(&self) -> f64 {
        if self.secciones.is_empty() {
            return 0.0;
        }
        let completadas = self
            .secciones
            .iter()
            .filter(|s| s.estado == EstadoSeccion::Aprobada || s.estado == EstadoSeccion::Entregada)
            .count();
        completadas as f64 / self.secciones.len() as f64 * 100.0
    }

    /// SME requests abiertas (sin responder).
    pub fn sme_pendientes(&self) -> Vec<&SolicitudSME> {
        self.solicitudes_sme
            .iter()
            .filter(|s| {
                s.estado == EstadoSolicitudSME::Pendiente
                    || s.estado == EstadoSolicitudSME::Enviada
                    || s.estado == EstadoSolicitudSME::EnProceso
            })
            .collect()
    }

    /// Revisiones de borrador pendientes.
    pub fn revisiones_pendientes(&self) -> Vec<&SolicitudRevision> {
        self.revisiones.iter().filter(|r| !r.completada).collect()
    }

    /// Escalaciones sin resolver.
    pub fn escalaciones_abiertas(&self) -> Vec<&EscalacionProblema> {
        self.escalaciones.iter().filter(|e| !e.resuelto).collect()
    }

    /// Puntos de acción de todas las reuniones que siguen pendientes.
    pub fn acciones_pendientes_total(&self) -> Vec<(&ReunionProyecto, &PuntoAccion)> {
        self.reuniones
            .iter()
            .flat_map(|r| r.acciones_pendientes().into_iter().map(move |a| (r, a)))
            .collect()
    }

    /// Agrega una sección estándar de plan de salud integrado.
    pub fn agregar_seccion(&mut self, s: SeccionPropuesta) {
        self.secciones.push(s);
    }

    /// Agrega hito al timeline.
    pub fn agregar_hito(&mut self, h: HitoTimeline) {
        self.timeline.push(h);
    }

    /// Agrega una reunión.
    pub fn agregar_reunion(&mut self, r: ReunionProyecto) {
        self.reuniones.push(r);
    }

    /// Registra una solicitud SME.
    pub fn agregar_solicitud_sme(&mut self, s: SolicitudSME) {
        self.solicitudes_sme.push(s);
    }

    /// Registra una solicitud de revisión.
    pub fn agregar_revision(&mut self, r: SolicitudRevision) {
        self.revisiones.push(r);
    }

    /// Registra una escalación.
    pub fn agregar_escalacion(&mut self, e: EscalacionProblema) {
        self.escalaciones.push(e);
    }

    /// Agrega entrada al log Salesforce.
    pub fn registrar_salesforce(&mut self, entrada: RegistroSalesforce) {
        self.log_salesforce.push(entrada);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Almacén principal
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlmacenPropuestas {
    pub propuestas: Vec<Propuesta>,
    pub smes: Vec<ContactoSME>,
}

impl AlmacenPropuestas {
    // ── CRUD propuestas ────────────────────────────────────────────────────

    pub fn agregar(&mut self, p: Propuesta) {
        self.propuestas.push(p);
    }

    pub fn propuesta_mut(&mut self, id: &str) -> Option<&mut Propuesta> {
        self.propuestas.iter_mut().find(|p| p.id == id)
    }

    pub fn propuesta(&self, id: &str) -> Option<&Propuesta> {
        self.propuestas.iter().find(|p| p.id == id)
    }

    pub fn eliminar(&mut self, id: &str) -> bool {
        let antes = self.propuestas.len();
        self.propuestas.retain(|p| p.id != id);
        self.propuestas.len() < antes
    }

    // ── Consultas ─────────────────────────────────────────────────────────

    /// Propuestas activas ordenadas por fecha de vencimiento (más urgente primero).
    pub fn activas_por_urgencia(&self) -> Vec<&Propuesta> {
        let mut lista: Vec<&Propuesta> = self
            .propuestas
            .iter()
            .filter(|p| p.estado.es_activa())
            .collect();
        lista.sort_by_key(|p| p.fecha_vencimiento);
        lista
    }

    /// Propuestas que vencen en los próximos `dias` días.
    pub fn vencen_pronto(&self, hoy: NaiveDate, dias: i64) -> Vec<&Propuesta> {
        self.propuestas
            .iter()
            .filter(|p| {
                p.estado.es_activa() && {
                    let d = p.dias_restantes(hoy);
                    d >= 0 && d <= dias
                }
            })
            .collect()
    }

    /// Dashboard resumen de todas las propuestas activas.
    pub fn dashboard(&self, hoy: NaiveDate) -> DashboardPropuestas {
        let activas: Vec<&Propuesta> = self
            .propuestas
            .iter()
            .filter(|p| p.estado.es_activa())
            .collect();
        let vencen_7 = self.vencen_pronto(hoy, 7).len();
        let sme_abiertas: usize = activas.iter().map(|p| p.sme_pendientes().len()).sum();
        let revisiones_pend: usize = activas
            .iter()
            .map(|p| p.revisiones_pendientes().len())
            .sum();
        let escalaciones_abiertas: usize = activas
            .iter()
            .map(|p| p.escalaciones_abiertas().len())
            .sum();
        let acciones_pend: usize = activas
            .iter()
            .map(|p| p.acciones_pendientes_total().len())
            .sum();

        DashboardPropuestas {
            total_activas: activas.len(),
            vencen_en_7_dias: vencen_7,
            sme_pendientes: sme_abiertas,
            revisiones_pendientes: revisiones_pend,
            escalaciones_abiertas,
            acciones_pendientes: acciones_pend,
            total_smes_registrados: self.smes.len(),
        }
    }

    // ── CRUD SMEs ─────────────────────────────────────────────────────────

    pub fn agregar_sme(&mut self, sme: ContactoSME) {
        self.smes.push(sme);
    }

    pub fn sme_mut(&mut self, id: &str) -> Option<&mut ContactoSME> {
        self.smes.iter_mut().find(|s| s.id == id)
    }

    pub fn smes_por_area(&self, area: &str) -> Vec<&ContactoSME> {
        self.smes
            .iter()
            .filter(|s| {
                s.activo
                    && s.area_especialidad
                        .to_lowercase()
                        .contains(&area.to_lowercase())
            })
            .collect()
    }
}

/// Resumen ejecutivo del estado de todas las propuestas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardPropuestas {
    pub total_activas: usize,
    pub vencen_en_7_dias: usize,
    pub sme_pendientes: usize,
    pub revisiones_pendientes: usize,
    pub escalaciones_abiertas: usize,
    pub acciones_pendientes: usize,
    pub total_smes_registrados: usize,
}
