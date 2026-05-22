// ═══════════════════════════════════════════════════════════════════════
//  Módulo: Obras y Ciclo Financiero
//
//  Flujo de información obligatorio:
//  RFI → Contacto cliente → Correo de requerimientos → Contrato
//  → Posición contable (disponible/exigible/realizable)
//  → Consulta previa al cliente ANTES de gastar cualquier peso
//  → 1er desembolso (solo materiales)
//  → Ejecución con reportes de avance compartidos al cliente
//  → 2do desembolso (80% - cubre todo lo operativo + mano de obra)
//  → Entrega + Pago final (20% - SOLO impuestos)
//
//  Principio: La empresa NUNCA pierde porque NUNCA gasta sin la
//  aprobación documentada del cliente en cada paso.
// ═══════════════════════════════════════════════════════════════════════

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

fn nuevo_id() -> String {
    Uuid::new_v4().to_string()[..8].to_string()
}

// ─────────────────────────────────────────────────────────────────────
//  Estado de la Obra
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum EstadoObra {
    #[default]
    RFI,
    ContactoCliente,
    CorreoRequerimientos,
    ContratoPendiente,
    ContratoFirmado,
    PrimerDesembolsoPendiente,
    PrimerDesembolsoRecibido,
    EnEjecucion,
    SegundoDesembolsoPendiente,
    SegundoDesembolsoRecibido,
    EntregaPendiente,
    Entregada,
    Completada,
    /// Cliente se echó para atrás — empresa protegida por las consultas previas
    SuspendidaCliente,
    Cancelada,
}

