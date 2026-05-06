//! # excel — Helpers unificados para Excel (lectura, escritura, edición, streaming)
//!
//! Expone funciones de conveniencia sobre las 4 librerías de Excel del proyecto:
//!
//! | Librería            | Rol                                               |
//! |---------------------|---------------------------------------------------|
//! | `calamine`          | **Lectura** de .xlsx/.xls/.ods existentes         |
//! | `rust_xlsxwriter`   | **Escritura** de .xlsx nuevos (alta fidelidad)    |
//! | `umya_spreadsheet`  | **Lectura + escritura + edición** de .xlsx        |
//! | `excelstream`       | **Streaming** de filas (CSV y .xlsx grandes)      |
//!
//! ## Cuándo usar qué
//!
//! - Crear un reporte nuevo desde cero → `rust_xlsxwriter` (ya usado en
//!   `exportar_simulacion_excel`).
//! - Leer un .xlsx que el usuario trajo → `calamine` (`leer_xlsx`).
//! - Modificar un .xlsx existente (añadir hoja, editar celda) → `umya_spreadsheet`
//!   (`abrir_xlsx`, `escribir_celda`, `guardar_xlsx`).
//! - Procesar filas de un CSV/xlsx grande sin cargar todo en memoria →
//!   `excelstream` (`iterar_filas_csv`).

#![cfg(feature = "desktop")]

// ── calamine ──────────────────────────────────────────────────────────────────

use calamine::{open_workbook, Reader, Xlsx};
use std::path::Path;

/// Lee todas las filas de la primera hoja de un .xlsx y las devuelve como
/// `Vec<Vec<String>>`. Las celdas vacías se convierten en `""`.
///
/// # Ejemplo
/// ```no_run
/// use omniplanner::io::excel::leer_xlsx;
/// let filas = leer_xlsx("reporte.xlsx").unwrap();
/// for fila in filas { println!("{:?}", fila); }
/// ```
pub fn leer_xlsx(ruta: impl AsRef<Path>) -> Result<Vec<Vec<String>>, String> {
    let ruta = ruta.as_ref();
    let mut wb: Xlsx<_> =
        open_workbook(ruta).map_err(|e| format!("calamine: no se pudo abrir {:?}: {}", ruta, e))?;

    let nombre_hoja = wb
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| "El archivo no tiene hojas".to_string())?;

    let rango = wb
        .worksheet_range(&nombre_hoja)
        .map_err(|e| format!("calamine: error al leer hoja '{}': {}", nombre_hoja, e))?;

    let mut filas: Vec<Vec<String>> = Vec::new();
    for fila in rango.rows() {
        let celdas = fila
            .iter()
            .map(|c| {
                use calamine::Data;
                match c {
                    Data::String(s) => s.clone(),
                    Data::Float(f) => format!("{}", f),
                    Data::Int(i) => format!("{}", i),
                    Data::Bool(b) => if *b { "true" } else { "false" }.to_string(),
                    Data::DateTime(_) => c.to_string(),
                    Data::DateTimeIso(s) => s.clone(),
                    _ => String::new(),
                }
            })
            .collect();
        filas.push(celdas);
    }
    Ok(filas)
}

/// Lee una hoja concreta por nombre de un .xlsx.
pub fn leer_hoja_xlsx(
    ruta: impl AsRef<Path>,
    nombre_hoja: &str,
) -> Result<Vec<Vec<String>>, String> {
    let ruta = ruta.as_ref();
    let mut wb: Xlsx<_> = open_workbook(ruta).map_err(|e| format!("calamine: {}", e))?;

    let rango = wb
        .worksheet_range(nombre_hoja)
        .map_err(|e| format!("calamine: hoja '{}' no encontrada: {}", nombre_hoja, e))?;

    let mut filas: Vec<Vec<String>> = Vec::new();
    for fila in rango.rows() {
        let celdas = fila
            .iter()
            .map(|c| {
                use calamine::Data;
                match c {
                    Data::String(s) => s.clone(),
                    Data::Float(f) => format!("{}", f),
                    Data::Int(i) => format!("{}", i),
                    _ => String::new(),
                }
            })
            .collect();
        filas.push(celdas);
    }
    Ok(filas)
}

/// Lista los nombres de todas las hojas de un .xlsx.
pub fn hojas_xlsx(ruta: impl AsRef<Path>) -> Result<Vec<String>, String> {
    let ruta = ruta.as_ref();
    let wb: Xlsx<_> = open_workbook(ruta).map_err(|e| format!("calamine: {}", e))?;
    Ok(wb.sheet_names().to_vec())
}

