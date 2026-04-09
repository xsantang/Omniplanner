use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, Weekday};
use colored::Colorize;
use dialoguer::{Input, Select, Confirm};

use omniplanner::agenda::{DiaMarcado, Evento, Frecuencia, HorarioEscritura, TipoDiaMarcado, TipoEvento};
use omniplanner::canvas::{Canvas, Elemento};
use omniplanner::diagrams::{Diagrama, Nodo, TipoConexion, TipoDiagrama, TipoNodo};
use omniplanner::mapper::{Codificacion, EsquemaMapa, Mapper};
use omniplanner::memoria::Recuerdo;
use omniplanner::storage::AppState;
use omniplanner::tasks::{Prioridad, Task, TaskStatus};
use omniplanner::sync;
use omniplanner::ml::{
    Activacion, Dataset, ModeloML, Perdida, Rng, TipoModelo,
    ANN, CNN, DNN, RNN, SVM, SVMMulticlase, TipoRNN,
    ArbolDecision, BosqueAleatorio,
    QTable, GridWorld, MultiBandit,
    dataset_iris_sintetico, dataset_xor, dataset_circulos, dataset_secuencia_temporal,
};

// ══════════════════════════════════════════════════════════════
//  Helpers de UI
// ══════════════════════════════════════════════════════════════

fn limpiar() {
    print!("\x1B[2J\x1B[H");
}

fn banner() {
    println!("{}", "╔══════════════════════════════════════════════╗".cyan());
    println!("{}", "║         ✦  O M N I P L A N N E R  ✦         ║".cyan().bold());
    println!("{}", "║   Tu asistente todo-en-uno de productividad  ║".cyan());
    println!("{}", "╚══════════════════════════════════════════════╝".cyan());
    println!();
}

fn separador(titulo: &str) {
    println!();
    println!("{}", format!("── {} ──", titulo).cyan().bold());
    println!();
}

fn pausa() {
    let _: String = Input::new()
        .with_prompt("  Presiona Enter para continuar".to_string())
        .default(String::new())
        .allow_empty(true)
        .interact_text()
        .unwrap_or_default();
}

fn pedir_texto(prompt: &str) -> Option<String> {
    let s: String = Input::new()
        .with_prompt(format!("  {} (vacío=cancelar)", prompt))
        .allow_empty(true)
        .interact_text()
        .unwrap_or_default();
    if s.trim().is_empty() { None } else { Some(s) }
}

fn pedir_texto_opcional(prompt: &str) -> String {
    Input::new()
        .with_prompt(format!("  {}", prompt))
        .default(String::new())
        .allow_empty(true)
        .interact_text()
        .unwrap_or_default()
}

fn formatear_fecha_es(f: NaiveDate) -> String {
    let dia = f.day();
    let mes = match f.month() {
        1 => "enero", 2 => "febrero", 3 => "marzo", 4 => "abril",
        5 => "mayo", 6 => "junio", 7 => "julio", 8 => "agosto",
        9 => "septiembre", 10 => "octubre", 11 => "noviembre", 12 => "diciembre",
        _ => "",
    };
    let anio = f.year();
    let dow = match f.weekday() {
        Weekday::Mon => "lunes", Weekday::Tue => "martes", Weekday::Wed => "miércoles",
        Weekday::Thu => "jueves", Weekday::Fri => "viernes", Weekday::Sat => "sábado",
        Weekday::Sun => "domingo",
    };
    format!("{} {} de {} de {}", dow, dia, mes, anio)
}

fn pedir_fecha(prompt: &str) -> Option<NaiveDate> {
    println!("    {} Formatos: hoy, mañana, 28/03/2026, 28-03-2026, 28032026,", "💡".to_string());
    println!("    {}           28 de marzo de 2026, march 28 2026, 2026-03-28", " ".to_string());
    loop {
        let s = pedir_texto_opcional(&format!("{} (vacío=cancelar)", prompt));
        if s.is_empty() { return None; }
        let candidatos = parsear_fecha_candidatos(&s);
        match candidatos.len() {
            0 => {
                println!("    {} No pude entender esa fecha. Intenta otro formato.", "✗".red());
            }
            1 => {
                let f = candidatos[0];
                println!("    {} Fecha: {}", "✓".green(), formatear_fecha_es(f));
                return Some(f);
            }
            _ => {
                println!("\n    {} Fecha ambigua — ¿cuál quisiste decir?\n", "⚠".yellow());
                let opciones: Vec<String> = candidatos.iter().enumerate().map(|(i, f)| {
                    let letra = (b'A' + i as u8) as char;
                    format!("  {} → {} ({})", letra, formatear_fecha_es(*f), f.format("%d/%m/%Y"))
                }).collect();
                let sel = Select::new()
                    .items(&opciones)
                    .default(0)
                    .interact_opt()
                    .unwrap_or(None);
                match sel {
                    Some(idx) => {
                        let f = candidatos[idx];
                        println!("    {} Fecha: {}", "✓".green(), formatear_fecha_es(f));
                        return Some(f);
                    }
                    None => {
                        println!("    {} Cancelado, intenta de nuevo.", "✗".red());
                    }
                }
            }
        }
    }
}

/// Devuelve todas las interpretaciones válidas de una fecha (sin duplicados).
/// Si no hay ambigüedad devuelve 0 o 1 candidato; si la hay, 2+.
fn parsear_fecha_candidatos(input: &str) -> Vec<NaiveDate> {
    let s = input.trim().to_lowercase();
    let hoy = Local::now().date_naive();

    // Atajos: hoy, mañana, etc. — no ambiguos
    match s.as_str() {
        "hoy" | "today" => return vec![hoy],
        "mañana" | "manana" | "tomorrow" => return vec![hoy + Duration::days(1)],
        "ayer" | "yesterday" => return vec![hoy - Duration::days(1)],
        "pasado mañana" | "pasado manana" => return vec![hoy + Duration::days(2)],
        _ => {}
    }

    // Día de la semana — no ambiguo
    if let Some(target) = dia_semana_a_weekday(&s) {
        let hoy_wd = hoy.weekday().num_days_from_monday();
        let target_wd = target.num_days_from_monday();
        let dias = if target_wd > hoy_wd {
            target_wd - hoy_wd
        } else {
            7 - (hoy_wd - target_wd)
        };
        return vec![hoy + Duration::days(dias as i64)];
    }

    // ISO: 2026-03-28 — no ambiguo
    if let Ok(f) = NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
        return vec![f];
    }

    // dd/mm/yyyy o dd-mm-yyyy — el separador indica formato explícito, no ambiguo
    if s.contains('/') || s.contains('-') {
        if let Ok(f) = NaiveDate::parse_from_str(&s, "%d/%m/%Y") {
            return vec![f];
        }
        if let Ok(f) = NaiveDate::parse_from_str(&s, "%d-%m-%Y") {
            return vec![f];
        }
        // Texto con separadores no reconocido
        return vec![];
    }

    // Texto con nombre de mes — no ambiguo
    if let Some(f) = parsear_fecha_texto_es(&s) {
        return vec![f];
    }
    if let Some(f) = parsear_fecha_texto_en(&s) {
        return vec![f];
    }

    // ═══ Solo dígitos: aquí puede haber ambigüedad ═══
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();

    if digits.len() == 8 {
        let mut candidatos: Vec<NaiveDate> = Vec::new();

        // Interpretación dd/mm/yyyy
        if let Some(f) = parse_ddmmyyyy(&digits) { candidatos.push(f); }
        // Interpretación mm/dd/yyyy
        if let Some(f) = parse_mmddyyyy(&digits) {
            if !candidatos.contains(&f) { candidatos.push(f); }
        }
        // Interpretación yyyy/mm/dd
        if let Ok(f) = NaiveDate::parse_from_str(&digits, "%Y%m%d") {
            if !candidatos.contains(&f) { candidatos.push(f); }
        }

        return candidatos;
    }

    if digits.len() == 6 {
        let mut candidatos: Vec<NaiveDate> = Vec::new();

        if let Some(f) = parse_ddmmyy(&digits) { candidatos.push(f); }
        if let Some(f) = parse_mmddyy(&digits) {
            if !candidatos.contains(&f) { candidatos.push(f); }
        }

        return candidatos;
    }

    vec![]
}

fn dia_semana_a_weekday(s: &str) -> Option<Weekday> {
    match s {
        "lunes" | "monday" | "mon" | "lun" => Some(Weekday::Mon),
        "martes" | "tuesday" | "tue" | "mar" => Some(Weekday::Tue),
        "miercoles" | "miércoles" | "wednesday" | "wed" | "mie" | "mié" => Some(Weekday::Wed),
        "jueves" | "thursday" | "thu" | "jue" => Some(Weekday::Thu),
        "viernes" | "friday" | "fri" | "vie" => Some(Weekday::Fri),
        "sabado" | "sábado" | "saturday" | "sat" | "sab" | "sáb" => Some(Weekday::Sat),
        "domingo" | "sunday" | "sun" | "dom" => Some(Weekday::Sun),
        _ => None,
    }
}

fn mes_texto_a_numero(s: &str) -> Option<u32> {
    match s {
        "enero" | "ene" | "january" | "jan" => Some(1),
        "febrero" | "feb" | "february" => Some(2),
        "marzo" | "mar" | "march" => Some(3),
        "abril" | "abr" | "april" | "apr" => Some(4),
        "mayo" | "may" => Some(5),
        "junio" | "jun" | "june" => Some(6),
        "julio" | "jul" | "july" => Some(7),
        "agosto" | "ago" | "august" | "aug" => Some(8),
        "septiembre" | "setiembre" | "sep" | "sept" | "september" => Some(9),
        "octubre" | "oct" | "october" => Some(10),
        "noviembre" | "nov" | "november" => Some(11),
        "diciembre" | "dic" | "december" | "dec" => Some(12),
        _ => {
            // Búsqueda parcial por si hay variantes o typos
            if s.starts_with("ene") { return Some(1); }
            if s.starts_with("feb") { return Some(2); }
            if s.starts_with("mar") && !s.starts_with("may") { return Some(3); }
            if s.starts_with("abr") || s.starts_with("apr") { return Some(4); }
            if s.starts_with("may") { return Some(5); }
            if s.starts_with("jun") { return Some(6); }
            if s.starts_with("jul") { return Some(7); }
            if s.starts_with("ago") || s.starts_with("aug") { return Some(8); }
            if s.starts_with("sep") || s.starts_with("set") { return Some(9); }
            if s.starts_with("oct") { return Some(10); }
            if s.starts_with("nov") { return Some(11); }
            if s.starts_with("dic") || s.starts_with("dec") { return Some(12); }
            None
        }
    }
}

fn parsear_fecha_texto_es(s: &str) -> Option<NaiveDate> {
    // "28 de marzo de 2026", "28 marzo 2026", "28 del marzo 2026"
    // Filtrar palabras "de" y "del" en vez de replace (más seguro)
    let partes: Vec<&str> = s.split_whitespace()
        .filter(|p| *p != "de" && *p != "del")
        .collect();
    if partes.len() >= 3 {
        let dia: u32 = partes[0].parse().ok()?;
        let mes = mes_texto_a_numero(partes[1])?;
        let anio: i32 = partes[2].parse().ok()?;
        return NaiveDate::from_ymd_opt(anio, mes, dia);
    }
    // "28 marzo" (año actual)
    if partes.len() == 2 {
        let dia: u32 = partes[0].parse().ok()?;
        let mes = mes_texto_a_numero(partes[1])?;
        let anio = Local::now().date_naive().year();
        return NaiveDate::from_ymd_opt(anio, mes, dia);
    }
    None
}

fn parsear_fecha_texto_en(s: &str) -> Option<NaiveDate> {
    // "march 28, 2026" o "march 28 2026" o "mar 28"
    let limpio: String = s.replace(',', " ").replace('/', " ");
    let partes: Vec<&str> = limpio.split_whitespace().collect();
    if partes.len() >= 2 {
        let mes = mes_texto_a_numero(partes[0])?;
        let dia: u32 = partes[1].parse().ok()?;
        let anio = if partes.len() >= 3 {
            partes[2].parse::<i32>().ok()?
        } else {
            Local::now().date_naive().year()
        };
        return NaiveDate::from_ymd_opt(anio, mes, dia);
    }
    None
}

fn parse_ddmmyy(s: &str) -> Option<NaiveDate> {
    let d: u32 = s[0..2].parse().ok()?;
    let m: u32 = s[2..4].parse().ok()?;
    let y: i32 = s[4..6].parse::<i32>().ok()? + 2000;
    NaiveDate::from_ymd_opt(y, m, d)
}

fn parse_mmddyy(s: &str) -> Option<NaiveDate> {
    let m: u32 = s[0..2].parse().ok()?;
    let d: u32 = s[2..4].parse().ok()?;
    let y: i32 = s[4..6].parse::<i32>().ok()? + 2000;
    NaiveDate::from_ymd_opt(y, m, d)
}

fn parse_ddmmyyyy(s: &str) -> Option<NaiveDate> {
    let d: u32 = s[0..2].parse().ok()?;
    let m: u32 = s[2..4].parse().ok()?;
    let y: i32 = s[4..8].parse().ok()?;
    NaiveDate::from_ymd_opt(y, m, d)
}

fn parse_mmddyyyy(s: &str) -> Option<NaiveDate> {
    let m: u32 = s[0..2].parse().ok()?;
    let d: u32 = s[2..4].parse().ok()?;
    let y: i32 = s[4..8].parse().ok()?;
    NaiveDate::from_ymd_opt(y, m, d)
}

fn pedir_hora(prompt: &str) -> Option<NaiveTime> {
    println!("    {} Formatos: 14:30, 2:30pm, 6pm, 1430, 6 (=06:00)", "💡".to_string());
    loop {
        let s = pedir_texto_opcional(&format!("{} (vacío=cancelar)", prompt));
        if s.is_empty() { return None; }
        match parsear_hora(&s) {
            Some(h) => {
                println!("    {} Hora: {}", "✓".green(), h.format("%H:%M"));
                return Some(h);
            }
            None => {
                println!("    {} No pude entender esa hora. Intenta otro formato.", "✗".red());
            }
        }
    }
}

fn parsear_hora(input: &str) -> Option<NaiveTime> {
    let s = input.trim().to_lowercase();

    // Detectar am/pm
    let es_pm = s.contains("pm") || s.contains("p.m") || s.contains("p m");
    let es_am = s.contains("am") || s.contains("a.m") || s.contains("a m");
    let limpio: String = s
        .replace("pm", "").replace("am", "")
        .replace("p.m.", "").replace("a.m.", "")
        .replace("p.m", "").replace("a.m", "")
        .replace("p m", "").replace("a m", "")
        .trim().to_string();

    // HH:MM formato estándar
    if let Ok(h) = NaiveTime::parse_from_str(&limpio, "%H:%M") {
        return Some(aplicar_ampm(h, es_am, es_pm));
    }

    // H:MM (ej: 6:30)
    if limpio.contains(':') {
        let partes: Vec<&str> = limpio.split(':').collect();
        if partes.len() == 2 {
            let hora: u32 = partes[0].trim().parse().ok()?;
            let min: u32 = partes[1].trim().parse().ok()?;
            let hora = ajustar_hora_ampm(hora, es_am, es_pm);
            return NaiveTime::from_hms_opt(hora, min, 0);
        }
    }

    // Solo dígitos
    let digits: String = limpio.chars().filter(|c| c.is_ascii_digit()).collect();

    match digits.len() {
        1 | 2 => {
            // "6" → 06:00, "14" → 14:00, "6pm" → 18:00
            let hora: u32 = digits.parse().ok()?;
            let hora = ajustar_hora_ampm(hora, es_am, es_pm);
            NaiveTime::from_hms_opt(hora, 0, 0)
        }
        3 => {
            // "630" → 06:30
            let hora: u32 = digits[0..1].parse().ok()?;
            let min: u32 = digits[1..3].parse().ok()?;
            let hora = ajustar_hora_ampm(hora, es_am, es_pm);
            NaiveTime::from_hms_opt(hora, min, 0)
        }
        4 => {
            // "1430" → 14:30, "0630" → 06:30
            let hora: u32 = digits[0..2].parse().ok()?;
            let min: u32 = digits[2..4].parse().ok()?;
            let hora = ajustar_hora_ampm(hora, es_am, es_pm);
            NaiveTime::from_hms_opt(hora, min, 0)
        }
        _ => None,
    }
}

fn ajustar_hora_ampm(mut hora: u32, es_am: bool, es_pm: bool) -> u32 {
    if es_pm && hora < 12 {
        hora += 12;
    } else if es_am && hora == 12 {
        hora = 0;
    }
    hora
}

fn aplicar_ampm(t: NaiveTime, es_am: bool, es_pm: bool) -> NaiveTime {
    let h = t.format("%H").to_string().parse::<u32>().unwrap_or(0);
    let m = t.format("%M").to_string().parse::<u32>().unwrap_or(0);
    let h = ajustar_hora_ampm(h, es_am, es_pm);
    NaiveTime::from_hms_opt(h, m, 0).unwrap_or(t)
}

fn menu(titulo: &str, opciones: &[&str]) -> Option<usize> {
    println!();
    println!("  {}", "↑↓ navegar, Enter seleccionar, Esc volver".dimmed());
    Select::new()
        .with_prompt(format!("  {}", titulo).bold().to_string())
        .items(opciones)
        .default(0)
        .interact_opt()
        .unwrap_or(None)
}

// ══════════════════════════════════════════════════════════════
//  Dashboard - la vista mágica de todo
// ══════════════════════════════════════════════════════════════

fn dashboard(state: &AppState) {
    let hoy = Local::now().date_naive();
    let dia = hoy.weekday();
    let ahora = Local::now().time();

    println!(
        "  {} {} ({:?}) - {}",
        "📅".to_string(),
        hoy.format("%d/%m/%Y"),
        dia,
        ahora.format("%H:%M")
    );
    println!();

    // Tareas de hoy
    let tareas_hoy = state.tasks.listar_por_fecha(hoy);
    let pendientes = state.tasks.listar_pendientes();
    if !tareas_hoy.is_empty() || !pendientes.is_empty() {
        println!("  {} {}", "📋 Tareas:".yellow().bold(),
            format!("({} hoy, {} pendientes)", tareas_hoy.len(), pendientes.len()).white());
        for t in &tareas_hoy {
            let icono = match t.estado {
                TaskStatus::Completada => "  ✅",
                TaskStatus::EnProgreso => "  🔄",
                TaskStatus::Cancelada => "  ❌",
                TaskStatus::Pendiente => "  ⬜",
            };
            println!("    {} {} - {} {}", icono, t.hora.format("%H:%M"), t.titulo, format!("[{}]", t.prioridad).dimmed());
        }
    }

    // Eventos de hoy
    let eventos_hoy = state.agenda.eventos_del_dia(hoy);
    if !eventos_hoy.is_empty() {
        println!("  {} {}", "📅 Eventos:".green().bold(),
            format!("({} hoy)", eventos_hoy.len()).white());
        for e in &eventos_hoy {
            let fin = e.hora_fin.map(|h| format!("-{}", h.format("%H:%M"))).unwrap_or_default();
            let icono = match e.tipo {
                TipoEvento::Cumpleanos => "🎂",
                TipoEvento::Pago => "💰",
                _ => "📌",
            };
            let concepto_txt = if e.concepto.is_empty() { String::new() } else { format!(" [{}]", e.concepto) };
            println!("    {} {}{} {} ({}){}", icono, e.hora_inicio.format("%H:%M"), fin, e.titulo, e.tipo, concepto_txt.dimmed());
            println!("       {} {} {}  {} {} {}",
                "📆".to_string(),
                "Evento:".dimmed(),
                e.fecha.format("%d/%m/%Y").to_string().cyan(),
                "🕐".to_string(),
                "Registrado:".dimmed(),
                e.creado.format("%d/%m/%Y %H:%M").to_string().dimmed(),
            );
        }
    }

    // Horarios de escritura hoy
    let horarios = state.agenda.horarios_del_dia(dia);
    if !horarios.is_empty() {
        println!("  {}", "✏️  Escritura:".magenta().bold());
        for h in &horarios {
            println!("    🖊️  {}-{} {}", h.hora_inicio.format("%H:%M"), h.hora_fin.format("%H:%M"), h.descripcion);
        }
    }

    // Follow-ups de hoy
    let follow_ups: Vec<_> = state.tasks.listar_follow_ups()
        .into_iter()
        .filter(|t| t.follow_up.map(|f| f.date() == hoy).unwrap_or(false))
        .collect();
    if !follow_ups.is_empty() {
        println!("  {}", "🔔 Follow-ups:".red().bold());
        for t in &follow_ups {
            println!("    ↻ {} ({})", t.titulo, t.follow_up.unwrap().time().format("%H:%M"));
        }
    }

    // Resumen rápido
    println!();
    println!(
        "  {} {} tareas  {} {} eventos  {} {} diagramas  {} {} canvas  {} {} recuerdos",
        "📋".to_string(), state.tasks.tareas.len(),
        "📅".to_string(), state.agenda.eventos.len(),
        "📊".to_string(), state.diagramas.len(),
        "✏️".to_string(), state.canvases.len(),
        "🧠".to_string(), state.memoria.recuerdos.len(),
    );

    if tareas_hoy.is_empty() && eventos_hoy.is_empty() && horarios.is_empty() && follow_ups.is_empty() {
        println!();
        println!("  {}", "✨ Día libre — sin compromisos pendientes".green());
    }

    // Estado de sincronización
    let sync_status = if state.sync.google_autenticado() && state.sync.email_configurado() {
        "🔗 Sync: ✅ Google Calendar + ✅ Email"
    } else if state.sync.google_autenticado() {
        "🔗 Sync: ✅ Google Calendar"
    } else if state.sync.email_configurado() {
        "🔗 Sync: ✅ Email"
    } else {
        ""
    };
    if !sync_status.is_empty() {
        println!("  {}", sync_status.dimmed());
    }
}

// ══════════════════════════════════════════════════════════════
//  Módulo: TAREAS
// ══════════════════════════════════════════════════════════════

fn menu_tareas(state: &mut AppState) {
    loop {
        limpiar();
        separador("📋 TAREAS");

        if !state.tasks.tareas.is_empty() {
            for t in &state.tasks.tareas {
                let icono = match t.estado {
                    TaskStatus::Completada => "✅",
                    TaskStatus::EnProgreso => "🔄",
                    TaskStatus::Cancelada => "❌",
                    TaskStatus::Pendiente => "⬜",
                };
                let fu = t.follow_up.map(|f| format!(" 🔔{}", f.format("%d/%m %H:%M"))).unwrap_or_default();
                println!("  {} {} | {} {} | {} | {}{}",
                    icono,
                    t.id.dimmed(),
                    t.fecha.format("%d/%m"),
                    t.hora.format("%H:%M"),
                    t.titulo,
                    t.prioridad,
                    fu,
                );
            }
        } else {
            println!("  {}", "(vacío — crea tu primera tarea)".dimmed());
        }

        let opciones = &[
            "➕ Nueva tarea",
            "✏️  Editar tarea (estado, fecha, hora, prioridad)",
            "🔔 Programar follow-up",
            "🏷️  Agregar etiqueta / recordar",
            "🗑️  Eliminar tarea",
            "← Volver al menú",
        ];

        match menu("¿Qué deseas hacer?", opciones) {
            Some(0) => nueva_tarea(state),
            Some(1) => editar_tarea(state),
            Some(2) => follow_up_tarea(state),
            Some(3) => recordar_tarea(state),
            Some(4) => eliminar_tarea(state),
            _ => return,
        }
    }
}

fn nueva_tarea(state: &mut AppState) {
    separador("➕ Nueva tarea");
    let titulo = match pedir_texto("Título") { Some(t) => t, None => return };
    let desc = pedir_texto_opcional("Descripción (opcional)");
    let fecha = match pedir_fecha("Fecha") { Some(f) => f, None => return };
    let hora = match pedir_hora("Hora") { Some(h) => h, None => return };

    let prioridades = &["Baja", "Media", "Alta", "⚠ Urgente"];
    let pi = match menu("Prioridad", prioridades) { Some(i) => i, None => return };
    let prioridad = match pi {
        0 => Prioridad::Baja,
        2 => Prioridad::Alta,
        3 => Prioridad::Urgente,
        _ => Prioridad::Media,
    };

    let tags = pedir_texto_opcional("Palabras clave (separadas por coma, opcional)");
    let mut tarea = Task::new(titulo.clone(), desc, fecha, hora, prioridad);

    // Auto-memorizar con palabras clave
    if !tags.is_empty() {
        let palabras: Vec<String> = tags.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        for p in &palabras {
            tarea.agregar_etiqueta(p.clone());
        }
        let recuerdo = Recuerdo::new(
            format!("Tarea: {}", titulo),
            palabras,
        ).con_origen("tarea", &tarea.id);
        state.memoria.agregar_recuerdo(recuerdo);
    }

    println!("\n  {} {}", "✓ Tarea creada:".green().bold(), tarea);
    state.tasks.agregar(tarea);
    pausa();
}

