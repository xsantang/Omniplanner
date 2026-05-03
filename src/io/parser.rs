//! # parser — Texto libre → datos estructurados (Fase 5.4)
//!
//! Convierte un bloque de texto (en español o inglés) en una colección de
//! [`ItemDetectado`] (tareas, eventos, pagos o notas), aplicando heurísticas
//! ligeras y reglas tolerantes:
//!
//! ## Líneas estructuradas (preferente)
//!
//! ```text
//! - comprar leche | 2026-05-10 | alta
//! * meeting with Ana | 2026-05-12 14:30 | reunion
//! payment to Visa | 2026-05-20 | $250 | nota: tasa 18%
//! ```
//!
//! Separador `|` (pipe). El primer campo es el título; el resto se
//! interpreta libremente: si parece fecha → fecha; si parece monto → monto;
//! si encaja con `alta/baja/urgente`/`high/low/urgent` → prioridad; etc.
//!
//! ## Líneas libres
//!
//! Cada línea no estructurada se intenta clasificar por palabra clave:
//! `reunión/meeting/cita/appointment` → Evento, `pagar/pay/factura/bill` →
//! Pago, `comprar/buy/llamar/call/enviar/send` → Tarea. Si no hay pista,
//! cae en `Nota`.
//!
//! Las fechas reconocidas: `YYYY-MM-DD`, `DD/MM/YYYY`, `MM/DD/YYYY`,
//! `YYYY/MM/DD`, `15 de mayo de 2026`, `May 15, 2026`, `mañana/tomorrow`,
//! `hoy/today`, `pasado mañana`, `próxima semana/next week`.

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveTime};

// ════════════════════════════════════════════════════════════════════════
//  Tipos públicos
// ════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub enum CategoriaItem {
    Tarea,
    Evento,
    Pago,
    Nota,
}

#[derive(Debug, Clone)]
pub struct ItemDetectado {
    pub categoria: CategoriaItem,
    pub titulo: String,
    pub fecha: Option<NaiveDate>,
    pub hora: Option<NaiveTime>,
    pub monto: Option<f64>,
    pub prioridad: Option<String>,
    pub etiquetas: Vec<String>,
    pub notas: Vec<String>,
    /// Línea original de la que se extrajo el ítem.
    pub linea_origen: String,
}

// ════════════════════════════════════════════════════════════════════════
//  API pública
// ════════════════════════════════════════════════════════════════════════

/// Parsea un bloque de texto. Cada línea no vacía produce un `ItemDetectado`.
pub fn parsear_texto(texto: &str) -> Vec<ItemDetectado> {
    texto
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !es_comentario(l))
        .map(parsear_linea)
        .collect()
}

fn es_comentario(s: &str) -> bool {
    s.starts_with('#') || s.starts_with("//")
}

// ════════════════════════════════════════════════════════════════════════
//  Núcleo: una línea → ItemDetectado
// ════════════════════════════════════════════════════════════════════════

