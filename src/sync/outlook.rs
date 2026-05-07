//! Importador iCal extendido para archivos exportados por Outlook/Exchange.
//!
//! Extiende el importador básico de `calendario.rs` con soporte para:
//! - Líneas dobladas (RFC 5545 §3.1 line folding: línea continúa si empieza con espacio)
//! - Bloques VTIMEZONE (se detectan y se usan para identificar la zona horaria)
//! - Propiedades X-MICROSOFT-* (CDO class, busy status, etc.)
//! - Propiedades con parámetros múltiples (DTSTART;TZID=...:valor)
//! - VALUE=DATE (todo el día, sin hora)
//! - Campos ORGANIZER y ATTENDEE (se extraen como texto plano)

use chrono::{NaiveDate, NaiveTime};
use std::collections::HashMap;

use super::calendario::EventoImportado;

// ─── Resultado extendido ─────────────────────────────────────────────────────

/// Evento importado desde Outlook con campos adicionales respecto al básico.
#[derive(Debug, Clone)]
pub struct EventoOutlook {
    pub base: EventoImportado,
    /// Zona horaria detectada en DTSTART;TZID=... (ej: "Eastern Standard Time")
    pub tzid: Option<String>,
    /// Organizer (nombre o email)
    pub organizer: Option<String>,
    /// Lista de asistentes
    pub attendees: Vec<String>,
    /// Si el evento es de todo el día (VALUE=DATE)
    pub todo_el_dia: bool,
    /// Estado de ocupación de Outlook: "FREE", "BUSY", "OOF", "TENTATIVE"
    pub busy_status: Option<String>,
    /// Clase del evento: "PUBLIC", "PRIVATE", "CONFIDENTIAL"
    pub clase: Option<String>,
    /// Propiedades X-MICROSOFT-* adicionales para auditoría/debug
    pub microsoft_props: HashMap<String, String>,
}

// ─── Parser principal ────────────────────────────────────────────────────────

/// Parsea un archivo .ics de Outlook/Exchange y retorna eventos extendidos.
///
/// Maneja correctamente:
/// - RFC 5545 line folding (continuación con espacio/tab inicial)
/// - VTIMEZONE blocks (omitidos en el resultado pero usados para tzid)
/// - Parámetros de propiedad (DTSTART;TZID=X:valor)
/// - VALUE=DATE (todo el día)
/// - X-MICROSOFT-* propiedades
pub fn importar_outlook(contenido: &str) -> Vec<EventoOutlook> {
    let lineas_unfolded = unfold_lines(contenido);
    let mut resultados = Vec::new();

    let mut en_vevent = false;
    let mut en_vtimezone = false;
    let mut campos: HashMap<String, String> = HashMap::new();
    let mut params_map: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut microsoft: HashMap<String, String> = HashMap::new();
    let mut attendees: Vec<String> = Vec::new();

    for linea in &lineas_unfolded {
        let linea = linea.as_str();

        match linea {
            "BEGIN:VEVENT" => {
                en_vevent = true;
                campos.clear();
                params_map.clear();
                microsoft.clear();
                attendees.clear();
            }
            "END:VEVENT" if en_vevent => {
                en_vevent = false;
                if let Some(ev) = parsear_evento_outlook(&campos, &params_map, &microsoft, &attendees) {
                    resultados.push(ev);
                }
            }
            "BEGIN:VTIMEZONE" => { en_vtimezone = true; }
            "END:VTIMEZONE" => { en_vtimezone = false; }
            _ if en_vtimezone => {} // ignorar contenido de VTIMEZONE
            _ if en_vevent => {
                let (nombre_prop, params, valor) = parsear_linea_ical(linea);
                let nombre_upper = nombre_prop.to_uppercase();

                // Propiedades Microsoft específicas
                if nombre_upper.starts_with("X-MICROSOFT-") {
                    microsoft.insert(nombre_upper, valor);
                } else if nombre_upper == "ATTENDEE" {
                    // Extraer CN o dirección de email del attendee
                    let cn = params.get("CN").cloned()
                        .or_else(|| extraer_mailto(&valor))
                        .unwrap_or(valor.clone());
                    attendees.push(cn);
                } else {
                    // Guardar parámetros para recuperar TZID, VALUE, etc.
                    if !params.is_empty() {
                        params_map.insert(nombre_upper.clone(), params);
                    }
                    campos.insert(nombre_upper, valor);
                }
            }
            _ => {}
        }
    }

    resultados
}