fn editar_tarea(state: &mut AppState) {
    if state.tasks.tareas.is_empty() {
        println!("  {}", "No hay tareas.".yellow());
        pausa();
        return;
    }

    let nombres: Vec<String> = state.tasks.tareas.iter()
        .map(|t| format!("{} - {} [{}] | {} {}", t.id, t.titulo, t.estado, t.fecha.format("%d/%m/%Y"), t.hora.format("%H:%M")))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    let idx = match menu("Selecciona la tarea", &refs) { Some(i) => i, None => return };

    loop {
        let t = &state.tasks.tareas[idx];
        let fu_str = t.follow_up.map(|f| format!(" | 🔔 {}", f.format("%d/%m/%Y %H:%M"))).unwrap_or_default();
        let tags_str = if t.etiquetas.is_empty() { String::new() } else { format!(" | 🏷️  {}", t.etiquetas.join(", ")) };

        println!();
        println!("  {} {}", "Editando:".bold(), t.titulo.bold());
        println!("  📆 {} {} | {} | {}{}{}",
            t.fecha.format("%d/%m/%Y"),
            t.hora.format("%H:%M"),
            t.estado,
            t.prioridad,
            fu_str,
            tags_str);
        println!();

        let opciones_editar = &[
            "✅ Cambiar estado",
            "📆 Cambiar fecha",
            "🕐 Cambiar hora",
            "🔺 Cambiar prioridad",
            "📝 Cambiar título",
            "📄 Cambiar descripción",
            "← Listo, volver",
        ];

        match menu("¿Qué quieres cambiar?", opciones_editar) {
            Some(0) => {
                let estados = &["Pendiente", "En Progreso", "Completada", "Cancelada"];
                if let Some(ei) = menu("Nuevo estado", estados) {
                    let nuevo = match ei {
                        0 => TaskStatus::Pendiente,
                        1 => TaskStatus::EnProgreso,
                        2 => TaskStatus::Completada,
                        3 => TaskStatus::Cancelada,
                        _ => continue,
                    };
                    let es_completada = nuevo == TaskStatus::Completada;
                    state.tasks.tareas[idx].cambiar_estado(nuevo);
                    println!("  {} Estado actualizado", "✓".green().bold());
                    if es_completada {
                        finalizar_tarea(state, idx);
                    }
                }
            }
            Some(1) => {
                if let Some(fecha) = pedir_fecha("Nueva fecha") {
                    state.tasks.tareas[idx].fecha = fecha;
                    state.tasks.tareas[idx].actualizado = chrono::Local::now().naive_local();
                    println!("  {} Fecha actualizada: {}", "✓".green().bold(), fecha.format("%d/%m/%Y"));
                }
            }
            Some(2) => {
                if let Some(hora) = pedir_hora("Nueva hora") {
                    state.tasks.tareas[idx].hora = hora;
                    state.tasks.tareas[idx].actualizado = chrono::Local::now().naive_local();
                    println!("  {} Hora actualizada: {}", "✓".green().bold(), hora.format("%H:%M"));
                }
            }
            Some(3) => {
                let prioridades = &["Baja", "Media", "Alta", "⚠ Urgente"];
                if let Some(pi) = menu("Nueva prioridad", prioridades) {
                    let nueva = match pi {
                        0 => Prioridad::Baja,
                        2 => Prioridad::Alta,
                        3 => Prioridad::Urgente,
                        _ => Prioridad::Media,
                    };
                    state.tasks.tareas[idx].prioridad = nueva;
                    state.tasks.tareas[idx].actualizado = chrono::Local::now().naive_local();
                    println!("  {} Prioridad actualizada", "✓".green().bold());
                }
            }
            Some(4) => {
                if let Some(titulo) = pedir_texto("Nuevo título") {
                    state.tasks.tareas[idx].titulo = titulo;
                    state.tasks.tareas[idx].actualizado = chrono::Local::now().naive_local();
                    println!("  {} Título actualizado", "✓".green().bold());
                }
            }
            Some(5) => {
                let desc = pedir_texto_opcional("Nueva descripción");
                state.tasks.tareas[idx].descripcion = desc;
                state.tasks.tareas[idx].actualizado = chrono::Local::now().naive_local();
                println!("  {} Descripción actualizada", "✓".green().bold());
            }
            _ => return,
        }
    }
}

fn follow_up_tarea(state: &mut AppState) {
    if state.tasks.tareas.is_empty() {
        println!("  {}", "No hay tareas.".yellow());
        pausa();
        return;
    }

    let nombres: Vec<String> = state.tasks.tareas.iter()
        .map(|t| format!("{} - {}", t.id, t.titulo))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    let idx = match menu("¿A cuál tarea?", &refs) { Some(i) => i, None => return };
    let fecha = match pedir_fecha("Fecha del follow-up") { Some(f) => f, None => return };
    let hora = match pedir_hora("Hora del follow-up") { Some(h) => h, None => return };
    let fh = NaiveDateTime::new(fecha, hora);

    state.tasks.tareas[idx].programar_follow_up(fh);
    println!("  {} Follow-up programado: {}", "🔔".to_string(), fh.format("%d/%m/%Y %H:%M"));
    pausa();
}

fn recordar_tarea(state: &mut AppState) {
    if state.tasks.tareas.is_empty() {
        println!("  {}", "No hay tareas.".yellow());
        pausa();
        return;
    }

    let nombres: Vec<String> = state.tasks.tareas.iter()
        .map(|t| format!("{} - {}", t.id, t.titulo))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    let idx = match menu("¿Cuál tarea?", &refs) { Some(i) => i, None => return };
    let palabras = match pedir_texto("Palabras clave para recordar (separadas por coma)") { Some(t) => t, None => return };
    let tags: Vec<String> = palabras.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();

    let tarea = &mut state.tasks.tareas[idx];
    for t in &tags {
        tarea.agregar_etiqueta(t.clone());
    }

    let recuerdo = Recuerdo::new(
        format!("Tarea: {}", tarea.titulo),
        tags,
    ).con_origen("tarea", &tarea.id);
    state.memoria.agregar_recuerdo(recuerdo);

    println!("  {} Palabras clave guardadas en la memoria", "🧠".to_string());
    pausa();
}

fn eliminar_tarea(state: &mut AppState) {
    if state.tasks.tareas.is_empty() {
        println!("  {}", "No hay tareas.".yellow());
        pausa();
        return;
    }

    let nombres: Vec<String> = state.tasks.tareas.iter()
        .map(|t| format!("{} - {}", t.id, t.titulo))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    let idx = match menu("¿Cuál eliminar?", &refs) { Some(i) => i, None => return };
    let nombre = state.tasks.tareas[idx].titulo.clone();

    if Confirm::new().with_prompt(format!("  ¿Eliminar '{}'?", nombre)).default(false).interact().unwrap_or(false) {
        state.tasks.tareas.remove(idx);
        println!("  {} Tarea eliminada", "✓".green());
    }
    pausa();
}

/// Flujo de finalización inteligente: al completar una tarea, ofrece conectarla
/// con palabras clave al diccionario neuronal y crear memoria automática.
fn finalizar_tarea(state: &mut AppState, idx: usize) {
    let tarea = &state.tasks.tareas[idx];
    let titulo = tarea.titulo.clone();
    let tarea_id = tarea.id.clone();
    let etiquetas_existentes = tarea.etiquetas.clone();

    println!();
    println!("  {} {}", "🎉 ¡Tarea completada!".green().bold(), titulo.bold());
    println!();

    // Mostrar sugerencias del diccionario basadas en etiquetas existentes
    if !etiquetas_existentes.is_empty() {
        let sugerencias = state.memoria.diccionario.sugerir(&etiquetas_existentes);
        if !sugerencias.is_empty() {
            println!("  {} Ideas relacionadas en tu diccionario:", "💡".to_string());
            for (idea, fuerza) in sugerencias.iter().take(5) {
                let barra = "█".repeat(*fuerza as usize).cyan();
                println!("    {} {} (fuerza: {})", barra, idea.yellow(), fuerza);
            }
            println!();
        }
    }

    // Preguntar si quiere conectar con palabras clave
    let conectar = Confirm::new()
        .with_prompt("  ¿Deseas conectar esta tarea con palabras clave en tu diccionario de ideas?")
        .default(true)
        .interact()
        .unwrap_or(false);

    if conectar {
        // Mostrar palabras clave existentes del diccionario
        let ideas_existentes = state.memoria.diccionario.todas_las_ideas();
        if !ideas_existentes.is_empty() {
            let mut sorted: Vec<&String> = ideas_existentes;
            sorted.sort();
            println!();
            println!("  {} Tu diccionario tiene: {}", "📚".to_string(),
                sorted.iter().map(|s| s.cyan().to_string()).collect::<Vec<_>>().join(", "));
            println!();
        }

        let input = match pedir_texto("Palabras clave (separadas por coma)") {
            Some(t) => t,
            None => {
                // Aún sin palabras, registrar la tarea con sus etiquetas
                if !etiquetas_existentes.is_empty() {
                    state.memoria.diccionario.registrar(
                        "tarea", &tarea_id, &titulo, &etiquetas_existentes, "completada",
                    );
                }
                return;
            }
        };

        let mut palabras: Vec<String> = input
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        // Incluir etiquetas existentes para reforzar conexiones
        for et in &etiquetas_existentes {
            if !palabras.iter().any(|p| p.to_lowercase() == et.to_lowercase()) {
                palabras.push(et.clone());
            }
        }

        // Pedir nota de contexto opcional
        let nota = pedir_texto_opcional("Nota rápida sobre lo aprendido o logrado (opcional)");

        // Registrar en diccionario neuronal
        state.memoria.diccionario.registrar(
            "tarea", &tarea_id, &titulo, &palabras, &nota,
        );

        // Agregar etiquetas a la tarea
        for p in &palabras {
            state.tasks.tareas[idx].agregar_etiqueta(p.clone());
        }

        // Crear recuerdo automático en memoria
        let contenido_recuerdo = if nota.is_empty() {
            format!("✅ Tarea completada: {}", titulo)
        } else {
            format!("✅ Tarea completada: {} — {}", titulo, nota)
        };
        let recuerdo = Recuerdo::new(contenido_recuerdo, palabras.clone())
            .con_origen("tarea", &tarea_id);
        state.memoria.agregar_recuerdo(recuerdo);

        // Mostrar conexiones resultantes
        println!();
        println!("  {} Guardado en diccionario neuronal:", "🧠".to_string().green().bold());
        println!("    🏷️  {}", palabras.iter().map(|p| p.cyan().to_string()).collect::<Vec<_>>().join(", "));

        let sugerencias = state.memoria.diccionario.sugerir(&palabras);
        if !sugerencias.is_empty() {
            println!();
            println!("  {} Posibles conexiones para explorar:", "🔮".to_string());
            for (idea, fuerza) in sugerencias.iter().take(5) {
                println!("    → {} (fuerza: {})", idea.yellow(), fuerza);
            }
        }

        // Ofrecer enlazar con otro elemento
        let enlazar = Confirm::new()
            .with_prompt("  ¿Enlazar esta tarea con otro elemento (evento, diagrama, canvas)?")
            .default(false)
            .interact()
            .unwrap_or(false);

        if enlazar {
            let modulos = &["📅 Evento", "📊 Diagrama", "✏️  Canvas"];
            if let Some(mi) = menu("¿Con qué módulo?", modulos) {
                let mi_real = mi + 1; // offset porque 0=Tarea en seleccionar_item_de_modulo
                let (mod_dest, id_dest) = seleccionar_item_de_modulo(state, mi_real);
                if !id_dest.is_empty() {
                    let relacion = pedir_texto_opcional("Relación (ej: 'derivó en', 'necesita', 'inspiró')");
                    let rel = if relacion.is_empty() { "completada → conectada".to_string() } else { relacion };
                    state.memoria.enlazar("tarea", &tarea_id, &mod_dest, &id_dest, &rel);
                    println!("  {} Enlace creado", "🔗".to_string().green());
                }
            }
        }
    } else {
        // No quiere conectar, pero si tiene etiquetas, registrar silenciosamente
        if !etiquetas_existentes.is_empty() {
            state.memoria.diccionario.registrar(
                "tarea", &tarea_id, &titulo, &etiquetas_existentes, "completada",
            );
        }
    }

    println!();
    println!("  {} Tarea finalizada y memorizada exitosamente", "✅".to_string().green().bold());
}

// ══════════════════════════════════════════════════════════════
//  Módulo: AGENDA
// ══════════════════════════════════════════════════════════════

fn menu_agenda(state: &mut AppState) {
    loop {
        limpiar();
        separador("📅 AGENDA");

        if !state.agenda.eventos.is_empty() {
            for e in &state.agenda.eventos {
                let fin = e.hora_fin.map(|h| format!("-{}", h.format("%H:%M"))).unwrap_or_default();
                let recur = e.etiqueta_recurrencia();
                let concepto = if e.concepto.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", e.concepto).dimmed().to_string()
                };
                let icono = match e.tipo {
                    TipoEvento::Cumpleanos => "🎂",
                    TipoEvento::Pago => "💰",
                    _ => "📌",
                };
                println!("  {} {} | {}{} | {} ({}){}{}",
                    icono,
                    e.id.dimmed(),
                    e.hora_inicio.format("%H:%M"), fin,
                    e.titulo,
                    e.tipo,
                    recur,
                    concepto,
                );
                println!("      {} {} {}  {} {} {}",
                    "📆".to_string(),
                    "Evento:".dimmed(),
                    e.fecha.format("%d/%m/%Y").to_string().cyan(),
                    "🕐".to_string(),
                    "Registrado:".dimmed(),
                    e.creado.format("%d/%m/%Y %H:%M").to_string().dimmed(),
                );
            }
        } else {
            println!("  {}", "(sin eventos — agenda tu primer evento)".dimmed());
        }

        if !state.agenda.horarios_escritura.is_empty() {
            println!();
            println!("  {}", "✏️  Horarios de escritura:".magenta().bold());
            for h in &state.agenda.horarios_escritura {
                println!("    🖊️  {:?} {}-{} {}", h.dia, h.hora_inicio.format("%H:%M"), h.hora_fin.format("%H:%M"), h.descripcion);
            }
        }

        let opciones = &[
            "📌 Nuevo evento",
            "✏️  Nuevo horario de escritura",
            "� Calendario anual",
            "🗑️  Eliminar evento",
            "🏷️  Recordar evento",
            "← Volver al menú",
        ];

        match menu("¿Qué deseas hacer?", opciones) {
            Some(0) => nuevo_evento(state),
            Some(1) => nuevo_horario(state),
            Some(2) => menu_calendario(state),
            Some(3) => eliminar_evento(state),
            Some(4) => recordar_evento(state),
            _ => return,
        }
    }
}

fn nuevo_evento(state: &mut AppState) {
    separador("📌 Nuevo evento");
    let titulo = match pedir_texto("Título") { Some(t) => t, None => return };
    let desc = pedir_texto_opcional("Descripción (opcional)");

    let tipos = &["Reunión", "Recordatorio", "Follow-Up", "Cita", "🎂 Cumpleaños", "💰 Pago", "Otro"];
    let ti = match menu("Tipo de evento", tipos) { Some(i) => i, None => return };
    let tipo = match ti {
        0 => TipoEvento::Reunion,
        1 => TipoEvento::Recordatorio,
        2 => TipoEvento::FollowUp,
        3 => TipoEvento::Cita,
        4 => TipoEvento::Cumpleanos,
        5 => TipoEvento::Pago,
        _ => TipoEvento::Otro("Otro".to_string()),
    };

    // Para cumpleaños solo pedir la fecha (se repite cada año automáticamente)
    let es_cumple = matches!(tipo, TipoEvento::Cumpleanos);

    let fecha = match pedir_fecha(if es_cumple { "Fecha de nacimiento" } else { "Fecha" }) {
        Some(f) => f,
        None => return,
    };

    let hora = if es_cumple {
        NaiveTime::from_hms_opt(0, 0, 0).unwrap()
    } else {
        match pedir_hora("Hora inicio") { Some(h) => h, None => return }
    };

    let hora_fin = if es_cumple {
        None
    } else {
        let tiene_fin = Confirm::new()
            .with_prompt("  ¿Tiene hora de fin?")
            .default(true)
            .interact()
            .unwrap_or(false);
        if tiene_fin { pedir_hora("Hora fin") } else { None }
    };

    // Frecuencia de recurrencia
    let frecuencia = if es_cumple {
        Frecuencia::Anual
    } else {
        let frecuencias = &[
            "Una sola vez",
            "Semanal",
            "Mensual",
            "Trimestral (cada 3 meses)",
            "Semestral (cada 6 meses)",
            "Anual",
        ];
        let fi = match menu("¿Con qué frecuencia se repite?", frecuencias) { Some(i) => i, None => return };
        match fi {
            0 => Frecuencia::UnaVez,
            1 => Frecuencia::Semanal,
            2 => Frecuencia::Mensual,
            3 => Frecuencia::Trimestral,
            4 => Frecuencia::Semestral,
            5 => Frecuencia::Anual,
            _ => Frecuencia::UnaVez,
        }
    };

    // Concepto: razón o motivo del evento
    let concepto = if es_cumple {
        let persona = pedir_texto_opcional("¿De quién es el cumpleaños? (concepto)");
        if persona.is_empty() { titulo.clone() } else { persona }
    } else {
        pedir_texto_opcional("Concepto / razón del evento (opcional)")
    };

    let tags = pedir_texto_opcional("Palabras clave (opcional, separadas por coma)");

    let mut evento = Evento::new(titulo.clone(), desc, tipo, fecha, hora, hora_fin);
    evento = evento.con_frecuencia(frecuencia).con_concepto(concepto);

    if !tags.is_empty() {
        let palabras: Vec<String> = tags.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        let recuerdo = Recuerdo::new(
            format!("Evento: {}", titulo),
            palabras,
        ).con_origen("evento", &evento.id);
        state.memoria.agregar_recuerdo(recuerdo);
    }

    println!("\n  {} {}", "✓ Evento creado:".green().bold(), evento);
    state.agenda.agregar_evento(evento);
    pausa();
}

fn nuevo_horario(state: &mut AppState) {
    separador("✏️  Nuevo horario de escritura");
    let dias = &["Lunes", "Martes", "Miércoles", "Jueves", "Viernes", "Sábado", "Domingo"];
    let di = match menu("Día de la semana", dias) { Some(i) => i, None => return };
    let dia = match di {
        0 => chrono::Weekday::Mon,
        1 => chrono::Weekday::Tue,
        2 => chrono::Weekday::Wed,
        3 => chrono::Weekday::Thu,
        4 => chrono::Weekday::Fri,
        5 => chrono::Weekday::Sat,
        _ => chrono::Weekday::Sun,
    };

    let inicio = match pedir_hora("Hora inicio") { Some(h) => h, None => return };
    let fin = match pedir_hora("Hora fin") { Some(h) => h, None => return };
    let desc = pedir_texto_opcional("Descripción");
    let desc = if desc.is_empty() { "Sesión de escritura".to_string() } else { desc };

    let horario = HorarioEscritura::new(dia, inicio, fin, desc);
    println!("  {} {}", "✓ Horario creado:".green().bold(), horario);
    state.agenda.agregar_horario(horario);
    pausa();
}

fn eliminar_evento(state: &mut AppState) {
    if state.agenda.eventos.is_empty() {
        println!("  {}", "No hay eventos.".yellow());
        pausa();
        return;
    }

    let nombres: Vec<String> = state.agenda.eventos.iter()
        .map(|e| format!("{} - {} ({})", e.id, e.titulo, e.tipo))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    let idx = match menu("¿Cuál eliminar?", &refs) { Some(i) => i, None => return };
    let nombre = state.agenda.eventos[idx].titulo.clone();

    if Confirm::new().with_prompt(format!("  ¿Eliminar '{}'?", nombre)).default(false).interact().unwrap_or(false) {
        state.agenda.eventos.remove(idx);
        println!("  {} Evento eliminado", "✓".green());
    }
    pausa();
}

fn recordar_evento(state: &mut AppState) {
    if state.agenda.eventos.is_empty() {
        println!("  {}", "No hay eventos.".yellow());
        pausa();
        return;
    }

    let nombres: Vec<String> = state.agenda.eventos.iter()
        .map(|e| format!("{} - {}", e.id, e.titulo))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    let idx = match menu("¿Cuál evento?", &refs) { Some(i) => i, None => return };
    let palabras = match pedir_texto("Palabras clave para recordar (separadas por coma)") { Some(t) => t, None => return };
    let tags: Vec<String> = palabras.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();

    let evento = &state.agenda.eventos[idx];
    let recuerdo = Recuerdo::new(
        format!("Evento: {}", evento.titulo),
        tags,
    ).con_origen("evento", &evento.id);
    state.memoria.agregar_recuerdo(recuerdo);

    println!("  {} Guardado en la memoria", "🧠".to_string());
    pausa();
}

// ══════════════════════════════════════════════════════════════
//  Módulo: CALENDARIO ANUAL
// ══════════════════════════════════════════════════════════════

fn es_bisiesto(anio: i32) -> bool {
    (anio % 4 == 0 && anio % 100 != 0) || anio % 400 == 0
}

fn dias_en_anio(anio: i32) -> u32 {
    if es_bisiesto(anio) { 366 } else { 365 }
}

fn nombre_mes(mes: u32) -> &'static str {
    match mes {
        1 => "ENERO", 2 => "FEBRERO", 3 => "MARZO", 4 => "ABRIL",
        5 => "MAYO", 6 => "JUNIO", 7 => "JULIO", 8 => "AGOSTO",
        9 => "SEPTIEMBRE", 10 => "OCTUBRE", 11 => "NOVIEMBRE", 12 => "DICIEMBRE",
        _ => "",
    }
}

fn dias_en_mes(anio: i32, mes: u32) -> u32 {
    match mes {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if es_bisiesto(anio) { 29 } else { 28 },
        _ => 0,
    }
}

fn colorear_dia(texto: &str, fecha: NaiveDate, es_hoy: bool, tiene_evento: bool, marcas: &[&DiaMarcado]) -> String {
    let es_finde = matches!(fecha.weekday(), Weekday::Sat | Weekday::Sun);
    if es_hoy {
        texto.on_white().black().bold().to_string()
    } else if !marcas.is_empty() {
        match &marcas[0].tipo {
            TipoDiaMarcado::Libre => texto.green().to_string(),
            TipoDiaMarcado::Feriado => texto.red().bold().to_string(),
            TipoDiaMarcado::Vacaciones => texto.cyan().to_string(),
            TipoDiaMarcado::Vencimiento => texto.yellow().bold().to_string(),
            TipoDiaMarcado::Importante => texto.magenta().bold().to_string(),
            TipoDiaMarcado::Otro(_) => texto.blue().to_string(),
        }
    } else if tiene_evento {
        texto.yellow().to_string()
    } else if es_finde {
        texto.red().dimmed().to_string()
    } else {
        texto.normal().to_string()
    }
}

const ENCABEZADO_DIAS: &str = " Lu  Ma  Mi  Ju  Vi  Sá  Do ";

fn centrar_texto(texto: &str, ancho: usize) -> (String, String) {
    let visible = texto.chars().count();
    let total_pad = ancho.saturating_sub(visible);
    let izq = total_pad / 2;
    let der = total_pad - izq;
    (" ".repeat(izq), " ".repeat(der))
}

fn imprimir_mes(anio: i32, mes: u32, state: &AppState) {
    let total_dias = dias_en_mes(anio, mes);
    let primer_dia = NaiveDate::from_ymd_opt(anio, mes, 1).unwrap();
    let offset = primer_dia.weekday().num_days_from_monday() as usize;
    let hoy = Local::now().date_naive();

    let header = format!("{} {}", nombre_mes(mes), anio);
    let (pad_i, pad_d) = centrar_texto(&header, 28);
    println!("    {}{}{}", pad_i, header.cyan().bold(), pad_d);
    println!("    {}", ENCABEZADO_DIAS.dimmed());
    print!("    ");

    let mut col = 0usize;
    for _ in 0..offset {
        print!("    ");
        col += 1;
    }

    for dia in 1..=total_dias {
        let fecha = NaiveDate::from_ymd_opt(anio, mes, dia).unwrap();
        let es_hoy = fecha == hoy;
        let tiene_evento = state.agenda.eventos.iter().any(|e| e.ocurre_en(fecha));
        let marcas = state.agenda.marcas_del_dia(fecha);
        let texto = format!("{:>3}", dia);
        let celda = colorear_dia(&texto, fecha, es_hoy, tiene_evento, &marcas);

        print!("{} ", celda);
        col += 1;

        if col == 7 {
            println!();
            print!("    ");
            col = 0;
        }
    }
    println!();
}

fn imprimir_calendario_anual(anio: i32, state: &AppState) {
    let bisiesto = if es_bisiesto(anio) { " (BISIESTO)" } else { "" };
    println!(
        "  {}",
        format!(
            "📆 CALENDARIO {} — {} días{}",
            anio,
            dias_en_anio(anio),
            bisiesto
        ).cyan().bold()
    );
    println!();

    // Leyenda de colores
    println!("  {} Hoy  {} Fin de semana  {} Evento  {} Libre  {} Feriado  {} Vacaciones  {} Vencimiento  {} Importante",
        "██".on_white().black(),
        "██".red().dimmed(),
        "██".yellow(),
        "██".green(),
        "██".red().bold(),
        "██".cyan(),
        "██".yellow().bold(),
        "██".magenta().bold(),
    );
    println!();

    let separador_col = "   "; // 3 espacios entre columnas de meses

    // Mostrar 3 meses por fila
    for fila in 0..4 {
        let meses: Vec<u32> = (1..=3).map(|m| fila * 3 + m).collect();

        // Encabezados de mes — centrar ANTES de aplicar color
        print!("  ");
        for (i, &mes) in meses.iter().enumerate() {
            if i > 0 { print!("{}", separador_col); }
            let header = format!("{} {}", nombre_mes(mes), anio);
            let (pad_i, pad_d) = centrar_texto(&header, 28);
            print!("{}{}{}", pad_i, header.cyan().bold(), pad_d);
        }
        println!();

        // Días de la semana
        print!("  ");
        for (i, _) in meses.iter().enumerate() {
            if i > 0 { print!("{}", separador_col); }
            print!("{}", ENCABEZADO_DIAS.dimmed());
        }
        println!();

        // Líneas de días
        let lineas_mes: Vec<Vec<String>> = meses.iter()
            .map(|&m| generar_lineas_mes(anio, m, state))
            .collect();

        let max_lineas = lineas_mes.iter().map(|l| l.len()).max().unwrap_or(0);
        for fila_linea in 0..max_lineas {
            print!("  ");
            for (i, lineas) in lineas_mes.iter().enumerate() {
                if i > 0 { print!("{}", separador_col); }
                if fila_linea < lineas.len() {
                    print!("{}", lineas[fila_linea]);
                } else {
                    print!("{}", " ".repeat(28));
                }
            }
            println!();
        }
        println!();
    }
}

fn generar_lineas_mes(anio: i32, mes: u32, state: &AppState) -> Vec<String> {
    let total_dias = dias_en_mes(anio, mes);
    let primer_dia = NaiveDate::from_ymd_opt(anio, mes, 1).unwrap();
    let offset = primer_dia.weekday().num_days_from_monday() as usize;
    let hoy = Local::now().date_naive();

    let mut lineas: Vec<String> = Vec::new();
    let mut linea = String::new();
    let mut col = 0usize;

    // Celdas vacías al inicio
    for _ in 0..offset {
        linea.push_str("    ");
        col += 1;
    }

    for dia in 1..=total_dias {
        let fecha = NaiveDate::from_ymd_opt(anio, mes, dia).unwrap();
        let es_hoy = fecha == hoy;
        let tiene_evento = state.agenda.eventos.iter().any(|e| e.ocurre_en(fecha));
        let marcas = state.agenda.marcas_del_dia(fecha);
        let texto = format!("{:>3}", dia);
        let celda = colorear_dia(&texto, fecha, es_hoy, tiene_evento, &marcas);

        linea.push_str(&format!("{} ", celda));
        col += 1;

        if col == 7 {
            lineas.push(linea);
            linea = String::new();
            col = 0;
        }
    }

    // Rellenar última línea incompleta hasta 28 caracteres visibles
    if col > 0 {
        for _ in col..7 {
            linea.push_str("    ");
        }
        lineas.push(linea);
    }

    lineas
}

