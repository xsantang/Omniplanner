//! Board visual de ideas — notas, imágenes, listas y secciones.
//!
//! [`Canvas`] actúa como un tablero donde se agregan [`Elemento`]s
//! de distintos tipos, con exportación a HTML.

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::fmt;

// ══════════════════════════════════════════════════════════════
//  Estructuras legacy (para compatibilidad con datos existentes)
// ══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Punto {
    pub x: f64,
    pub y: f64,
    pub presion: f64,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trazo {
    pub puntos: Vec<Punto>,
    pub color: String,
    pub grosor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextoReconocido {
    pub texto: String,
    pub confianza: f64,
    pub idioma: String,
}

// ══════════════════════════════════════════════════════════════
//  Nuevo sistema: Canvas como Board de Ideas
// ══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TipoElemento {
    Nota,    // Texto libre / idea
    Imagen,  // Referencia a archivo de imagen
    Lista,   // Lista de items
    Seccion, // Separador visual
}

impl fmt::Display for TipoElemento {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TipoElemento::Nota => write!(f, "📝 Nota"),
            TipoElemento::Imagen => write!(f, "🖼️  Imagen"),
            TipoElemento::Lista => write!(f, "📋 Lista"),
            TipoElemento::Seccion => write!(f, "── Sección"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Elemento {
    pub id: String,
    pub tipo: TipoElemento,
    pub contenido: String,
    pub color: String,
    pub creado: NaiveDateTime,
}

impl Elemento {
    pub fn nota(contenido: String, color: String) -> Self {
        Elemento {
            id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
            tipo: TipoElemento::Nota,
            contenido,
            color,
            creado: chrono::Local::now().naive_local(),
        }
    }

    pub fn imagen(ruta: String) -> Self {
        Elemento {
            id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
            tipo: TipoElemento::Imagen,
            contenido: ruta,
            color: String::new(),
            creado: chrono::Local::now().naive_local(),
        }
    }

    pub fn lista(contenido: String, color: String) -> Self {
        Elemento {
            id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
            tipo: TipoElemento::Lista,
            contenido,
            color,
            creado: chrono::Local::now().naive_local(),
        }
    }

    pub fn seccion(titulo: String) -> Self {
        Elemento {
            id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
            tipo: TipoElemento::Seccion,
            contenido: titulo,
            color: String::new(),
            creado: chrono::Local::now().naive_local(),
        }
    }
}

impl fmt::Display for Elemento {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.tipo {
            TipoElemento::Nota => write!(f, "📝 {}", self.contenido),
            TipoElemento::Imagen => write!(f, "🖼️  {}", self.contenido),
            TipoElemento::Lista => {
                let items: Vec<&str> = self.contenido.lines().collect();
                write!(f, "📋 {} items", items.len())
            }
            TipoElemento::Seccion => write!(f, "── {} ──", self.contenido),
        }
    }
}

/// Canvas: board de ideas con notas, imágenes y listas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Canvas {
    pub id: String,
    pub nombre: String,
    pub ancho: u32,
    pub alto: u32,
    // Legacy
    #[serde(default)]
    pub trazos: Vec<Trazo>,
    #[serde(default)]
    pub textos_reconocidos: Vec<TextoReconocido>,
    // Nuevo
    #[serde(default)]
    pub elementos: Vec<Elemento>,
}

impl Canvas {
    pub fn new(nombre: String, ancho: u32, alto: u32) -> Self {
        Canvas {
            id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
            nombre,
            ancho,
            alto,
            trazos: Vec::new(),
            textos_reconocidos: Vec::new(),
            elementos: Vec::new(),
        }
    }

    pub fn agregar_elemento(&mut self, elem: Elemento) {
        self.elementos.push(elem);
    }

    pub fn eliminar_elemento(&mut self, id: &str) -> bool {
        let antes = self.elementos.len();
        self.elementos.retain(|e| e.id != id);
        self.elementos.len() != antes
    }

    pub fn limpiar(&mut self) {
        self.elementos.clear();
        self.trazos.clear();
        self.textos_reconocidos.clear();
    }

    pub fn total_elementos(&self) -> usize {
        self.elementos.len() + self.trazos.len()
    }