// ─── Conversión a EventoImportado básico ─────────────────────────────────────

/// Convierte un slice de EventoOutlook a EventoImportado para compatibilidad
/// con el código existente de la agenda.
pub fn a_eventos_importados(eventos: &[EventoOutlook]) -> Vec<EventoImportado> {
    eventos.iter().map(|e| e.base.clone()).collect()
}

// ─── Helpers internos ────────────────────────────────────────────────────────

/// RFC 5545 §3.1 — Une líneas dobladas: si la siguiente línea empieza con
/// espacio o tab, es continuación de la anterior (se elimina el separador).
fn unfold_lines(contenido: &str) -> Vec<String> {
    let mut resultado: Vec<String> = Vec::new();
    for linea in contenido.lines() {
        if (linea.starts_with(' ') || linea.starts_with('\t')) && !resultado.is_empty() {
            if let Some(last) = resultado.last_mut() {
                last.push_str(linea.trim_start_matches([' ', '\t']));
            }
        } else {
            resultado.push(linea.to_string());
        }
    }
    resultado
}

/// Parsea una línea iCal en (nombre_propiedad, parámetros, valor).
///
/// Ejemplos:
/// - `DTSTART;TZID=Eastern Standard Time:20260506T090000`
///   → ("DTSTART", {"TZID": "Eastern Standard Time"}, "20260506T090000")
/// - `SUMMARY:Reunión de equipo`
///   → ("SUMMARY", {}, "Reunión de equipo")
fn parsear_linea_ical(linea: &str) -> (String, HashMap<String, String>, String) {
    // Separar nombre+params de valor por el primer ':'
    // Cuidado: el valor puede contener ':', los parámetros no
    let colon_pos = linea.find(':').unwrap_or(linea.len());
    let parte_nombre = &linea[..colon_pos];
    let valor = if colon_pos < linea.len() {
        linea[colon_pos + 1..].to_string()
    } else {
        String::new()
    };

    // Separar nombre de parámetros por ';'
    let mut partes = parte_nombre.splitn(2, ';');
    let nombre = partes.next().unwrap_or("").to_string();
    let params_str = partes.next().unwrap_or("");

    let mut params: HashMap<String, String> = HashMap::new();
    for param in params_str.split(';') {
        if let Some((k, v)) = param.split_once('=') {
            params.insert(k.trim().to_uppercase(), v.trim().trim_matches('"').to_string());
        }
    }

    (nombre, params, valor)
}

fn parsear_evento_outlook(
    campos: &HashMap<String, String>,
    params: &HashMap<String, HashMap<String, String>>,
    microsoft: &HashMap<String, String>,
    attendees: &[String],
) -> Option<EventoOutlook> {
    let titulo = campos.get("SUMMARY")?.clone();
    let dtstart_raw = campos.get("DTSTART")?;

    // Detectar si es todo el día (VALUE=DATE o formato YYYYMMDD sin T)
    let value_param = params
        .get("DTSTART")
        .and_then(|p| p.get("VALUE"))
        .map(|s| s.as_str());
    let todo_el_dia = value_param == Some("DATE") || !dtstart_raw.contains('T');

    let tzid = params
        .get("DTSTART")
        .and_then(|p| p.get("TZID"))
        .cloned();

    let (fecha, hora_inicio) = parsear_dt_outlook(dtstart_raw, todo_el_dia)?;

    let hora_fin = campos
        .get("DTEND")
        .and_then(|dt| {
            let es_dia = params
                .get("DTEND")
                .and_then(|p| p.get("VALUE"))
                .map(|v| v == "DATE")
                .unwrap_or(!dt.contains('T'));
            parsear_dt_outlook(dt, es_dia)
        })
        .map(|(_, h)| h);

    let descripcion = campos
        .get("DESCRIPTION")
        .map(|d| desescapar(d))
        .unwrap_or_default();
    let uid = campos.get("UID").cloned().unwrap_or_default();

    // Organizer: puede ser "CN=Nombre:MAILTO:email" o solo email
    let organizer = campos.get("ORGANIZER").map(|o| {
        if let Some(cn) = params.get("ORGANIZER").and_then(|p| p.get("CN")) {
            cn.clone()
        } else {
            extraer_mailto(o).unwrap_or_else(|| o.clone())
        }
    });

    let busy_status = microsoft
        .get("X-MICROSOFT-CDO-BUSYSTATUS")
        .cloned()
        .or_else(|| campos.get("X-MICROSOFT-CDO-BUSYSTATUS").cloned());

    let clase = campos.get("CLASS").cloned();

    Some(EventoOutlook {
        base: EventoImportado {
            titulo: desescapar(&titulo),
            descripcion,
            fecha,
            hora_inicio,
            hora_fin,
            uid,
        },
        tzid,
        organizer,
        attendees: attendees.to_vec(),
        todo_el_dia,
        busy_status,
        clase,
        microsoft_props: microsoft.clone(),
    })
}