fn parsear_linea(linea: &str) -> ItemDetectado {
    let original = linea.to_string();
    // Quitar viñetas: -, *, •, números
    let cuerpo = quitar_vinetas(linea).trim().to_string();

    // Si tiene `|` lo tratamos como estructurado
    let tiene_pipe = cuerpo.contains('|');
    let partes: Vec<String> = if tiene_pipe {
        cuerpo.split('|').map(|s| s.trim().to_string()).collect()
    } else {
        vec![cuerpo.clone()]
    };

    let mut titulo = partes.first().cloned().unwrap_or_default();
    let mut fecha: Option<NaiveDate> = None;
    let mut hora: Option<NaiveTime> = None;
    let mut monto: Option<f64> = None;
    let mut prioridad: Option<String> = None;
    let mut etiquetas: Vec<String> = Vec::new();
    let mut notas: Vec<String> = Vec::new();

    // Procesar resto de campos (si hay pipes)
    for parte in partes.iter().skip(1) {
        clasificar_campo(
            parte,
            &mut fecha,
            &mut hora,
            &mut monto,
            &mut prioridad,
            &mut etiquetas,
            &mut notas,
        );
    }

    // Sobre el título, también buscar inline fecha/hora/monto si NO había pipes
    if !tiene_pipe {
        if let Some((sin_fecha, f, h)) = extraer_fecha_y_hora_inline(&titulo) {
            titulo = sin_fecha;
            if fecha.is_none() {
                fecha = Some(f);
            }
            if hora.is_none() && h.is_some() {
                hora = h;
            }
        }
        if let Some((sin_monto, m)) = extraer_monto_inline(&titulo) {
            titulo = sin_monto;
            if monto.is_none() {
                monto = Some(m);
            }
        }
    }

    let titulo = titulo.trim().to_string();
    let categoria = clasificar_categoria(&titulo, monto.is_some());

    ItemDetectado {
        categoria,
        titulo,
        fecha,
        hora,
        monto,
        prioridad,
        etiquetas,
        notas,
        linea_origen: original,
    }
}

fn quitar_vinetas(s: &str) -> String {
    let trimmed = s.trim_start();
    // Soporta -, *, •, ·, números seguidos de . o )
    let bytes = trimmed.as_bytes();
    let mut idx = 0;
    if !bytes.is_empty() {
        let primer = trimmed.chars().next().unwrap();
        if matches!(primer, '-' | '*' | '•' | '·' | '–' | '—') {
            idx = primer.len_utf8();
        } else if primer.is_ascii_digit() {
            // Buscar dígitos seguidos de . o )
            let mut i = 0;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i < bytes.len() && (bytes[i] == b'.' || bytes[i] == b')') {
                idx = i + 1;
            }
        }
    }
    trimmed[idx..].trim_start().to_string()
}

fn clasificar_campo(
    valor: &str,
    fecha: &mut Option<NaiveDate>,
    hora: &mut Option<NaiveTime>,
    monto: &mut Option<f64>,
    prioridad: &mut Option<String>,
    etiquetas: &mut Vec<String>,
    notas: &mut Vec<String>,
) {
    let v = valor.trim();
    if v.is_empty() {
        return;
    }
    let lower = v.to_lowercase();

    // Prefijo explícito: "nota:", "note:", "tag:", "etiqueta:"
    if let Some(rest) = quitar_prefijo(&lower, &["nota:", "note:", "notas:", "notes:"]) {
        let r = v[v.len() - rest.len()..].trim().to_string();
        notas.push(r);
        return;
    }
    if let Some(rest) = quitar_prefijo(&lower, &["tag:", "tags:", "etiqueta:", "etiquetas:"]) {
        let r = v[v.len() - rest.len()..].trim();
        for t in r.split([',', ';', '#']) {
            let t = t.trim();
            if !t.is_empty() {
                etiquetas.push(t.to_string());
            }
        }
        return;
    }
    if let Some(rest) = quitar_prefijo(&lower, &["prio:", "priority:", "prioridad:"]) {
        let r = v[v.len() - rest.len()..].trim().to_string();
        *prioridad = Some(r);
        return;
    }

    // Prioridad libre
    if matches!(
        lower.as_str(),
        "alta" | "baja" | "media" | "urgente" | "high" | "low" | "medium" | "urgent"
    ) {
        *prioridad = Some(v.to_string());
        return;
    }

    // Etiqueta tipo #algo
    if let Some(t) = lower.strip_prefix('#') {
        etiquetas.push(t.to_string());
        return;
    }

    // Monto
    if let Some(m) = parsear_monto(v) {
        *monto = Some(m);
        return;
    }

    // Fecha (acepta también "fecha + hora")
    if let Some((f, h)) = parsear_fecha_y_hora(v) {
        if fecha.is_none() {
            *fecha = Some(f);
        }
        if hora.is_none() && h.is_some() {
            *hora = h;
        }
        return;
    }

    // Sólo hora
    if let Some(h) = parsear_hora(v) {
        if hora.is_none() {
            *hora = Some(h);
        }
        return;
    }

    // Si nada coincide, lo guardamos como nota libre
    notas.push(v.to_string());
}

