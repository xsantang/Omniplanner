//! Control de versiones tipo Git — commits, ramas e historial.
//!
//! Cada [`Snapshot`] almacena el estado serializado con hash SHA-256.
//! [`DataVcs`] gestiona ramas y permite checkout entre ellas.

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use uuid::Uuid;

/// Un snapshot (versión) de datos
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub hash: String,
    pub mensaje: String,
    pub autor: String,
    pub timestamp: NaiveDateTime,
    pub datos: String, // JSON serializado del estado
    pub padre_id: Option<String>,
}

impl Snapshot {
    pub fn new(datos: String, mensaje: String, autor: String, padre_id: Option<String>) -> Self {
        let hash = calcular_hash(&datos);
        Snapshot {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            hash,
            mensaje,
            autor,
            timestamp: chrono::Local::now().naive_local(),
            datos,
            padre_id,
        }
    }
}

impl fmt::Display for Snapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} | {} | {} | {}",
            &self.hash[..7],
            self.mensaje,
            self.autor,
            self.timestamp.format("%Y-%m-%d %H:%M"),
            self.id
        )
    }
}

fn calcular_hash(datos: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(datos.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Rama del historial de versiones
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rama {
    pub nombre: String,
    pub snapshot_ids: Vec<String>,
}

impl Rama {
    pub fn new(nombre: String) -> Self {
        Rama {
            nombre,
            snapshot_ids: Vec::new(),
        }
    }
}

/// Sistema de control de versiones para datos
#[derive(Debug, Serialize, Deserialize)]
pub struct DataVcs {
    pub snapshots: Vec<Snapshot>,
    pub ramas: Vec<Rama>,
    pub rama_actual: String,
}

impl DataVcs {
    pub fn new() -> Self {
        let main = Rama::new("main".to_string());
        DataVcs {
            snapshots: Vec::new(),
            ramas: vec![main],
            rama_actual: "main".to_string(),
        }
    }

    /// Crear un nuevo snapshot (commit)
    pub fn commit(&mut self, datos: String, mensaje: String, autor: String) -> String {
        let padre_id = self.ultimo_snapshot_id();
        let snapshot = Snapshot::new(datos, mensaje, autor, padre_id);
        let id = snapshot.id.clone();

        self.snapshots.push(snapshot);

        // Agregar a la rama actual
        if let Some(rama) = self.ramas.iter_mut().find(|r| r.nombre == self.rama_actual) {
            rama.snapshot_ids.push(id.clone());
        }

        id
    }

    /// Último snapshot de la rama actual
    pub fn ultimo_snapshot_id(&self) -> Option<String> {
        self.ramas
            .iter()
            .find(|r| r.nombre == self.rama_actual)
            .and_then(|r| r.snapshot_ids.last().cloned())
    }

    /// Obtener un snapshot por ID
    pub fn obtener(&self, id: &str) -> Option<&Snapshot> {
        self.snapshots.iter().find(|s| s.id == id)
    }

    /// Crear nueva rama desde la posición actual
    pub fn crear_rama(&mut self, nombre: String) -> bool {
        if self.ramas.iter().any(|r| r.nombre == nombre) {
            return false;
        }

        let snapshot_ids = self
            .ramas
            .iter()
            .find(|r| r.nombre == self.rama_actual)
            .map(|r| r.snapshot_ids.clone())
            .unwrap_or_default();

        self.ramas.push(Rama {
            nombre: nombre.clone(),
            snapshot_ids,
        });
        self.rama_actual = nombre;
        true
    }

    /// Cambiar de rama
    pub fn cambiar_rama(&mut self, nombre: &str) -> bool {
        if self.ramas.iter().any(|r| r.nombre == nombre) {
            self.rama_actual = nombre.to_string();
            true
        } else {
            false
        }
    }

    /// Log de la rama actual
    pub fn log(&self) -> Vec<&Snapshot> {
        let ids: Vec<String> = self
            .ramas
            .iter()
            .find(|r| r.nombre == self.rama_actual)
            .map(|r| r.snapshot_ids.clone())
            .unwrap_or_default();

        ids.iter()
            .filter_map(|id| self.snapshots.iter().find(|s| s.id == *id))
            .collect()
    }

    /// Diff entre dos snapshots (comparación simple)
    pub fn diff(&self, id_a: &str, id_b: &str) -> Option<String> {
        let a = self.obtener(id_a)?;
        let b = self.obtener(id_b)?;

        if a.hash == b.hash {
            return Some("Sin cambios".to_string());
        }

        let lineas_a: Vec<&str> = a.datos.lines().collect();
        let lineas_b: Vec<&str> = b.datos.lines().collect();

        let mut resultado = String::new();
        let max_len = lineas_a.len().max(lineas_b.len());

        for i in 0..max_len {
            let la = lineas_a.get(i).unwrap_or(&"");
            let lb = lineas_b.get(i).unwrap_or(&"");
            if la != lb {
                resultado.push_str(&format!("- {}\n+ {}\n", la, lb));
            }
        }

        Some(resultado)
    }
}

impl Default for DataVcs {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vcs_commit_y_log() {
        let mut vcs = DataVcs::new();
        assert_eq!(vcs.rama_actual, "main");
        assert!(vcs.ultimo_snapshot_id().is_none());

        let id1 = vcs.commit("{\"v\":1}".into(), "inicial".into(), "test".into());
        assert_eq!(vcs.log().len(), 1);
        assert!(vcs.obtener(&id1).is_some());
        assert_eq!(vcs.obtener(&id1).unwrap().mensaje, "inicial");

        let id2 = vcs.commit("{\"v\":2}".into(), "segundo".into(), "test".into());
        assert_eq!(vcs.log().len(), 2);
        assert_eq!(vcs.obtener(&id2).unwrap().padre_id, Some(id1));
    }

    #[test]
    fn test_vcs_ramas() {
        let mut vcs = DataVcs::new();
        vcs.commit("data1".into(), "c1".into(), "a".into());

        assert!(vcs.crear_rama("feature".into()));
        assert_eq!(vcs.rama_actual, "feature");
        assert!(!vcs.crear_rama("feature".into())); // duplicada

        vcs.commit("data2".into(), "c2 en feature".into(), "a".into());
        assert_eq!(vcs.log().len(), 2); // hereda c1 + c2

        assert!(vcs.cambiar_rama("main"));
        assert_eq!(vcs.log().len(), 1); // solo c1
        assert!(!vcs.cambiar_rama("noexiste"));
    }

    #[test]
    fn test_vcs_hash_consistente() {
        let s1 = Snapshot::new("mismos datos".into(), "m1".into(), "a".into(), None);
        let s2 = Snapshot::new("mismos datos".into(), "m2".into(), "b".into(), None);
        assert_eq!(s1.hash, s2.hash); // mismo dato = mismo hash

        let s3 = Snapshot::new("otros datos".into(), "m3".into(), "a".into(), None);
        assert_ne!(s1.hash, s3.hash); // distinto dato = distinto hash
    }

    #[test]
    fn test_vcs_diff() {
        let mut vcs = DataVcs::new();
        let id1 = vcs.commit("linea1\nlinea2".into(), "v1".into(), "a".into());
        let id2 = vcs.commit("linea1\nmodificada".into(), "v2".into(), "a".into());

        let diff = vcs.diff(&id1, &id2).unwrap();
        assert!(diff.contains("linea2"));
        assert!(diff.contains("modificada"));

        // diff consigo mismo
        let same = vcs.diff(&id1, &id1).unwrap();
        assert_eq!(same, "Sin cambios");

        // diff con ID inexistente
        assert!(vcs.diff(&id1, "noexiste").is_none());
    }
}