fn parsear_dt_outlook(s: &str, todo_el_dia: bool) -> Option<(NaiveDate, NaiveTime)> {
    // Quitar sufijo 'Z' (UTC) — no hacemos conversión de zona, solo parseo
    let s = s.trim_end_matches('Z');
    if todo_el_dia || s.len() == 8 {
        let d = NaiveDate::parse_from_str(&s[..8.min(s.len())], "%Y%m%d").ok()?;
        Some((d, NaiveTime::from_hms_opt(0, 0, 0).unwrap()))
    } else if s.len() >= 15 {
        // Formato: 20260506T090000
        let año = s[..4].parse::<i32>().ok()?;
        let mes = s[4..6].parse::<u32>().ok()?;
        let dia = s[6..8].parse::<u32>().ok()?;
        let hora = s[9..11].parse::<u32>().ok()?;
        let min = s[11..13].parse::<u32>().ok()?;
        let seg = s[13..15].parse::<u32>().ok()?;
        let d = NaiveDate::from_ymd_opt(año, mes, dia)?;
        let t = NaiveTime::from_hms_opt(hora, min, seg)?;
        Some((d, t))
    } else {
        None
    }
}

fn extraer_mailto(s: &str) -> Option<String> {
    s.to_uppercase()
        .find("MAILTO:")
        .map(|pos| s[pos + 7..].to_string())
}

fn desescapar(s: &str) -> String {
    s.replace("\\n", "\n")
        .replace("\\N", "\n")
        .replace("\\,", ",")
        .replace("\\;", ";")
        .replace("\\\\", "\\")
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unfold_lines() {
        let ics = "BEGIN:VCALENDAR\r\nSUMMARY:Reuni\r\n ón de equipo\r\nEND:VCALENDAR";
        let unfolded = unfold_lines(ics);
        assert!(unfolded.iter().any(|l| l.contains("Reunión de equipo")));
    }

    #[test]
    fn test_parsear_linea_ical_con_tzid() {
        let linea = "DTSTART;TZID=Eastern Standard Time:20260506T090000";
        let (nombre, params, valor) = parsear_linea_ical(linea);
        assert_eq!(nombre, "DTSTART");
        assert_eq!(params.get("TZID").map(|s| s.as_str()), Some("Eastern Standard Time"));
        assert_eq!(valor, "20260506T090000");
    }

    #[test]
    fn test_importar_outlook_basico() {
        let ics = "\
BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
BEGIN:VTIMEZONE\r\n\
TZID:Eastern Standard Time\r\n\
END:VTIMEZONE\r\n\
BEGIN:VEVENT\r\n\
UID:abc123@outlook\r\n\
DTSTART;TZID=Eastern Standard Time:20260510T100000\r\n\
DTEND;TZID=Eastern Standard Time:20260510T110000\r\n\
SUMMARY:Reunión mensual\r\n\
DESCRIPTION:Agenda del mes\r\n\
X-MICROSOFT-CDO-BUSYSTATUS:BUSY\r\n\
END:VEVENT\r\n\
END:VCALENDAR";
        let eventos = importar_outlook(ics);
        assert_eq!(eventos.len(), 1);
        assert_eq!(eventos[0].base.titulo, "Reunión mensual");
        assert_eq!(eventos[0].busy_status.as_deref(), Some("BUSY"));
        assert_eq!(eventos[0].tzid.as_deref(), Some("Eastern Standard Time"));
    }

    #[test]
    fn test_todo_el_dia() {
        let ics = "\
BEGIN:VCALENDAR\r\n\
BEGIN:VEVENT\r\n\
UID:dia@outlook\r\n\
DTSTART;VALUE=DATE:20260601\r\n\
DTEND;VALUE=DATE:20260602\r\n\
SUMMARY:Día libre\r\n\
END:VEVENT\r\n\
END:VCALENDAR";
        let eventos = importar_outlook(ics);
        assert_eq!(eventos.len(), 1);
        assert!(eventos[0].todo_el_dia);
    }
}
