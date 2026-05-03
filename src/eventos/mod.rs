//! Bus de eventos central — el hilo conductor de Omniplanner.
//!
//! Cada acción significativa en cualquier módulo (registrar pago, crear tarea,
//! agendar reunión, etc.) emite un [`EventoSistema`] al bus. Esto permite:
//!
//! - **Paper trail**: historial inmutable de toda transacción/decisión.
//! - **Sincronización entre módulos**: registrar pago genera recordatorio en
//!   agenda + nota en memoria + tarea de seguimiento si aplica.
//! - **Vista unificada**: "Lo que pasó hoy" lee del bus en vez de cada módulo.
//! - **Trazabilidad**: cada evento tiene `referencias` cruzadas que permiten
//!   navegar de un pago → reunión → contacto → factura.

use chrono::{Local, NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

// ═══════════════════════════════════════════════════════════════════════
//  Tipos
// ═══════════════════════════════════════════════════════════════════════

/// Origen del evento (qué módulo lo emitió).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Modulo {
    Rastreador,
    Presupuesto,
    Agenda,
    Tareas,
    Memoria,
    Cartera,   // futuro: cobros por cobrar
    Contactos, // futuro: CRM
    Facturas,  // futuro: emisión
    Sync,
    Sistema,
    Otro(String),
}

impl fmt::Display for Modulo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Modulo::Rastreador => write!(f, "Rastreador"),
            Modulo::Presupuesto => write!(f, "Presupuesto"),
            Modulo::Agenda => write!(f, "Agenda"),
            Modulo::Tareas => write!(f, "Tareas"),
            Modulo::Memoria => write!(f, "Memoria"),
            Modulo::Cartera => write!(f, "Cartera"),
            Modulo::Contactos => write!(f, "Contactos"),
            Modulo::Facturas => write!(f, "Facturas"),
            Modulo::Sync => write!(f, "Sync"),
            Modulo::Sistema => write!(f, "Sistema"),
            Modulo::Otro(s) => write!(f, "{}", s),
        }
    }
}

/// Categoría semántica del evento — independiente del módulo origen.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TipoEvento {
    /// Pago realizado (deuda, gasto, transferencia).
    PagoRealizado,
    /// Pago programado a futuro (compromiso).
    PagoProgramado,
    /// Cobro recibido.
    CobroRecibido,
    /// Cobro pendiente (alguien me debe).
    CobroPendiente,
    /// Compromiso de pago de un tercero.
    CompromisoCobro,
    /// Nueva deuda registrada.
    DeudaCreada,
    /// Deuda liquidada.
    DeudaLiquidada,
    /// Reunión agendada.
    Reunion,
    /// Llamada pendiente o realizada.
    Llamada,
    /// Recordatorio genérico.
    Recordatorio,
    /// Tarea creada/completada.
    Tarea,
    /// Nota o anotación.
    Nota,
    /// Factura emitida.
    FacturaEmitida,
    /// Factura cobrada.
    FacturaCobrada,
    /// Recibo/comprobante registrado.
    Recibo,
    /// Decisión registrada (escenario, matriz).
    Decision,
    /// Alerta del sistema (atraso, déficit, etc.).
    Alerta,
    /// Otro tipo personalizado.
    Otro(String),
}

impl fmt::Display for TipoEvento {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TipoEvento::PagoRealizado => write!(f, "💸 Pago realizado"),
            TipoEvento::PagoProgramado => write!(f, "🗓️  Pago programado"),
            TipoEvento::CobroRecibido => write!(f, "💵 Cobro recibido"),
            TipoEvento::CobroPendiente => write!(f, "⏳ Cobro pendiente"),
            TipoEvento::CompromisoCobro => write!(f, "🤝 Compromiso de cobro"),
            TipoEvento::DeudaCreada => write!(f, "🆕 Deuda creada"),
            TipoEvento::DeudaLiquidada => write!(f, "✅ Deuda liquidada"),
            TipoEvento::Reunion => write!(f, "👥 Reunión"),
            TipoEvento::Llamada => write!(f, "📞 Llamada"),
            TipoEvento::Recordatorio => write!(f, "🔔 Recordatorio"),
            TipoEvento::Tarea => write!(f, "📋 Tarea"),
            TipoEvento::Nota => write!(f, "📝 Nota"),
            TipoEvento::FacturaEmitida => write!(f, "🧾 Factura emitida"),
            TipoEvento::FacturaCobrada => write!(f, "💰 Factura cobrada"),
            TipoEvento::Recibo => write!(f, "🧾 Recibo"),
            TipoEvento::Decision => write!(f, "🎯 Decisión"),
            TipoEvento::Alerta => write!(f, "⚠️  Alerta"),
            TipoEvento::Otro(s) => write!(f, "{}", s),
        }
    }
}

