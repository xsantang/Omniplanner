use chrono::{NaiveDate, NaiveTime, NaiveDateTime};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pendiente,
    EnProgreso,
    Completada,
    Cancelada,
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskStatus::Pendiente => write!(f, "Pendiente"),
            TaskStatus::EnProgreso => write!(f, "En Progreso"),
            TaskStatus::Completada => write!(f, "Completada"),
            TaskStatus::Cancelada => write!(f, "Cancelada"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Prioridad {
    Baja,
    Media,
    Alta,
    Urgente,
}

impl fmt::Display for Prioridad {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Prioridad::Baja => write!(f, "Baja"),
            Prioridad::Media => write!(f, "Media"),
            Prioridad::Alta => write!(f, "Alta"),
            Prioridad::Urgente => write!(f, "⚠ Urgente"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub titulo: String,
    pub descripcion: String,
    pub fecha: NaiveDate,
    pub hora: NaiveTime,
    pub estado: TaskStatus,
    pub prioridad: Prioridad,
    pub etiquetas: Vec<String>,
    pub follow_up: Option<NaiveDateTime>,
    pub creado: NaiveDateTime,
    pub actualizado: NaiveDateTime,
}

impl Task {
    pub fn new(
        titulo: String,
        descripcion: String,
        fecha: NaiveDate,
        hora: NaiveTime,
        prioridad: Prioridad,
    ) -> Self {
        let ahora = chrono::Local::now().naive_local();
        Task {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            titulo,
            descripcion,
            fecha,
            hora,
            estado: TaskStatus::Pendiente,
            prioridad,
            etiquetas: Vec::new(),
            follow_up: None,
            creado: ahora,
            actualizado: ahora,
        }
    }

    pub fn programar_follow_up(&mut self, fecha_hora: NaiveDateTime) {
        self.follow_up = Some(fecha_hora);
        self.actualizado = chrono::Local::now().naive_local();
    }

    pub fn cambiar_estado(&mut self, nuevo: TaskStatus) {
        self.estado = nuevo;
        self.actualizado = chrono::Local::now().naive_local();
    }

    pub fn agregar_etiqueta(&mut self, etiqueta: String) {
        if !self.etiquetas.contains(&etiqueta) {
            self.etiquetas.push(etiqueta);
            self.actualizado = chrono::Local::now().naive_local();
        }
    }

    pub fn fecha_hora(&self) -> NaiveDateTime {
        NaiveDateTime::new(self.fecha, self.hora)
    }
}

impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} | {} {} | {} | {}",
            self.id, self.titulo, self.fecha, self.hora, self.prioridad, self.estado
        )
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct TaskManager {
    pub tareas: Vec<Task>,
}

impl TaskManager {
    pub fn new() -> Self {
        TaskManager { tareas: Vec::new() }
    }

    pub fn agregar(&mut self, tarea: Task) {
        self.tareas.push(tarea);
    }

    pub fn eliminar(&mut self, id: &str) -> bool {
        let len_antes = self.tareas.len();
        self.tareas.retain(|t| t.id != id);
        self.tareas.len() != len_antes
    }

    pub fn buscar(&self, id: &str) -> Option<&Task> {
        self.tareas.iter().find(|t| t.id == id)
    }

    pub fn buscar_mut(&mut self, id: &str) -> Option<&mut Task> {
        self.tareas.iter_mut().find(|t| t.id == id)
    }

    pub fn listar_por_fecha(&self, fecha: NaiveDate) -> Vec<&Task> {
        self.tareas.iter().filter(|t| t.fecha == fecha).collect()
    }

    pub fn listar_pendientes(&self) -> Vec<&Task> {
        self.tareas
            .iter()
            .filter(|t| t.estado == TaskStatus::Pendiente || t.estado == TaskStatus::EnProgreso)
            .collect()
    }

    pub fn listar_follow_ups(&self) -> Vec<&Task> {
        self.tareas.iter().filter(|t| t.follow_up.is_some()).collect()
    }

    pub fn ordenar_por_fecha(&mut self) {
        self.tareas.sort_by(|a, b| a.fecha_hora().cmp(&b.fecha_hora()));
    }

    pub fn ordenar_por_prioridad(&mut self) {
        self.tareas.sort_by(|a, b| {
            let prio = |p: &Prioridad| match p {
                Prioridad::Urgente => 0,
                Prioridad::Alta => 1,
                Prioridad::Media => 2,
                Prioridad::Baja => 3,
            };
            prio(&a.prioridad).cmp(&prio(&b.prioridad))
        });
    }
}