fn calcular_diferencia_fechas() {
    separador("📏 Calcular distancia entre fechas");

    let desde = match pedir_fecha("Fecha inicio") { Some(f) => f, None => return };
    let hasta = match pedir_fecha("Fecha fin") { Some(f) => f, None => return };

    let dias = (hasta - desde).num_days();
    let semanas = dias / 7;
    let dias_restantes = dias % 7;
    let meses_aprox = dias as f64 / 30.44;

    println!();
    println!("  {} {} → {}", "📅".to_string(), desde.format("%d/%m/%Y"), hasta.format("%d/%m/%Y"));
    println!();
    println!("  {} {} días calendario", "📏".to_string(), dias.to_string().cyan().bold());
    println!("  {} {} semanas y {} días", "📅".to_string(),
        semanas.to_string().cyan().bold(),
        dias_restantes.to_string().cyan()
    );
    println!("  {} ~{:.1} meses", "🗓️".to_string(), meses_aprox);

    // Contar fines de semana
    let mut fines_semana = 0i64;
    let mut dias_laborales = 0i64;
    let mut fecha = desde;
    let fin = hasta;
    while fecha <= fin {
        if matches!(fecha.weekday(), Weekday::Sat | Weekday::Sun) {
            fines_semana += 1;
        } else {
            dias_laborales += 1;
        }
        fecha += Duration::days(1);
    }
    println!("  {} {} días laborales, {} fines de semana", "🏢".to_string(),
        dias_laborales.to_string().green(),
        fines_semana.to_string().red()
    );
    println!();
    pausa();
}

fn avanzar_semanas() {
    separador("📐 Avanzar semanas/días desde una fecha");

    let desde = match pedir_fecha("Fecha base") { Some(f) => f, None => return };

    let opciones = &["Semanas", "Días", "Meses"];
    let unidad = match menu("¿Qué unidad avanzar?", opciones) { Some(i) => i, None => return };

    let cantidad_str = pedir_texto_opcional("Cantidad");
    let cantidad: i64 = match cantidad_str.parse() {
        Ok(n) => n,
        Err(_) => { println!("  {} Número inválido", "✗".red()); pausa(); return; }
    };

    let resultado = match unidad {
        0 => desde + Duration::weeks(cantidad),
        1 => desde + Duration::days(cantidad),
        2 => {
            // Avanzar meses
            let meses_totales = desde.month0() as i64 + cantidad;
            let anio_extra = meses_totales / 12;
            let mes_nuevo = (meses_totales % 12) as u32 + 1;
            let anio_nuevo = desde.year() as i64 + anio_extra;
            let dia = desde.day().min(dias_en_mes(anio_nuevo as i32, mes_nuevo));
            NaiveDate::from_ymd_opt(anio_nuevo as i32, mes_nuevo, dia).unwrap_or(desde)
        }
        _ => desde,
    };

    let nombre_unidad = match unidad { 0 => "semanas", 1 => "días", _ => "meses" };

    println!();
    println!("  {} {} + {} {} = {}", "📅".to_string(),
        desde.format("%d/%m/%Y"),
        cantidad.to_string().cyan().bold(),
        nombre_unidad,
        resultado.format("%A %d de %B de %Y").to_string().green().bold()
    );

    let dias_diff = (resultado - desde).num_days().abs();
    println!("  {} ({} días calendario)", "📏".to_string(), dias_diff);
    println!();
    pausa();
}

fn marcar_dia_calendario(state: &mut AppState) {
    separador("🎨 Marcar día en el calendario");

    let opciones_modo = &["Marcar un día específico", "Marcar un rango de fechas", "Limpiar marcas de un día"];
    let modo = match menu("¿Qué deseas hacer?", opciones_modo) { Some(i) => i, None => return };

    if modo == 2 {
        let fecha = match pedir_fecha("Fecha a limpiar") { Some(f) => f, None => return };
        state.agenda.limpiar_marcas(fecha);
        println!("  {} Marcas eliminadas para {}", "✓".green(), fecha.format("%d/%m/%Y"));
        pausa();
        return;
    }

    let tipos = &["Libre (verde)", "Feriado (rojo)", "Vacaciones (cyan)", "Vencimiento (amarillo)", "Importante (magenta)", "Otro (azul)"];
    let ti = match menu("Tipo de marca", tipos) { Some(i) => i, None => return };
    let tipo = match ti {
        0 => TipoDiaMarcado::Libre,
        1 => TipoDiaMarcado::Feriado,
        2 => TipoDiaMarcado::Vacaciones,
        3 => TipoDiaMarcado::Vencimiento,
        4 => TipoDiaMarcado::Importante,
        _ => {
            let nombre = pedir_texto_opcional("Nombre del tipo");
            TipoDiaMarcado::Otro(if nombre.is_empty() { "Otro".to_string() } else { nombre })
        }
    };

    let nota = pedir_texto_opcional("Nota (opcional)");

    if modo == 0 {
        let fecha = match pedir_fecha("Fecha") { Some(f) => f, None => return };
        state.agenda.marcar_dia(DiaMarcado { fecha, tipo, nota });
        println!("  {} Día {} marcado", "✓".green(), fecha.format("%d/%m/%Y"));
    } else {
        let desde = match pedir_fecha("Desde") { Some(f) => f, None => return };
        let hasta = match pedir_fecha("Hasta") { Some(f) => f, None => return };
        let dias = (hasta - desde).num_days().abs() + 1;
        state.agenda.marcar_rango(desde, hasta, tipo, nota);
        println!("  {} {} días marcados ({} → {})", "✓".green(), dias,
            desde.format("%d/%m/%Y"), hasta.format("%d/%m/%Y"));
    }
    pausa();
}

fn ver_mes_detallado(state: &AppState) {
    separador("🔍 Ver mes en detalle");

    let hoy = Local::now().date_naive();
    let anio_str = pedir_texto_opcional(&format!("Año (Enter={})", hoy.year()));
    let anio: i32 = if anio_str.is_empty() { hoy.year() } else {
        match anio_str.parse() { Ok(a) => a, Err(_) => { println!("  {} Año inválido", "✗".red()); pausa(); return; } }
    };

    let meses_nombres = &["Enero","Febrero","Marzo","Abril","Mayo","Junio","Julio","Agosto","Septiembre","Octubre","Noviembre","Diciembre"];
    let default_mes = (hoy.month() - 1) as usize;
    let mi = Select::new()
        .with_prompt("  Mes")
        .items(meses_nombres)
        .default(default_mes)
        .interact_opt()
        .unwrap_or(None);
    let mes = match mi { Some(i) => (i + 1) as u32, None => return };

    limpiar();
    println!();
    imprimir_mes(anio, mes, state);
    println!();

    // Listar eventos del mes
    let total_dias = dias_en_mes(anio, mes);
    let mut hay_info = false;

    for dia in 1..=total_dias {
        let fecha = NaiveDate::from_ymd_opt(anio, mes, dia).unwrap();
        let eventos: Vec<_> = state.agenda.eventos.iter().filter(|e| e.ocurre_en(fecha)).collect();
        let marcas = state.agenda.marcas_del_dia(fecha);

        if !eventos.is_empty() || !marcas.is_empty() {
            hay_info = true;
            let dia_nombre = match fecha.weekday() {
                Weekday::Mon => "Lun", Weekday::Tue => "Mar", Weekday::Wed => "Mié",
                Weekday::Thu => "Jue", Weekday::Fri => "Vie", Weekday::Sat => "Sáb", Weekday::Sun => "Dom",
            };
            println!("  {} {} {}:", "📌".to_string(), fecha.format("%d/%m"), dia_nombre);
            for e in &eventos {
                let fin = e.hora_fin.map(|h| format!("-{}", h.format("%H:%M"))).unwrap_or_default();
                let icono = match e.tipo {
                    TipoEvento::Cumpleanos => "🎂",
                    TipoEvento::Pago => "💰",
                    _ => "📅",
                };
                let recur = e.etiqueta_recurrencia();
                let concepto_txt = if e.concepto.is_empty() { String::new() } else { format!(" [{}]", e.concepto) };
                println!("      {} {}{} {}{}{}", icono, e.hora_inicio.format("%H:%M"), fin, e.titulo, recur, concepto_txt.dimmed());
                println!("         {} {} {}  {} {} {}",
                    "📆".to_string(),
                    "Evento:".dimmed(),
                    e.fecha.format("%d/%m/%Y").to_string().cyan(),
                    "🕐".to_string(),
                    "Registrado:".dimmed(),
                    e.creado.format("%d/%m/%Y %H:%M").to_string().dimmed(),
                );
            }
            for m in &marcas {
                let nota_txt = if m.nota.is_empty() { String::new() } else { format!(" — {}", m.nota) };
                println!("      🎨 {}{}", m.tipo, nota_txt);
            }
        }
    }

    if !hay_info {
        println!("  {}", "(sin eventos ni marcas en este mes)".dimmed());
    }

    println!();
    pausa();
}

fn ver_resumen_trimestral(state: &AppState) {
    separador("📊 Resumen trimestral");

    let hoy = Local::now().date_naive();
    let anio = hoy.year();
    let trimestre_actual = ((hoy.month() - 1) / 3) as usize;

    let trimestres = &["Q1 (Ene-Mar)", "Q2 (Abr-Jun)", "Q3 (Jul-Sep)", "Q4 (Oct-Dic)"];
    let qi = Select::new()
        .with_prompt("  Trimestre")
        .items(trimestres)
        .default(trimestre_actual)
        .interact_opt()
        .unwrap_or(None);
    let q = match qi { Some(i) => i, None => return };

    let mes_inicio = (q as u32) * 3 + 1;
    let mes_fin = mes_inicio + 2;

    limpiar();
    println!();
    println!("  {}", format!("📊 {} {} — {}", trimestres[q], anio,
        if es_bisiesto(anio) { "Año bisiesto" } else { "Año regular" }).cyan().bold());
    println!();

    let mut total_eventos = 0;
    let mut total_marcas = 0;
    let mut dias_libres = 0;
    let mut dias_feriado = 0;

    for mes in mes_inicio..=mes_fin {
        let total_dias = dias_en_mes(anio, mes);
        imprimir_mes(anio, mes, state);
        println!();

        for dia in 1..=total_dias {
            let fecha = NaiveDate::from_ymd_opt(anio, mes, dia).unwrap();
            total_eventos += state.agenda.eventos.iter().filter(|e| e.ocurre_en(fecha)).count();
            let marcas = state.agenda.marcas_del_dia(fecha);
            total_marcas += marcas.len();
            for m in &marcas {
                match m.tipo {
                    TipoDiaMarcado::Libre => dias_libres += 1,
                    TipoDiaMarcado::Feriado => dias_feriado += 1,
                    _ => {}
                }
            }
        }
    }

    println!("  {} {} eventos programados", "📌".to_string(), total_eventos.to_string().cyan().bold());
    println!("  {} {} días marcados ({} libres, {} feriados)", "🎨".to_string(),
        total_marcas.to_string().cyan(), dias_libres.to_string().green(), dias_feriado.to_string().red());
    println!();
    pausa();
}

fn menu_calendario(state: &mut AppState) {
    loop {
        limpiar();
        let hoy = Local::now().date_naive();
        let anio = hoy.year();

        imprimir_calendario_anual(anio, state);

        let opciones = &[
            "📆 Ver otro año",
            "🔍 Ver mes en detalle",
            "📏 Calcular distancia entre fechas",
            "📐 Avanzar semanas/días/meses desde fecha",
            "🎨 Marcar días (libre, feriado, vacaciones...)",
            "📊 Resumen trimestral",
            "← Volver",
        ];

        match menu("Calendario", opciones) {
            Some(0) => {
                let anio_str = pedir_texto_opcional("Año a mostrar");
                if let Ok(a) = anio_str.parse::<i32>() {
                    limpiar();
                    imprimir_calendario_anual(a, state);
                    pausa();
                }
            }
            Some(1) => ver_mes_detallado(state),
            Some(2) => calcular_diferencia_fechas(),
            Some(3) => avanzar_semanas(),
            Some(4) => marcar_dia_calendario(state),
            Some(5) => ver_resumen_trimestral(state),
            _ => return,
        }
    }
}

// ══════════════════════════════════════════════════════════════
//  Módulo: CANVAS
// ══════════════════════════════════════════════════════════════

fn menu_canvas(state: &mut AppState) {
    loop {
        limpiar();
        separador("🎨 CANVAS — Board de Ideas");

        if !state.canvases.is_empty() {
            for c in &state.canvases {
                println!("  🖼️  [{}] {} — {} elementos", c.id.dimmed(), c.nombre, c.total_elementos());
            }
        } else {
            println!("  {}", "(sin canvas — crea tu primer board de ideas)".dimmed());
        }

        let opciones = &[
            "🖼️  Nuevo canvas",
            "📝 Agregar nota / idea",
            "🖼️  Agregar imagen",
            "📋 Agregar lista",
            "── Agregar sección",
            "👁️  Ver canvas completo",
            "✏️  Editar elemento",
            "🗑️  Eliminar elemento",
            "🌐 Exportar a HTML (abrir en navegador)",
            "🗑️  Eliminar canvas completo",
            "← Volver al menú",
        ];

        match menu("¿Qué deseas hacer?", opciones) {
            Some(0) => nuevo_canvas(state),
            Some(1) => agregar_nota_canvas(state),
            Some(2) => agregar_imagen_canvas(state),
            Some(3) => agregar_lista_canvas(state),
            Some(4) => agregar_seccion_canvas(state),
            Some(5) => ver_canvas(state),
            Some(6) => editar_elemento_canvas(state),
            Some(7) => eliminar_elemento_canvas(state),
            Some(8) => exportar_canvas_html(state),
            Some(9) => eliminar_canvas(state),
            _ => return,
        }
    }
}

fn nuevo_canvas(state: &mut AppState) {
    separador("🖼️  Nuevo canvas / board");
    let nombre = match pedir_texto("Nombre (ej: Ideas proyecto, Brainstorm, Inspiración)") { Some(t) => t, None => return };
    let c = Canvas::new(nombre.clone(), 800, 600);
    println!("  {} [{}] {}", "✓ Canvas creado:".green().bold(), c.id, nombre);
    state.canvases.push(c);
    pausa();
}

fn seleccionar_canvas(state: &AppState) -> Option<usize> {
    if state.canvases.is_empty() {
        println!("  {}", "No hay canvases creados.".yellow());
        pausa();
        return None;
    }
    let nombres: Vec<String> = state.canvases.iter()
        .map(|c| format!("[{}] {} ({} elementos)", c.id, c.nombre, c.total_elementos()))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
    menu("Selecciona canvas", &refs)
}

fn agregar_nota_canvas(state: &mut AppState) {
    let idx = match seleccionar_canvas(state) { Some(i) => i, None => return };
    println!("  Escribe tu nota o idea. Puede ser larga, un resumen,");
    println!("  una cita, lo que quieras poner en el board.");
    let contenido = match pedir_texto("Nota") { Some(t) => t, None => return };

    let colores = &["🔵 Azul", "🟢 Verde", "🟡 Amarillo", "🔴 Rojo", "🟣 Morado", "⚪ Blanco"];
    let ci = match menu("Color de la nota", colores) { Some(i) => i, None => return };
    let color = match ci {
        0 => "#00d4ff",
        1 => "#4ecdc4",
        2 => "#f9ca24",
        3 => "#ff6b6b",
        4 => "#a29bfe",
        _ => "#ffffff",
    };

    state.canvases[idx].agregar_elemento(Elemento::nota(contenido, color.to_string()));
    println!("  {} Nota agregada al canvas", "✓".green().bold());
    pausa();
}

fn agregar_imagen_canvas(state: &mut AppState) {
    let idx = match seleccionar_canvas(state) { Some(i) => i, None => return };
    println!("  Ingresa la ruta al archivo de imagen o una URL.");
    println!("  Ejemplos:");
    println!("    C:\\Users\\fotos\\idea.png");
    println!("    /storage/emulated/0/DCIM/foto.jpg");
    println!("    https://ejemplo.com/imagen.png");
    let ruta = match pedir_texto("Ruta o URL de la imagen") { Some(t) => t, None => return };

    // Verificar si es archivo local
    if !ruta.starts_with("http://") && !ruta.starts_with("https://") {
        if !std::path::Path::new(&ruta).exists() {
            println!("  {} El archivo '{}' no existe. ¿Agregar de todos modos?", "⚠️".to_string(), ruta);
            if !Confirm::new()
                .with_prompt("  ¿Continuar?")
                .default(false)
                .interact()
                .unwrap_or(false)
            {
                return;
            }
        }
    }

    state.canvases[idx].agregar_elemento(Elemento::imagen(ruta));
    println!("  {} Imagen agregada al canvas", "✓".green().bold());
    pausa();
}

fn agregar_lista_canvas(state: &mut AppState) {
    let idx = match seleccionar_canvas(state) { Some(i) => i, None => return };
    println!("  Escribe los items separados por coma o punto y coma.");
    let items_str = match pedir_texto("Items (separados por , o ;)") { Some(t) => t, None => return };

    let items: Vec<&str> = items_str.split(|c| c == ',' || c == ';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if items.is_empty() {
        println!("  {}", "No se ingresaron items.".yellow());
        pausa();
        return;
    }

    let contenido = items.join("\n");

    let colores = &["🟢 Verde", "🔵 Azul", "🟡 Amarillo", "⚪ Blanco"];
    let ci = match menu("Color", colores) { Some(i) => i, None => return };
    let color = match ci {
        0 => "#4ecdc4",
        1 => "#00d4ff",
        2 => "#f9ca24",
        _ => "#ffffff",
    };

    state.canvases[idx].agregar_elemento(Elemento::lista(contenido, color.to_string()));
    println!("  {} Lista con {} items agregada", "✓".green().bold(), items.len());
    pausa();
}

fn agregar_seccion_canvas(state: &mut AppState) {
    let idx = match seleccionar_canvas(state) { Some(i) => i, None => return };
    let titulo = match pedir_texto("Título de la sección") { Some(t) => t, None => return };
    state.canvases[idx].agregar_elemento(Elemento::seccion(titulo));
    println!("  {} Sección agregada", "✓".green().bold());
    pausa();
}

fn ver_canvas(state: &AppState) {
    let idx = match seleccionar_canvas(state) { Some(i) => i, None => return };
    let c = &state.canvases[idx];
    separador(&format!("🎨 {}", c.nombre));

    if c.elementos.is_empty() && c.trazos.is_empty() {
        println!("  {}", "(canvas vacío)".dimmed());
    } else {
        for elem in &c.elementos {
            match &elem.tipo {
                omniplanner::canvas::TipoElemento::Nota => {
                    println!("  📝 [{}] {}", elem.id.dimmed(), elem.contenido);
                    println!("     {}", elem.creado.format("%d/%m/%Y %H:%M").to_string().dimmed());
                }
                omniplanner::canvas::TipoElemento::Imagen => {
                    println!("  🖼️  [{}] {}", elem.id.dimmed(), elem.contenido);
                    println!("     {}", elem.creado.format("%d/%m/%Y %H:%M").to_string().dimmed());
                }
                omniplanner::canvas::TipoElemento::Lista => {
                    println!("  📋 [{}] Lista:", elem.id.dimmed());
                    for (i, item) in elem.contenido.lines().enumerate() {
                        println!("     {}. {}", i + 1, item);
                    }
                    println!("     {}", elem.creado.format("%d/%m/%Y %H:%M").to_string().dimmed());
                }
                omniplanner::canvas::TipoElemento::Seccion => {
                    println!();
                    println!("  {} {} {}", "──".dimmed(), elem.contenido.bold(), "──".dimmed());
                }
            }
            println!();
        }
        if !c.trazos.is_empty() {
            println!("  {} {} trazos legacy", "✏️".to_string(), c.trazos.len());
        }
    }
    pausa();
}

fn editar_elemento_canvas(state: &mut AppState) {
    let idx = match seleccionar_canvas(state) { Some(i) => i, None => return };
    if state.canvases[idx].elementos.is_empty() {
        println!("  {}", "No hay elementos para editar.".yellow());
        pausa();
        return;
    }

    let nombres: Vec<String> = state.canvases[idx].elementos.iter()
        .map(|e| format!("[{}] {}", e.id, e))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
    let ei = match menu("¿Cuál elemento?", &refs) { Some(i) => i, None => return };

    let nuevo = match pedir_texto("Nuevo contenido") { Some(t) => t, None => return };
    state.canvases[idx].elementos[ei].contenido = nuevo;
    println!("  {} Elemento actualizado", "✓".green().bold());
    pausa();
}

fn eliminar_elemento_canvas(state: &mut AppState) {
    let idx = match seleccionar_canvas(state) { Some(i) => i, None => return };
    if state.canvases[idx].elementos.is_empty() {
        println!("  {}", "No hay elementos para eliminar.".yellow());
        pausa();
        return;
    }

    let nombres: Vec<String> = state.canvases[idx].elementos.iter()
        .map(|e| format!("[{}] {}", e.id, e))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
    let ei = match menu("¿Cuál eliminar?", &refs) { Some(i) => i, None => return };

    let id = state.canvases[idx].elementos[ei].id.clone();
    state.canvases[idx].eliminar_elemento(&id);
    println!("  {} Elemento eliminado", "✓".green().bold());
    pausa();
}

fn exportar_canvas_html(state: &AppState) {
    let idx = match seleccionar_canvas(state) { Some(i) => i, None => return };
    let nombre_archivo = format!("{}.html", state.canvases[idx].nombre.replace(' ', "_").to_lowercase());
    let sugerencia = pedir_texto_opcional(&format!("Archivo (Enter = {})", nombre_archivo));
    let archivo = if sugerencia.is_empty() { nombre_archivo } else { sugerencia };

    let html = state.canvases[idx].exportar_html();
    match std::fs::write(&archivo, &html) {
        Ok(_) => {
            println!("  {} Exportado a '{}'", "✓".green(), archivo);
            if Confirm::new()
                .with_prompt("  ¿Abrir en navegador?")
                .default(true)
                .interact()
                .unwrap_or(false)
            {
                let _ = open::that(&archivo);
            }
        }
        Err(e) => println!("  {} Error: {}", "✗".red(), e),
    }
    pausa();
}

fn eliminar_canvas(state: &mut AppState) {
    let idx = match seleccionar_canvas(state) { Some(i) => i, None => return };
    let nombre = state.canvases[idx].nombre.clone();

    if Confirm::new()
        .with_prompt(&format!("  ¿Eliminar canvas '{}'? Esto no se puede deshacer", nombre))
        .default(false)
        .interact()
        .unwrap_or(false)
    {
        state.canvases.remove(idx);
        println!("  {} Canvas '{}' eliminado", "✓".green().bold(), nombre);
    }
    pausa();
}

// ══════════════════════════════════════════════════════════════
//  Módulo: DIAGRAMAS
// ══════════════════════════════════════════════════════════════

fn menu_diagramas(state: &mut AppState) {
    loop {
        limpiar();
        separador("📊 DIAGRAMAS");

        if !state.diagramas.is_empty() {
            for d in &state.diagramas {
                println!("  📊 [{}] {} — {} | {} nodos, {} conexiones",
                    d.id.dimmed(), d.nombre, d.tipo, d.nodos.len(), d.conexiones.len());
            }
        } else {
            println!("  {}", "(sin diagramas — crea tu primer diagrama)".dimmed());
        }

        let opciones = &[
            "📊 Nuevo diagrama",
            "➕ Agregar nodo",
            "🔗 Conectar nodos",
            "📋 Ver Mermaid",
            "📝 Ver pseudocódigo",
            "✅ Validar diagrama",
            "🏷️  Recordar diagrama",
            "← Volver al menú",
        ];

        match menu("¿Qué deseas hacer?", opciones) {
            Some(0) => nuevo_diagrama(state),
            Some(1) => agregar_nodo(state),
            Some(2) => conectar_nodos(state),
            Some(3) => ver_mermaid(state),
            Some(4) => ver_pseudo(state),
            Some(5) => validar_diagrama(state),
            Some(6) => recordar_diagrama(state),
            _ => return,
        }
    }
}

fn seleccionar_diagrama(state: &AppState) -> Option<usize> {
    if state.diagramas.is_empty() {
        println!("  {}", "No hay diagramas.".yellow());
        pausa();
        return None;
    }
    let nombres: Vec<String> = state.diagramas.iter()
        .map(|d| format!("[{}] {} ({})", d.id, d.nombre, d.tipo))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
    menu("Selecciona diagrama", &refs)
}

fn nuevo_diagrama(state: &mut AppState) {
    separador("📊 Nuevo diagrama");
    let nombre = match pedir_texto("Nombre") { Some(t) => t, None => return };
    let tipos = &["Diagrama de Flujo", "Algoritmo", "Proceso", "Flujo de Datos", "Libre"];
    let ti = match menu("Tipo", tipos) { Some(i) => i, None => return };
    let tipo = match ti {
        0 => TipoDiagrama::Flujo,
        1 => TipoDiagrama::Algoritmo,
        2 => TipoDiagrama::Proceso,
        3 => TipoDiagrama::DatosFlujo,
        _ => TipoDiagrama::Libre,
    };

    let d = Diagrama::new(nombre.clone(), tipo);
    println!("  {} [{}] {}", "✓ Diagrama creado:".green().bold(), d.id, nombre);
    state.diagramas.push(d);
    pausa();
}

fn agregar_nodo(state: &mut AppState) {
    let idx = match seleccionar_diagrama(state) { Some(i) => i, None => return };

    let tipos_nodo = &["⬤ Inicio", "◯ Fin", "▭ Proceso", "◇ Decisión", "▱ Entrada/Salida", "● Conector", "▭▭ Subproceso", "▤ Dato"];
    let ni = match menu("Tipo de nodo", tipos_nodo) { Some(i) => i, None => return };
    let tipo = match ni {
        0 => TipoNodo::Inicio,
        1 => TipoNodo::Fin,
        3 => TipoNodo::Decision,
        4 => TipoNodo::EntradaSalida,
        5 => TipoNodo::Conector,
        6 => TipoNodo::Subproceso,
        7 => TipoNodo::Dato,
        _ => TipoNodo::Proceso,
    };

    let etiqueta = match pedir_texto("Etiqueta del nodo") { Some(t) => t, None => return };
    let nodo = Nodo::new(tipo, etiqueta.clone(), 0.0, 0.0);
    let nid = state.diagramas[idx].agregar_nodo(nodo);
    println!("  {} Nodo [{}] '{}' agregado", "✓".green(), nid, etiqueta);

    // ¿Agregar otro?
    if Confirm::new().with_prompt("  ¿Agregar otro nodo?").default(true).interact().unwrap_or(false) {
        agregar_nodo_al(state, idx);
    }
    pausa();
}

fn agregar_nodo_al(state: &mut AppState, idx: usize) {
    let tipos_nodo = &["⬤ Inicio", "◯ Fin", "▭ Proceso", "◇ Decisión", "▱ Entrada/Salida", "● Conector", "▭▭ Subproceso", "▤ Dato"];
    let ni = match menu("Tipo de nodo", tipos_nodo) { Some(i) => i, None => return };
    let tipo = match ni {
        0 => TipoNodo::Inicio,
        1 => TipoNodo::Fin,
        3 => TipoNodo::Decision,
        4 => TipoNodo::EntradaSalida,
        5 => TipoNodo::Conector,
        6 => TipoNodo::Subproceso,
        7 => TipoNodo::Dato,
        _ => TipoNodo::Proceso,
    };

    let etiqueta = match pedir_texto("Etiqueta del nodo") { Some(t) => t, None => return };
    let nodo = Nodo::new(tipo, etiqueta.clone(), 0.0, 0.0);
    let nid = state.diagramas[idx].agregar_nodo(nodo);
    println!("  {} Nodo [{}] '{}' agregado", "✓".green(), nid, etiqueta);

    if Confirm::new().with_prompt("  ¿Agregar otro nodo?").default(true).interact().unwrap_or(false) {
        agregar_nodo_al(state, idx);
    }
}