/// Estado actual del evento (su ciclo de vida).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum EstadoEvento {
    /// Pendiente de ejecutar (un compromiso, recordatorio futuro).
    #[default]
    Pendiente,
    /// En curso o iniciado.
    EnCurso,
    /// Completado/realizado satisfactoriamente.
    Realizado,
    /// Cancelado por el usuario.
    Cancelado,
    /// Falló o se incumplió.
    Fallido,
}

impl fmt::Display for EstadoEvento {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EstadoEvento::Pendiente => write!(f, "⏳ pendiente"),
            EstadoEvento::EnCurso => write!(f, "🔄 en curso"),
            EstadoEvento::Realizado => write!(f, "✅ realizado"),
            EstadoEvento::Cancelado => write!(f, "🚫 cancelado"),
            EstadoEvento::Fallido => write!(f, "❌ fallido"),
        }
    }
}

/// Referencia cruzada a otro objeto del sistema.
///
/// Permite que un evento apunte a la deuda concreta, el contacto, la factura,
/// la línea de presupuesto, etc.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Referencia {
    /// Módulo donde vive el objeto referenciado.
    pub modulo: String,
    /// Tipo de objeto: "deuda", "contacto", "factura", "tarea", "evento_agenda".
    pub tipo: String,
    /// Identificador único o nombre del objeto.
    pub id: String,
    /// Etiqueta humana ("Carrington Mortgage", "Juan Pérez", "Factura #2025-04").
    pub etiqueta: String,
}

impl Referencia {
    pub fn nueva(modulo: &str, tipo: &str, id: &str, etiqueta: &str) -> Self {
        Self {
            modulo: modulo.to_string(),
            tipo: tipo.to_string(),
            id: id.to_string(),
            etiqueta: etiqueta.to_string(),
        }
    }
}

/// Adjunto del evento — recibo, contrato, captura, enlace.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Adjunto {
    /// Nombre descriptivo ("Recibo de pago", "Contrato firmado").
    pub nombre: String,
    /// Ruta local al archivo o URL externa.
    pub ruta: String,
    /// Tipo: "archivo", "url", "imagen", "pdf".
    pub tipo: String,
    /// Notas adicionales.
    pub nota: String,
}

// ═══════════════════════════════════════════════════════════════════════
//  EventoSistema — el corazón del bus
// ═══════════════════════════════════════════════════════════════════════

/// Un evento del sistema — registra cualquier acción significativa con
/// trazabilidad completa: quién (módulo), qué (tipo), cuándo (fecha),
/// con quién (contraparte), por cuánto (monto), referencias y adjuntos.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventoSistema {
    /// ID único del evento (UUID corto).
    pub id: String,
    /// Módulo que originó el evento.
    pub origen: Modulo,
    /// Categoría semántica.
    pub tipo: TipoEvento,
    /// Estado actual.
    #[serde(default)]
    pub estado: EstadoEvento,
    /// Título corto.
    pub titulo: String,
    /// Descripción larga (opcional).
    #[serde(default)]
    pub descripcion: String,
    /// Fecha del evento (cuándo ocurrió o cuándo debe ocurrir).
    pub fecha: NaiveDate,
    /// Timestamp de creación.
    pub creado: NaiveDateTime,
    /// Monto involucrado (si aplica): pago, cobro, factura.
    #[serde(default)]
    pub monto: Option<f64>,
    /// Contraparte: persona/entidad relacionada (banco, cliente, proveedor).
    #[serde(default)]
    pub contraparte: String,
    /// Referencias cruzadas a otros objetos del sistema.
    #[serde(default)]
    pub referencias: Vec<Referencia>,
    /// Adjuntos (recibos, contratos, enlaces).
    #[serde(default)]
    pub adjuntos: Vec<Adjunto>,
    /// IDs de eventos relacionados (un pago programado → pago realizado).
    #[serde(default)]
    pub eventos_relacionados: Vec<String>,
    /// Etiquetas para filtrar/buscar.
    #[serde(default)]
    pub etiquetas: Vec<String>,
    /// Notas libres.
    #[serde(default)]
    pub notas: Vec<String>,
}