    /// Exportar board completo a HTML (visual, puede abrirse en navegador)
    pub fn exportar_html(&self) -> String {
        let mut html = String::new();
        html.push_str("<!DOCTYPE html>\n<html lang=\"es\">\n<head>\n");
        html.push_str("<meta charset=\"UTF-8\">\n");
        html.push_str(&format!("<title>Canvas: {}</title>\n", self.nombre));
        html.push_str("<style>\n");
        html.push_str("  body { font-family: 'Segoe UI', sans-serif; background: #1a1a2e; color: #eee; padding: 20px; }\n");
        html.push_str(
            "  h1 { color: #00d4ff; border-bottom: 2px solid #00d4ff; padding-bottom: 10px; }\n",
        );
        html.push_str("  .board { display: flex; flex-wrap: wrap; gap: 16px; }\n");
        html.push_str("  .card { background: #16213e; border-radius: 12px; padding: 16px; min-width: 250px; max-width: 400px; box-shadow: 0 4px 12px #0003; }\n");
        html.push_str("  .card.nota { border-left: 4px solid #00d4ff; }\n");
        html.push_str("  .card.imagen { border-left: 4px solid #ff6b6b; }\n");
        html.push_str("  .card.lista { border-left: 4px solid #4ecdc4; }\n");
        html.push_str("  .card.seccion { background: none; border: none; width: 100%; font-size: 1.4em; color: #00d4ff; border-bottom: 1px solid #333; margin-top: 20px; }\n");
        html.push_str("  .card img { max-width: 100%; border-radius: 8px; }\n");
        html.push_str("  .card ul { padding-left: 20px; }\n");
        html.push_str("  .meta { font-size: 0.8em; color: #666; margin-top: 8px; }\n");
        html.push_str("</style>\n</head>\n<body>\n");
        html.push_str(&format!("<h1>🎨 {}</h1>\n", self.nombre));
        html.push_str("<div class=\"board\">\n");

        for elem in &self.elementos {
            match &elem.tipo {
                TipoElemento::Nota => {
                    let contenido_html = elem.contenido.replace('\n', "<br>");
                    html.push_str(&format!(
                        "<div class=\"card nota\"><p>{}</p><div class=\"meta\">{}</div></div>\n",
                        contenido_html,
                        elem.creado.format("%d/%m/%Y %H:%M")
                    ));
                }
                TipoElemento::Imagen => {
                    html.push_str(&format!(
                        "<div class=\"card imagen\"><img src=\"{}\" alt=\"imagen\"><div class=\"meta\">{}</div></div>\n",
                        elem.contenido,
                        elem.creado.format("%d/%m/%Y %H:%M")
                    ));
                }
                TipoElemento::Lista => {
                    let items: String = elem
                        .contenido
                        .lines()
                        .map(|l| format!("<li>{}</li>", l))
                        .collect();
                    html.push_str(&format!(
                        "<div class=\"card lista\"><ul>{}</ul><div class=\"meta\">{}</div></div>\n",
                        items,
                        elem.creado.format("%d/%m/%Y %H:%M")
                    ));
                }
                TipoElemento::Seccion => {
                    html.push_str(&format!(
                        "<div class=\"card seccion\">── {} ──</div>\n",
                        elem.contenido
                    ));
                }
            }
        }

        html.push_str("</div>\n</body>\n</html>");
        html
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canvas_crud() {
        let mut c = Canvas::new("Ideas".to_string(), 800, 600);
        assert_eq!(c.total_elementos(), 0);

        let nota = Elemento::nota("Hola mundo".into(), "#ff0".into());
        let id = nota.id.clone();
        c.agregar_elemento(nota);
        assert_eq!(c.total_elementos(), 1);

        assert!(c.eliminar_elemento(&id));
        assert_eq!(c.total_elementos(), 0);
        assert!(!c.eliminar_elemento("noexiste"));
    }

    #[test]
    fn test_canvas_tipos_elemento() {
        let nota = Elemento::nota("texto".into(), "red".into());
        assert!(matches!(nota.tipo, TipoElemento::Nota));

        let img = Elemento::imagen("/path/img.png".into());
        assert!(matches!(img.tipo, TipoElemento::Imagen));

        let lista = Elemento::lista("item1\nitem2\nitem3".into(), "blue".into());
        assert!(matches!(lista.tipo, TipoElemento::Lista));
        let display = format!("{}", lista);
        assert!(display.contains("3 items"));

        let sec = Elemento::seccion("Sección A".into());
        assert!(matches!(sec.tipo, TipoElemento::Seccion));
    }

    #[test]
    fn test_canvas_limpiar() {
        let mut c = Canvas::new("Test".into(), 100, 100);
        c.agregar_elemento(Elemento::nota("a".into(), "#fff".into()));
        c.agregar_elemento(Elemento::nota("b".into(), "#fff".into()));
        assert_eq!(c.total_elementos(), 2);
        c.limpiar();
        assert_eq!(c.total_elementos(), 0);
    }

    #[test]
    fn test_canvas_exportar_html() {
        let mut c = Canvas::new("Export".into(), 800, 600);
        c.agregar_elemento(Elemento::nota("Nota test".into(), "#0ff".into()));
        c.agregar_elemento(Elemento::seccion("Sec".into()));
        let html = c.exportar_html();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Nota test"));
        assert!(html.contains("Sec"));
    }
}