fn conectar_nodos(state: &mut AppState) {
    let idx = match seleccionar_diagrama(state) { Some(i) => i, None => return };

    if state.diagramas[idx].nodos.len() < 2 {
        println!("  {}", "Necesitas al menos 2 nodos para conectar.".yellow());
        pausa();
        return;
    }

    let nodos: Vec<String> = state.diagramas[idx].nodos.iter()
        .map(|n| format!("[{}] {} {}", n.id, n.tipo, n.etiqueta))
        .collect();
    let refs: Vec<&str> = nodos.iter().map(|s| s.as_str()).collect();

    println!("  Selecciona el nodo ORIGEN:");
    let oi = match menu("Origen", &refs) { Some(i) => i, None => return };
    println!("  Selecciona el nodo DESTINO:");
    let di = match menu("Destino", &refs) { Some(i) => i, None => return };

    let etiqueta = pedir_texto_opcional("Etiqueta de la conexión (ej: Sí, No, opcional)");
    let etiqueta = if etiqueta.is_empty() { None } else { Some(etiqueta) };

    let origen_id = state.diagramas[idx].nodos[oi].id.clone();
    let destino_id = state.diagramas[idx].nodos[di].id.clone();

    state.diagramas[idx].conectar(&origen_id, &destino_id, TipoConexion::Flecha, etiqueta);
    println!("  {} Conexión creada", "✓".green());

    if Confirm::new().with_prompt("  ¿Crear otra conexión?").default(true).interact().unwrap_or(false) {
        conectar_nodos_en(state, idx);
    }
    pausa();
}

fn conectar_nodos_en(state: &mut AppState, idx: usize) {
    let nodos: Vec<String> = state.diagramas[idx].nodos.iter()
        .map(|n| format!("[{}] {} {}", n.id, n.tipo, n.etiqueta))
        .collect();
    let refs: Vec<&str> = nodos.iter().map(|s| s.as_str()).collect();

    let oi = match menu("Origen", &refs) { Some(i) => i, None => return };
    let di = match menu("Destino", &refs) { Some(i) => i, None => return };
    let etiqueta = pedir_texto_opcional("Etiqueta (opcional)");
    let etiqueta = if etiqueta.is_empty() { None } else { Some(etiqueta) };

    let origen_id = state.diagramas[idx].nodos[oi].id.clone();
    let destino_id = state.diagramas[idx].nodos[di].id.clone();
    state.diagramas[idx].conectar(&origen_id, &destino_id, TipoConexion::Flecha, etiqueta);
    println!("  {} Conexión creada", "✓".green());

    if Confirm::new().with_prompt("  ¿Otra conexión?").default(false).interact().unwrap_or(false) {
        conectar_nodos_en(state, idx);
    }
}

fn ver_mermaid(state: &AppState) {
    let idx = match seleccionar_diagrama(state) { Some(i) => i, None => return };
    separador("Mermaid");
    println!("{}", state.diagramas[idx].exportar_mermaid());
    pausa();
}

fn ver_pseudo(state: &AppState) {
    let idx = match seleccionar_diagrama(state) { Some(i) => i, None => return };
    separador("Pseudocódigo");
    println!("{}", state.diagramas[idx].exportar_pseudocodigo());
    pausa();
}

fn validar_diagrama(state: &AppState) {
    let idx = match seleccionar_diagrama(state) { Some(i) => i, None => return };
    let errores = state.diagramas[idx].validar_flujo();
    if errores.is_empty() {
        println!("  {} Diagrama válido", "✓".green().bold());
    } else {
        println!("  {}", "Errores encontrados:".red().bold());
        for e in errores {
            println!("    {} {}", "✗".red(), e);
        }
    }
    pausa();
}

fn recordar_diagrama(state: &mut AppState) {
    let idx = match seleccionar_diagrama(state) { Some(i) => i, None => return };
    let palabras = match pedir_texto("Palabras clave (separadas por coma)") { Some(t) => t, None => return };
    let tags: Vec<String> = palabras.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();

    let diag = &state.diagramas[idx];
    let recuerdo = Recuerdo::new(
        format!("Diagrama: {}", diag.nombre),
        tags,
    ).con_origen("diagrama", &diag.id);
    state.memoria.agregar_recuerdo(recuerdo);

    println!("  {} Guardado en la memoria", "🧠".to_string());
    pausa();
}

// ══════════════════════════════════════════════════════════════
//  Módulo: VERSIONES (VCS)
// ══════════════════════════════════════════════════════════════

fn menu_versiones(state: &mut AppState) {
    loop {
        limpiar();
        separador("💾 VERSIONES — Source Control");

        println!("  Rama actual: {}", state.vcs.rama_actual.cyan().bold());
        println!("  Ramas: {}", state.vcs.ramas.iter().map(|r| {
            if r.nombre == state.vcs.rama_actual { format!("*{}", r.nombre).green().to_string() }
            else { r.nombre.clone() }
        }).collect::<Vec<_>>().join(", "));

        let log = state.vcs.log();
        if !log.is_empty() {
            println!();
            println!("  {}", "Historial:".bold());
            for s in log.iter().rev().take(10) {
                println!("    {} {} — {} ({})",
                    format!("[{}]", &s.hash[..7]).yellow(),
                    s.mensaje,
                    s.autor.dimmed(),
                    s.timestamp.format("%d/%m %H:%M")
                );
            }
        }

        let opciones = &[
            "💾 Nuevo commit (guardar versión)",
            "🌿 Crear rama",
            "🔀 Cambiar de rama",
            "📋 Ver log completo",
            "← Volver al menú",
        ];

        match menu("¿Qué deseas hacer?", opciones) {
            Some(0) => {
                let mensaje = match pedir_texto("Mensaje del commit") { Some(t) => t, None => continue };
                let autor = pedir_texto_opcional("Autor");
                let autor = if autor.is_empty() { "usuario".to_string() } else { autor };
                let datos = serde_json::to_string(&state.tasks).unwrap_or_default();
                let id = state.vcs.commit(datos, mensaje.clone(), autor);
                println!("  {} Commit [{}]: {}", "✓".green(), id, mensaje);
                pausa();
            }
            Some(1) => {
                let nombre = match pedir_texto("Nombre de la nueva rama") { Some(t) => t, None => continue };
                if state.vcs.crear_rama(nombre.clone()) {
                    println!("  {} Rama '{}' creada y activada", "✓".green(), nombre);
                } else {
                    println!("  {} La rama '{}' ya existe", "✗".red(), nombre);
                }
                pausa();
            }
            Some(2) => {
                let ramas: Vec<String> = state.vcs.ramas.iter().map(|r| r.nombre.clone()).collect();
                let refs: Vec<&str> = ramas.iter().map(|s| s.as_str()).collect();
                let idx = match menu("Selecciona rama", &refs) { Some(i) => i, None => continue };
                state.vcs.cambiar_rama(&ramas[idx]);
                println!("  {} Cambiado a '{}'", "✓".green(), ramas[idx]);
                pausa();
            }
            Some(3) => {
                let log = state.vcs.log();
                separador("Log completo");
                for s in log.iter().rev() {
                    println!("  {} {} — {} ({})", format!("[{}]", &s.hash[..7]).yellow(), s.mensaje, s.autor, s.timestamp.format("%d/%m/%Y %H:%M"));
                }
                pausa();
            }
            _ => return,
        }
    }
}

// ══════════════════════════════════════════════════════════════
//  Módulo: MAPEO Y CODIFICACIÓN
// ══════════════════════════════════════════════════════════════

fn menu_mapeo(state: &mut AppState) {
    loop {
        limpiar();
        separador("🔄 MAPEO Y CODIFICACIÓN");

        if !state.mapper.esquemas.is_empty() {
            for e in &state.mapper.esquemas {
                println!("  🔄 {}", e);
            }
        }

        let opciones = &[
            "🔤 Codificar texto (Base64 / Hex / Binario)",
            "🔓 Decodificar hex → texto",
            "📐 Nuevo esquema de mapeo",
            "📏 Agregar regla a esquema",
            "← Volver al menú",
        ];

        match menu("¿Qué deseas hacer?", opciones) {
            Some(0) => {
                let texto = match pedir_texto("Texto a codificar") { Some(t) => t, None => continue };
                let formatos = &["Base64", "Hexadecimal", "Binario"];
                let fi = match menu("Formato", formatos) { Some(i) => i, None => continue };
                let cod = match fi {
                    0 => Codificacion::Base64,
                    1 => Codificacion::Hex,
                    _ => Codificacion::Binario,
                };
                let resultado = Mapper::codificar(&texto, &cod);
                println!("\n  {} → {}", formatos[fi].cyan(), resultado.green().bold());
                pausa();
            }
            Some(1) => {
                let hex = match pedir_texto("Texto en hexadecimal") { Some(t) => t, None => continue };
                match Mapper::decodificar_hex(&hex) {
                    Some(texto) => println!("  {} → {}", "hex".cyan(), texto.green().bold()),
                    None => println!("  {} Formato hex inválido", "✗".red()),
                }
                pausa();
            }
            Some(2) => {
                let nombre = match pedir_texto("Nombre del esquema") { Some(t) => t, None => continue };
                let cods = &["UTF-8", "JSON", "CSV", "Base64", "Hex", "Binario"];
                let ei = match menu("Codificación de entrada", cods) { Some(i) => i, None => continue };
                let si = match menu("Codificación de salida", cods) { Some(i) => i, None => continue };
                let parse = |i: usize| match i {
                    1 => Codificacion::Json,
                    2 => Codificacion::Csv,
                    3 => Codificacion::Base64,
                    4 => Codificacion::Hex,
                    5 => Codificacion::Binario,
                    _ => Codificacion::Utf8,
                };
                let esquema = EsquemaMapa::new(nombre, parse(ei), parse(si));
                println!("  {} {}", "✓ Esquema creado:".green(), esquema);
                state.mapper.agregar_esquema(esquema);
                pausa();
            }
            Some(3) => {
                if state.mapper.esquemas.is_empty() {
                    println!("  {}", "No hay esquemas.".yellow());
                    pausa();
                    continue;
                }
                let nombres: Vec<String> = state.mapper.esquemas.iter().map(|e| format!("[{}] {}", e.id, e.nombre)).collect();
                let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
                let idx = match menu("Esquema", &refs) { Some(i) => i, None => continue };
                let origen = match pedir_texto("Campo origen") { Some(t) => t, None => continue };
                let destino = match pedir_texto("Campo destino") { Some(t) => t, None => continue };
                let trans = pedir_texto_opcional("Transformación (uppercase, lowercase, trim, reverse, prefix:X, suffix:X)");
                let trans = if trans.is_empty() { None } else { Some(trans) };
                state.mapper.esquemas[idx].agregar_regla(origen.clone(), destino.clone(), trans);
                println!("  {} {} → {}", "✓".green(), origen, destino);
                pausa();
            }
            _ => return,
        }
    }
}

// ══════════════════════════════════════════════════════════════
//  Módulo: MEMORIA — Búsqueda y conexiones
// ══════════════════════════════════════════════════════════════

fn menu_memoria(state: &mut AppState) {
    loop {
        limpiar();
        separador("🧠 MEMORIA — Tu segundo cerebro");

        if !state.memoria.recuerdos.is_empty() {
            let mut palabras: Vec<&String> = state.memoria.palabras_clave();
            palabras.sort();
            palabras.dedup();
            println!("  {} {}", "📚 Recuerdos:".bold(), state.memoria.recuerdos.len());
            println!("  {} {}", "🏷️  Palabras clave:".bold(),
                if palabras.is_empty() { "(ninguna)".dimmed().to_string() }
                else { palabras.iter().map(|p| p.cyan().to_string()).collect::<Vec<_>>().join(", ") });
            if !state.memoria.enlaces.is_empty() {
                println!("  {} {}", "🔗 Enlaces:".bold(), state.memoria.enlaces.len());
            }
            let n_ideas = state.memoria.diccionario.todas_las_ideas().len();
            let n_conexiones = state.memoria.diccionario.conexiones.len();
            if n_ideas > 0 {
                println!("  {} {} ideas, {} conexiones neuronales", "🧬 Diccionario:".bold(), n_ideas, n_conexiones);
            }
        } else {
            println!("  {}", "(vacío — crea tu primer recuerdo o apunte)".dimmed());
            println!();
            println!("  {}", "La memoria es tu espacio para anotar TODO lo que".dimmed());
            println!("  {}", "necesites: citas, ideas, apuntes, instrucciones...".dimmed());
        }

        let opciones = &[
            "📝 Nuevo apunte / recuerdo",
            "🔍 Buscar por palabra clave",
            "📋 Ver todos los recuerdos",
            "🧬 Diccionario neuronal de ideas",
            "✏️  Editar un recuerdo",
            "🏷️  Gestionar palabras clave",
            "🔗 Enlazar dos elementos",
            "🗑️  Eliminar un recuerdo",
            "← Volver al menú",
        ];

        match menu("¿Qué deseas hacer?", opciones) {
            Some(0) => crear_recuerdo(state),
            Some(1) => buscar_memoria(state),
            Some(2) => ver_recuerdos(state),
            Some(3) => ver_diccionario(state),
            Some(4) => editar_recuerdo(state),
            Some(5) => gestionar_palabras_clave(state),
            Some(6) => enlazar_elementos(state),
            Some(7) => eliminar_recuerdo(state),
            _ => return,
        }
    }
}

fn ver_diccionario(state: &mut AppState) {
    if state.memoria.diccionario.conexiones.is_empty() && state.memoria.diccionario.historial.is_empty() {
        println!("  {}", "El diccionario neuronal está vacío.".dimmed());
        println!("  {}", "Se llenará automáticamente al completar tareas con palabras clave.".dimmed());
        pausa();
        return;
    }

    loop {
        limpiar();
        separador("🧬 DICCIONARIO NEURONAL DE IDEAS");

        let mut ideas: Vec<String> = state.memoria.diccionario.todas_las_ideas().into_iter().cloned().collect();
        ideas.sort();
        println!("  {} {} ideas | {} conexiones | {} entradas",
            "📊".to_string(),
            ideas.len(),
            state.memoria.diccionario.conexiones.len(),
            state.memoria.diccionario.historial.len());
        println!();

        // Mostrar las conexiones más fuertes
        {
            let mut conexiones_ord: Vec<(String, String, u32, String)> = state.memoria.diccionario.conexiones.iter()
                .map(|c| (c.palabra_a.clone(), c.palabra_b.clone(), c.fuerza, c.contexto.last().cloned().unwrap_or_default()))
                .collect();
            conexiones_ord.sort_by(|a, b| b.2.cmp(&a.2));

            if !conexiones_ord.is_empty() {
                println!("  {} Conexiones más fuertes:", "🔥".to_string());
                for (pa, pb, fuerza, ctx) in conexiones_ord.iter().take(10) {
                    let barra = "█".repeat(*fuerza as usize).cyan();
                    let ctx_str = if ctx.is_empty() { String::new() }
                        else { format!(" ({})", ctx.dimmed()) };
                    println!("    {} {} ↔ {} [{}]{}",
                        barra,
                        pa.yellow(),
                        pb.yellow(),
                        fuerza,
                        ctx_str);
                }
                println!();
            }
        }

        let opciones = &[
            "🔍 Explorar una idea",
            "📋 Ver historial completo",
            "🗺️  Mapa de todas las ideas",
            "← Volver",
        ];

        match menu("¿Qué deseas hacer?", opciones) {
            Some(0) => explorar_idea(state),
            Some(1) => {
                separador("📋 Historial del diccionario");
                for (i, e) in state.memoria.diccionario.historial.iter().enumerate().rev() {
                    println!("  {} [{}] {} — \"{}\"",
                        format!("{}.", i + 1).dimmed(),
                        e.modulo.cyan(),
                        e.item_titulo.bold(),
                        if e.nota.is_empty() { "-".to_string() } else { e.nota.clone() });
                    println!("    🏷️  {} | {}",
                        e.palabras.join(", ").yellow(),
                        e.creado.format("%d/%m/%Y %H:%M").to_string().dimmed());
                }
                pausa();
            }
            Some(2) => {
                separador("🗺️  Mapa de ideas");
                for idea in &ideas {
                    let relacionadas = state.memoria.diccionario.ideas_relacionadas(idea);
                    if !relacionadas.is_empty() {
                        let rels: Vec<String> = relacionadas.iter()
                            .map(|(r, f)| format!("{}({})", r, f))
                            .collect();
                        println!("  {} → {}", idea.yellow().bold(), rels.join(", ").cyan());
                    }
                }
                pausa();
            }
            _ => return,
        }
    }
}

fn explorar_idea(state: &mut AppState) {
    let dic = &state.memoria.diccionario;
    let mut ideas: Vec<String> = dic.todas_las_ideas().into_iter().cloned().collect();
    ideas.sort();

    if ideas.is_empty() {
        println!("  {}", "No hay ideas en el diccionario.".yellow());
        pausa();
        return;
    }

    let refs: Vec<&str> = ideas.iter().map(|s| s.as_str()).collect();
    let idx = match menu("Selecciona una idea para explorar", &refs) {
        Some(i) => i,
        None => return,
    };
    let idea = &ideas[idx];

    println!();
    println!("  {} Explorando: {}", "🧬".to_string(), idea.yellow().bold());
    println!();

    // Ideas relacionadas
    let relacionadas = dic.ideas_relacionadas(idea);
    if !relacionadas.is_empty() {
        println!("  {} Conexiones:", "🔗".to_string());
        for (rel, fuerza) in &relacionadas {
            let barra = "█".repeat(*fuerza as usize).cyan();
            println!("    {} {} (fuerza: {})", barra, rel.yellow(), fuerza);
        }
        println!();
    }

    // Recuerdos con esta palabra
    let recuerdos = state.memoria.recuerdos_con_palabra(idea);
    if !recuerdos.is_empty() {
        println!("  {} Recuerdos asociados:", "🧠".to_string());
        for r in &recuerdos {
            println!("    • [{}] {}", r.id.dimmed(), r.contenido);
        }
        println!();
    }

    // Historial de esta palabra
    let historial: Vec<_> = dic.historial.iter()
        .filter(|e| e.palabras.iter().any(|p| p.to_lowercase() == idea.to_lowercase()))
        .collect();
    if !historial.is_empty() {
        println!("  {} Historial:", "📋".to_string());
        for e in &historial {
            println!("    • [{}] {} — {} ({})",
                e.modulo.cyan(), e.item_titulo.bold(),
                if e.nota.is_empty() { "-" } else { &e.nota },
                e.creado.format("%d/%m/%Y").to_string().dimmed());
        }
        println!();
    }

    // Sugerir ideas basadas en esta
    let sugerencias = dic.sugerir(&[idea.clone()]);
    if !sugerencias.is_empty() {
        println!("  {} Sugerencias para explorar:", "🔮".to_string());
        for (sug, score) in sugerencias.iter().take(5) {
            println!("    → {} (relevancia: {})", sug.yellow(), score);
        }
    }

    pausa();
}

fn buscar_memoria(state: &AppState) {
    let consulta = match pedir_texto("¿Qué buscas?") { Some(t) => t, None => return };
    let q = consulta.to_lowercase();
    let hoy = Local::now().naive_local();
    let hoy_fecha = hoy.date();

    // ── Estructura unificada para resultados ──
    struct Hallazgo {
        icono: &'static str,
        modulo: String,
        titulo: String,
        detalle: String,
        fecha: NaiveDate,
        hora: Option<NaiveTime>,
        estado: String,
        id: String,
        palabras: Vec<String>,
        enlaces_info: Vec<String>,
    }

    let mut hallazgos: Vec<Hallazgo> = Vec::new();

    // ── 1. Buscar en Recuerdos ──
    for r in &state.memoria.recuerdos {
        let coincide = r.palabras_clave.iter().any(|p| p.contains(&q))
            || r.contenido.to_lowercase().contains(&q);
        if coincide {
            let mut enlaces_info = Vec::new();
            if let (Some(modulo), Some(id)) = (&r.modulo_origen, &r.item_id) {
                for e in state.memoria.enlaces_de(modulo, id) {
                    enlaces_info.push(format!("🔗 {} [{}] ↔ {} [{}] ({})",
                        e.origen_modulo, e.origen_id, e.destino_modulo, e.destino_id, e.relacion));
                }
            }
            hallazgos.push(Hallazgo {
                icono: "🧠",
                modulo: "Recuerdo".to_string(),
                titulo: r.contenido.chars().take(60).collect::<String>(),
                detalle: if r.contenido.len() > 60 { r.contenido.clone() } else { String::new() },
                fecha: r.creado.date(),
                hora: Some(r.creado.time()),
                estado: r.modulo_origen.clone().unwrap_or_else(|| "apunte".to_string()),
                id: r.id.clone(),
                palabras: r.palabras_clave.clone(),
                enlaces_info,
            });
        }
    }

    // ── 2. Buscar en Tareas ──
    for t in &state.tasks.tareas {
        let coincide = t.titulo.to_lowercase().contains(&q)
            || t.descripcion.to_lowercase().contains(&q)
            || t.etiquetas.iter().any(|e| e.to_lowercase().contains(&q));
        if coincide {
            let enlaces_info: Vec<String> = state.memoria.enlaces_de("tarea", &t.id)
                .iter()
                .map(|e| format!("🔗 {} [{}] ↔ {} [{}] ({})",
                    e.origen_modulo, e.origen_id, e.destino_modulo, e.destino_id, e.relacion))
                .collect();

            // Si tiene follow-up, mostrar como entrada separada con su fecha
            let follow_up_info = if let Some(fu) = &t.follow_up {
                format!("⏰ Follow-up: {} {}", fu.date().format("%d/%m/%Y"), fu.time().format("%H:%M"))
            } else {
                String::new()
            };

            let mut detalle_parts: Vec<String> = Vec::new();
            if !t.descripcion.is_empty() {
                detalle_parts.push(t.descripcion.clone());
            }
            if !follow_up_info.is_empty() {
                detalle_parts.push(follow_up_info.clone());
            }

            // Entrada principal de la tarea (con su fecha original)
            hallazgos.push(Hallazgo {
                icono: "📋",
                modulo: "Tarea".to_string(),
                titulo: t.titulo.clone(),
                detalle: detalle_parts.join(" | "),
                fecha: t.fecha,
                hora: Some(t.hora),
                estado: format!("{} | {}", t.estado, t.prioridad),
                id: t.id.clone(),
                palabras: t.etiquetas.clone(),
                enlaces_info: enlaces_info.clone(),
            });

            // Si hay follow-up, crear entrada adicional con la fecha del follow-up
            if let Some(fu) = &t.follow_up {
                hallazgos.push(Hallazgo {
                    icono: "⏰",
                    modulo: "Follow-Up".to_string(),
                    titulo: format!("[Follow-Up] {}", t.titulo),
                    detalle: format!("📋 Tarea original: {} ({})", t.titulo, t.fecha.format("%d/%m/%Y")),
                    fecha: fu.date(),
                    hora: Some(fu.time()),
                    estado: format!("{} | {}", t.estado, t.prioridad),
                    id: t.id.clone(),
                    palabras: t.etiquetas.clone(),
                    enlaces_info,
                });
            }
        }
    }

    // ── 3. Buscar en Eventos de Agenda ──
    for e in &state.agenda.eventos {
        let coincide = e.titulo.to_lowercase().contains(&q)
            || e.descripcion.to_lowercase().contains(&q)
            || e.concepto.to_lowercase().contains(&q)
            || e.notas.iter().any(|n| n.to_lowercase().contains(&q));
        if coincide {
            let enlaces_info: Vec<String> = state.memoria.enlaces_de("evento", &e.id)
                .iter()
                .map(|en| format!("🔗 {} [{}] ↔ {} [{}] ({})",
                    en.origen_modulo, en.origen_id, en.destino_modulo, en.destino_id, en.relacion))
                .collect();
            let hora_str = e.hora_fin
                .map(|fin| format!("{} - {}", e.hora_inicio, fin))
                .unwrap_or_else(|| format!("{}", e.hora_inicio));
            let concepto_txt = if e.concepto.is_empty() { String::new() } else { format!(" [{}]", e.concepto) };
            let recur_txt = e.etiqueta_recurrencia();

            // Si es recurrente, buscar próxima ocurrencia futura y la más reciente pasada
            if e.frecuencia != Frecuencia::UnaVez {
                // Solo buscar ocurrencia en el año actual
                let anio_actual = hoy_fecha.year();
                let inicio_anio = NaiveDate::from_ymd_opt(anio_actual, 1, 1).unwrap();
                let fin_anio = NaiveDate::from_ymd_opt(anio_actual, 12, 31).unwrap();
                let ocurrencias_anio = e.frecuencia.proximas_ocurrencias(e.fecha, inicio_anio, fin_anio);

                if let Some(&proxima) = ocurrencias_anio.iter().find(|&&f| f >= hoy_fecha) {
                    // Próxima ocurrencia este año (aún no pasó)
                    let icono_tipo = match e.tipo {
                        TipoEvento::Cumpleanos => "🎂",
                        TipoEvento::Pago => "💰",
                        _ => "📅",
                    };
                    let detalle_futuro = if matches!(e.tipo, TipoEvento::Cumpleanos) {
                        let anios = proxima.year() - e.fecha.year();
                        format!("Cumple {} años{}", anios, concepto_txt)
                    } else {
                        let base = if e.descripcion.is_empty() {
                            format!("{}{}", hora_str, concepto_txt)
                        } else {
                            format!("{} | {}{}", hora_str, e.descripcion, concepto_txt)
                        };
                        format!("{}{}", base, recur_txt)
                    };
                    hallazgos.push(Hallazgo {
                        icono: icono_tipo,
                        modulo: "Evento".to_string(),
                        titulo: format!("{}{}", e.titulo, recur_txt),
                        detalle: detalle_futuro,
                        fecha: proxima,
                        hora: Some(e.hora_inicio),
                        estado: format!("{}", e.tipo),
                        id: e.id.clone(),
                        palabras: Vec::new(),
                        enlaces_info: enlaces_info.clone(),
                    });
                } else if let Some(&pasada) = ocurrencias_anio.last() {
                    // Ya pasó este año, mostrar la ocurrencia que ya fue
                    let icono_tipo = match e.tipo {
                        TipoEvento::Cumpleanos => "🎂",
                        TipoEvento::Pago => "💰",
                        _ => "📅",
                    };
                    let detalle_pasado = if matches!(e.tipo, TipoEvento::Cumpleanos) {
                        let anios = pasada.year() - e.fecha.year();
                        format!("Cumplió {} años{}", anios, concepto_txt)
                    } else {
                        format!("{}{}{}", hora_str, concepto_txt, recur_txt)
                    };
                    hallazgos.push(Hallazgo {
                        icono: icono_tipo,
                        modulo: "Evento".to_string(),
                        titulo: format!("{}{}", e.titulo, recur_txt),
                        detalle: detalle_pasado,
                        fecha: pasada,
                        hora: Some(e.hora_inicio),
                        estado: format!("{}", e.tipo),
                        id: e.id.clone(),
                        palabras: Vec::new(),
                        enlaces_info: enlaces_info.clone(),
                    });
                }

                // Registro original compacto (solo si es de otro año)
                if e.fecha.year() < anio_actual {
                    let dias_desde = (hoy_fecha - e.fecha).num_days();
                    let anios = dias_desde / 365;
                    let meses = (dias_desde % 365) / 30;
                    let resumen = if meses > 0 {
                        format!("Origen: {} (hace ~{} año(s) y {} mes(es))", e.fecha.format("%d/%m/%Y"), anios, meses)
                    } else {
                        format!("Origen: {} (hace ~{} año(s))", e.fecha.format("%d/%m/%Y"), anios)
                    };
                    hallazgos.push(Hallazgo {
                        icono: "🗓️ ",
                        modulo: "Registro original".to_string(),
                        titulo: format!("{} — fecha base", e.titulo),
                        detalle: resumen,
                        fecha: e.fecha,
                        hora: Some(e.hora_inicio),
                        estado: format!("{}{}", e.tipo, recur_txt),
                        id: e.id.clone(),
                        palabras: Vec::new(),
                        enlaces_info: Vec::new(),
                    });
                }
            } else {
                // Evento único: una sola entrada
                hallazgos.push(Hallazgo {
                    icono: "📅",
                    modulo: "Evento".to_string(),
                    titulo: e.titulo.clone(),
                    detalle: if e.descripcion.is_empty() {
                        format!("{}{}", hora_str, concepto_txt)
                    } else {
                        format!("{} | {}{}", hora_str, e.descripcion, concepto_txt)
                    },
                    fecha: e.fecha,
                    hora: Some(e.hora_inicio),
                    estado: format!("{}", e.tipo),
                    id: e.id.clone(),
                    palabras: Vec::new(),
                    enlaces_info,
                });
            }
        }
    }

    // ── 4. Buscar en Diagramas ──
    for d in &state.diagramas {
        if d.nombre.to_lowercase().contains(&q) {
            hallazgos.push(Hallazgo {
                icono: "📊",
                modulo: "Diagrama".to_string(),
                titulo: d.nombre.clone(),
                detalle: String::new(),
                fecha: hoy_fecha,
                hora: None,
                estado: format!("{}", d.tipo),
                id: d.id.clone(),
                palabras: Vec::new(),
                enlaces_info: Vec::new(),
            });
        }
    }

    // ── 5. Buscar en Canvases ──
    for c in &state.canvases {
        if c.nombre.to_lowercase().contains(&q) {
            hallazgos.push(Hallazgo {
                icono: "✏️ ",
                modulo: "Canvas".to_string(),
                titulo: c.nombre.clone(),
                detalle: String::new(),
                fecha: hoy_fecha,
                hora: None,
                estado: String::new(),
                id: c.id.clone(),
                palabras: Vec::new(),
                enlaces_info: Vec::new(),
            });
        }
    }

    // ── Sin resultados ──
    if hallazgos.is_empty() {
        println!();
        println!("  {}", format!("No se encontró \"{}\" en ningún módulo.", consulta).yellow());
        pausa();
        return;
    }

    // ── Ordenar por fecha descendente (más recientes primero) ──
    hallazgos.sort_by(|a, b| {
        b.fecha.cmp(&a.fecha)
            .then(b.hora.cmp(&a.hora))
    });

    // ── Separar pasado / hoy / futuro ──
    let futuro: Vec<&Hallazgo> = hallazgos.iter().filter(|h| h.fecha > hoy_fecha).collect();
    let hoy_items: Vec<&Hallazgo> = hallazgos.iter().filter(|h| h.fecha == hoy_fecha).collect();
    let pasado: Vec<&Hallazgo> = hallazgos.iter().filter(|h| h.fecha < hoy_fecha).collect();

    // ── Mostrar resultados ──
    separador(&format!("🔍 \"{}\" — {} coincidencias", consulta, hallazgos.len()));

    let mostrar_hallazgo = |h: &Hallazgo| {
        let dias = (h.fecha - hoy_fecha).num_days();
        let tiempo_rel = if dias == 0 {
            "hoy".to_string()
        } else if dias == 1 {
            "mañana".to_string()
        } else if dias == -1 {
            "ayer".to_string()
        } else if dias > 1 {
            format!("en {} días", dias)
        } else {
            format!("hace {} días", -dias)
        };

        let hora_str = h.hora.map(|t| format!(" {}", t.format("%H:%M"))).unwrap_or_default();
        let estado_str = if h.estado.is_empty() { String::new() } else { format!(" — {}", h.estado) };

        println!("  {} {} {} [{}]{}",
            h.icono,
            h.titulo.bold(),
            format!("({})", h.modulo).dimmed(),
            h.id.dimmed(),
            estado_str.dimmed());
        println!("     📆 {}{} ({})",
            h.fecha.format("%d/%m/%Y"),
            hora_str,
            tiempo_rel.cyan());
        if !h.detalle.is_empty() {
            println!("     📄 {}", h.detalle.dimmed());
        }
        if !h.palabras.is_empty() {
            println!("     🏷️  {}", h.palabras.join(", ").cyan());
        }
        for enlace in &h.enlaces_info {
            println!("     {}", enlace);
        }
        println!();
    };

    if !futuro.is_empty() {
        println!("  {}", "▶ PRÓXIMAMENTE (futuro)".green().bold());
        println!();
        for h in &futuro {
            mostrar_hallazgo(h);
        }
    }

    if !hoy_items.is_empty() {
        println!("  {}", "● HOY".yellow().bold());
        println!();
        for h in &hoy_items {
            mostrar_hallazgo(h);
        }
    }

    if !pasado.is_empty() {
        println!("  {}", "◀ HISTORIAL (pasado, más reciente primero)".dimmed().bold());
        println!();
        for h in &pasado {
            mostrar_hallazgo(h);
        }
    }

    pausa();
}

