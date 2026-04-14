//! Diagramas de flujo con exportación a Mermaid.js y pseudocódigo.
//!
//! Soporta nodos de varios tipos (inicio, fin, decisión, proceso),
//! conexiones condicionales y validación de estructura.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Tipo de nodo en un diagrama
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TipoNodo {
    Inicio,
    Fin,
    Proceso,
    Decision,
    EntradaSalida,
    Conector,
    Subproceso,
    Dato,
}

impl fmt::Display for TipoNodo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TipoNodo::Inicio => write!(f, "⬤ Inicio"),
            TipoNodo::Fin => write!(f, "◯ Fin"),
            TipoNodo::Proceso => write!(f, "▭ Proceso"),
            TipoNodo::Decision => write!(f, "◇ Decisión"),
            TipoNodo::EntradaSalida => write!(f, "▱ E/S"),
            TipoNodo::Conector => write!(f, "● Conector"),
            TipoNodo::Subproceso => write!(f, "▭▭ Subproceso"),
            TipoNodo::Dato => write!(f, "▤ Dato"),
        }
    }
}

/// Nodo de un diagrama
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nodo {
    pub id: String,
    pub tipo: TipoNodo,
    pub etiqueta: String,
    pub x: f64,
    pub y: f64,
    pub ancho: f64,
    pub alto: f64,
    pub metadata: std::collections::HashMap<String, String>,
}

impl Nodo {
    pub fn new(tipo: TipoNodo, etiqueta: String, x: f64, y: f64) -> Self {
        let (ancho, alto) = match tipo {
            TipoNodo::Inicio | TipoNodo::Fin | TipoNodo::Conector => (60.0, 60.0),
            TipoNodo::Decision => (120.0, 80.0),
            _ => (140.0, 60.0),
        };
        Nodo {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            tipo,
            etiqueta,
            x,
            y,
            ancho,
            alto,
            metadata: std::collections::HashMap::new(),
        }
    }
}

impl fmt::Display for Nodo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} [{}] ({:.0},{:.0})",
            self.tipo, self.etiqueta, self.x, self.y
        )
    }
}

/// Tipo de conexión entre nodos
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TipoConexion {
    Flecha,
    LineaRecta,
    Condicional(String), // etiqueta Si/No, True/False
}

/// Conexión (arista) entre dos nodos
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conexion {
    pub origen_id: String,
    pub destino_id: String,
    pub tipo: TipoConexion,
    pub etiqueta: Option<String>,
}

/// Tipo de diagrama
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TipoDiagrama {
    Flujo,
    Algoritmo,
    Proceso,
    DatosFlujo,
    Libre,
}

impl fmt::Display for TipoDiagrama {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TipoDiagrama::Flujo => write!(f, "Diagrama de Flujo"),
            TipoDiagrama::Algoritmo => write!(f, "Algoritmo"),
            TipoDiagrama::Proceso => write!(f, "Diagrama de Proceso"),
            TipoDiagrama::DatosFlujo => write!(f, "Flujo de Datos"),
            TipoDiagrama::Libre => write!(f, "Diagrama Libre"),
        }
    }
}

/// Diagrama completo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagrama {
    pub id: String,
    pub nombre: String,
    pub tipo: TipoDiagrama,
    pub nodos: Vec<Nodo>,
    pub conexiones: Vec<Conexion>,
    pub creado: chrono::NaiveDateTime,
}

impl Diagrama {
    pub fn new(nombre: String, tipo: TipoDiagrama) -> Self {
        Diagrama {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            nombre,
            tipo,
            nodos: Vec::new(),
            conexiones: Vec::new(),
            creado: chrono::Local::now().naive_local(),
        }
    }

    pub fn agregar_nodo(&mut self, nodo: Nodo) -> String {
        let id = nodo.id.clone();
        self.nodos.push(nodo);
        id
    }

    pub fn conectar(
        &mut self,
        origen_id: &str,
        destino_id: &str,
        tipo: TipoConexion,
        etiqueta: Option<String>,
    ) {
        self.conexiones.push(Conexion {
            origen_id: origen_id.to_string(),
            destino_id: destino_id.to_string(),
            tipo,
            etiqueta,
        });
    }

