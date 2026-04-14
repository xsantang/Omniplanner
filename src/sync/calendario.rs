use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use std::collections::HashMap;

use super::SyncConfig;
use crate::agenda::{Evento, TipoEvento};
use crate::tasks::{Prioridad, Task, TaskStatus};

// ══════════════════════════════════════════════════════════════
//  iCalendar (.ics) — Exportar / Importar
// ══════════════════════════════════════════════════════════════

pub fn exportar_ical(eventos: &[&Evento], tareas: &[&Task]) -> String {
    let mut ical = String::new();
    ical.push_str("BEGIN:VCALENDAR\r\n");
    ical.push_str("VERSION:2.0\r\n");
    ical.push_str("PRODID:-//OmniPlanner//ES\r\n");
    ical.push_str("CALSCALE:GREGORIAN\r\n");
    ical.push_str("METHOD:PUBLISH\r\n");

    for evento in eventos {
        ical.push_str("BEGIN:VEVENT\r\n");
        ical.push_str(&format!("UID:evt-{}@omniplanner\r\n", evento.id));
        ical.push_str(&format!(
            "DTSTART:{}\r\n",
            formato_ical_dt(evento.fecha, evento.hora_inicio)
        ));
        if let Some(fin) = evento.hora_fin {
            ical.push_str(&format!("DTEND:{}\r\n", formato_ical_dt(evento.fecha, fin)));
        }
        ical.push_str(&format!("SUMMARY:{}\r\n", escapar_ical(&evento.titulo)));
        if !evento.descripcion.is_empty() {
            ical.push_str(&format!(
                "DESCRIPTION:{}\r\n",
                escapar_ical(&evento.descripcion)
            ));
        }
        ical.push_str(&format!(
            "CATEGORIES:{}\r\n",
            formato_tipo_evento(&evento.tipo)
        ));
        ical.push_str("END:VEVENT\r\n");
    }

    for tarea in tareas {
        ical.push_str("BEGIN:VTODO\r\n");
        ical.push_str(&format!("UID:task-{}@omniplanner\r\n", tarea.id));
        ical.push_str(&format!(
            "DTSTART:{}\r\n",
            formato_ical_dt(tarea.fecha, tarea.hora)
        ));
        ical.push_str(&format!("SUMMARY:{}\r\n", escapar_ical(&tarea.titulo)));
        if !tarea.descripcion.is_empty() {
            ical.push_str(&format!(
                "DESCRIPTION:{}\r\n",
                escapar_ical(&tarea.descripcion)
            ));
        }
        ical.push_str(&format!(
            "PRIORITY:{}\r\n",
            prioridad_ical(&tarea.prioridad)
        ));
        ical.push_str(&format!("STATUS:{}\r\n", estado_ical(&tarea.estado)));
        if !tarea.etiquetas.is_empty() {
            ical.push_str(&format!("CATEGORIES:{}\r\n", tarea.etiquetas.join(",")));
        }
        if let Some(fu) = tarea.follow_up {
            ical.push_str("BEGIN:VALARM\r\n");
            ical.push_str("ACTION:DISPLAY\r\n");
            ical.push_str(&format!(
                "TRIGGER;VALUE=DATE-TIME:{}\r\n",
                fu.format("%Y%m%dT%H%M%S")
            ));
            ical.push_str(&format!(
                "DESCRIPTION:Follow-up: {}\r\n",
                escapar_ical(&tarea.titulo)
            ));
            ical.push_str("END:VALARM\r\n");
        }
        ical.push_str("END:VTODO\r\n");
    }

    ical.push_str("END:VCALENDAR\r\n");
    ical
}

#[derive(Debug, Clone)]
pub struct EventoImportado {
    pub titulo: String,
    pub descripcion: String,
    pub fecha: NaiveDate,
    pub hora_inicio: NaiveTime,
    pub hora_fin: Option<NaiveTime>,
    pub uid: String,
}

pub fn importar_ical(contenido: &str) -> Vec<EventoImportado> {
    let mut resultados = Vec::new();
    let mut en_evento = false;
    let mut campos: HashMap<String, String> = HashMap::new();

    for linea in contenido.lines() {
        let linea = linea.trim();
        if linea == "BEGIN:VEVENT" {
            en_evento = true;
            campos.clear();
        } else if linea == "END:VEVENT" && en_evento {
            en_evento = false;
            if let Some(ev) = parsear_evento_ical(&campos) {
                resultados.push(ev);
            }
        } else if en_evento {
            if let Some((clave, valor)) = linea.split_once(':') {
                let clave_base = clave.split(';').next().unwrap_or(clave);
                campos.insert(clave_base.to_uppercase(), valor.to_string());
            }
        }
    }

    resultados
}