fn enlazar_elementos(state: &mut AppState) {
    let modulos = &["📋 Tarea", "📅 Evento", "📊 Diagrama", "✏️  Canvas"];

    println!("  Selecciona el PRIMER elemento:");
    let m1 = match menu("Módulo origen", modulos) { Some(i) => i, None => return };
    let (mod1, id1) = seleccionar_item_de_modulo(state, m1);
    if id1.is_empty() { return; }

    println!("  Selecciona el SEGUNDO elemento:");
    let m2 = match menu("Módulo destino", modulos) { Some(i) => i, None => return };
    let (mod2, id2) = seleccionar_item_de_modulo(state, m2);
    if id2.is_empty() { return; }

    let relacion = match pedir_texto("Relación (ej: 'necesita', 'depende de', 'parte de')") { Some(t) => t, None => return };

    state.memoria.enlazar(&mod1, &id1, &mod2, &id2, &relacion);
    println!("  {} Enlace creado: {} ↔ {} ({})", "🔗".to_string(), mod1, mod2, relacion);
    pausa();
}

fn seleccionar_item_de_modulo(state: &AppState, modulo_idx: usize) -> (String, String) {
    match modulo_idx {
        0 => {
            if state.tasks.tareas.is_empty() { println!("  Sin tareas."); return (String::new(), String::new()); }
            let items: Vec<String> = state.tasks.tareas.iter().map(|t| format!("{} - {}", t.id, t.titulo)).collect();
            let refs: Vec<&str> = items.iter().map(|s| s.as_str()).collect();
            let i = match menu("Selecciona", &refs) { Some(i) => i, None => return (String::new(), String::new()) };
            ("tarea".to_string(), state.tasks.tareas[i].id.clone())
        }
        1 => {
            if state.agenda.eventos.is_empty() { println!("  Sin eventos."); return (String::new(), String::new()); }
            let items: Vec<String> = state.agenda.eventos.iter().map(|e| format!("{} - {}", e.id, e.titulo)).collect();
            let refs: Vec<&str> = items.iter().map(|s| s.as_str()).collect();
            let i = match menu("Selecciona", &refs) { Some(i) => i, None => return (String::new(), String::new()) };
            ("evento".to_string(), state.agenda.eventos[i].id.clone())
        }
        2 => {
            if state.diagramas.is_empty() { println!("  Sin diagramas."); return (String::new(), String::new()); }
            let items: Vec<String> = state.diagramas.iter().map(|d| format!("{} - {}", d.id, d.nombre)).collect();
            let refs: Vec<&str> = items.iter().map(|s| s.as_str()).collect();
            let i = match menu("Selecciona", &refs) { Some(i) => i, None => return (String::new(), String::new()) };
            ("diagrama".to_string(), state.diagramas[i].id.clone())
        }
        3 => {
            if state.canvases.is_empty() { println!("  Sin canvases."); return (String::new(), String::new()); }
            let items: Vec<String> = state.canvases.iter().map(|c| format!("{} - {}", c.id, c.nombre)).collect();
            let refs: Vec<&str> = items.iter().map(|s| s.as_str()).collect();
            let i = match menu("Selecciona", &refs) { Some(i) => i, None => return (String::new(), String::new()) };
            ("canvas".to_string(), state.canvases[i].id.clone())
        }
        _ => (String::new(), String::new()),
    }
}

fn crear_recuerdo(state: &mut AppState) {
    separador("📝 Nuevo apunte / recuerdo");

    println!("  Escribe lo que quieras recordar: una nota, idea, cita,");
    println!("  instrucciones, lo que sea. Tu mente no tiene que cargar");
    println!("  con todo — para eso está OmniPlanner.");
    println!();

    let tema = match pedir_texto("¿Sobre qué tema es? (ej: trabajo, salud, idea, compras)") { Some(t) => t, None => return };

    println!();
    println!("  Ahora escribe tu apunte. Puede ser tan largo como quieras.");
    println!("  {}", "(una línea por ahora, pero ponle todo lo que necesites)".dimmed());
    let contenido = match pedir_texto("¿Qué quieres recordar?") { Some(t) => t, None => return };

    println!();
    let mas_tags = pedir_texto_opcional("Más palabras clave (separadas por coma, opcional)");

    let mut tags: Vec<String> = vec![tema.trim().to_string()];
    if !mas_tags.is_empty() {
        for t in mas_tags.split(',') {
            let t = t.trim().to_string();
            if !t.is_empty() && !tags.contains(&t) {
                tags.push(t);
            }
        }
    }

    let vincular = Confirm::new()
        .with_prompt("  ¿Vincular a una tarea, evento o diagrama existente?")
        .default(false)
        .interact()
        .unwrap_or(false);

    let mut recuerdo = Recuerdo::new(contenido.clone(), tags.clone());

    if vincular {
        let modulos = &["📋 Tarea", "📅 Evento", "📊 Diagrama", "✏️  Canvas"];
        let mi = match menu("¿De qué módulo?", modulos) { Some(i) => i, None => return };
        let (modulo, id) = seleccionar_item_de_modulo(state, mi);
        if !id.is_empty() {
            recuerdo = recuerdo.con_origen(&modulo, &id);
        }
    }

    println!();
    println!("  {} Apunte guardado:", "🧠".to_string().green().bold());
    println!("    \"{}\"", contenido.cyan());
    println!("    🏷️  {}", tags.join(", ").yellow());
    state.memoria.agregar_recuerdo(recuerdo);
    pausa();
}

fn ver_recuerdos(state: &AppState) {
    if state.memoria.recuerdos.is_empty() {
        println!("  {}", "Sin recuerdos guardados.".dimmed());
        println!("  {}", "Usa '📝 Nuevo apunte' para empezar a anotar.".dimmed());
        pausa();
        return;
    }

    separador("📚 Todos los recuerdos");

    // Agrupar por primera palabra clave (tema)
    let mut temas: std::collections::HashMap<String, Vec<&Recuerdo>> = std::collections::HashMap::new();
    for r in &state.memoria.recuerdos {
        let tema = r.palabras_clave.first().cloned().unwrap_or_else(|| "sin tema".to_string());
        temas.entry(tema).or_default().push(r);
    }

    let mut temas_ord: Vec<_> = temas.keys().cloned().collect();
    temas_ord.sort();

    for tema in &temas_ord {
        println!("  {} {}", "▸".cyan(), tema.to_uppercase().bold());
        for r in &temas[tema] {
            let origen = match (&r.modulo_origen, &r.item_id) {
                (Some(m), Some(id)) => format!(" → {} [{}]", m, id),
                _ => String::new(),
            };
            let fecha = r.creado.format("%d/%m/%Y %H:%M");
            println!("    {} [{}] {}", "•".to_string(), r.id.dimmed(), r.contenido);
            println!("      🏷️  {} {} {}", r.palabras_clave.join(", ").cyan(), origen.dimmed(), format!("({})", fecha).dimmed());
        }
        println!();
    }

    println!("  Total: {} recuerdos en {} temas", state.memoria.recuerdos.len(), temas_ord.len());
    pausa();
}

fn editar_recuerdo(state: &mut AppState) {
    if state.memoria.recuerdos.is_empty() {
        println!("  {}", "Sin recuerdos para editar.".yellow());
        pausa();
        return;
    }

    let nombres: Vec<String> = state.memoria.recuerdos.iter()
        .map(|r| {
            let preview = if r.contenido.len() > 50 {
                format!("{}...", &r.contenido[..50])
            } else {
                r.contenido.clone()
            };
            format!("[{}] {} ({})", r.id, preview, r.palabras_clave.join(", "))
        })
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    let idx = match menu("¿Cuál recuerdo editar?", &refs) { Some(i) => i, None => return };
    let id = state.memoria.recuerdos[idx].id.clone();

    println!();
    println!("  Contenido actual: {}", state.memoria.recuerdos[idx].contenido.cyan());
    println!("  Palabras clave:   {}", state.memoria.recuerdos[idx].palabras_clave.join(", ").yellow());
    println!();

    let opciones = &[
        "✏️  Cambiar el texto del recuerdo",
        "➕ Agregar palabras clave",
        "🗑️  Quitar una palabra clave",
        "← Cancelar",
    ];

    match menu("¿Qué editar?", opciones) {
        Some(0) => {
            if let Some(nuevo) = pedir_texto("Nuevo contenido") {
                state.memoria.editar_contenido(&id, nuevo.clone());
                println!("  {} Contenido actualizado: \"{}\"", "✓".green(), nuevo);
            }
        }
        Some(1) => {
            let nuevas = match pedir_texto("Palabras clave a agregar (separadas por coma)") { Some(t) => t, None => { pausa(); return } };
            let mut agregadas = 0;
            for p in nuevas.split(',') {
                let p = p.trim();
                if !p.is_empty() && state.memoria.agregar_palabra_a_recuerdo(&id, p) {
                    agregadas += 1;
                }
            }
            println!("  {} {} palabras clave agregadas", "✓".green(), agregadas);
        }
        Some(2) => {
            let palabras = state.memoria.recuerdos[idx].palabras_clave.clone();
            if palabras.is_empty() {
                println!("  {}", "Este recuerdo no tiene palabras clave.".yellow());
            } else {
                let refs_p: Vec<&str> = palabras.iter().map(|s| s.as_str()).collect();
                let pi = match menu("¿Cuál palabra quitar?", &refs_p) { Some(i) => i, None => { pausa(); return } };
                let palabra = palabras[pi].clone();

                if Confirm::new()
                    .with_prompt(format!("  ¿Seguro que quieres quitar '{}'?", palabra))
                    .default(false)
                    .interact()
                    .unwrap_or(false)
                {
                    state.memoria.quitar_palabra_de_recuerdo(&id, &palabra);
                    println!("  {} Palabra '{}' eliminada de este recuerdo", "✓".green(), palabra);
                }
            }
        }
        _ => {}
    }
    pausa();
}

fn gestionar_palabras_clave(state: &mut AppState) {
    loop {
        limpiar();
        separador("🏷️  Gestionar palabras clave");

        let mut palabras: Vec<String> = state.memoria.palabras_clave().into_iter().cloned().collect();
        palabras.sort();

        if palabras.is_empty() {
            println!("  {}", "No hay palabras clave registradas.".dimmed());
            pausa();
            return;
        }

        println!("  Palabras clave en el sistema ({}):\n", palabras.len());
        for (i, p) in palabras.iter().enumerate() {
            let count = state.memoria.recuerdos_con_palabra(p).len();
            println!("    {}. {} ({} recuerdos)", i + 1, p.cyan(), count);
        }

        let opciones = &[
            "🔍 Ver recuerdos de una palabra clave",
            "🗑️  Eliminar una palabra clave (de todos los recuerdos)",
            "🗑️  Eliminar palabra clave de un recuerdo específico",
            "← Volver",
        ];

        match menu("¿Qué hacer?", opciones) {
            Some(0) => {
                let refs_p: Vec<&str> = palabras.iter().map(|s| s.as_str()).collect();
                let pi = match menu("¿Qué palabra clave?", &refs_p) { Some(i) => i, None => continue };
                let palabra = &palabras[pi];

                let recuerdos = state.memoria.recuerdos_con_palabra(palabra);
                if recuerdos.is_empty() {
                    println!("  {}", "No hay recuerdos con esa palabra.".dimmed());
                } else {
                    println!();
                    println!("  Recuerdos con '{}' ({}):", palabra.cyan(), recuerdos.len());
                    for r in &recuerdos {
                        println!("    • [{}] {}", r.id.dimmed(), r.contenido);
                        println!("      🏷️  {}", r.palabras_clave.join(", ").dimmed());
                    }
                }
                pausa();
            }
            Some(1) => {
                let refs_p: Vec<&str> = palabras.iter().map(|s| s.as_str()).collect();
                let pi = match menu("¿Qué palabra clave eliminar?", &refs_p) { Some(i) => i, None => continue };
                let palabra = palabras[pi].clone();
                let count = state.memoria.recuerdos_con_palabra(&palabra).len();

                println!();
                println!("  {} La palabra '{}' aparece en {} recuerdos.", "⚠".yellow(), palabra.cyan(), count);
                println!("  Se eliminará de todos, pero los recuerdos se conservan.");

                if Confirm::new()
                    .with_prompt(format!("  ¿Estás seguro de eliminar '{}'?", palabra))
                    .default(false)
                    .interact()
                    .unwrap_or(false)
                {
                    let afectados = state.memoria.eliminar_palabra_global(&palabra);
                    println!("  {} Palabra '{}' eliminada de {} recuerdos", "✓".green(), palabra, afectados);
                } else {
                    println!("  Cancelado.");
                }
                pausa();
            }
            Some(2) => {
                let refs_p: Vec<&str> = palabras.iter().map(|s| s.as_str()).collect();
                let pi = match menu("¿De qué palabra clave?", &refs_p) { Some(i) => i, None => continue };
                let palabra = palabras[pi].clone();

                let recuerdos_ids: Vec<(String, String)> = state.memoria.recuerdos_con_palabra(&palabra)
                    .iter()
                    .map(|r| {
                        let preview = if r.contenido.len() > 40 {
                            format!("{}...", &r.contenido[..40])
                        } else {
                            r.contenido.clone()
                        };
                        (r.id.clone(), format!("[{}] {}", r.id, preview))
                    })
                    .collect();

                if recuerdos_ids.is_empty() {
                    println!("  {}", "No hay recuerdos con esa palabra.".dimmed());
                    pausa();
                    continue;
                }

                let labels: Vec<&str> = recuerdos_ids.iter().map(|(_, l)| l.as_str()).collect();
                let ri = match menu("¿De cuál recuerdo quitar esta palabra?", &labels) { Some(i) => i, None => continue };
                let rid = recuerdos_ids[ri].0.clone();

                if Confirm::new()
                    .with_prompt(format!("  ¿Quitar '{}' de este recuerdo?", palabra))
                    .default(false)
                    .interact()
                    .unwrap_or(false)
                {
                    state.memoria.quitar_palabra_de_recuerdo(&rid, &palabra);
                    println!("  {} Palabra quitada", "✓".green());
                }
                pausa();
            }
            _ => return,
        }
    }
}

fn eliminar_recuerdo(state: &mut AppState) {
    if state.memoria.recuerdos.is_empty() {
        println!("  {}", "Sin recuerdos para eliminar.".yellow());
        pausa();
        return;
    }

    let nombres: Vec<String> = state.memoria.recuerdos.iter()
        .map(|r| {
            let preview = if r.contenido.len() > 50 {
                format!("{}...", &r.contenido[..50])
            } else {
                r.contenido.clone()
            };
            format!("[{}] {} ({})", r.id, preview, r.palabras_clave.join(", "))
        })
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    let idx = match menu("¿Cuál recuerdo eliminar?", &refs) { Some(i) => i, None => return };
    let id = state.memoria.recuerdos[idx].id.clone();
    let contenido = state.memoria.recuerdos[idx].contenido.clone();

    println!();
    println!("  Contenido: \"{}\"", contenido.cyan());
    println!("  {} Esta acción no se puede deshacer.", "⚠".yellow());

    if Confirm::new()
        .with_prompt(format!("  ¿Estás seguro de eliminar este recuerdo?"))
        .default(false)
        .interact()
        .unwrap_or(false)
    {
        state.memoria.eliminar_recuerdo(&id);
        println!("  {} Recuerdo eliminado", "✓".green());
    } else {
        println!("  Cancelado.");
    }
    pausa();
}

// ══════════════════════════════════════════════════════════════
//  Módulo: SINCRONIZACIÓN — Calendario y Email
// ══════════════════════════════════════════════════════════════

fn menu_sync(state: &mut AppState) {
    loop {
        limpiar();
        separador("🔗 SINCRONIZACIÓN");

        // Estado
        let gcal = if state.sync.google_configurado() {
            if state.sync.google_autenticado() {
                "✅ Conectado".green().to_string()
            } else {
                "⚠️  No autenticado".yellow().to_string()
            }
        } else {
            "❌ No configurado".red().to_string()
        };

        let email = if state.sync.email_configurado() {
            "✅ Configurado".green().to_string()
        } else {
            "❌ No configurado".red().to_string()
        };

        let gist_estado = if state.sync.gist_configurado() {
            if !state.sync.gist_id.is_empty() {
                "✅ Sincronizado".green().to_string()
            } else {
                "⚠️  Token listo (sin Gist aún)".yellow().to_string()
            }
        } else {
            "❌ No configurado".red().to_string()
        };

        let drive_estado = if state.sync.google_autenticado() {
            if state.sync.drive_configurado() {
                "✅ Sincronizado".green().to_string()
            } else {
                "⚠️  Listo (sin archivo aún)".yellow().to_string()
            }
        } else {
            "❌ Autentica Google primero".red().to_string()
        };

        println!("  {} GitHub Gist:    {}", "🔑".to_string(), gist_estado);
        println!("  Google Calendar: {}", gcal);
        println!("  Email SMTP:      {}", email);
        println!("  Google Drive:    {}", drive_estado);
        println!(
            "  Eventos sincronizados: {}  |  Tareas sincronizadas: {}",
            state.sync.mapa_eventos.len(),
            state.sync.mapa_tareas.len()
        );

        let opciones = &[
            "🔑 Subir datos via GitHub Gist (push)",
            "🔑 Descargar datos via GitHub Gist (pull)",
            "🔑 Buscar Gist existente (otro dispositivo)",
            "🔑 Configurar GitHub Gist (token)",
            "───────────────────────────",
            "☁️  Subir datos a Google Drive (push)",
            "☁️  Descargar datos de Google Drive (pull)",
            "☁️  Buscar archivo en Drive (otro dispositivo)",
            "───────────────────────────",
            "📅 Exportar a archivo .ics",
            "📅 Importar archivo .ics",
            "📅 Sincronizar → Google Calendar",
            "📅 Importar ← Google Calendar",
            "🔄 Re-sincronizar todo (limpiar mapeo y volver a enviar)",
            "🌐 Abrir Dashboard Web (ver desde celular)",
            "💾 Exportar estado completo (data.json)",
            "💾 Importar estado completo (data.json)",
            "📧 Enviar resumen diario",
            "📧 Enviar recordatorio de tarea",
            "📧 Enviar follow-up por email",
            "⚙️  Configurar Google Calendar",
            "⚙️  Configurar Email (SMTP)",
            "← Volver al menú",
        ];

        match menu("¿Qué deseas hacer?", opciones) {
            Some(0) => gist_push_datos(state),
            Some(1) => gist_pull_datos(state),
            Some(2) => gist_buscar_existente(state),
            Some(3) => configurar_gist(state),
            Some(4) => {} // separador
            Some(5) => drive_push(state),
            Some(6) => drive_pull(state),
            Some(7) => drive_buscar(state),
            Some(8) => {} // separador
            Some(9) => exportar_ics(state),
            Some(10) => importar_ics(state),
            Some(11) => sync_push_google(state),
            Some(12) => sync_pull_google(state),
            Some(13) => resync_google(state),
            Some(14) => iniciar_dashboard_web(state),
            Some(15) => exportar_estado(state),
            Some(16) => importar_estado(state),
            Some(17) => enviar_resumen(state),
            Some(18) => enviar_recordatorio(state),
            Some(19) => enviar_followup_email(state),
            Some(20) => configurar_google(state),
            Some(21) => configurar_email(state),
            _ => return,
        }
    }
}

// ══════════════════════════════════════════════════════════════
//  GitHub Gist — Sincronización de data.json
// ══════════════════════════════════════════════════════════════

fn configurar_gist(state: &mut AppState) {
    separador("🔑 Configurar GitHub Gist");

    println!("  Para sincronizar tus datos entre dispositivos necesitas");
    println!("  un token de GitHub (Personal Access Token).");
    println!();
    println!("  {}:", "Pasos".cyan().bold());
    println!("  1. Ve a: https://github.com/settings/tokens?type=beta");
    println!("  2. Haz clic en 'Generate new token'");
    println!("  3. Dale un nombre (ej: 'OmniPlanner Sync')");
    println!("  4. En permisos, selecciona: {} → Read and Write", "Gists".bold());
    println!("  5. Copia el token generado");
    println!();

    if state.sync.gist_configurado() {
        println!("  {} Token actual: {}...{}",
            "✓".green(),
            &state.sync.gist_token[..4.min(state.sync.gist_token.len())],
            if state.sync.gist_token.len() > 8 {
                &state.sync.gist_token[state.sync.gist_token.len() - 4..]
            } else { "" }
        );
        if !state.sync.gist_id.is_empty() {
            println!("  {} Gist ID: {}", "📎".to_string(), state.sync.gist_id.cyan());
        }
        if !Confirm::new()
            .with_prompt("  ¿Reconfigurar?")
            .default(false)
            .interact()
            .unwrap_or(false)
        {
            pausa();
            return;
        }
    }

    let token = match pedir_texto("Token de GitHub") {
        Some(t) => t.trim().to_string(),
        None => return,
    };

    // Verificar que el token funciona
    println!("  Verificando token...");
    match ureq::get("https://api.github.com/user")
        .set("Authorization", &format!("Bearer {}", token))
        .set("User-Agent", "OmniPlanner")
        .call()
    {
        Ok(resp) => {
            let body: serde_json::Value = resp.into_json().unwrap_or_default();
            let usuario = body["login"].as_str().unwrap_or("desconocido");
            println!("  {} Token válido. Usuario: {}", "✓".green(), usuario.cyan());
            state.sync.gist_token = token;

            // Buscar si ya hay un gist existente
            println!("  Buscando Gist existente...");
            match sync::gist::gist_buscar(&state.sync.gist_token) {
                Ok(Some(id)) => {
                    println!("  {} Gist encontrado: {}", "✓".green(), id.cyan());
                    state.sync.gist_id = id;
                }
                Ok(None) => {
                    println!("  {} No hay Gist previo. Se creará uno al hacer push.", "ℹ".to_string());
                    state.sync.gist_id = String::new();
                }
                Err(e) => println!("  {} Error buscando: {}", "⚠".yellow(), e),
            }
        }
        Err(e) => {
            println!("  {} Token inválido: {}", "✗".red(), e);
            println!("  Verifica que el token sea correcto y tenga permiso de Gists.");
        }
    }

    pausa();
}

