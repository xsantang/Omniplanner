use chrono::{NaiveDate, NaiveTime, NaiveDateTime};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TipoEvento {
    Reunion,
    Recordatorio,
    FollowUp,
    Cita,
    Otro(String),
}

impl fmt::Display for TipoEvento {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TipoEvento::Reunion => write!(f, "Reunión"),
            TipoEvento::Recordatorio => write!(f, "Recordatorio"),
            TipoEvento::FollowUp => write!(f, "Follow-Up"),
            TipoEvento::Cita => write!(f, "Cita"),
            TipoEvento::Otro(s) => write!(f, "{}", s),
        }
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
            notas: Vec::new(),
            creado: chrono::Local::now().naive_local(),
        }
    }

    pub fn agregar_nota(&mut self, nota: String) {
        self.notas.push(nota);
    }

    pub fn duracion_minutos(&self) -> Option<i64> {
        self.hora_fin.map(|fin| {
            (fin - self.hora_inicio).num_minutes()
        })
    }
}

impl fmt::Display for Evento {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fin = self.hora_fin
            .map(|h| format!(" - {}", h))
            .unwrap_or_default();
        write!(
            f,
            "[{}] {} | {} {}{} | {}",
            self.id, self.titulo, self.fecha, self.hora_inicio, fin, self.tipo
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
    pub fn new(dia: chrono::Weekday, hora_inicio: NaiveTime, hora_fin: NaiveTime, descripcion: String) -> Self {
        HorarioEscritura { dia, hora_inicio, hora_fin, descripcion }
    }
}

impl fmt::Display for HorarioEscritura {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {} - {} | {}", self.dia, self.hora_inicio, self.hora_fin, self.descripcion)
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Agenda {
    pub eventos: Vec<Evento>,
    pub horarios_escritura: Vec<HorarioEscritura>,
}

impl Agenda {
    pub fn new() -> Self {
        Agenda {
            eventos: Vec::new(),
            horarios_escritura: Vec::new(),
        }
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
        self.eventos.iter().filter(|e| e.fecha == fecha).collect()
    }

    pub fn eventos_por_tipo(&self, tipo: &str) -> Vec<&Evento> {
        self.eventos
            .iter()
            .filter(|e| format!("{}", e.tipo).to_lowercase().contains(&tipo.to_lowercase()))
            .collect()
    }

    pub fn horarios_del_dia(&self, dia: chrono::Weekday) -> Vec<&HorarioEscritura> {
        self.horarios_escritura.iter().filter(|h| h.dia == dia).collect()
    }

    pub fn ordenar_eventos(&mut self) {
        self.eventos.sort_by(|a, b| {
            a.fecha.cmp(&b.fecha).then(a.hora_inicio.cmp(&b.hora_inicio))
        });
    }
}
