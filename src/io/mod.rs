//! # io — Capa común de export/import (Fase 5.1)
//!
//! Helpers unificados para que cada módulo (Tareas, Agenda, Memoria,
//! Rastreador, Bitácora, Presupuesto, Asesor) exporte y vuelva a importar
//! sus datos en formatos estándar **bilingües ES/EN**:
//!
//! - **CSV** (separador `,` con BOM UTF-8 para Excel; cabeceras `clave/key`)
//! - **Markdown** (legible humano)
//! - **JSON** (estructurado, ida/vuelta perfecta vía serde)
//! - **Excel `.xlsx`** (sólo `desktop`, vía `rust_xlsxwriter`)
//! - **SQL** (`CREATE TABLE` + `INSERT`)
//!
//! ## Convenciones
//!
//! - Todos los exports van a `<raíz>/exports/<modulo>/` (subcarpeta por
//!   módulo). El path raíz se obtiene de [`dir_exportacion`].
//! - Los nombres de archivo incluyen timestamp `YYYYMMDD_HHMMSS`.
//! - Los CSV usan cabeceras bilingües: `fecha/date`, `monto/amount`, etc.,
//!   para que cualquier sistema (en español o inglés) los reconozca.

pub mod excel;
pub mod parser;

#[cfg(not(target_arch = "wasm32"))]
use std::fs;
#[cfg(not(target_arch = "wasm32"))]
use std::io::Write;
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

// ════════════════════════════════════════════════════════════════════════
//  Paths
// ════════════════════════════════════════════════════════════════════════

/// Raíz de exportación: `<workspace>/exports/`. Mantiene compatibilidad con
/// `AlmacenAsesor::dir_exportacion()` que ya existía.
#[cfg(not(target_arch = "wasm32"))]
pub fn dir_exportacion() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("exports");
    let _ = fs::create_dir_all(&dir);
    dir
}

/// Subcarpeta por módulo: `<exports>/<modulo>/`. La crea si no existe.
#[cfg(not(target_arch = "wasm32"))]
pub fn dir_modulo(modulo: &str) -> PathBuf {
    let dir = dir_exportacion().join(modulo);
    let _ = fs::create_dir_all(&dir);
    dir
}

/// Genera un nombre de archivo con timestamp y extensión.
///
/// `nombre_archivo("tareas", "csv")` → `tareas_20260502_143012.csv`.
pub fn nombre_archivo(prefijo: &str, ext: &str) -> String {
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    format!("{}_{}.{}", prefijo, stamp, ext)
}

// ════════════════════════════════════════════════════════════════════════
//  Cabeceras bilingües
// ════════════════════════════════════════════════════════════════════════

/// Devuelve una cabecera bilingüe en formato `es/en`. Si las dos coinciden,
/// devuelve sólo una.
pub fn bil(es: &str, en: &str) -> String {
    if es.eq_ignore_ascii_case(en) {
        es.to_string()
    } else {
        format!("{}/{}", es, en)
    }
}

/// Normaliza una cabecera entrante (en español o inglés) a su clave canónica
/// en español. Útil para importar CSV que vengan con cabeceras en otro idioma.
///
/// Acepta tanto la forma combinada `es/en` como la simple.
pub fn normalizar_cabecera(h: &str) -> String {
    let limpio = h.trim().trim_start_matches('\u{FEFF}').to_lowercase();
    // Quedarse sólo con la parte "es" si viene combinada
    let primero = limpio.split('/').next().unwrap_or(&limpio).trim();
    // Mapear términos en inglés frecuentes a su forma en español
    let mapeo = [
        ("date", "fecha"),
        ("amount", "monto"),
        ("title", "titulo"),
        ("name", "nombre"),
        ("description", "descripcion"),
        ("priority", "prioridad"),
        ("status", "estado"),
        ("tags", "etiquetas"),
        ("notes", "notas"),
        ("module", "modulo"),
        ("type", "tipo"),
        ("counterparty", "contraparte"),
        ("debt", "deuda"),
        ("month", "mes"),
        ("payment", "pago"),
        ("balance", "saldo"),
        ("category", "categoria"),
        ("kind", "tipo"),
        ("created", "creado"),
        ("due", "vence"),
        ("time", "hora"),
        ("reminder", "recordatorio"),
        ("event", "evento"),
        ("task", "tarea"),
        ("memory", "recuerdo"),
        ("link", "enlace"),
        ("related", "relacionados"),
        ("attachments", "adjuntos"),
        ("references", "referencias"),
    ];
    for (en, es) in mapeo {
        if primero == en {
            return es.to_string();
        }
    }
    primero.to_string()
}

// ════════════════════════════════════════════════════════════════════════
//  CSV
// ════════════════════════════════════════════════════════════════════════