fn quitar_prefijo<'a>(s: &'a str, prefs: &[&str]) -> Option<&'a str> {
    for p in prefs {
        if let Some(r) = s.strip_prefix(p) {
            return Some(r);
        }
    }
    None
}

// ════════════════════════════════════════════════════════════════════════
//  Categoría por palabra clave (bilingüe)
// ════════════════════════════════════════════════════════════════════════

fn clasificar_categoria(titulo: &str, tiene_monto: bool) -> CategoriaItem {
    let t = titulo.to_lowercase();
    if tiene_monto || contiene_palabra(&t, KW_PAGO) {
        return CategoriaItem::Pago;
    }
    if contiene_palabra(&t, KW_EVENTO) {
        return CategoriaItem::Evento;
    }
    if contiene_palabra(&t, KW_TAREA) {
        return CategoriaItem::Tarea;
    }
    CategoriaItem::Nota
}

fn contiene_palabra(t: &str, palabras: &[&str]) -> bool {
    palabras.iter().any(|p| {
        // Match por palabra completa: rodeada de no-letra o inicio/fin
        if let Some(pos) = t.find(p) {
            let antes = if pos == 0 {
                None
            } else {
                t[..pos].chars().next_back()
            };
            let despues = t[pos + p.len()..].chars().next();
            let limite = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric());
            limite(antes) && limite(despues)
        } else {
            false
        }
    })
}

const KW_TAREA: &[&str] = &[
    "tarea", "task", "comprar", "buy", "llamar", "call", "enviar", "send", "escribir", "write",
    "preparar", "prepare", "revisar", "review", "leer", "read", "estudiar", "study", "to-do",
    "todo",
];
const KW_EVENTO: &[&str] = &[
    "reunion",
    "reunión",
    "meeting",
    "cita",
    "appointment",
    "evento",
    "event",
    "cumpleaños",
    "cumpleanos",
    "birthday",
    "almuerzo",
    "lunch",
    "cena",
    "dinner",
    "llamada",
    "videollamada",
    "videocall",
    "entrevista",
    "interview",
];
const KW_PAGO: &[&str] = &[
    "pago",
    "pagar",
    "payment",
    "pay",
    "factura",
    "bill",
    "invoice",
    "renta",
    "rent",
    "hipoteca",
    "mortgage",
    "tarjeta",
    "credit",
    "transferencia",
    "transfer",
    "cobro",
    "charge",
];

// ════════════════════════════════════════════════════════════════════════
//  Fechas (bilingüe)
// ════════════════════════════════════════════════════════════════════════

const MESES_ES: &[(&str, u32)] = &[
    ("enero", 1),
    ("febrero", 2),
    ("marzo", 3),
    ("abril", 4),
    ("mayo", 5),
    ("junio", 6),
    ("julio", 7),
    ("agosto", 8),
    ("septiembre", 9),
    ("setiembre", 9),
    ("octubre", 10),
    ("noviembre", 11),
    ("diciembre", 12),
];
const MESES_EN: &[(&str, u32)] = &[
    ("january", 1),
    ("february", 2),
    ("march", 3),
    ("april", 4),
    ("may", 5),
    ("june", 6),
    ("july", 7),
    ("august", 8),
    ("september", 9),
    ("october", 10),
    ("november", 11),
    ("december", 12),
];