fn parsear_evento_ical(campos: &HashMap<String, String>) -> Option<EventoImportado> {
    let titulo = campos.get("SUMMARY")?.clone();
    let dtstart = campos.get("DTSTART")?;
    let (fecha, hora_inicio) = parsear_fecha_ical(dtstart)?;

    let hora_fin = campos
        .get("DTEND")
        .and_then(|dt| parsear_fecha_ical(dt))
        .map(|(_, h)| h);

    let descripcion = campos.get("DESCRIPTION").cloned().unwrap_or_default();
    let uid = campos.get("UID").cloned().unwrap_or_default();

    Some(EventoImportado {
        titulo: desescapar_ical(&titulo),
        descripcion: desescapar_ical(&descripcion),
        fecha,
        hora_inicio,
        hora_fin,
        uid,
    })
}

fn parsear_fecha_ical(s: &str) -> Option<(NaiveDate, NaiveTime)> {
    let s = s.trim_end_matches('Z');
    if s.len() >= 15 {
        let dt = NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%S").ok()?;
        Some((dt.date(), dt.time()))
    } else if s.len() >= 8 {
        let d = NaiveDate::parse_from_str(&s[..8], "%Y%m%d").ok()?;
        Some((d, NaiveTime::from_hms_opt(0, 0, 0).unwrap()))
    } else {
        None
    }
}

fn formato_ical_dt(fecha: NaiveDate, hora: NaiveTime) -> String {
    NaiveDateTime::new(fecha, hora)
        .format("%Y%m%dT%H%M%S")
        .to_string()
}

fn escapar_ical(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(',', "\\,")
        .replace(';', "\\;")
        .replace('\n', "\\n")
}

fn desescapar_ical(s: &str) -> String {
    s.replace("\\n", "\n")
        .replace("\\,", ",")
        .replace("\\;", ";")
        .replace("\\\\", "\\")
}

fn formato_tipo_evento(tipo: &TipoEvento) -> &str {
    match tipo {
        TipoEvento::Reunion => "MEETING",
        TipoEvento::Recordatorio => "REMINDER",
        TipoEvento::FollowUp => "FOLLOW-UP",
        TipoEvento::Cita => "APPOINTMENT",
        TipoEvento::Cumpleanos => "BIRTHDAY",
        TipoEvento::Pago => "PAYMENT",
        TipoEvento::Otro(_) => "OTHER",
    }
}

fn prioridad_ical(p: &Prioridad) -> u8 {
    match p {
        Prioridad::Urgente => 1,
        Prioridad::Alta => 3,
        Prioridad::Media => 5,
        Prioridad::Baja => 9,
    }
}

fn estado_ical(e: &TaskStatus) -> &str {
    match e {
        TaskStatus::Pendiente => "NEEDS-ACTION",
        TaskStatus::EnProgreso => "IN-PROCESS",
        TaskStatus::Completada => "COMPLETED",
        TaskStatus::Cancelada => "CANCELLED",
    }
}

// ══════════════════════════════════════════════════════════════
//  Google Calendar API (OAuth2 + REST)
// ══════════════════════════════════════════════════════════════

const REDIRECT_PORT: u16 = 8085;

fn redirect_uri() -> String {
    format!("http://localhost:{}", REDIRECT_PORT)
}

pub fn google_auth_url(config: &SyncConfig) -> String {
    format!(
        "https://accounts.google.com/o/oauth2/v2/auth\
         ?client_id={}\
         &redirect_uri={}\
         &response_type=code\
         &scope=https://www.googleapis.com/auth/calendar%20https://www.googleapis.com/auth/drive.file\
         &access_type=offline\
         &prompt=consent",
        config.google_client_id,
        redirect_uri()
    )
}