/// Escapa un campo para CSV (RFC 4180): rodea con comillas dobles si contiene
/// `,`, `"`, `\n` o `\r`, y duplica comillas internas.
pub fn escapar_csv(s: &str) -> String {
    let necesita = s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r');
    if necesita {
        let escapado = s.replace('"', "\"\"");
        format!("\"{}\"", escapado)
    } else {
        s.to_string()
    }
}

/// Escribe un CSV completo a la ruta indicada. Antepone BOM UTF-8 para que
/// Excel detecte la codificación correctamente.
#[cfg(not(target_arch = "wasm32"))]
pub fn escribir_csv(
    ruta: &Path,
    cabeceras: &[String],
    filas: &[Vec<String>],
) -> Result<(), String> {
    let mut f = fs::File::create(ruta).map_err(|e| format!("crear CSV: {}", e))?;
    // BOM
    f.write_all(b"\xEF\xBB\xBF")
        .map_err(|e| format!("BOM: {}", e))?;
    // Cabeceras
    let linea: Vec<String> = cabeceras.iter().map(|c| escapar_csv(c)).collect();
    writeln!(f, "{}", linea.join(",")).map_err(|e| format!("cabeceras: {}", e))?;
    // Filas
    for fila in filas {
        let linea: Vec<String> = fila.iter().map(|c| escapar_csv(c)).collect();
        writeln!(f, "{}", linea.join(",")).map_err(|e| format!("fila: {}", e))?;
    }
    Ok(())
}

/// Lee un CSV simple (RFC 4180 reducido) y devuelve cabeceras + filas.
///
/// Soporta:
/// - BOM UTF-8 al inicio.
/// - Campos entrecomillados con `"` y comillas dobladas (`""`).
/// - Saltos de línea dentro de campos entrecomillados.
/// - Separador coma `,`.
#[cfg(not(target_arch = "wasm32"))]
pub fn leer_csv(ruta: &Path) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let bytes = fs::read(ruta).map_err(|e| format!("leer CSV: {}", e))?;
    let texto = String::from_utf8_lossy(&bytes);
    parsear_csv(texto.trim_start_matches('\u{FEFF}'))
}

/// Parsea un texto CSV en memoria (mismo formato que [`leer_csv`]).
pub fn parsear_csv(texto: &str) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let mut filas: Vec<Vec<String>> = Vec::new();
    let mut campo = String::new();
    let mut fila: Vec<String> = Vec::new();
    let mut entre_comillas = false;
    let mut chars = texto.chars().peekable();
    while let Some(c) = chars.next() {
        if entre_comillas {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    campo.push('"');
                    chars.next();
                } else {
                    entre_comillas = false;
                }
            } else {
                campo.push(c);
            }
        } else {
            match c {
                '"' => entre_comillas = true,
                ',' => {
                    fila.push(std::mem::take(&mut campo));
                }
                '\r' => {}
                '\n' => {
                    fila.push(std::mem::take(&mut campo));
                    filas.push(std::mem::take(&mut fila));
                }
                _ => campo.push(c),
            }
        }
    }
    // Última línea sin newline final
    if !campo.is_empty() || !fila.is_empty() {
        fila.push(campo);
        filas.push(fila);
    }
    if filas.is_empty() {
        return Err("CSV vacío".to_string());
    }
    let cabeceras = filas.remove(0);
    Ok((cabeceras, filas))
}

// ════════════════════════════════════════════════════════════════════════
//  Markdown
// ════════════════════════════════════════════════════════════════════════