impl EventoSistema {
    pub fn nuevo(origen: Modulo, tipo: TipoEvento, titulo: impl Into<String>) -> Self {
        let ahora = Local::now().naive_local();
        Self {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            origen,
            tipo,
            estado: EstadoEvento::default(),
            titulo: titulo.into(),
            descripcion: String::new(),
            fecha: ahora.date(),
            creado: ahora,
            monto: None,
            contraparte: String::new(),
            referencias: Vec::new(),
            adjuntos: Vec::new(),
            eventos_relacionados: Vec::new(),
            etiquetas: Vec::new(),
            notas: Vec::new(),
        }
    }

    pub fn con_descripcion(mut self, desc: impl Into<String>) -> Self {
        self.descripcion = desc.into();
        self
    }

    pub fn con_fecha(mut self, fecha: NaiveDate) -> Self {
        self.fecha = fecha;
        self
    }

    pub fn con_monto(mut self, monto: f64) -> Self {
        self.monto = Some(monto);
        self
    }

    pub fn con_contraparte(mut self, contraparte: impl Into<String>) -> Self {
        self.contraparte = contraparte.into();
        self
    }

    pub fn con_estado(mut self, estado: EstadoEvento) -> Self {
        self.estado = estado;
        self
    }

    pub fn con_referencia(mut self, r: Referencia) -> Self {
        self.referencias.push(r);
        self
    }

    pub fn con_etiqueta(mut self, etiqueta: impl Into<String>) -> Self {
        self.etiquetas.push(etiqueta.into());
        self
    }

    pub fn con_nota(mut self, nota: impl Into<String>) -> Self {
        self.notas.push(nota.into());
        self
    }

    pub fn agregar_adjunto(&mut self, adj: Adjunto) {
        self.adjuntos.push(adj);
    }

    pub fn relacionar_con(&mut self, evento_id: impl Into<String>) {
        self.eventos_relacionados.push(evento_id.into());
    }

    /// ¿El evento está vencido (pendiente con fecha pasada)?
    pub fn esta_vencido(&self) -> bool {
        self.estado == EstadoEvento::Pendiente && self.fecha < Local::now().date_naive()
    }

    /// ¿El evento es de hoy?
    pub fn es_de_hoy(&self) -> bool {
        self.fecha == Local::now().date_naive()
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  BusEventos — almacén central
// ═══════════════════════════════════════════════════════════════════════

/// Bus central de eventos del sistema. Vive dentro de `AppState` y persiste
/// en `data.json`. No es un canal en tiempo real — es un libro mayor
/// inmutable de lo que ha pasado y va a pasar.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BusEventos {
    /// Todos los eventos del sistema, ordenados por fecha de creación.
    pub eventos: Vec<EventoSistema>,
}

impl BusEventos {
    pub fn nuevo() -> Self {
        Self::default()
    }

    /// Emite un evento al bus y devuelve su ID.
    pub fn emitir(&mut self, evento: EventoSistema) -> String {
        let id = evento.id.clone();
        self.eventos.push(evento);
        id
    }

    /// Busca un evento por ID.
    pub fn buscar(&self, id: &str) -> Option<&EventoSistema> {
        self.eventos.iter().find(|e| e.id == id)
    }

    /// Busca y permite mutar un evento por ID.
    pub fn buscar_mut(&mut self, id: &str) -> Option<&mut EventoSistema> {
        self.eventos.iter_mut().find(|e| e.id == id)
    }

    /// Marca un evento como realizado.
    pub fn marcar_realizado(&mut self, id: &str) -> bool {
        if let Some(e) = self.buscar_mut(id) {
            e.estado = EstadoEvento::Realizado;
            true
        } else {
            false
        }
    }

    /// Eventos pendientes (no realizados ni cancelados).
    pub fn pendientes(&self) -> Vec<&EventoSistema> {
        self.eventos
            .iter()
            .filter(|e| matches!(e.estado, EstadoEvento::Pendiente | EstadoEvento::EnCurso))
            .collect()
    }