/// Levanta un servidor local temporal para capturar el código OAuth.
/// Retorna el código de autorización.
pub fn escuchar_codigo_oauth() -> Result<String, String> {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind(format!("127.0.0.1:{}", REDIRECT_PORT))
        .map_err(|e| format!("No se pudo abrir puerto {}: {}", REDIRECT_PORT, e))?;

    // Esperar una conexión (el navegador redirige aquí)
    let (mut stream, _) = listener
        .accept()
        .map_err(|e| format!("Error aceptando conexión: {}", e))?;

    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .map_err(|e| format!("Error leyendo request: {}", e))?;

    // Extraer código de "GET /?code=XXXX&scope=... HTTP/1.1"
    let codigo = request_line
        .split_whitespace()
        .nth(1) // el path "/?code=..."
        .and_then(|path| {
            path.split('?').nth(1).and_then(|query| {
                query.split('&').find_map(|param| {
                    let mut kv = param.splitn(2, '=');
                    match (kv.next(), kv.next()) {
                        (Some("code"), Some(v)) => Some(v.to_string()),
                        _ => None,
                    }
                })
            })
        })
        .ok_or("No se encontró el código de autorización en la respuesta")?;

    // Responder al navegador
    let html = "<html><body style='font-family:sans-serif;text-align:center;padding:40px'>\
                <h2>✅ OmniPlanner autorizado</h2>\
                <p>Puedes cerrar esta pestaña y volver a la terminal.</p>\
                </body></html>";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
        html.len(),
        html
    );
    let _ = stream.write_all(response.as_bytes());

    Ok(codigo)
}

pub fn google_intercambiar_codigo(config: &mut SyncConfig, codigo: &str) -> Result<(), String> {
    let resp = ureq::post("https://oauth2.googleapis.com/token")
        .send_form(&[
            ("code", codigo.trim()),
            ("client_id", &config.google_client_id),
            ("client_secret", &config.google_client_secret),
            ("redirect_uri", &redirect_uri()),
            ("grant_type", "authorization_code"),
        ])
        .map_err(|e| format!("Error OAuth: {}", e))?;

    let body: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("Error parseando respuesta: {}", e))?;

    config.google_access_token = body["access_token"].as_str().map(|s| s.to_string());
    config.google_refresh_token = body["refresh_token"].as_str().map(|s| s.to_string());

    if config.google_access_token.is_none() {
        return Err(format!("Token no recibido: {}", body));
    }

    Ok(())
}

pub fn google_refrescar_token(config: &mut SyncConfig) -> Result<(), String> {
    let refresh = config
        .google_refresh_token
        .as_ref()
        .ok_or("No hay refresh_token")?;

    let resp = ureq::post("https://oauth2.googleapis.com/token")
        .send_form(&[
            ("refresh_token", refresh.as_str()),
            ("client_id", &config.google_client_id),
            ("client_secret", &config.google_client_secret),
            ("grant_type", "refresh_token"),
        ])
        .map_err(|e| format!("Error refrescando token: {}", e))?;

    let body: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("Error parseando respuesta: {}", e))?;

    config.google_access_token = body["access_token"].as_str().map(|s| s.to_string());

    if config.google_access_token.is_none() {
        return Err("No se pudo refrescar el token".to_string());
    }

    Ok(())
}

pub fn google_crear_evento(config: &SyncConfig, evento: &Evento) -> Result<String, String> {
    let token = config
        .google_access_token
        .as_ref()
        .ok_or("No autenticado con Google Calendar")?;

    let hora_fin = evento
        .hora_fin
        .unwrap_or_else(|| evento.hora_inicio + chrono::Duration::hours(1));

    let body = serde_json::json!({
        "summary": evento.titulo,
        "description": evento.descripcion,
        "start": {
            "dateTime": formato_google_dt(evento.fecha, evento.hora_inicio),
            "timeZone": "America/New_York"
        },
        "end": {
            "dateTime": formato_google_dt(evento.fecha, hora_fin),
            "timeZone": "America/New_York"
        }
    });

    let url = format!(
        "https://www.googleapis.com/calendar/v3/calendars/{}/events",
        config.google_calendar_id
    );

    let resp = ureq::post(&url)
        .set("Authorization", &format!("Bearer {}", token))
        .send_json(body)
        .map_err(|e| format!("Error creando evento: {}", e))?;

    let result: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("Error parseando respuesta: {}", e))?;

    result["id"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or("No se obtuvo ID del evento creado".to_string())
}