// ── umya_spreadsheet ──────────────────────────────────────────────────────────

use umya_spreadsheet::Spreadsheet;

/// Abre un .xlsx existente con umya para lectura/edición.
pub fn abrir_xlsx_umya(ruta: impl AsRef<Path>) -> Result<Spreadsheet, String> {
    let ruta = ruta.as_ref();
    umya_spreadsheet::reader::xlsx::read(ruta)
        .map_err(|e| format!("umya: no se pudo abrir {:?}: {}", ruta, e))
}

/// Escribe un valor de texto en una celda de una hoja (coordenadas 1-based).
/// `col` usa notación Excel: "A", "B", ... o número de columna (1-based → "A"=1).
///
/// # Ejemplo
/// ```no_run
/// use omniplanner::io::excel::{abrir_xlsx_umya, escribir_celda_umya, guardar_xlsx_umya};
/// let mut wb = abrir_xlsx_umya("reporte.xlsx").unwrap();
/// escribir_celda_umya(&mut wb, "Hoja1", "A", 1, "Hola");
/// guardar_xlsx_umya(&wb, "reporte_editado.xlsx").unwrap();
/// ```
pub fn escribir_celda_umya(wb: &mut Spreadsheet, hoja: &str, col: &str, fila: u32, valor: &str) {
    let coord = format!("{}{}", col, fila);
    if let Some(ws) = wb.get_sheet_by_name_mut(hoja) {
        ws.get_cell_mut(&*coord).set_value(valor);
    }
}

/// Escribe un número en una celda.
pub fn escribir_numero_umya(wb: &mut Spreadsheet, hoja: &str, col: &str, fila: u32, valor: f64) {
    let coord = format!("{}{}", col, fila);
    if let Some(ws) = wb.get_sheet_by_name_mut(hoja) {
        ws.get_cell_mut(&*coord).set_value_number(valor);
    }
}

/// Lee el valor de texto de una celda.
pub fn leer_celda_umya(wb: &Spreadsheet, hoja: &str, col: &str, fila: u32) -> String {
    let coord = format!("{}{}", col, fila);
    wb.get_sheet_by_name(hoja)
        .and_then(|ws| ws.get_cell(&*coord))
        .map(|c| c.get_value().to_string())
        .unwrap_or_default()
}

/// Agrega una hoja nueva al workbook.
pub fn agregar_hoja_umya(wb: &mut Spreadsheet, nombre: &str) {
    wb.new_sheet(nombre).ok();
}

/// Guarda el workbook en un archivo .xlsx.
pub fn guardar_xlsx_umya(wb: &Spreadsheet, ruta: impl AsRef<Path>) -> Result<(), String> {
    let ruta = ruta.as_ref();
    umya_spreadsheet::writer::xlsx::write(wb, ruta)
        .map_err(|e| format!("umya: no se pudo guardar {:?}: {}", ruta, e))
}

/// Crea un workbook umya nuevo en memoria (sin archivo base).
pub fn nuevo_xlsx_umya() -> Spreadsheet {
    umya_spreadsheet::new_file()
}

// ── rust_xlsxwriter — helpers de conveniencia ─────────────────────────────────

use rust_xlsxwriter::{Format, FormatAlign, FormatBorder, Workbook, Worksheet};

/// Formatos reutilizables para reportes.
pub struct FmtSet {
    pub titulo: Format,
    pub encabezado: Format,
    pub dinero: Format,
    pub dinero_verde: Format,
    pub dinero_rojo: Format,
    pub celda: Format,
    pub celda_izq: Format,
    pub resumen: Format,
}

impl FmtSet {
    pub fn nuevo() -> Self {
        Self {
            titulo: Format::new()
                .set_bold()
                .set_font_size(14.0)
                .set_align(FormatAlign::Center),
            encabezado: Format::new()
                .set_bold()
                .set_font_size(11.0)
                .set_border(FormatBorder::Thin)
                .set_background_color("4472C4")
                .set_font_color("FFFFFF")
                .set_align(FormatAlign::Center),
            dinero: Format::new()
                .set_num_format("$#,##0.00")
                .set_border(FormatBorder::Thin),
            dinero_verde: Format::new()
                .set_num_format("$#,##0.00")
                .set_border(FormatBorder::Thin)
                .set_font_color("008000"),
            dinero_rojo: Format::new()
                .set_num_format("$#,##0.00")
                .set_border(FormatBorder::Thin)
                .set_font_color("C00000"),
            celda: Format::new()
                .set_border(FormatBorder::Thin)
                .set_align(FormatAlign::Center),
            celda_izq: Format::new().set_border(FormatBorder::Thin),
            resumen: Format::new()
                .set_bold()
                .set_font_size(12.0)
                .set_background_color("D9E2F3"),
        }
    }
}