    /// Eventos vencidos: pendientes con fecha pasada.
    pub fn vencidos(&self) -> Vec<&EventoSistema> {
        self.eventos.iter().filter(|e| e.esta_vencido()).collect()
    }

    /// Eventos de una fecha concreta.
    pub fn de_fecha(&self, fecha: NaiveDate) -> Vec<&EventoSistema> {
        self.eventos.iter().filter(|e| e.fecha == fecha).collect()
    }

    /// Eventos de hoy.
    pub fn de_hoy(&self) -> Vec<&EventoSistema> {
        let hoy = Local::now().date_naive();
        self.de_fecha(hoy)
    }

    /// Eventos en un rango de fechas (inclusive).
    pub fn en_rango(&self, desde: NaiveDate, hasta: NaiveDate) -> Vec<&EventoSistema> {
        self.eventos
            .iter()
            .filter(|e| e.fecha >= desde && e.fecha <= hasta)
            .collect()
    }

    /// Eventos por módulo de origen.
    pub fn por_modulo(&self, modulo: &Modulo) -> Vec<&EventoSistema> {
        self.eventos
            .iter()
            .filter(|e| &e.origen == modulo)
            .collect()
    }

    /// Eventos por tipo.
    pub fn por_tipo(&self, tipo: &TipoEvento) -> Vec<&EventoSistema> {
        self.eventos.iter().filter(|e| &e.tipo == tipo).collect()
    }

    /// Eventos relacionados con una referencia (deuda, contacto, factura).
    /// Compara `modulo` + `tipo` + `id`.
    pub fn por_referencia(&self, modulo: &str, tipo: &str, id: &str) -> Vec<&EventoSistema> {
        self.eventos
            .iter()
            .filter(|e| {
                e.referencias
                    .iter()
                    .any(|r| r.modulo == modulo && r.tipo == tipo && r.id == id)
            })
            .collect()
    }

    /// Próximos eventos (pendientes con fecha futura), ordenados por fecha.
    pub fn proximos(&self, limite: usize) -> Vec<&EventoSistema> {
        let hoy = Local::now().date_naive();
        let mut v: Vec<&EventoSistema> = self
            .eventos
            .iter()
            .filter(|e| {
                e.fecha >= hoy
                    && matches!(e.estado, EstadoEvento::Pendiente | EstadoEvento::EnCurso)
            })
            .collect();
        v.sort_by_key(|e| e.fecha);
        v.into_iter().take(limite).collect()
    }

    /// Total de eventos.
    pub fn total(&self) -> usize {
        self.eventos.len()
    }

    /// Vincula dos eventos en ambas direcciones (si no estaban ya enlazados).
    /// Retorna true si ambos existen y la relación quedó establecida.
    pub fn relacionar_eventos(&mut self, id_a: &str, id_b: &str) -> bool {
        if id_a == id_b {
            return false;
        }
        let exists_a = self.eventos.iter().any(|e| e.id == id_a);
        let exists_b = self.eventos.iter().any(|e| e.id == id_b);
        if !exists_a || !exists_b {
            return false;
        }
        if let Some(ev) = self.eventos.iter_mut().find(|e| e.id == id_a) {
            if !ev.eventos_relacionados.iter().any(|x| x == id_b) {
                ev.eventos_relacionados.push(id_b.to_string());
            }
        }
        if let Some(ev) = self.eventos.iter_mut().find(|e| e.id == id_b) {
            if !ev.eventos_relacionados.iter().any(|x| x == id_a) {
                ev.eventos_relacionados.push(id_a.to_string());
            }
        }
        true
    }

    /// Acceso de solo lectura a todos los eventos almacenados.
    pub fn todos(&self) -> &[EventoSistema] {
        &self.eventos
    }