pub fn google_crear_evento_tarea(config: &SyncConfig, tarea: &Task) -> Result<String, String> {
    let token = config
        .google_access_token
        .as_ref()
        .ok_or("No autenticado con Google Calendar")?;

    let desc = format!(
        "{}\nPrioridad: {}\nEstado: {}",
        tarea.descripcion, tarea.prioridad, tarea.estado
    );

    let hora_fin = tarea.hora + chrono::Duration::hours(1);

    let body = serde_json::json!({
        "summary": format!("[{}] {}", tarea.prioridad, tarea.titulo),
        "description": desc,
        "start": {
            "dateTime": formato_google_dt(tarea.fecha, tarea.hora),
            "timeZone": "America/New_York"
        },
        "end": {
            "dateTime": formato_google_dt(tarea.fecha, hora_fin),
            "timeZone": "America/New_York"
        },
        "colorId": color_prioridad(&tarea.prioridad)
    });

    let url = format!(
        "https://www.googleapis.com/calendar/v3/calendars/{}/events",
        config.google_calendar_id
    );

    let resp = ureq::post(&url)
        .set("Authorization", &format!("Bearer {}", token))
        .send_json(body)
        .map_err(|e| format!("Error creando evento: {}", e))?;

    let result: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("Error parseando respuesta: {}", e))?;

    result["id"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or("No se obtuvo ID del evento creado".to_string())
}

pub fn google_listar_eventos(
    config: &SyncConfig,
    fecha: NaiveDate,
) -> Result<Vec<EventoImportado>, String> {
    let token = config
        .google_access_token
        .as_ref()
        .ok_or("No autenticado con Google Calendar")?;

    let time_min = format!("{}T00:00:00Z", fecha.format("%Y-%m-%d"));
    let time_max = format!("{}T23:59:59Z", fecha.format("%Y-%m-%d"));

    let url = format!(
        "https://www.googleapis.com/calendar/v3/calendars/{}/events\
         ?timeMin={}&timeMax={}&singleEvents=true&orderBy=startTime",
        config.google_calendar_id, time_min, time_max
    );

    let resp = ureq::get(&url)
        .set("Authorization", &format!("Bearer {}", token))
        .call()
        .map_err(|e| format!("Error listando eventos: {}", e))?;

    let body: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("Error parseando respuesta: {}", e))?;

    let mut eventos = Vec::new();
    if let Some(items) = body["items"].as_array() {
        for item in items {
            let titulo = item["summary"].as_str().unwrap_or("Sin título").to_string();
            let descripcion = item["description"].as_str().unwrap_or("").to_string();
            let uid = item["id"].as_str().unwrap_or("").to_string();

            let (fecha_ev, hora_inicio) = if let Some(dt) = item["start"]["dateTime"].as_str() {
                parsear_google_dt(dt).unwrap_or((fecha, NaiveTime::from_hms_opt(0, 0, 0).unwrap()))
            } else {
                (fecha, NaiveTime::from_hms_opt(0, 0, 0).unwrap())
            };

            let hora_fin = item["end"]["dateTime"]
                .as_str()
                .and_then(parsear_google_dt)
                .map(|(_, h)| h);

            eventos.push(EventoImportado {
                titulo,
                descripcion,
                fecha: fecha_ev,
                hora_inicio,
                hora_fin,
                uid,
            });
        }
    }

    Ok(eventos)
}

pub fn google_eliminar_evento(config: &SyncConfig, google_event_id: &str) -> Result<(), String> {
    let token = config
        .google_access_token
        .as_ref()
        .ok_or("No autenticado con Google Calendar")?;

    let url = format!(
        "https://www.googleapis.com/calendar/v3/calendars/{}/events/{}",
        config.google_calendar_id, google_event_id
    );

    ureq::delete(&url)
        .set("Authorization", &format!("Bearer {}", token))
        .call()
        .map_err(|e| format!("Error eliminando evento: {}", e))?;

    Ok(())
}

fn formato_google_dt(fecha: NaiveDate, hora: NaiveTime) -> String {
    format!("{}T{}", fecha.format("%Y-%m-%d"), hora.format("%H:%M:%S"))
}

fn parsear_google_dt(s: &str) -> Option<(NaiveDate, NaiveTime)> {
    let parts: Vec<&str> = s.splitn(2, 'T').collect();
    if parts.len() < 2 {
        return None;
    }
    let fecha = NaiveDate::parse_from_str(parts[0], "%Y-%m-%d").ok()?;
    let time_part = parts[1];
    let time_str = if time_part.len() >= 8 {
        &time_part[..8]
    } else {
        time_part
    };
    let hora = NaiveTime::parse_from_str(time_str, "%H:%M:%S").ok()?;
    Some((fecha, hora))
}

fn color_prioridad(p: &Prioridad) -> &str {
    match p {
        Prioridad::Urgente => "11",
        Prioridad::Alta => "6",
        Prioridad::Media => "5",
        Prioridad::Baja => "2",
    }
}