fn gist_push_datos(state: &mut AppState) {
    separador("🔑 Subir datos via GitHub Gist");

    if !state.sync.gist_configurado() {
        println!("  {} Primero configura tu token de GitHub.", "✗".red());
        println!("  {} Usa '🔑 Configurar GitHub Gist (token)'", "💡".to_string());
        pausa();
        return;
    }

    let json = match serde_json::to_string_pretty(state) {
        Ok(j) => j,
        Err(e) => {
            println!("  {} Error serializando datos: {}", "✗".red(), e);
            pausa();
            return;
        }
    };

    let ahora = chrono::Local::now().format("%d/%m/%Y %H:%M").to_string();

    if state.sync.gist_id.is_empty() {
        println!("  Creando Gist privado...");
    } else {
        println!("  Actualizando Gist...");
    }

    match sync::gist::gist_push(&state.sync, &json) {
        Ok(gist_id) => {
            let es_nuevo = state.sync.gist_id.is_empty();
            state.sync.gist_id = gist_id.clone();
            if es_nuevo {
                println!("  {} Gist creado: {}", "✓".green(), gist_id.cyan());
            } else {
                println!("  {} Datos actualizados en Gist", "✓".green());
            }
            println!("  {} {} tareas, {} eventos, {} diagramas, {} canvas, {} recuerdos",
                "📦".to_string(),
                state.tasks.tareas.len(),
                state.agenda.eventos.len(),
                state.diagramas.len(),
                state.canvases.len(),
                state.memoria.recuerdos.len(),
            );
            println!("  {} {}", "🕐".to_string(), ahora);
            println!();
            println!("  {} Para descargar en otro dispositivo:", "💡".to_string());
            println!("  Usa el mismo token de GitHub y haz 'pull'.");
        }
        Err(e) => {
            println!("  {} Error: {}", "✗".red(), e);
        }
    }

    // Guardar el gist_id
    if let Err(e) = state.guardar() {
        println!("  {} Error guardando estado local: {}", "⚠".yellow(), e);
    }

    pausa();
}

fn gist_pull_datos(state: &mut AppState) {
    separador("🔑 Descargar datos via GitHub Gist");

    if !state.sync.gist_configurado() {
        println!("  {} Primero configura tu token de GitHub.", "✗".red());
        pausa();
        return;
    }

    if state.sync.gist_id.is_empty() {
        println!("  {} No hay Gist vinculado.", "✗".red());
        println!("  {} Usa 'Buscar Gist existente' o haz push primero.", "💡".to_string());
        pausa();
        return;
    }

    println!("  Descargando de GitHub Gist...");

    let contenido = match sync::gist::gist_pull(&state.sync) {
        Ok(c) => c,
        Err(e) => {
            println!("  {} Error: {}", "✗".red(), e);
            pausa();
            return;
        }
    };

    let remoto: AppState = match serde_json::from_str(&contenido) {
        Ok(s) => s,
        Err(e) => {
            println!("  {} Error parseando datos remotos: {}", "✗".red(), e);
            pausa();
            return;
        }
    };

    println!("\n  {} Datos remotos:", "📊".to_string());
    println!("    Tareas:    {}", remoto.tasks.tareas.len());
    println!("    Eventos:   {}", remoto.agenda.eventos.len());
    println!("    Canvases:  {}", remoto.canvases.len());
    println!("    Diagramas: {}", remoto.diagramas.len());
    println!("    Recuerdos: {}", remoto.memoria.recuerdos.len());

    println!("\n  {} Datos locales:", "📊".to_string());
    println!("    Tareas:    {}", state.tasks.tareas.len());
    println!("    Eventos:   {}", state.agenda.eventos.len());
    println!("    Canvases:  {}", state.canvases.len());
    println!("    Diagramas: {}", state.diagramas.len());
    println!("    Recuerdos: {}", state.memoria.recuerdos.len());

    println!("\n  {} Esto REEMPLAZARÁ todos tus datos locales.", "⚠".yellow());

    let confirmar = Confirm::new()
        .with_prompt("  ¿Continuar?")
        .default(false)
        .interact()
        .unwrap_or(false);

    if !confirmar {
        println!("  Cancelado.");
        pausa();
        return;
    }

    // Preservar config local de sync
    let sync_local = state.sync.clone();

    *state = remoto;
    state.sync.gist_token = sync_local.gist_token;
    state.sync.gist_id = sync_local.gist_id;
    state.sync.google_client_id = sync_local.google_client_id;
    state.sync.google_client_secret = sync_local.google_client_secret;
    state.sync.google_access_token = sync_local.google_access_token;
    state.sync.google_refresh_token = sync_local.google_refresh_token;
    state.sync.drive_file_id = sync_local.drive_file_id;
    state.sync.smtp_server = sync_local.smtp_server;
    state.sync.smtp_port = sync_local.smtp_port;
    state.sync.smtp_usuario = sync_local.smtp_usuario;
    state.sync.smtp_password = sync_local.smtp_password;
    state.sync.email_remitente = sync_local.email_remitente;
    state.sync.email_destinatario = sync_local.email_destinatario;

    if let Err(e) = state.guardar() {
        println!("  {} Error guardando: {}", "⚠".yellow(), e);
    }

    println!("  {} Datos descargados y aplicados.", "✓".green());
    pausa();
}

fn gist_buscar_existente(state: &mut AppState) {
    separador("🔑 Buscar Gist existente");

    if !state.sync.gist_configurado() {
        println!("  {} Primero configura tu token de GitHub.", "✗".red());
        pausa();
        return;
    }

    println!("  Buscando 'omniplanner_data.json' en tus Gists...");

    match sync::gist::gist_buscar(&state.sync.gist_token) {
        Ok(Some(gist_id)) => {
            println!("  {} Gist encontrado: {}", "✓".green(), gist_id.cyan());
            if Confirm::new()
                .with_prompt("  ¿Vincular este Gist para sincronizar?")
                .default(true)
                .interact()
                .unwrap_or(false)
            {
                state.sync.gist_id = gist_id;
                if let Err(e) = state.guardar() {
                    println!("  {} Error guardando: {}", "⚠".yellow(), e);
                }
                println!("  {} Vinculado. Ya puedes hacer pull para descargar los datos.", "✓".green());
            }
        }
        Ok(None) => {
            println!("  {} No se encontró ningún Gist de OmniPlanner.", "⚠".yellow());
            println!("  Haz push primero desde el dispositivo que tenga tus datos.");
        }
        Err(e) => {
            println!("  {} Error: {}", "✗".red(), e);
        }
    }
    pausa();
}

// ══════════════════════════════════════════════════════════════
//  Google Drive — Sincronización de data.json
// ══════════════════════════════════════════════════════════════

fn drive_push(state: &mut AppState) {
    separador("☁️  Subir datos a Google Drive");

    if !state.sync.google_autenticado() {
        println!("  {} Primero autentica tu cuenta de Google (Configurar Google Calendar).", "✗".red());
        pausa();
        return;
    }

    // Serializar estado completo
    let json = match serde_json::to_string_pretty(state) {
        Ok(j) => j,
        Err(e) => {
            println!("  {} Error serializando datos: {}", "✗".red(), e);
            pausa();
            return;
        }
    };

    let ahora = chrono::Local::now().format("%d/%m/%Y %H:%M").to_string();

    if state.sync.drive_file_id.is_empty() {
        println!("  Creando archivo en Google Drive...");
    } else {
        println!("  Actualizando archivo en Google Drive...");
    }

    let resultado = match sync::drive::drive_push(&state.sync, &json) {
        Ok(file_id) => Ok(file_id),
        Err(e) if e.contains("401") || e.contains("403") => {
            println!("  {} Token expirado, refrescando...", "🔄".to_string());
            match sync::calendario::google_refrescar_token(&mut state.sync) {
                Ok(()) => {
                    println!("  {} Token refrescado, reintentando...", "✓".green());
                    sync::drive::drive_push(&state.sync, &json)
                }
                Err(re) => {
                    println!("  {} No se pudo refrescar token: {}", "✗".red(), re);
                    println!("  {} Re-autentica Google desde Configurar Google Calendar.", "💡".to_string());
                    Err(e)
                }
            }
        }
        Err(e) => Err(e),
    };

    match resultado {
        Ok(file_id) => {
            if state.sync.drive_file_id.is_empty() {
                println!("  {} Archivo creado en Drive", "✓".green());
            } else {
                println!("  {} Datos actualizados en Drive", "✓".green());
            }
            state.sync.drive_file_id = file_id;
            println!("  {} {} tareas, {} eventos, {} diagramas, {} canvas, {} recuerdos",
                "📦".to_string(),
                state.tasks.tareas.len(),
                state.agenda.eventos.len(),
                state.diagramas.len(),
                state.canvases.len(),
                state.memoria.recuerdos.len(),
            );
            println!("  {} {}", "🕐".to_string(), ahora);
        }
        Err(e) => {
            println!("  {} Error: {}", "✗".red(), e);
        }
    }
    pausa();
}

fn drive_pull(state: &mut AppState) {
    separador("☁️  Descargar datos de Google Drive");

    if !state.sync.google_autenticado() {
        println!("  {} Primero autentica tu cuenta de Google.", "✗".red());
        pausa();
        return;
    }

    if state.sync.drive_file_id.is_empty() {
        println!("  {} No hay archivo de Drive vinculado.", "✗".red());
        println!("  {} Usa 'Buscar archivo en Drive' si ya subiste datos desde otro dispositivo.", "💡".to_string());
        pausa();
        return;
    }

    println!("  Descargando de Google Drive...");

    let contenido = match sync::drive::drive_pull(&state.sync) {
        Ok(c) => c,
        Err(e) if e.contains("401") || e.contains("403") => {
            println!("  {} Token expirado, refrescando...", "🔄".to_string());
            match sync::calendario::google_refrescar_token(&mut state.sync) {
                Ok(()) => {
                    println!("  {} Token refrescado, reintentando...", "✓".green());
                    match sync::drive::drive_pull(&state.sync) {
                        Ok(c) => c,
                        Err(e2) => {
                            println!("  {} Error: {}", "✗".red(), e2);
                            pausa();
                            return;
                        }
                    }
                }
                Err(re) => {
                    println!("  {} No se pudo refrescar token: {}", "✗".red(), re);
                    println!("  {} Re-autentica Google desde Configurar Google Calendar.", "💡".to_string());
                    pausa();
                    return;
                }
            }
        }
        Err(e) => {
            println!("  {} Error: {}", "✗".red(), e);
            pausa();
            return;
        }
    };

    let remoto: AppState = match serde_json::from_str(&contenido) {
        Ok(s) => s,
        Err(e) => {
            println!("  {} Error parseando datos remotos: {}", "✗".red(), e);
            pausa();
            return;
        }
    };

    println!("\n  {} Datos remotos:", "📊".to_string());
    println!("    Tareas:    {}", remoto.tasks.tareas.len());
    println!("    Eventos:   {}", remoto.agenda.eventos.len());
    println!("    Canvases:  {}", remoto.canvases.len());
    println!("    Diagramas: {}", remoto.diagramas.len());
    println!("    Recuerdos: {}", remoto.memoria.recuerdos.len());

    println!("\n  {} Datos locales:", "📊".to_string());
    println!("    Tareas:    {}", state.tasks.tareas.len());
    println!("    Eventos:   {}", state.agenda.eventos.len());
    println!("    Canvases:  {}", state.canvases.len());
    println!("    Diagramas: {}", state.diagramas.len());
    println!("    Recuerdos: {}", state.memoria.recuerdos.len());

    println!("\n  {} Esto REEMPLAZARÁ todos tus datos locales.", "⚠".yellow());

    let confirmar = Confirm::new()
        .with_prompt("  ¿Continuar?")
        .default(false)
        .interact()
        .unwrap_or(false);

    if !confirmar {
        println!("  {} Cancelado.", "✗".red());
        pausa();
        return;
    }

    // Preservar config de sync local
    let sync_local = state.sync.clone();
    *state = remoto;
    state.sync.google_client_id = sync_local.google_client_id;
    state.sync.google_client_secret = sync_local.google_client_secret;
    state.sync.google_access_token = sync_local.google_access_token;
    state.sync.google_refresh_token = sync_local.google_refresh_token;
    state.sync.drive_file_id = sync_local.drive_file_id;
    state.sync.smtp_server = sync_local.smtp_server;
    state.sync.smtp_port = sync_local.smtp_port;
    state.sync.smtp_usuario = sync_local.smtp_usuario;
    state.sync.smtp_password = sync_local.smtp_password;
    state.sync.email_remitente = sync_local.email_remitente;
    state.sync.email_destinatario = sync_local.email_destinatario;

    println!("  {} Datos descargados y aplicados.", "✓".green());
    pausa();
}

fn drive_buscar(state: &mut AppState) {
    separador("☁️  Buscar archivo en Google Drive");

    if !state.sync.google_autenticado() {
        println!("  {} Primero autentica tu cuenta de Google.", "✗".red());
        pausa();
        return;
    }

    println!("  Buscando 'omniplanner_data.json' en tu Drive...");

    let resultado = match sync::drive::drive_buscar(&state.sync) {
        Ok(r) => Ok(r),
        Err(e) if e.contains("401") || e.contains("403") => {
            println!("  {} Token expirado, refrescando...", "🔄".to_string());
            match sync::calendario::google_refrescar_token(&mut state.sync) {
                Ok(()) => sync::drive::drive_buscar(&state.sync),
                Err(re) => {
                    println!("  {} No se pudo refrescar token: {}", "✗".red(), re);
                    Err(e)
                }
            }
        }
        Err(e) => Err(e),
    };

    match resultado {
        Ok(Some(file_id)) => {
            println!("  {} Archivo encontrado: {}", "✓".green(), file_id.cyan());
            if Confirm::new()
                .with_prompt("  ¿Vincular este archivo para sincronizar?")
                .default(true)
                .interact()
                .unwrap_or(false)
            {
                state.sync.drive_file_id = file_id;
                println!("  {} Vinculado. Ya puedes hacer pull para descargar los datos.", "✓".green());
            }
        }
        Ok(None) => {
            println!("  {} No se encontró ningún archivo de OmniPlanner en Drive.", "⚠".yellow());
            println!("  Haz push primero desde el dispositivo que tenga tus datos.");
        }
        Err(e) => {
            println!("  {} Error: {}", "✗".red(), e);
        }
    }
    pausa();
}

fn exportar_ics(state: &AppState) {
    separador("📅 Exportar a .ics");

    let opciones = &["Solo eventos", "Solo tareas", "Todo"];
    let sel = match menu("¿Qué exportar?", opciones) { Some(i) => i, None => return };

    let eventos: Vec<&Evento> = if sel == 0 || sel == 2 {
        state.agenda.eventos.iter().collect()
    } else {
        vec![]
    };
    let tareas: Vec<&Task> = if sel == 1 || sel == 2 {
        state.tasks.tareas.iter().collect()
    } else {
        vec![]
    };

    if eventos.is_empty() && tareas.is_empty() {
        println!("  {}", "No hay datos para exportar.".yellow());
        pausa();
        return;
    }

    let ical = sync::calendario::exportar_ical(&eventos, &tareas);
    let archivo = match pedir_texto("Archivo de salida (ej: omniplanner.ics)") { Some(t) => t, None => return };

    match std::fs::write(&archivo, &ical) {
        Ok(_) => println!(
            "  {} Exportado a '{}' ({} eventos, {} tareas)",
            "✓".green(),
            archivo,
            eventos.len(),
            tareas.len()
        ),
        Err(e) => println!("  {} Error: {}", "✗".red(), e),
    }
    pausa();
}

fn importar_ics(state: &mut AppState) {
    separador("📅 Importar .ics");
    let archivo = match pedir_texto("Archivo .ics a importar") { Some(t) => t, None => return };

    let contenido = match std::fs::read_to_string(&archivo) {
        Ok(c) => c,
        Err(e) => {
            println!("  {} Error leyendo archivo: {}", "✗".red(), e);
            pausa();
            return;
        }
    };

    let eventos = sync::calendario::importar_ical(&contenido);

    if eventos.is_empty() {
        println!("  {}", "No se encontraron eventos en el archivo.".yellow());
        pausa();
        return;
    }

    println!("  Encontrados {} eventos:", eventos.len());
    for (i, e) in eventos.iter().enumerate() {
        let fin = e
            .hora_fin
            .map(|h| format!("-{}", h.format("%H:%M")))
            .unwrap_or_default();
        println!(
            "    {}. {} | {} {}{}",
            i + 1,
            e.titulo,
            e.fecha.format("%d/%m/%Y"),
            e.hora_inicio.format("%H:%M"),
            fin
        );
    }

    if Confirm::new()
        .with_prompt("  ¿Importar todos?")
        .default(true)
        .interact()
        .unwrap_or(false)
    {
        let mut count = 0;
        for ei in eventos {
            let evento = Evento::new(
                ei.titulo,
                ei.descripcion,
                TipoEvento::Otro("Importado".to_string()),
                ei.fecha,
                ei.hora_inicio,
                ei.hora_fin,
            );
            state.agenda.agregar_evento(evento);
            count += 1;
        }
        println!("  {} {} eventos importados", "✓".green(), count);
    }
    pausa();
}

fn resync_google(state: &mut AppState) {
    if !state.sync.google_autenticado() {
        println!("  {} Primero configura y autentica Google Calendar", "✗".red());
        pausa();
        return;
    }

    separador("🔄 Re-sincronizar todo");
    println!("  Esto limpiará el registro de sincronización y enviará");
    println!("  todos los eventos y tareas de nuevo a Google Calendar.");
    println!();
    println!("  {} eventos registrados, {} tareas registradas",
        state.sync.mapa_eventos.len(), state.sync.mapa_tareas.len());

    if !Confirm::new()
        .with_prompt("  ¿Continuar?")
        .default(false)
        .interact()
        .unwrap_or(false)
    {
        return;
    }

    state.sync.mapa_eventos.clear();
    state.sync.mapa_tareas.clear();
    println!("  {} Mapeo limpiado. Sincronizando...", "✓".green());
    println!();

    sync_push_google(state);
}

fn sync_push_google(state: &mut AppState) {
    if !state.sync.google_autenticado() {
        println!(
            "  {} Primero configura y autentica Google Calendar",
            "✗".red()
        );
        pausa();
        return;
    }

    separador("📅 Sincronizar → Google Calendar");

    let opciones = &["Sincronizar eventos", "Sincronizar tareas", "Sincronizar todo"];
    let sel = match menu("¿Qué sincronizar?", opciones) { Some(i) => i, None => return };

    if sel == 0 || sel == 2 {
        let mut ok = 0;
        let mut err = 0;
        let ids: Vec<(String, String)> = state
            .agenda
            .eventos
            .iter()
            .filter(|e| !state.sync.mapa_eventos.contains_key(&e.id))
            .map(|e| (e.id.clone(), e.titulo.clone()))
            .collect();

        for (eid, titulo) in &ids {
            let evento = state.agenda.eventos.iter().find(|e| e.id == *eid).unwrap();
            match sync::calendario::google_crear_evento(&state.sync, evento) {
                Ok(gid) => {
                    state.sync.mapa_eventos.insert(eid.clone(), gid);
                    ok += 1;
                }
                Err(e) => {
                    println!("  {} Error sincronizando '{}': {}", "✗".red(), titulo, e);
                    err += 1;
                }
            }
        }
        println!(
            "  📅 Eventos: {} sincronizados, {} errores",
            ok, err
        );
    }

    if sel == 1 || sel == 2 {
        let mut ok = 0;
        let mut err = 0;
        let ids: Vec<(String, String)> = state
            .tasks
            .tareas
            .iter()
            .filter(|t| !state.sync.mapa_tareas.contains_key(&t.id))
            .map(|t| (t.id.clone(), t.titulo.clone()))
            .collect();

        for (tid, titulo) in &ids {
            let tarea = state.tasks.tareas.iter().find(|t| t.id == *tid).unwrap();
            match sync::calendario::google_crear_evento_tarea(&state.sync, tarea) {
                Ok(gid) => {
                    state.sync.mapa_tareas.insert(tid.clone(), gid);
                    ok += 1;
                }
                Err(e) => {
                    println!("  {} Error sincronizando '{}': {}", "✗".red(), titulo, e);
                    err += 1;
                }
            }
        }
        println!(
            "  📋 Tareas: {} sincronizadas, {} errores",
            ok, err
        );
    }

    pausa();
}

fn sync_pull_google(state: &mut AppState) {
    if !state.sync.google_autenticado() {
        println!(
            "  {} Primero configura y autentica Google Calendar",
            "✗".red()
        );
        pausa();
        return;
    }

    separador("📅 Importar ← Google Calendar");
    let fecha = match pedir_fecha("Fecha a consultar") {
        Some(f) => f,
        None => return,
    };

    match sync::calendario::google_listar_eventos(&state.sync, fecha) {
        Ok(eventos) => {
            if eventos.is_empty() {
                println!("  {}", "No hay eventos para esa fecha.".yellow());
            } else {
                println!("  Encontrados {} eventos:", eventos.len());
                for (i, e) in eventos.iter().enumerate() {
                    let fin = e
                        .hora_fin
                        .map(|h| format!("-{}", h.format("%H:%M")))
                        .unwrap_or_default();
                    println!(
                        "    {}. {} | {}{}",
                        i + 1,
                        e.titulo,
                        e.hora_inicio.format("%H:%M"),
                        fin
                    );
                }

                if Confirm::new()
                    .with_prompt("  ¿Importar a la agenda?")
                    .default(true)
                    .interact()
                    .unwrap_or(false)
                {
                    let mut count = 0;
                    for ei in eventos {
                        let evento = Evento::new(
                            ei.titulo,
                            ei.descripcion,
                            TipoEvento::Otro("Google Calendar".to_string()),
                            ei.fecha,
                            ei.hora_inicio,
                            ei.hora_fin,
                        );
                        state.agenda.agregar_evento(evento);
                        count += 1;
                    }
                    println!("  {} {} eventos importados", "✓".green(), count);
                }
            }
        }
        Err(e) => {
            println!("  {} Error: {}", "✗".red(), e);
        }
    }
    pausa();
}

fn enviar_resumen(state: &AppState) {
    if !state.sync.email_configurado() {
        println!("  {} Primero configura el email SMTP", "✗".red());
        pausa();
        return;
    }

    let hoy = Local::now().date_naive();
    let tareas: Vec<&Task> = state.tasks.listar_por_fecha(hoy);
    let eventos: Vec<&Evento> = state.agenda.eventos_del_dia(hoy);
    let follow_ups: Vec<&Task> = state
        .tasks
        .listar_follow_ups()
        .into_iter()
        .filter(|t| t.follow_up.map(|f| f.date() == hoy).unwrap_or(false))
        .collect();

    match sync::correo::enviar_resumen_diario(&state.sync, &tareas, &eventos, &follow_ups) {
        Ok(()) => println!(
            "  {} Resumen diario enviado a {}",
            "✓".green(),
            state.sync.email_destinatario
        ),
        Err(e) => println!("  {} Error: {}", "✗".red(), e),
    }
    pausa();
}

fn enviar_recordatorio(state: &AppState) {
    if !state.sync.email_configurado() {
        println!("  {} Primero configura el email SMTP", "✗".red());
        pausa();
        return;
    }

    if state.tasks.tareas.is_empty() {
        println!("  {}", "No hay tareas.".yellow());
        pausa();
        return;
    }

    let nombres: Vec<String> = state
        .tasks
        .tareas
        .iter()
        .map(|t| format!("{} - {} [{}]", t.id, t.titulo, t.prioridad))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    let idx = match menu("¿De cuál tarea enviar recordatorio?", &refs) { Some(i) => i, None => return };
    let tarea = &state.tasks.tareas[idx];

    match sync::correo::enviar_recordatorio_tarea(&state.sync, tarea) {
        Ok(()) => println!(
            "  {} Recordatorio enviado a {}",
            "✓".green(),
            state.sync.email_destinatario
        ),
        Err(e) => println!("  {} Error: {}", "✗".red(), e),
    }
    pausa();
}

fn enviar_followup_email(state: &AppState) {
    if !state.sync.email_configurado() {
        println!("  {} Primero configura el email SMTP", "✗".red());
        pausa();
        return;
    }

    let follow_ups: Vec<&Task> = state.tasks.listar_follow_ups();
    if follow_ups.is_empty() {
        println!("  {}", "No hay tareas con follow-up.".yellow());
        pausa();
        return;
    }

    let nombres: Vec<String> = follow_ups
        .iter()
        .map(|t| {
            format!(
                "{} - {} (follow-up: {})",
                t.id,
                t.titulo,
                t.follow_up.unwrap().format("%d/%m %H:%M")
            )
        })
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    let idx = match menu("¿De cuál tarea?", &refs) { Some(i) => i, None => return };
    let tarea = follow_ups[idx];
    let mensaje = match pedir_texto("Mensaje del follow-up") { Some(t) => t, None => return };

    match sync::correo::enviar_follow_up(&state.sync, tarea, &mensaje) {
        Ok(()) => println!(
            "  {} Follow-up enviado a {}",
            "✓".green(),
            state.sync.email_destinatario
        ),
        Err(e) => println!("  {} Error: {}", "✗".red(), e),
    }
    pausa();
}

fn iniciar_dashboard_web(state: &mut AppState) {
    separador("📲 Dashboard Web — Ver desde el celular");

    println!("  Esto inicia un servidor web local en tu PC.");
    println!("  Desde tu celular (en la misma red WiFi), abre la URL");
    println!("  que aparecerá a continuación en el navegador.");
    println!();
    println!("  {} Los datos se capturan al momento de iniciar.", "Nota:".yellow().bold());
    println!("  Si agregas algo nuevo, reinicia el dashboard.");
    println!();

    let puerto: u16 = Input::new()
        .with_prompt("  Puerto (recomendado: 8080)")
        .default(8080u16)
        .interact_text()
        .unwrap_or(8080);

    match sync::servidor::iniciar_servidor(state, puerto) {
        Ok(url) => {
            println!();
            println!("  {}", "╔══════════════════════════════════════════════╗".green());
            println!("  {} Dashboard disponible en:                   {}", "║".green(), "║".green());
            println!("  {}   {}   {}", "║".green(), url.cyan().bold(), "║".green());
            println!("  {} Se refresca automáticamente cada 30 seg    {}", "║".green(), "║".green());
            println!("  {}", "╚══════════════════════════════════════════════╝".green());
            println!();
            println!("  {} También disponible:", "📡".to_string());
            println!("    {}  → Dashboard visual", format!("{}/", url).cyan());
            println!("    {}  → Datos JSON (para apps)", format!("{}/api/state.json", url).cyan());
            println!();
            println!("  El servidor sigue activo mientras OmniPlanner esté abierto.");
            println!("  Puedes seguir usando el menú normalmente.");
        }
        Err(e) => {
            println!("  {} Error iniciando servidor: {}", "✗".red(), e);
            println!("  Intenta con otro puerto (ej: 8081, 9090).");
        }
    }
    pausa();
}