/// Acepta `s = "2026-05-10"`, `"2026-05-10 14:30"`, `"15 de mayo 2026"`, etc.
pub fn parsear_fecha_y_hora(s: &str) -> Option<(NaiveDate, Option<NaiveTime>)> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // Probar formatos con hora
    for fmt in &[
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%d/%m/%Y %H:%M",
        "%m/%d/%Y %H:%M",
    ] {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
            return Some((dt.date(), Some(dt.time())));
        }
    }
    // Sólo fecha
    if let Some(f) = parsear_fecha(s) {
        return Some((f, None));
    }
    // Patrón "<fecha> <hora>"
    if let Some((izq, der)) = s.rsplit_once(' ') {
        if let (Some(f), Some(h)) = (parsear_fecha(izq), parsear_hora(der)) {
            return Some((f, Some(h)));
        }
    }
    None
}

pub fn parsear_fecha(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    let lower = s.to_lowercase();
    let hoy = Local::now().date_naive();

    // Atajos relativos
    match lower.as_str() {
        "hoy" | "today" => return Some(hoy),
        "mañana" | "manana" | "tomorrow" => return Some(hoy + Duration::days(1)),
        "pasado mañana" | "pasado manana" | "day after tomorrow" => {
            return Some(hoy + Duration::days(2))
        }
        "ayer" | "yesterday" => return Some(hoy - Duration::days(1)),
        "próxima semana" | "proxima semana" | "next week" => return Some(hoy + Duration::days(7)),
        "próximo mes" | "proximo mes" | "next month" => return Some(hoy + Duration::days(30)),
        _ => {}
    }

    // Formatos numéricos
    for fmt in &[
        "%Y-%m-%d", "%Y/%m/%d", "%d/%m/%Y", "%m/%d/%Y", "%d-%m-%Y", "%d.%m.%Y",
    ] {
        if let Ok(d) = NaiveDate::parse_from_str(s, fmt) {
            return Some(d);
        }
    }

    // "15 de mayo de 2026" / "15 mayo 2026"
    let toks: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect();
    if toks.len() >= 2 {
        // ES: día mes [año]
        if let (Ok(dia), Some(mes)) = (toks[0].parse::<u32>(), buscar_mes(&toks)) {
            let anio = extraer_anio(&toks).unwrap_or(hoy.year() as u32);
            if let Some(f) = NaiveDate::from_ymd_opt(anio as i32, mes, dia) {
                return Some(f);
            }
        }
        // EN: month day [year]
        if let Some(mes) = buscar_mes(&toks) {
            for t in &toks {
                if let Ok(dia) = t.parse::<u32>() {
                    if dia <= 31 {
                        let anio = extraer_anio(&toks).unwrap_or(hoy.year() as u32);
                        if let Some(f) = NaiveDate::from_ymd_opt(anio as i32, mes, dia) {
                            return Some(f);
                        }
                    }
                }
            }
        }
    }
    None
}

fn buscar_mes(toks: &[&str]) -> Option<u32> {
    for t in toks {
        for (n, m) in MESES_ES.iter().chain(MESES_EN.iter()) {
            if t == n {
                return Some(*m);
            }
        }
    }
    None
}

fn extraer_anio(toks: &[&str]) -> Option<u32> {
    for t in toks {
        if let Ok(n) = t.parse::<u32>() {
            if (1900..=2200).contains(&n) {
                return Some(n);
            }
        }
    }
    None
}

pub fn parsear_hora(s: &str) -> Option<NaiveTime> {
    let s = s.trim();
    let lower = s.to_lowercase();
    // "14:30", "14:30:00", "9:00"
    for fmt in &["%H:%M:%S", "%H:%M"] {
        if let Ok(h) = NaiveTime::parse_from_str(s, fmt) {
            return Some(h);
        }
    }
    // "6pm", "6 pm", "6:30pm", "11am"
    let limpio = lower.replace(' ', "");
    let (hh, ampm) = if let Some(stripped) = limpio.strip_suffix("pm") {
        (stripped, true)
    } else if let Some(stripped) = limpio.strip_suffix("am") {
        (stripped, false)
    } else {
        return None;
    };
    let (hora, min) = if let Some((h, m)) = hh.split_once(':') {
        (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?)
    } else {
        (hh.parse::<u32>().ok()?, 0)
    };
    let h24 = match (hora, ampm) {
        (12, false) => 0,
        (12, true) => 12,
        (h, true) => h + 12,
        (h, false) => h,
    };
    NaiveTime::from_hms_opt(h24, min, 0)
}

