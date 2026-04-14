//! Calendario y agenda de eventos con soporte de recurrencia.
//!
//! Incluye [`Evento`], [`Agenda`], frecuencias de repetición
//! y horarios de escritura/marcado de días.

use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TipoEvento {
    Reunion,
    Recordatorio,
    FollowUp,
    Cita,
    Cumpleanos,
    Pago,
    Otro(String),
}

impl fmt::Display for TipoEvento {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TipoEvento::Reunion => write!(f, "Reunión"),
            TipoEvento::Recordatorio => write!(f, "Recordatorio"),
            TipoEvento::FollowUp => write!(f, "Follow-Up"),
            TipoEvento::Cita => write!(f, "Cita"),
            TipoEvento::Cumpleanos => write!(f, "Cumpleaños"),
            TipoEvento::Pago => write!(f, "Pago"),
            TipoEvento::Otro(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Frecuencia {
    UnaVez,
    Semanal,
    Mensual,
    Trimestral,
    Semestral,
    Anual,
}

impl fmt::Display for Frecuencia {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Frecuencia::UnaVez => write!(f, "Una vez"),
            Frecuencia::Semanal => write!(f, "Semanal"),
            Frecuencia::Mensual => write!(f, "Mensual"),
            Frecuencia::Trimestral => write!(f, "Trimestral"),
            Frecuencia::Semestral => write!(f, "Semestral"),
            Frecuencia::Anual => write!(f, "Anual"),
        }
    }
}

impl Default for Frecuencia {
    fn default() -> Self {
        Frecuencia::UnaVez
    }
}

impl Frecuencia {
    /// Dado una fecha base, genera las próximas N ocurrencias futuras
    pub fn proximas_ocurrencias(
        &self,
        fecha_base: NaiveDate,
        desde: NaiveDate,
        hasta: NaiveDate,
    ) -> Vec<NaiveDate> {
        if *self == Frecuencia::UnaVez {
            if fecha_base >= desde && fecha_base <= hasta {
                return vec![fecha_base];
            }
            return vec![];
        }

        let mut resultados = Vec::new();
        let mut fecha = fecha_base;

        // Retroceder hasta antes de 'desde' para no perder ocurrencias
        // (para Anual/Cumpleaños, la fecha_base puede ser de hace años)
        while fecha < desde {
            fecha = match self {
                Frecuencia::Semanal => fecha + chrono::Duration::days(7),
                Frecuencia::Mensual => avanzar_meses(fecha, 1),
                Frecuencia::Trimestral => avanzar_meses(fecha, 3),
                Frecuencia::Semestral => avanzar_meses(fecha, 6),
                Frecuencia::Anual => avanzar_meses(fecha, 12),
                Frecuencia::UnaVez => break,
            };
        }

        while fecha <= hasta {
            resultados.push(fecha);
            fecha = match self {
                Frecuencia::Semanal => fecha + chrono::Duration::days(7),
                Frecuencia::Mensual => avanzar_meses(fecha, 1),
                Frecuencia::Trimestral => avanzar_meses(fecha, 3),
                Frecuencia::Semestral => avanzar_meses(fecha, 6),
                Frecuencia::Anual => avanzar_meses(fecha, 12),
                Frecuencia::UnaVez => break,
            };
        }

        resultados
    }
}

fn avanzar_meses(fecha: NaiveDate, meses: u32) -> NaiveDate {
    let total_meses = fecha.month() - 1 + meses;
    let anio_extra = total_meses / 12;
    let mes_nuevo = (total_meses % 12) + 1;
    let anio_nuevo = fecha.year() + anio_extra as i32;
    let dia_max = dias_en_mes_util(anio_nuevo, mes_nuevo);
    let dia = fecha.day().min(dia_max);
    NaiveDate::from_ymd_opt(anio_nuevo, mes_nuevo, dia).unwrap_or(fecha)
}