fn exportar_estado(state: &AppState) {
    separador("💾 Exportar estado completo");

    println!("  Exporta TODOS tus datos (tareas, eventos, diagramas, etc.)");
    println!("  a un archivo JSON que puedes guardar en Google Drive, USB,");
    println!("  enviarlo por email, etc.");
    println!();

    let nombre = match pedir_texto("Archivo de salida (ej: omniplanner_backup.json)") { Some(t) => t, None => return };

    match serde_json::to_string_pretty(state) {
        Ok(json) => {
            match std::fs::write(&nombre, &json) {
                Ok(_) => {
                    let tamano = json.len() as f64 / 1024.0;
                    println!("  {} Estado exportado a '{}' ({:.1} KB)", "✓".green(), nombre, tamano);
                    println!("  Contiene: {} tareas, {} eventos, {} diagramas, {} canvas, {} recuerdos",
                        state.tasks.tareas.len(),
                        state.agenda.eventos.len(),
                        state.diagramas.len(),
                        state.canvases.len(),
                        state.memoria.recuerdos.len(),
                    );
                    println!();
                    println!("  {} Para sincronizar con otro dispositivo:", "💡".to_string());
                    println!("    1. Sube este archivo a Google Drive / OneDrive / Dropbox");
                    println!("    2. En el otro dispositivo, descárgalo e impórtalo");
                }
                Err(e) => println!("  {} Error escribiendo archivo: {}", "✗".red(), e),
            }
        }
        Err(e) => println!("  {} Error serializando: {}", "✗".red(), e),
    }
    pausa();
}

fn importar_estado(state: &mut AppState) {
    separador("💾 Importar estado completo");

    println!("  {} Esto reemplazará TODOS tus datos actuales.", "⚠ ATENCIÓN:".red().bold());
    println!("  Se recomienda exportar un backup antes de importar.");
    println!();

    let archivo = match pedir_texto("Archivo JSON a importar") { Some(t) => t, None => return };

    let contenido = match std::fs::read_to_string(&archivo) {
        Ok(c) => c,
        Err(e) => {
            println!("  {} Error leyendo archivo: {}", "✗".red(), e);
            pausa();
            return;
        }
    };

    let nuevo: AppState = match serde_json::from_str(&contenido) {
        Ok(s) => s,
        Err(e) => {
            println!("  {} Error: el archivo no es un estado válido de OmniPlanner", "✗".red());
            println!("  Detalle: {}", e);
            pausa();
            return;
        }
    };

    println!("  El archivo contiene:");
    println!("    📋 {} tareas", nuevo.tasks.tareas.len());
    println!("    📅 {} eventos", nuevo.agenda.eventos.len());
    println!("    📊 {} diagramas", nuevo.diagramas.len());
    println!("    ✏️  {} canvas", nuevo.canvases.len());
    println!("    🧠 {} recuerdos", nuevo.memoria.recuerdos.len());
    println!();

    let opciones = &[
        "🔄 Reemplazar todo (sobreescribir)",
        "➕ Mezclar (agregar lo nuevo sin borrar lo existente)",
        "❌ Cancelar",
    ];

    match menu("¿Cómo importar?", opciones) {
        Some(0) => {
            if Confirm::new()
                .with_prompt("  ¿Estás seguro? Se perderán los datos actuales")
                .default(false)
                .interact()
                .unwrap_or(false)
            {
                *state = nuevo;
                println!("  {} Estado importado correctamente", "✓".green());
            }
        }
        Some(1) => {
            // Mezclar: agregar items que no existan por ID
            let mut tareas_nuevas = 0;
            for t in nuevo.tasks.tareas {
                if state.tasks.buscar(&t.id).is_none() {
                    state.tasks.agregar(t);
                    tareas_nuevas += 1;
                }
            }

            let mut eventos_nuevos = 0;
            let ids_existentes: Vec<String> = state.agenda.eventos.iter().map(|e| e.id.clone()).collect();
            for e in nuevo.agenda.eventos {
                if !ids_existentes.contains(&e.id) {
                    state.agenda.agregar_evento(e);
                    eventos_nuevos += 1;
                }
            }

            let mut diagramas_nuevos = 0;
            let ids_d: Vec<String> = state.diagramas.iter().map(|d| d.id.clone()).collect();
            for d in nuevo.diagramas {
                if !ids_d.contains(&d.id) {
                    state.diagramas.push(d);
                    diagramas_nuevos += 1;
                }
            }

            let mut canvas_nuevos = 0;
            let ids_c: Vec<String> = state.canvases.iter().map(|c| c.id.clone()).collect();
            for c in nuevo.canvases {
                if !ids_c.contains(&c.id) {
                    state.canvases.push(c);
                    canvas_nuevos += 1;
                }
            }

            let mut recuerdos_nuevos = 0;
            let ids_r: Vec<String> = state.memoria.recuerdos.iter().map(|r| r.id.clone()).collect();
            for r in nuevo.memoria.recuerdos {
                if !ids_r.contains(&r.id) {
                    state.memoria.agregar_recuerdo(r);
                    recuerdos_nuevos += 1;
                }
            }

            println!("  {} Mezclado:", "✓".green());
            println!("    +{} tareas, +{} eventos, +{} diagramas, +{} canvas, +{} recuerdos",
                tareas_nuevas, eventos_nuevos, diagramas_nuevos, canvas_nuevos, recuerdos_nuevos);
        }
        _ => {
            println!("  Importación cancelada.");
        }
    }
    pausa();
}

fn configurar_google(state: &mut AppState) {
    separador("⚙️  Configurar Google Calendar");

    println!("  Para usar Google Calendar necesitas:");
    println!("  1. Ir a https://console.cloud.google.com");
    println!("  2. Crear un proyecto y habilitar Google Calendar API");
    println!("  3. Crear credenciales OAuth 2.0 (tipo Escritorio)");
    println!("  4. Copiar el Client ID y Client Secret");
    println!();

    if state.sync.google_configurado() {
        println!(
            "  Configuración actual: Client ID = {}...{}",
            &state.sync.google_client_id[..8.min(state.sync.google_client_id.len())],
            if state.sync.google_client_id.len() > 20 {
                &state.sync.google_client_id[state.sync.google_client_id.len() - 8..]
            } else {
                ""
            }
        );
        if !Confirm::new()
            .with_prompt("  ¿Reconfigurar?")
            .default(false)
            .interact()
            .unwrap_or(false)
        {
            // Solo re-autenticar si ya está configurado
            if !state.sync.google_autenticado() {
                return autenticar_google(state);
            }
            pausa();
            return;
        }
    }

    let client_id = match pedir_texto("Client ID") { Some(t) => t, None => return };
    let client_secret = match pedir_texto("Client Secret") { Some(t) => t, None => return };
    let calendar_id = pedir_texto_opcional("Calendar ID (vacío = primary)");

    state.sync.google_client_id = client_id;
    state.sync.google_client_secret = client_secret;
    if !calendar_id.is_empty() {
        state.sync.google_calendar_id = calendar_id;
    }

    println!("  {} Credenciales guardadas", "✓".green());

    if Confirm::new()
        .with_prompt("  ¿Autenticar ahora?")
        .default(true)
        .interact()
        .unwrap_or(false)
    {
        autenticar_google(state);
    } else {
        pausa();
    }
}

fn autenticar_google(state: &mut AppState) {
    let url = sync::calendario::google_auth_url(&state.sync);
    println!("\n  Abriendo navegador para autorización...");
    println!("  Inicia sesión con tu cuenta de Google y autoriza OmniPlanner.");
    let _ = open::that(&url);

    println!("  {} Esperando autorización en el navegador...", "⏳".to_string());

    let codigo = match sync::calendario::escuchar_codigo_oauth() {
        Ok(c) => c,
        Err(e) => {
            println!("  {} Error capturando código: {}", "✗".red(), e);
            println!("  Intenta pegar el código manualmente:");
            let c = match pedir_texto("Código de autorización") { Some(t) => t, None => return };
            c
        }
    };

    match sync::calendario::google_intercambiar_codigo(&mut state.sync, &codigo) {
        Ok(()) => println!(
            "  {} Google Calendar conectado exitosamente",
            "✓".green()
        ),
        Err(e) => println!("  {} Error: {}", "✗".red(), e),
    }
    pausa();
}

fn configurar_email(state: &mut AppState) {
    separador("⚙️  Configurar Email (SMTP)");

    println!(
        "  {} La contraseña se almacena localmente en texto plano.",
        "Nota:".yellow().bold()
    );
    println!("  Para Gmail, usa una 'Contraseña de aplicación' (no tu contraseña normal).");
    println!("  https://myaccount.google.com/apppasswords");
    println!();

    let presets = &[
        "Gmail (smtp.gmail.com)",
        "Outlook (smtp.office365.com)",
        "Otro servidor",
    ];
    let pi = match menu("Proveedor", presets) { Some(i) => i, None => return };

    let server = match pi {
        0 => "smtp.gmail.com".to_string(),
        1 => "smtp.office365.com".to_string(),
        _ => match pedir_texto("Servidor SMTP") { Some(t) => t, None => return },
    };

    let usuario = match pedir_texto("Usuario SMTP (email)") { Some(t) => t, None => return };
    let password = match pedir_texto("Contraseña / App Password") { Some(t) => t, None => return };
    let remitente = match pedir_texto("Email remitente (ej: Tu Nombre <tu@email.com>)") { Some(t) => t, None => return };
    let destinatario = match pedir_texto("Email destinatario (para recibir notificaciones)") { Some(t) => t, None => return };

    state.sync.smtp_server = server;
    state.sync.smtp_port = 587;
    state.sync.smtp_usuario = usuario;
    state.sync.smtp_password = password;
    state.sync.email_remitente = remitente;
    state.sync.email_destinatario = destinatario;

    println!("  {} Configuración SMTP guardada", "✓".green());

    if Confirm::new()
        .with_prompt("  ¿Enviar email de prueba?")
        .default(true)
        .interact()
        .unwrap_or(false)
    {
        match sync::correo::enviar_correo(
            &state.sync,
            "OmniPlanner — Email de prueba",
            "Si recibes este email, la configuración es correcta.\n\n— OmniPlanner",
        ) {
            Ok(()) => println!("  {} Email de prueba enviado exitosamente", "✓".green()),
            Err(e) => println!("  {} Error: {}", "✗".red(), e),
        }
    }

    pausa();
}

// ══════════════════════════════════════════════════════════════
//  Módulo: REPORTES (Diario y Semanal)
// ══════════════════════════════════════════════════════════════

fn menu_reportes(state: &mut AppState) {
    loop {
        limpiar();
        separador("📄 REPORTES");
        println!("  Genera un resumen imprimible de tus actividades.");
        println!();

        let opciones = &[
            "📋 Reporte del día (hoy)",
            "📋 Reporte de una fecha específica",
            "📅 Reporte semanal (esta semana)",
            "📅 Reporte semanal (semana de una fecha)",
            "💾 Exportar reporte a archivo .txt",
            "← Volver al menú",
        ];

        match menu("¿Qué reporte deseas?", opciones) {
            Some(0) => {
                let hoy = Local::now().date_naive();
                let reporte = generar_reporte_diario(state, hoy);
                limpiar();
                println!("{}", reporte);
                pausa();
            }
            Some(1) => {
                if let Some(fecha) = pedir_fecha("Fecha del reporte") {
                    let reporte = generar_reporte_diario(state, fecha);
                    limpiar();
                    println!("{}", reporte);
                    pausa();
                }
            }
            Some(2) => {
                let hoy = Local::now().date_naive();
                let inicio = hoy - Duration::days(hoy.weekday().num_days_from_monday() as i64);
                let reporte = generar_reporte_semanal(state, inicio);
                limpiar();
                println!("{}", reporte);
                pausa();
            }
            Some(3) => {
                if let Some(fecha) = pedir_fecha("Cualquier día de la semana deseada") {
                    let inicio = fecha - Duration::days(fecha.weekday().num_days_from_monday() as i64);
                    let reporte = generar_reporte_semanal(state, inicio);
                    limpiar();
                    println!("{}", reporte);
                    pausa();
                }
            }
            Some(4) => {
                let tipos = &["Diario (hoy)", "Semanal (esta semana)"];
                let ti = match menu("Tipo de reporte", tipos) { Some(i) => i, None => continue };
                let hoy = Local::now().date_naive();
                let reporte = if ti == 0 {
                    generar_reporte_diario(state, hoy)
                } else {
                    let inicio = hoy - Duration::days(hoy.weekday().num_days_from_monday() as i64);
                    generar_reporte_semanal(state, inicio)
                };
                let nombre = match pedir_texto("Nombre del archivo (ej: reporte.txt)") { Some(t) => t, None => continue };
                match std::fs::write(&nombre, &reporte) {
                    Ok(_) => println!("  {} Reporte guardado en '{}'", "✓".green(), nombre),
                    Err(e) => println!("  {} Error: {}", "✗".red(), e),
                }
                pausa();
            }
            _ => return,
        }
    }
}

fn nombre_dia_es(wd: Weekday) -> &'static str {
    match wd {
        Weekday::Mon => "Lunes",
        Weekday::Tue => "Martes",
        Weekday::Wed => "Miércoles",
        Weekday::Thu => "Jueves",
        Weekday::Fri => "Viernes",
        Weekday::Sat => "Sábado",
        Weekday::Sun => "Domingo",
    }
}

fn nombre_mes_es(m: u32) -> &'static str {
    match m {
        1 => "Enero", 2 => "Febrero", 3 => "Marzo", 4 => "Abril",
        5 => "Mayo", 6 => "Junio", 7 => "Julio", 8 => "Agosto",
        9 => "Septiembre", 10 => "Octubre", 11 => "Noviembre", 12 => "Diciembre",
        _ => "",
    }
}

fn generar_reporte_diario(state: &AppState, fecha: NaiveDate) -> String {
    let dia = nombre_dia_es(fecha.weekday());
    let mes = nombre_mes_es(fecha.month());
    let mut r = String::new();

    r.push_str("╔══════════════════════════════════════════════════════════╗\n");
    r.push_str("║              OMNIPLANNER — REPORTE DIARIO               ║\n");
    r.push_str("╚══════════════════════════════════════════════════════════╝\n");
    r.push_str(&format!("\n  Fecha: {} {} de {} de {}\n", dia, fecha.day(), mes, fecha.year()));
    r.push_str(&format!("  Generado: {}\n", Local::now().format("%d/%m/%Y %H:%M")));
    r.push_str("\n──────────────────────────────────────────────────────────\n");

    // Tareas del día
    let tareas = state.tasks.listar_por_fecha(fecha);
    r.push_str(&format!("\n  📋 TAREAS DEL DÍA ({})\n\n", tareas.len()));
    if tareas.is_empty() {
        r.push_str("    (sin tareas para este día)\n");
    } else {
        for t in &tareas {
            let icono = match t.estado {
                TaskStatus::Completada => "✅",
                TaskStatus::EnProgreso => "🔄",
                TaskStatus::Cancelada => "❌",
                TaskStatus::Pendiente => "⬜",
            };
            r.push_str(&format!("    {} {} - {} [{}]\n", icono, t.hora.format("%H:%M"), t.titulo, t.prioridad));
            if !t.descripcion.is_empty() {
                r.push_str(&format!("       {}\n", t.descripcion));
            }
            if !t.etiquetas.is_empty() {
                r.push_str(&format!("       Etiquetas: {}\n", t.etiquetas.join(", ")));
            }
        }
    }

    // Eventos del día
    let eventos = state.agenda.eventos_del_dia(fecha);
    r.push_str(&format!("\n  📅 EVENTOS ({})\n\n", eventos.len()));
    if eventos.is_empty() {
        r.push_str("    (sin eventos para este día)\n");
    } else {
        for e in &eventos {
            let fin = e.hora_fin.map(|h| format!(" - {}", h.format("%H:%M"))).unwrap_or_default();
            r.push_str(&format!("    📌 {}{} {} ({})\n", e.hora_inicio.format("%H:%M"), fin, e.titulo, e.tipo));
            if !e.descripcion.is_empty() {
                r.push_str(&format!("       {}\n", e.descripcion));
            }
        }
    }

    // Horarios de escritura
    let horarios = state.agenda.horarios_del_dia(fecha.weekday());
    if !horarios.is_empty() {
        r.push_str(&format!("\n  ✏️  HORARIOS DE ESCRITURA ({})\n\n", horarios.len()));
        for h in &horarios {
            r.push_str(&format!("    🖊️  {} - {} {}\n", h.hora_inicio.format("%H:%M"), h.hora_fin.format("%H:%M"), h.descripcion));
        }
    }

    // Follow-ups del día
    let follow_ups: Vec<_> = state.tasks.listar_follow_ups()
        .into_iter()
        .filter(|t| t.follow_up.map(|f| f.date() == fecha).unwrap_or(false))
        .collect();
    if !follow_ups.is_empty() {
        r.push_str(&format!("\n  🔔 FOLLOW-UPS ({})\n\n", follow_ups.len()));
        for t in &follow_ups {
            r.push_str(&format!("    ↻ {} {} (tarea: {})\n",
                t.follow_up.unwrap().time().format("%H:%M"), t.titulo, t.estado));
        }
    }

    // Tareas pendientes globales
    let pendientes = state.tasks.listar_pendientes();
    let otras_pendientes: Vec<_> = pendientes.iter()
        .filter(|t| t.fecha != fecha)
        .collect();
    if !otras_pendientes.is_empty() {
        r.push_str(&format!("\n  ⏳ OTRAS TAREAS PENDIENTES ({})\n\n", otras_pendientes.len()));
        for t in otras_pendientes.iter().take(10) {
            r.push_str(&format!("    ⬜ {} {} - {} [{}]\n",
                t.fecha.format("%d/%m"), t.hora.format("%H:%M"), t.titulo, t.prioridad));
        }
        if otras_pendientes.len() > 10 {
            r.push_str(&format!("    ... y {} más\n", otras_pendientes.len() - 10));
        }
    }

    r.push_str("\n══════════════════════════════════════════════════════════\n");
    r
}

fn generar_reporte_semanal(state: &AppState, lunes: NaiveDate) -> String {
    let domingo = lunes + Duration::days(6);
    let mes_ini = nombre_mes_es(lunes.month());
    let mes_fin = nombre_mes_es(domingo.month());
    let mut r = String::new();

    r.push_str("╔══════════════════════════════════════════════════════════╗\n");
    r.push_str("║             OMNIPLANNER — REPORTE SEMANAL               ║\n");
    r.push_str("╚══════════════════════════════════════════════════════════╝\n");
    r.push_str(&format!("\n  Semana: {} {} {} — {} {} {}\n",
        lunes.day(), mes_ini, lunes.year(),
        domingo.day(), mes_fin, domingo.year()));
    r.push_str(&format!("  Generado: {}\n", Local::now().format("%d/%m/%Y %H:%M")));

    // Resumen total de la semana
    let mut total_tareas = 0;
    let mut total_completadas = 0;
    let mut total_eventos = 0;

    for i in 0..7 {
        let dia = lunes + Duration::days(i);
        let tareas = state.tasks.listar_por_fecha(dia);
        total_tareas += tareas.len();
        total_completadas += tareas.iter().filter(|t| t.estado == TaskStatus::Completada).count();
        total_eventos += state.agenda.eventos_del_dia(dia).len();
    }

    r.push_str(&format!("\n  📊 RESUMEN: {} tareas ({} completadas), {} eventos\n",
        total_tareas, total_completadas, total_eventos));

    // Día por día
    for i in 0..7 {
        let dia = lunes + Duration::days(i);
        let nombre = nombre_dia_es(dia.weekday());
        let mes = nombre_mes_es(dia.month());

        r.push_str(&format!("\n──────────────────────────────────────────────────────────\n"));
        r.push_str(&format!("  {} {} de {} de {}\n", nombre, dia.day(), mes, dia.year()));

        let tareas = state.tasks.listar_por_fecha(dia);
        let eventos = state.agenda.eventos_del_dia(dia);
        let horarios = state.agenda.horarios_del_dia(dia.weekday());

        if tareas.is_empty() && eventos.is_empty() && horarios.is_empty() {
            r.push_str("    ✨ Día libre\n");
            continue;
        }

        for t in &tareas {
            let icono = match t.estado {
                TaskStatus::Completada => "✅",
                TaskStatus::EnProgreso => "🔄",
                TaskStatus::Cancelada => "❌",
                TaskStatus::Pendiente => "⬜",
            };
            r.push_str(&format!("    {} {} {} [{}]\n", icono, t.hora.format("%H:%M"), t.titulo, t.prioridad));
        }

        for e in &eventos {
            let fin = e.hora_fin.map(|h| format!("-{}", h.format("%H:%M"))).unwrap_or_default();
            r.push_str(&format!("    📌 {}{} {} ({})\n", e.hora_inicio.format("%H:%M"), fin, e.titulo, e.tipo));
        }

        for h in &horarios {
            r.push_str(&format!("    🖊️  {}-{} {}\n", h.hora_inicio.format("%H:%M"), h.hora_fin.format("%H:%M"), h.descripcion));
        }
    }

    // Follow-ups de la semana
    let follow_ups: Vec<_> = state.tasks.listar_follow_ups()
        .into_iter()
        .filter(|t| {
            t.follow_up.map(|f| {
                let d = f.date();
                d >= lunes && d <= domingo
            }).unwrap_or(false)
        })
        .collect();
    if !follow_ups.is_empty() {
        r.push_str("\n──────────────────────────────────────────────────────────\n");
        r.push_str(&format!("  🔔 FOLLOW-UPS DE LA SEMANA ({})\n\n", follow_ups.len()));
        for t in &follow_ups {
            let fu = t.follow_up.unwrap();
            r.push_str(&format!("    ↻ {} {} — {}\n",
                fu.format("%d/%m %H:%M"), t.titulo, t.estado));
        }
    }

    r.push_str("\n══════════════════════════════════════════════════════════\n");
    r
}

// ══════════════════════════════════════════════════════════════
//  MAIN — Menú principal interactivo
// ══════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════
//  Módulo ML — Inteligencia Artificial
// ══════════════════════════════════════════════════════════════

fn menu_ml(state: &mut AppState) {
    loop {
        limpiar();
        separador("🤖 INTELIGENCIA ARTIFICIAL");

        // Resumen
        println!("  {} modelos entrenados — {} datasets cargados",
            state.ml.modelos.len().to_string().green(),
            state.ml.datasets.len().to_string().green());
        println!();

        let opciones = &[
            "🧪  Datasets (crear / cargar / ver)",
            "🧠  Red Neuronal Artificial (ANN)",
            "📐  Máquina de Vectores de Soporte (SVM)",
            "🌳  Árbol de Decisión",
            "🌲  Bosque Aleatorio (Random Forest)",
            "🔬  Red Neuronal Profunda (DNN)",
            "🖼️   Red Convolucional (CNN)",
            "🔁  Red Recurrente (RNN / LSTM)",
            "🎮  Aprendizaje por Refuerzo (Q-Learning)",
            "📊  Ver modelos entrenados",
            "🔙  Volver",
        ];

        match menu("Selecciona un algoritmo:", opciones) {
            Some(0) => menu_ml_datasets(state),
            Some(1) => menu_ml_ann(state),
            Some(2) => menu_ml_svm(state),
            Some(3) => menu_ml_arbol(state),
            Some(4) => menu_ml_bosque(state),
            Some(5) => menu_ml_dnn(state),
            Some(6) => menu_ml_cnn(state),
            Some(7) => menu_ml_rnn(state),
            Some(8) => menu_ml_rl(state),
            Some(9) => menu_ml_ver_modelos(state),
            _ => return,
        }
    }
}

fn ml_elegir_dataset(state: &AppState) -> Option<usize> {
    if state.ml.datasets.is_empty() {
        println!("  {} No hay datasets. Crea uno primero.", "✗".red());
        pausa();
        return None;
    }
    let nombres: Vec<String> = state.ml.datasets.iter()
        .map(|d| format!("{} ({} muestras, {} features, {} clases)",
            d.nombre, d.num_muestras(), d.num_features(), d.num_clases()))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
    menu("Selecciona un dataset:", &refs)
}

fn pedir_f64(prompt: &str, default: f64) -> f64 {
    let s: String = Input::new()
        .with_prompt(format!("  {} (default: {})", prompt, default))
        .default(default.to_string())
        .interact_text()
        .unwrap_or_else(|_| default.to_string());
    s.parse().unwrap_or(default)
}

fn pedir_usize(prompt: &str, default: usize) -> usize {
    let s: String = Input::new()
        .with_prompt(format!("  {} (default: {})", prompt, default))
        .default(default.to_string())
        .interact_text()
        .unwrap_or_else(|_| default.to_string());
    s.parse().unwrap_or(default)
}

// ── Datasets ──

fn menu_ml_datasets(state: &mut AppState) {
    loop {
        limpiar();
        separador("🧪 DATASETS");

        let opciones = &[
            "📦  Generar dataset Iris sintético",
            "📦  Generar dataset XOR",
            "📦  Generar dataset Círculos",
            "✏️   Crear dataset manual",
            "📋  Ver datasets cargados",
            "🗑️   Eliminar dataset",
            "🔙  Volver",
        ];

        match menu("Datasets:", opciones) {
            Some(0) => {
                let ds = dataset_iris_sintetico(42);
                ds.resumen();
                state.ml.datasets.push(ds);
                println!("  {} Dataset Iris sintético creado", "✓".green());
                pausa();
            }
            Some(1) => {
                let ds = dataset_xor(42);
                ds.resumen();
                state.ml.datasets.push(ds);
                println!("  {} Dataset XOR creado", "✓".green());
                pausa();
            }
            Some(2) => {
                let ds = dataset_circulos(42);
                ds.resumen();
                state.ml.datasets.push(ds);
                println!("  {} Dataset Círculos creado", "✓".green());
                pausa();
            }
            Some(3) => {
                if let Some(nombre) = pedir_texto("Nombre del dataset") {
                    let num_features = pedir_usize("Número de features", 2);
                    let num_clases = pedir_usize("Número de clases", 2);
                    let num_muestras = pedir_usize("Número de muestras", 50);

                    let mut ds = Dataset::nuevo(&nombre);
                    ds.nombres_clases = (0..num_clases).map(|i| format!("Clase {}", i)).collect();
                    ds.nombres_features = (0..num_features).map(|i| format!("F{}", i)).collect();

                    let mut rng = Rng::new(42);
                    for _ in 0..num_muestras {
                        let clase = rng.usize_rango(num_clases);
                        let features: Vec<f64> = (0..num_features)
                            .map(|_| rng.normal() + clase as f64 * 2.0)
                            .collect();
                        ds.agregar_muestra(features, clase);
                    }

                    ds.resumen();
                    state.ml.datasets.push(ds);
                    println!("  {} Dataset creado con datos aleatorios por clase", "✓".green());
                }
                pausa();
            }
            Some(4) => {
                if state.ml.datasets.is_empty() {
                    println!("  No hay datasets cargados.");
                } else {
                    for (i, ds) in state.ml.datasets.iter().enumerate() {
                        println!("  {}.", (i + 1).to_string().cyan());
                        ds.resumen();
                        println!();
                    }
                }
                pausa();
            }
            Some(5) => {
                if let Some(idx) = ml_elegir_dataset(state) {
                    let nombre = state.ml.datasets[idx].nombre.clone();
                    state.ml.datasets.remove(idx);
                    println!("  {} Dataset '{}' eliminado", "✓".green(), nombre);
                    pausa();
                }
            }
            _ => return,
        }
    }
}