    pub fn eliminar_nodo(&mut self, id: &str) {
        self.nodos.retain(|n| n.id != id);
        self.conexiones
            .retain(|c| c.origen_id != id && c.destino_id != id);
    }

    pub fn buscar_nodo(&self, id: &str) -> Option<&Nodo> {
        self.nodos.iter().find(|n| n.id == id)
    }

    /// Validar que el diagrama de flujo tiene inicio y fin
    pub fn validar_flujo(&self) -> Vec<String> {
        let mut errores = Vec::new();

        let tiene_inicio = self.nodos.iter().any(|n| n.tipo == TipoNodo::Inicio);
        let tiene_fin = self.nodos.iter().any(|n| n.tipo == TipoNodo::Fin);

        if !tiene_inicio {
            errores.push("Falta nodo de Inicio".to_string());
        }
        if !tiene_fin {
            errores.push("Falta nodo de Fin".to_string());
        }

        // Verificar nodos huérfanos (sin conexiones)
        for nodo in &self.nodos {
            let conectado = self
                .conexiones
                .iter()
                .any(|c| c.origen_id == nodo.id || c.destino_id == nodo.id);
            if !conectado && self.nodos.len() > 1 {
                errores.push(format!("Nodo huérfano: {} [{}]", nodo.etiqueta, nodo.id));
            }
        }

        errores
    }

    /// Exportar a formato Mermaid (para renderizado)
    pub fn exportar_mermaid(&self) -> String {
        let mut out = String::from("flowchart TD\n");

        for nodo in &self.nodos {
            let forma = match nodo.tipo {
                TipoNodo::Inicio | TipoNodo::Fin => {
                    format!("    {}(({}))\n", nodo.id, nodo.etiqueta)
                }
                TipoNodo::Decision => format!("    {}{{{{{}}}}}\n", nodo.id, nodo.etiqueta),
                TipoNodo::EntradaSalida => format!("    {}[/{}\\]\n", nodo.id, nodo.etiqueta),
                TipoNodo::Subproceso => format!("    {}[[{}]]\n", nodo.id, nodo.etiqueta),
                _ => format!("    {}[{}]\n", nodo.id, nodo.etiqueta),
            };
            out.push_str(&forma);
        }

        for conn in &self.conexiones {
            let label = conn.etiqueta.as_deref().unwrap_or("");
            if label.is_empty() {
                out.push_str(&format!("    {} --> {}\n", conn.origen_id, conn.destino_id));
            } else {
                out.push_str(&format!(
                    "    {} -->|{}| {}\n",
                    conn.origen_id, label, conn.destino_id
                ));
            }
        }

        out
    }

