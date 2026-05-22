// ═══════════════════════════════════════════════════════════════════════
//  Módulo: Cobranzas y Gestión de Cobro
//
//  Sistema de alertas con workflow obligatorio que garantiza que
//  NINGÚN cobro quede sin acción. La empresa SIEMPRE cobra.
//
//  Flujo de alerta:
//  Nueva → Vista → Aprobada → EnProceso → Completada
//                                       → NoCompletada → Reagendada
//                                                      → Cancelada
//
//  Regla: si una acción no fue completada, el sistema pregunta
//  cuándo reagendar y crea el recordatorio en el calendario.
//  El cliente que diga "llámame el 14" SIEMPRE recibirá esa llamada.
// ═══════════════════════════════════════════════════════════════════════

use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

fn nuevo_id() -> String {
    Uuid::new_v4().to_string()[..8].to_string()
}

// ─────────────────────────────────────────────────────────────────────
//  Prioridad
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum Prioridad {
    #[default]
    Baja,
    Media,
    Alta,
    /// Requiere acción inmediata hoy
    Critica,
}

impl Prioridad {
    pub fn nombre(&self) -> &str {
        match self {
            Self::Baja => "Baja",
            Self::Media => "Media",
            Self::Alta => "Alta",
            Self::Critica => "CRÍTICA",
        }
    }