fn dias_en_mes_util(anio: i32, mes: u32) -> u32 {
    match mes {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (anio % 4 == 0 && anio % 100 != 0) || anio % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evento {
    pub id: String,
    pub titulo: String,
    pub descripcion: String,
    pub tipo: TipoEvento,
    pub fecha: NaiveDate,
    pub hora_inicio: NaiveTime,
    pub hora_fin: Option<NaiveTime>,
    pub recurrente: bool,
    #[serde(default)]
    pub frecuencia: Frecuencia,
    #[serde(default)]
    pub concepto: String,
    pub notas: Vec<String>,
    pub creado: NaiveDateTime,
}

impl Evento {
    pub fn new(
        titulo: String,
        descripcion: String,
        tipo: TipoEvento,
        fecha: NaiveDate,
        hora_inicio: NaiveTime,
        hora_fin: Option<NaiveTime>,
    ) -> Self {
        Evento {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            titulo,
            descripcion,
            tipo,
            fecha,
            hora_inicio,
            hora_fin,
            recurrente: false,
            frecuencia: Frecuencia::UnaVez,
            concepto: String::new(),
            notas: Vec::new(),
            creado: chrono::Local::now().naive_local(),
        }
    }

    pub fn con_frecuencia(mut self, frecuencia: Frecuencia) -> Self {
        self.recurrente = frecuencia != Frecuencia::UnaVez;
        self.frecuencia = frecuencia;
        self
    }

    pub fn con_concepto(mut self, concepto: String) -> Self {
        self.concepto = concepto;
        self
    }

    /// Genera las ocurrencias de este evento en un rango de fechas
    pub fn ocurrencias_en_rango(&self, desde: NaiveDate, hasta: NaiveDate) -> Vec<NaiveDate> {
        self.frecuencia
            .proximas_ocurrencias(self.fecha, desde, hasta)
    }

    /// ¿Este evento ocurre en esta fecha? (considerando recurrencia)
    pub fn ocurre_en(&self, fecha: NaiveDate) -> bool {
        if self.frecuencia == Frecuencia::UnaVez {
            return self.fecha == fecha;
        }
        // Para no iterar todo, chequeamos si la fecha está en las ocurrencias del año
        let inicio_anio = NaiveDate::from_ymd_opt(fecha.year(), 1, 1).unwrap();
        let fin_anio = NaiveDate::from_ymd_opt(fecha.year(), 12, 31).unwrap();
        self.frecuencia
            .proximas_ocurrencias(self.fecha, inicio_anio, fin_anio)
            .contains(&fecha)
    }

    pub fn agregar_nota(&mut self, nota: String) {
        self.notas.push(nota);
    }

    pub fn duracion_minutos(&self) -> Option<i64> {
        self.hora_fin
            .map(|fin| (fin - self.hora_inicio).num_minutes())
    }

    pub fn etiqueta_recurrencia(&self) -> String {
        match self.frecuencia {
            Frecuencia::UnaVez => String::new(),
            _ => format!(" 🔄{}", self.frecuencia),
        }
    }
}

impl fmt::Display for Evento {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fin = self
            .hora_fin
            .map(|h| format!(" - {}", h))
            .unwrap_or_default();
        let recur = self.etiqueta_recurrencia();
        let concepto = if self.concepto.is_empty() {
            String::new()
        } else {
            format!(" ({})", self.concepto)
        };
        write!(
            f,
            "[{}] {} | {} {}{} | {}{}{}",
            self.id, self.titulo, self.fecha, self.hora_inicio, fin, self.tipo, recur, concepto
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HorarioEscritura {
    pub dia: chrono::Weekday,
    pub hora_inicio: NaiveTime,
    pub hora_fin: NaiveTime,
    pub descripcion: String,
}

impl HorarioEscritura {
    pub fn new(
        dia: chrono::Weekday,
        hora_inicio: NaiveTime,
        hora_fin: NaiveTime,
        descripcion: String,
    ) -> Self {
        HorarioEscritura {
            dia,
            hora_inicio,
            hora_fin,
            descripcion,
        }
    }
}

impl fmt::Display for HorarioEscritura {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?}: {} - {} | {}",
            self.dia, self.hora_inicio, self.hora_fin, self.descripcion
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TipoDiaMarcado {
    Libre,
    Feriado,
    Vacaciones,
    Vencimiento,
    Importante,
    Otro(String),
}

impl fmt::Display for TipoDiaMarcado {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TipoDiaMarcado::Libre => write!(f, "Libre"),
            TipoDiaMarcado::Feriado => write!(f, "Feriado"),
            TipoDiaMarcado::Vacaciones => write!(f, "Vacaciones"),
            TipoDiaMarcado::Vencimiento => write!(f, "Vencimiento"),
            TipoDiaMarcado::Importante => write!(f, "Importante"),
            TipoDiaMarcado::Otro(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiaMarcado {
    pub fecha: NaiveDate,
    pub tipo: TipoDiaMarcado,
    pub nota: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Agenda {
    pub eventos: Vec<Evento>,
    pub horarios_escritura: Vec<HorarioEscritura>,
    #[serde(default)]
    pub dias_marcados: Vec<DiaMarcado>,
}

impl Agenda {
    pub fn new() -> Self {
        Agenda {
            eventos: Vec::new(),
            horarios_escritura: Vec::new(),
            dias_marcados: Vec::new(),
        }
    }

    pub fn marcar_dia(&mut self, dia: DiaMarcado) {
        self.dias_marcados.push(dia);
    }

    pub fn marcas_del_dia(&self, fecha: NaiveDate) -> Vec<&DiaMarcado> {
        self.dias_marcados
            .iter()
            .filter(|d| d.fecha == fecha)
            .collect()
    }

    pub fn marcar_rango(
        &mut self,
        desde: NaiveDate,
        hasta: NaiveDate,
        tipo: TipoDiaMarcado,
        nota: String,
    ) {
        let mut fecha = desde;
        while fecha <= hasta {
            self.dias_marcados.push(DiaMarcado {
                fecha,
                tipo: tipo.clone(),
                nota: nota.clone(),
            });
            fecha += chrono::Duration::days(1);
        }
    }

    pub fn limpiar_marcas(&mut self, fecha: NaiveDate) {
        self.dias_marcados.retain(|d| d.fecha != fecha);
    }

    pub fn agregar_evento(&mut self, evento: Evento) {
        self.eventos.push(evento);
    }

    pub fn agregar_horario(&mut self, horario: HorarioEscritura) {
        self.horarios_escritura.push(horario);
    }

    pub fn eliminar_evento(&mut self, id: &str) -> bool {
        let len = self.eventos.len();
        self.eventos.retain(|e| e.id != id);
        self.eventos.len() != len
    }

    pub fn eventos_del_dia(&self, fecha: NaiveDate) -> Vec<&Evento> {
        self.eventos.iter().filter(|e| e.ocurre_en(fecha)).collect()
    }

    pub fn eventos_por_tipo(&self, tipo: &str) -> Vec<&Evento> {
        self.eventos
            .iter()
            .filter(|e| {
                format!("{}", e.tipo)
                    .to_lowercase()
                    .contains(&tipo.to_lowercase())
            })
            .collect()
    }

    pub fn horarios_del_dia(&self, dia: chrono::Weekday) -> Vec<&HorarioEscritura> {
        self.horarios_escritura
            .iter()
            .filter(|h| h.dia == dia)
            .collect()
    }

    pub fn ordenar_eventos(&mut self) {
        self.eventos.sort_by(|a, b| {
            a.fecha
                .cmp(&b.fecha)
                .then(a.hora_inicio.cmp(&b.hora_inicio))
        });
    }
}