impl EstadoObra {
    pub fn nombre(&self) -> &str {
        match self {
            Self::RFI => "RFI recibido",
            Self::ContactoCliente => "Contacto con cliente",
            Self::CorreoRequerimientos => "Correo de requerimientos enviado",
            Self::ContratoPendiente => "Contrato pendiente de firma",
            Self::ContratoFirmado => "Contrato firmado",
            Self::PrimerDesembolsoPendiente => "Esperando 1er desembolso",
            Self::PrimerDesembolsoRecibido => "1er desembolso recibido — comprando materiales",
            Self::EnEjecucion => "En ejecución",
            Self::SegundoDesembolsoPendiente => "Esperando 2do desembolso (80%)",
            Self::SegundoDesembolsoRecibido => "2do desembolso recibido",
            Self::EntregaPendiente => "Entrega pendiente — esperando pago final (20%)",
            Self::Entregada => "Entregada al cliente",
            Self::Completada => "Completada ✓",
            Self::SuspendidaCliente => "Suspendida por el cliente",
            Self::Cancelada => "Cancelada",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
//  RFI — Request for Information
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RFI {
    pub id: String,
    pub fecha: NaiveDate,
    pub canal: String,
    pub descripcion: String,
    pub necesidades: Vec<String>,
    pub urgencia: String,
}

impl RFI {
    pub fn nuevo(fecha: NaiveDate, canal: String, descripcion: String) -> Self {
        Self {
            id: nuevo_id(),
            fecha,
            canal,
            descripcion,
            necesidades: Vec::new(),
            urgencia: String::new(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
//  Contacto con el Cliente (llamada / reunión)
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactoCliente {
    pub id: String,
    pub fecha: NaiveDate,
    pub tipo: String,
    pub resumen: String,
    pub acuerdos: Vec<String>,
    pub proxima_accion: String,
    pub registrado_por: String,
}

impl ContactoCliente {
    pub fn nuevo(fecha: NaiveDate, tipo: String, resumen: String, registrado_por: String) -> Self {
        Self {
            id: nuevo_id(),
            fecha,
            tipo,
            resumen,
            acuerdos: Vec::new(),
            proxima_accion: String::new(),
            registrado_por,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
//  Correo de Requerimientos
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CorreoRequerimiento {
    pub id: String,
    pub fecha: NaiveDate,
    pub asunto: String,
    pub requerimientos: Vec<String>,
    pub plazo_respuesta: Option<NaiveDate>,
    pub respondido: bool,
    pub respuesta_cliente: String,
    pub fecha_respuesta: Option<NaiveDate>,
}

impl CorreoRequerimiento {
    pub fn nuevo(fecha: NaiveDate, asunto: String) -> Self {
        Self {
            id: nuevo_id(),
            fecha,
            asunto,
            ..Default::default()
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
//  Contrato
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Contrato {
    pub numero: String,
    pub fecha_firma: Option<NaiveDate>,
    pub valor_total: f64,
    pub condiciones: Vec<String>,
    pub plazos: Vec<PlazoContrato>,
    pub penalidades: Vec<String>,
    pub firmado: bool,
    pub notas: String,
    /// Porcentaje del valor total destinado al 1er desembolso (materiales)
    pub pct_primer_desembolso: f64,
    /// 2do desembolso = 80% del total → cubre TODO lo operativo y mano de obra
    pub pct_segundo_desembolso: f64,
    /// Pago final = 20% del total → ÚNICAMENTE para impuestos
    pub pct_pago_final: f64,
}

impl Contrato {
    pub fn con_estructura_estandar(valor_total: f64, pct_primer: f64) -> Self {
        Self {
            valor_total,
            pct_primer_desembolso: pct_primer,
            pct_segundo_desembolso: 80.0,
            pct_pago_final: 20.0,
            ..Default::default()
        }
    }

    pub fn monto_primer(&self) -> f64 {
        self.valor_total * self.pct_primer_desembolso / 100.0
    }

    pub fn monto_segundo(&self) -> f64 {
        self.valor_total * self.pct_segundo_desembolso / 100.0
    }

    pub fn monto_final(&self) -> f64 {
        self.valor_total * self.pct_pago_final / 100.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlazoContrato {
    pub id: String,
    pub descripcion: String,
    pub fecha_limite: NaiveDate,
    pub cumplido: bool,
    pub fecha_cumplimiento: Option<NaiveDate>,
}

impl PlazoContrato {
    pub fn nuevo(descripcion: String, fecha_limite: NaiveDate) -> Self {
        Self {
            id: nuevo_id(),
            descripcion,
            fecha_limite,
            cumplido: false,
            fecha_cumplimiento: None,
        }
    }

    pub fn dias_restantes(&self, hoy: NaiveDate) -> i64 {
        (self.fecha_limite - hoy).num_days()
    }
}

// ─────────────────────────────────────────────────────────────────────
//  Posición Contable  (Disponible / Exigible / Realizable)
// ─────────────────────────────────────────────────────────────────────

/// Clasificación contable estándar del activo corriente de la obra.
/// - Disponible  : efectivo en caja/banco asignado a la obra
/// - Exigible    : lo que el cliente nos debe (cuentas por cobrar)
/// - Realizable  : materiales comprados / inventario / activos vendibles
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PosicionContable {
    pub disponible: f64,
    pub exigible: f64,
    pub realizable: f64,
    pub total_activo_corriente: f64,
    pub fecha_calculo: Option<NaiveDate>,
    pub notas: String,
}

impl PosicionContable {
    pub fn recalcular(&mut self, hoy: NaiveDate) {
        self.total_activo_corriente = self.disponible + self.exigible + self.realizable;
        self.fecha_calculo = Some(hoy);
    }
}

// ─────────────────────────────────────────────────────────────────────
//  Hitos del Proyecto — puntos de control sin duración (cero días)
//
//  Se clasifican en tres fases del ciclo de vida:
//  • Iniciales   — inicio y arranque formal del proyecto
//  • Intermedios — avance por etapas y entregables parciales
//  • Finales     — cierre, entrega y aprobación definitiva
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum TipoHito {
    #[default]
    Inicial,
    Intermedio,
    Final,
}

impl TipoHito {
    pub fn nombre(&self) -> &'static str {
        match self {
            TipoHito::Inicial => "Inicial",
            TipoHito::Intermedio => "Intermedio",
            TipoHito::Final => "Final",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hito {
    pub id: String,
    pub tipo: TipoHito,
    pub nombre: String,
    pub descripcion: String,
    /// Fecha comprometida para alcanzar el hito
    pub fecha_planificada: Option<NaiveDate>,
    /// Fecha en que se cumplió realmente
    pub fecha_cumplido: Option<NaiveDate>,
    pub completado: bool,
    /// Nombre de quien validó o aprobó el logro del hito
    pub aprobado_por: String,
    /// Referencia de evidencia: número de correo, acta, foto, etc.
    pub evidencia: String,
}

impl Hito {
    pub fn nuevo(
        tipo: TipoHito,
        nombre: impl Into<String>,
        descripcion: impl Into<String>,
    ) -> Self {
        Self {
            id: nuevo_id(),
            tipo,
            nombre: nombre.into(),
            descripcion: descripcion.into(),
            fecha_planificada: None,
            fecha_cumplido: None,
            completado: false,
            aprobado_por: String::new(),
            evidencia: String::new(),
        }
    }
}

/// Plantillas de hitos por fase. Devuelve (nombre, descripción).
pub fn plantillas_hitos(tipo: &TipoHito) -> Vec<(&'static str, &'static str)> {
    match tipo {
        TipoHito::Inicial => vec![
            (
                "Aprobación del presupuesto",
                "El cliente aprueba el presupuesto y la estructura de costos propuesta",
            ),
            (
                "Firma del contrato / Acta de constitución",
                "Contrato firmado por ambas partes — inicio oficial y legal del proyecto",
            ),
            (
                "Primer desembolso recibido",
                "Fondos para materiales disponibles — la obra puede comenzar físicamente",
            ),
            (
                "Aprobación del diseño preliminar",
                "El cliente aprueba planos, maqueta o diseño inicial de la obra",
            ),
            (
                "Contratación de proveedores clave",
                "Proveedores y subcontratistas principales seleccionados y contratados",
            ),
            (
                "Permisos y licencias obtenidos",
                "Permisos municipales, de construcción u otros requeridos están en regla",
            ),
        ],
        TipoHito::Intermedio => {
            vec![
            ("Avance 25% — primera revisión con el cliente",
             "Primera revisión formal de avance: el cliente verifica el progreso y cronograma"),
            ("Estructura base completada",
             "Cimentación, estructura o base principal de la obra finalizada"),
            ("Avance 50% — punto medio del proyecto",
             "Mitad del proyecto: revisión de costos, cronograma y ajustes necesarios"),
            ("Instalaciones completadas",
             "Instalaciones eléctricas, plomería, hidráulica u otras críticas terminadas"),
            ("Avance 75% — revisión pre-entrega",
             "Revisión de acabados, detalles finales y correcciones antes de entrega"),
            ("Aprobación de etapa/módulo clave",
             "Fase o módulo crítico del proyecto completado y aprobado por el cliente"),
            ("Segunda consulta presupuestal aprobada",
             "El cliente aprueba el segundo tramo de gastos operativos y mano de obra"),
        ]
        }
        TipoHito::Final => vec![
            ("Reporte final al 100% registrado",
             "Reporte de avance al 100% entregado y documentado formalmente"),
            ("Entrega física al cliente",
             "La obra o producto terminado es entregado físicamente al cliente"),
            ("Pago final recibido",
             "Último desembolso (20% impuestos) cobrado — ciclo financiero cerrado"),
            ("Confirmación formal del cliente",
             "El cliente confirma por escrito la aceptación del trabajo realizado"),
            ("Acta de cierre firmada",
             "Cierre formal con registro de quién autorizó internamente y quién aprobó el cliente"),
            ("Lanzamiento / apertura oficial",
             "El cliente inaugura, lanza o pone en operación el resultado del proyecto"),
            ("Garantía y soporte post-entrega acordados",
             "Términos de garantía y soporte documentados y acordados con el cliente"),
        ],
    }
}

// ─────────────────────────────────────────────────────────────────────
//  Consulta Previa al Cliente
//
//  REGLA FUNDAMENTAL: Antes de gastar CUALQUIER peso de la empresa se
//  le consulta al cliente con detalle de concepto y monto. Solo al
//  recibir aprobación documentada se procede. Así la empresa NUNCA
//  pierde y NUNCA está en contra del cliente.
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum EstadoConsulta {
    #[default]
    PendienteRespuesta,
    Aprobada,
    Rechazada,
    ModificadaYAprobada,
}

impl EstadoConsulta {
    pub fn nombre(&self) -> &str {
        match self {
            Self::PendienteRespuesta => "Pendiente respuesta del cliente",
            Self::Aprobada => "Aprobada por el cliente",
            Self::Rechazada => "Rechazada por el cliente",
            Self::ModificadaYAprobada => "Modificada y aprobada",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsultaPrevia {
    pub id: String,
    pub fecha: NaiveDate,
    pub concepto: String,
    pub detalle: String,
    pub monto_propuesto: f64,
    pub etapa: String,
    pub estado: EstadoConsulta,
    pub respuesta_cliente: String,
    pub fecha_respuesta: Option<NaiveDate>,
    pub aprobado_por: String,
    pub medio_confirmacion: String,
}

impl ConsultaPrevia {
    pub fn nueva(
        fecha: NaiveDate,
        concepto: String,
        detalle: String,
        monto: f64,
        etapa: String,
    ) -> Self {
        Self {
            id: nuevo_id(),
            fecha,
            concepto,
            detalle,
            monto_propuesto: monto,
            etapa,
            estado: EstadoConsulta::PendienteRespuesta,
            respuesta_cliente: String::new(),
            fecha_respuesta: None,
            aprobado_por: String::new(),
            medio_confirmacion: String::new(),
        }
    }

    pub fn esta_aprobada(&self) -> bool {
        matches!(
            self.estado,
            EstadoConsulta::Aprobada | EstadoConsulta::ModificadaYAprobada
        )
    }
}

// ─────────────────────────────────────────────────────────────────────
//  Desembolsos del Cliente
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum NumeroDesembolso {
    #[default]
    Primero,
    Segundo,
    Final,
}

impl NumeroDesembolso {
    pub fn nombre(&self) -> &str {
        match self {
            Self::Primero => "1ro — Compra de materiales",
            Self::Segundo => "2do — 80% (operativo + mano de obra)",
            Self::Final => "Final — 20% (solo impuestos)",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Desembolso {
    pub id: String,
    pub numero: NumeroDesembolso,
    pub monto_esperado: f64,
    pub monto_recibido: f64,
    pub fecha_esperada: Option<NaiveDate>,
    pub fecha_real: Option<NaiveDate>,
    pub recibido: bool,
    pub notas: String,
    pub destino_autorizado: String,
}

impl Desembolso {
    pub fn nuevo(numero: NumeroDesembolso, monto_esperado: f64, destino: &str) -> Self {
        Self {
            id: nuevo_id(),
            numero,
            monto_esperado,
            monto_recibido: 0.0,
            fecha_esperada: None,
            fecha_real: None,
            recibido: false,
            notas: String::new(),
            destino_autorizado: destino.to_string(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
//  Gastos de la Obra
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CategoriaGasto {
    Materiales,
    ManoObra,
    Viaticos,
    GastosRepresentacion,
    Empleados,
    Impuesto,
    ServiciosSubcontratados,
    EquiposAlquiler,
    OperativoGeneral,
    Otro(String),
}

impl CategoriaGasto {
    pub fn nombre(&self) -> String {
        match self {
            Self::Materiales => "Materiales".to_string(),
            Self::ManoObra => "Mano de obra".to_string(),
            Self::Viaticos => "Viáticos".to_string(),
            Self::GastosRepresentacion => "Gastos de representación".to_string(),
            Self::Empleados => "Empleados / Salarios".to_string(),
            Self::Impuesto => "Impuesto".to_string(),
            Self::ServiciosSubcontratados => "Servicios subcontratados".to_string(),
            Self::EquiposAlquiler => "Equipos / Alquiler".to_string(),
            Self::OperativoGeneral => "Operativo general".to_string(),
            Self::Otro(s) => s.clone(),
        }
    }

    /// Regla del ciclo: cada categoría de gasto solo es válida en su desembolso
    pub fn valido_para(&self, des: &NumeroDesembolso) -> bool {
        match des {
            NumeroDesembolso::Primero => matches!(self, Self::Materiales),
            NumeroDesembolso::Segundo => !matches!(self, Self::Impuesto),
            NumeroDesembolso::Final => matches!(self, Self::Impuesto),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GastoObra {
    pub id: String,
    pub categoria: CategoriaGasto,
    pub descripcion: String,
    pub monto: f64,
    pub fecha: NaiveDate,
    pub desembolso_origen: NumeroDesembolso,
    pub comprobante: String,
    /// ID de la ConsultaPrevia que autorizó este gasto
    pub consulta_id: String,
    pub aprobado: bool,
    pub beneficiario: String,
}

impl GastoObra {
    pub fn nuevo(
        cat: CategoriaGasto,
        desc: String,
        monto: f64,
        fecha: NaiveDate,
        des: NumeroDesembolso,
        consulta_id: String,
    ) -> Self {
        Self {
            id: nuevo_id(),
            categoria: cat,
            descripcion: desc,
            monto,
            fecha,
            desembolso_origen: des,
            comprobante: String::new(),
            consulta_id,
            aprobado: true,
            beneficiario: String::new(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
//  Cambio de Alcance
//
//  Siempre es iniciativa documentada del CLIENTE, nunca de la empresa.
//  Esto protege a la empresa de reclamos: si el cliente pidió algo
//  extra, hay constancia de que fue su requerimiento.
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum EstadoCambio {
    #[default]
    Solicitado,
    Evaluando,
    Aprobado,
    Rechazado,
    Implementado,
}

impl EstadoCambio {
    pub fn nombre(&self) -> &str {
        match self {
            Self::Solicitado => "Solicitado por el cliente",
            Self::Evaluando => "En evaluación",
            Self::Aprobado => "Aprobado",
            Self::Rechazado => "Rechazado",
            Self::Implementado => "Implementado",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CambioAlcance {
    pub id: String,
    pub fecha: NaiveDate,
    pub descripcion: String,
    /// Siempre documentado como solicitud del cliente
    pub solicitado_por: String,
    pub impacto_costo_adicional: f64,
    pub impacto_plazo_dias: i32,
    pub estado: EstadoCambio,
    pub aprobado_cliente: bool,
    pub fecha_aprobacion: Option<NaiveDate>,
    pub medio_aprobacion: String,
    pub referencia_documento: String,
}

impl CambioAlcance {
    pub fn nuevo(fecha: NaiveDate, desc: String, solicitado_por: String) -> Self {
        Self {
            id: nuevo_id(),
            fecha,
            descripcion: desc,
            solicitado_por,
            impacto_costo_adicional: 0.0,
            impacto_plazo_dias: 0,
            estado: EstadoCambio::Solicitado,
            aprobado_cliente: false,
            fecha_aprobacion: None,
            medio_aprobacion: String::new(),
            referencia_documento: String::new(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
//  Reporte de Avance (compartido con el cliente)
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReporteAvance {
    pub id: String,
    pub fecha: NaiveDate,
    pub pct_completado: f64,
    pub etapa_actual: String,
    pub actividades_completadas: Vec<String>,
    pub actividades_pendientes: Vec<String>,
    pub gastos_a_fecha: f64,
    pub entregado_al_cliente: bool,
    pub confirmado_por_cliente: bool,
    pub observaciones_cliente: String,
    pub preparado_por: String,
}

impl ReporteAvance {
    pub fn nuevo(fecha: NaiveDate, pct: f64, etapa: String, preparado_por: String) -> Self {
        Self {
            id: nuevo_id(),
            fecha,
            pct_completado: pct,
            etapa_actual: etapa,
            actividades_completadas: Vec::new(),
            actividades_pendientes: Vec::new(),
            gastos_a_fecha: 0.0,
            entregado_al_cliente: false,
            confirmado_por_cliente: false,
            observaciones_cliente: String::new(),
            preparado_por,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
//  Salud del Ciclo Financiero
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SaludCiclo {
    pub ciclo_integro: bool,
    pub alertas: Vec<String>,
    pub materiales_ejecutado: f64,
    pub operativo_ejecutado: f64,
    pub impuesto_reservado: f64,
    pub impuesto_pagado: f64,
    pub fondo_impuesto_intacto: bool,
    pub gastos_sin_consulta: usize,
}

// ─────────────────────────────────────────────────────────────────────
//  Auditoría de Protección (¿puede la empresa demostrar que el cliente
//  aprobó cada acción?)
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuditoriaProteccion {
    pub total_gastos: usize,
    pub gastos_con_consulta: usize,
    pub porcentaje_cobertura: f64,
    pub cambios_documentados: usize,
    pub cambios_aprobados_cliente: usize,
    pub reportes_enviados: usize,
    pub reportes_confirmados: usize,
    /// true = empresa puede demostrar que cada acción fue aprobada por el cliente
    pub empresa_protegida: bool,
    pub riesgo: String,
}

// ─────────────────────────────────────────────────────────────────────
//  Diagrama de flujo completo de la obra (14 pasos) — Marco Lógico
// ─────────────────────────────────────────────────────────────────────

/// Nivel jerárquico en el Marco Lógico del proyecto.
/// Fin > Propósito > Componente > Actividad.
#[derive(Debug, Clone, PartialEq)]
pub enum NivelMarcoLogico {
    /// Objetivo estratégico de la empresa (cobrar 100%, cerrar con documentación)
    Fin,
    /// Resultado directo que produce el proyecto (obra entregada y aceptada)
    Proposito,
    /// Producto intermedio entregable (contrato firmado, desembolso recibido)
    Componente,
    /// Tarea concreta que produce el componente (registrar RFI, enviar correo)
    Actividad,
}

impl NivelMarcoLogico {
    pub fn nombre(&self) -> &'static str {
        match self {
            Self::Fin => "FIN",
            Self::Proposito => "PROPÓSITO",
            Self::Componente => "COMPONENTE",
            Self::Actividad => "ACTIVIDAD",
        }
    }
    pub fn icono(&self) -> &'static str {
        match self {
            Self::Fin => "🎯",
            Self::Proposito => "🏁",
            Self::Componente => "📦",
            Self::Actividad => "⚙️ ",
        }
    }
}

/// Componente del Marco Lógico al que pertenece el paso.
#[derive(Debug, Clone, PartialEq)]
pub enum ComponenteObra {
    /// A — Contratación documentada (Pasos 1-5): mandato legal del proyecto
    A,
    /// B — Ejecución financiera controlada (Pasos 6-9): dinero en caja, gastos aprobados
    B,
    /// C — Entrega y seguimiento (Pasos 10-13): evidencia de que la obra terminó
    C,
    /// Cierre formal (Paso 14): acta con firmas de ambas partes
    CierreFormal,
}

impl ComponenteObra {
    pub fn nombre(&self) -> &'static str {
        match self {
            Self::A => "A — Contratación documentada",
            Self::B => "B — Ejecución financiera",
            Self::C => "C — Entrega y seguimiento",
            Self::CierreFormal => "Cierre formal del proyecto",
        }
    }
    pub fn rango_pasos(&self) -> &'static str {
        match self {
            Self::A => "Pasos 1-5",
            Self::B => "Pasos 6-9",
            Self::C => "Pasos 10-13",
            Self::CierreFormal => "Paso 14",
        }
    }
    pub fn objetivo(&self) -> &'static str {
        match self {
            Self::A => "Obtener mandato legal del cliente antes de ejecutar",
            Self::B => "Ejecutar el presupuesto con dinero del cliente, no de la empresa",
            Self::C => "Documentar la entrega para cerrar cualquier reclamo posterior",
            Self::CierreFormal => "Dejar constancia legal de quién autorizó y quién aprobó",
        }
    }
}

/// Estado de un paso en el flujo de la obra.
#[derive(Debug, Clone, PartialEq)]
pub enum EstadoPaso {
    /// Completado con datos registrados
    Completado,
    /// Requerido ahora mismo pero no completado (bloqueante)
    Faltante,
    /// No aplica todavía (pasos anteriores incompletos)
    Pendiente,
}

/// Un paso del flujo completo de la obra, desde RFI hasta cierre formal.
/// Cada paso lleva su posición en el Marco Lógico para guía bidireccional.
#[derive(Debug, Clone)]
pub struct PasoFlujo {
    pub numero: u8,
    pub nombre: &'static str,
    pub estado: EstadoPaso,
    /// Descripción de lo encontrado (datos concretos si hecho, qué falta si no)
    pub detalle: String,
    /// Consecuencia/riesgo si se omite
    pub riesgo: &'static str,
    /// Número de opción en menu_obras que resuelve este paso
    pub opcion_menu: Option<&'static str>,
    /// Nivel en la jerarquía del Marco Lógico
    pub nivel: NivelMarcoLogico,
    /// Componente del Marco Lógico al que pertenece
    pub componente: ComponenteObra,
    /// Indicador verificable: cómo sabemos que este paso está completado
    pub indicador: &'static str,
    /// Este paso puede revisarse/actualizarse sin invalidar el proyecto
    pub puede_revisarse: bool,
    /// Números de paso (1-indexed) que se ven afectados si este cambia
    pub afecta_si_cambia: &'static [u8],
}

// ─────────────────────────────────────────────────────────────────────
//  Acta de Cierre — registro formal de quién autorizó y aprobó el fin
// ─────────────────────────────────────────────────────────────────────

/// Registra la firma digital del cierre: quién en la empresa autorizó,
/// quién del lado del cliente aprobó y cómo se documentó.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActaCierre {
    /// Fecha en que se formalizó el cierre
    pub fecha: Option<NaiveDate>,
    /// Nombre del responsable interno que autorizó el cierre de la obra
    pub autorizado_por: String,
    /// Nombre del representante del cliente que aprobó la entrega
    pub aprobado_por_cliente: String,
    /// Medio de confirmación: email, reunión presencial, llamada, documento, etc.
    pub medio_aprobacion: String,
    /// Número o referencia del documento de aceptación (acta física, correo, etc.)
    pub referencia_documento: String,
    /// Observaciones del cliente al recibir la obra
    pub observaciones_cliente: String,
}

/// Un ítem del resultado de validación de cierre.
/// `bloqueante = true` → impide el cierre hasta resolverse.
#[derive(Debug, Clone)]
pub struct ItemValidacion {
    pub ok: bool,
    pub bloqueante: bool,
    pub descripcion: String,
    /// Qué opción del menú de obras resuelve este punto
    pub opcion_menu: Option<&'static str>,
}

/// Resultado completo de la validación de cierre.
#[derive(Debug, Clone)]
pub struct ResultadoCierre {
    pub puede_cerrar: bool,
    pub items: Vec<ItemValidacion>,
}

// ─────────────────────────────────────────────────────────────────────
//  Obra — Entidad Principal
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Obra {
    pub id: String,
    pub nombre: String,
    pub cliente: String,
    pub telefono_cliente: String,
    pub email_cliente: String,
    pub estado: EstadoObra,
    pub fecha_inicio: NaiveDate,

    // ── Flujo de información ──────────────────────────────────────────
    pub rfi: Option<RFI>,
    pub contactos: Vec<ContactoCliente>,
    pub correo_requerimiento: Option<CorreoRequerimiento>,
    pub contrato: Contrato,

    // ── Posición contable ─────────────────────────────────────────────
    pub posicion: PosicionContable,

    // ── Desembolsos (recibidos del cliente) ───────────────────────────
    pub desembolsos: Vec<Desembolso>,

    // ── Transparencia: consulta antes de gastar ───────────────────────
    pub consultas: Vec<ConsultaPrevia>,

    // ── Gastos (solo si tienen consulta aprobada) ─────────────────────
    pub gastos: Vec<GastoObra>,

    // ── Cambios de alcance (iniciativa del cliente documentada) ───────
    pub cambios_alcance: Vec<CambioAlcance>,

    // ── Reportes compartidos con el cliente ───────────────────────────
    pub reportes_avance: Vec<ReporteAvance>,

    // ── Hitos del proyecto (puntos de control por fase) ───────────────
    #[serde(default)]
    pub hitos: Vec<Hito>,

    // ── Estado del ciclo ──────────────────────────────────────────────
    pub salud: SaludCiclo,
    // ── Acta de cierre formal (quién autorizó, quién aprobó) ─────────
    pub acta_cierre: ActaCierre,
    // ── Origen del proyecto en el ciclo de negocio ───────────────────
    /// ID de la propuesta/cotización que originó esta obra (si la hay)
    #[serde(default)]
    pub propuesta_origen_id: Option<String>,
    /// Nombre del origen si no hay propuesta formal registrada (ej. "Referido por X")
    #[serde(default)]
    pub origen_descripcion: String,
    /// Presupuesto de base cero — hoja independiente por obra
    #[serde(default)]
    pub presupuesto_obra: PresupuestoObra,
    pub notas: String,
}

impl Obra {
    pub fn nueva(nombre: String, cliente: String, fecha: NaiveDate) -> Self {
        Self {
            id: nuevo_id(),
            nombre,
            cliente,
            telefono_cliente: String::new(),
            email_cliente: String::new(),
            estado: EstadoObra::RFI,
            fecha_inicio: fecha,
            rfi: None,
            contactos: Vec::new(),
            correo_requerimiento: None,
            contrato: Contrato::default(),
            posicion: PosicionContable::default(),
            desembolsos: Vec::new(),
            consultas: Vec::new(),
            gastos: Vec::new(),
            cambios_alcance: Vec::new(),
            reportes_avance: Vec::new(),
            hitos: Vec::new(),
            salud: SaludCiclo::default(),
            acta_cierre: ActaCierre::default(),
            propuesta_origen_id: None,
            origen_descripcion: String::new(),
            presupuesto_obra: PresupuestoObra::default(),
            notas: String::new(),
        }
    }

    pub fn total_gastos_desembolso(&self, des: &NumeroDesembolso) -> f64 {
        self.gastos
            .iter()
            .filter(|g| &g.desembolso_origen == des && g.aprobado)
            .map(|g| g.monto)
            .sum()
    }

    pub fn pct_avance(&self) -> f64 {
        self.reportes_avance
            .last()
            .map(|r| r.pct_completado)
            .unwrap_or(0.0)
    }

    pub fn total_cobrado(&self) -> f64 {
        self.desembolsos
            .iter()
            .filter(|d| d.recibido)
            .map(|d| d.monto_recibido)
            .sum()
    }

    pub fn total_gastado(&self) -> f64 {
        self.gastos
            .iter()
            .filter(|g| g.aprobado)
            .map(|g| g.monto)
            .sum()
    }

    pub fn saldo_disponible(&self) -> f64 {
        self.total_cobrado() - self.total_gastado()
    }

    /// Calcula los tres valores de la posición contable directamente del flujo
    /// registrado en la obra, sin necesidad de ingreso manual.
    ///
    /// - **disponible**: efectivo en caja = cobrado del cliente − todos los gastos aprobados
    /// - **exigible**:   lo que el cliente todavía nos debe = desembolsos no recibidos
    /// - **realizable**: valor de materiales en inventario = gastos de categoría Materiales
    ///
    /// El total activo corriente = disponible + exigible + realizable.
    /// Nota: si hay cuentas por cobrar en el módulo Cobranzas vinculadas a esta
    /// obra, súmalas al exigible en la capa de presentación (main.rs) para incluir
    /// facturas emitidas fuera del ciclo de desembolsos.
    pub fn calcular_posicion_auto(&self) -> (f64, f64, f64) {
        let disponible = self.saldo_disponible();
        let exigible: f64 = self
            .desembolsos
            .iter()
            .filter(|d| !d.recibido)
            .map(|d| d.monto_esperado)
            .sum();
        let realizable: f64 = self
            .gastos
            .iter()
            .filter(|g| g.aprobado && matches!(g.categoria, CategoriaGasto::Materiales))
            .map(|g| g.monto)
            .sum();
        (disponible, exigible, realizable)
    }

    /// Valida el flujo COMPLETO de la obra en sus 14 pasos, desde el RFI inicial
    /// hasta el acta de cierre. Determina el estado de cada paso:
    /// - `Completado`: datos registrados y verificados
    /// - `Faltante`:   debería estar hecho pero no lo está (requerido ahora)
    /// - `Pendiente`:  no aplica todavía (pasos previos bloqueantes incompletos)
    pub fn validar_flujo_completo(&self) -> Vec<PasoFlujo> {
        // Calcula qué pasos están completados (solo datos, sin secuencia)
        let rfi_ok = self.rfi.is_some();
        let contacto_ok = !self.contactos.is_empty();
        let correo_ok = self.correo_requerimiento.is_some();
        let contrato_cfg_ok = self.contrato.valor_total > 0.0;
        let contrato_fir_ok = self.contrato.firmado;
        let d1 = self
            .desembolsos
            .iter()
            .find(|d| d.numero == NumeroDesembolso::Primero);
        let d2 = self
            .desembolsos
            .iter()
            .find(|d| d.numero == NumeroDesembolso::Segundo);
        let d3 = self
            .desembolsos
            .iter()
            .find(|d| d.numero == NumeroDesembolso::Final);
        let d1_ok = d1.map(|d| d.recibido).unwrap_or(false);
        let d2_ok = d2.map(|d| d.recibido).unwrap_or(false);
        let d3_ok = d3.map(|d| d.recibido).unwrap_or(false);
        let n_aprobadas = self.consultas.iter().filter(|c| c.esta_aprobada()).count();
        let consultas_ok = n_aprobadas > 0;
        let gastos_total = self.gastos.len();
        let gastos_con_cons = self
            .gastos
            .iter()
            .filter(|g| g.aprobado && !g.consulta_id.is_empty())
            .count();
        let gastos_ok = gastos_total > 0 && gastos_con_cons == gastos_total;
        let n_reportes = self.reportes_avance.len();
        let reporte_ok = n_reportes > 0;
        let reporte100_ok = self
            .reportes_avance
            .iter()
            .any(|r| r.pct_completado >= 100.0);
        let confirmado_ok = self
            .reportes_avance
            .iter()
            .any(|r| r.pct_completado >= 100.0 && r.confirmado_por_cliente);
        let acta_ok = !self.acta_cierre.autorizado_por.is_empty()
            && !self.acta_cierre.aprobado_por_cliente.is_empty();

        // Secuencia de dependencias: si un paso bloqueante está incompleto,
        // todos los siguientes pasan a "Pendiente" en lugar de "Faltante"
        let secuencia: &[bool] = &[
            rfi_ok,
            contacto_ok,
            correo_ok,
            contrato_cfg_ok,
            contrato_fir_ok,
            d1_ok,
            consultas_ok,
            gastos_ok,
            d2_ok,
            reporte_ok,
            d3_ok,
            reporte100_ok,
            confirmado_ok,
            acta_ok,
        ];
        // Índice del primer paso incompleto con bloqueo (pasos 0-4 y 5,8,10 son bloqueantes)
        let bloqueantes_idx: &[usize] = &[0, 1, 2, 3, 4, 5, 8, 10, 11, 13]; // RFI, contacto, correo, contrato×2, d1, d2, d3, rep100, acta
        let primer_bloqueo = bloqueantes_idx.iter().find(|&&i| !secuencia[i]).copied();

        let estado_paso = |idx: usize, ok: bool| -> EstadoPaso {
            if ok {
                return EstadoPaso::Completado;
            }
            match primer_bloqueo {
                Some(pi) if idx > pi && bloqueantes_idx.contains(&pi) => EstadoPaso::Pendiente,
                _ => EstadoPaso::Faltante,
            }
        };

        vec![
            PasoFlujo {
                numero: 1, nombre: "RFI — solicitud inicial del cliente",
                estado: estado_paso(0, rfi_ok),
                detalle: if rfi_ok {
                    format!("Canal: {} | {}",
                        self.rfi.as_ref().map(|r| r.canal.as_str()).unwrap_or("-"),
                        &self.rfi.as_ref().map(|r| r.descripcion.as_str()).unwrap_or("-")[..self.rfi.as_ref().map(|r| r.descripcion.len().min(50)).unwrap_or(0)])
                } else { "Sin RFI — no hay evidencia de la solicitud inicial del cliente".to_string() },
                riesgo: "Sin RFI no puedes demostrar qué solicitó el cliente ni cuándo inició el proceso",
                opcion_menu: Some("3 — Registrar RFI"),
                nivel: NivelMarcoLogico::Actividad,
                componente: ComponenteObra::A,
                indicador: "RFI documentado en el sistema con canal y descripción del pedido",
                puede_revisarse: true,
                afecta_si_cambia: &[],
            },
            PasoFlujo {
                numero: 2, nombre: "Contacto con el cliente documentado",
                estado: estado_paso(1, contacto_ok),
                detalle: if contacto_ok {
                    format!("{} contacto(s) registrado(s)", self.contactos.len())
                } else { "Sin contactos — no hay historial de comunicación con el cliente".to_string() },
                riesgo: "Sin historial de contacto el cliente puede negar acuerdos verbales",
                opcion_menu: Some("4 — Registrar contacto con cliente"),
                nivel: NivelMarcoLogico::Actividad,
                componente: ComponenteObra::A,
                indicador: "Al menos un contacto registrado con tipo, resumen y próxima acción",
                puede_revisarse: true,
                afecta_si_cambia: &[],
            },
            PasoFlujo {
                numero: 3, nombre: "Correo de requerimientos enviado",
                estado: estado_paso(2, correo_ok),
                detalle: if correo_ok { "Correo de requerimientos documentado".to_string() }
                         else { "Sin correo — falta el documento que especifica qué acordó el cliente".to_string() },
                riesgo: "Sin este correo el cliente puede alegar que pedía algo diferente a lo ejecutado",
                opcion_menu: Some("5 — Correo de requerimientos"),
                nivel: NivelMarcoLogico::Actividad,
                componente: ComponenteObra::A,
                indicador: "Correo con alcance técnico acordado documentado y fechado",
                puede_revisarse: true,
                afecta_si_cambia: &[4],   // si cambia el alcance, revisar el contrato
            },
            PasoFlujo {
                numero: 4, nombre: "Contrato configurado (valor y estructura)",
                estado: estado_paso(3, contrato_cfg_ok),
                detalle: if contrato_cfg_ok {
                    format!("Total: ${:.2} | No.: {} | 1ro: ${:.2} | 2do: ${:.2} | Final: ${:.2}",
                        self.contrato.valor_total, self.contrato.numero,
                        self.contrato.monto_primer(), self.contrato.monto_segundo(), self.contrato.monto_final())
                } else { "Sin valor de contrato — no se sabe cuánto cobra la empresa".to_string() },
                riesgo: "Sin contrato configurado no hay base para cobrar ni para los desembolsos",
                opcion_menu: Some("6 — Configurar contrato"),
                nivel: NivelMarcoLogico::Componente,
                componente: ComponenteObra::A,
                indicador: "Contrato con valor total > 0 y estructura de 3 desembolsos definida",
                puede_revisarse: true,
                afecta_si_cambia: &[5, 6, 9, 11],   // firma + los 3 desembolsos
            },
            PasoFlujo {
                numero: 5, nombre: "Contrato firmado por el cliente",
                estado: estado_paso(4, contrato_fir_ok),
                detalle: if contrato_fir_ok {
                    format!("Firmado el {} — {} desembolsos creados",
                        self.contrato.fecha_firma.map(|f| f.to_string()).unwrap_or_else(|| "-".to_string()),
                        self.desembolsos.len())
                } else { "Contrato NO firmado — sin firma no hay respaldo legal ni desembolsos".to_string() },
                riesgo: "Sin firma, la empresa trabaja sin contrato. El cliente puede no pagar y no hay nada que reclamar",
                opcion_menu: Some("7 — Firmar contrato (crea desembolsos automáticamente)"),
                nivel: NivelMarcoLogico::Componente,
                componente: ComponenteObra::A,
                indicador: "contrato.firmado = true y 3 desembolsos creados automáticamente",
                puede_revisarse: false,  // acto legal — para cambios usa opción 15 (CambioAlcance)
                afecta_si_cambia: &[],
            },
            PasoFlujo {
                numero: 6, nombre: "1er desembolso recibido (materiales)",
                estado: estado_paso(5, d1_ok),
                detalle: if let Some(d) = d1 {
                    if d.recibido { format!("${:.2} recibidos el {}", d.monto_recibido, d.fecha_real.map(|f| f.to_string()).unwrap_or_else(|| "-".to_string())) }
                    else { format!("PENDIENTE — se esperan ${:.2} para comprar materiales", d.monto_esperado) }
                } else { "No hay desembolso creado — firma el contrato primero".to_string() },
                riesgo: "Sin fondos para materiales la obra no puede iniciar. La empresa pone el dinero de su bolsillo",
                opcion_menu: Some("12 — Registrar desembolso recibido"),
                nivel: NivelMarcoLogico::Componente,
                componente: ComponenteObra::B,
                indicador: "Desembolso Primero marcado como recibido con monto y fecha",
                puede_revisarse: true,   // si el monto fue incorrecto, se puede corregir
                afecta_si_cambia: &[8],  // gastos dependen de que haya fondos
            },
            PasoFlujo {
                numero: 7, nombre: "Consultas previas aprobadas por el cliente",
                estado: estado_paso(6, consultas_ok),
                detalle: if consultas_ok { format!("{} consulta(s) con aprobación documentada del cliente", n_aprobadas) }
                         else { "Sin consultas aprobadas — los gastos no tienen autorización del cliente".to_string() },
                riesgo: "Gastar sin autorización del cliente puede llevar a que rechace los gastos y no pague",
                opcion_menu: Some("9/10 — Consulta previa + respuesta del cliente"),
                nivel: NivelMarcoLogico::Actividad,
                componente: ComponenteObra::B,
                indicador: "Al menos 1 consulta con estado Aprobada o ModificadaYAprobada",
                puede_revisarse: true,   // siempre se pueden agregar más consultas
                afecta_si_cambia: &[8],  // cada nueva consulta habilita un nuevo gasto
            },
            PasoFlujo {
                numero: 8, nombre: "Gastos registrados con consulta aprobada",
                estado: estado_paso(7, gastos_ok),
                detalle: if gastos_total == 0 { "Sin gastos registrados".to_string() }
                         else if gastos_con_cons < gastos_total {
                             format!("{}/{} gastos tienen consulta — {} sin autorización del cliente",
                                 gastos_con_cons, gastos_total, gastos_total - gastos_con_cons)
                         } else {
                             format!("{} gasto(s) | Total: ${:.2}", gastos_total, self.total_gastado())
                         },
                riesgo: "Gastos sin consulta aprobada son riesgo legal — el cliente puede reclamar que no autorizó",
                opcion_menu: Some("13 — Registrar gasto (requiere consulta aprobada)"),
                nivel: NivelMarcoLogico::Actividad,
                componente: ComponenteObra::B,
                indicador: "Todos los gastos vinculados a una consulta_id aprobada",
                puede_revisarse: true,   // se pueden corregir o agregar más
                afecta_si_cambia: &[],
            },
            PasoFlujo {
                numero: 9, nombre: "2do desembolso recibido (80% operativo)",
                estado: estado_paso(8, d2_ok),
                detalle: if let Some(d) = d2 {
                    if d.recibido { format!("${:.2} recibidos el {}", d.monto_recibido, d.fecha_real.map(|f| f.to_string()).unwrap_or_else(|| "-".to_string())) }
                    else { format!("PENDIENTE — se esperan ${:.2} (80% operativo + mano de obra)", d.monto_esperado) }
                } else { "No hay desembolso creado — firma el contrato primero".to_string() },
                riesgo: "Sin el 2do pago la empresa debe adelantar gastos operativos y mano de obra",
                opcion_menu: Some("12 — Registrar desembolso recibido"),
                nivel: NivelMarcoLogico::Componente,
                componente: ComponenteObra::B,
                indicador: "Desembolso Segundo marcado como recibido con monto y fecha",
                puede_revisarse: true,
                afecta_si_cambia: &[],
            },
            PasoFlujo {
                numero: 10, nombre: "Reporte de avance enviado al cliente",
                estado: if reporte_ok { EstadoPaso::Completado } else { EstadoPaso::Faltante },
                detalle: if reporte_ok {
                    format!("{} reporte(s) | Último avance: {:.0}%", n_reportes, self.pct_avance())
                } else { "Sin reportes de avance — el cliente no tiene evidencia del progreso".to_string() },
                riesgo: "Sin reportes el cliente no puede hacer seguimiento. Puede reclamar que no avanzó",
                opcion_menu: Some("16 — Reporte de avance para el cliente"),
                nivel: NivelMarcoLogico::Actividad,
                componente: ComponenteObra::C,
                indicador: "Al menos 1 reporte de avance registrado con porcentaje y descripción",
                puede_revisarse: true,   // siempre se pueden agregar más reportes
                afecta_si_cambia: &[12, 13], // avance lleva al reporte 100% y confirmación
            },
            PasoFlujo {
                numero: 11, nombre: "Pago final recibido (20% impuestos)",
                estado: estado_paso(10, d3_ok),
                detalle: if let Some(d) = d3 {
                    if d.recibido { format!("${:.2} recibidos el {}", d.monto_recibido, d.fecha_real.map(|f| f.to_string()).unwrap_or_else(|| "-".to_string())) }
                    else { format!("PENDIENTE — se esperan ${:.2} (solo para impuestos)", d.monto_esperado) }
                } else { "No hay desembolso final — firma el contrato primero".to_string() },
                riesgo: "Sin el pago final la empresa cubre los impuestos de su bolsillo. Pérdida directa garantizada",
                opcion_menu: Some("12 — Registrar desembolso recibido"),
                nivel: NivelMarcoLogico::Componente,
                componente: ComponenteObra::C,
                indicador: "Desembolso Final marcado como recibido con monto y fecha",
                puede_revisarse: true,
                afecta_si_cambia: &[],
            },
            PasoFlujo {
                numero: 12, nombre: "Reporte final al 100% registrado",
                estado: estado_paso(11, reporte100_ok),
                detalle: if reporte100_ok { "Reporte de avance al 100% documentado".to_string() }
                         else { "Sin reporte al 100% — no hay constancia formal de que la obra terminó".to_string() },
                riesgo: "Sin este reporte el cliente puede reclamar que la obra quedó incompleta",
                opcion_menu: Some("16 — Reporte de avance (% = 100)"),
                nivel: NivelMarcoLogico::Actividad,
                componente: ComponenteObra::C,
                indicador: "Reporte con pct_completado = 100 registrado y documentado",
                puede_revisarse: true,
                afecta_si_cambia: &[13],  // la confirmación del cliente viene después
            },
            PasoFlujo {
                numero: 13, nombre: "Reporte final confirmado por el cliente",
                estado: if confirmado_ok { EstadoPaso::Completado } else { EstadoPaso::Faltante },
                detalle: if confirmado_ok { "El cliente confirmó formalmente la recepción de la obra".to_string() }
                         else { "Sin confirmación del cliente — no hay evidencia de que aceptó el trabajo".to_string() },
                riesgo: "Sin aceptación del cliente puede reclamar defectos después de que la empresa se retire",
                opcion_menu: Some("16 — Reporte de avance (marcar como confirmado por cliente)"),
                nivel: NivelMarcoLogico::Componente,
                componente: ComponenteObra::C,
                indicador: "Reporte al 100% con confirmado_por_cliente = true",
                puede_revisarse: false,  // es un acto del cliente — irreversible
                afecta_si_cambia: &[],
            },
            PasoFlujo {
                numero: 14, nombre: "Acta de cierre (quién autorizó / quién aprobó)",
                estado: estado_paso(13, acta_ok),
                detalle: if acta_ok {
                    format!("Autorizado por: {} | Aprobado (cliente): {} | Fecha: {}",
                        self.acta_cierre.autorizado_por,
                        self.acta_cierre.aprobado_por_cliente,
                        self.acta_cierre.fecha.map(|f| f.to_string()).unwrap_or_else(|| "-".to_string()))
                } else { "Sin acta de cierre — no hay registro formal de quién autorizó y quién aprobó".to_string() },
                riesgo: "Sin acta no hay constancia legal del cierre. Cualquier reclamo posterior no tiene fecha de corte",
                opcion_menu: Some("20 — Actualizar estado → Completada"),
                nivel: NivelMarcoLogico::Proposito,
                componente: ComponenteObra::CierreFormal,
                indicador: "Acta con autorizado_por + aprobado_por_cliente no vacíos y fecha registrada",
                puede_revisarse: false,  // es el cierre legal del proyecto
                afecta_si_cambia: &[],
            },
        ]
    }

    /// Detecta redundancias y regresiones necesarias en el flujo.
    /// Devuelve mensajes de acción correctiva con el paso al que hay que regresar.
    pub fn verificar_redundancias(&self) -> Vec<String> {
        let mut alertas: Vec<String> = Vec::new();

        // ← Paso 7→8: gastos sin consulta aprobada
        let sin_cons = self
            .gastos
            .iter()
            .filter(|g| g.consulta_id.is_empty())
            .count();
        if sin_cons > 0 {
            alertas.push(format!(
                "← Paso 7→8: {} gasto(s) sin consulta aprobada — registra la consulta (op. 9) y apruébala (op. 10) antes de continuar",
                sin_cons
            ));
        }

        // ← Paso 6 o 9: gastos superan lo cobrado
        let gastado = self.total_gastado();
        let cobrado = self.total_cobrado();
        if gastado > cobrado && cobrado > 0.0 {
            alertas.push(format!(
                "← Paso 6/9: Gastos (${:.2}) superan lo cobrado (${:.2}) — solicita el desembolso pendiente (op. 12)",
                gastado, cobrado
            ));
        }

        // ← Paso 7: consultas esperando respuesta del cliente
        let pendientes = self
            .consultas
            .iter()
            .filter(|c| matches!(c.estado, EstadoConsulta::PendienteRespuesta))
            .count();
        if pendientes > 0 {
            alertas.push(format!(
                "← Paso 7: {} consulta(s) sin respuesta — el cliente debe aprobar (op. 10) antes de registrar gastos",
                pendientes
            ));
        }

        // ← Paso 3: hay cambios de alcance pero el correo de requerimientos no se ha actualizado
        if !self.cambios_alcance.is_empty() && self.correo_requerimiento.is_some() {
            alertas.push(
                "← Paso 3: Hay cambios de alcance registrados — considera actualizar el correo de requerimientos (op. 5) para coherencia documental".to_string()
            );
        }

        // ← Paso 13: reporte al 100% sin confirmación del cliente
        let rep100 = self
            .reportes_avance
            .iter()
            .any(|r| r.pct_completado >= 100.0);
        let confirmado = self
            .reportes_avance
            .iter()
            .any(|r| r.confirmado_por_cliente);
        if rep100 && !confirmado {
            alertas.push(
                "← Paso 13: Reporte al 100% pendiente de confirmación — pide al cliente que confirme (op. 16 → opción confirmar)".to_string()
            );
        }

        // ← Paso 10: hay gastos pero no hay reporte de avance
        if !self.gastos.is_empty() && self.reportes_avance.is_empty() {
            alertas.push(
                "← Paso 10: Hay gastos ejecutados pero ningún reporte enviado — envía un reporte de avance (op. 16) para mantener informado al cliente".to_string()
            );
        }

        // Contrato firmado pero no hay desembolsos → inconsistencia interna
        if self.contrato.firmado && self.desembolsos.is_empty() {
            alertas.push(
                "← Paso 5: Contrato marcado como firmado pero sin desembolsos — vuelve a firmarlo (op. 7) para regenerar los desembolsos".to_string()
            );
        }

        alertas
    }
    /// Devuelve un `ResultadoCierre` con la lista de ítems y si se puede cerrar.
    /// Los ítems `bloqueante = true` IMPIDEN el cierre hasta resolverse.
    pub fn validar_cierre(&self) -> ResultadoCierre {
        let mut items: Vec<ItemValidacion> = Vec::new();

        // 1. Contrato firmado
        items.push(ItemValidacion {
            ok: self.contrato.firmado,
            bloqueante: true,
            descripcion: if self.contrato.firmado {
                format!(
                    "Contrato firmado: {} | Total: ${:.2}",
                    self.contrato.numero, self.contrato.valor_total
                )
            } else {
                "Contrato NO firmado — firma el contrato antes de cerrar (opción 7)".to_string()
            },
            opcion_menu: Some("7 — Firmar contrato"),
        });

        // 2. Valor del contrato definido
        items.push(ItemValidacion {
            ok: self.contrato.valor_total > 0.0,
            bloqueante: true,
            descripcion: if self.contrato.valor_total > 0.0 {
                format!("Valor de contrato: ${:.2}", self.contrato.valor_total)
            } else {
                "Valor del contrato no definido — configura el contrato (opción 6)".to_string()
            },
            opcion_menu: Some("6 — Configurar contrato"),
        });

        // 3. Todos los desembolsos recibidos
        let des_total = self.desembolsos.len();
        let des_recibidos = self.desembolsos.iter().filter(|d| d.recibido).count();
        let des_pendientes: Vec<_> = self.desembolsos.iter().filter(|d| !d.recibido).collect();
        items.push(ItemValidacion {
            ok: des_pendientes.is_empty() && des_total > 0,
            bloqueante: true,
            descripcion: if des_pendientes.is_empty() && des_total > 0 {
                format!(
                    "Todos los desembolsos recibidos ({}/{})",
                    des_recibidos, des_total
                )
            } else if des_total == 0 {
                "No se han creado desembolsos — firma el contrato primero (opción 7)".to_string()
            } else {
                let nombres: Vec<_> = des_pendientes.iter().map(|d| d.numero.nombre()).collect();
                format!(
                    "Desembolsos PENDIENTES ({}/{}): {}",
                    des_total - des_recibidos,
                    des_total,
                    nombres.join(", ")
                )
            },
            opcion_menu: Some("12 — Registrar desembolso recibido"),
        });

        // 4. Sin consultas pendientes de respuesta
        let consultas_pendientes = self
            .consultas
            .iter()
            .filter(|c| matches!(c.estado, EstadoConsulta::PendienteRespuesta))
            .count();
        items.push(ItemValidacion {
            ok: consultas_pendientes == 0,
            bloqueante: true,
            descripcion: if consultas_pendientes == 0 {
                "Todas las consultas tienen respuesta del cliente".to_string()
            } else {
                format!(
                    "{} consulta(s) sin respuesta — obtén aprobación del cliente (opción 10)",
                    consultas_pendientes
                )
            },
            opcion_menu: Some("10 — Responder consulta"),
        });

        // 5. Ciclo financiero íntegro
        let ciclo_ok = self.salud.ciclo_integro
            && !self
                .salud
                .alertas
                .iter()
                .any(|a| a.contains("gasto(s) sin consulta"));
        items.push(ItemValidacion {
            ok: ciclo_ok,
            bloqueante: true,
            descripcion: if ciclo_ok {
                "Ciclo financiero íntegro — todos los gastos tienen consulta aprobada".to_string()
            } else {
                format!(
                    "Ciclo financiero con alertas: {} — verifica (opción 17)",
                    self.salud.alertas.first().cloned().unwrap_or_default()
                )
            },
            opcion_menu: Some("17 — Verificar ciclo financiero"),
        });

        // 6. Al menos un reporte de avance al 100%
        let reporte_100 = self
            .reportes_avance
            .iter()
            .any(|r| r.pct_completado >= 100.0);
        items.push(ItemValidacion {
            ok: reporte_100,
            bloqueante: true,
            descripcion: if reporte_100 {
                "Reporte de avance al 100% registrado".to_string()
            } else {
                "Falta reporte de avance al 100% — registra el reporte final (opción 16)"
                    .to_string()
            },
            opcion_menu: Some("16 — Reporte de avance"),
        });

        // 7. Reporte final confirmado por el cliente (advertencia, no bloqueo)
        let confirmado = self
            .reportes_avance
            .iter()
            .any(|r| r.pct_completado >= 100.0 && r.confirmado_por_cliente);
        items.push(ItemValidacion {
            ok: confirmado,
            bloqueante: false,
            descripcion: if confirmado {
                "Reporte final confirmado por el cliente ✓".to_string()
            } else {
                "Reporte final NO confirmado por el cliente — solicita confirmación (opción 16)"
                    .to_string()
            },
            opcion_menu: Some("16 — Reporte de avance"),
        });

        // 8. Sin cambios de alcance sin resolver (advertencia)
        let cambios_sin_resolver = self
            .cambios_alcance
            .iter()
            .filter(|c| matches!(c.estado, EstadoCambio::Solicitado | EstadoCambio::Evaluando))
            .count();
        items.push(ItemValidacion {
            ok: cambios_sin_resolver == 0,
            bloqueante: false,
            descripcion: if cambios_sin_resolver == 0 {
                "Todos los cambios de alcance están resueltos".to_string()
            } else {
                format!("{} cambio(s) de alcance sin resolver — pueden quedar pendientes si el cliente los dejó así (opción 15)", cambios_sin_resolver)
            },
            opcion_menu: Some("15 — Cambio de alcance"),
        });

        // 9. Balance no negativo (advertencia)
        let saldo = self.saldo_disponible();
        items.push(ItemValidacion {
            ok: saldo >= 0.0,
            bloqueante: false,
            descripcion: if saldo >= 0.0 {
                format!("Balance positivo: ${:.2} disponible al cierre", saldo)
            } else {
                format!(
                    "Balance NEGATIVO: ${:.2} — se gastó más de lo cobrado (verifica gastos)",
                    saldo
                )
            },
            opcion_menu: Some("14 — Ver gastos"),
        });

        let puede_cerrar = items.iter().all(|i| !i.bloqueante || i.ok);
        ResultadoCierre {
            puede_cerrar,
            items,
        }
    }

    pub fn verificar_ciclo(&mut self) {
        let mut alertas: Vec<String> = Vec::new();
        let total = self.contrato.valor_total;

        if total <= 0.0 {
            self.salud.ciclo_integro = false;
            self.salud.alertas = vec!["Valor del contrato no definido".to_string()];
            return;
        }

        // Regla 1: 1er desembolso → SOLO materiales
        let no_mat_1er = self
            .gastos
            .iter()
            .filter(|g| g.desembolso_origen == NumeroDesembolso::Primero)
            .filter(|g| g.categoria != CategoriaGasto::Materiales)
            .count();
        if no_mat_1er > 0 {
            alertas.push(format!(
                "⚠  {} gasto(s) del 1er desembolso NO son materiales",
                no_mat_1er
            ));
        }

        // Regla 2: 2do desembolso → NO impuestos (esos van en el final)
        let imp_2do = self
            .gastos
            .iter()
            .filter(|g| g.desembolso_origen == NumeroDesembolso::Segundo)
            .filter(|g| g.categoria == CategoriaGasto::Impuesto)
            .count();
        if imp_2do > 0 {
            alertas.push(format!(
                "⚠  {} impuesto(s) cargado(s) al 2do desembolso — deben ir en el pago final",
                imp_2do
            ));
        }

        // Regla 3: Pago final → ÚNICAMENTE impuestos
        let no_imp_final = self
            .gastos
            .iter()
            .filter(|g| g.desembolso_origen == NumeroDesembolso::Final)
            .filter(|g| g.categoria != CategoriaGasto::Impuesto)
            .count();
        if no_imp_final > 0 {
            alertas.push(format!(
                "🚨 {} gasto(s) del pago final NO son impuestos — CICLO EN RIESGO",
                no_imp_final
            ));
        }

        // Regla 4: Todo gasto debe tener consulta previa aprobada
        let sin_consulta = self
            .gastos
            .iter()
            .filter(|g| g.consulta_id.is_empty())
            .count();
        if sin_consulta > 0 {
            alertas.push(format!(
                "⚠  {} gasto(s) sin consulta previa al cliente",
                sin_consulta
            ));
        }
        self.salud.gastos_sin_consulta = sin_consulta;

        // Calcular acumulados
        self.salud.materiales_ejecutado = self.total_gastos_desembolso(&NumeroDesembolso::Primero);
        self.salud.operativo_ejecutado = self.total_gastos_desembolso(&NumeroDesembolso::Segundo);
        self.salud.impuesto_reservado = total * self.contrato.pct_pago_final / 100.0;
        self.salud.impuesto_pagado = self.total_gastos_desembolso(&NumeroDesembolso::Final);
        self.salud.fondo_impuesto_intacto = no_imp_final == 0 && imp_2do == 0;

        self.salud.ciclo_integro = alertas.is_empty();
        self.salud.alertas = alertas;
    }

    // ── Auditoría de protección ────────────────────────────────────────
    pub fn auditoria(&self) -> AuditoriaProteccion {
        let total_gastos = self.gastos.len();
        let con_consulta = self
            .gastos
            .iter()
            .filter(|g| !g.consulta_id.is_empty() && g.aprobado)
            .count();
        let cambios_doc = self.cambios_alcance.len();
        let cambios_aprobados = self
            .cambios_alcance
            .iter()
            .filter(|c| c.aprobado_cliente)
            .count();
        let reportes_env = self.reportes_avance.len();
        let reportes_conf = self
            .reportes_avance
            .iter()
            .filter(|r| r.confirmado_por_cliente)
            .count();

        let cobertura = if total_gastos > 0 {
            con_consulta as f64 / total_gastos as f64 * 100.0
        } else {
            100.0
        };

        let empresa_protegida =
            con_consulta == total_gastos && (cambios_doc == 0 || cambios_aprobados == cambios_doc);

        let riesgo = if empresa_protegida {
            "NINGUNO — empresa completamente protegida".to_string()
        } else if cobertura >= 80.0 {
            "BAJO — mayoría de gastos con consulta previa".to_string()
        } else if cobertura >= 50.0 {
            "MEDIO — revisar gastos sin consulta".to_string()
        } else {
            "ALTO — empresa expuesta, regularizar de inmediato".to_string()
        };

        AuditoriaProteccion {
            total_gastos,
            gastos_con_consulta: con_consulta,
            porcentaje_cobertura: cobertura,
            cambios_documentados: cambios_doc,
            cambios_aprobados_cliente: cambios_aprobados,
            reportes_enviados: reportes_env,
            reportes_confirmados: reportes_conf,
            empresa_protegida,
            riesgo,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
//  Dashboard
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DashboardObras {
    pub total: usize,
    pub activas: usize,
    pub completadas: usize,
    pub suspendidas: usize,
    pub valor_portafolio: f64,
    pub total_cobrado: f64,
    pub pendiente_cobrar: f64,
    pub total_gastado: f64,
    pub saldo: f64,
    pub ciclos_intactos: usize,
    pub ciclos_con_alerta: usize,
}

// ─────────────────────────────────────────────────────────────────────
//  Presupuesto de Base Cero por Obra
//
//  Cada obra tiene su propia "hoja de cálculo": partidas de gasto que
//  se justifican desde cero en cada período. Sin herencia automática
//  del período anterior — cada partida existe porque se justifica.
// ─────────────────────────────────────────────────────────────────────

/// Prioridad de la partida presupuestal.
/// En base cero, si el presupuesto es insuficiente se cortan primero
/// las Deseables, luego las Importantes, y nunca las Esenciales.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum PrioridadPartida {
    Esencial, // La obra no avanza sin esto
    #[default]
    Importante, // Beneficio significativo, pero puede esperar
    Deseable, // Mejora o comodidad — prescindible si hay recorte
}

impl PrioridadPartida {
    pub fn nombre(&self) -> &'static str {
        match self {
            PrioridadPartida::Esencial => "Esencial",
            PrioridadPartida::Importante => "Importante",
            PrioridadPartida::Deseable => "Deseable",
        }
    }
    pub fn icono(&self) -> &'static str {
        match self {
            PrioridadPartida::Esencial => "🔴",
            PrioridadPartida::Importante => "🟡",
            PrioridadPartida::Deseable => "🟢",
        }
    }
}

/// Una partida de gasto justificada desde cero para un período.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartidaPresupuesto {
    pub id: String,
    /// Categoría (ej. "Mano de obra", "Materiales", "Subcontrato")
    pub categoria: String,
    /// Descripción específica del gasto
    pub descripcion: String,
    /// Por qué es necesario este gasto en este período (la justificación base cero)
    pub justificacion: String,
    /// Período al que corresponde (YYYY-MM, ej. "2026-05")
    pub periodo: String,
    pub prioridad: PrioridadPartida,
    pub monto_solicitado: f64,
    pub monto_aprobado: f64,
    /// Lo que realmente se ejecutó/gastó
    pub monto_ejecutado: f64,
    pub aprobada: bool,
    /// Quién aprobó la partida
    pub aprobado_por: String,
    /// Referencia al gasto real registrado en Vec<GastoObra> (opcional)
    pub gasto_id: Option<String>,
}

impl PartidaPresupuesto {
    pub fn nueva(
        categoria: impl Into<String>,
        descripcion: impl Into<String>,
        justificacion: impl Into<String>,
        periodo: impl Into<String>,
        monto_solicitado: f64,
        prioridad: PrioridadPartida,
    ) -> Self {
        Self {
            id: nuevo_id(),
            categoria: categoria.into(),
            descripcion: descripcion.into(),
            justificacion: justificacion.into(),
            periodo: periodo.into(),
            prioridad,
            monto_solicitado,
            monto_aprobado: 0.0,
            monto_ejecutado: 0.0,
            aprobada: false,
            aprobado_por: String::new(),
            gasto_id: None,
        }
    }

    pub fn varianza(&self) -> f64 {
        self.monto_aprobado - self.monto_ejecutado
    }
    pub fn pct_ejecucion(&self) -> f64 {
        if self.monto_aprobado > 0.0 {
            self.monto_ejecutado / self.monto_aprobado * 100.0
        } else {
            0.0
        }
    }
}

/// El presupuesto de base cero de una obra — una por obra, períodos separados.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PresupuestoObra {
    pub partidas: Vec<PartidaPresupuesto>,
    /// Período activo al abrir el módulo (YYYY-MM)
    pub periodo_activo: String,
    pub notas: String,
}

impl PresupuestoObra {
    /// Partidas del período dado
    pub fn partidas_periodo(&self, periodo: &str) -> Vec<&PartidaPresupuesto> {
        self.partidas
            .iter()
            .filter(|p| p.periodo == periodo)
            .collect()
    }

    /// Lista de períodos únicos con partidas registradas
    pub fn periodos(&self) -> Vec<String> {
        let mut ps: Vec<String> = self.partidas.iter().map(|p| p.periodo.clone()).collect();
        ps.sort();
        ps.dedup();
        ps
    }

    /// Totales del período: (solicitado, aprobado, ejecutado)
    pub fn totales_periodo(&self, periodo: &str) -> (f64, f64, f64) {
        self.partidas_periodo(periodo)
            .iter()
            .fold((0.0, 0.0, 0.0), |acc, p| {
                (
                    acc.0 + p.monto_solicitado,
                    acc.1 + p.monto_aprobado,
                    acc.2 + p.monto_ejecutado,
                )
            })
    }

    /// Totales globales de toda la obra
    pub fn totales_globales(&self) -> (f64, f64, f64) {
        self.partidas.iter().fold((0.0, 0.0, 0.0), |acc, p| {
            (
                acc.0 + p.monto_solicitado,
                acc.1 + p.monto_aprobado,
                acc.2 + p.monto_ejecutado,
            )
        })
    }
}

// ─────────────────────────────────────────────────────────────────────
//  Almacén
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlmacenObras {
    #[serde(default)]
    pub obras: Vec<Obra>,
}

impl AlmacenObras {
    pub fn agregar(&mut self, obra: Obra) {
        self.obras.push(obra);
    }

    pub fn obra(&self, id: &str) -> Option<&Obra> {
        self.obras.iter().find(|o| o.id == id)
    }

    pub fn obra_mut(&mut self, id: &str) -> Option<&mut Obra> {
        self.obras.iter_mut().find(|o| o.id == id)
    }

    pub fn activas(&self) -> Vec<&Obra> {
        self.obras
            .iter()
            .filter(|o| !matches!(o.estado, EstadoObra::Completada | EstadoObra::Cancelada))
            .collect()
    }

    pub fn dashboard(&self) -> DashboardObras {
        let mut d = DashboardObras {
            total: self.obras.len(),
            ..Default::default()
        };
        for o in &self.obras {
            match o.estado {
                EstadoObra::Completada => d.completadas += 1,
                EstadoObra::SuspendidaCliente | EstadoObra::Cancelada => d.suspendidas += 1,
                _ => d.activas += 1,
            }
            d.valor_portafolio += o.contrato.valor_total;
            let cobrado = o.total_cobrado();
            let gastado = o.total_gastado();
            d.total_cobrado += cobrado;
            d.total_gastado += gastado;
            d.saldo += cobrado - gastado;
            for des in &o.desembolsos {
                if !des.recibido {
                    d.pendiente_cobrar += des.monto_esperado;
                }
            }
            if o.salud.ciclo_integro {
                d.ciclos_intactos += 1;
            } else {
                d.ciclos_con_alerta += 1;
            }
        }
        d
    }
}
