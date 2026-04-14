use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;

use chrono::{Datelike, Duration, Local};

use crate::storage::AppState;
use crate::tasks::{Prioridad, TaskStatus};

/// Datos serializados para el dashboard (snapshot del estado)
pub struct DashboardData {
    pub html: String,
}

/// Genera el HTML completo del dashboard responsive
pub fn generar_dashboard_html(state: &AppState) -> String {
    let hoy = Local::now().date_naive();
    let dia = nombre_dia(hoy.weekday());
    let mes = nombre_mes(hoy.month());
    let ahora = Local::now().format("%H:%M").to_string();

    // Datos para el dashboard
    let tareas_hoy = state.tasks.listar_por_fecha(hoy);
    let pendientes = state.tasks.listar_pendientes();
    let eventos_hoy = state.agenda.eventos_del_dia(hoy);
    let horarios = state.agenda.horarios_del_dia(hoy.weekday());
    let follow_ups: Vec<_> = state
        .tasks
        .listar_follow_ups()
        .into_iter()
        .filter(|t| t.follow_up.map(|f| f.date() == hoy).unwrap_or(false))
        .collect();

    // Semana
    let lunes = hoy - Duration::days(hoy.weekday().num_days_from_monday() as i64);

    let mut html = String::new();
    html.push_str(&format!(r#"<!DOCTYPE html>
<html lang="es">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>OmniPlanner — Dashboard</title>
<meta http-equiv="refresh" content="30">
<style>
:root {{
  --bg: #0d1117;
  --card: #161b22;
  --border: #30363d;
  --text: #e6edf3;
  --muted: #8b949e;
  --accent: #58a6ff;
  --green: #3fb950;
  --yellow: #d29922;
  --red: #f85149;
  --purple: #bc8cff;
}}
* {{ margin:0; padding:0; box-sizing:border-box; }}
body {{
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  background: var(--bg);
  color: var(--text);
  padding: 16px;
  max-width: 900px;
  margin: 0 auto;
}}
.header {{
  text-align: center;
  padding: 20px 0;
  border-bottom: 1px solid var(--border);
  margin-bottom: 20px;
}}
.header h1 {{
  color: var(--accent);
  font-size: 1.8em;
  letter-spacing: 2px;
}}
.header .date {{
  color: var(--muted);
  font-size: 1.1em;
  margin-top: 8px;
}}
.stats {{
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
  gap: 12px;
  margin-bottom: 24px;
}}
.stat {{
  background: var(--card);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 16px;
  text-align: center;
}}
.stat .num {{
  font-size: 2em;
  font-weight: bold;
  color: var(--accent);
}}
.stat .label {{
  color: var(--muted);
  font-size: 0.85em;
  margin-top: 4px;
}}
.section {{
  background: var(--card);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 16px;
  margin-bottom: 16px;
}}
.section h2 {{
  font-size: 1.1em;
  margin-bottom: 12px;
  color: var(--accent);
}}
.item {{
  padding: 10px 0;
  border-bottom: 1px solid var(--border);
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}}
.item:last-child {{ border-bottom: none; }}
.item .time {{
  font-weight: bold;
  color: var(--yellow);
  min-width: 55px;
}}
.item .title {{
  flex: 1;
  min-width: 100px;
}}
.badge {{
  display: inline-block;
  padding: 2px 8px;
  border-radius: 12px;
  font-size: 0.75em;
  font-weight: bold;
}}
.badge-done {{ background: var(--green); color: #000; }}
.badge-progress {{ background: var(--yellow); color: #000; }}
.badge-pending {{ background: var(--border); color: var(--text); }}
.badge-cancelled {{ background: var(--red); color: #fff; }}
.badge-urgente {{ background: var(--red); color: #fff; }}
.badge-alta {{ background: #f0883e; color: #000; }}
.badge-media {{ background: var(--yellow); color: #000; }}
.badge-baja {{ background: var(--green); color: #000; }}
.empty {{
  color: var(--muted);
  font-style: italic;
  padding: 8px 0;
}}
.week-day {{
  margin-bottom: 12px;
}}
.week-day h3 {{
  font-size: 0.95em;
  color: var(--purple);
  margin-bottom: 6px;
  padding-bottom: 4px;
  border-bottom: 1px solid var(--border);
}}
.footer {{
  text-align: center;
  color: var(--muted);
  font-size: 0.8em;
  padding: 16px 0;
}}
.tab-bar {{
  display: flex;
  gap: 8px;
  margin-bottom: 20px;
  overflow-x: auto;
}}
.tab {{
  padding: 8px 16px;
  border-radius: 20px;
  background: var(--card);
  border: 1px solid var(--border);
  color: var(--muted);
  cursor: pointer;
  white-space: nowrap;
  font-size: 0.9em;
  text-decoration: none;
}}
.tab.active {{
  background: var(--accent);
  color: #000;
  border-color: var(--accent);
  font-weight: bold;
}}
.view {{ display: none; }}
.view.active {{ display: block; }}
</style>
</head>
<body>

<div class="header">
  <h1>✦ OMNIPLANNER ✦</h1>
  <div class="date">{dia} {d} de {mes} de {y} — {ahora}</div>
</div>

<div class="stats">
  <div class="stat"><div class="num">{n_tareas_hoy}</div><div class="label">Tareas hoy</div></div>
  <div class="stat"><div class="num">{n_eventos_hoy}</div><div class="label">Eventos hoy</div></div>
  <div class="stat"><div class="num">{n_pendientes}</div><div class="label">Pendientes</div></div>
  <div class="stat"><div class="num">{n_followups}</div><div class="label">Follow-ups</div></div>
  <div class="stat"><div class="num">{n_total_tareas}</div><div class="label">Total tareas</div></div>
  <div class="stat"><div class="num">{n_total_eventos}</div><div class="label">Total eventos</div></div>
</div>

<div class="tab-bar">
  <a class="tab active" onclick="showView('hoy')">📋 Hoy</a>
  <a class="tab" onclick="showView('semana')">📅 Semana</a>
  <a class="tab" onclick="showView('todas')">📊 Todas las tareas</a>
  <a class="tab" onclick="showView('agenda')">📌 Todos los eventos</a>
</div>
"#,
        dia = dia,
        d = hoy.day(),
        mes = mes,
        y = hoy.year(),
        ahora = ahora,
        n_tareas_hoy = tareas_hoy.len(),
        n_eventos_hoy = eventos_hoy.len(),
        n_pendientes = pendientes.len(),
        n_followups = follow_ups.len(),
        n_total_tareas = state.tasks.tareas.len(),
        n_total_eventos = state.agenda.eventos.len(),
    ));

    // === Vista HOY ===
    html.push_str(r#"<div id="view-hoy" class="view active">"#);

    // Tareas de hoy
    html.push_str(r#"<div class="section"><h2>📋 Tareas de hoy</h2>"#);
    if tareas_hoy.is_empty() {
        html.push_str(r#"<div class="empty">Sin tareas para hoy</div>"#);
    } else {
        for t in &tareas_hoy {
            let (badge_class, badge_text) = match t.estado {
                TaskStatus::Completada => ("badge-done", "✅ Completada"),
                TaskStatus::EnProgreso => ("badge-progress", "🔄 En progreso"),
                TaskStatus::Cancelada => ("badge-cancelled", "❌ Cancelada"),
                TaskStatus::Pendiente => ("badge-pending", "⬜ Pendiente"),
            };
            let prio_class = match t.prioridad {
                Prioridad::Urgente => "badge-urgente",
                Prioridad::Alta => "badge-alta",
                Prioridad::Media => "badge-media",
                Prioridad::Baja => "badge-baja",
            };
            html.push_str(&format!(
                r#"<div class="item">
                    <span class="time">{hora}</span>
                    <span class="title">{titulo}</span>
                    <span class="badge {bc}">{bt}</span>
                    <span class="badge {pc}">{prio}</span>
                </div>"#,
                hora = t.hora.format("%H:%M"),
                titulo = html_escape(&t.titulo),
                bc = badge_class,
                bt = badge_text,
                pc = prio_class,
                prio = t.prioridad,
            ));
        }
    }
    html.push_str("</div>");

    // Eventos de hoy
    html.push_str(r#"<div class="section"><h2>📅 Eventos de hoy</h2>"#);
    if eventos_hoy.is_empty() {
        html.push_str(r#"<div class="empty">Sin eventos para hoy</div>"#);
    } else {
        for e in &eventos_hoy {
            let fin = e
                .hora_fin
                .map(|h| format!(" - {}", h.format("%H:%M")))
                .unwrap_or_default();
            html.push_str(&format!(
                r#"<div class="item">
                    <span class="time">{hora}{fin}</span>
                    <span class="title">{titulo}</span>
                    <span class="badge badge-media">{tipo}</span>
                </div>"#,
                hora = e.hora_inicio.format("%H:%M"),
                fin = fin,
                titulo = html_escape(&e.titulo),
                tipo = e.tipo,
            ));
        }
    }
    html.push_str("</div>");

    // Horarios de escritura
    if !horarios.is_empty() {
        html.push_str(r#"<div class="section"><h2>✏️ Escritura hoy</h2>"#);
        for h in &horarios {
            html.push_str(&format!(
                r#"<div class="item">
                    <span class="time">{inicio} - {fin}</span>
                    <span class="title">{desc}</span>
                </div>"#,
                inicio = h.hora_inicio.format("%H:%M"),
                fin = h.hora_fin.format("%H:%M"),
                desc = html_escape(&h.descripcion),
            ));
        }
        html.push_str("</div>");
    }

    // Follow-ups
    if !follow_ups.is_empty() {
        html.push_str(r#"<div class="section"><h2>🔔 Follow-ups de hoy</h2>"#);
        for t in &follow_ups {
            html.push_str(&format!(
                r#"<div class="item">
                    <span class="time">{hora}</span>
                    <span class="title">{titulo}</span>
                </div>"#,
                hora = t.follow_up.unwrap().time().format("%H:%M"),
                titulo = html_escape(&t.titulo),
            ));
        }
        html.push_str("</div>");
    }

    html.push_str("</div>"); // end view-hoy

    // === Vista SEMANA ===
    html.push_str(r#"<div id="view-semana" class="view">"#);
    html.push_str(r#"<div class="section"><h2>📅 Vista semanal</h2>"#);
    for i in 0..7 {
        let dia_sem = lunes + Duration::days(i);
        let es_hoy = dia_sem == hoy;
        let nombre = nombre_dia(dia_sem.weekday());
        let mes_d = nombre_mes(dia_sem.month());
        let marker = if es_hoy { " ← HOY" } else { "" };

        html.push_str(&format!(
            r#"<div class="week-day"><h3>{nombre} {d} de {mes}{marker}</h3>"#,
            nombre = nombre,
            d = dia_sem.day(),
            mes = mes_d,
            marker = marker
        ));

        let tareas_dia = state.tasks.listar_por_fecha(dia_sem);
        let eventos_dia = state.agenda.eventos_del_dia(dia_sem);

        if tareas_dia.is_empty() && eventos_dia.is_empty() {
            html.push_str(r#"<div class="empty">Día libre</div>"#);
        }

        for t in &tareas_dia {
            let (badge_class, badge_text) = match t.estado {
                TaskStatus::Completada => ("badge-done", "✅"),
                TaskStatus::EnProgreso => ("badge-progress", "🔄"),
                TaskStatus::Cancelada => ("badge-cancelled", "❌"),
                TaskStatus::Pendiente => ("badge-pending", "⬜"),
            };
            html.push_str(&format!(
                r#"<div class="item">
                    <span class="time">{hora}</span>
                    <span class="badge {bc}">{bt}</span>
                    <span class="title">{titulo} [{prio}]</span>
                </div>"#,
                hora = t.hora.format("%H:%M"),
                bc = badge_class,
                bt = badge_text,
                titulo = html_escape(&t.titulo),
                prio = t.prioridad,
            ));
        }

        for e in &eventos_dia {
            let fin = e
                .hora_fin
                .map(|h| format!("-{}", h.format("%H:%M")))
                .unwrap_or_default();
            html.push_str(&format!(
                r#"<div class="item">
                    <span class="time">{hora}{fin}</span>
                    <span class="title">📌 {titulo} ({tipo})</span>
                </div>"#,
                hora = e.hora_inicio.format("%H:%M"),
                fin = fin,
                titulo = html_escape(&e.titulo),
                tipo = e.tipo,
            ));
        }

        html.push_str("</div>");
    }
    html.push_str("</div></div>"); // end section + view-semana

    // === Vista TODAS LAS TAREAS ===
    html.push_str(r#"<div id="view-todas" class="view">"#);
    html.push_str(r#"<div class="section"><h2>📊 Todas las tareas</h2>"#);
    if state.tasks.tareas.is_empty() {
        html.push_str(r#"<div class="empty">Sin tareas registradas</div>"#);
    } else {
        for t in &state.tasks.tareas {
            let (badge_class, badge_text) = match t.estado {
                TaskStatus::Completada => ("badge-done", "✅ Completada"),
                TaskStatus::EnProgreso => ("badge-progress", "🔄 En progreso"),
                TaskStatus::Cancelada => ("badge-cancelled", "❌ Cancelada"),
                TaskStatus::Pendiente => ("badge-pending", "⬜ Pendiente"),
            };
            let prio_class = match t.prioridad {
                Prioridad::Urgente => "badge-urgente",
                Prioridad::Alta => "badge-alta",
                Prioridad::Media => "badge-media",
                Prioridad::Baja => "badge-baja",
            };
            let fu = t
                .follow_up
                .map(|f| format!(" 🔔 {}", f.format("%d/%m %H:%M")))
                .unwrap_or_default();
            html.push_str(&format!(
                r#"<div class="item">
                    <span class="time">{fecha} {hora}</span>
                    <span class="title">{titulo}{fu}</span>
                    <span class="badge {bc}">{bt}</span>
                    <span class="badge {pc}">{prio}</span>
                </div>"#,
                fecha = t.fecha.format("%d/%m"),
                hora = t.hora.format("%H:%M"),
                titulo = html_escape(&t.titulo),
                fu = fu,
                bc = badge_class,
                bt = badge_text,
                pc = prio_class,
                prio = t.prioridad,
            ));
        }
    }
    html.push_str("</div></div>");

    // === Vista TODOS LOS EVENTOS ===
    html.push_str(r#"<div id="view-agenda" class="view">"#);
    html.push_str(r#"<div class="section"><h2>📌 Todos los eventos</h2>"#);
    if state.agenda.eventos.is_empty() {
        html.push_str(r#"<div class="empty">Sin eventos registrados</div>"#);
    } else {
        for e in &state.agenda.eventos {
            let fin = e
                .hora_fin
                .map(|h| format!(" - {}", h.format("%H:%M")))
                .unwrap_or_default();
            html.push_str(&format!(
                r#"<div class="item">
                    <span class="time">{fecha} {hora}{fin}</span>
                    <span class="title">{titulo}</span>
                    <span class="badge badge-media">{tipo}</span>
                </div>"#,
                fecha = e.fecha.format("%d/%m"),
                hora = e.hora_inicio.format("%H:%M"),
                fin = fin,
                titulo = html_escape(&e.titulo),
                tipo = e.tipo,
            ));
        }
    }
    html.push_str("</div></div>");

    // JavaScript para tabs
    html.push_str(
        r#"
<div class="footer">
  OmniPlanner — Auto-refresca cada 30 segundos<br>
  Abre esta URL desde cualquier dispositivo en la misma red WiFi
</div>

<script>
function showView(id) {
  document.querySelectorAll('.view').forEach(v => v.classList.remove('active'));
  document.querySelectorAll('.tab').forEach(t => t.classList.remove('active'));
  document.getElementById('view-' + id).classList.add('active');
  event.target.classList.add('active');
}
</script>
</body></html>"#,
    );

    html
}

/// Inicia el servidor web en un hilo separado y retorna la dirección
pub fn iniciar_servidor(state: &AppState, puerto: u16) -> Result<String, String> {
    let html = generar_dashboard_html(state);

    // También servir el JSON completo del estado
    let json = serde_json::to_string_pretty(state)
        .map_err(|e| format!("Error serializando estado: {}", e))?;

    // Detectar IP local
    let ip_local = detectar_ip_local().unwrap_or_else(|| "127.0.0.1".to_string());

    let addr = format!("0.0.0.0:{}", puerto);
    let listener = TcpListener::bind(&addr)
        .map_err(|e| format!("No se pudo abrir puerto {}: {}", puerto, e))?;

    let url = format!("http://{}:{}", ip_local, puerto);

    let html_arc = Arc::new(html);
    let json_arc = Arc::new(json);

    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    let html = html_arc.clone();
                    let json = json_arc.clone();
                    thread::spawn(move || {
                        let mut reader = BufReader::new(match stream.try_clone() {
                            Ok(s) => s,
                            Err(_) => return,
                        });
                        let mut request_line = String::new();
                        if reader.read_line(&mut request_line).is_err() {
                            return;
                        }

                        let path = request_line.split_whitespace().nth(1).unwrap_or("/");

                        // Leer el resto de los headers (para que no se quede colgado)
                        loop {
                            let mut line = String::new();
                            if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
                                break;
                            }
                        }

                        let (content_type, body) = match path {
                            "/api/state" | "/api/state.json" => {
                                ("application/json; charset=utf-8", json.as_str().to_string())
                            }
                            _ => ("text/html; charset=utf-8", html.as_str().to_string()),
                        };

                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}",
                            content_type,
                            body.len(),
                            body
                        );
                        let _ = stream.write_all(response.as_bytes());
                    });
                }
                Err(_) => continue,
            }
        }
    });

    Ok(url)
}

fn detectar_ip_local() -> Option<String> {
    // Truco: abrir un socket UDP (sin enviar nada) para detectar la IP local
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let addr = socket.local_addr().ok()?;
    Some(addr.ip().to_string())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn nombre_dia(wd: chrono::Weekday) -> &'static str {
    match wd {
        chrono::Weekday::Mon => "Lunes",
        chrono::Weekday::Tue => "Martes",
        chrono::Weekday::Wed => "Miércoles",
        chrono::Weekday::Thu => "Jueves",
        chrono::Weekday::Fri => "Viernes",
        chrono::Weekday::Sat => "Sábado",
        chrono::Weekday::Sun => "Domingo",
    }
}

fn nombre_mes(m: u32) -> &'static str {
    match m {
        1 => "Enero",
        2 => "Febrero",
        3 => "Marzo",
        4 => "Abril",
        5 => "Mayo",
        6 => "Junio",
        7 => "Julio",
        8 => "Agosto",
        9 => "Septiembre",
        10 => "Octubre",
        11 => "Noviembre",
        12 => "Diciembre",
        _ => "",
    }
}