// Inline: dentro del título encontrar fecha y hora; devolver título sin
// esos tokens si los reconocimos.
fn extraer_fecha_y_hora_inline(s: &str) -> Option<(String, NaiveDate, Option<NaiveTime>)> {
    // Estrategia barata: probar cada token y combinaciones de 2-3 tokens.
    let palabras: Vec<&str> = s.split_whitespace().collect();
    let n = palabras.len();
    // Buscar ventanas de tamaño 1..=4
    for tam in (1..=4).rev() {
        if tam > n {
            continue;
        }
        for i in 0..=n - tam {
            let frag = palabras[i..i + tam].join(" ");
            if let Some((f, h)) = parsear_fecha_y_hora(&frag) {
                let mut nuevas: Vec<&str> = Vec::new();
                nuevas.extend_from_slice(&palabras[..i]);
                nuevas.extend_from_slice(&palabras[i + tam..]);
                return Some((nuevas.join(" "), f, h));
            }
        }
    }
    None
}

// ════════════════════════════════════════════════════════════════════════
//  Montos (bilingüe)
// ════════════════════════════════════════════════════════════════════════

pub fn parsear_monto(s: &str) -> Option<f64> {
    let s = s.trim();
    // Empezar con símbolo $/€/£ o terminar con USD/EUR/MXN
    let limpio = s
        .trim_start_matches(['$', '€', '£', '¥'])
        .trim_end_matches(|c: char| c.is_alphabetic())
        .trim()
        .replace([',', ' '], "");
    let n: f64 = limpio.parse().ok()?;
    // Si el original NO tenía marca monetaria evidente, exigir que parezca dinero:
    // o empieza por símbolo, o el sufijo es una divisa.
    let tiene_simbolo = s.starts_with(['$', '€', '£', '¥']);
    let tiene_divisa = [
        "USD", "EUR", "MXN", "GBP", "JPY", "ARS", "CLP", "COP", "PEN",
    ]
    .iter()
    .any(|d| s.to_uppercase().ends_with(d));
    if tiene_simbolo || tiene_divisa {
        Some(n)
    } else {
        None
    }
}

fn extraer_monto_inline(s: &str) -> Option<(String, f64)> {
    let palabras: Vec<&str> = s.split_whitespace().collect();
    for (i, w) in palabras.iter().enumerate() {
        if let Some(m) = parsear_monto(w) {
            let mut nuevas = palabras.clone();
            nuevas.remove(i);
            return Some((nuevas.join(" "), m));
        }
    }
    None
}