/// Escribe una fila de encabezados en la hoja, comenzando en (fila, col_ini).
/// Devuelve la siguiente fila disponible.
pub fn escribir_encabezados(
    ws: &mut Worksheet,
    fila: u32,
    col_ini: u16,
    headers: &[&str],
    fmt: &Format,
) -> Result<u32, String> {
    for (i, h) in headers.iter().enumerate() {
        ws.write_string_with_format(fila, col_ini + i as u16, *h, fmt)
            .map_err(|e| e.to_string())?;
    }
    Ok(fila + 1)
}

/// Escribe una fila de strings en la hoja.
pub fn escribir_fila_str(
    ws: &mut Worksheet,
    fila: u32,
    col_ini: u16,
    valores: &[&str],
    fmt: &Format,
) -> Result<u32, String> {
    for (i, v) in valores.iter().enumerate() {
        ws.write_string_with_format(fila, col_ini + i as u16, *v, fmt)
            .map_err(|e| e.to_string())?;
    }
    Ok(fila + 1)
}

/// Escribe una fila de f64 en la hoja con formato de dinero.
pub fn escribir_fila_dinero(
    ws: &mut Worksheet,
    fila: u32,
    col_ini: u16,
    valores: &[f64],
    fmt: &Format,
) -> Result<u32, String> {
    for (i, v) in valores.iter().enumerate() {
        ws.write_number_with_format(fila, col_ini + i as u16, *v, fmt)
            .map_err(|e| e.to_string())?;
    }
    Ok(fila + 1)
}

/// Guarda el workbook en la ruta indicada.
pub fn guardar_xlsx(wb: &mut Workbook, ruta: impl AsRef<Path>) -> Result<(), String> {
    wb.save(ruta.as_ref()).map_err(|e| e.to_string())
}

// ── excelstream — iteración de filas CSV/xlsx ─────────────────────────────────

/// Itera las filas de un archivo CSV y llama al callback para cada una.
/// La primera fila (encabezados) se entrega igual que las demás.
///
/// # Ejemplo
/// ```no_run
/// use omniplanner::io::excel::iterar_filas_csv;
/// iterar_filas_csv("datos.csv", |fila| {
///     println!("{:?}", fila);
/// }).unwrap();
/// ```
pub fn iterar_filas_csv(
    ruta: impl AsRef<Path>,
    mut callback: impl FnMut(Vec<String>),
) -> Result<(), String> {
    use std::io::{BufRead, BufReader};
    let ruta = ruta.as_ref();
    let f = std::fs::File::open(ruta)
        .map_err(|e| format!("iterar_filas_csv: no se pudo abrir {:?}: {}", ruta, e))?;
    let reader = BufReader::new(f);
    for linea in reader.lines() {
        let linea = linea.map_err(|e| format!("iterar_filas_csv: error de lectura: {}", e))?;
        // Separar por coma (sin manejar comas dentro de comillas — para uso simple)
        let campos: Vec<String> = linea.split(',').map(|s| s.trim().to_string()).collect();
        callback(campos);
    }
    Ok(())
}

/// Convierte un Vec<Vec<String>> a CSV y lo escribe en disco con BOM UTF-8
/// para que Excel lo abra correctamente en español.
pub fn escribir_csv(ruta: impl AsRef<Path>, filas: &[Vec<String>]) -> Result<(), String> {
    use std::io::Write;
    let ruta = ruta.as_ref();
    let mut f = std::fs::File::create(ruta).map_err(|e| format!("escribir_csv: {}", e))?;
    // BOM UTF-8
    f.write_all(b"\xEF\xBB\xBF").map_err(|e| e.to_string())?;
    for fila in filas {
        let linea = fila
            .iter()
            .map(|c| {
                if c.contains(',') || c.contains('"') {
                    format!("\"{}\"", c.replace('"', "\"\""))
                } else {
                    c.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(",");
        writeln!(f, "{}", linea).map_err(|e| e.to_string())?;
    }
    Ok(())
}