    /// Exportar diagrama a pseudocódigo (para algoritmos)
    pub fn exportar_pseudocodigo(&self) -> String {
        let mut pseudo = String::from("ALGORITMO: ");
        pseudo.push_str(&self.nombre);
        pseudo.push('\n');
        pseudo.push_str(&"=".repeat(40));
        pseudo.push('\n');

        for nodo in &self.nodos {
            match nodo.tipo {
                TipoNodo::Inicio => {
                    pseudo.push_str(&format!("INICIO: {}\n", nodo.etiqueta));
                }
                TipoNodo::Fin => {
                    pseudo.push_str(&format!("FIN: {}\n", nodo.etiqueta));
                }
                TipoNodo::Proceso => {
                    pseudo.push_str(&format!("  HACER: {}\n", nodo.etiqueta));
                }
                TipoNodo::Decision => {
                    pseudo.push_str(&format!("  SI ({}) ENTONCES\n", nodo.etiqueta));
                    // buscar conexiones condicionales
                    for conn in &self.conexiones {
                        if conn.origen_id == nodo.id {
                            if let Some(ref label) = conn.etiqueta {
                                pseudo.push_str(&format!(
                                    "    {}: ir a {}\n",
                                    label, conn.destino_id
                                ));
                            }
                        }
                    }
                }
                TipoNodo::EntradaSalida => {
                    pseudo.push_str(&format!("  LEER/ESCRIBIR: {}\n", nodo.etiqueta));
                }
                _ => {
                    pseudo.push_str(&format!("  {}: {}\n", nodo.tipo, nodo.etiqueta));
                }
            }
        }

        pseudo
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagrama_crud() {
        let mut d = Diagrama::new("Test Flow".into(), TipoDiagrama::Flujo);
        let inicio = Nodo::new(TipoNodo::Inicio, "Start".into(), 0.0, 0.0);
        let fin = Nodo::new(TipoNodo::Fin, "End".into(), 0.0, 100.0);
        let id_inicio = d.agregar_nodo(inicio);
        let id_fin = d.agregar_nodo(fin);

        assert_eq!(d.nodos.len(), 2);
        assert!(d.buscar_nodo(&id_inicio).is_some());

        d.conectar(&id_inicio, &id_fin, TipoConexion::Flecha, None);
        assert_eq!(d.conexiones.len(), 1);

        d.eliminar_nodo(&id_inicio);
        assert_eq!(d.nodos.len(), 1);
        assert_eq!(d.conexiones.len(), 0); // conexión eliminada en cascada
    }

    #[test]
    fn test_diagrama_validar_flujo() {
        let mut d = Diagrama::new("Incompleto".into(), TipoDiagrama::Flujo);
        let errores = d.validar_flujo();
        assert!(errores.iter().any(|e| e.contains("Inicio")));
        assert!(errores.iter().any(|e| e.contains("Fin")));

        let inicio = Nodo::new(TipoNodo::Inicio, "I".into(), 0.0, 0.0);
        let fin = Nodo::new(TipoNodo::Fin, "F".into(), 0.0, 100.0);
        let id_i = d.agregar_nodo(inicio);
        let id_f = d.agregar_nodo(fin);
        d.conectar(&id_i, &id_f, TipoConexion::Flecha, None);

        let errores = d.validar_flujo();
        assert!(errores.is_empty(), "Errores: {:?}", errores);
    }

    #[test]
    fn test_diagrama_nodo_huerfano() {
        let mut d = Diagrama::new("Huérfano".into(), TipoDiagrama::Algoritmo);
        let inicio = Nodo::new(TipoNodo::Inicio, "I".into(), 0.0, 0.0);
        let fin = Nodo::new(TipoNodo::Fin, "F".into(), 0.0, 100.0);
        let proc = Nodo::new(TipoNodo::Proceso, "Suelto".into(), 50.0, 50.0);
        let id_i = d.agregar_nodo(inicio);
        let id_f = d.agregar_nodo(fin);
        d.agregar_nodo(proc);
        d.conectar(&id_i, &id_f, TipoConexion::Flecha, None);

        let errores = d.validar_flujo();
        assert!(errores.iter().any(|e| e.contains("huérfano")));
    }

    #[test]
    fn test_diagrama_mermaid() {
        let mut d = Diagrama::new("Mermaid".into(), TipoDiagrama::Flujo);
        let id_i = d.agregar_nodo(Nodo::new(TipoNodo::Inicio, "Start".into(), 0.0, 0.0));
        let id_p = d.agregar_nodo(Nodo::new(TipoNodo::Proceso, "Work".into(), 0.0, 50.0));
        let id_f = d.agregar_nodo(Nodo::new(TipoNodo::Fin, "End".into(), 0.0, 100.0));
        d.conectar(&id_i, &id_p, TipoConexion::Flecha, None);
        d.conectar(&id_p, &id_f, TipoConexion::Flecha, Some("done".into()));

        let mmd = d.exportar_mermaid();
        assert!(mmd.starts_with("flowchart TD"));
        assert!(mmd.contains("Start"));
        assert!(mmd.contains("|done|"));
    }

    #[test]
    fn test_diagrama_pseudocodigo() {
        let mut d = Diagrama::new("Algo".into(), TipoDiagrama::Algoritmo);
        d.agregar_nodo(Nodo::new(TipoNodo::Inicio, "Begin".into(), 0.0, 0.0));
        d.agregar_nodo(Nodo::new(TipoNodo::Proceso, "Calc".into(), 0.0, 50.0));
        d.agregar_nodo(Nodo::new(TipoNodo::Fin, "Done".into(), 0.0, 100.0));

        let pseudo = d.exportar_pseudocodigo();
        assert!(pseudo.contains("ALGORITMO: Algo"));
        assert!(pseudo.contains("INICIO: Begin"));
        assert!(pseudo.contains("HACER: Calc"));
        assert!(pseudo.contains("FIN: Done"));
    }
}