// ════════════════════════════════════════════════════════════════════════
//  Tests
// ════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fecha_iso_y_relativa() {
        assert!(parsear_fecha("2026-05-10").is_some());
        assert!(parsear_fecha("10/05/2026").is_some());
        assert_eq!(parsear_fecha("hoy"), Some(Local::now().date_naive()));
        assert_eq!(parsear_fecha("today"), Some(Local::now().date_naive()));
        assert_eq!(
            parsear_fecha("tomorrow"),
            Some(Local::now().date_naive() + Duration::days(1))
        );
    }

    #[test]
    fn fecha_textual_es_en() {
        let f = parsear_fecha("15 de mayo de 2026").unwrap();
        assert_eq!(f, NaiveDate::from_ymd_opt(2026, 5, 15).unwrap());
        let f = parsear_fecha("May 15 2026").unwrap();
        assert_eq!(f, NaiveDate::from_ymd_opt(2026, 5, 15).unwrap());
    }

    #[test]
    fn hora_24h_y_ampm() {
        assert_eq!(parsear_hora("14:30"), NaiveTime::from_hms_opt(14, 30, 0));
        assert_eq!(parsear_hora("6pm"), NaiveTime::from_hms_opt(18, 0, 0));
        assert_eq!(parsear_hora("11:15am"), NaiveTime::from_hms_opt(11, 15, 0));
        assert_eq!(parsear_hora("12am"), NaiveTime::from_hms_opt(0, 0, 0));
        assert_eq!(parsear_hora("12pm"), NaiveTime::from_hms_opt(12, 0, 0));
    }

    #[test]
    fn monto_con_simbolo() {
        assert_eq!(parsear_monto("$50"), Some(50.0));
        assert_eq!(parsear_monto("$1,250.75"), Some(1250.75));
        assert_eq!(parsear_monto("50 USD"), Some(50.0));
        assert_eq!(
            parsear_monto("€99,90".replace(',', ".").as_str()),
            Some(99.90)
        );
        // Sin símbolo ni divisa → no es monto
        assert_eq!(parsear_monto("50"), None);
    }

    #[test]
    fn parsea_linea_estructurada() {
        let it = parsear_linea("- comprar leche | 2026-05-10 | alta | #compras");
        assert_eq!(it.titulo, "comprar leche");
        assert_eq!(
            it.fecha,
            Some(NaiveDate::from_ymd_opt(2026, 5, 10).unwrap())
        );
        assert_eq!(it.prioridad.as_deref(), Some("alta"));
        assert_eq!(it.etiquetas, vec!["compras"]);
        assert_eq!(it.categoria, CategoriaItem::Tarea);
    }

    #[test]
    fn parsea_pago_con_monto() {
        let it = parsear_linea("pagar tarjeta Visa | 2026-05-20 | $250 | nota: tasa 18%");
        assert_eq!(it.categoria, CategoriaItem::Pago);
        assert_eq!(it.monto, Some(250.0));
        assert_eq!(
            it.fecha,
            Some(NaiveDate::from_ymd_opt(2026, 5, 20).unwrap())
        );
        assert_eq!(it.notas, vec!["tasa 18%"]);
    }

    #[test]
    fn parsea_evento_libre_inline() {
        let it = parsear_linea("Meeting with Ana 2026-05-12 14:30");
        assert_eq!(it.categoria, CategoriaItem::Evento);
        assert!(it.titulo.to_lowercase().contains("meeting"));
        assert_eq!(
            it.fecha,
            Some(NaiveDate::from_ymd_opt(2026, 5, 12).unwrap())
        );
        assert_eq!(it.hora, NaiveTime::from_hms_opt(14, 30, 0));
    }

    #[test]
    fn nota_si_no_hay_pista() {
        let it = parsear_linea("recordatorio random sin fecha");
        // "recordatorio" no está en KW pero tampoco palabra de tarea/pago/evento
        // → cae en Nota
        assert!(matches!(
            it.categoria,
            CategoriaItem::Nota | CategoriaItem::Tarea
        ));
    }

    #[test]
    fn ignora_comentarios_y_vacios() {
        let txt = "# comentario\n\n- tarea uno | hoy\n// otra cosa\n* tarea dos";
        let v = parsear_texto(txt);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].titulo, "tarea uno");
        assert_eq!(v[1].titulo, "tarea dos");
    }

    #[test]
    fn quita_vinetas_correctamente() {
        assert_eq!(quitar_vinetas("- hola"), "hola");
        assert_eq!(quitar_vinetas("* hola"), "hola");
        assert_eq!(quitar_vinetas("1. hola"), "hola");
        assert_eq!(quitar_vinetas("12) hola"), "hola");
        assert_eq!(quitar_vinetas("hola"), "hola");
        assert_eq!(quitar_vinetas("• hola"), "hola");
    }
}