/// Escribe un Markdown con título, descripción opcional y una tabla.
#[cfg(not(target_arch = "wasm32"))]
pub fn escribir_markdown_tabla(
    ruta: &Path,
    titulo: &str,
    descripcion: Option<&str>,
    cabeceras: &[String],
    filas: &[Vec<String>],
) -> Result<(), String> {
    let mut f = fs::File::create(ruta).map_err(|e| format!("crear MD: {}", e))?;
    writeln!(f, "# {}", titulo).map_err(|e| format!("titulo: {}", e))?;
    if let Some(d) = descripcion {
        writeln!(f, "\n{}\n", d).map_err(|e| format!("desc: {}", e))?;
    }
    writeln!(
        f,
        "\n_Generado: {}_\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    )
    .map_err(|e| format!("ts: {}", e))?;
    if cabeceras.is_empty() {
        return Ok(());
    }
    let escapar = |s: &str| s.replace('|', "\\|").replace('\n', " ");
    writeln!(
        f,
        "| {} |",
        cabeceras
            .iter()
            .map(|c| escapar(c))
            .collect::<Vec<_>>()
            .join(" | ")
    )
    .map_err(|e| format!("cab: {}", e))?;
    writeln!(
        f,
        "|{}|",
        cabeceras
            .iter()
            .map(|_| "---")
            .collect::<Vec<_>>()
            .join("|")
    )
    .map_err(|e| format!("sep: {}", e))?;
    for fila in filas {
        writeln!(
            f,
            "| {} |",
            fila.iter()
                .map(|c| escapar(c))
                .collect::<Vec<_>>()
                .join(" | ")
        )
        .map_err(|e| format!("fila: {}", e))?;
    }
    Ok(())
}

// ════════════════════════════════════════════════════════════════════════
//  JSON
// ════════════════════════════════════════════════════════════════════════

/// Serializa cualquier `Serialize` a JSON con formato pretty.
#[cfg(not(target_arch = "wasm32"))]
pub fn escribir_json<T: serde::Serialize>(ruta: &Path, valor: &T) -> Result<(), String> {
    let txt = serde_json::to_string_pretty(valor).map_err(|e| format!("serializar JSON: {}", e))?;
    fs::write(ruta, txt).map_err(|e| format!("escribir JSON: {}", e))
}

/// Lee un JSON y lo deserializa a `T`.
#[cfg(not(target_arch = "wasm32"))]
pub fn leer_json<T: for<'de> serde::Deserialize<'de>>(ruta: &Path) -> Result<T, String> {
    let txt = fs::read_to_string(ruta).map_err(|e| format!("leer JSON: {}", e))?;
    serde_json::from_str(&txt).map_err(|e| format!("parse JSON: {}", e))
}

// ════════════════════════════════════════════════════════════════════════
//  SQL
// ════════════════════════════════════════════════════════════════════════

/// Escribe un script SQL con `CREATE TABLE IF NOT EXISTS` + `INSERT`s.
///
/// Las cabeceras se sanean a identificadores ASCII en minúsculas.
#[cfg(not(target_arch = "wasm32"))]
pub fn escribir_sql(
    ruta: &Path,
    tabla: &str,
    cabeceras: &[String],
    filas: &[Vec<String>],
) -> Result<(), String> {
    let mut f = fs::File::create(ruta).map_err(|e| format!("crear SQL: {}", e))?;
    let cols: Vec<String> = cabeceras.iter().map(|c| sanear_ident(c)).collect();
    let cols_def = cols
        .iter()
        .map(|c| format!("  {} TEXT", c))
        .collect::<Vec<_>>()
        .join(",\n");
    writeln!(
        f,
        "-- Generado por Omniplanner el {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    )
    .map_err(|e| format!("hdr: {}", e))?;
    writeln!(
        f,
        "CREATE TABLE IF NOT EXISTS {} (\n{}\n);\n",
        sanear_ident(tabla),
        cols_def
    )
    .map_err(|e| format!("create: {}", e))?;
    for fila in filas {
        let valores: Vec<String> = fila.iter().map(|v| escapar_sql(v)).collect();
        writeln!(
            f,
            "INSERT INTO {} ({}) VALUES ({});",
            sanear_ident(tabla),
            cols.join(", "),
            valores.join(", ")
        )
        .map_err(|e| format!("insert: {}", e))?;
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn sanear_ident(s: &str) -> String {
    // Convierte "fecha/date" → "fecha_date", elimina caracteres no válidos
    let s = s.trim().to_lowercase();
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if c == '_' || c == '/' || c == '-' || c == ' ' {
            out.push('_');
        }
        // Otros caracteres se omiten
    }
    if out.is_empty() {
        return "col".to_string();
    }
    if out
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
    {
        out.insert(0, '_');
    }
    out
}

#[cfg(not(target_arch = "wasm32"))]
fn escapar_sql(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

// ════════════════════════════════════════════════════════════════════════
//  Excel (.xlsx) — sólo en desktop (rust_xlsxwriter)
// ════════════════════════════════════════════════════════════════════════

/// Escribe un libro Excel con una sola hoja a partir de cabeceras y filas.
#[cfg(all(not(target_arch = "wasm32"), feature = "desktop"))]
pub fn escribir_xlsx(
    ruta: &Path,
    hoja: &str,
    cabeceras: &[String],
    filas: &[Vec<String>],
) -> Result<(), String> {
    use rust_xlsxwriter::{Format, Workbook};
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    let nombre_hoja = if hoja.is_empty() { "Datos" } else { hoja };
    ws.set_name(nombre_hoja)
        .map_err(|e| format!("hoja: {}", e))?;
    let bold = Format::new().set_bold();
    for (c, h) in cabeceras.iter().enumerate() {
        ws.write_string_with_format(0, c as u16, h, &bold)
            .map_err(|e| format!("cab: {}", e))?;
    }
    for (r, fila) in filas.iter().enumerate() {
        for (c, v) in fila.iter().enumerate() {
            // Intentar como número si parece numérico
            if let Ok(n) = v.parse::<f64>() {
                ws.write_number((r + 1) as u32, c as u16, n)
                    .map_err(|e| format!("num: {}", e))?;
            } else {
                ws.write_string((r + 1) as u32, c as u16, v)
                    .map_err(|e| format!("str: {}", e))?;
            }
        }
    }
    wb.save(ruta).map_err(|e| format!("guardar xlsx: {}", e))?;
    Ok(())
}

/// Lee la primera hoja de un libro `.xlsx`. Devuelve cabeceras y filas como
/// `String`. Sólo disponible con el feature `desktop` (vía `calamine`).
#[cfg(all(not(target_arch = "wasm32"), feature = "desktop"))]
pub fn leer_xlsx(ruta: &Path) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    use calamine::{open_workbook_auto, Data, Reader};
    let mut wb = open_workbook_auto(ruta).map_err(|e| format!("abrir xlsx: {}", e))?;
    let nombres = wb.sheet_names().to_vec();
    let primera = nombres
        .first()
        .ok_or_else(|| "xlsx sin hojas".to_string())?;
    let rango = wb
        .worksheet_range(primera)
        .map_err(|e| format!("rango: {}", e))?;
    let mut iter = rango.rows();
    let cabeceras: Vec<String> = iter
        .next()
        .map(|r| r.iter().map(celda_a_string).collect())
        .unwrap_or_default();
    let filas: Vec<Vec<String>> = iter
        .map(|r| r.iter().map(celda_a_string).collect())
        .collect();
    return Ok((cabeceras, filas));

    fn celda_a_string(c: &Data) -> String {
        match c {
            Data::Empty => String::new(),
            Data::String(s) => s.clone(),
            Data::Float(f) => {
                if f.fract() == 0.0 {
                    format!("{}", *f as i64)
                } else {
                    format!("{}", f)
                }
            }
            Data::Int(i) => i.to_string(),
            Data::Bool(b) => b.to_string(),
            Data::DateTime(dt) => dt.to_string(),
            Data::DateTimeIso(s) => s.clone(),
            Data::DurationIso(s) => s.clone(),
            Data::Error(e) => format!("#ERR:{:?}", e),
        }
    }
}

// ════════════════════════════════════════════════════════════════════════
//  Tests
// ════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapar_csv_basico() {
        assert_eq!(escapar_csv("hola"), "hola");
        assert_eq!(escapar_csv("a,b"), "\"a,b\"");
        assert_eq!(escapar_csv("a\"b"), "\"a\"\"b\"");
        assert_eq!(escapar_csv("a\nb"), "\"a\nb\"");
    }

    #[test]
    fn parsear_csv_basico() {
        let txt = "a,b,c\n1,2,3\n\"x,y\",\"\"\"q\"\"\",z\n";
        let (h, filas) = parsear_csv(txt).expect("parse");
        assert_eq!(h, vec!["a", "b", "c"]);
        assert_eq!(filas.len(), 2);
        assert_eq!(filas[0], vec!["1", "2", "3"]);
        assert_eq!(filas[1], vec!["x,y", "\"q\"", "z"]);
    }

    #[test]
    fn parsear_csv_con_bom() {
        let txt = "\u{FEFF}id,nombre\n1,Juan\n";
        let limpio = txt.trim_start_matches('\u{FEFF}');
        let (h, filas) = parsear_csv(limpio).expect("parse");
        assert_eq!(h, vec!["id", "nombre"]);
        assert_eq!(filas[0], vec!["1", "Juan"]);
    }

    #[test]
    fn cabecera_bilingue() {
        assert_eq!(bil("fecha", "date"), "fecha/date");
        assert_eq!(bil("id", "id"), "id");
        assert_eq!(bil("ID", "id"), "ID");
    }

    #[test]
    fn normalizar_cabeceras_es_y_en() {
        assert_eq!(normalizar_cabecera("fecha"), "fecha");
        assert_eq!(normalizar_cabecera("date"), "fecha");
        assert_eq!(normalizar_cabecera("Fecha/Date"), "fecha");
        assert_eq!(normalizar_cabecera("amount"), "monto");
        assert_eq!(normalizar_cabecera("\u{FEFF}title"), "titulo");
    }

    #[test]
    fn sanear_identificador_sql() {
        assert_eq!(sanear_ident("fecha/date"), "fecha_date");
        assert_eq!(sanear_ident("Monto $"), "monto_");
        assert_eq!(sanear_ident("123abc"), "_123abc");
        assert_eq!(sanear_ident(""), "col");
    }

    #[test]
    fn escapar_sql_basico() {
        assert_eq!(escapar_sql("hola"), "'hola'");
        assert_eq!(escapar_sql("o'clock"), "'o''clock'");
    }
}
