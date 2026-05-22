//! Gestión de Solicitudes de Servicio (Intake) — Coordinación y Seguimiento de Casos.
//!
//! Cubre el ciclo completo de una solicitud de servicio:
//!   Recepción → Validación de datos → Checklist → Asignación a equipo →
//!   Seguimiento / Outreach por info faltante → Cierre / Resolución.
//!
//! # Tipos principales
//! - [`Caso`]                   — solicitud completa con datos del cliente, pago y asignación
//! - [`ItemChecklist`]          — ítem del checklist de pre-trabajo
//! - [`SolicitudInfoFaltante`]  — outreach para obtener información pendiente
//! - [`RequisitosCliente`]      — configuración por cliente/cuenta
//! - [`NotaCaso`]               — nota de texto asociada al caso
//! - [`EventoCaso`]             — historial/auditoría del caso
//! - [`AlmacenCasos`]           — almacén persistible en JSON

use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ══════════════════════════════════════════════════════════════════════════════
//  Enums
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EstadoCaso {
    Recibido,
    ValidandoDatos,
    PendienteInformacion,
    ChecklistCompleto,
    EnRevisionClinica,
    Aprobado,
    Negado,
    Cerrado,
    Cancelado,
}

impl EstadoCaso {
    pub fn nombre(&self) -> &str {
        match self {
            EstadoCaso::Recibido => "Recibido",
            EstadoCaso::ValidandoDatos => "Validando Datos",
            EstadoCaso::PendienteInformacion => "Pendiente de Información",
            EstadoCaso::ChecklistCompleto => "Checklist Completo",
            EstadoCaso::EnRevisionClinica => "En Revisión / Proceso",
            EstadoCaso::Aprobado => "Aprobado",
            EstadoCaso::Negado => "Negado",
            EstadoCaso::Cerrado => "Cerrado",
            EstadoCaso::Cancelado => "Cancelado",
        }
    }

    pub fn es_activo(&self) -> bool {
        !matches!(
            self,
            EstadoCaso::Aprobado | EstadoCaso::Negado | EstadoCaso::Cerrado | EstadoCaso::Cancelado
        )
    }