    pub fn orden(&self) -> u8 {
        match self {
            Self::Critica => 0,
            Self::Alta => 1,
            Self::Media => 2,
            Self::Baja => 3,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
//  Tipo de Alerta
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TipoAlerta {
    /// Desembolso/pago no recibido, ya venció
    PagoVencido,
    /// Pago próximo a vencer (días indicados)
    PagoProximo,
    /// Hay trabajo ejecutado que aún no se ha facturado
    FacturaPendiente,
    /// Llamada programada (por el sistema o por acuerdo con el cliente)
    LlamadaProgramada,
    /// Una llamada no fue contestada — requiere reintento
    LlamadaSinContestacion,
    /// El cliente acordó ser llamado en esta fecha exacta
    LlamadaAcordadaCliente,
    /// Reunión pendiente de confirmar o realizar
    ReunionPendiente,
    /// Documento / contrato / aprobación esperada
    DocumentoPendiente,
    /// Obra avanzando pero 2do desembolso no ha llegado
    ObraEnRiesgoSinCobro,
    /// Cuenta por cobrar con mora
    CuentaMora,
    /// Recordatorio general de gestión
    RecordatorioGeneral,
}

impl TipoAlerta {
    pub fn nombre(&self) -> &str {
        match self {
            Self::PagoVencido => "Pago vencido",
            Self::PagoProximo => "Pago próximo a vencer",
            Self::FacturaPendiente => "Factura pendiente de emisión",
            Self::LlamadaProgramada => "Llamada programada",
            Self::LlamadaSinContestacion => "Llamada sin contestación — reintentar",
            Self::LlamadaAcordadaCliente => "Llamada acordada con el cliente",
            Self::ReunionPendiente => "Reunión pendiente",
            Self::DocumentoPendiente => "Documento pendiente",
            Self::ObraEnRiesgoSinCobro => "Obra avanzando sin cobro del siguiente desembolso",
            Self::CuentaMora => "Cuenta en mora",
            Self::RecordatorioGeneral => "Recordatorio de gestión",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
//  Acción requerida por la alerta
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum AccionRequerida {
    #[default]
    LlamarCliente,
    EnviarEmail,
    EnviarWhatsapp,
    EmitirFactura,
    CoordinarReunion,
    VerificarTransferencia,
    EnviarEstadoCuenta,
    AplicarPenalidad,
    EscalarGerencia,
    AbrirExpedienteLegal,
    Otro(String),
}

impl AccionRequerida {
    pub fn nombre(&self) -> String {
        match self {
            Self::LlamarCliente => "Llamar al cliente".to_string(),
            Self::EnviarEmail => "Enviar email".to_string(),
            Self::EnviarWhatsapp => "Enviar WhatsApp".to_string(),
            Self::EmitirFactura => "Emitir factura".to_string(),
            Self::CoordinarReunion => "Coordinar reunión".to_string(),
            Self::VerificarTransferencia => "Verificar transferencia bancaria".to_string(),
            Self::EnviarEstadoCuenta => "Enviar estado de cuenta".to_string(),
            Self::AplicarPenalidad => "Aplicar penalidad por mora".to_string(),
            Self::EscalarGerencia => "Escalar a gerencia".to_string(),
            Self::AbrirExpedienteLegal => "Abrir expediente legal".to_string(),
            Self::Otro(s) => s.clone(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
//  Estado del Workflow de la Alerta
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum EstadoAlerta {
    /// Generada pero no vista aún
    #[default]
    Nueva,
    /// Usuario la vio (click 1)
    Vista,
    /// Usuario aprobó la acción (click 2)
    Aprobada,
    /// Se está ejecutando la acción ahora
    EnProceso,
    /// Acción completada exitosamente
    Completada,
    /// No pudo completarse — tiene nueva fecha
    Reagendada,
    /// Cancelada con motivo justificado
    Cancelada,
}

impl EstadoAlerta {
    pub fn nombre(&self) -> &str {
        match self {
            Self::Nueva => "Nueva ⚡",
            Self::Vista => "Vista 👁",
            Self::Aprobada => "Aprobada ✓",
            Self::EnProceso => "En proceso ↻",
            Self::Completada => "Completada ✅",
            Self::Reagendada => "Reagendada 📅",
            Self::Cancelada => "Cancelada ✗",
        }
    }

    pub fn puede_avanzar(&self) -> bool {
        !matches!(self, Self::Completada | Self::Cancelada)
    }

    pub fn siguiente(&self) -> Option<EstadoAlerta> {
        match self {
            Self::Nueva => Some(Self::Vista),
            Self::Vista => Some(Self::Aprobada),
            Self::Aprobada => Some(Self::EnProceso),
            Self::EnProceso => Some(Self::Completada),
            _ => None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
//  Intento de Contacto (registro de cada llamada / comunicación)
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TipoContacto {
    Llamada,
    Email,
    WhatsApp,
    Reunion,
    Fax,
    Otro(String),
}

impl TipoContacto {
    pub fn nombre(&self) -> String {
        match self {
            Self::Llamada => "Llamada telefónica".to_string(),
            Self::Email => "Email".to_string(),
            Self::WhatsApp => "WhatsApp".to_string(),
            Self::Reunion => "Reunión".to_string(),
            Self::Fax => "Fax".to_string(),
            Self::Otro(s) => s.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResultadoContacto {
    Exitoso,
    SinContestacion,
    Voicemail,
    Ocupado,
    NumeroInvalido,
    /// Cliente pidió que lo llamen en fecha/hora específica
    ClienteReagendo,
    Rechazado,
    Otro(String),
}

impl ResultadoContacto {
    pub fn nombre(&self) -> String {
        match self {
            Self::Exitoso => "Exitoso — comunicación lograda".to_string(),
            Self::SinContestacion => "Sin contestación".to_string(),
            Self::Voicemail => "Buzón de voz".to_string(),
            Self::Ocupado => "Número ocupado".to_string(),
            Self::NumeroInvalido => "Número inválido / fuera de servicio".to_string(),
            Self::ClienteReagendo => "Cliente solicitó reagendar".to_string(),
            Self::Rechazado => "Llamada rechazada".to_string(),
            Self::Otro(s) => s.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentoContacto {
    pub id: String,
    pub fecha: NaiveDateTime,
    pub tipo: TipoContacto,
    pub resultado: ResultadoContacto,
    pub telefono_usado: String,
    pub duracion_min: u32,
    pub notas: String,
    /// Si el cliente dijo "llámame el X a las Y", se captura aquí
    pub proximo_intento_acordado: Option<NaiveDateTime>,
    pub acuerdo_descripcion: String,
    pub registrado_por: String,
}

impl IntentoContacto {
    pub fn nuevo(
        fecha: NaiveDateTime,
        tipo: TipoContacto,
        resultado: ResultadoContacto,
        tel: String,
        registrado_por: String,
    ) -> Self {
        Self {
            id: nuevo_id(),
            fecha,
            tipo,
            resultado,
            telefono_usado: tel,
            duracion_min: 0,
            notas: String::new(),
            proximo_intento_acordado: None,
            acuerdo_descripcion: String::new(),
            registrado_por,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
//  Alerta de Cobranza (entidad central del módulo)
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertaCobranza {
    pub id: String,
    pub obra_id: String,
    pub cuenta_id: String,
    pub cliente: String,
    pub tipo: TipoAlerta,
    pub prioridad: Prioridad,
    pub estado: EstadoAlerta,
    pub titulo: String,
    pub descripcion: String,
    pub monto_relacionado: f64,

    // ── Fecha y vencimiento ───────────────────────────────────────────
    pub fecha_creacion: NaiveDateTime,
    pub fecha_vencimiento: Option<NaiveDate>,
    pub dias_mora: i64,

    // ── Acción y responsable ──────────────────────────────────────────
    pub accion_requerida: AccionRequerida,
    pub responsable: String,
    pub telefono_contacto: String,
    pub email_contacto: String,
    pub numero_cuenta_banco: String,
    pub banco: String,

    // ── Workflow timestamps ────────────────────────────────────────────
    pub fecha_vista: Option<NaiveDateTime>,
    pub fecha_aprobada: Option<NaiveDateTime>,
    pub fecha_completada: Option<NaiveDateTime>,

    // ── Si no se completó → reagendar ────────────────────────────────
    pub motivo_no_completado: String,
    pub reagendado_para: Option<NaiveDateTime>,
    pub motivo_cancelacion: String,

    // ── Historial de intentos de contacto ────────────────────────────
    pub intentos: Vec<IntentoContacto>,

    // ── Enlace al módulo agenda ───────────────────────────────────────
    pub evento_agenda_id: String,
    pub notas_gestion: String,
    pub auto_generada: bool,
}

impl AlertaCobranza {
    #[allow(clippy::too_many_arguments)]
    pub fn nueva(
        obra_id: String,
        cuenta_id: String,
        cliente: String,
        tipo: TipoAlerta,
        prioridad: Prioridad,
        titulo: String,
        descripcion: String,
        monto: f64,
        fecha: NaiveDateTime,
    ) -> Self {
        Self {
            id: nuevo_id(),
            obra_id,
            cuenta_id,
            cliente,
            tipo,
            prioridad,
            estado: EstadoAlerta::Nueva,
            titulo,
            descripcion,
            monto_relacionado: monto,
            fecha_creacion: fecha,
            fecha_vencimiento: None,
            dias_mora: 0,
            accion_requerida: AccionRequerida::LlamarCliente,
            responsable: String::new(),
            telefono_contacto: String::new(),
            email_contacto: String::new(),
            numero_cuenta_banco: String::new(),
            banco: String::new(),
            fecha_vista: None,
            fecha_aprobada: None,
            fecha_completada: None,
            motivo_no_completado: String::new(),
            reagendado_para: None,
            motivo_cancelacion: String::new(),
            intentos: Vec::new(),
            evento_agenda_id: String::new(),
            notas_gestion: String::new(),
            auto_generada: false,
        }
    }

    pub fn esta_activa(&self) -> bool {
        !matches!(
            self.estado,
            EstadoAlerta::Completada | EstadoAlerta::Cancelada
        )
    }

    pub fn avanzar_workflow(&mut self, ahora: NaiveDateTime) {
        match self.estado {
            EstadoAlerta::Nueva => {
                self.estado = EstadoAlerta::Vista;
                self.fecha_vista = Some(ahora);
            }
            EstadoAlerta::Vista => {
                self.estado = EstadoAlerta::Aprobada;
                self.fecha_aprobada = Some(ahora);
            }
            EstadoAlerta::Aprobada => {
                self.estado = EstadoAlerta::EnProceso;
            }
            EstadoAlerta::EnProceso => {
                self.estado = EstadoAlerta::Completada;
                self.fecha_completada = Some(ahora);
            }
            _ => {}
        }
    }

    pub fn marcar_completada(&mut self, ahora: NaiveDateTime) {
        self.estado = EstadoAlerta::Completada;
        self.fecha_completada = Some(ahora);
    }

    pub fn reagendar(&mut self, nueva_fecha: NaiveDateTime, motivo: String) {
        self.estado = EstadoAlerta::Reagendada;
        self.reagendado_para = Some(nueva_fecha);
        self.motivo_no_completado = motivo;
    }

    pub fn cancelar(&mut self, motivo: String) {
        self.estado = EstadoAlerta::Cancelada;
        self.motivo_cancelacion = motivo;
    }

    pub fn intentos_sin_exito(&self) -> usize {
        self.intentos
            .iter()
            .filter(|i| !matches!(i.resultado, ResultadoContacto::Exitoso))
            .count()
    }
}

// ─────────────────────────────────────────────────────────────────────
//  Perfil de Cobranza del Cliente
//  (banco de memoria: quién, cuándo, a qué número, a qué cuenta)
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum HistorialPago {
    Excelente,
    Bueno,
    #[default]
    Nuevo,
    Regular,
    Malo,
    EnDisputa,
}

impl HistorialPago {
    pub fn nombre(&self) -> &str {
        match self {
            Self::Excelente => "Excelente — paga antes del vencimiento",
            Self::Bueno => "Bueno — paga a tiempo",
            Self::Nuevo => "Nuevo — sin historial",
            Self::Regular => "Regular — paga con demora menor",
            Self::Malo => "Malo — paga con demora significativa",
            Self::EnDisputa => "En disputa",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfilCobranzaCliente {
    pub id: String,
    pub nombre: String,
    pub empresa: String,
    pub rut_cedula: String,
    pub cargo_responsable: String,
    pub responsable_pago: String,

    // ── Contactos (múltiples) ─────────────────────────────────────────
    pub telefono_principal: String,
    pub telefono_alternativo: String,
    pub whatsapp: String,
    pub email: String,
    pub email_facturacion: String,

    // ── Datos bancarios ───────────────────────────────────────────────
    pub banco: String,
    pub tipo_cuenta: String,
    pub numero_cuenta: String,
    pub titular_cuenta: String,

    // ── Términos y preferencias ────────────────────────────────────────
    pub dias_credito: i32,
    pub horario_preferido_contacto: String,
    pub historial_pago: HistorialPago,
    pub requiere_factura_previa: bool,
    pub requiere_orden_compra: bool,
    pub notas: String,

    pub fecha_creacion: NaiveDate,
    pub ultima_actualizacion: Option<NaiveDate>,
}

impl PerfilCobranzaCliente {
    pub fn nuevo(nombre: String, fecha: NaiveDate) -> Self {
        Self {
            id: nuevo_id(),
            nombre,
            empresa: String::new(),
            rut_cedula: String::new(),
            cargo_responsable: String::new(),
            responsable_pago: String::new(),
            telefono_principal: String::new(),
            telefono_alternativo: String::new(),
            whatsapp: String::new(),
            email: String::new(),
            email_facturacion: String::new(),
            banco: String::new(),
            tipo_cuenta: String::new(),
            numero_cuenta: String::new(),
            titular_cuenta: String::new(),
            dias_credito: 30,
            horario_preferido_contacto: String::new(),
            historial_pago: HistorialPago::Nuevo,
            requiere_factura_previa: false,
            requiere_orden_compra: false,
            notas: String::new(),
            fecha_creacion: fecha,
            ultima_actualizacion: None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
//  Cuenta por Cobrar
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum EstadoCuenta {
    #[default]
    Vigente,
    PorVencer,
    Vencida,
    EnNegociacion,
    PagadaParcial,
    Pagada,
    Incobrable,
}

impl EstadoCuenta {
    pub fn nombre(&self) -> &str {
        match self {
            Self::Vigente => "Vigente",
            Self::PorVencer => "Por vencer (≤ 7 días)",
            Self::Vencida => "VENCIDA",
            Self::EnNegociacion => "En negociación",
            Self::PagadaParcial => "Pagada parcialmente",
            Self::Pagada => "Pagada ✓",
            Self::Incobrable => "Incobrable",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TipoCobro {
    Efectivo,
    Transferencia,
    Cheque,
    TarjetaCredito,
    Credito,
    Otro(String),
}

impl TipoCobro {
    pub fn nombre(&self) -> String {
        match self {
            Self::Efectivo => "Efectivo".to_string(),
            Self::Transferencia => "Transferencia bancaria".to_string(),
            Self::Cheque => "Cheque".to_string(),
            Self::TarjetaCredito => "Tarjeta de crédito".to_string(),
            Self::Credito => "Crédito".to_string(),
            Self::Otro(s) => s.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistroCobro {
    pub id: String,
    pub fecha: NaiveDate,
    pub monto: f64,
    pub tipo: TipoCobro,
    pub referencia_bancaria: String,
    pub registrado_por: String,
    pub notas: String,
}

impl RegistroCobro {
    pub fn nuevo(
        fecha: NaiveDate,
        monto: f64,
        tipo: TipoCobro,
        referencia: String,
        por: String,
    ) -> Self {
        Self {
            id: nuevo_id(),
            fecha,
            monto,
            tipo,
            referencia_bancaria: referencia,
            registrado_por: por,
            notas: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuentaCobrar {
    pub id: String,
    pub obra_id: String,
    pub perfil_id: String,
    pub cliente: String,
    pub descripcion: String,
    pub numero_factura: String,
    pub monto_total: f64,
    pub monto_cobrado: f64,
    pub fecha_emision: NaiveDate,
    pub fecha_vencimiento: NaiveDate,
    pub estado: EstadoCuenta,
    pub pagos: Vec<RegistroCobro>,
    pub notas: String,
}

impl CuentaCobrar {
    pub fn nueva(
        obra_id: String,
        cliente: String,
        desc: String,
        monto: f64,
        emision: NaiveDate,
        vencimiento: NaiveDate,
    ) -> Self {
        Self {
            id: nuevo_id(),
            obra_id,
            perfil_id: String::new(),
            cliente,
            descripcion: desc,
            numero_factura: String::new(),
            monto_total: monto,
            monto_cobrado: 0.0,
            fecha_emision: emision,
            fecha_vencimiento: vencimiento,
            estado: EstadoCuenta::Vigente,
            pagos: Vec::new(),
            notas: String::new(),
        }
    }

    pub fn monto_pendiente(&self) -> f64 {
        (self.monto_total - self.monto_cobrado).max(0.0)
    }

    pub fn dias_mora(&self, hoy: NaiveDate) -> i64 {
        if hoy > self.fecha_vencimiento {
            (hoy - self.fecha_vencimiento).num_days()
        } else {
            0
        }
    }

    pub fn dias_para_vencer(&self, hoy: NaiveDate) -> i64 {
        (self.fecha_vencimiento - hoy).num_days()
    }

    pub fn actualizar_estado(&mut self, hoy: NaiveDate) {
        if self.monto_pendiente() <= 0.0 {
            self.estado = EstadoCuenta::Pagada;
        } else if self.monto_cobrado > 0.0 {
            self.estado = EstadoCuenta::PagadaParcial;
        } else if hoy > self.fecha_vencimiento {
            self.estado = EstadoCuenta::Vencida;
        } else if self.dias_para_vencer(hoy) <= 7 {
            self.estado = EstadoCuenta::PorVencer;
        } else {
            self.estado = EstadoCuenta::Vigente;
        }
    }

    pub fn registrar_pago(&mut self, cobro: RegistroCobro, hoy: NaiveDate) {
        self.monto_cobrado += cobro.monto;
        self.pagos.push(cobro);
        self.actualizar_estado(hoy);
    }
}

// ─────────────────────────────────────────────────────────────────────
//  Dashboard de Cobranzas
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DashboardCobranzas {
    pub total_por_cobrar: f64,
    pub monto_vencido: f64,
    pub monto_por_vencer_7dias: f64,
    pub monto_por_vencer_30dias: f64,
    pub monto_cobrado_mes: f64,
    pub cuentas_activas: usize,
    pub cuentas_vencidas: usize,
    pub alertas_criticas: usize,
    pub alertas_altas: usize,
    pub alertas_pendientes_total: usize,
    pub llamadas_programadas_hoy: usize,
    pub llamadas_acordadas_cliente_pendientes: usize,
    pub eficiencia_pct: f64,
}

// ─────────────────────────────────────────────────────────────────────
//  Almacén de Cobranzas
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlmacenCobranzas {
    #[serde(default)]
    pub perfiles: Vec<PerfilCobranzaCliente>,
    #[serde(default)]
    pub cuentas: Vec<CuentaCobrar>,
    #[serde(default)]
    pub alertas: Vec<AlertaCobranza>,
}

impl AlmacenCobranzas {
    // ── Perfiles ──────────────────────────────────────────────────────
    pub fn agregar_perfil(&mut self, p: PerfilCobranzaCliente) {
        self.perfiles.push(p);
    }
    pub fn perfil(&self, id: &str) -> Option<&PerfilCobranzaCliente> {
        self.perfiles.iter().find(|p| p.id == id)
    }
    pub fn perfil_mut(&mut self, id: &str) -> Option<&mut PerfilCobranzaCliente> {
        self.perfiles.iter_mut().find(|p| p.id == id)
    }
    pub fn buscar_perfil(&self, q: &str) -> Vec<&PerfilCobranzaCliente> {
        let q = q.to_lowercase();
        self.perfiles
            .iter()
            .filter(|p| {
                p.nombre.to_lowercase().contains(&q) || p.empresa.to_lowercase().contains(&q)
            })
            .collect()
    }

    // ── Cuentas por cobrar ────────────────────────────────────────────
    pub fn agregar_cuenta(&mut self, c: CuentaCobrar) {
        self.cuentas.push(c);
    }
    pub fn cuenta(&self, id: &str) -> Option<&CuentaCobrar> {
        self.cuentas.iter().find(|c| c.id == id)
    }
    pub fn cuenta_mut(&mut self, id: &str) -> Option<&mut CuentaCobrar> {
        self.cuentas.iter_mut().find(|c| c.id == id)
    }
    pub fn cuentas_activas(&self) -> Vec<&CuentaCobrar> {
        self.cuentas
            .iter()
            .filter(|c| !matches!(c.estado, EstadoCuenta::Pagada | EstadoCuenta::Incobrable))
            .collect()
    }
    pub fn cuentas_vencidas(&self, hoy: NaiveDate) -> Vec<&CuentaCobrar> {
        self.cuentas
            .iter()
            .filter(|c| c.dias_mora(hoy) > 0 && c.monto_pendiente() > 0.0)
            .collect()
    }

    // ── Alertas ────────────────────────────────────────────────────────
    pub fn agregar_alerta(&mut self, a: AlertaCobranza) {
        self.alertas.push(a);
    }
    pub fn alerta(&self, id: &str) -> Option<&AlertaCobranza> {
        self.alertas.iter().find(|a| a.id == id)
    }
    pub fn alerta_mut(&mut self, id: &str) -> Option<&mut AlertaCobranza> {
        self.alertas.iter_mut().find(|a| a.id == id)
    }

    /// Alertas activas ordenadas por prioridad (críticas primero)
    pub fn alertas_activas(&self) -> Vec<&AlertaCobranza> {
        let mut lista: Vec<&AlertaCobranza> =
            self.alertas.iter().filter(|a| a.esta_activa()).collect();
        lista.sort_by_key(|a| a.prioridad.orden());
        lista
    }

    pub fn alertas_criticas(&self) -> Vec<&AlertaCobranza> {
        self.alertas_activas()
            .into_iter()
            .filter(|a| a.prioridad == Prioridad::Critica)
            .collect()
    }

    pub fn llamadas_hoy(&self, hoy: NaiveDate) -> Vec<&AlertaCobranza> {
        self.alertas
            .iter()
            .filter(|a| {
                a.esta_activa()
                    && (matches!(
                        a.tipo,
                        TipoAlerta::LlamadaProgramada | TipoAlerta::LlamadaAcordadaCliente
                    ) && a.fecha_vencimiento == Some(hoy))
                    || (a.reagendado_para.map(|dt| dt.date()) == Some(hoy))
            })
            .collect()
    }

    /// Genera alertas automáticas a partir de cuentas vencidas y próximas
    pub fn generar_alertas_automaticas(&mut self, hoy: NaiveDate, ahora: NaiveDateTime) {
        let ids_con_alerta: Vec<String> = self
            .alertas
            .iter()
            .filter(|a| {
                a.esta_activa()
                    && matches!(a.tipo, TipoAlerta::PagoVencido | TipoAlerta::PagoProximo)
            })
            .map(|a| a.cuenta_id.clone())
            .collect();

        let mut nuevas: Vec<AlertaCobranza> = Vec::new();
        for c in &self.cuentas {
            if matches!(c.estado, EstadoCuenta::Pagada | EstadoCuenta::Incobrable) {
                continue;
            }
            if ids_con_alerta.contains(&c.id) {
                continue;
            }

            let mora = c.dias_mora(hoy);
            let para_vencer = c.dias_para_vencer(hoy);

            if mora > 0 {
                let prioridad = if mora > 30 {
                    Prioridad::Critica
                } else if mora > 7 {
                    Prioridad::Alta
                } else {
                    Prioridad::Media
                };
                let mut a = AlertaCobranza::nueva(
                    c.obra_id.clone(),
                    c.id.clone(),
                    c.cliente.clone(),
                    TipoAlerta::PagoVencido,
                    prioridad,
                    format!("Pago vencido: {} ({} días mora)", c.cliente, mora),
                    format!(
                        "La cuenta #{} por ${:.2} está vencida hace {} días.",
                        c.numero_factura,
                        c.monto_pendiente(),
                        mora
                    ),
                    c.monto_pendiente(),
                    ahora,
                );
                a.fecha_vencimiento = Some(c.fecha_vencimiento);
                a.dias_mora = mora;
                a.auto_generada = true;
                nuevas.push(a);
            } else if (0..=7).contains(&para_vencer) {
                let prioridad = if para_vencer <= 2 {
                    Prioridad::Alta
                } else {
                    Prioridad::Media
                };
                let mut a = AlertaCobranza::nueva(
                    c.obra_id.clone(),
                    c.id.clone(),
                    c.cliente.clone(),
                    TipoAlerta::PagoProximo,
                    prioridad,
                    format!(
                        "Pago próximo: {} — vence en {} días",
                        c.cliente, para_vencer
                    ),
                    format!(
                        "La cuenta #{} por ${:.2} vence el {}.",
                        c.numero_factura,
                        c.monto_pendiente(),
                        c.fecha_vencimiento
                    ),
                    c.monto_pendiente(),
                    ahora,
                );
                a.fecha_vencimiento = Some(c.fecha_vencimiento);
                a.auto_generada = true;
                nuevas.push(a);
            }
        }
        self.alertas.extend(nuevas);
    }

    // ── Dashboard ─────────────────────────────────────────────────────
    pub fn dashboard(&self, hoy: NaiveDate) -> DashboardCobranzas {
        let mut d = DashboardCobranzas::default();
        let mut total_facturable = 0.0f64;

        for c in &self.cuentas {
            let pendiente = c.monto_pendiente();
            if pendiente <= 0.0 {
                continue;
            }
            total_facturable += c.monto_total;
            d.total_por_cobrar += pendiente;
            let mora = c.dias_mora(hoy);
            let para_vencer = c.dias_para_vencer(hoy);
            if mora > 0 {
                d.monto_vencido += pendiente;
                d.cuentas_vencidas += 1;
            }
            if (0..=7).contains(&para_vencer) {
                d.monto_por_vencer_7dias += pendiente;
            }
            if (0..=30).contains(&para_vencer) {
                d.monto_por_vencer_30dias += pendiente;
            }
            if !matches!(c.estado, EstadoCuenta::Pagada) {
                d.cuentas_activas += 1;
            }
        }

        // Cobrado este mes
        let (anio, mes) = (hoy.year_ce().1 as i32, hoy.month0() + 1);
        for c in &self.cuentas {
            for pago in &c.pagos {
                if pago.fecha.year() == anio && pago.fecha.month() == mes {
                    d.monto_cobrado_mes += pago.monto;
                }
            }
        }

        for a in self.alertas_activas() {
            d.alertas_pendientes_total += 1;
            match a.prioridad {
                Prioridad::Critica => d.alertas_criticas += 1,
                Prioridad::Alta => d.alertas_altas += 1,
                _ => {}
            }
        }
        d.llamadas_programadas_hoy = self.llamadas_hoy(hoy).len();
        d.llamadas_acordadas_cliente_pendientes = self
            .alertas
            .iter()
            .filter(|a| a.esta_activa() && matches!(a.tipo, TipoAlerta::LlamadaAcordadaCliente))
            .count();

        d.eficiencia_pct = if total_facturable > 0.0 {
            (total_facturable - d.total_por_cobrar) / total_facturable * 100.0
        } else {
            100.0
        };

        d
    }

    /// Genera líneas CSV para abrir en Excel (facturación)
    pub fn exportar_csv_facturacion(&self, hoy: NaiveDate) -> String {
        let mut csv = "Nro Factura,Cliente,Descripcion,Total,Cobrado,Pendiente,Emision,Vencimiento,Estado,Dias Mora\n".to_string();
        for c in &self.cuentas {
            csv.push_str(&format!(
                "{},{},{},{:.2},{:.2},{:.2},{},{},{},{}\n",
                c.numero_factura,
                c.cliente,
                c.descripcion.replace(',', ";"),
                c.monto_total,
                c.monto_cobrado,
                c.monto_pendiente(),
                c.fecha_emision,
                c.fecha_vencimiento,
                c.estado.nombre(),
                c.dias_mora(hoy),
            ));
        }
        csv
    }
}

// ─────────────────────────────────────────────────────────────────────
//  Helpers de fecha
// ─────────────────────────────────────────────────────────────────────

use chrono::Datelike;
