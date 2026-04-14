//! Gestión de tareas con prioridades, etiquetas, follow-ups y estados.
//!
//! Provee [`Task`] y [`TaskManager`] para CRUD completo de tareas
//! con soporte de serialización para persistencia.

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

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
        self.tareas
            .iter()
            .filter(|t| t.follow_up.is_some())
            .collect()
    }

    pub fn ordenar_por_fecha(&mut self) {
        self.tareas
            .sort_by(|a, b| a.fecha_hora().cmp(&b.fecha_hora()));
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn tarea_test(titulo: &str, prioridad: Prioridad) -> Task {
        Task::new(
            titulo.to_string(),
            "desc".to_string(),
            NaiveDate::from_ymd_opt(2026, 4, 9).unwrap(),
            NaiveTime::from_hms_opt(10, 0, 0).unwrap(),
            prioridad,
        )
    }

    #[test]
    fn test_task_crud() {
        let mut mgr = TaskManager::new();
        let t = tarea_test("Comprar leche", Prioridad::Media);
        let id = t.id.clone();
        mgr.agregar(t);

        assert_eq!(mgr.tareas.len(), 1);
        assert!(mgr.buscar(&id).is_some());
        assert_eq!(mgr.buscar(&id).unwrap().titulo, "Comprar leche");

        assert!(mgr.eliminar(&id));
        assert_eq!(mgr.tareas.len(), 0);
        assert!(!mgr.eliminar("noexiste"));
    }

    #[test]
    fn test_task_estados() {
        let mut t = tarea_test("Test", Prioridad::Alta);
        assert_eq!(t.estado, TaskStatus::Pendiente);

        t.cambiar_estado(TaskStatus::EnProgreso);
        assert_eq!(t.estado, TaskStatus::EnProgreso);

        t.cambiar_estado(TaskStatus::Completada);
        assert_eq!(t.estado, TaskStatus::Completada);
    }

    #[test]
    fn test_task_etiquetas() {
        let mut t = tarea_test("Test", Prioridad::Baja);
        t.agregar_etiqueta("trabajo".to_string());
        t.agregar_etiqueta("urgente".to_string());
        t.agregar_etiqueta("trabajo".to_string()); // duplicado
        assert_eq!(t.etiquetas.len(), 2);
    }

    #[test]
    fn test_task_follow_up() {
        let mut t = tarea_test("Test", Prioridad::Media);
        assert!(t.follow_up.is_none());
        let fu = NaiveDate::from_ymd_opt(2026, 4, 15)
            .unwrap()
            .and_hms_opt(14, 0, 0)
            .unwrap();
        t.programar_follow_up(fu);
        assert_eq!(t.follow_up, Some(fu));
    }

    #[test]
    fn test_task_manager_filtros() {
        let mut mgr = TaskManager::new();
        let fecha = NaiveDate::from_ymd_opt(2026, 4, 9).unwrap();
        let otra = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();

        let mut t1 = tarea_test("A", Prioridad::Alta);
        let mut t2 = Task::new(
            "B".into(),
            "d".into(),
            otra,
            NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
            Prioridad::Baja,
        );
        t2.cambiar_estado(TaskStatus::Completada);

        let fu = fecha.and_hms_opt(12, 0, 0).unwrap();
        t1.programar_follow_up(fu);

        mgr.agregar(t1);
        mgr.agregar(t2);

        assert_eq!(mgr.listar_por_fecha(fecha).len(), 1);
        assert_eq!(mgr.listar_pendientes().len(), 1);
        assert_eq!(mgr.listar_follow_ups().len(), 1);
    }

    #[test]
    fn test_task_manager_ordenar() {
        let mut mgr = TaskManager::new();
        mgr.agregar(tarea_test("Baja", Prioridad::Baja));
        mgr.agregar(tarea_test("Urgente", Prioridad::Urgente));
        mgr.agregar(tarea_test("Alta", Prioridad::Alta));

        mgr.ordenar_por_prioridad();
        assert_eq!(mgr.tareas[0].titulo, "Urgente");
        assert_eq!(mgr.tareas[1].titulo, "Alta");
        assert_eq!(mgr.tareas[2].titulo, "Baja");
    }

    #[test]
    fn test_task_display() {
        let t = tarea_test("Demo", Prioridad::Urgente);
        let s = format!("{}", t);
        assert!(s.contains("Demo"));
        assert!(s.contains("Urgente"));
    }
}
