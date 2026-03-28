use serde::{Deserialize, Serialize};
use std::fmt;

/// Punto en el canvas (escritura a mano / dibujo)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Punto {
    pub x: f64,
    pub y: f64,
    pub presion: f64,   // 0.0 - 1.0, para punteros con sensibilidad
    pub timestamp_ms: u64,
}

/// Un trazo completo (desde que el puntero toca hasta que se levanta)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trazo {
    pub puntos: Vec<Punto>,
    pub color: String,
    pub grosor: f64,
}

impl Trazo {
    pub fn new(color: String, grosor: f64) -> Self {
        Trazo {
            puntos: Vec::new(),
            color,
            grosor,
        }
    }

    pub fn agregar_punto(&mut self, punto: Punto) {
        self.puntos.push(punto);
    }

    /// Bounding box del trazo: (min_x, min_y, max_x, max_y)
    pub fn bounding_box(&self) -> Option<(f64, f64, f64, f64)> {
        if self.puntos.is_empty() {
            return None;
        }
        let min_x = self.puntos.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
        let min_y = self.puntos.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
        let max_x = self.puntos.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
        let max_y = self.puntos.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
        Some((min_x, min_y, max_x, max_y))
    }

    /// Longitud total del trazo
    pub fn longitud(&self) -> f64 {
        self.puntos.windows(2).map(|w| {
            let dx = w[1].x - w[0].x;
            let dy = w[1].y - w[0].y;
            (dx * dx + dy * dy).sqrt()
        }).sum()
    }
}

/// Resultado de reconocimiento de escritura a mano
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextoReconocido {
    pub texto: String,
    pub confianza: f64, // 0.0 - 1.0
    pub idioma: String,
}

impl fmt::Display for TextoReconocido {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\"{}\" (confianza: {:.0}%, idioma: {})", self.texto, self.confianza * 100.0, self.idioma)
    }
}

/// Canvas completo con sus trazos
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Canvas {
    pub id: String,
    pub nombre: String,
    pub ancho: u32,
    pub alto: u32,
    pub trazos: Vec<Trazo>,
    pub textos_reconocidos: Vec<TextoReconocido>,
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
        }
    }

    pub fn agregar_trazo(&mut self, trazo: Trazo) {
        self.trazos.push(trazo);
    }

    pub fn limpiar(&mut self) {
        self.trazos.clear();
        self.textos_reconocidos.clear();
    }

    /// Reconocimiento simplificado de escritura basado en análisis de trazos
    /// En producción esto se conectaría a un motor OCR/HWR
    pub fn reconocer_escritura(&mut self) -> Vec<TextoReconocido> {
        // Análisis básico de patrones de trazos
        let mut resultados = Vec::new();

        for (i, trazo) in self.trazos.iter().enumerate() {
            if trazo.puntos.len() < 3 {
                continue;
            }

            let patron = analizar_patron(trazo);
            let texto = TextoReconocido {
                texto: format!("[trazo-{}: {}]", i, patron),
                confianza: 0.5,
                idioma: "es".to_string(),
            };
            resultados.push(texto);
        }

        self.textos_reconocidos = resultados.clone();
        resultados
    }

    /// Exportar trazos a formato SVG
    pub fn exportar_svg(&self) -> String {
        let mut svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">"#,
            self.ancho, self.alto, self.ancho, self.alto
        );
        svg.push('\n');

        for trazo in &self.trazos {
            if trazo.puntos.is_empty() {
                continue;
            }
            let mut path = format!(
                r#"  <path d="M {:.1} {:.1}"#,
                trazo.puntos[0].x, trazo.puntos[0].y
            );
            for p in &trazo.puntos[1..] {
                path.push_str(&format!(" L {:.1} {:.1}", p.x, p.y));
            }
            path.push_str(&format!(
                r#"" stroke="{}" stroke-width="{}" fill="none" stroke-linecap="round"/>"#,
                trazo.color, trazo.grosor
            ));
            svg.push_str(&path);
            svg.push('\n');
        }

        svg.push_str("</svg>");
        svg
    }
}

/// Clasificación básica de patrones de un trazo
fn analizar_patron(trazo: &Trazo) -> &'static str {
    if trazo.puntos.len() < 2 {
        return "punto";
    }

    let bb = trazo.bounding_box().unwrap();
    let ancho = bb.2 - bb.0;
    let alto = bb.3 - bb.1;
    let ratio = if alto > 0.001 { ancho / alto } else { 100.0 };
    let longitud = trazo.longitud();
    let diagonal = (ancho * ancho + alto * alto).sqrt();

    if longitud < 5.0 {
        "punto"
    } else if ratio > 3.0 {
        "linea-horizontal"
    } else if ratio < 0.33 {
        "linea-vertical"
    } else if diagonal > 0.001 && longitud / diagonal > 3.0 {
        "curva-cerrada"
    } else {
        "trazo-libre"
    }
}
