use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

/// Formato de codificación
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Codificacion {
    Base64,
    Hex,
    Binario,
    Utf8,
    Json,
    Csv,
}

impl fmt::Display for Codificacion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Codificacion::Base64 => write!(f, "Base64"),
            Codificacion::Hex => write!(f, "Hexadecimal"),
            Codificacion::Binario => write!(f, "Binario"),
            Codificacion::Utf8 => write!(f, "UTF-8"),
            Codificacion::Json => write!(f, "JSON"),
            Codificacion::Csv => write!(f, "CSV"),
        }
    }
}

/// Regla de mapeo entre campos
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReglaMapa {
    pub campo_origen: String,
    pub campo_destino: String,
    pub transformacion: Option<String>, // expresión de transformación
}

/// Esquema de mapeo completo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EsquemaMapa {
    pub id: String,
    pub nombre: String,
    pub reglas: Vec<ReglaMapa>,
    pub codificacion_entrada: Codificacion,
    pub codificacion_salida: Codificacion,
}

impl EsquemaMapa {
    pub fn new(nombre: String, cod_entrada: Codificacion, cod_salida: Codificacion) -> Self {
        EsquemaMapa {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            nombre,
            reglas: Vec::new(),
            codificacion_entrada: cod_entrada,
            codificacion_salida: cod_salida,
        }
    }

    pub fn agregar_regla(&mut self, origen: String, destino: String, transformacion: Option<String>) {
        self.reglas.push(ReglaMapa {
            campo_origen: origen,
            campo_destino: destino,
            transformacion,
        });
    }
}

impl fmt::Display for EsquemaMapa {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} | {} -> {} | {} reglas",
            self.id,
            self.nombre,
            self.codificacion_entrada,
            self.codificacion_salida,
            self.reglas.len()
        )
    }
}

/// Motor de mapeo y codificación
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Mapper {
    pub esquemas: Vec<EsquemaMapa>,
}

impl Mapper {
    pub fn new() -> Self {
        Mapper {
            esquemas: Vec::new(),
        }
    }

    pub fn agregar_esquema(&mut self, esquema: EsquemaMapa) {
        self.esquemas.push(esquema);
    }

    /// Aplicar mapeo a datos JSON
    pub fn aplicar_mapeo(
        &self,
        esquema_id: &str,
        datos: &HashMap<String, String>,
    ) -> Option<HashMap<String, String>> {
        let esquema = self.esquemas.iter().find(|e| e.id == esquema_id)?;
        let mut resultado = HashMap::new();

        for regla in &esquema.reglas {
            if let Some(valor) = datos.get(&regla.campo_origen) {
                let valor_transformado = match &regla.transformacion {
                    Some(t) => aplicar_transformacion(valor, t),
                    None => valor.clone(),
                };
                resultado.insert(regla.campo_destino.clone(), valor_transformado);
            }
        }

        Some(resultado)
    }

    /// Codificar string
    pub fn codificar(datos: &str, formato: &Codificacion) -> String {
        match formato {
            Codificacion::Base64 => {
                use std::io::Write;
                let mut buf = Vec::new();
                // Codificación base64 manual simple
                let tabla = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
                let bytes = datos.as_bytes();
                let mut i = 0;
                while i < bytes.len() {
                    let b0 = bytes[i] as u32;
                    let b1 = if i + 1 < bytes.len() { bytes[i + 1] as u32 } else { 0 };
                    let b2 = if i + 2 < bytes.len() { bytes[i + 2] as u32 } else { 0 };
                    let triple = (b0 << 16) | (b1 << 8) | b2;

                    let _ = buf.write_all(&[tabla[((triple >> 18) & 0x3F) as usize]]);
                    let _ = buf.write_all(&[tabla[((triple >> 12) & 0x3F) as usize]]);
                    if i + 1 < bytes.len() {
                        let _ = buf.write_all(&[tabla[((triple >> 6) & 0x3F) as usize]]);
                    } else {
                        let _ = buf.write_all(b"=");
                    }
                    if i + 2 < bytes.len() {
                        let _ = buf.write_all(&[tabla[(triple & 0x3F) as usize]]);
                    } else {
                        let _ = buf.write_all(b"=");
                    }
                    i += 3;
                }
                String::from_utf8(buf).unwrap_or_default()
            }
            Codificacion::Hex => {
                datos.as_bytes().iter().map(|b| format!("{:02x}", b)).collect()
            }
            Codificacion::Binario => {
                datos.as_bytes().iter().map(|b| format!("{:08b}", b)).collect::<Vec<_>>().join(" ")
            }
            _ => datos.to_string(),
        }
    }

    /// Decodificar string desde hex
    pub fn decodificar_hex(hex: &str) -> Option<String> {
        let bytes: Result<Vec<u8>, _> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
            .collect();
        bytes.ok().and_then(|b| String::from_utf8(b).ok())
    }
}

fn aplicar_transformacion(valor: &str, transformacion: &str) -> String {
    match transformacion {
        "uppercase" => valor.to_uppercase(),
        "lowercase" => valor.to_lowercase(),
        "trim" => valor.trim().to_string(),
        "reverse" => valor.chars().rev().collect(),
        t if t.starts_with("prefix:") => format!("{}{}", &t[7..], valor),
        t if t.starts_with("suffix:") => format!("{}{}", valor, &t[7..]),
        _ => valor.to_string(),
    }
}