// ── ANN ──

fn menu_ml_ann(state: &mut AppState) {
    limpiar();
    separador("🧠 RED NEURONAL ARTIFICIAL (ANN)");

    println!("  Perceptrón multicapa con backpropagation.");
    println!();

    let Some(ds_idx) = ml_elegir_dataset(state) else { return };

    let mut ds = state.ml.datasets[ds_idx].clone();
    ds.normalizar();
    let (train, test) = ds.dividir(0.8, 42);

    let n_features = train.num_features();
    let n_clases = train.num_clases();

    println!();
    println!("  {} Train: {} muestras — Test: {} muestras", "📊".to_string(),
        train.num_muestras(), test.num_muestras());

    let hidden = pedir_usize("Neuronas capa oculta", 16);
    let epocas = pedir_usize("Épocas", 100);
    let lr = pedir_f64("Tasa de aprendizaje", 0.01);
    let batch = pedir_usize("Batch size", 16);

    let capas = vec![
        (hidden, Activacion::ReLU),
        (hidden / 2, Activacion::ReLU),
        (n_clases, Activacion::Softmax),
    ];

    println!();
    separador("Entrenando ANN...");

    let mut ann = ANN::nueva(n_features, &capas, lr, Perdida::CrossEntropy, 42);
    ann.resumen();
    println!();

    let x_train = train.a_matriz();
    let y_train = train.etiquetas_one_hot();
    ann.entrenar(&x_train, &y_train, epocas, batch);

    let x_test = test.a_matriz();
    let prec_train = ann.precision(&x_train, &train.etiquetas);
    let prec_test = ann.precision(&x_test, &test.etiquetas);

    println!();
    println!("  {} Precisión train: {:.2}%", "✓".green(), prec_train * 100.0);
    println!("  {} Precisión test:  {:.2}%", "✓".green(), prec_test * 100.0);

    let modelo = ModeloML {
        id: uuid::Uuid::new_v4().to_string(),
        nombre: format!("ANN — {}", ds.nombre),
        tipo: TipoModelo::ANN(ann),
        creado: Local::now().format("%Y-%m-%d %H:%M").to_string(),
        precision_train: Some(prec_train),
        precision_test: Some(prec_test),
    };
    state.ml.agregar_modelo(modelo);
    println!("  {} Modelo guardado", "✓".green());
    pausa();
}

// ── SVM ──

fn menu_ml_svm(state: &mut AppState) {
    limpiar();
    separador("📐 MÁQUINA DE VECTORES DE SOPORTE (SVM)");

    let Some(ds_idx) = ml_elegir_dataset(state) else { return };

    let mut ds = state.ml.datasets[ds_idx].clone();
    ds.normalizar();
    let (train, test) = ds.dividir(0.8, 42);

    let n_features = train.num_features();
    let n_clases = train.num_clases();

    let c_param = pedir_f64("Parámetro C (regularización)", 1.0);
    let lr = pedir_f64("Tasa de aprendizaje", 0.001);
    let epocas = pedir_usize("Épocas", 200);

    println!();
    separador("Entrenando SVM...");

    if n_clases <= 2 {
        let y_train: Vec<f64> = train.etiquetas.iter().map(|&e| if e == 1 { 1.0 } else { -1.0 }).collect();
        let y_test: Vec<f64> = test.etiquetas.iter().map(|&e| if e == 1 { 1.0 } else { -1.0 }).collect();

        let mut svm = SVM::nuevo(n_features, c_param, lr);
        svm.entrenar(&train.features, &y_train, epocas);

        let prec_train = svm.precision(&train.features, &y_train);
        let prec_test = svm.precision(&test.features, &y_test);

        println!();
        svm.resumen();
        println!("  {} Precisión train: {:.2}%", "✓".green(), prec_train * 100.0);
        println!("  {} Precisión test:  {:.2}%", "✓".green(), prec_test * 100.0);

        let modelo = ModeloML {
            id: uuid::Uuid::new_v4().to_string(),
            nombre: format!("SVM — {}", ds.nombre),
            tipo: TipoModelo::SVM(svm),
            creado: Local::now().format("%Y-%m-%d %H:%M").to_string(),
            precision_train: Some(prec_train),
            precision_test: Some(prec_test),
        };
        state.ml.agregar_modelo(modelo);
    } else {
        let mut svm = SVMMulticlase::nuevo(n_features, n_clases, c_param, lr);
        svm.entrenar(&train.features, &train.etiquetas, epocas);

        let prec_train = svm.precision(&train.features, &train.etiquetas);
        let prec_test = svm.precision(&test.features, &test.etiquetas);

        println!();
        println!("  {} Precisión train: {:.2}%", "✓".green(), prec_train * 100.0);
        println!("  {} Precisión test:  {:.2}%", "✓".green(), prec_test * 100.0);

        let modelo = ModeloML {
            id: uuid::Uuid::new_v4().to_string(),
            nombre: format!("SVM Multi — {}", ds.nombre),
            tipo: TipoModelo::SVMMulti(svm),
            creado: Local::now().format("%Y-%m-%d %H:%M").to_string(),
            precision_train: Some(prec_train),
            precision_test: Some(prec_test),
        };
        state.ml.agregar_modelo(modelo);
    }

    println!("  {} Modelo guardado", "✓".green());
    pausa();
}

// ── Árbol de Decisión ──

fn menu_ml_arbol(state: &mut AppState) {
    limpiar();
    separador("🌳 ÁRBOL DE DECISIÓN");

    let Some(ds_idx) = ml_elegir_dataset(state) else { return };

    let mut ds = state.ml.datasets[ds_idx].clone();
    ds.normalizar();
    let (train, test) = ds.dividir(0.8, 42);

    let max_prof = pedir_usize("Profundidad máxima", 10);
    let min_split = pedir_usize("Min muestras para split", 2);

    println!();
    separador("Entrenando Árbol de Decisión...");

    let mut arbol = ArbolDecision::nuevo(max_prof, min_split, train.num_clases());
    arbol.entrenar(&train.features, &train.etiquetas);

    let prec_train = arbol.precision(&train.features, &train.etiquetas);
    let prec_test = arbol.precision(&test.features, &test.etiquetas);

    arbol.resumen();
    println!();
    println!("  {} Precisión train: {:.2}%", "✓".green(), prec_train * 100.0);
    println!("  {} Precisión test:  {:.2}%", "✓".green(), prec_test * 100.0);

    // Importancia de features
    let imp = arbol.importancia_features();
    if !imp.is_empty() {
        println!();
        println!("  Importancia de features (num. splits):");
        for (feat, cnt) in &imp {
            let nombre = ds.nombres_features.get(*feat).map(|s| s.as_str()).unwrap_or("?");
            println!("    F{} ({}): {} splits", feat, nombre, cnt);
        }
    }

    let modelo = ModeloML {
        id: uuid::Uuid::new_v4().to_string(),
        nombre: format!("Árbol — {}", ds.nombre),
        tipo: TipoModelo::ArbolDecision(arbol),
        creado: Local::now().format("%Y-%m-%d %H:%M").to_string(),
        precision_train: Some(prec_train),
        precision_test: Some(prec_test),
    };
    state.ml.agregar_modelo(modelo);
    println!("  {} Modelo guardado", "✓".green());
    pausa();
}

// ── Bosque Aleatorio ──

fn menu_ml_bosque(state: &mut AppState) {
    limpiar();
    separador("🌲 BOSQUE ALEATORIO (RANDOM FOREST)");

    let Some(ds_idx) = ml_elegir_dataset(state) else { return };

    let mut ds = state.ml.datasets[ds_idx].clone();
    ds.normalizar();
    let (train, test) = ds.dividir(0.8, 42);

    let n_features = train.num_features();
    let num_arboles = pedir_usize("Número de árboles", 50);
    let max_prof = pedir_usize("Profundidad máxima", 10);
    let max_feat = pedir_usize(&format!("Max features por split (sqrt ~ {})", (n_features as f64).sqrt() as usize), (n_features as f64).sqrt().ceil() as usize);

    println!();
    separador("Entrenando Bosque Aleatorio...");

    let mut bosque = BosqueAleatorio::nuevo(num_arboles, max_prof, max_feat, train.num_clases());
    bosque.entrenar(&train.features, &train.etiquetas, 42);

    let prec_train = bosque.precision(&train.features, &train.etiquetas);
    let prec_test = bosque.precision(&test.features, &test.etiquetas);

    bosque.resumen();
    println!();
    println!("  {} Precisión train: {:.2}%", "✓".green(), prec_train * 100.0);
    println!("  {} Precisión test:  {:.2}%", "✓".green(), prec_test * 100.0);

    let imp = bosque.importancia_features();
    if !imp.is_empty() {
        println!();
        println!("  Top features (aggregated splits):");
        for (feat, cnt) in imp.iter().take(10) {
            let nombre = ds.nombres_features.get(*feat).map(|s| s.as_str()).unwrap_or("?");
            println!("    F{} ({}): {} splits", feat, nombre, cnt);
        }
    }

    let modelo = ModeloML {
        id: uuid::Uuid::new_v4().to_string(),
        nombre: format!("Bosque — {}", ds.nombre),
        tipo: TipoModelo::BosqueAleatorio(bosque),
        creado: Local::now().format("%Y-%m-%d %H:%M").to_string(),
        precision_train: Some(prec_train),
        precision_test: Some(prec_test),
    };
    state.ml.agregar_modelo(modelo);
    println!("  {} Modelo guardado", "✓".green());
    pausa();
}

// ── DNN ──

fn menu_ml_dnn(state: &mut AppState) {
    limpiar();
    separador("🔬 RED NEURONAL PROFUNDA (DNN)");

    println!("  Con dropout, momentum y múltiples capas ocultas.");
    println!();

    let Some(ds_idx) = ml_elegir_dataset(state) else { return };

    let mut ds = state.ml.datasets[ds_idx].clone();
    ds.normalizar();
    let (train, test) = ds.dividir(0.8, 42);

    let n_features = train.num_features();
    let n_clases = train.num_clases();

    let h1 = pedir_usize("Neuronas capa 1", 64);
    let h2 = pedir_usize("Neuronas capa 2", 32);
    let h3 = pedir_usize("Neuronas capa 3", 16);
    let dropout = pedir_f64("Dropout (0.0 - 0.5)", 0.2);
    let momentum = pedir_f64("Momentum", 0.9);
    let lr = pedir_f64("Tasa de aprendizaje", 0.005);
    let epocas = pedir_usize("Épocas", 200);
    let batch = pedir_usize("Batch size", 32);

    let capas = vec![
        (h1, Activacion::ReLU, dropout),
        (h2, Activacion::ReLU, dropout),
        (h3, Activacion::ReLU, dropout * 0.5),
        (n_clases, Activacion::Softmax, 0.0),
    ];

    println!();
    separador("Entrenando DNN...");

    let mut dnn = DNN::nueva(n_features, &capas, lr, momentum, Perdida::CrossEntropy, 42);
    dnn.resumen();
    println!();

    let x_train = train.a_matriz();
    let y_train = train.etiquetas_one_hot();
    let x_test = test.a_matriz();

    dnn.entrenar(&x_train, &y_train, epocas, batch);

    let prec_train = dnn.precision(&x_train, &train.etiquetas);
    let prec_test = dnn.precision(&x_test, &test.etiquetas);

    println!();
    println!("  {} Precisión train: {:.2}%", "✓".green(), prec_train * 100.0);
    println!("  {} Precisión test:  {:.2}%", "✓".green(), prec_test * 100.0);

    let modelo = ModeloML {
        id: uuid::Uuid::new_v4().to_string(),
        nombre: format!("DNN — {}", ds.nombre),
        tipo: TipoModelo::DNN(dnn),
        creado: Local::now().format("%Y-%m-%d %H:%M").to_string(),
        precision_train: Some(prec_train),
        precision_test: Some(prec_test),
    };
    state.ml.agregar_modelo(modelo);
    println!("  {} Modelo guardado", "✓".green());
    pausa();
}

// ── CNN ──

fn menu_ml_cnn(state: &mut AppState) {
    limpiar();
    separador("🖼️ RED NEURONAL CONVOLUCIONAL (CNN)");

    println!("  CNN 1D: Conv → MaxPool → Dense.");
    println!();

    let Some(ds_idx) = ml_elegir_dataset(state) else { return };

    let mut ds = state.ml.datasets[ds_idx].clone();
    ds.normalizar();
    let (train, test) = ds.dividir(0.8, 42);

    let n_features = train.num_features();
    let n_clases = train.num_clases();

    if n_features < 4 {
        println!("  {} La CNN necesita al menos 4 features para la convolución.", "⚠".yellow());
        println!("    Dataset actual tiene {} features.", n_features);
        pausa();
        return;
    }

    let num_filtros = pedir_usize("Número de filtros", 8);
    let kernel = pedir_usize("Tamaño del kernel", 3);
    let pool = pedir_usize("Tamaño del pool", 2);
    let hidden = pedir_usize("Neuronas capa densa", 16);
    let lr = pedir_f64("Tasa de aprendizaje", 0.01);
    let epocas = pedir_usize("Épocas", 50);

    let capas_densas = vec![(hidden, Activacion::ReLU)];

    println!();
    separador("Entrenando CNN...");

    let mut cnn = CNN::nueva_1d(n_features, num_filtros, kernel, pool, &capas_densas, lr, n_clases, 42);
    cnn.resumen();
    println!();

    cnn.entrenar(&train.features, &train.etiquetas, epocas);

    let prec_train = cnn.precision(&train.features, &train.etiquetas);
    let prec_test = cnn.precision(&test.features, &test.etiquetas);

    println!();
    println!("  {} Precisión train: {:.2}%", "✓".green(), prec_train * 100.0);
    println!("  {} Precisión test:  {:.2}%", "✓".green(), prec_test * 100.0);

    let modelo = ModeloML {
        id: uuid::Uuid::new_v4().to_string(),
        nombre: format!("CNN — {}", ds.nombre),
        tipo: TipoModelo::CNN(cnn),
        creado: Local::now().format("%Y-%m-%d %H:%M").to_string(),
        precision_train: Some(prec_train),
        precision_test: Some(prec_test),
    };
    state.ml.agregar_modelo(modelo);
    println!("  {} Modelo guardado", "✓".green());
    pausa();
}

// ── RNN / LSTM ──

fn menu_ml_rnn(state: &mut AppState) {
    limpiar();
    separador("🔁 RED NEURONAL RECURRENTE (RNN / LSTM)");

    println!("  Procesa secuencias temporales.");
    println!();

    let tipo_opciones = &["RNN Simple (Elman)", "LSTM"];
    let tipo_idx = menu("Tipo de RNN:", tipo_opciones).unwrap_or(0);
    let tipo = if tipo_idx == 1 { TipoRNN::LSTM } else { TipoRNN::Simple };

    let hidden = pedir_usize("Tamaño capa oculta", 16);
    let lr = pedir_f64("Tasa de aprendizaje", 0.005);
    let epocas = pedir_usize("Épocas", 100);

    println!();
    println!("  Generando dataset de secuencias temporales...");
    let (secuencias, objetivos) = dataset_secuencia_temporal(42);

    let n_train = (secuencias.len() as f64 * 0.8) as usize;
    let seq_train = &secuencias[..n_train];
    let obj_train = &objetivos[..n_train];
    let seq_test = &secuencias[n_train..];
    let obj_test = &objetivos[n_train..];

    println!("  Train: {} secuencias — Test: {} secuencias", seq_train.len(), seq_test.len());
    println!();
    separador("Entrenando RNN...");

    let mut rnn = RNN::nueva(tipo, 1, hidden, 1, lr, 42);
    rnn.resumen();
    println!();

    rnn.entrenar(seq_train, obj_train, epocas);

    // Evaluar
    let mut error_test = 0.0;
    for (seq, obj) in seq_test.iter().zip(obj_test) {
        let pred = rnn.predecir(seq);
        let err: f64 = pred.iter().zip(obj).map(|(p, t)| (p - t).powi(2)).sum::<f64>();
        error_test += err;
    }
    let mse_test = error_test / seq_test.len() as f64;

    println!();
    println!("  {} MSE test: {:.6}", "✓".green(), mse_test);

    // Mostrar algunas predicciones
    println!();
    println!("  Ejemplo de predicciones:");
    for i in 0..5.min(seq_test.len()) {
        let pred = rnn.predecir(&seq_test[i]);
        println!("    Objetivo: {:.3} → Predicción: {:.3}",
            obj_test[i][0], pred[0]);
    }

    let modelo = ModeloML {
        id: uuid::Uuid::new_v4().to_string(),
        nombre: format!("RNN — secuencias temporales"),
        tipo: TipoModelo::RNN(rnn),
        creado: Local::now().format("%Y-%m-%d %H:%M").to_string(),
        precision_train: None,
        precision_test: None,
    };
    state.ml.agregar_modelo(modelo);
    println!("  {} Modelo guardado", "✓".green());
    pausa();
}

// ── Aprendizaje por Refuerzo ──

fn menu_ml_rl(state: &mut AppState) {
    loop {
        limpiar();
        separador("🎮 APRENDIZAJE POR REFUERZO");

        let opciones = &[
            "🗺️   Q-Learning en GridWorld",
            "🎰  Multi-Armed Bandit",
            "🔙  Volver",
        ];

        match menu("Selecciona entorno:", opciones) {
            Some(0) => menu_ml_rl_gridworld(state),
            Some(1) => menu_ml_rl_bandit(state),
            _ => return,
        }
    }
}

fn menu_ml_rl_gridworld(state: &mut AppState) {
    limpiar();
    separador("🗺️ Q-LEARNING — GRIDWORLD");

    let filas = pedir_usize("Filas del grid", 5);
    let cols = pedir_usize("Columnas del grid", 5);
    let meta = (filas - 1, cols - 1);

    println!("  Meta: ({}, {})", meta.0, meta.1);
    let num_obs = pedir_usize("Número de obstáculos aleatorios", 3);

    let mut rng = Rng::new(42);
    let mut obstaculos = Vec::new();
    for _ in 0..num_obs {
        loop {
            let pos = (rng.usize_rango(filas), rng.usize_rango(cols));
            if pos != (0, 0) && pos != meta && !obstaculos.contains(&pos) {
                obstaculos.push(pos);
                break;
            }
        }
    }
    println!("  Obstáculos: {:?}", obstaculos);

    let alpha = pedir_f64("Alpha (aprendizaje)", 0.1);
    let gamma = pedir_f64("Gamma (descuento)", 0.99);
    let epsilon = pedir_f64("Epsilon inicial", 1.0);
    let episodios = pedir_usize("Episodios", 5000);

    let mut grid = GridWorld::nuevo(filas, cols, meta).con_obstaculos(obstaculos);
    let mut q = QTable::nueva(4, alpha, gamma, epsilon); // 4 acciones: ↑↓←→

    println!();
    separador("Entrenando agente Q-Learning...");

    grid.entrenar_agente(&mut q, episodios, filas * cols * 2);

    q.resumen();
    println!();
    println!("  Política aprendida:");
    grid.mostrar_politica(&q);

    let modelo = ModeloML {
        id: uuid::Uuid::new_v4().to_string(),
        nombre: format!("Q-Learning — Grid {}x{}", filas, cols),
        tipo: TipoModelo::QLearning(q),
        creado: Local::now().format("%Y-%m-%d %H:%M").to_string(),
        precision_train: None,
        precision_test: None,
    };
    state.ml.agregar_modelo(modelo);
    println!("  {} Modelo guardado", "✓".green());
    pausa();
}

fn menu_ml_rl_bandit(_state: &mut AppState) {
    limpiar();
    separador("🎰 MULTI-ARMED BANDIT");

    let n_brazos = pedir_usize("Número de brazos", 5);
    let episodios = pedir_usize("Episodios", 10000);
    let epsilon = pedir_f64("Epsilon (exploración)", 0.1);

    let mut rng = Rng::new(42);
    let probs: Vec<f64> = (0..n_brazos).map(|_| rng.rango(0.1, 0.9)).collect();
    println!();
    println!("  Probabilidades reales (ocultas): {:?}", probs.iter().map(|p| format!("{:.2}", p)).collect::<Vec<_>>());

    let mut bandit = MultiBandit::nuevo(probs.clone());

    println!();
    separador("Entrenando ε-greedy...");

    let historial = bandit.entrenar_epsilon_greedy(episodios, epsilon);

    println!();
    println!("  Resultados tras {} episodios:", episodios);
    for i in 0..n_brazos {
        let ratio = if bandit.conteos[i] > 0 {
            bandit.recompensas_acumuladas[i] / bandit.conteos[i] as f64
        } else { 0.0 };
        println!("    Brazo {}: prob real={:.2}, tiradas={}, ratio ganancia={:.3}",
            i, probs[i], bandit.conteos[i], ratio);
    }

    let mejor_real = bandit.mejor_brazo();
    let mas_tirado = bandit.conteos.iter().enumerate().max_by_key(|(_, &c)| c).map(|(i, _)| i).unwrap_or(0);

    println!();
    println!("  {} Mejor brazo real: {} — Más explotado: {}",
        if mejor_real == mas_tirado { "✓".green() } else { "⚠".yellow() },
        mejor_real, mas_tirado);

    if let Some(ultimo) = historial.last() {
        println!("  Recompensa promedio final: {:.4}", ultimo);
    }

    pausa();
}

// ── Ver modelos ──

fn menu_ml_ver_modelos(state: &mut AppState) {
    limpiar();
    separador("📊 MODELOS ENTRENADOS");

    if state.ml.modelos.is_empty() {
        println!("  No hay modelos entrenados aún.");
        pausa();
        return;
    }

    for (i, m) in state.ml.modelos.iter().enumerate() {
        let tipo_str = match &m.tipo {
            TipoModelo::ANN(_) => "ANN",
            TipoModelo::SVM(_) => "SVM",
            TipoModelo::SVMMulti(_) => "SVM Multi",
            TipoModelo::ArbolDecision(_) => "Árbol de Decisión",
            TipoModelo::BosqueAleatorio(_) => "Bosque Aleatorio",
            TipoModelo::DNN(_) => "DNN",
            TipoModelo::CNN(_) => "CNN",
            TipoModelo::RNN(_) => "RNN",
            TipoModelo::QLearning(_) => "Q-Learning",
        };

        println!("  {}. {} [{}]", (i + 1).to_string().cyan(), m.nombre, tipo_str.yellow());
        println!("     Creado: {}", m.creado);
        if let Some(pt) = m.precision_train {
            println!("     Precisión train: {:.2}%", pt * 100.0);
        }
        if let Some(pe) = m.precision_test {
            println!("     Precisión test:  {:.2}%", pe * 100.0);
        }
        println!();
    }

    let opciones = &["🔍  Ver detalles de un modelo", "🗑️   Eliminar un modelo", "🔙  Volver"];
    match menu("Acciones:", opciones) {
        Some(0) => {
            let nombres: Vec<String> = state.ml.modelos.iter().map(|m| m.nombre.clone()).collect();
            let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
            if let Some(idx) = menu("Selecciona modelo:", &refs) {
                println!();
                match &state.ml.modelos[idx].tipo {
                    TipoModelo::ANN(m) => m.resumen(),
                    TipoModelo::SVM(m) => m.resumen(),
                    TipoModelo::SVMMulti(_) => println!("  SVM Multiclase"),
                    TipoModelo::ArbolDecision(m) => m.resumen(),
                    TipoModelo::BosqueAleatorio(m) => m.resumen(),
                    TipoModelo::DNN(m) => m.resumen(),
                    TipoModelo::CNN(m) => m.resumen(),
                    TipoModelo::RNN(m) => m.resumen(),
                    TipoModelo::QLearning(m) => m.resumen(),
                }
                pausa();
            }
        }
        Some(1) => {
            let nombres: Vec<String> = state.ml.modelos.iter().map(|m| m.nombre.clone()).collect();
            let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
            if let Some(idx) = menu("Eliminar modelo:", &refs) {
                let nombre = state.ml.modelos[idx].nombre.clone();
                state.ml.modelos.remove(idx);
                println!("  {} Modelo '{}' eliminado", "✓".green(), nombre);
                pausa();
            }
        }
        _ => {}
    }
}

fn main() {
    let mut state = match AppState::cargar() {
        Ok(s) => s,
        Err(_) => AppState::new(),
    };

    loop {
        limpiar();
        banner();
        dashboard(&state);

        let opciones = &[
            "📋  Tareas",
            "📅  Agenda y Horarios",
            "✏️   Canvas (Escritura a mano)",
            "📊  Diagramas de Flujo",
            "💾  Versiones (Source Control)",
            "🔄  Mapeo y Codificación",
            "🧠  Memoria (Buscar y conectar todo)",
            "🔗  Sincronización (Calendario y Email)",
            "📄  Reportes (Diario / Semanal)",
            "🤖  Inteligencia Artificial (ML)",
            "❌  Salir",
        ];

        match menu("¿Qué módulo quieres usar?", opciones) {
            Some(0) => menu_tareas(&mut state),
            Some(1) => menu_agenda(&mut state),
            Some(2) => menu_canvas(&mut state),
            Some(3) => menu_diagramas(&mut state),
            Some(4) => menu_versiones(&mut state),
            Some(5) => menu_mapeo(&mut state),
            Some(6) => menu_memoria(&mut state),
            Some(7) => menu_sync(&mut state),
            Some(8) => menu_reportes(&mut state),
            Some(9) => menu_ml(&mut state),
            _ => {
                // Guardar antes de salir
                if let Err(e) = state.guardar() {
                    eprintln!("  {} Error guardando: {}", "✗".red(), e);
                } else {
                    println!("  {} Datos guardados. ¡Hasta pronto!", "✓".green());
                }
                return;
            }
        }

        // Auto-guardar después de cada acción
        if let Err(e) = state.guardar() {
            eprintln!("  {} Error guardando: {}", "✗".red(), e);
        }
    }
}