    /// Elimina un evento por ID. Devuelve true si lo encontró y eliminó.
    pub fn eliminar(&mut self, id: &str) -> bool {
        let antes = self.eventos.len();
        self.eventos.retain(|e| e.id != id);
        self.eventos.len() != antes
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn evento_nuevo_tiene_id_y_fecha_hoy() {
        let e = EventoSistema::nuevo(
            Modulo::Rastreador,
            TipoEvento::PagoRealizado,
            "Pago Carrington",
        );
        assert!(!e.id.is_empty());
        assert_eq!(e.fecha, Local::now().date_naive());
        assert_eq!(e.estado, EstadoEvento::Pendiente);
    }

    #[test]
    fn builder_chain() {
        let e = EventoSistema::nuevo(Modulo::Rastreador, TipoEvento::PagoRealizado, "Pago")
            .con_monto(2567.08)
            .con_contraparte("Carrington Mortgage")
            .con_estado(EstadoEvento::Realizado)
            .con_etiqueta("hipoteca")
            .con_nota("Cubre mayo + junio");
        assert_eq!(e.monto, Some(2567.08));
        assert_eq!(e.contraparte, "Carrington Mortgage");
        assert_eq!(e.estado, EstadoEvento::Realizado);
        assert_eq!(e.etiquetas, vec!["hipoteca"]);
        assert_eq!(e.notas, vec!["Cubre mayo + junio"]);
    }

    #[test]
    fn bus_emitir_y_buscar() {
        let mut bus = BusEventos::nuevo();
        let e = EventoSistema::nuevo(Modulo::Rastreador, TipoEvento::PagoRealizado, "Pago");
        let id = bus.emitir(e);
        assert_eq!(bus.total(), 1);
        assert!(bus.buscar(&id).is_some());
    }

    #[test]
    fn bus_filtros() {
        let mut bus = BusEventos::nuevo();
        let hoy = Local::now().date_naive();
        let ayer = hoy - Duration::days(1);

        bus.emitir(EventoSistema::nuevo(
            Modulo::Rastreador,
            TipoEvento::PagoRealizado,
            "A",
        ));
        bus.emitir(
            EventoSistema::nuevo(Modulo::Agenda, TipoEvento::Recordatorio, "B").con_fecha(ayer),
        );

        assert_eq!(bus.de_hoy().len(), 1);
        assert_eq!(bus.por_modulo(&Modulo::Rastreador).len(), 1);
        assert_eq!(bus.por_tipo(&TipoEvento::Recordatorio).len(), 1);
    }

    #[test]
    fn bus_vencidos() {
        let mut bus = BusEventos::nuevo();
        let ayer = Local::now().date_naive() - Duration::days(1);
        bus.emitir(
            EventoSistema::nuevo(Modulo::Rastreador, TipoEvento::PagoProgramado, "Atrasado")
                .con_fecha(ayer),
        );
        bus.emitir(EventoSistema::nuevo(
            Modulo::Rastreador,
            TipoEvento::PagoRealizado,
            "Hoy",
        ));
        assert_eq!(bus.vencidos().len(), 1);
    }

    #[test]
    fn bus_por_referencia() {
        let mut bus = BusEventos::nuevo();
        let r = Referencia::nueva("rastreador", "deuda", "carrington", "Carrington Mortgage");
        bus.emitir(
            EventoSistema::nuevo(Modulo::Rastreador, TipoEvento::PagoRealizado, "Pago")
                .con_referencia(r),
        );
        let resultados = bus.por_referencia("rastreador", "deuda", "carrington");
        assert_eq!(resultados.len(), 1);
    }

    #[test]
    fn bus_marcar_realizado() {
        let mut bus = BusEventos::nuevo();
        let id = bus.emitir(EventoSistema::nuevo(
            Modulo::Rastreador,
            TipoEvento::PagoProgramado,
            "P",
        ));
        assert!(bus.marcar_realizado(&id));
        assert_eq!(bus.buscar(&id).unwrap().estado, EstadoEvento::Realizado);
    }

    #[test]
    fn bus_relacionar_eventos() {
        let mut bus = BusEventos::nuevo();
        let a = bus.emitir(EventoSistema::nuevo(
            Modulo::Rastreador,
            TipoEvento::PagoRealizado,
            "A",
        ));
        let b = bus.emitir(EventoSistema::nuevo(
            Modulo::Presupuesto,
            TipoEvento::PagoRealizado,
            "B",
        ));
        assert!(bus.relacionar_eventos(&a, &b));
        assert!(bus.buscar(&a).unwrap().eventos_relacionados.contains(&b));
        assert!(bus.buscar(&b).unwrap().eventos_relacionados.contains(&a));
        // idempotente
        assert!(bus.relacionar_eventos(&a, &b));
        assert_eq!(bus.buscar(&a).unwrap().eventos_relacionados.len(), 1);
        // ids inexistentes
        assert!(!bus.relacionar_eventos(&a, "no-existe"));
        // mismo id
        assert!(!bus.relacionar_eventos(&a, &a));
    }
}