    pub fn permite_ruteo_clinico(&self) -> bool {
        matches!(self, EstadoCaso::ChecklistCompleto)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UrgenciaCaso {
    Rutina,
    Urgente,
    Critico,
}

impl UrgenciaCaso {
    pub fn nombre(&self) -> &str {
        match self {
            UrgenciaCaso::Rutina => "Rutina",
            UrgenciaCaso::Urgente => "Urgente",
            UrgenciaCaso::Critico => "Crítico",
        }
    }

    /// Horas SLA objetivo para cada nivel de urgencia.
    pub fn horas_sla(&self) -> i64 {
        match self {
            UrgenciaCaso::Rutina => 72,
            UrgenciaCaso::Urgente => 24,
            UrgenciaCaso::Critico => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TipoSolicitud {
    AutorizacionPrevia,
    Referido,
    ContinuacionCuidado,
    Emergencia,
    CierreGapCuidado,
    Otro(String),
}

impl TipoSolicitud {
    pub fn nombre(&self) -> &str {
        match self {
            TipoSolicitud::AutorizacionPrevia => "Autorización Previa",
            TipoSolicitud::Referido => "Referido",
            TipoSolicitud::ContinuacionCuidado => "Continuación de Cuidado",
            TipoSolicitud::Emergencia => "Emergencia",
            TipoSolicitud::CierreGapCuidado => "Cierre de Gap de Cuidado",
            TipoSolicitud::Otro(s) => s.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MetodoContacto {
    LlamadaTelefonica,
    Email,
    Fax,
    Portal,
    Correo,
}

impl MetodoContacto {
    pub fn nombre(&self) -> &str {
        match self {
            MetodoContacto::LlamadaTelefonica => "Llamada Telefónica",
            MetodoContacto::Email => "Email",
            MetodoContacto::Fax => "Fax",
            MetodoContacto::Portal => "Portal",
            MetodoContacto::Correo => "Correo",
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Sub-estructuras de datos del caso
// ══════════════════════════════════════════════════════════════════════════════

/// Datos del cliente (persona, empresa u organización solicitante).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DatosPatiente {
    pub nombre: String,
    pub apellido: String,
    pub fecha_nacimiento: String, // YYYY-MM-DD
    pub id_miembro: String,
    pub telefono: String,
    pub direccion: String,
    pub genero: String,
    #[serde(default)]
    pub notas: String,
}

impl DatosPatiente {
    pub fn nombre_completo(&self) -> String {
        format!("{} {}", self.nombre, self.apellido)
    }

    pub fn completo(&self) -> bool {
        !self.nombre.is_empty()
            && !self.apellido.is_empty()
            && !self.fecha_nacimiento.is_empty()
            && !self.id_miembro.is_empty()
    }
}

/// Datos del método de pago o entidad responsable del financiamiento.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DatosSeguro {
    pub aseguradora: String,
    pub plan: String,
    pub numero_poliza: String,
    pub grupo: String,
    pub vigencia_inicio: String,
    pub vigencia_fin: String,
    pub deducible: f64,
    pub copago: f64,
    #[serde(default)]
    pub autorizado_verificado: bool,
}

impl DatosSeguro {
    pub fn completo(&self) -> bool {
        !self.aseguradora.is_empty() && !self.numero_poliza.is_empty() && !self.plan.is_empty()
    }
}

/// Información de asignación: quién solicita, qué área es responsable, y el proceso/servicio a ejecutar.
/// En contexto médico: médico referidor, especialidad destino, ICD-10, CPT.
/// En contexto general: solicitante, área destino, clasificación, proceso.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DatosReferido {
    pub medico_referidor: String,
    pub npi_referidor: String,
    pub especialidad_destino: String,
    pub medico_destino: String,
    pub npi_destino: String,
    /// Código ICD-10 de diagnóstico.
    pub diagnostico_icd10: String,
    pub descripcion_diagnostico: String,
    /// Código CPT del procedimiento solicitado.
    pub procedimiento_cpt: String,
    pub descripcion_procedimiento: String,
    pub notas_clinicas_adjuntas: bool,
}

impl DatosReferido {
    pub fn completo(&self) -> bool {
        !self.medico_referidor.is_empty()
            && !self.diagnostico_icd10.is_empty()
            && !self.especialidad_destino.is_empty()
    }
}

/// Ítem individual del checklist de pre-trabajo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemChecklist {
    pub id: String,
    pub descripcion: String,
    pub requerido: bool,
    pub completado: bool,
    pub fecha_completado: Option<NaiveDate>,
    #[serde(default)]
    pub notas: String,
}

impl ItemChecklist {
    pub fn nuevo(descripcion: impl Into<String>, requerido: bool) -> Self {
        ItemChecklist {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            descripcion: descripcion.into(),
            requerido,
            completado: false,
            fecha_completado: None,
            notas: String::new(),
        }
    }
}

/// Nota de texto asociada al caso (historial de trabajo).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotaCaso {
    pub id: String,
    pub texto: String,
    pub autor: String,
    pub fecha: NaiveDateTime,
}

impl NotaCaso {
    pub fn nueva(texto: impl Into<String>, autor: impl Into<String>, fecha: NaiveDateTime) -> Self {
        NotaCaso {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            texto: texto.into(),
            autor: autor.into(),
            fecha,
        }
    }
}

/// Evento de auditoría del caso.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventoCaso {
    pub tipo: String,
    pub descripcion: String,
    pub usuario: String,
    pub fecha: NaiveDateTime,
}

/// Outreach para obtener información incompleta del caso.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolicitudInfoFaltante {
    pub id: String,
    pub caso_id: String,
    pub campo_faltante: String,
    pub contacto: String,
    pub metodo: MetodoContacto,
    pub fecha_solicitud: NaiveDate,
    pub fecha_limite: NaiveDate,
    pub resuelto: bool,
    pub fecha_resolucion: Option<NaiveDate>,
    #[serde(default)]
    pub notas: String,
}

impl SolicitudInfoFaltante {
    pub fn nueva(
        caso_id: impl Into<String>,
        campo_faltante: impl Into<String>,
        contacto: impl Into<String>,
        metodo: MetodoContacto,
        fecha: NaiveDate,
        limite: NaiveDate,
    ) -> Self {
        SolicitudInfoFaltante {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            caso_id: caso_id.into(),
            campo_faltante: campo_faltante.into(),
            contacto: contacto.into(),
            metodo,
            fecha_solicitud: fecha,
            fecha_limite: limite,
            resuelto: false,
            fecha_resolucion: None,
            notas: String::new(),
        }
    }

    pub fn dias_para_vencer(&self, hoy: NaiveDate) -> i64 {
        (self.fecha_limite - hoy).num_days()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Caso principal
// ══════════════════════════════════════════════════════════════════════════════

/// Solicitud de servicio completa.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Caso {
    pub id: String,
    /// Número de caso legible (ej. "AUT-2026-00142").
    pub numero_caso: String,
    pub tipo: TipoSolicitud,
    pub estado: EstadoCaso,
    pub urgencia: UrgenciaCaso,

    // Datos principales
    pub paciente: DatosPatiente,
    pub seguro: DatosSeguro,
    pub referido: DatosReferido,

    // Documentación
    pub checklist: Vec<ItemChecklist>,
    pub solicitudes_info: Vec<SolicitudInfoFaltante>,
    pub notas: Vec<NotaCaso>,
    pub historial: Vec<EventoCaso>,

    // Routing y SLA
    pub equipo_clinico: String,
    pub asignado_a: String,
    pub id_cliente: String,
    pub fecha_recibida: NaiveDate,
    pub fecha_limite_sla: NaiveDate,
    pub fecha_ruteo_clinico: Option<NaiveDate>,
    pub fecha_cierre: Option<NaiveDate>,
    pub sla_cumplido: Option<bool>,

    pub creado: NaiveDate,
}

impl Caso {
    pub fn nuevo(
        numero_caso: impl Into<String>,
        tipo: TipoSolicitud,
        urgencia: UrgenciaCaso,
        fecha_recibida: NaiveDate,
        id_cliente: impl Into<String>,
    ) -> Self {
        let dias_sla = (urgencia.horas_sla() / 24).max(1);
        let fecha_limite_sla = fecha_recibida + chrono::Duration::days(dias_sla);
        Caso {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            numero_caso: numero_caso.into(),
            tipo,
            estado: EstadoCaso::Recibido,
            urgencia,
            paciente: DatosPatiente::default(),
            seguro: DatosSeguro::default(),
            referido: DatosReferido::default(),
            checklist: Vec::new(),
            solicitudes_info: Vec::new(),
            notas: Vec::new(),
            historial: Vec::new(),
            equipo_clinico: String::new(),
            asignado_a: String::new(),
            id_cliente: id_cliente.into(),
            fecha_recibida,
            fecha_limite_sla,
            fecha_ruteo_clinico: None,
            fecha_cierre: None,
            sla_cumplido: None,
            creado: fecha_recibida,
        }
    }

    /// Días restantes antes del vencimiento del SLA.
    pub fn dias_para_sla(&self, hoy: NaiveDate) -> i64 {
        (self.fecha_limite_sla - hoy).num_days()
    }

    /// Porcentaje del checklist completado.
    pub fn checklist_pct(&self) -> f64 {
        let total = self.checklist.len();
        if total == 0 {
            return 0.0;
        }
        let done = self.checklist.iter().filter(|i| i.completado).count();
        done as f64 / total as f64 * 100.0
    }

    /// Checklist de requeridos completado al 100%.
    pub fn checklist_requeridos_ok(&self) -> bool {
        self.checklist
            .iter()
            .filter(|i| i.requerido)
            .all(|i| i.completado)
    }

    /// Campos de datos faltantes para completar el caso.
    pub fn campos_faltantes(&self) -> Vec<String> {
        let mut faltan = Vec::new();
        if !self.paciente.completo() {
            faltan.push("Datos del paciente incompletos".to_string());
        }
        if !self.seguro.completo() {
            faltan.push("Datos de seguro incompletos".to_string());
        }
        if !self.referido.completo() {
            faltan.push("Datos de referido incompletos".to_string());
        }
        if !self.checklist_requeridos_ok() {
            faltan.push("Checklist de requeridos pendiente".to_string());
        }
        faltan
    }

    /// El caso está listo para rutearse al equipo clínico.
    pub fn listo_para_ruteo(&self) -> bool {
        self.paciente.completo()
            && self.seguro.completo()
            && self.referido.completo()
            && self.checklist_requeridos_ok()
    }

    /// Solicitudes de información pendientes.
    pub fn info_pendiente(&self) -> Vec<&SolicitudInfoFaltante> {
        self.solicitudes_info
            .iter()
            .filter(|s| !s.resuelto)
            .collect()
    }

    /// Registra un evento en el historial de auditoría.
    pub fn registrar_evento(
        &mut self,
        tipo: impl Into<String>,
        descripcion: impl Into<String>,
        usuario: impl Into<String>,
        fecha: NaiveDateTime,
    ) {
        self.historial.push(EventoCaso {
            tipo: tipo.into(),
            descripcion: descripcion.into(),
            usuario: usuario.into(),
            fecha,
        });
    }

    /// Completa un ítem del checklist.
    pub fn completar_checklist_item(&mut self, item_id: &str, hoy: NaiveDate) -> bool {
        if let Some(item) = self.checklist.iter_mut().find(|i| i.id == item_id) {
            item.completado = true;
            item.fecha_completado = Some(hoy);
            return true;
        }
        false
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Requisitos por cliente
// ══════════════════════════════════════════════════════════════════════════════

/// Configuración y requisitos específicos por cuenta/cliente.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequisitosCliente {
    pub id: String,
    pub nombre_cliente: String,
    pub requisitos: Vec<String>,
    pub politicas: Vec<String>,
    pub notas_workflow: String,
    /// Checklist estándar que se aplica a todos los casos de este cliente.
    pub checklist_plantilla: Vec<String>,
    pub sla_horas_rutina: i64,
    pub sla_horas_urgente: i64,
}

impl RequisitosCliente {
    pub fn nuevo(nombre_cliente: impl Into<String>) -> Self {
        RequisitosCliente {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            nombre_cliente: nombre_cliente.into(),
            requisitos: Vec::new(),
            politicas: Vec::new(),
            notas_workflow: String::new(),
            checklist_plantilla: Vec::new(),
            sla_horas_rutina: 72,
            sla_horas_urgente: 24,
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Métricas e indicadores
// ══════════════════════════════════════════════════════════════════════════════

/// Métricas de rendimiento del intake y workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricasIntake {
    pub total_activos: usize,
    pub en_sla: usize,
    pub fuera_sla: usize,
    pub pendientes_info: usize,
    pub en_revision_clinica: usize,
    pub criticos: usize,
    pub urgentes: usize,
    pub completados_hoy: usize,
}

// ══════════════════════════════════════════════════════════════════════════════
//  Almacén principal
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlmacenCasos {
    pub casos: Vec<Caso>,
    pub clientes: Vec<RequisitosCliente>,
    /// Contador secuencial para generar números de caso.
    #[serde(default)]
    pub contador_caso: u64,
}

impl AlmacenCasos {
    // ── Generación de número de caso ──────────────────────────────────────

    pub fn siguiente_numero(&mut self, prefijo: &str) -> String {
        self.contador_caso += 1;
        format!("{}-{:05}", prefijo, self.contador_caso)
    }

    // ── CRUD casos ────────────────────────────────────────────────────────

    pub fn agregar(&mut self, c: Caso) {
        self.casos.push(c);
    }

    pub fn caso_mut(&mut self, id: &str) -> Option<&mut Caso> {
        self.casos
            .iter_mut()
            .find(|c| c.id == id || c.numero_caso == id)
    }

    pub fn caso(&self, id: &str) -> Option<&Caso> {
        self.casos
            .iter()
            .find(|c| c.id == id || c.numero_caso == id)
    }

    pub fn eliminar(&mut self, id: &str) -> bool {
        let antes = self.casos.len();
        self.casos.retain(|c| c.id != id && c.numero_caso != id);
        self.casos.len() < antes
    }

    // ── Cola de trabajo / Queue ───────────────────────────────────────────

    /// Casos activos ordenados por urgencia y fecha SLA (más urgente primero).
    pub fn cola_trabajo(&self, hoy: NaiveDate) -> Vec<&Caso> {
        let mut lista: Vec<&Caso> = self.casos.iter().filter(|c| c.estado.es_activo()).collect();
        lista.sort_by(|a, b| {
            // Crítico < Urgente < Rutina, luego por días SLA
            let ord_urg = |c: &Caso| match c.urgencia {
                UrgenciaCaso::Critico => 0,
                UrgenciaCaso::Urgente => 1,
                UrgenciaCaso::Rutina => 2,
            };
            ord_urg(a)
                .cmp(&ord_urg(b))
                .then(a.dias_para_sla(hoy).cmp(&b.dias_para_sla(hoy)))
        });
        lista
    }

    /// Casos con SLA vencido o que vencen en los próximos `horas` horas.
    pub fn casos_criticos_sla(&self, hoy: NaiveDate, dias: i64) -> Vec<&Caso> {
        self.casos
            .iter()
            .filter(|c| c.estado.es_activo() && c.dias_para_sla(hoy) <= dias)
            .collect()
    }

    /// Casos con información pendiente que requieren outreach.
    pub fn requieren_outreach(&self) -> Vec<&Caso> {
        self.casos
            .iter()
            .filter(|c| {
                c.estado.es_activo()
                    && (c.estado == EstadoCaso::PendienteInformacion
                        || !c.info_pendiente().is_empty())
            })
            .collect()
    }

    /// Casos listos para rutearse al equipo clínico.
    pub fn listos_para_ruteo(&self) -> Vec<&Caso> {
        self.casos
            .iter()
            .filter(|c| {
                c.estado.es_activo()
                    && c.estado != EstadoCaso::EnRevisionClinica
                    && c.listo_para_ruteo()
            })
            .collect()
    }

    /// Métricas actuales del intake.
    pub fn metricas(&self, hoy: NaiveDate) -> MetricasIntake {
        let activos: Vec<&Caso> = self.casos.iter().filter(|c| c.estado.es_activo()).collect();
        let en_sla = activos.iter().filter(|c| c.dias_para_sla(hoy) >= 0).count();
        MetricasIntake {
            total_activos: activos.len(),
            en_sla,
            fuera_sla: activos.len().saturating_sub(en_sla),
            pendientes_info: activos
                .iter()
                .filter(|c| c.estado == EstadoCaso::PendienteInformacion)
                .count(),
            en_revision_clinica: activos
                .iter()
                .filter(|c| c.estado == EstadoCaso::EnRevisionClinica)
                .count(),
            criticos: activos
                .iter()
                .filter(|c| c.urgencia == UrgenciaCaso::Critico)
                .count(),
            urgentes: activos
                .iter()
                .filter(|c| c.urgencia == UrgenciaCaso::Urgente)
                .count(),
            completados_hoy: self
                .casos
                .iter()
                .filter(|c| c.fecha_cierre == Some(hoy))
                .count(),
        }
    }

    // ── CRUD clientes ─────────────────────────────────────────────────────

    pub fn agregar_cliente(&mut self, c: RequisitosCliente) {
        self.clientes.push(c);
    }

    pub fn cliente_mut(&mut self, id: &str) -> Option<&mut RequisitosCliente> {
        self.clientes
            .iter_mut()
            .find(|c| c.id == id || c.nombre_cliente == id)
    }

    pub fn cliente(&self, id: &str) -> Option<&RequisitosCliente> {
        self.clientes
            .iter()
            .find(|c| c.id == id || c.nombre_cliente == id)
    }
}
