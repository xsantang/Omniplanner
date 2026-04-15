#![allow(
    clippy::while_let_loop,
    clippy::format_in_format_args,
    clippy::needless_range_loop,
    clippy::single_element_loop
)]

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, Weekday};
use colored::Colorize;
use dialoguer::{Confirm, Input, Select};

use omniplanner::agenda::{
    DiaMarcado, Evento, Frecuencia, HorarioEscritura, TipoDiaMarcado, TipoEvento,
};
use omniplanner::canvas::{Canvas, Elemento};
use omniplanner::diagrams::{Diagrama, Nodo, TipoConexion, TipoDiagrama, TipoNodo};
use omniplanner::mapper::{Codificacion, EsquemaMapa, Mapper};
use omniplanner::memoria::Recuerdo;
use omniplanner::ml::presupuesto_cero::{
    self, Categoria, LineaPresupuesto, PlantillaPresupuesto, PresupuestoMensual, SaludPresupuesto,
};
use omniplanner::ml::{
    dataset_circulos, dataset_iris_sintetico, dataset_secuencia_temporal, dataset_xor, Activacion,
    AnalisisDeuda, ArbolDecision, BosqueAleatorio, ComparacionRapida, CorteBancario, Dataset,
    DeudaRastreada, FrecuenciaPago, GridWorld, ImpactoAccion, IngresoRastreado, MatrizDecision,
    MetaAhorro, ModeloML, Movimiento, MultiBandit, Perdida, QTable, RegistroAsesor, Rng,
    SVMMulticlase, SimulacionLibertad, TipoModelo, TipoRNN, TipoRegistro, ANN, CNN, DNN, RNN, SVM,
};
use omniplanner::nlp::{DatosEntrenamiento, TipoRelacion, Valoracion};
use omniplanner::storage::AppState;
use omniplanner::sync;
use omniplanner::tasks::{Prioridad, Task, TaskStatus};
use rust_xlsxwriter::{Format, FormatAlign, FormatBorder, Workbook};

// ══════════════════════════════════════════════════════════════
//  Helpers de UI
// ══════════════════════════════════════════════════════════════

fn limpiar() {
    print!("\x1B[2J\x1B[H");
}

fn banner() {
    println!(
        "{}",
        "╔══════════════════════════════════════════════╗".cyan()
    );
    println!(
        "{}",
        "║         ✦  O M N I P L A N N E R  ✦         ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║   Tu asistente todo-en-uno de productividad  ║".cyan()
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════╝".cyan()
    );
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
    if s.trim().is_empty() {
        None
    } else {
        Some(s)
    }
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
        1 => "enero",
        2 => "febrero",
        3 => "marzo",
        4 => "abril",
        5 => "mayo",
        6 => "junio",
        7 => "julio",
        8 => "agosto",
        9 => "septiembre",
        10 => "octubre",
        11 => "noviembre",
        12 => "diciembre",
        _ => "",
    };
    let anio = f.year();
    let dow = match f.weekday() {
        Weekday::Mon => "lunes",
        Weekday::Tue => "martes",
        Weekday::Wed => "miércoles",
        Weekday::Thu => "jueves",
        Weekday::Fri => "viernes",
        Weekday::Sat => "sábado",
        Weekday::Sun => "domingo",
    };
    format!("{} {} de {} de {}", dow, dia, mes, anio)
}

fn pedir_fecha(prompt: &str) -> Option<NaiveDate> {
    println!("    💡 Formatos: hoy, mañana, 28/03/2026, 28-03-2026, 28032026,");
    println!("                28 de marzo de 2026, march 28 2026, 2026-03-28");
    loop {
        let s = pedir_texto_opcional(&format!("{} (vacío=cancelar)", prompt));
        if s.is_empty() {
            return None;
        }
        let candidatos = parsear_fecha_candidatos(&s);
        match candidatos.len() {
            0 => {
                println!(
                    "    {} No pude entender esa fecha. Intenta otro formato.",
                    "✗".red()
                );
            }
            1 => {
                let f = candidatos[0];
                println!("    {} Fecha: {}", "✓".green(), formatear_fecha_es(f));
                return Some(f);
            }
            _ => {
                println!(
                    "\n    {} Fecha ambigua — ¿cuál quisiste decir?\n",
                    "⚠".yellow()
                );
                let opciones: Vec<String> = candidatos
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        let letra = (b'A' + i as u8) as char;
                        format!(
                            "  {} → {} ({})",
                            letra,
                            formatear_fecha_es(*f),
                            f.format("%d/%m/%Y")
                        )
                    })
                    .collect();
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
        if let Some(f) = parse_ddmmyyyy(&digits) {
            candidatos.push(f);
        }
        // Interpretación mm/dd/yyyy
        if let Some(f) = parse_mmddyyyy(&digits) {
            if !candidatos.contains(&f) {
                candidatos.push(f);
            }
        }
        // Interpretación yyyy/mm/dd
        if let Ok(f) = NaiveDate::parse_from_str(&digits, "%Y%m%d") {
            if !candidatos.contains(&f) {
                candidatos.push(f);
            }
        }

        return candidatos;
    }

    if digits.len() == 6 {
        let mut candidatos: Vec<NaiveDate> = Vec::new();

        if let Some(f) = parse_ddmmyy(&digits) {
            candidatos.push(f);
        }
        if let Some(f) = parse_mmddyy(&digits) {
            if !candidatos.contains(&f) {
                candidatos.push(f);
            }
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
            if s.starts_with("ene") {
                return Some(1);
            }
            if s.starts_with("feb") {
                return Some(2);
            }
            if s.starts_with("mar") && !s.starts_with("may") {
                return Some(3);
            }
            if s.starts_with("abr") || s.starts_with("apr") {
                return Some(4);
            }
            if s.starts_with("may") {
                return Some(5);
            }
            if s.starts_with("jun") {
                return Some(6);
            }
            if s.starts_with("jul") {
                return Some(7);
            }
            if s.starts_with("ago") || s.starts_with("aug") {
                return Some(8);
            }
            if s.starts_with("sep") || s.starts_with("set") {
                return Some(9);
            }
            if s.starts_with("oct") {
                return Some(10);
            }
            if s.starts_with("nov") {
                return Some(11);
            }
            if s.starts_with("dic") || s.starts_with("dec") {
                return Some(12);
            }
            None
        }
    }
}

fn parsear_fecha_texto_es(s: &str) -> Option<NaiveDate> {
    // "28 de marzo de 2026", "28 marzo 2026", "28 del marzo 2026"
    // Filtrar palabras "de" y "del" en vez de replace (más seguro)
    let partes: Vec<&str> = s
        .split_whitespace()
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
    let limpio: String = s.replace([',', '/'], " ");
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
    println!("    💡 Formatos: 14:30, 2:30pm, 6pm, 1430, 6 (=06:00)");
    loop {
        let s = pedir_texto_opcional(&format!("{} (vacío=cancelar)", prompt));
        if s.is_empty() {
            return None;
        }
        match parsear_hora(&s) {
            Some(h) => {
                println!("    {} Hora: {}", "✓".green(), h.format("%H:%M"));
                return Some(h);
            }
            None => {
                println!(
                    "    {} No pude entender esa hora. Intenta otro formato.",
                    "✗".red()
                );
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
        .replace("pm", "")
        .replace("am", "")
        .replace("p.m.", "")
        .replace("a.m.", "")
        .replace("p.m", "")
        .replace("a.m", "")
        .replace("p m", "")
        .replace("a m", "")
        .trim()
        .to_string();

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
        "  📅 {} ({:?}) - {}",
        hoy.format("%d/%m/%Y"),
        dia,
        ahora.format("%H:%M")
    );
    println!();

    // Tareas de hoy
    let tareas_hoy = state.tasks.listar_por_fecha(hoy);
    let pendientes = state.tasks.listar_pendientes();
    if !tareas_hoy.is_empty() || !pendientes.is_empty() {
        println!(
            "  {} {}",
            "📋 Tareas:".yellow().bold(),
            format!(
                "({} hoy, {} pendientes)",
                tareas_hoy.len(),
                pendientes.len()
            )
            .white()
        );
        for t in &tareas_hoy {
            let icono = match t.estado {
                TaskStatus::Completada => "  ✅",
                TaskStatus::EnProgreso => "  🔄",
                TaskStatus::Cancelada => "  ❌",
                TaskStatus::Pendiente => "  ⬜",
            };
            println!(
                "    {} {} - {} {}",
                icono,
                t.hora.format("%H:%M"),
                t.titulo,
                format!("[{}]", t.prioridad).dimmed()
            );
        }
    }

    // Eventos de hoy
    let eventos_hoy = state.agenda.eventos_del_dia(hoy);
    if !eventos_hoy.is_empty() {
        println!(
            "  {} {}",
            "📅 Eventos:".green().bold(),
            format!("({} hoy)", eventos_hoy.len()).white()
        );
        for e in &eventos_hoy {
            let fin = e
                .hora_fin
                .map(|h| format!("-{}", h.format("%H:%M")))
                .unwrap_or_default();
            let icono = match e.tipo {
                TipoEvento::Cumpleanos => "🎂",
                TipoEvento::Pago => "💰",
                _ => "📌",
            };
            let concepto_txt = if e.concepto.is_empty() {
                String::new()
            } else {
                format!(" [{}]", e.concepto)
            };
            println!(
                "    {} {}{} {} ({}){}",
                icono,
                e.hora_inicio.format("%H:%M"),
                fin,
                e.titulo,
                e.tipo,
                concepto_txt.dimmed()
            );
            println!(
                "       📆 {} {}  🕐 {} {}",
                "Evento:".dimmed(),
                e.fecha.format("%d/%m/%Y").to_string().cyan(),
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
            println!(
                "    🖊️  {}-{} {}",
                h.hora_inicio.format("%H:%M"),
                h.hora_fin.format("%H:%M"),
                h.descripcion
            );
        }
    }

    // Follow-ups de hoy
    let follow_ups: Vec<_> = state
        .tasks
        .listar_follow_ups()
        .into_iter()
        .filter(|t| t.follow_up.map(|f| f.date() == hoy).unwrap_or(false))
        .collect();
    if !follow_ups.is_empty() {
        println!("  {}", "🔔 Follow-ups:".red().bold());
        for t in &follow_ups {
            println!(
                "    ↻ {} ({})",
                t.titulo,
                t.follow_up.unwrap().time().format("%H:%M")
            );
        }
    }

    // Resumen rápido
    println!();
    println!(
        "  📋 {} tareas  📅 {} eventos  📊 {} diagramas  ✏️ {} canvas  🧠 {} recuerdos",
        state.tasks.tareas.len(),
        state.agenda.eventos.len(),
        state.diagramas.len(),
        state.canvases.len(),
        state.memoria.recuerdos.len(),
    );

    if tareas_hoy.is_empty()
        && eventos_hoy.is_empty()
        && horarios.is_empty()
        && follow_ups.is_empty()
    {
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
                let fu = t
                    .follow_up
                    .map(|f| format!(" 🔔{}", f.format("%d/%m %H:%M")))
                    .unwrap_or_default();
                println!(
                    "  {} {} | {} {} | {} | {}{}",
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
    let titulo = match pedir_texto("Título") {
        Some(t) => t,
        None => return,
    };
    let desc = pedir_texto_opcional("Descripción (opcional)");
    let fecha = match pedir_fecha("Fecha") {
        Some(f) => f,
        None => return,
    };
    let hora = match pedir_hora("Hora") {
        Some(h) => h,
        None => return,
    };

    let prioridades = &["Baja", "Media", "Alta", "⚠ Urgente"];
    let pi = match menu("Prioridad", prioridades) {
        Some(i) => i,
        None => return,
    };
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
        let palabras: Vec<String> = tags
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        for p in &palabras {
            tarea.agregar_etiqueta(p.clone());
        }
        let recuerdo =
            Recuerdo::new(format!("Tarea: {}", titulo), palabras).con_origen("tarea", &tarea.id);
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

    let nombres: Vec<String> = state
        .tasks
        .tareas
        .iter()
        .map(|t| {
            format!(
                "{} - {} [{}] | {} {}",
                t.id,
                t.titulo,
                t.estado,
                t.fecha.format("%d/%m/%Y"),
                t.hora.format("%H:%M")
            )
        })
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    let idx = match menu("Selecciona la tarea", &refs) {
        Some(i) => i,
        None => return,
    };

    loop {
        let t = &state.tasks.tareas[idx];
        let fu_str = t
            .follow_up
            .map(|f| format!(" | 🔔 {}", f.format("%d/%m/%Y %H:%M")))
            .unwrap_or_default();
        let tags_str = if t.etiquetas.is_empty() {
            String::new()
        } else {
            format!(" | 🏷️  {}", t.etiquetas.join(", "))
        };

        println!();
        println!("  {} {}", "Editando:".bold(), t.titulo.bold());
        println!(
            "  📆 {} {} | {} | {}{}{}",
            t.fecha.format("%d/%m/%Y"),
            t.hora.format("%H:%M"),
            t.estado,
            t.prioridad,
            fu_str,
            tags_str
        );
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
                    println!(
                        "  {} Fecha actualizada: {}",
                        "✓".green().bold(),
                        fecha.format("%d/%m/%Y")
                    );
                }
            }
            Some(2) => {
                if let Some(hora) = pedir_hora("Nueva hora") {
                    state.tasks.tareas[idx].hora = hora;
                    state.tasks.tareas[idx].actualizado = chrono::Local::now().naive_local();
                    println!(
                        "  {} Hora actualizada: {}",
                        "✓".green().bold(),
                        hora.format("%H:%M")
                    );
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

    let nombres: Vec<String> = state
        .tasks
        .tareas
        .iter()
        .map(|t| format!("{} - {}", t.id, t.titulo))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    let idx = match menu("¿A cuál tarea?", &refs) {
        Some(i) => i,
        None => return,
    };
    let fecha = match pedir_fecha("Fecha del follow-up") {
        Some(f) => f,
        None => return,
    };
    let hora = match pedir_hora("Hora del follow-up") {
        Some(h) => h,
        None => return,
    };
    let fh = NaiveDateTime::new(fecha, hora);

    state.tasks.tareas[idx].programar_follow_up(fh);
    println!("  🔔 Follow-up programado: {}", fh.format("%d/%m/%Y %H:%M"));
    pausa();
}

fn recordar_tarea(state: &mut AppState) {
    if state.tasks.tareas.is_empty() {
        println!("  {}", "No hay tareas.".yellow());
        pausa();
        return;
    }

    let nombres: Vec<String> = state
        .tasks
        .tareas
        .iter()
        .map(|t| format!("{} - {}", t.id, t.titulo))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    let idx = match menu("¿Cuál tarea?", &refs) {
        Some(i) => i,
        None => return,
    };
    let palabras = match pedir_texto("Palabras clave para recordar (separadas por coma)") {
        Some(t) => t,
        None => return,
    };
    let tags: Vec<String> = palabras
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let tarea = &mut state.tasks.tareas[idx];
    for t in &tags {
        tarea.agregar_etiqueta(t.clone());
    }

    let recuerdo =
        Recuerdo::new(format!("Tarea: {}", tarea.titulo), tags).con_origen("tarea", &tarea.id);
    state.memoria.agregar_recuerdo(recuerdo);

    println!("  🧠 Palabras clave guardadas en la memoria");
    pausa();
}

fn eliminar_tarea(state: &mut AppState) {
    if state.tasks.tareas.is_empty() {
        println!("  {}", "No hay tareas.".yellow());
        pausa();
        return;
    }

    let nombres: Vec<String> = state
        .tasks
        .tareas
        .iter()
        .map(|t| format!("{} - {}", t.id, t.titulo))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    let idx = match menu("¿Cuál eliminar?", &refs) {
        Some(i) => i,
        None => return,
    };
    let nombre = state.tasks.tareas[idx].titulo.clone();

    if Confirm::new()
        .with_prompt(format!("  ¿Eliminar '{}'?", nombre))
        .default(false)
        .interact()
        .unwrap_or(false)
    {
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
    println!(
        "  {} {}",
        "🎉 ¡Tarea completada!".green().bold(),
        titulo.bold()
    );
    println!();

    // Mostrar sugerencias del diccionario basadas en etiquetas existentes
    if !etiquetas_existentes.is_empty() {
        let sugerencias = state.memoria.diccionario.sugerir(&etiquetas_existentes);
        if !sugerencias.is_empty() {
            println!("  💡 Ideas relacionadas en tu diccionario:");
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
            println!(
                "  📚 Tu diccionario tiene: {}",
                sorted
                    .iter()
                    .map(|s| s.cyan().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            println!();
        }

        let input = match pedir_texto("Palabras clave (separadas por coma)") {
            Some(t) => t,
            None => {
                // Aún sin palabras, registrar la tarea con sus etiquetas
                if !etiquetas_existentes.is_empty() {
                    state.memoria.diccionario.registrar(
                        "tarea",
                        &tarea_id,
                        &titulo,
                        &etiquetas_existentes,
                        "completada",
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
            if !palabras
                .iter()
                .any(|p| p.to_lowercase() == et.to_lowercase())
            {
                palabras.push(et.clone());
            }
        }

        // Pedir nota de contexto opcional
        let nota = pedir_texto_opcional("Nota rápida sobre lo aprendido o logrado (opcional)");

        // Registrar en diccionario neuronal
        state
            .memoria
            .diccionario
            .registrar("tarea", &tarea_id, &titulo, &palabras, &nota);

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
        let recuerdo =
            Recuerdo::new(contenido_recuerdo, palabras.clone()).con_origen("tarea", &tarea_id);
        state.memoria.agregar_recuerdo(recuerdo);

        // Mostrar conexiones resultantes
        println!();
        println!(
            "  {} Guardado en diccionario neuronal:",
            "🧠".to_string().green().bold()
        );
        println!(
            "    🏷️  {}",
            palabras
                .iter()
                .map(|p| p.cyan().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );

        let sugerencias = state.memoria.diccionario.sugerir(&palabras);
        if !sugerencias.is_empty() {
            println!();
            println!("  🔮 Posibles conexiones para explorar:");
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
                    let relacion =
                        pedir_texto_opcional("Relación (ej: 'derivó en', 'necesita', 'inspiró')");
                    let rel = if relacion.is_empty() {
                        "completada → conectada".to_string()
                    } else {
                        relacion
                    };
                    state
                        .memoria
                        .enlazar("tarea", &tarea_id, &mod_dest, &id_dest, &rel);
                    println!("  {} Enlace creado", "🔗".to_string().green());
                }
            }
        }
    } else {
        // No quiere conectar, pero si tiene etiquetas, registrar silenciosamente
        if !etiquetas_existentes.is_empty() {
            state.memoria.diccionario.registrar(
                "tarea",
                &tarea_id,
                &titulo,
                &etiquetas_existentes,
                "completada",
            );
        }
    }

    println!();
    println!(
        "  {} Tarea finalizada y memorizada exitosamente",
        "✅".to_string().green().bold()
    );
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
                let fin = e
                    .hora_fin
                    .map(|h| format!("-{}", h.format("%H:%M")))
                    .unwrap_or_default();
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
                println!(
                    "  {} {} | {}{} | {} ({}){}{}",
                    icono,
                    e.id.dimmed(),
                    e.hora_inicio.format("%H:%M"),
                    fin,
                    e.titulo,
                    e.tipo,
                    recur,
                    concepto,
                );
                println!(
                    "      📆 {} {}  🕐 {} {}",
                    "Evento:".dimmed(),
                    e.fecha.format("%d/%m/%Y").to_string().cyan(),
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
                println!(
                    "    🖊️  {:?} {}-{} {}",
                    h.dia,
                    h.hora_inicio.format("%H:%M"),
                    h.hora_fin.format("%H:%M"),
                    h.descripcion
                );
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
    let titulo = match pedir_texto("Título") {
        Some(t) => t,
        None => return,
    };
    let desc = pedir_texto_opcional("Descripción (opcional)");

    let tipos = &[
        "Reunión",
        "Recordatorio",
        "Follow-Up",
        "Cita",
        "🎂 Cumpleaños",
        "💰 Pago",
        "Otro",
    ];
    let ti = match menu("Tipo de evento", tipos) {
        Some(i) => i,
        None => return,
    };
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

    let fecha = match pedir_fecha(if es_cumple {
        "Fecha de nacimiento"
    } else {
        "Fecha"
    }) {
        Some(f) => f,
        None => return,
    };

    let hora = if es_cumple {
        NaiveTime::from_hms_opt(0, 0, 0).unwrap()
    } else {
        match pedir_hora("Hora inicio") {
            Some(h) => h,
            None => return,
        }
    };

    let hora_fin = if es_cumple {
        None
    } else {
        let tiene_fin = Confirm::new()
            .with_prompt("  ¿Tiene hora de fin?")
            .default(true)
            .interact()
            .unwrap_or(false);
        if tiene_fin {
            pedir_hora("Hora fin")
        } else {
            None
        }
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
        let fi = match menu("¿Con qué frecuencia se repite?", frecuencias) {
            Some(i) => i,
            None => return,
        };
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
        if persona.is_empty() {
            titulo.clone()
        } else {
            persona
        }
    } else {
        pedir_texto_opcional("Concepto / razón del evento (opcional)")
    };

    let tags = pedir_texto_opcional("Palabras clave (opcional, separadas por coma)");

    let mut evento = Evento::new(titulo.clone(), desc, tipo, fecha, hora, hora_fin);
    evento = evento.con_frecuencia(frecuencia).con_concepto(concepto);

    if !tags.is_empty() {
        let palabras: Vec<String> = tags
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let recuerdo =
            Recuerdo::new(format!("Evento: {}", titulo), palabras).con_origen("evento", &evento.id);
        state.memoria.agregar_recuerdo(recuerdo);
    }

    println!("\n  {} {}", "✓ Evento creado:".green().bold(), evento);
    state.agenda.agregar_evento(evento);
    pausa();
}

fn nuevo_horario(state: &mut AppState) {
    separador("✏️  Nuevo horario de escritura");
    let dias = &[
        "Lunes",
        "Martes",
        "Miércoles",
        "Jueves",
        "Viernes",
        "Sábado",
        "Domingo",
    ];
    let di = match menu("Día de la semana", dias) {
        Some(i) => i,
        None => return,
    };
    let dia = match di {
        0 => chrono::Weekday::Mon,
        1 => chrono::Weekday::Tue,
        2 => chrono::Weekday::Wed,
        3 => chrono::Weekday::Thu,
        4 => chrono::Weekday::Fri,
        5 => chrono::Weekday::Sat,
        _ => chrono::Weekday::Sun,
    };

    let inicio = match pedir_hora("Hora inicio") {
        Some(h) => h,
        None => return,
    };
    let fin = match pedir_hora("Hora fin") {
        Some(h) => h,
        None => return,
    };
    let desc = pedir_texto_opcional("Descripción");
    let desc = if desc.is_empty() {
        "Sesión de escritura".to_string()
    } else {
        desc
    };

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

    let nombres: Vec<String> = state
        .agenda
        .eventos
        .iter()
        .map(|e| format!("{} - {} ({})", e.id, e.titulo, e.tipo))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    let idx = match menu("¿Cuál eliminar?", &refs) {
        Some(i) => i,
        None => return,
    };
    let nombre = state.agenda.eventos[idx].titulo.clone();

    if Confirm::new()
        .with_prompt(format!("  ¿Eliminar '{}'?", nombre))
        .default(false)
        .interact()
        .unwrap_or(false)
    {
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

    let nombres: Vec<String> = state
        .agenda
        .eventos
        .iter()
        .map(|e| format!("{} - {}", e.id, e.titulo))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    let idx = match menu("¿Cuál evento?", &refs) {
        Some(i) => i,
        None => return,
    };
    let palabras = match pedir_texto("Palabras clave para recordar (separadas por coma)") {
        Some(t) => t,
        None => return,
    };
    let tags: Vec<String> = palabras
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let evento = &state.agenda.eventos[idx];
    let recuerdo =
        Recuerdo::new(format!("Evento: {}", evento.titulo), tags).con_origen("evento", &evento.id);
    state.memoria.agregar_recuerdo(recuerdo);

    println!("  🧠 Guardado en la memoria");
    pausa();
}

// ══════════════════════════════════════════════════════════════
//  Módulo: CALENDARIO ANUAL
// ══════════════════════════════════════════════════════════════

fn es_bisiesto(anio: i32) -> bool {
    (anio % 4 == 0 && anio % 100 != 0) || anio % 400 == 0
}

fn dias_en_anio(anio: i32) -> u32 {
    if es_bisiesto(anio) {
        366
    } else {
        365
    }
}

fn nombre_mes(mes: u32) -> &'static str {
    match mes {
        1 => "ENERO",
        2 => "FEBRERO",
        3 => "MARZO",
        4 => "ABRIL",
        5 => "MAYO",
        6 => "JUNIO",
        7 => "JULIO",
        8 => "AGOSTO",
        9 => "SEPTIEMBRE",
        10 => "OCTUBRE",
        11 => "NOVIEMBRE",
        12 => "DICIEMBRE",
        _ => "",
    }
}

fn dias_en_mes(anio: i32, mes: u32) -> u32 {
    match mes {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if es_bisiesto(anio) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

fn colorear_dia(
    texto: &str,
    fecha: NaiveDate,
    es_hoy: bool,
    tiene_evento: bool,
    marcas: &[&DiaMarcado],
) -> String {
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
        )
        .cyan()
        .bold()
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
            if i > 0 {
                print!("{}", separador_col);
            }
            let header = format!("{} {}", nombre_mes(mes), anio);
            let (pad_i, pad_d) = centrar_texto(&header, 28);
            print!("{}{}{}", pad_i, header.cyan().bold(), pad_d);
        }
        println!();

        // Días de la semana
        print!("  ");
        for (i, _) in meses.iter().enumerate() {
            if i > 0 {
                print!("{}", separador_col);
            }
            print!("{}", ENCABEZADO_DIAS.dimmed());
        }
        println!();

        // Líneas de días
        let lineas_mes: Vec<Vec<String>> = meses
            .iter()
            .map(|&m| generar_lineas_mes(anio, m, state))
            .collect();

        let max_lineas = lineas_mes.iter().map(|l| l.len()).max().unwrap_or(0);
        for fila_linea in 0..max_lineas {
            print!("  ");
            for (i, lineas) in lineas_mes.iter().enumerate() {
                if i > 0 {
                    print!("{}", separador_col);
                }
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

    let desde = match pedir_fecha("Fecha inicio") {
        Some(f) => f,
        None => return,
    };
    let hasta = match pedir_fecha("Fecha fin") {
        Some(f) => f,
        None => return,
    };

    let dias = (hasta - desde).num_days();
    let semanas = dias / 7;
    let dias_restantes = dias % 7;
    let meses_aprox = dias as f64 / 30.44;

    println!();
    println!(
        "  📅 {} → {}",
        desde.format("%d/%m/%Y"),
        hasta.format("%d/%m/%Y")
    );
    println!();
    println!("  📏 {} días calendario", dias.to_string().cyan().bold());
    println!(
        "  📅 {} semanas y {} días",
        semanas.to_string().cyan().bold(),
        dias_restantes.to_string().cyan()
    );
    println!("  🗓️ ~{:.1} meses", meses_aprox);

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
    println!(
        "  🏢 {} días laborales, {} fines de semana",
        dias_laborales.to_string().green(),
        fines_semana.to_string().red()
    );
    println!();
    pausa();
}

fn avanzar_semanas() {
    separador("📐 Avanzar semanas/días desde una fecha");

    let desde = match pedir_fecha("Fecha base") {
        Some(f) => f,
        None => return,
    };

    let opciones = &["Semanas", "Días", "Meses"];
    let unidad = match menu("¿Qué unidad avanzar?", opciones) {
        Some(i) => i,
        None => return,
    };

    let cantidad_str = pedir_texto_opcional("Cantidad");
    let cantidad: i64 = match cantidad_str.parse() {
        Ok(n) => n,
        Err(_) => {
            println!("  {} Número inválido", "✗".red());
            pausa();
            return;
        }
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

    let nombre_unidad = match unidad {
        0 => "semanas",
        1 => "días",
        _ => "meses",
    };

    println!();
    println!(
        "  📅 {} + {} {} = {}",
        desde.format("%d/%m/%Y"),
        cantidad.to_string().cyan().bold(),
        nombre_unidad,
        resultado
            .format("%A %d de %B de %Y")
            .to_string()
            .green()
            .bold()
    );

    let dias_diff = (resultado - desde).num_days().abs();
    println!("  📏 ({} días calendario)", dias_diff);
    println!();
    pausa();
}

fn marcar_dia_calendario(state: &mut AppState) {
    separador("🎨 Marcar día en el calendario");

    let opciones_modo = &[
        "Marcar un día específico",
        "Marcar un rango de fechas",
        "Limpiar marcas de un día",
    ];
    let modo = match menu("¿Qué deseas hacer?", opciones_modo) {
        Some(i) => i,
        None => return,
    };

    if modo == 2 {
        let fecha = match pedir_fecha("Fecha a limpiar") {
            Some(f) => f,
            None => return,
        };
        state.agenda.limpiar_marcas(fecha);
        println!(
            "  {} Marcas eliminadas para {}",
            "✓".green(),
            fecha.format("%d/%m/%Y")
        );
        pausa();
        return;
    }

    let tipos = &[
        "Libre (verde)",
        "Feriado (rojo)",
        "Vacaciones (cyan)",
        "Vencimiento (amarillo)",
        "Importante (magenta)",
        "Otro (azul)",
    ];
    let ti = match menu("Tipo de marca", tipos) {
        Some(i) => i,
        None => return,
    };
    let tipo = match ti {
        0 => TipoDiaMarcado::Libre,
        1 => TipoDiaMarcado::Feriado,
        2 => TipoDiaMarcado::Vacaciones,
        3 => TipoDiaMarcado::Vencimiento,
        4 => TipoDiaMarcado::Importante,
        _ => {
            let nombre = pedir_texto_opcional("Nombre del tipo");
            TipoDiaMarcado::Otro(if nombre.is_empty() {
                "Otro".to_string()
            } else {
                nombre
            })
        }
    };

    let nota = pedir_texto_opcional("Nota (opcional)");

    if modo == 0 {
        let fecha = match pedir_fecha("Fecha") {
            Some(f) => f,
            None => return,
        };
        state.agenda.marcar_dia(DiaMarcado { fecha, tipo, nota });
        println!("  {} Día {} marcado", "✓".green(), fecha.format("%d/%m/%Y"));
    } else {
        let desde = match pedir_fecha("Desde") {
            Some(f) => f,
            None => return,
        };
        let hasta = match pedir_fecha("Hasta") {
            Some(f) => f,
            None => return,
        };
        let dias = (hasta - desde).num_days().abs() + 1;
        state.agenda.marcar_rango(desde, hasta, tipo, nota);
        println!(
            "  {} {} días marcados ({} → {})",
            "✓".green(),
            dias,
            desde.format("%d/%m/%Y"),
            hasta.format("%d/%m/%Y")
        );
    }
    pausa();
}

fn ver_mes_detallado(state: &AppState) {
    separador("🔍 Ver mes en detalle");

    let hoy = Local::now().date_naive();
    let anio_str = pedir_texto_opcional(&format!("Año (Enter={})", hoy.year()));
    let anio: i32 = if anio_str.is_empty() {
        hoy.year()
    } else {
        match anio_str.parse() {
            Ok(a) => a,
            Err(_) => {
                println!("  {} Año inválido", "✗".red());
                pausa();
                return;
            }
        }
    };

    let meses_nombres = &[
        "Enero",
        "Febrero",
        "Marzo",
        "Abril",
        "Mayo",
        "Junio",
        "Julio",
        "Agosto",
        "Septiembre",
        "Octubre",
        "Noviembre",
        "Diciembre",
    ];
    let default_mes = (hoy.month() - 1) as usize;
    let mi = Select::new()
        .with_prompt("  Mes")
        .items(meses_nombres)
        .default(default_mes)
        .interact_opt()
        .unwrap_or(None);
    let mes = match mi {
        Some(i) => (i + 1) as u32,
        None => return,
    };

    limpiar();
    println!();
    imprimir_mes(anio, mes, state);
    println!();

    // Listar eventos del mes
    let total_dias = dias_en_mes(anio, mes);
    let mut hay_info = false;

    for dia in 1..=total_dias {
        let fecha = NaiveDate::from_ymd_opt(anio, mes, dia).unwrap();
        let eventos: Vec<_> = state
            .agenda
            .eventos
            .iter()
            .filter(|e| e.ocurre_en(fecha))
            .collect();
        let marcas = state.agenda.marcas_del_dia(fecha);

        if !eventos.is_empty() || !marcas.is_empty() {
            hay_info = true;
            let dia_nombre = match fecha.weekday() {
                Weekday::Mon => "Lun",
                Weekday::Tue => "Mar",
                Weekday::Wed => "Mié",
                Weekday::Thu => "Jue",
                Weekday::Fri => "Vie",
                Weekday::Sat => "Sáb",
                Weekday::Sun => "Dom",
            };
            println!("  📌 {} {}:", fecha.format("%d/%m"), dia_nombre);
            for e in &eventos {
                let fin = e
                    .hora_fin
                    .map(|h| format!("-{}", h.format("%H:%M")))
                    .unwrap_or_default();
                let icono = match e.tipo {
                    TipoEvento::Cumpleanos => "🎂",
                    TipoEvento::Pago => "💰",
                    _ => "📅",
                };
                let recur = e.etiqueta_recurrencia();
                let concepto_txt = if e.concepto.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", e.concepto)
                };
                println!(
                    "      {} {}{} {}{}{}",
                    icono,
                    e.hora_inicio.format("%H:%M"),
                    fin,
                    e.titulo,
                    recur,
                    concepto_txt.dimmed()
                );
                println!(
                    "         📆 {} {}  🕐 {} {}",
                    "Evento:".dimmed(),
                    e.fecha.format("%d/%m/%Y").to_string().cyan(),
                    "Registrado:".dimmed(),
                    e.creado.format("%d/%m/%Y %H:%M").to_string().dimmed(),
                );
            }
            for m in &marcas {
                let nota_txt = if m.nota.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", m.nota)
                };
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

    let trimestres = &[
        "Q1 (Ene-Mar)",
        "Q2 (Abr-Jun)",
        "Q3 (Jul-Sep)",
        "Q4 (Oct-Dic)",
    ];
    let qi = Select::new()
        .with_prompt("  Trimestre")
        .items(trimestres)
        .default(trimestre_actual)
        .interact_opt()
        .unwrap_or(None);
    let q = match qi {
        Some(i) => i,
        None => return,
    };

    let mes_inicio = (q as u32) * 3 + 1;
    let mes_fin = mes_inicio + 2;

    limpiar();
    println!();
    println!(
        "  {}",
        format!(
            "📊 {} {} — {}",
            trimestres[q],
            anio,
            if es_bisiesto(anio) {
                "Año bisiesto"
            } else {
                "Año regular"
            }
        )
        .cyan()
        .bold()
    );
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
            total_eventos += state
                .agenda
                .eventos
                .iter()
                .filter(|e| e.ocurre_en(fecha))
                .count();
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

    println!(
        "  📌 {} eventos programados",
        total_eventos.to_string().cyan().bold()
    );
    println!(
        "  🎨 {} días marcados ({} libres, {} feriados)",
        total_marcas.to_string().cyan(),
        dias_libres.to_string().green(),
        dias_feriado.to_string().red()
    );
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
                println!(
                    "  🖼️  [{}] {} — {} elementos",
                    c.id.dimmed(),
                    c.nombre,
                    c.total_elementos()
                );
            }
        } else {
            println!(
                "  {}",
                "(sin canvas — crea tu primer board de ideas)".dimmed()
            );
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
    let nombre = match pedir_texto("Nombre (ej: Ideas proyecto, Brainstorm, Inspiración)") {
        Some(t) => t,
        None => return,
    };
    let c = Canvas::new(nombre.clone(), 800, 600);
    println!(
        "  {} [{}] {}",
        "✓ Canvas creado:".green().bold(),
        c.id,
        nombre
    );
    state.canvases.push(c);
    pausa();
}

fn seleccionar_canvas(state: &AppState) -> Option<usize> {
    if state.canvases.is_empty() {
        println!("  {}", "No hay canvases creados.".yellow());
        pausa();
        return None;
    }
    let nombres: Vec<String> = state
        .canvases
        .iter()
        .map(|c| {
            format!(
                "[{}] {} ({} elementos)",
                c.id,
                c.nombre,
                c.total_elementos()
            )
        })
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
    menu("Selecciona canvas", &refs)
}

fn agregar_nota_canvas(state: &mut AppState) {
    let idx = match seleccionar_canvas(state) {
        Some(i) => i,
        None => return,
    };
    println!("  Escribe tu nota o idea. Puede ser larga, un resumen,");
    println!("  una cita, lo que quieras poner en el board.");
    let contenido = match pedir_texto("Nota") {
        Some(t) => t,
        None => return,
    };

    let colores = &[
        "🔵 Azul",
        "🟢 Verde",
        "🟡 Amarillo",
        "🔴 Rojo",
        "🟣 Morado",
        "⚪ Blanco",
    ];
    let ci = match menu("Color de la nota", colores) {
        Some(i) => i,
        None => return,
    };
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
    let idx = match seleccionar_canvas(state) {
        Some(i) => i,
        None => return,
    };
    println!("  Ingresa la ruta al archivo de imagen o una URL.");
    println!("  Ejemplos:");
    println!("    C:\\Users\\fotos\\idea.png");
    println!("    /storage/emulated/0/DCIM/foto.jpg");
    println!("    https://ejemplo.com/imagen.png");
    let ruta = match pedir_texto("Ruta o URL de la imagen") {
        Some(t) => t,
        None => return,
    };

    // Verificar si es archivo local
    if !ruta.starts_with("http://")
        && !ruta.starts_with("https://")
        && !std::path::Path::new(&ruta).exists()
    {
        println!(
            "  ⚠️ El archivo '{}' no existe. ¿Agregar de todos modos?",
            ruta
        );
        if !Confirm::new()
            .with_prompt("  ¿Continuar?")
            .default(false)
            .interact()
            .unwrap_or(false)
        {
            return;
        }
    }

    state.canvases[idx].agregar_elemento(Elemento::imagen(ruta));
    println!("  {} Imagen agregada al canvas", "✓".green().bold());
    pausa();
}

fn agregar_lista_canvas(state: &mut AppState) {
    let idx = match seleccionar_canvas(state) {
        Some(i) => i,
        None => return,
    };
    println!("  Escribe los items separados por coma o punto y coma.");
    let items_str = match pedir_texto("Items (separados por , o ;)") {
        Some(t) => t,
        None => return,
    };

    let items: Vec<&str> = items_str
        .split([',', ';'])
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
    let ci = match menu("Color", colores) {
        Some(i) => i,
        None => return,
    };
    let color = match ci {
        0 => "#4ecdc4",
        1 => "#00d4ff",
        2 => "#f9ca24",
        _ => "#ffffff",
    };

    state.canvases[idx].agregar_elemento(Elemento::lista(contenido, color.to_string()));
    println!(
        "  {} Lista con {} items agregada",
        "✓".green().bold(),
        items.len()
    );
    pausa();
}

fn agregar_seccion_canvas(state: &mut AppState) {
    let idx = match seleccionar_canvas(state) {
        Some(i) => i,
        None => return,
    };
    let titulo = match pedir_texto("Título de la sección") {
        Some(t) => t,
        None => return,
    };
    state.canvases[idx].agregar_elemento(Elemento::seccion(titulo));
    println!("  {} Sección agregada", "✓".green().bold());
    pausa();
}

fn ver_canvas(state: &AppState) {
    let idx = match seleccionar_canvas(state) {
        Some(i) => i,
        None => return,
    };
    let c = &state.canvases[idx];
    separador(&format!("🎨 {}", c.nombre));

    if c.elementos.is_empty() && c.trazos.is_empty() {
        println!("  {}", "(canvas vacío)".dimmed());
    } else {
        for elem in &c.elementos {
            match &elem.tipo {
                omniplanner::canvas::TipoElemento::Nota => {
                    println!("  📝 [{}] {}", elem.id.dimmed(), elem.contenido);
                    println!(
                        "     {}",
                        elem.creado.format("%d/%m/%Y %H:%M").to_string().dimmed()
                    );
                }
                omniplanner::canvas::TipoElemento::Imagen => {
                    println!("  🖼️  [{}] {}", elem.id.dimmed(), elem.contenido);
                    println!(
                        "     {}",
                        elem.creado.format("%d/%m/%Y %H:%M").to_string().dimmed()
                    );
                }
                omniplanner::canvas::TipoElemento::Lista => {
                    println!("  📋 [{}] Lista:", elem.id.dimmed());
                    for (i, item) in elem.contenido.lines().enumerate() {
                        println!("     {}. {}", i + 1, item);
                    }
                    println!(
                        "     {}",
                        elem.creado.format("%d/%m/%Y %H:%M").to_string().dimmed()
                    );
                }
                omniplanner::canvas::TipoElemento::Seccion => {
                    println!();
                    println!(
                        "  {} {} {}",
                        "──".dimmed(),
                        elem.contenido.bold(),
                        "──".dimmed()
                    );
                }
            }
            println!();
        }
        if !c.trazos.is_empty() {
            println!("  ✏️ {} trazos legacy", c.trazos.len());
        }
    }
    pausa();
}

fn editar_elemento_canvas(state: &mut AppState) {
    let idx = match seleccionar_canvas(state) {
        Some(i) => i,
        None => return,
    };
    if state.canvases[idx].elementos.is_empty() {
        println!("  {}", "No hay elementos para editar.".yellow());
        pausa();
        return;
    }

    let nombres: Vec<String> = state.canvases[idx]
        .elementos
        .iter()
        .map(|e| format!("[{}] {}", e.id, e))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
    let ei = match menu("¿Cuál elemento?", &refs) {
        Some(i) => i,
        None => return,
    };

    let nuevo = match pedir_texto("Nuevo contenido") {
        Some(t) => t,
        None => return,
    };
    state.canvases[idx].elementos[ei].contenido = nuevo;
    println!("  {} Elemento actualizado", "✓".green().bold());
    pausa();
}

fn eliminar_elemento_canvas(state: &mut AppState) {
    let idx = match seleccionar_canvas(state) {
        Some(i) => i,
        None => return,
    };
    if state.canvases[idx].elementos.is_empty() {
        println!("  {}", "No hay elementos para eliminar.".yellow());
        pausa();
        return;
    }

    let nombres: Vec<String> = state.canvases[idx]
        .elementos
        .iter()
        .map(|e| format!("[{}] {}", e.id, e))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
    let ei = match menu("¿Cuál eliminar?", &refs) {
        Some(i) => i,
        None => return,
    };

    let id = state.canvases[idx].elementos[ei].id.clone();
    state.canvases[idx].eliminar_elemento(&id);
    println!("  {} Elemento eliminado", "✓".green().bold());
    pausa();
}

fn exportar_canvas_html(state: &AppState) {
    let idx = match seleccionar_canvas(state) {
        Some(i) => i,
        None => return,
    };
    let nombre_archivo = format!(
        "{}.html",
        state.canvases[idx].nombre.replace(' ', "_").to_lowercase()
    );
    let sugerencia = pedir_texto_opcional(&format!("Archivo (Enter = {})", nombre_archivo));
    let archivo = if sugerencia.is_empty() {
        nombre_archivo
    } else {
        sugerencia
    };

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
    let idx = match seleccionar_canvas(state) {
        Some(i) => i,
        None => return,
    };
    let nombre = state.canvases[idx].nombre.clone();

    if Confirm::new()
        .with_prompt(format!(
            "  ¿Eliminar canvas '{}'? Esto no se puede deshacer",
            nombre
        ))
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
                println!(
                    "  📊 [{}] {} — {} | {} nodos, {} conexiones",
                    d.id.dimmed(),
                    d.nombre,
                    d.tipo,
                    d.nodos.len(),
                    d.conexiones.len()
                );
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
    let nombres: Vec<String> = state
        .diagramas
        .iter()
        .map(|d| format!("[{}] {} ({})", d.id, d.nombre, d.tipo))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
    menu("Selecciona diagrama", &refs)
}

fn nuevo_diagrama(state: &mut AppState) {
    separador("📊 Nuevo diagrama");
    let nombre = match pedir_texto("Nombre") {
        Some(t) => t,
        None => return,
    };
    let tipos = &[
        "Diagrama de Flujo",
        "Algoritmo",
        "Proceso",
        "Flujo de Datos",
        "Libre",
    ];
    let ti = match menu("Tipo", tipos) {
        Some(i) => i,
        None => return,
    };
    let tipo = match ti {
        0 => TipoDiagrama::Flujo,
        1 => TipoDiagrama::Algoritmo,
        2 => TipoDiagrama::Proceso,
        3 => TipoDiagrama::DatosFlujo,
        _ => TipoDiagrama::Libre,
    };

    let d = Diagrama::new(nombre.clone(), tipo);
    println!(
        "  {} [{}] {}",
        "✓ Diagrama creado:".green().bold(),
        d.id,
        nombre
    );
    state.diagramas.push(d);
    pausa();
}

fn agregar_nodo(state: &mut AppState) {
    let idx = match seleccionar_diagrama(state) {
        Some(i) => i,
        None => return,
    };

    let tipos_nodo = &[
        "⬤ Inicio",
        "◯ Fin",
        "▭ Proceso",
        "◇ Decisión",
        "▱ Entrada/Salida",
        "● Conector",
        "▭▭ Subproceso",
        "▤ Dato",
    ];
    let ni = match menu("Tipo de nodo", tipos_nodo) {
        Some(i) => i,
        None => return,
    };
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

    let etiqueta = match pedir_texto("Etiqueta del nodo") {
        Some(t) => t,
        None => return,
    };
    let nodo = Nodo::new(tipo, etiqueta.clone(), 0.0, 0.0);
    let nid = state.diagramas[idx].agregar_nodo(nodo);
    println!("  {} Nodo [{}] '{}' agregado", "✓".green(), nid, etiqueta);

    // ¿Agregar otro?
    if Confirm::new()
        .with_prompt("  ¿Agregar otro nodo?")
        .default(true)
        .interact()
        .unwrap_or(false)
    {
        agregar_nodo_al(state, idx);
    }
    pausa();
}

fn agregar_nodo_al(state: &mut AppState, idx: usize) {
    let tipos_nodo = &[
        "⬤ Inicio",
        "◯ Fin",
        "▭ Proceso",
        "◇ Decisión",
        "▱ Entrada/Salida",
        "● Conector",
        "▭▭ Subproceso",
        "▤ Dato",
    ];
    let ni = match menu("Tipo de nodo", tipos_nodo) {
        Some(i) => i,
        None => return,
    };
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

    let etiqueta = match pedir_texto("Etiqueta del nodo") {
        Some(t) => t,
        None => return,
    };
    let nodo = Nodo::new(tipo, etiqueta.clone(), 0.0, 0.0);
    let nid = state.diagramas[idx].agregar_nodo(nodo);
    println!("  {} Nodo [{}] '{}' agregado", "✓".green(), nid, etiqueta);

    if Confirm::new()
        .with_prompt("  ¿Agregar otro nodo?")
        .default(true)
        .interact()
        .unwrap_or(false)
    {
        agregar_nodo_al(state, idx);
    }
}

fn conectar_nodos(state: &mut AppState) {
    let idx = match seleccionar_diagrama(state) {
        Some(i) => i,
        None => return,
    };

    if state.diagramas[idx].nodos.len() < 2 {
        println!("  {}", "Necesitas al menos 2 nodos para conectar.".yellow());
        pausa();
        return;
    }

    let nodos: Vec<String> = state.diagramas[idx]
        .nodos
        .iter()
        .map(|n| format!("[{}] {} {}", n.id, n.tipo, n.etiqueta))
        .collect();
    let refs: Vec<&str> = nodos.iter().map(|s| s.as_str()).collect();

    println!("  Selecciona el nodo ORIGEN:");
    let oi = match menu("Origen", &refs) {
        Some(i) => i,
        None => return,
    };
    println!("  Selecciona el nodo DESTINO:");
    let di = match menu("Destino", &refs) {
        Some(i) => i,
        None => return,
    };

    let etiqueta = pedir_texto_opcional("Etiqueta de la conexión (ej: Sí, No, opcional)");
    let etiqueta = if etiqueta.is_empty() {
        None
    } else {
        Some(etiqueta)
    };

    let origen_id = state.diagramas[idx].nodos[oi].id.clone();
    let destino_id = state.diagramas[idx].nodos[di].id.clone();

    state.diagramas[idx].conectar(&origen_id, &destino_id, TipoConexion::Flecha, etiqueta);
    println!("  {} Conexión creada", "✓".green());

    if Confirm::new()
        .with_prompt("  ¿Crear otra conexión?")
        .default(true)
        .interact()
        .unwrap_or(false)
    {
        conectar_nodos_en(state, idx);
    }
    pausa();
}

fn conectar_nodos_en(state: &mut AppState, idx: usize) {
    let nodos: Vec<String> = state.diagramas[idx]
        .nodos
        .iter()
        .map(|n| format!("[{}] {} {}", n.id, n.tipo, n.etiqueta))
        .collect();
    let refs: Vec<&str> = nodos.iter().map(|s| s.as_str()).collect();

    let oi = match menu("Origen", &refs) {
        Some(i) => i,
        None => return,
    };
    let di = match menu("Destino", &refs) {
        Some(i) => i,
        None => return,
    };
    let etiqueta = pedir_texto_opcional("Etiqueta (opcional)");
    let etiqueta = if etiqueta.is_empty() {
        None
    } else {
        Some(etiqueta)
    };

    let origen_id = state.diagramas[idx].nodos[oi].id.clone();
    let destino_id = state.diagramas[idx].nodos[di].id.clone();
    state.diagramas[idx].conectar(&origen_id, &destino_id, TipoConexion::Flecha, etiqueta);
    println!("  {} Conexión creada", "✓".green());

    if Confirm::new()
        .with_prompt("  ¿Otra conexión?")
        .default(false)
        .interact()
        .unwrap_or(false)
    {
        conectar_nodos_en(state, idx);
    }
}

fn ver_mermaid(state: &AppState) {
    let idx = match seleccionar_diagrama(state) {
        Some(i) => i,
        None => return,
    };
    separador("Mermaid");
    println!("{}", state.diagramas[idx].exportar_mermaid());
    pausa();
}

fn ver_pseudo(state: &AppState) {
    let idx = match seleccionar_diagrama(state) {
        Some(i) => i,
        None => return,
    };
    separador("Pseudocódigo");
    println!("{}", state.diagramas[idx].exportar_pseudocodigo());
    pausa();
}

fn validar_diagrama(state: &AppState) {
    let idx = match seleccionar_diagrama(state) {
        Some(i) => i,
        None => return,
    };
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
    let idx = match seleccionar_diagrama(state) {
        Some(i) => i,
        None => return,
    };
    let palabras = match pedir_texto("Palabras clave (separadas por coma)") {
        Some(t) => t,
        None => return,
    };
    let tags: Vec<String> = palabras
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let diag = &state.diagramas[idx];
    let recuerdo =
        Recuerdo::new(format!("Diagrama: {}", diag.nombre), tags).con_origen("diagrama", &diag.id);
    state.memoria.agregar_recuerdo(recuerdo);

    println!("  🧠 Guardado en la memoria");
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
        println!(
            "  Ramas: {}",
            state
                .vcs
                .ramas
                .iter()
                .map(|r| {
                    if r.nombre == state.vcs.rama_actual {
                        format!("*{}", r.nombre).green().to_string()
                    } else {
                        r.nombre.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        );

        let log = state.vcs.log();
        if !log.is_empty() {
            println!();
            println!("  {}", "Historial:".bold());
            for s in log.iter().rev().take(10) {
                println!(
                    "    {} {} — {} ({})",
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
                let mensaje = match pedir_texto("Mensaje del commit") {
                    Some(t) => t,
                    None => continue,
                };
                let autor = pedir_texto_opcional("Autor");
                let autor = if autor.is_empty() {
                    "usuario".to_string()
                } else {
                    autor
                };
                let datos = serde_json::to_string(&state.tasks).unwrap_or_default();
                let id = state.vcs.commit(datos, mensaje.clone(), autor);
                println!("  {} Commit [{}]: {}", "✓".green(), id, mensaje);
                pausa();
            }
            Some(1) => {
                let nombre = match pedir_texto("Nombre de la nueva rama") {
                    Some(t) => t,
                    None => continue,
                };
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
                let idx = match menu("Selecciona rama", &refs) {
                    Some(i) => i,
                    None => continue,
                };
                state.vcs.cambiar_rama(&ramas[idx]);
                println!("  {} Cambiado a '{}'", "✓".green(), ramas[idx]);
                pausa();
            }
            Some(3) => {
                let log = state.vcs.log();
                separador("Log completo");
                for s in log.iter().rev() {
                    println!(
                        "  {} {} — {} ({})",
                        format!("[{}]", &s.hash[..7]).yellow(),
                        s.mensaje,
                        s.autor,
                        s.timestamp.format("%d/%m/%Y %H:%M")
                    );
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
                let texto = match pedir_texto("Texto a codificar") {
                    Some(t) => t,
                    None => continue,
                };
                let formatos = &["Base64", "Hexadecimal", "Binario"];
                let fi = match menu("Formato", formatos) {
                    Some(i) => i,
                    None => continue,
                };
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
                let hex = match pedir_texto("Texto en hexadecimal") {
                    Some(t) => t,
                    None => continue,
                };
                match Mapper::decodificar_hex(&hex) {
                    Some(texto) => println!("  {} → {}", "hex".cyan(), texto.green().bold()),
                    None => println!("  {} Formato hex inválido", "✗".red()),
                }
                pausa();
            }
            Some(2) => {
                let nombre = match pedir_texto("Nombre del esquema") {
                    Some(t) => t,
                    None => continue,
                };
                let cods = &["UTF-8", "JSON", "CSV", "Base64", "Hex", "Binario"];
                let ei = match menu("Codificación de entrada", cods) {
                    Some(i) => i,
                    None => continue,
                };
                let si = match menu("Codificación de salida", cods) {
                    Some(i) => i,
                    None => continue,
                };
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
                let nombres: Vec<String> = state
                    .mapper
                    .esquemas
                    .iter()
                    .map(|e| format!("[{}] {}", e.id, e.nombre))
                    .collect();
                let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
                let idx = match menu("Esquema", &refs) {
                    Some(i) => i,
                    None => continue,
                };
                let origen = match pedir_texto("Campo origen") {
                    Some(t) => t,
                    None => continue,
                };
                let destino = match pedir_texto("Campo destino") {
                    Some(t) => t,
                    None => continue,
                };
                let trans = pedir_texto_opcional(
                    "Transformación (uppercase, lowercase, trim, reverse, prefix:X, suffix:X)",
                );
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
            println!(
                "  {} {}",
                "📚 Recuerdos:".bold(),
                state.memoria.recuerdos.len()
            );
            println!(
                "  {} {}",
                "🏷️  Palabras clave:".bold(),
                if palabras.is_empty() {
                    "(ninguna)".dimmed().to_string()
                } else {
                    palabras
                        .iter()
                        .map(|p| p.cyan().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            );
            if !state.memoria.enlaces.is_empty() {
                println!("  {} {}", "🔗 Enlaces:".bold(), state.memoria.enlaces.len());
            }
            let n_ideas = state.memoria.diccionario.todas_las_ideas().len();
            let n_conexiones = state.memoria.diccionario.conexiones.len();
            if n_ideas > 0 {
                println!(
                    "  {} {} ideas, {} conexiones neuronales",
                    "🧬 Diccionario:".bold(),
                    n_ideas,
                    n_conexiones
                );
            }
        } else {
            println!(
                "  {}",
                "(vacío — crea tu primer recuerdo o apunte)".dimmed()
            );
            println!();
            println!(
                "  {}",
                "La memoria es tu espacio para anotar TODO lo que".dimmed()
            );
            println!(
                "  {}",
                "necesites: citas, ideas, apuntes, instrucciones...".dimmed()
            );
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
    if state.memoria.diccionario.conexiones.is_empty()
        && state.memoria.diccionario.historial.is_empty()
    {
        println!("  {}", "El diccionario neuronal está vacío.".dimmed());
        println!(
            "  {}",
            "Se llenará automáticamente al completar tareas con palabras clave.".dimmed()
        );
        pausa();
        return;
    }

    loop {
        limpiar();
        separador("🧬 DICCIONARIO NEURONAL DE IDEAS");

        let mut ideas: Vec<String> = state
            .memoria
            .diccionario
            .todas_las_ideas()
            .into_iter()
            .cloned()
            .collect();
        ideas.sort();
        println!(
            "  📊 {} ideas | {} conexiones | {} entradas",
            ideas.len(),
            state.memoria.diccionario.conexiones.len(),
            state.memoria.diccionario.historial.len()
        );
        println!();

        // Mostrar las conexiones más fuertes
        {
            let mut conexiones_ord: Vec<(String, String, u32, String)> = state
                .memoria
                .diccionario
                .conexiones
                .iter()
                .map(|c| {
                    (
                        c.palabra_a.clone(),
                        c.palabra_b.clone(),
                        c.fuerza,
                        c.contexto.last().cloned().unwrap_or_default(),
                    )
                })
                .collect();
            conexiones_ord.sort_by(|a, b| b.2.cmp(&a.2));

            if !conexiones_ord.is_empty() {
                println!("  🔥 Conexiones más fuertes:");
                for (pa, pb, fuerza, ctx) in conexiones_ord.iter().take(10) {
                    let barra = "█".repeat(*fuerza as usize).cyan();
                    let ctx_str = if ctx.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", ctx.dimmed())
                    };
                    println!(
                        "    {} {} ↔ {} [{}]{}",
                        barra,
                        pa.yellow(),
                        pb.yellow(),
                        fuerza,
                        ctx_str
                    );
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
                    println!(
                        "  {} [{}] {} — \"{}\"",
                        format!("{}.", i + 1).dimmed(),
                        e.modulo.cyan(),
                        e.item_titulo.bold(),
                        if e.nota.is_empty() {
                            "-".to_string()
                        } else {
                            e.nota.clone()
                        }
                    );
                    println!(
                        "    🏷️  {} | {}",
                        e.palabras.join(", ").yellow(),
                        e.creado.format("%d/%m/%Y %H:%M").to_string().dimmed()
                    );
                }
                pausa();
            }
            Some(2) => {
                separador("🗺️  Mapa de ideas");
                for idea in &ideas {
                    let relacionadas = state.memoria.diccionario.ideas_relacionadas(idea);
                    if !relacionadas.is_empty() {
                        let rels: Vec<String> = relacionadas
                            .iter()
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
    println!("  🧬 Explorando: {}", idea.yellow().bold());
    println!();

    // Ideas relacionadas
    let relacionadas = dic.ideas_relacionadas(idea);
    if !relacionadas.is_empty() {
        println!("  🔗 Conexiones:");
        for (rel, fuerza) in &relacionadas {
            let barra = "█".repeat(*fuerza as usize).cyan();
            println!("    {} {} (fuerza: {})", barra, rel.yellow(), fuerza);
        }
        println!();
    }

    // Recuerdos con esta palabra
    let recuerdos = state.memoria.recuerdos_con_palabra(idea);
    if !recuerdos.is_empty() {
        println!("  🧠 Recuerdos asociados:");
        for r in &recuerdos {
            println!("    • [{}] {}", r.id.dimmed(), r.contenido);
        }
        println!();
    }

    // Historial de esta palabra
    let historial: Vec<_> = dic
        .historial
        .iter()
        .filter(|e| {
            e.palabras
                .iter()
                .any(|p| p.to_lowercase() == idea.to_lowercase())
        })
        .collect();
    if !historial.is_empty() {
        println!("  📋 Historial:");
        for e in &historial {
            println!(
                "    • [{}] {} — {} ({})",
                e.modulo.cyan(),
                e.item_titulo.bold(),
                if e.nota.is_empty() { "-" } else { &e.nota },
                e.creado.format("%d/%m/%Y").to_string().dimmed()
            );
        }
        println!();
    }

    // Sugerir ideas basadas en esta
    let sugerencias = dic.sugerir(std::slice::from_ref(idea));
    if !sugerencias.is_empty() {
        println!("  🔮 Sugerencias para explorar:");
        for (sug, score) in sugerencias.iter().take(5) {
            println!("    → {} (relevancia: {})", sug.yellow(), score);
        }
    }

    pausa();
}

fn buscar_memoria(state: &AppState) {
    let consulta = match pedir_texto("¿Qué buscas?") {
        Some(t) => t,
        None => return,
    };
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
                    enlaces_info.push(format!(
                        "🔗 {} [{}] ↔ {} [{}] ({})",
                        e.origen_modulo, e.origen_id, e.destino_modulo, e.destino_id, e.relacion
                    ));
                }
            }
            hallazgos.push(Hallazgo {
                icono: "🧠",
                modulo: "Recuerdo".to_string(),
                titulo: r.contenido.chars().take(60).collect::<String>(),
                detalle: if r.contenido.len() > 60 {
                    r.contenido.clone()
                } else {
                    String::new()
                },
                fecha: r.creado.date(),
                hora: Some(r.creado.time()),
                estado: r
                    .modulo_origen
                    .clone()
                    .unwrap_or_else(|| "apunte".to_string()),
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
            let enlaces_info: Vec<String> = state
                .memoria
                .enlaces_de("tarea", &t.id)
                .iter()
                .map(|e| {
                    format!(
                        "🔗 {} [{}] ↔ {} [{}] ({})",
                        e.origen_modulo, e.origen_id, e.destino_modulo, e.destino_id, e.relacion
                    )
                })
                .collect();

            // Si tiene follow-up, mostrar como entrada separada con su fecha
            let follow_up_info = if let Some(fu) = &t.follow_up {
                format!(
                    "⏰ Follow-up: {} {}",
                    fu.date().format("%d/%m/%Y"),
                    fu.time().format("%H:%M")
                )
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
                    detalle: format!(
                        "📋 Tarea original: {} ({})",
                        t.titulo,
                        t.fecha.format("%d/%m/%Y")
                    ),
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
            let enlaces_info: Vec<String> = state
                .memoria
                .enlaces_de("evento", &e.id)
                .iter()
                .map(|en| {
                    format!(
                        "🔗 {} [{}] ↔ {} [{}] ({})",
                        en.origen_modulo,
                        en.origen_id,
                        en.destino_modulo,
                        en.destino_id,
                        en.relacion
                    )
                })
                .collect();
            let hora_str = e
                .hora_fin
                .map(|fin| format!("{} - {}", e.hora_inicio, fin))
                .unwrap_or_else(|| format!("{}", e.hora_inicio));
            let concepto_txt = if e.concepto.is_empty() {
                String::new()
            } else {
                format!(" [{}]", e.concepto)
            };
            let recur_txt = e.etiqueta_recurrencia();

            // Si es recurrente, buscar próxima ocurrencia futura y la más reciente pasada
            if e.frecuencia != Frecuencia::UnaVez {
                // Solo buscar ocurrencia en el año actual
                let anio_actual = hoy_fecha.year();
                let inicio_anio = NaiveDate::from_ymd_opt(anio_actual, 1, 1).unwrap();
                let fin_anio = NaiveDate::from_ymd_opt(anio_actual, 12, 31).unwrap();
                let ocurrencias_anio =
                    e.frecuencia
                        .proximas_ocurrencias(e.fecha, inicio_anio, fin_anio);

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
                        format!(
                            "Origen: {} (hace ~{} año(s) y {} mes(es))",
                            e.fecha.format("%d/%m/%Y"),
                            anios,
                            meses
                        )
                    } else {
                        format!(
                            "Origen: {} (hace ~{} año(s))",
                            e.fecha.format("%d/%m/%Y"),
                            anios
                        )
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
        println!(
            "  {}",
            format!("No se encontró \"{}\" en ningún módulo.", consulta).yellow()
        );
        pausa();
        return;
    }

    // ── Ordenar por fecha descendente (más recientes primero) ──
    hallazgos.sort_by(|a, b| b.fecha.cmp(&a.fecha).then(b.hora.cmp(&a.hora)));

    // ── Separar pasado / hoy / futuro ──
    let futuro: Vec<&Hallazgo> = hallazgos.iter().filter(|h| h.fecha > hoy_fecha).collect();
    let hoy_items: Vec<&Hallazgo> = hallazgos.iter().filter(|h| h.fecha == hoy_fecha).collect();
    let pasado: Vec<&Hallazgo> = hallazgos.iter().filter(|h| h.fecha < hoy_fecha).collect();

    // ── Mostrar resultados ──
    separador(&format!(
        "🔍 \"{}\" — {} coincidencias",
        consulta,
        hallazgos.len()
    ));

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

        let hora_str = h
            .hora
            .map(|t| format!(" {}", t.format("%H:%M")))
            .unwrap_or_default();
        let estado_str = if h.estado.is_empty() {
            String::new()
        } else {
            format!(" — {}", h.estado)
        };

        println!(
            "  {} {} {} [{}]{}",
            h.icono,
            h.titulo.bold(),
            format!("({})", h.modulo).dimmed(),
            h.id.dimmed(),
            estado_str.dimmed()
        );
        println!(
            "     📆 {}{} ({})",
            h.fecha.format("%d/%m/%Y"),
            hora_str,
            tiempo_rel.cyan()
        );
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
        println!(
            "  {}",
            "◀ HISTORIAL (pasado, más reciente primero)".dimmed().bold()
        );
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
    let m1 = match menu("Módulo origen", modulos) {
        Some(i) => i,
        None => return,
    };
    let (mod1, id1) = seleccionar_item_de_modulo(state, m1);
    if id1.is_empty() {
        return;
    }

    println!("  Selecciona el SEGUNDO elemento:");
    let m2 = match menu("Módulo destino", modulos) {
        Some(i) => i,
        None => return,
    };
    let (mod2, id2) = seleccionar_item_de_modulo(state, m2);
    if id2.is_empty() {
        return;
    }

    let relacion = match pedir_texto("Relación (ej: 'necesita', 'depende de', 'parte de')") {
        Some(t) => t,
        None => return,
    };

    state.memoria.enlazar(&mod1, &id1, &mod2, &id2, &relacion);
    println!("  🔗 Enlace creado: {} ↔ {} ({})", mod1, mod2, relacion);
    pausa();
}

fn seleccionar_item_de_modulo(state: &AppState, modulo_idx: usize) -> (String, String) {
    match modulo_idx {
        0 => {
            if state.tasks.tareas.is_empty() {
                println!("  Sin tareas.");
                return (String::new(), String::new());
            }
            let items: Vec<String> = state
                .tasks
                .tareas
                .iter()
                .map(|t| format!("{} - {}", t.id, t.titulo))
                .collect();
            let refs: Vec<&str> = items.iter().map(|s| s.as_str()).collect();
            let i = match menu("Selecciona", &refs) {
                Some(i) => i,
                None => return (String::new(), String::new()),
            };
            ("tarea".to_string(), state.tasks.tareas[i].id.clone())
        }
        1 => {
            if state.agenda.eventos.is_empty() {
                println!("  Sin eventos.");
                return (String::new(), String::new());
            }
            let items: Vec<String> = state
                .agenda
                .eventos
                .iter()
                .map(|e| format!("{} - {}", e.id, e.titulo))
                .collect();
            let refs: Vec<&str> = items.iter().map(|s| s.as_str()).collect();
            let i = match menu("Selecciona", &refs) {
                Some(i) => i,
                None => return (String::new(), String::new()),
            };
            ("evento".to_string(), state.agenda.eventos[i].id.clone())
        }
        2 => {
            if state.diagramas.is_empty() {
                println!("  Sin diagramas.");
                return (String::new(), String::new());
            }
            let items: Vec<String> = state
                .diagramas
                .iter()
                .map(|d| format!("{} - {}", d.id, d.nombre))
                .collect();
            let refs: Vec<&str> = items.iter().map(|s| s.as_str()).collect();
            let i = match menu("Selecciona", &refs) {
                Some(i) => i,
                None => return (String::new(), String::new()),
            };
            ("diagrama".to_string(), state.diagramas[i].id.clone())
        }
        3 => {
            if state.canvases.is_empty() {
                println!("  Sin canvases.");
                return (String::new(), String::new());
            }
            let items: Vec<String> = state
                .canvases
                .iter()
                .map(|c| format!("{} - {}", c.id, c.nombre))
                .collect();
            let refs: Vec<&str> = items.iter().map(|s| s.as_str()).collect();
            let i = match menu("Selecciona", &refs) {
                Some(i) => i,
                None => return (String::new(), String::new()),
            };
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

    let tema = match pedir_texto("¿Sobre qué tema es? (ej: trabajo, salud, idea, compras)") {
        Some(t) => t,
        None => return,
    };

    println!();
    println!("  Ahora escribe tu apunte. Puede ser tan largo como quieras.");
    println!(
        "  {}",
        "(una línea por ahora, pero ponle todo lo que necesites)".dimmed()
    );
    let contenido = match pedir_texto("¿Qué quieres recordar?") {
        Some(t) => t,
        None => return,
    };

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
        let mi = match menu("¿De qué módulo?", modulos) {
            Some(i) => i,
            None => return,
        };
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
        println!(
            "  {}",
            "Usa '📝 Nuevo apunte' para empezar a anotar.".dimmed()
        );
        pausa();
        return;
    }

    separador("📚 Todos los recuerdos");

    // Agrupar por primera palabra clave (tema)
    let mut temas: std::collections::HashMap<String, Vec<&Recuerdo>> =
        std::collections::HashMap::new();
    for r in &state.memoria.recuerdos {
        let tema = r
            .palabras_clave
            .first()
            .cloned()
            .unwrap_or_else(|| "sin tema".to_string());
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
            println!("    • [{}] {}", r.id.dimmed(), r.contenido);
            println!(
                "      🏷️  {} {} {}",
                r.palabras_clave.join(", ").cyan(),
                origen.dimmed(),
                format!("({})", fecha).dimmed()
            );
        }
        println!();
    }

    println!(
        "  Total: {} recuerdos en {} temas",
        state.memoria.recuerdos.len(),
        temas_ord.len()
    );
    pausa();
}

fn editar_recuerdo(state: &mut AppState) {
    if state.memoria.recuerdos.is_empty() {
        println!("  {}", "Sin recuerdos para editar.".yellow());
        pausa();
        return;
    }

    let nombres: Vec<String> = state
        .memoria
        .recuerdos
        .iter()
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

    let idx = match menu("¿Cuál recuerdo editar?", &refs) {
        Some(i) => i,
        None => return,
    };
    let id = state.memoria.recuerdos[idx].id.clone();

    println!();
    println!(
        "  Contenido actual: {}",
        state.memoria.recuerdos[idx].contenido.cyan()
    );
    println!(
        "  Palabras clave:   {}",
        state.memoria.recuerdos[idx]
            .palabras_clave
            .join(", ")
            .yellow()
    );
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
            let nuevas = match pedir_texto("Palabras clave a agregar (separadas por coma)") {
                Some(t) => t,
                None => {
                    pausa();
                    return;
                }
            };
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
                let pi = match menu("¿Cuál palabra quitar?", &refs_p) {
                    Some(i) => i,
                    None => {
                        pausa();
                        return;
                    }
                };
                let palabra = palabras[pi].clone();

                if Confirm::new()
                    .with_prompt(format!("  ¿Seguro que quieres quitar '{}'?", palabra))
                    .default(false)
                    .interact()
                    .unwrap_or(false)
                {
                    state.memoria.quitar_palabra_de_recuerdo(&id, &palabra);
                    println!(
                        "  {} Palabra '{}' eliminada de este recuerdo",
                        "✓".green(),
                        palabra
                    );
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

        let mut palabras: Vec<String> = state
            .memoria
            .palabras_clave()
            .into_iter()
            .cloned()
            .collect();
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
                let pi = match menu("¿Qué palabra clave?", &refs_p) {
                    Some(i) => i,
                    None => continue,
                };
                let palabra = &palabras[pi];

                let recuerdos = state.memoria.recuerdos_con_palabra(palabra);
                if recuerdos.is_empty() {
                    println!("  {}", "No hay recuerdos con esa palabra.".dimmed());
                } else {
                    println!();
                    println!(
                        "  Recuerdos con '{}' ({}):",
                        palabra.cyan(),
                        recuerdos.len()
                    );
                    for r in &recuerdos {
                        println!("    • [{}] {}", r.id.dimmed(), r.contenido);
                        println!("      🏷️  {}", r.palabras_clave.join(", ").dimmed());
                    }
                }
                pausa();
            }
            Some(1) => {
                let refs_p: Vec<&str> = palabras.iter().map(|s| s.as_str()).collect();
                let pi = match menu("¿Qué palabra clave eliminar?", &refs_p) {
                    Some(i) => i,
                    None => continue,
                };
                let palabra = palabras[pi].clone();
                let count = state.memoria.recuerdos_con_palabra(&palabra).len();

                println!();
                println!(
                    "  {} La palabra '{}' aparece en {} recuerdos.",
                    "⚠".yellow(),
                    palabra.cyan(),
                    count
                );
                println!("  Se eliminará de todos, pero los recuerdos se conservan.");

                if Confirm::new()
                    .with_prompt(format!("  ¿Estás seguro de eliminar '{}'?", palabra))
                    .default(false)
                    .interact()
                    .unwrap_or(false)
                {
                    let afectados = state.memoria.eliminar_palabra_global(&palabra);
                    println!(
                        "  {} Palabra '{}' eliminada de {} recuerdos",
                        "✓".green(),
                        palabra,
                        afectados
                    );
                } else {
                    println!("  Cancelado.");
                }
                pausa();
            }
            Some(2) => {
                let refs_p: Vec<&str> = palabras.iter().map(|s| s.as_str()).collect();
                let pi = match menu("¿De qué palabra clave?", &refs_p) {
                    Some(i) => i,
                    None => continue,
                };
                let palabra = palabras[pi].clone();

                let recuerdos_ids: Vec<(String, String)> = state
                    .memoria
                    .recuerdos_con_palabra(&palabra)
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
                let ri = match menu("¿De cuál recuerdo quitar esta palabra?", &labels) {
                    Some(i) => i,
                    None => continue,
                };
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

    let nombres: Vec<String> = state
        .memoria
        .recuerdos
        .iter()
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

    let idx = match menu("¿Cuál recuerdo eliminar?", &refs) {
        Some(i) => i,
        None => return,
    };
    let id = state.memoria.recuerdos[idx].id.clone();
    let contenido = state.memoria.recuerdos[idx].contenido.clone();

    println!();
    println!("  Contenido: \"{}\"", contenido.cyan());
    println!("  {} Esta acción no se puede deshacer.", "⚠".yellow());

    if Confirm::new()
        .with_prompt("  ¿Estás seguro de eliminar este recuerdo?".to_string())
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
                if state.sync.auto_sync {
                    "✅ Sync automático ACTIVO".green().to_string()
                } else {
                    "✅ Sincronizado (manual)".green().to_string()
                }
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

        println!("  🔑 GitHub Gist:    {}", gist_estado);
        println!("  Google Calendar: {}", gcal);
        println!("  Email SMTP:      {}", email);
        println!("  Google Drive:    {}", drive_estado);
        println!(
            "  Eventos sincronizados: {}  |  Tareas sincronizadas: {}",
            state.sync.mapa_eventos.len(),
            state.sync.mapa_tareas.len()
        );

        let toggle_auto = if state.sync.auto_sync {
            "🔄 Desactivar sync automático"
        } else {
            "🔄 Activar sync automático"
        };

        let opciones = &[
            "🔑 Subir datos via GitHub Gist (push)",
            "🔑 Descargar datos via GitHub Gist (pull)",
            "🔑 Buscar Gist existente (otro dispositivo)",
            "🔑 Configurar GitHub Gist (token)",
            toggle_auto,
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
            Some(4) => {
                // Toggle auto-sync
                if !state.sync.gist_configurado() {
                    println!("  {} Primero configura tu token de GitHub.", "✗".red());
                    pausa();
                } else {
                    state.sync.auto_sync = !state.sync.auto_sync;
                    if state.sync.auto_sync {
                        println!(
                            "  {} Sync automático {}",
                            "✓".green(),
                            "ACTIVADO".green().bold()
                        );
                        println!("    Tus datos se respaldarán en la nube automáticamente.");
                    } else {
                        println!("  ℹ Sync automático {}", "DESACTIVADO".yellow().bold());
                    }
                    pausa();
                }
            }
            Some(5) => {} // separador
            Some(6) => drive_push(state),
            Some(7) => drive_pull(state),
            Some(8) => drive_buscar(state),
            Some(9) => {} // separador
            Some(10) => exportar_ics(state),
            Some(11) => importar_ics(state),
            Some(12) => sync_push_google(state),
            Some(13) => sync_pull_google(state),
            Some(14) => resync_google(state),
            Some(15) => iniciar_dashboard_web(state),
            Some(16) => exportar_estado(state),
            Some(17) => importar_estado(state),
            Some(18) => enviar_resumen(state),
            Some(19) => enviar_recordatorio(state),
            Some(20) => enviar_followup_email(state),
            Some(21) => configurar_google(state),
            Some(22) => configurar_email(state),
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
    println!(
        "  4. En permisos, selecciona: {} → Read and Write",
        "Gists".bold()
    );
    println!("  5. Copia el token generado");
    println!();

    if state.sync.gist_configurado() {
        println!(
            "  {} Token actual: {}...{}",
            "✓".green(),
            &state.sync.gist_token[..4.min(state.sync.gist_token.len())],
            if state.sync.gist_token.len() > 8 {
                &state.sync.gist_token[state.sync.gist_token.len() - 4..]
            } else {
                ""
            }
        );
        if !state.sync.gist_id.is_empty() {
            println!("  📎 Gist ID: {}", state.sync.gist_id.cyan());
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
            println!(
                "  {} Token válido. Usuario: {}",
                "✓".green(),
                usuario.cyan()
            );
            state.sync.gist_token = token;
            state.sync.auto_sync = true; // Activar sync automático

            // Buscar si ya hay un gist existente
            println!("  Buscando Gist existente...");
            match sync::gist::gist_buscar(&state.sync.gist_token) {
                Ok(Some(id)) => {
                    println!("  {} Gist encontrado: {}", "✓".green(), id.cyan());
                    state.sync.gist_id = id;
                }
                Ok(None) => {
                    println!("  ℹ No hay Gist previo. Se creará uno al hacer push.");
                    state.sync.gist_id = String::new();
                }
                Err(e) => println!("  {} Error buscando: {}", "⚠".yellow(), e),
            }

            println!();
            println!(
                "  {} Sync automático {} — tus datos se respaldarán",
                "✓".green(),
                "ACTIVADO".green().bold()
            );
            println!("    en la nube cada vez que guardes.");

            // Hacer push inmediato para que haya datos en la nube desde ya
            println!();
            println!("  Subiendo datos a la nube...");
            let json = serde_json::to_string_pretty(&*state).unwrap_or_default();
            match sync::gist::gist_push(&state.sync, &json) {
                Ok(gist_id) => {
                    state.sync.gist_id = gist_id;
                    println!(
                        "  {} ¡Datos sincronizados! Ya puedes acceder desde otro dispositivo.",
                        "✓".green()
                    );
                    println!("  💡 En el otro dispositivo: configura el mismo token y haz 'pull'.");
                }
                Err(e) => {
                    println!("  {} Error subiendo: {}", "⚠".yellow(), e);
                    println!("  Se reintentará automáticamente en el próximo guardado.");
                }
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
        println!("  💡 Usa '🔑 Configurar GitHub Gist (token)'");
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
            println!(
                "  📦 {} tareas, {} eventos, {} diagramas, {} canvas, {} recuerdos",
                state.tasks.tareas.len(),
                state.agenda.eventos.len(),
                state.diagramas.len(),
                state.canvases.len(),
                state.memoria.recuerdos.len(),
            );
            println!("  🕐 {}", ahora);
            println!();
            println!("  💡 Para descargar en otro dispositivo:");
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
        println!("  💡 Usa 'Buscar Gist existente' o haz push primero.");
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

    println!("\n  📊 Datos remotos:");
    println!("    Tareas:    {}", remoto.tasks.tareas.len());
    println!("    Eventos:   {}", remoto.agenda.eventos.len());
    println!("    Canvases:  {}", remoto.canvases.len());
    println!("    Diagramas: {}", remoto.diagramas.len());
    println!("    Recuerdos: {}", remoto.memoria.recuerdos.len());

    println!("\n  📊 Datos locales:");
    println!("    Tareas:    {}", state.tasks.tareas.len());
    println!("    Eventos:   {}", state.agenda.eventos.len());
    println!("    Canvases:  {}", state.canvases.len());
    println!("    Diagramas: {}", state.diagramas.len());
    println!("    Recuerdos: {}", state.memoria.recuerdos.len());

    println!(
        "\n  {} Esto REEMPLAZARÁ todos tus datos locales.",
        "⚠".yellow()
    );

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
                println!(
                    "  {} Vinculado. Ya puedes hacer pull para descargar los datos.",
                    "✓".green()
                );
            }
        }
        Ok(None) => {
            println!(
                "  {} No se encontró ningún Gist de OmniPlanner.",
                "⚠".yellow()
            );
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
        println!(
            "  {} Primero autentica tu cuenta de Google (Configurar Google Calendar).",
            "✗".red()
        );
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
            println!("  🔄 Token expirado, refrescando...");
            match sync::calendario::google_refrescar_token(&mut state.sync) {
                Ok(()) => {
                    println!("  {} Token refrescado, reintentando...", "✓".green());
                    sync::drive::drive_push(&state.sync, &json)
                }
                Err(re) => {
                    println!("  {} No se pudo refrescar token: {}", "✗".red(), re);
                    println!("  💡 Re-autentica Google desde Configurar Google Calendar.");
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
            println!(
                "  📦 {} tareas, {} eventos, {} diagramas, {} canvas, {} recuerdos",
                state.tasks.tareas.len(),
                state.agenda.eventos.len(),
                state.diagramas.len(),
                state.canvases.len(),
                state.memoria.recuerdos.len(),
            );
            println!("  🕐 {}", ahora);
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
        println!("  💡 Usa 'Buscar archivo en Drive' si ya subiste datos desde otro dispositivo.");
        pausa();
        return;
    }

    println!("  Descargando de Google Drive...");

    let contenido = match sync::drive::drive_pull(&state.sync) {
        Ok(c) => c,
        Err(e) if e.contains("401") || e.contains("403") => {
            println!("  🔄 Token expirado, refrescando...");
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
                    println!("  💡 Re-autentica Google desde Configurar Google Calendar.");
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

    println!("\n  📊 Datos remotos:");
    println!("    Tareas:    {}", remoto.tasks.tareas.len());
    println!("    Eventos:   {}", remoto.agenda.eventos.len());
    println!("    Canvases:  {}", remoto.canvases.len());
    println!("    Diagramas: {}", remoto.diagramas.len());
    println!("    Recuerdos: {}", remoto.memoria.recuerdos.len());

    println!("\n  📊 Datos locales:");
    println!("    Tareas:    {}", state.tasks.tareas.len());
    println!("    Eventos:   {}", state.agenda.eventos.len());
    println!("    Canvases:  {}", state.canvases.len());
    println!("    Diagramas: {}", state.diagramas.len());
    println!("    Recuerdos: {}", state.memoria.recuerdos.len());

    println!(
        "\n  {} Esto REEMPLAZARÁ todos tus datos locales.",
        "⚠".yellow()
    );

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
            println!("  🔄 Token expirado, refrescando...");
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
                println!(
                    "  {} Vinculado. Ya puedes hacer pull para descargar los datos.",
                    "✓".green()
                );
            }
        }
        Ok(None) => {
            println!(
                "  {} No se encontró ningún archivo de OmniPlanner en Drive.",
                "⚠".yellow()
            );
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
    let sel = match menu("¿Qué exportar?", opciones) {
        Some(i) => i,
        None => return,
    };

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
    let archivo = match pedir_texto("Archivo de salida (ej: omniplanner.ics)") {
        Some(t) => t,
        None => return,
    };

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
    let archivo = match pedir_texto("Archivo .ics a importar") {
        Some(t) => t,
        None => return,
    };

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
        println!(
            "  {} Primero configura y autentica Google Calendar",
            "✗".red()
        );
        pausa();
        return;
    }

    separador("🔄 Re-sincronizar todo");
    println!("  Esto limpiará el registro de sincronización y enviará");
    println!("  todos los eventos y tareas de nuevo a Google Calendar.");
    println!();
    println!(
        "  {} eventos registrados, {} tareas registradas",
        state.sync.mapa_eventos.len(),
        state.sync.mapa_tareas.len()
    );

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

    let opciones = &[
        "Sincronizar eventos",
        "Sincronizar tareas",
        "Sincronizar todo",
    ];
    let sel = match menu("¿Qué sincronizar?", opciones) {
        Some(i) => i,
        None => return,
    };

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
        println!("  📅 Eventos: {} sincronizados, {} errores", ok, err);
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
        println!("  📋 Tareas: {} sincronizadas, {} errores", ok, err);
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

    let idx = match menu("¿De cuál tarea enviar recordatorio?", &refs) {
        Some(i) => i,
        None => return,
    };
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

    let idx = match menu("¿De cuál tarea?", &refs) {
        Some(i) => i,
        None => return,
    };
    let tarea = follow_ups[idx];
    let mensaje = match pedir_texto("Mensaje del follow-up") {
        Some(t) => t,
        None => return,
    };

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
    println!(
        "  {} Los datos se capturan al momento de iniciar.",
        "Nota:".yellow().bold()
    );
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
            println!(
                "  {}",
                "╔══════════════════════════════════════════════╗".green()
            );
            println!(
                "  {} Dashboard disponible en:                   {}",
                "║".green(),
                "║".green()
            );
            println!(
                "  {}   {}   {}",
                "║".green(),
                url.cyan().bold(),
                "║".green()
            );
            println!(
                "  {} Se refresca automáticamente cada 30 seg    {}",
                "║".green(),
                "║".green()
            );
            println!(
                "  {}",
                "╚══════════════════════════════════════════════╝".green()
            );
            println!();
            println!("  📡 También disponible:");
            println!("    {}  → Dashboard visual", format!("{}/", url).cyan());
            println!(
                "    {}  → Datos JSON (para apps)",
                format!("{}/api/state.json", url).cyan()
            );
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

    let nombre = match pedir_texto("Archivo de salida (ej: omniplanner_backup.json)") {
        Some(t) => t,
        None => return,
    };

    match serde_json::to_string_pretty(state) {
        Ok(json) => match std::fs::write(&nombre, &json) {
            Ok(_) => {
                let tamano = json.len() as f64 / 1024.0;
                println!(
                    "  {} Estado exportado a '{}' ({:.1} KB)",
                    "✓".green(),
                    nombre,
                    tamano
                );
                println!(
                    "  Contiene: {} tareas, {} eventos, {} diagramas, {} canvas, {} recuerdos",
                    state.tasks.tareas.len(),
                    state.agenda.eventos.len(),
                    state.diagramas.len(),
                    state.canvases.len(),
                    state.memoria.recuerdos.len(),
                );
                println!();
                println!("  💡 Para sincronizar con otro dispositivo:");
                println!("    1. Sube este archivo a Google Drive / OneDrive / Dropbox");
                println!("    2. En el otro dispositivo, descárgalo e impórtalo");
            }
            Err(e) => println!("  {} Error escribiendo archivo: {}", "✗".red(), e),
        },
        Err(e) => println!("  {} Error serializando: {}", "✗".red(), e),
    }
    pausa();
}

fn importar_estado(state: &mut AppState) {
    separador("💾 Importar estado completo");

    println!(
        "  {} Esto reemplazará TODOS tus datos actuales.",
        "⚠ ATENCIÓN:".red().bold()
    );
    println!("  Se recomienda exportar un backup antes de importar.");
    println!();

    let archivo = match pedir_texto("Archivo JSON a importar") {
        Some(t) => t,
        None => return,
    };

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
            println!(
                "  {} Error: el archivo no es un estado válido de OmniPlanner",
                "✗".red()
            );
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
            let ids_existentes: Vec<String> =
                state.agenda.eventos.iter().map(|e| e.id.clone()).collect();
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
            let ids_r: Vec<String> = state
                .memoria
                .recuerdos
                .iter()
                .map(|r| r.id.clone())
                .collect();
            for r in nuevo.memoria.recuerdos {
                if !ids_r.contains(&r.id) {
                    state.memoria.agregar_recuerdo(r);
                    recuerdos_nuevos += 1;
                }
            }

            println!("  {} Mezclado:", "✓".green());
            println!(
                "    +{} tareas, +{} eventos, +{} diagramas, +{} canvas, +{} recuerdos",
                tareas_nuevas, eventos_nuevos, diagramas_nuevos, canvas_nuevos, recuerdos_nuevos
            );
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

    let client_id = match pedir_texto("Client ID") {
        Some(t) => t,
        None => return,
    };
    let client_secret = match pedir_texto("Client Secret") {
        Some(t) => t,
        None => return,
    };
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

    println!("  ⏳ Esperando autorización en el navegador...");

    let codigo = match sync::calendario::escuchar_codigo_oauth() {
        Ok(c) => c,
        Err(e) => {
            println!("  {} Error capturando código: {}", "✗".red(), e);
            println!("  Intenta pegar el código manualmente:");

            match pedir_texto("Código de autorización") {
                Some(t) => t,
                None => return,
            }
        }
    };

    match sync::calendario::google_intercambiar_codigo(&mut state.sync, &codigo) {
        Ok(()) => println!("  {} Google Calendar conectado exitosamente", "✓".green()),
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
    let pi = match menu("Proveedor", presets) {
        Some(i) => i,
        None => return,
    };

    let server = match pi {
        0 => "smtp.gmail.com".to_string(),
        1 => "smtp.office365.com".to_string(),
        _ => match pedir_texto("Servidor SMTP") {
            Some(t) => t,
            None => return,
        },
    };

    let usuario = match pedir_texto("Usuario SMTP (email)") {
        Some(t) => t,
        None => return,
    };
    let password = match pedir_texto("Contraseña / App Password") {
        Some(t) => t,
        None => return,
    };
    let remitente = match pedir_texto("Email remitente (ej: Tu Nombre <tu@email.com>)") {
        Some(t) => t,
        None => return,
    };
    let destinatario = match pedir_texto("Email destinatario (para recibir notificaciones)") {
        Some(t) => t,
        None => return,
    };

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
                    let inicio =
                        fecha - Duration::days(fecha.weekday().num_days_from_monday() as i64);
                    let reporte = generar_reporte_semanal(state, inicio);
                    limpiar();
                    println!("{}", reporte);
                    pausa();
                }
            }
            Some(4) => {
                let tipos = &["Diario (hoy)", "Semanal (esta semana)"];
                let ti = match menu("Tipo de reporte", tipos) {
                    Some(i) => i,
                    None => continue,
                };
                let hoy = Local::now().date_naive();
                let reporte = if ti == 0 {
                    generar_reporte_diario(state, hoy)
                } else {
                    let inicio = hoy - Duration::days(hoy.weekday().num_days_from_monday() as i64);
                    generar_reporte_semanal(state, inicio)
                };
                let nombre = match pedir_texto("Nombre del archivo (ej: reporte.txt)") {
                    Some(t) => t,
                    None => continue,
                };
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

fn generar_reporte_diario(state: &AppState, fecha: NaiveDate) -> String {
    let dia = nombre_dia_es(fecha.weekday());
    let mes = nombre_mes_es(fecha.month());
    let mut r = String::new();

    r.push_str("╔══════════════════════════════════════════════════════════╗\n");
    r.push_str("║              OMNIPLANNER — REPORTE DIARIO               ║\n");
    r.push_str("╚══════════════════════════════════════════════════════════╝\n");
    r.push_str(&format!(
        "\n  Fecha: {} {} de {} de {}\n",
        dia,
        fecha.day(),
        mes,
        fecha.year()
    ));
    r.push_str(&format!(
        "  Generado: {}\n",
        Local::now().format("%d/%m/%Y %H:%M")
    ));
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
            r.push_str(&format!(
                "    {} {} - {} [{}]\n",
                icono,
                t.hora.format("%H:%M"),
                t.titulo,
                t.prioridad
            ));
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
            let fin = e
                .hora_fin
                .map(|h| format!(" - {}", h.format("%H:%M")))
                .unwrap_or_default();
            r.push_str(&format!(
                "    📌 {}{} {} ({})\n",
                e.hora_inicio.format("%H:%M"),
                fin,
                e.titulo,
                e.tipo
            ));
            if !e.descripcion.is_empty() {
                r.push_str(&format!("       {}\n", e.descripcion));
            }
        }
    }

    // Horarios de escritura
    let horarios = state.agenda.horarios_del_dia(fecha.weekday());
    if !horarios.is_empty() {
        r.push_str(&format!(
            "\n  ✏️  HORARIOS DE ESCRITURA ({})\n\n",
            horarios.len()
        ));
        for h in &horarios {
            r.push_str(&format!(
                "    🖊️  {} - {} {}\n",
                h.hora_inicio.format("%H:%M"),
                h.hora_fin.format("%H:%M"),
                h.descripcion
            ));
        }
    }

    // Follow-ups del día
    let follow_ups: Vec<_> = state
        .tasks
        .listar_follow_ups()
        .into_iter()
        .filter(|t| t.follow_up.map(|f| f.date() == fecha).unwrap_or(false))
        .collect();
    if !follow_ups.is_empty() {
        r.push_str(&format!("\n  🔔 FOLLOW-UPS ({})\n\n", follow_ups.len()));
        for t in &follow_ups {
            r.push_str(&format!(
                "    ↻ {} {} (tarea: {})\n",
                t.follow_up.unwrap().time().format("%H:%M"),
                t.titulo,
                t.estado
            ));
        }
    }

    // Tareas pendientes globales
    let pendientes = state.tasks.listar_pendientes();
    let otras_pendientes: Vec<_> = pendientes.iter().filter(|t| t.fecha != fecha).collect();
    if !otras_pendientes.is_empty() {
        r.push_str(&format!(
            "\n  ⏳ OTRAS TAREAS PENDIENTES ({})\n\n",
            otras_pendientes.len()
        ));
        for t in otras_pendientes.iter().take(10) {
            r.push_str(&format!(
                "    ⬜ {} {} - {} [{}]\n",
                t.fecha.format("%d/%m"),
                t.hora.format("%H:%M"),
                t.titulo,
                t.prioridad
            ));
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
    r.push_str(&format!(
        "\n  Semana: {} {} {} — {} {} {}\n",
        lunes.day(),
        mes_ini,
        lunes.year(),
        domingo.day(),
        mes_fin,
        domingo.year()
    ));
    r.push_str(&format!(
        "  Generado: {}\n",
        Local::now().format("%d/%m/%Y %H:%M")
    ));

    // Resumen total de la semana
    let mut total_tareas = 0;
    let mut total_completadas = 0;
    let mut total_eventos = 0;

    for i in 0..7 {
        let dia = lunes + Duration::days(i);
        let tareas = state.tasks.listar_por_fecha(dia);
        total_tareas += tareas.len();
        total_completadas += tareas
            .iter()
            .filter(|t| t.estado == TaskStatus::Completada)
            .count();
        total_eventos += state.agenda.eventos_del_dia(dia).len();
    }

    r.push_str(&format!(
        "\n  📊 RESUMEN: {} tareas ({} completadas), {} eventos\n",
        total_tareas, total_completadas, total_eventos
    ));

    // Día por día
    for i in 0..7 {
        let dia = lunes + Duration::days(i);
        let nombre = nombre_dia_es(dia.weekday());
        let mes = nombre_mes_es(dia.month());

        r.push_str("\n──────────────────────────────────────────────────────────\n");
        r.push_str(&format!(
            "  {} {} de {} de {}\n",
            nombre,
            dia.day(),
            mes,
            dia.year()
        ));

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
            r.push_str(&format!(
                "    {} {} {} [{}]\n",
                icono,
                t.hora.format("%H:%M"),
                t.titulo,
                t.prioridad
            ));
        }

        for e in &eventos {
            let fin = e
                .hora_fin
                .map(|h| format!("-{}", h.format("%H:%M")))
                .unwrap_or_default();
            r.push_str(&format!(
                "    📌 {}{} {} ({})\n",
                e.hora_inicio.format("%H:%M"),
                fin,
                e.titulo,
                e.tipo
            ));
        }

        for h in &horarios {
            r.push_str(&format!(
                "    🖊️  {}-{} {}\n",
                h.hora_inicio.format("%H:%M"),
                h.hora_fin.format("%H:%M"),
                h.descripcion
            ));
        }
    }

    // Follow-ups de la semana
    let follow_ups: Vec<_> = state
        .tasks
        .listar_follow_ups()
        .into_iter()
        .filter(|t| {
            t.follow_up
                .map(|f| {
                    let d = f.date();
                    d >= lunes && d <= domingo
                })
                .unwrap_or(false)
        })
        .collect();
    if !follow_ups.is_empty() {
        r.push_str("\n──────────────────────────────────────────────────────────\n");
        r.push_str(&format!(
            "  🔔 FOLLOW-UPS DE LA SEMANA ({})\n\n",
            follow_ups.len()
        ));
        for t in &follow_ups {
            let fu = t.follow_up.unwrap();
            r.push_str(&format!(
                "    ↻ {} {} — {}\n",
                fu.format("%d/%m %H:%M"),
                t.titulo,
                t.estado
            ));
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
        println!(
            "  {} modelos entrenados — {} datasets cargados",
            state.ml.modelos.len().to_string().green(),
            state.ml.datasets.len().to_string().green()
        );
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
    let nombres: Vec<String> = state
        .ml
        .datasets
        .iter()
        .map(|d| {
            format!(
                "{} ({} muestras, {} features, {} clases)",
                d.nombre,
                d.num_muestras(),
                d.num_features(),
                d.num_clases()
            )
        })
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
                    println!(
                        "  {} Dataset creado con datos aleatorios por clase",
                        "✓".green()
                    );
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

    let Some(ds_idx) = ml_elegir_dataset(state) else {
        return;
    };

    let mut ds = state.ml.datasets[ds_idx].clone();
    ds.normalizar();
    let (train, test) = ds.dividir(0.8, 42);

    let n_features = train.num_features();
    let n_clases = train.num_clases();

    println!();
    println!(
        "  📊 Train: {} muestras — Test: {} muestras",
        train.num_muestras(),
        test.num_muestras()
    );

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
    println!(
        "  {} Precisión train: {:.2}%",
        "✓".green(),
        prec_train * 100.0
    );
    println!(
        "  {} Precisión test:  {:.2}%",
        "✓".green(),
        prec_test * 100.0
    );

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

    let Some(ds_idx) = ml_elegir_dataset(state) else {
        return;
    };

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
        let y_train: Vec<f64> = train
            .etiquetas
            .iter()
            .map(|&e| if e == 1 { 1.0 } else { -1.0 })
            .collect();
        let y_test: Vec<f64> = test
            .etiquetas
            .iter()
            .map(|&e| if e == 1 { 1.0 } else { -1.0 })
            .collect();

        let mut svm = SVM::nuevo(n_features, c_param, lr);
        svm.entrenar(&train.features, &y_train, epocas);

        let prec_train = svm.precision(&train.features, &y_train);
        let prec_test = svm.precision(&test.features, &y_test);

        println!();
        svm.resumen();
        println!(
            "  {} Precisión train: {:.2}%",
            "✓".green(),
            prec_train * 100.0
        );
        println!(
            "  {} Precisión test:  {:.2}%",
            "✓".green(),
            prec_test * 100.0
        );

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
        println!(
            "  {} Precisión train: {:.2}%",
            "✓".green(),
            prec_train * 100.0
        );
        println!(
            "  {} Precisión test:  {:.2}%",
            "✓".green(),
            prec_test * 100.0
        );

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

    let Some(ds_idx) = ml_elegir_dataset(state) else {
        return;
    };

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
    println!(
        "  {} Precisión train: {:.2}%",
        "✓".green(),
        prec_train * 100.0
    );
    println!(
        "  {} Precisión test:  {:.2}%",
        "✓".green(),
        prec_test * 100.0
    );

    // Importancia de features
    let imp = arbol.importancia_features();
    if !imp.is_empty() {
        println!();
        println!("  Importancia de features (num. splits):");
        for (feat, cnt) in &imp {
            let nombre = ds
                .nombres_features
                .get(*feat)
                .map(|s| s.as_str())
                .unwrap_or("?");
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

    let Some(ds_idx) = ml_elegir_dataset(state) else {
        return;
    };

    let mut ds = state.ml.datasets[ds_idx].clone();
    ds.normalizar();
    let (train, test) = ds.dividir(0.8, 42);

    let n_features = train.num_features();
    let num_arboles = pedir_usize("Número de árboles", 50);
    let max_prof = pedir_usize("Profundidad máxima", 10);
    let max_feat = pedir_usize(
        &format!(
            "Max features por split (sqrt ~ {})",
            (n_features as f64).sqrt() as usize
        ),
        (n_features as f64).sqrt().ceil() as usize,
    );

    println!();
    separador("Entrenando Bosque Aleatorio...");

    let mut bosque = BosqueAleatorio::nuevo(num_arboles, max_prof, max_feat, train.num_clases());
    bosque.entrenar(&train.features, &train.etiquetas, 42);

    let prec_train = bosque.precision(&train.features, &train.etiquetas);
    let prec_test = bosque.precision(&test.features, &test.etiquetas);

    bosque.resumen();
    println!();
    println!(
        "  {} Precisión train: {:.2}%",
        "✓".green(),
        prec_train * 100.0
    );
    println!(
        "  {} Precisión test:  {:.2}%",
        "✓".green(),
        prec_test * 100.0
    );

    let imp = bosque.importancia_features();
    if !imp.is_empty() {
        println!();
        println!("  Top features (aggregated splits):");
        for (feat, cnt) in imp.iter().take(10) {
            let nombre = ds
                .nombres_features
                .get(*feat)
                .map(|s| s.as_str())
                .unwrap_or("?");
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

    let Some(ds_idx) = ml_elegir_dataset(state) else {
        return;
    };

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
    println!(
        "  {} Precisión train: {:.2}%",
        "✓".green(),
        prec_train * 100.0
    );
    println!(
        "  {} Precisión test:  {:.2}%",
        "✓".green(),
        prec_test * 100.0
    );

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

    let Some(ds_idx) = ml_elegir_dataset(state) else {
        return;
    };

    let mut ds = state.ml.datasets[ds_idx].clone();
    ds.normalizar();
    let (train, test) = ds.dividir(0.8, 42);

    let n_features = train.num_features();
    let n_clases = train.num_clases();

    if n_features < 4 {
        println!(
            "  {} La CNN necesita al menos 4 features para la convolución.",
            "⚠".yellow()
        );
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

    let mut cnn = CNN::nueva_1d(
        n_features,
        num_filtros,
        kernel,
        pool,
        &capas_densas,
        lr,
        n_clases,
        42,
    );
    cnn.resumen();
    println!();

    cnn.entrenar(&train.features, &train.etiquetas, epocas);

    let prec_train = cnn.precision(&train.features, &train.etiquetas);
    let prec_test = cnn.precision(&test.features, &test.etiquetas);

    println!();
    println!(
        "  {} Precisión train: {:.2}%",
        "✓".green(),
        prec_train * 100.0
    );
    println!(
        "  {} Precisión test:  {:.2}%",
        "✓".green(),
        prec_test * 100.0
    );

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
    let tipo = if tipo_idx == 1 {
        TipoRNN::LSTM
    } else {
        TipoRNN::Simple
    };

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

    println!(
        "  Train: {} secuencias — Test: {} secuencias",
        seq_train.len(),
        seq_test.len()
    );
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
        let err: f64 = pred
            .iter()
            .zip(obj)
            .map(|(p, t)| (p - t).powi(2))
            .sum::<f64>();
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
        println!(
            "    Objetivo: {:.3} → Predicción: {:.3}",
            obj_test[i][0], pred[0]
        );
    }

    let modelo = ModeloML {
        id: uuid::Uuid::new_v4().to_string(),
        nombre: "RNN — secuencias temporales".to_string(),
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
    println!(
        "  Probabilidades reales (ocultas): {:?}",
        probs
            .iter()
            .map(|p| format!("{:.2}", p))
            .collect::<Vec<_>>()
    );

    let mut bandit = MultiBandit::nuevo(probs.clone());

    println!();
    separador("Entrenando ε-greedy...");

    let historial = bandit.entrenar_epsilon_greedy(episodios, epsilon);

    println!();
    println!("  Resultados tras {} episodios:", episodios);
    for i in 0..n_brazos {
        let ratio = if bandit.conteos[i] > 0 {
            bandit.recompensas_acumuladas[i] / bandit.conteos[i] as f64
        } else {
            0.0
        };
        println!(
            "    Brazo {}: prob real={:.2}, tiradas={}, ratio ganancia={:.3}",
            i, probs[i], bandit.conteos[i], ratio
        );
    }

    let mejor_real = bandit.mejor_brazo();
    let mas_tirado = bandit
        .conteos
        .iter()
        .enumerate()
        .max_by_key(|(_, &c)| c)
        .map(|(i, _)| i)
        .unwrap_or(0);

    println!();
    println!(
        "  {} Mejor brazo real: {} — Más explotado: {}",
        if mejor_real == mas_tirado {
            "✓".green()
        } else {
            "⚠".yellow()
        },
        mejor_real,
        mas_tirado
    );

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

        println!(
            "  {}. {} [{}]",
            (i + 1).to_string().cyan(),
            m.nombre,
            tipo_str.yellow()
        );
        println!("     Creado: {}", m.creado);
        if let Some(pt) = m.precision_train {
            println!("     Precisión train: {:.2}%", pt * 100.0);
        }
        if let Some(pe) = m.precision_test {
            println!("     Precisión test:  {:.2}%", pe * 100.0);
        }
        println!();
    }

    let opciones = &[
        "🔍  Ver detalles de un modelo",
        "🗑️   Eliminar un modelo",
        "🔙  Volver",
    ];
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

// ══════════════════════════════════════════════════════════════
//  Menú NLP — Procesamiento de Lenguaje Natural
// ══════════════════════════════════════════════════════════════

fn menu_nlp(state: &mut AppState) {
    loop {
        limpiar();
        println!(
            "{}",
            "╔══════════════════════════════════════════════╗".cyan()
        );
        println!(
            "{}",
            "║     🗣️  Procesamiento de Lenguaje Natural    ║".cyan()
        );
        println!(
            "{}",
            "╚══════════════════════════════════════════════╝".cyan()
        );
        println!();

        state.nlp.motor.resumen();
        println!();

        let opciones = &[
            "💬  Chat (Conversación interactiva)",
            "😊  Analizar Sentimiento",
            "🎯  Clasificar Intención",
            "📚  Base de Conocimiento",
            "🔄  Historial de Conversaciones",
            "⭐  Sistema de Feedback",
            "🧠  Entrenar Modelos NLP",
            "📊  Embeddings de Palabras",
            "⚙️   Configuración NLP",
            "🔙  Volver",
        ];

        match menu("¿Qué quieres hacer?", opciones) {
            Some(0) => menu_nlp_chat(state),
            Some(1) => menu_nlp_sentimiento(state),
            Some(2) => menu_nlp_intencion(state),
            Some(3) => menu_nlp_conocimiento(state),
            Some(4) => menu_nlp_conversaciones(state),
            Some(5) => menu_nlp_feedback(state),
            Some(6) => menu_nlp_entrenar(state),
            Some(7) => menu_nlp_embeddings(state),
            Some(8) => menu_nlp_config(state),
            _ => return,
        }
    }
}

fn menu_nlp_chat(state: &mut AppState) {
    limpiar();
    println!("{}", "  💬 Chat Interactivo — NLP".cyan().bold());
    println!("  Escribe mensajes naturales. Escribe 'salir' para volver.\n");

    // Asegurar conversación activa
    if state.nlp.motor.conversaciones.conversacion_activa.is_none() {
        state.nlp.motor.nueva_conversacion();
    }

    loop {
        let input: String = match Input::new().with_prompt("  Tú").interact_text() {
            Ok(v) => v,
            Err(_) => return,
        };

        let input = input.trim().to_string();
        if input.is_empty() {
            continue;
        }
        if input == "salir" || input == "exit" || input == "quit" {
            return;
        }

        let resultado = state.nlp.motor.procesar(&input);

        // Mostrar respuesta
        println!("\n  🤖 {}", resultado.respuesta.green());

        // Info adicional
        println!(
            "  {} Intención: {} (confianza: {:.0}%)",
            "→".dimmed(),
            resultado.intencion.yellow(),
            resultado.confianza_intencion * 100.0
        );
        println!(
            "  {} Sentimiento: {} (score: {:.2})",
            "→".dimmed(),
            resultado.sentimiento.yellow(),
            resultado.score_sentimiento
        );

        if !resultado.entidades.is_empty() {
            let ents: Vec<String> = resultado
                .entidades
                .iter()
                .map(|(t, v)| format!("{}:{}", t, v))
                .collect();
            println!("  {} Entidades: {}", "→".dimmed(), ents.join(", "));
        }

        if let Some(fuente) = &resultado.fuente_conocimiento {
            println!("  {} Fuente: 📚 {}", "→".dimmed(), fuente);
        }

        if resultado.ambigua {
            println!("  {} ⚠️ Consulta ambigua detectada", "→".dimmed());
        }

        if !resultado.sugerencias.is_empty() {
            println!("  {} Sugerencias:", "→".dimmed());
            for s in &resultado.sugerencias {
                println!("    • {}", s.dimmed());
            }
        }

        // Pedir feedback rápido (opcional)
        println!();
        let fb_opciones = &[
            "👍 Buena respuesta",
            "👎 Mala respuesta",
            "Continuar sin valorar",
        ];
        if let Some(fb) = menu_compacto(fb_opciones) {
            match fb {
                0 => {
                    state.nlp.motor.registrar_feedback(
                        &input,
                        &resultado.respuesta,
                        Valoracion::Buena,
                        None,
                    );
                    println!("  {} Feedback registrado ✅", "→".dimmed());
                }
                1 => {
                    state.nlp.motor.registrar_feedback(
                        &input,
                        &resultado.respuesta,
                        Valoracion::Mala,
                        None,
                    );
                    println!("  {} Feedback registrado. Mejoraré. 📝", "→".dimmed());
                }
                _ => {}
            }
        }
        println!();
    }
}

fn menu_nlp_sentimiento(state: &mut AppState) {
    limpiar();
    println!("{}", "  😊 Análisis de Sentimiento".cyan().bold());
    println!("  Escribe textos para analizar. 'salir' para volver.\n");

    loop {
        let input: String = match Input::new().with_prompt("  Texto").interact_text() {
            Ok(v) => v,
            Err(_) => return,
        };

        if input.trim() == "salir" {
            return;
        }

        let res = state.nlp.motor.sentimiento.analizar(&input);

        println!("\n  ╭──────────────────────────────────────╮");
        println!(
            "  │ {} Polaridad: {} {}",
            res.polaridad.emoji(),
            res.polaridad.nombre(),
            " ".repeat(20 - res.polaridad.nombre().len())
        );
        println!(
            "  │ Score: {:.3} (confianza: {:.0}%)     ",
            res.score,
            res.confianza * 100.0
        );

        if !res.emociones.is_empty() {
            println!("  │ Emociones:");
            for (emo, val) in &res.emociones {
                let barra = "█".repeat((val * 10.0) as usize);
                println!("  │   {}: {} {:.2}", emo, barra, val);
            }
        }

        if !res.palabras_clave.is_empty() {
            println!("  │ Palabras clave:");
            for (p, s) in &res.palabras_clave {
                let signo = if *s > 0.0 { "+" } else { "" };
                println!("  │   '{}' → {}{:.2}", p, signo, s);
            }
        }
        println!("  ╰──────────────────────────────────────╯\n");
    }
}

fn menu_nlp_intencion(state: &mut AppState) {
    limpiar();
    println!("{}", "  🎯 Clasificación de Intención".cyan().bold());
    println!("  Escribe frases para clasificar. 'salir' para volver.\n");

    loop {
        let input: String = match Input::new().with_prompt("  Frase").interact_text() {
            Ok(v) => v,
            Err(_) => return,
        };

        if input.trim() == "salir" {
            return;
        }

        let intent = state.nlp.motor.intencion.clasificar(&input);
        let ambigua = state.nlp.motor.intencion.es_ambigua(&intent);

        println!(
            "\n  Intención: {} {}",
            intent.categoria.nombre().yellow().bold(),
            if ambigua {
                "⚠️ (ambigua)".to_string()
            } else {
                String::new()
            }
        );
        println!("  Confianza: {:.1}%", intent.confianza * 100.0);

        if !intent.alternativas.is_empty() {
            println!("  Alternativas:");
            for (cat, score) in &intent.alternativas {
                println!("    - {} ({:.1}%)", cat.nombre(), score * 100.0);
            }
        }

        if !intent.entidades.is_empty() {
            println!("  Entidades detectadas:");
            for ent in &intent.entidades {
                println!(
                    "    - {}: '{}' (pos: {})",
                    ent.tipo, ent.valor, ent.posicion
                );
            }
        }
        println!();
    }
}

fn menu_nlp_conocimiento(state: &mut AppState) {
    loop {
        limpiar();
        println!("{}", "  📚 Base de Conocimiento".cyan().bold());
        state.nlp.motor.conocimiento.resumen();
        println!();

        let opciones = &[
            "🔍  Buscar en la base",
            "➕  Agregar entrada",
            "🔗  Agregar relación",
            "📂  Ver por categoría",
            "🔙  Volver",
        ];

        match menu("Opción", opciones) {
            Some(0) => {
                let query: String = match Input::new().with_prompt("  Buscar").interact_text() {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let resultados = state.nlp.motor.conocimiento.buscar(&query, 5);
                if resultados.is_empty() {
                    println!("  No se encontraron resultados.");
                } else {
                    for (i, res) in resultados.iter().enumerate() {
                        println!(
                            "\n  {}. {} (relevancia: {:.2})",
                            i + 1,
                            res.entrada.titulo.yellow().bold(),
                            res.relevancia
                        );
                        println!(
                            "     Categoría: {} | Etiquetas: {}",
                            res.entrada.categoria,
                            res.entrada.etiquetas.join(", ")
                        );
                        let contenido = if res.entrada.contenido.len() > 120 {
                            format!("{}...", &res.entrada.contenido[..120])
                        } else {
                            res.entrada.contenido.clone()
                        };
                        println!("     {}", contenido.dimmed());
                        println!("     Razón: {}", res.razon.dimmed());
                    }
                }
                pausa();
            }
            Some(1) => {
                let titulo: String = match Input::new().with_prompt("  Título").interact_text() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let contenido: String =
                    match Input::new().with_prompt("  Contenido").interact_text() {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                let categoria: String =
                    match Input::new().with_prompt("  Categoría").interact_text() {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                let etiquetas_str: String = match Input::new()
                    .with_prompt("  Etiquetas (separadas por coma)")
                    .interact_text()
                {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let etiquetas: Vec<String> = etiquetas_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                let id = state
                    .nlp
                    .motor
                    .conocimiento
                    .agregar(&titulo, &contenido, &categoria, &etiquetas);
                println!("  {} Entrada creada: {}", "✓".green(), id);
                pausa();
            }
            Some(2) => {
                let entradas = &state.nlp.motor.conocimiento.entradas;
                if entradas.len() < 2 {
                    println!("  Necesitas al menos 2 entradas para crear relaciones.");
                    pausa();
                    continue;
                }
                let nombres: Vec<String> = entradas
                    .iter()
                    .map(|e| format!("{} ({})", e.titulo, e.id))
                    .collect();
                let nombres_ref: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

                println!("  Selecciona origen:");
                let origen_idx = match menu("Origen", &nombres_ref) {
                    Some(i) => i,
                    None => continue,
                };
                println!("  Selecciona destino:");
                let destino_idx = match menu("Destino", &nombres_ref) {
                    Some(i) => i,
                    None => continue,
                };

                let tipos_rel = &[
                    "Es un",
                    "Tiene parte",
                    "Relacionado",
                    "Sinónimo",
                    "Antónimo",
                    "Ejemplo",
                    "Causa",
                    "Prerequisito",
                ];
                let tipo_idx = match menu("Tipo de relación", tipos_rel) {
                    Some(i) => i,
                    None => continue,
                };
                let tipo = match tipo_idx {
                    0 => TipoRelacion::EsUn,
                    1 => TipoRelacion::TieneParte,
                    2 => TipoRelacion::Relacionado,
                    3 => TipoRelacion::Sinonimo,
                    4 => TipoRelacion::Antonimo,
                    5 => TipoRelacion::Ejemplo,
                    6 => TipoRelacion::Causa,
                    _ => TipoRelacion::Prerequisito,
                };

                let origen_id = entradas[origen_idx].id.clone();
                let destino_id = entradas[destino_idx].id.clone();
                state
                    .nlp
                    .motor
                    .conocimiento
                    .agregar_relacion(&origen_id, &destino_id, tipo, 0.8);
                println!("  {} Relación creada", "✓".green());
                pausa();
            }
            Some(3) => {
                let cats = state.nlp.motor.conocimiento.categorias.clone();
                if cats.is_empty() {
                    println!("  No hay categorías.");
                    pausa();
                    continue;
                }
                let cats_ref: Vec<&str> = cats.iter().map(|s| s.as_str()).collect();
                if let Some(idx) = menu("Categoría", &cats_ref) {
                    let entradas = state
                        .nlp
                        .motor
                        .conocimiento
                        .buscar_por_categoria(&cats[idx]);
                    println!(
                        "\n  Categoría: {} ({} entradas)",
                        cats[idx].yellow(),
                        entradas.len()
                    );
                    for e in entradas {
                        println!(
                            "    • {} — {}",
                            e.titulo.bold(),
                            &e.contenido[..e.contenido.len().min(60)]
                        );
                    }
                    pausa();
                }
            }
            _ => return,
        }
    }
}

fn menu_nlp_conversaciones(state: &mut AppState) {
    limpiar();
    println!("{}", "  🔄 Historial de Conversaciones".cyan().bold());

    let convs = &state.nlp.motor.conversaciones.conversaciones;
    if convs.is_empty() {
        println!("\n  No hay conversaciones registradas.");
        pausa();
        return;
    }

    for conv in convs {
        conv.resumen();
        println!();

        let ultimos = conv.ultimos_turnos(6);
        for turno in ultimos {
            let icono = match turno.rol {
                omniplanner::nlp::Rol::Usuario => "👤",
                omniplanner::nlp::Rol::Sistema => "🤖",
            };
            let texto = if turno.texto.len() > 80 {
                format!("{}...", &turno.texto[..80])
            } else {
                turno.texto.clone()
            };
            println!("    {} [{}] {}", icono, turno.timestamp, texto);
        }
        println!();
    }
    pausa();
}

fn menu_nlp_feedback(state: &mut AppState) {
    limpiar();
    println!("{}", "  ⭐ Sistema de Feedback".cyan().bold());

    state.nlp.motor.feedback.resumen();
    println!();

    if !state.nlp.motor.feedback.feedbacks.is_empty() {
        println!("  Últimos feedbacks:");
        let total = state.nlp.motor.feedback.feedbacks.len();
        let inicio = total.saturating_sub(5);
        for fb in &state.nlp.motor.feedback.feedbacks[inicio..] {
            println!(
                "    #{} [{}] {} — '{}' → '{}'",
                fb.id,
                fb.timestamp,
                fb.valoracion.nombre(),
                if fb.consulta_original.len() > 30 {
                    &fb.consulta_original[..30]
                } else {
                    &fb.consulta_original
                },
                if fb.respuesta_dada.len() > 30 {
                    &fb.respuesta_dada[..30]
                } else {
                    &fb.respuesta_dada
                },
            );
        }
    }
    pausa();
}

fn menu_nlp_entrenar(state: &mut AppState) {
    limpiar();
    println!("{}", "  🧠 Entrenar Modelos NLP".cyan().bold());
    println!();

    let opciones = &[
        "🎯  Entrenar todo (sentimiento + intención)",
        "😊  Solo sentimiento",
        "🎯  Solo intención",
        "📊  Ver datos de entrenamiento",
        "🔙  Volver",
    ];

    match menu("¿Qué entrenar?", opciones) {
        Some(0) => {
            println!("\n  Entrenando todos los modelos NLP...\n");
            state.nlp.motor.entrenar_completo();
            println!("\n  {} Modelos entrenados exitosamente", "✓".green());
            pausa();
        }
        Some(1) => {
            println!("\n  Entrenando análisis de sentimiento...\n");
            let datos = DatosEntrenamiento::sentimiento_es();
            let epocas = pedir_usize("  Épocas", 100);
            let lr = pedir_f64("  Learning rate", 0.05);
            state.nlp.motor.entrenar_sentimiento(&datos, epocas, lr);
            println!("\n  {} Modelo de sentimiento entrenado", "✓".green());
            pausa();
        }
        Some(2) => {
            println!("\n  Entrenando clasificador de intención...\n");
            let datos = DatosEntrenamiento::intenciones_es();
            let epocas = pedir_usize("  Épocas", 100);
            let lr = pedir_f64("  Learning rate", 0.1);
            state.nlp.motor.entrenar_intencion(&datos, epocas, lr);
            println!("\n  {} Clasificador de intención entrenado", "✓".green());
            pausa();
        }
        Some(3) => {
            println!("\n  📊 Datos de entrenamiento disponibles:\n");
            let sent = DatosEntrenamiento::sentimiento_es();
            println!("  Sentimiento: {} ejemplos", sent.len());
            for (texto, score) in sent.iter().take(5) {
                println!("    [{:+.1}] '{}'", score, texto);
            }
            println!("    ... y {} más\n", sent.len().saturating_sub(5));

            let intent = DatosEntrenamiento::intenciones_es();
            println!("  Intención: {} ejemplos", intent.len());
            for (texto, cat) in intent.iter().take(5) {
                println!("    [{}] '{}'", cat.nombre(), texto);
            }
            println!("    ... y {} más", intent.len().saturating_sub(5));
            pausa();
        }
        _ => {}
    }
}

fn menu_nlp_embeddings(state: &mut AppState) {
    limpiar();
    println!("{}", "  📊 Word Embeddings".cyan().bold());
    println!();

    let _state = state; // prevent unused warning
    let opciones = &[
        "🎓  Entrenar embeddings con textos",
        "🔍  Buscar palabras similares",
        "📐  Vector de una palabra",
        "🔙  Volver",
    ];

    match menu("Opción", opciones) {
        Some(0) => {
            println!(
                "\n  Ingresa textos de entrenamiento (uno por línea, línea vacía para terminar):\n"
            );
            let mut corpus = Vec::new();
            loop {
                let linea: String = match Input::new()
                    .with_prompt("  Texto")
                    .allow_empty(true)
                    .interact_text()
                {
                    Ok(v) => v,
                    Err(_) => break,
                };
                if linea.is_empty() {
                    break;
                }
                corpus.push(linea);
            }

            if corpus.is_empty() {
                corpus = vec![
                    "el gato come pescado fresco".to_string(),
                    "el perro come carne de res".to_string(),
                    "el gato duerme en la casa".to_string(),
                    "el perro corre en el parque".to_string(),
                    "la tarea esta pendiente de revision".to_string(),
                    "crear una nueva tarea urgente".to_string(),
                    "completar el proyecto de rust".to_string(),
                    "programar reunion para mañana temprano".to_string(),
                    "el codigo tiene errores de compilacion".to_string(),
                    "el programa funciona correctamente ahora".to_string(),
                ];
                println!("  Usando corpus por defecto ({} textos)", corpus.len());
            }

            let dim = pedir_usize("  Dimensión de vectores", 20);
            let epocas = pedir_usize("  Épocas", 10);
            let lr = pedir_f64("  Learning rate", 0.05);

            println!("\n  Entrenando embeddings...\n");
            let mut emb = omniplanner::nlp::WordEmbeddings::nuevo(dim);
            let corpus_ref: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
            emb.entrenar(&corpus_ref, 3, epocas, lr);

            println!(
                "\n  {} Embeddings entrenados. Vocabulario: {} palabras",
                "✓".green(),
                emb.vocab_index.len()
            );

            println!("\n  Palabras similares a 'gato':");
            for (p, sim) in emb.mas_similares("gato", 5) {
                println!("    {}: {:.3}", p, sim);
            }
            pausa();
        }
        Some(1) => {
            println!("  (Primero entrena embeddings con la opción anterior)");
            pausa();
        }
        Some(2) => {
            println!("  (Primero entrena embeddings con la opción anterior)");
            pausa();
        }
        _ => {}
    }
}

fn menu_nlp_config(state: &mut AppState) {
    limpiar();
    println!("{}", "  ⚙️  Configuración NLP".cyan().bold());
    println!();

    println!("  Configuración actual:");
    println!("  ─────────────────────");
    println!(
        "    Umbral de confianza: {:.2}",
        state.nlp.motor.config.umbral_confianza
    );
    println!(
        "    Max resultados KB: {}",
        state.nlp.motor.config.max_resultados_kb
    );
    println!(
        "    Usar sentimiento: {}",
        state.nlp.motor.config.usar_sentimiento
    );
    println!(
        "    Usar conocimiento: {}",
        state.nlp.motor.config.usar_conocimiento
    );
    println!(
        "    Usar feedback: {}",
        state.nlp.motor.config.usar_feedback
    );
    println!("    Idioma: {}", state.nlp.motor.config.idioma_preferido);
    println!();

    let opciones = &[
        "Cambiar umbral de confianza",
        "Toggle sentimiento",
        "Toggle base de conocimiento",
        "Toggle feedback",
        "Cambiar idioma (es/en)",
        "Restaurar valores por defecto",
        "Volver",
    ];

    match menu("Ajustar", opciones) {
        Some(0) => {
            state.nlp.motor.config.umbral_confianza = pedir_f64("  Nuevo umbral (0.0-1.0)", 0.35);
            println!("  {} Actualizado", "✓".green());
            pausa();
        }
        Some(1) => {
            state.nlp.motor.config.usar_sentimiento = !state.nlp.motor.config.usar_sentimiento;
            println!(
                "  {} Sentimiento: {}",
                "✓".green(),
                state.nlp.motor.config.usar_sentimiento
            );
            pausa();
        }
        Some(2) => {
            state.nlp.motor.config.usar_conocimiento = !state.nlp.motor.config.usar_conocimiento;
            println!(
                "  {} Conocimiento: {}",
                "✓".green(),
                state.nlp.motor.config.usar_conocimiento
            );
            pausa();
        }
        Some(3) => {
            state.nlp.motor.config.usar_feedback = !state.nlp.motor.config.usar_feedback;
            println!(
                "  {} Feedback: {}",
                "✓".green(),
                state.nlp.motor.config.usar_feedback
            );
            pausa();
        }
        Some(4) => {
            let idiomas = &["es (Español)", "en (English)"];
            if let Some(i) = menu("Idioma", idiomas) {
                state.nlp.motor.config.idioma_preferido = if i == 0 {
                    "es".to_string()
                } else {
                    "en".to_string()
                };
                println!(
                    "  {} Idioma: {}",
                    "✓".green(),
                    state.nlp.motor.config.idioma_preferido
                );
            }
            pausa();
        }
        Some(5) => {
            state.nlp.motor.config = omniplanner::nlp::ConfigNLP::default();
            println!("  {} Configuración restaurada", "✓".green());
            pausa();
        }
        _ => {}
    }
}

/// Menú compacto inline (sin limpiar pantalla)
fn menu_compacto(opciones: &[&str]) -> Option<usize> {
    Select::new()
        .items(opciones)
        .default(0)
        .interact_opt()
        .ok()
        .flatten()
}

// ══════════════════════════════════════════════════════════════
//  Menú ASESOR INTELIGENTE — Decisiones prácticas y finanzas
// ══════════════════════════════════════════════════════════════

fn menu_asesor(state: &mut AppState) {
    loop {
        limpiar();
        println!(
            "{}",
            "╔══════════════════════════════════════════════╗".cyan()
        );
        println!(
            "{}",
            "║    💡 A S E S O R   I N T E L I G E N T E   ║"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "║   Decisiones financieras y productivas       ║".cyan()
        );
        println!(
            "{}",
            "╚══════════════════════════════════════════════╝".cyan()
        );
        println!();

        // Resumen rápido del estado financiero
        let balance = state.asesor.presupuesto.balance_mensual();
        let n_deudas = state.asesor.analisis_deudas.len();
        let n_decisiones = state.asesor.matrices.len();
        let n_acciones = state.asesor.diccionario.acciones.len();
        let n_registros = state.asesor.registros.len();
        let n_rastreadas = state.asesor.rastreador.deudas.len();

        if state.asesor.presupuesto.ingresos.is_empty()
            && state.asesor.presupuesto.gastos.is_empty()
        {
            println!(
                "  {}",
                "📌 Tip: Configura tu presupuesto para obtener proyecciones reales.".yellow()
            );
        } else {
            let color_balance = if balance >= 0.0 {
                format!("${:.2}", balance).green()
            } else {
                format!("-${:.2}", balance.abs()).red()
            };
            println!("  💰 Balance mensual: {}", color_balance);
        }
        if n_deudas > 0 {
            println!("  📊 {} deudas analizadas", n_deudas);
        }
        if n_decisiones > 0 {
            println!("  🧮 {} matrices de decisión", n_decisiones);
        }
        if n_acciones > 0 {
            println!("  📝 {} acciones registradas", n_acciones);
        }
        if n_registros > 0 {
            println!("  📂 {} registros guardados", n_registros);
        }
        if n_rastreadas > 0 {
            let deuda_total = state.asesor.rastreador.deuda_total_actual();
            println!(
                "  🔎 {} deudas rastreadas — Total: {}",
                n_rastreadas,
                format!("${:.2}", deuda_total).red()
            );
        }
        println!();

        let opciones = &[
            "💳  Analizar deuda / tarjeta de crédito",
            "🏦  Ingresar corte bancario (calcular tasa real)",
            "📊  Presupuesto Base Cero (cada dólar asignado)",
            "⚖️   Comparación rápida (A vs B)",
            "🧮  Matriz de decisión (multi-criterio)",
            "💰  Presupuesto mensual (simple)",
            "📈  Proyecciones de ahorro",
            "📝  Registrar acción / decisión",
            "�  Rastreador de Deudas (multi-cuenta + diagnóstico)",
            "📂  Historial y Exportación",
            "🔙  Volver",
        ];

        match menu("¿Qué necesitas analizar?", opciones) {
            Some(0) => menu_asesor_deuda(state),
            Some(1) => menu_asesor_corte_bancario(state),
            Some(2) => menu_presupuesto_cero(state),
            Some(3) => menu_asesor_comparacion(state),
            Some(4) => menu_asesor_matriz(state),
            Some(5) => menu_asesor_presupuesto(state),
            Some(6) => menu_asesor_proyecciones(state),
            Some(7) => menu_asesor_registrar_accion(state),
            Some(8) => menu_asesor_rastreador(state),
            Some(9) => menu_asesor_historial(state),
            _ => return,
        }
    }
}

// ── Análisis de deuda ──

fn menu_asesor_deuda(state: &mut AppState) {
    limpiar();
    separador("💳 ANÁLISIS DE DEUDA");

    println!("  Ingresa los datos de tu deuda:\n");

    let nombre = match pedir_texto("Nombre (ej: Tarjeta Visa, Préstamo)") {
        Some(n) => n,
        None => return,
    };
    let saldo = pedir_f64("Saldo total ($)", 0.0);
    if saldo <= 0.0 {
        println!("  {} El saldo debe ser mayor a 0", "✗".red());
        pausa();
        return;
    }
    let tasa_anual = pedir_f64("Tasa de interés ANUAL (%, ej: 36)", 36.0);
    let tasa_mensual = tasa_anual / 100.0 / 12.0;
    let pago_min = pedir_f64("Pago mínimo mensual ($)", saldo * 0.05);

    let deuda = AnalisisDeuda::nuevo(&nombre, saldo, tasa_mensual, pago_min);

    // Preguntar opciones de pago
    println!();
    println!("  💡 Ahora define las opciones de pago a comparar.");
    println!(
        "  {} Opción 1 ya es el pago mínimo (${:.2})",
        "→".dimmed(),
        pago_min
    );
    println!();

    let mut montos: Vec<(String, f64)> =
        vec![(format!("Pago mínimo (${:.0})", pago_min), pago_min)];

    // Pedir montos adicionales
    for i in 2..=5 {
        let prompt = format!("Monto opción {} (0=terminar)", i);
        let monto = pedir_f64(&prompt, 0.0);
        if monto <= 0.0 {
            break;
        }
        montos.push((format!("Pago ${:.0}", monto), monto));
    }

    // Agregar opción de pago total si no está
    if !montos.iter().any(|(_, m)| (*m - saldo).abs() < 0.01) {
        montos.push((format!("Pago total (${:.0})", saldo), saldo));
    }

    let montos_ref: Vec<(&str, f64)> = montos.iter().map(|(n, m)| (n.as_str(), *m)).collect();
    let opciones = deuda.comparar_opciones(&montos_ref);

    // Mostrar tabla comparativa
    println!();
    println!(
        "{}",
        "  ╔═══════════════════════════════════════════════════════════════════════════════╗"
            .cyan()
    );
    println!(
        "  ║  {} — Saldo: ${:.2} — Tasa: {:.1}% anual",
        nombre.bold(),
        saldo,
        tasa_anual
    );
    println!(
        "{}",
        "  ╠═══════════════════════════════════════════════════════════════════════════════╣"
            .cyan()
    );
    println!(
        "  ║  {:<25} {:>8} {:>12} {:>12} {:>12}",
        "Opción", "Meses", "Intereses", "Total pagado", "Ahorro"
    );
    println!(
        "{}",
        "  ╠═══════════════════════════════════════════════════════════════════════════════╣"
            .cyan()
    );

    let mejor = AnalisisDeuda::mejor_opcion(&opciones);

    for (i, op) in opciones.iter().enumerate() {
        let marca = if Some(i) == mejor { " ⭐" } else { "" };
        let ahorro_str = if op.ahorro_vs_minimo > 0.01 {
            format!("${:.2}", op.ahorro_vs_minimo).green().to_string()
        } else {
            "—".dimmed().to_string()
        };
        println!(
            "  ║  {:<25} {:>6}m   ${:>10.2}   ${:>10.2}   {:>10}{}",
            op.nombre,
            op.meses_para_liquidar,
            op.total_intereses,
            op.total_pagado,
            ahorro_str,
            marca
        );
    }

    println!(
        "{}",
        "  ╚═══════════════════════════════════════════════════════════════════════════════╝"
            .cyan()
    );

    if let Some(idx) = mejor {
        println!();
        println!(
            "  ⭐ La mejor opción es: {}",
            opciones[idx].nombre.green().bold()
        );
        if opciones[idx].ahorro_vs_minimo > 0.01 {
            println!(
                "  💰 Te ahorras ${:.2} en intereses vs. pago mínimo",
                opciones[idx].ahorro_vs_minimo
            );
        }
    }

    // Preguntar si quiere ver proyección detallada
    println!();
    if Confirm::new()
        .with_prompt("  ¿Ver proyección mes a mes de alguna opción?")
        .default(true)
        .interact()
        .unwrap_or(false)
    {
        let nombres_op: Vec<String> = opciones.iter().map(|o| o.nombre.clone()).collect();
        let refs: Vec<&str> = nombres_op.iter().map(|s| s.as_str()).collect();
        if let Some(idx) = menu("¿Cuál opción?", &refs) {
            let proy = deuda.proyeccion_mensual(opciones[idx].monto_mensual, 60);
            println!();
            println!(
                "  {:<5} {:>10} {:>10} {:>12} {:>12} {:>14}",
                "Mes", "Pago", "Interés", "A capital", "Saldo", "Int. acum."
            );
            println!("  {}", "─".repeat(65));
            for f in &proy {
                println!(
                    "  {:<5} ${:>8.2} ${:>8.2} ${:>10.2} ${:>10.2} ${:>12.2}",
                    f.mes,
                    f.pago,
                    f.interes,
                    f.abono_capital,
                    f.saldo_restante,
                    f.intereses_acumulados
                );
            }
            println!("  {}", "─".repeat(65));
        }
    }

    // Guardar análisis
    state.asesor.analisis_deudas.push(deuda.clone());

    // Registro automático
    let mejor_nombre = mejor.map(|i| opciones[i].nombre.clone());
    let resumen_reg = format!(
        "Saldo ${:.2}, tasa {:.1}%/año, mejor: {}",
        saldo,
        tasa_anual,
        mejor_nombre.as_deref().unwrap_or("N/A")
    );
    let id = state.asesor.siguiente_id();
    let hoy = Local::now().format("%Y-%m-%d").to_string();
    let hora = Local::now().format("%H:%M").to_string();
    let reg = RegistroAsesor::nuevo(
        id,
        &hoy,
        &hora,
        &nombre,
        &resumen_reg,
        vec!["deuda".into(), "finanzas".into(), nombre.clone()],
        TipoRegistro::AnalisisDeuda {
            deuda,
            opciones: opciones.clone(),
            mejor_opcion: mejor_nombre,
        },
    );
    state.asesor.registros.push(reg);

    // Registrar en memoria
    let contenido = format!(
        "Análisis de deuda: {} — Saldo ${:.2} — Mejor opción: {}",
        nombre,
        saldo,
        mejor
            .map(|i| opciones[i].nombre.clone())
            .unwrap_or_default()
    );
    let recuerdo = omniplanner::memoria::Recuerdo::new(
        contenido,
        vec!["deuda".into(), "finanzas".into(), nombre.clone()],
    )
    .con_origen("asesor", &nombre);
    state.memoria.agregar_recuerdo(recuerdo);

    pausa();
}

// ── Corte bancario — datos reales del estado de cuenta ──

fn menu_asesor_corte_bancario(state: &mut AppState) {
    limpiar();
    separador("🏦 CORTE BANCARIO — DATOS REALES");

    println!("  📄 Ingresa los datos tal como aparecen en tu estado de cuenta.");
    println!("  💡 Solo necesitas los montos, el sistema calcula la tasa de interés.");
    println!();

    let nombre = match pedir_texto("Nombre de la tarjeta (ej: Visa Banco X)") {
        Some(n) => n,
        None => return,
    };

    let mut corte = CorteBancario::nuevo(&nombre);
    corte.fecha_corte = pedir_texto_opcional("Fecha de corte (ej: 2026-04-01)");

    println!();
    println!("  📋 Datos del estado de cuenta:");
    corte.saldo_anterior = pedir_f64("Saldo anterior (período pasado) $", 0.0);
    corte.pago_realizado = pedir_f64("Pago(s) realizado(s) en el período $", 0.0);
    corte.compras_periodo = pedir_f64("Nuevas compras / cargos en el período $", 0.0);
    corte.intereses_cobrados = pedir_f64("Intereses cobrados (aparece en el corte) $", 0.0);
    corte.otros_cargos = pedir_f64(
        "Otros cargos (comisiones, seguros, IVA interés, etc.) $",
        0.0,
    );
    corte.saldo_al_corte = pedir_f64("Saldo al corte (nuevo saldo total) $", 0.0);

    println!();
    println!("  💰 Datos de pago:");
    corte.pago_minimo = pedir_f64("Pago mínimo que indica el banco $", 0.0);
    corte.pago_no_intereses = pedir_f64("Pago para no generar intereses $", corte.saldo_al_corte);

    // Analizar
    let analisis = corte.analizar();
    let est = &analisis.estrategia;

    // ═══════════════════════════════════════════════════════
    //  SECCIÓN 1: Resumen del corte
    // ═══════════════════════════════════════════════════════
    println!();
    println!(
        "{}",
        "  ╔═══════════════════════════════════════════════════════════════════╗".cyan()
    );
    println!("  ║  {} — Corte: {}", nombre.bold(), corte.fecha_corte);
    println!(
        "{}",
        "  ╠═══════════════════════════════════════════════════════════════════╣".cyan()
    );
    println!("  ║");
    println!("  ║  📋 Movimientos del período:");
    println!(
        "  ║    Saldo anterior:        ${:>10.2}",
        corte.saldo_anterior
    );
    println!(
        "  ║    Pago realizado:       -${:>10.2}",
        corte.pago_realizado
    );
    if corte.compras_periodo > 0.0 {
        println!(
            "  ║    Nuevas compras:       +${:>10.2}",
            corte.compras_periodo
        );
    }
    println!(
        "  ║    Intereses cobrados:   +${:>10.2}  {}",
        corte.intereses_cobrados,
        "← esto es lo que te cobra el banco".dimmed()
    );
    if corte.otros_cargos > 0.0 {
        println!(
            "  ║    Otros cargos:         +${:>10.2}",
            corte.otros_cargos
        );
    }
    println!("  ║    ─────────────────────────────────");
    println!(
        "  ║    Saldo al corte:        ${:>10.2}",
        corte.saldo_al_corte
    );
    println!("  ║");

    if analisis.diferencia_vs_real > 0.01 {
        println!(
            "  ║  ⚠️ Los números no cuadran (dif: ${:.2}) — puede haber cargos no listados",
            analisis.diferencia_vs_real
        );
        println!("  ║");
    }

    // ═══════════════════════════════════════════════════════
    //  SECCIÓN 2: Diagnóstico — ¿te están cobrando intereses?
    // ═══════════════════════════════════════════════════════
    println!(
        "{}",
        "  ╠═══════════════════════════════════════════════════════════════════╣".cyan()
    );
    println!("  ║");
    if est.tiene_intereses {
        println!(
            "  ║  {} {}",
            "🔴 SÍ TE ESTÁN COBRANDO INTERESES".red().bold(),
            format!("(${:.2} este mes)", corte.intereses_cobrados).red()
        );
        println!(
            "  ║     Tasa mensual: {}  →  Tasa anual: {}",
            format!("{:.2}%", analisis.tasa_mensual_calculada * 100.0)
                .yellow()
                .bold(),
            format!("{:.1}%", analisis.tasa_anual_calculada * 100.0)
                .yellow()
                .bold()
        );
        println!(
            "  ║     Saldo que generó interés: ${:.2}",
            analisis.saldo_que_genero_interes
        );
    } else {
        println!(
            "  ║  {} No te cobraron intereses este corte",
            "🟢 SIN INTERESES".green().bold()
        );
    }
    println!("  ║");

    // ═══════════════════════════════════════════════════════
    //  SECCIÓN 3: Tu pago — a dónde se fue
    // ═══════════════════════════════════════════════════════
    if corte.pago_realizado > 0.0 && est.tiene_intereses {
        println!(
            "{}",
            "  ╠═══════════════════════════════════════════════════════════════════╣".cyan()
        );
        println!("  ║");
        println!("  ║  💰 TU PAGO DE ${:.2}:", corte.pago_realizado);
        println!(
            "  ║    ✅ A reducir deuda:  ${:>10.2}  ({})",
            analisis.pago_a_capital,
            format!("{:.0}%", 100.0 - analisis.pct_pago_a_interes).green()
        );
        println!(
            "  ║    🔴 Al banco (interés):${:>10.2}  ({})",
            analisis.pago_a_interes,
            format!("{:.0}%", analisis.pct_pago_a_interes).red()
        );
        if !est.pago_cubre_intereses {
            println!("  ║");
            println!("  ║  ⛔ Tu pago NI SIQUIERA cubre los intereses.");
            println!(
                "  ║     {}",
                "La deuda está CRECIENDO cada mes aunque pagues."
                    .red()
                    .bold()
            );
        }
        println!("  ║");
    }

    // ═══════════════════════════════════════════════════════
    //  SECCIÓN 4: ESTRATEGIA — cómo eliminar intereses
    // ═══════════════════════════════════════════════════════
    println!(
        "{}",
        "  ╠═══════════════════════════════════════════════════════════════════╣".cyan()
    );
    println!("  ║");
    println!("  ║  🎯 ESTRATEGIA PARA ELIMINAR INTERESES:");
    println!("  ║");

    if est.tiene_intereses {
        println!("  ║  → Para CORTAR los intereses de raíz este mes, necesitas pagar:");
        println!(
            "  ║     {}",
            format!("${:.2}", est.monto_corta_intereses).green().bold()
        );
        println!(
            "  ║     {}",
            "(Es el \"pago para no generar intereses\" del estado de cuenta)".dimmed()
        );
        if est.interes_residual_estimado > 0.01 {
            println!("  ║");
            println!(
                "  ║  ⚠️ Ojo: puede aparecer ~${:.2} de interés residual en el siguiente",
                est.interes_residual_estimado
            );
            println!("  ║     corte (interés de días entre compra y corte). Es normal y es poco.");
        }
    } else {
        println!("  ║  ✅ Estás libre de intereses. Sigue pagando el total cada mes.");
    }

    println!("  ║");
    println!(
        "{}",
        "  ╚═══════════════════════════════════════════════════════════════════╝".cyan()
    );

    // ═══════════════════════════════════════════════════════
    //  SECCIÓN 5: Tabla comparativa — mínimo vs actual vs sin intereses vs total
    // ═══════════════════════════════════════════════════════
    println!();
    println!("  📊 COMPARACIÓN: ¿Qué pasa con cada estrategia de pago?");
    println!();
    println!(
        "{}",
        "  ╔══════════════════════════════════════════════════════════════════════════════════════╗"
            .cyan()
    );
    println!(
        "  ║  {:<35} {:>8} {:>14} {:>14} {:>14}",
        "Estrategia", "Meses", "Intereses $", "Total pagado $", "Regalas al banco"
    );
    println!(
        "{}",
        "  ╠══════════════════════════════════════════════════════════════════════════════════════╣"
            .cyan()
    );

    // Tabla de planes
    let planes = [
        (&est.plan_minimo, est.dinero_regalado_al_banco_minimo),
        (&est.plan_actual, est.dinero_regalado_al_banco_actual),
        (&est.plan_sin_intereses, 0.0),
        (&est.plan_total, 0.0),
    ];

    let mejor_total = planes
        .iter()
        .map(|(p, _)| p.total_pagado)
        .fold(f64::INFINITY, f64::min);

    for (plan, regalado) in &planes {
        let es_mejor = (plan.total_pagado - mejor_total).abs() < 0.01;
        let marca = if es_mejor { " ⭐" } else { "" };

        let regalado_str = if *regalado > 0.01 {
            format!("${:.2}", regalado).red().to_string()
        } else {
            "—".dimmed().to_string()
        };

        let nombre_display = format!("{} (${:.0}/mes)", plan.nombre, plan.monto_mensual);

        println!(
            "  ║  {:<35} {:>6}m   ${:>12.2}   ${:>12.2}   {:>14}{}",
            nombre_display,
            plan.meses_para_liquidar,
            plan.total_intereses,
            plan.total_pagado,
            regalado_str,
            marca
        );
    }

    println!(
        "{}",
        "  ╚══════════════════════════════════════════════════════════════════════════════════════╝"
            .cyan()
    );

    // ═══════════════════════════════════════════════════════
    //  SECCIÓN 6: Veredicto claro
    // ═══════════════════════════════════════════════════════
    println!();
    if est.tiene_intereses {
        if est.dinero_regalado_al_banco_minimo > 0.01 {
            println!(
                "  🔴 Si sigues pagando el mínimo, le regalas ${:.2} al banco en intereses.",
                est.dinero_regalado_al_banco_minimo
            );
        }
        if est.dinero_regalado_al_banco_actual > 0.01
            && (est.dinero_regalado_al_banco_actual - est.dinero_regalado_al_banco_minimo).abs()
                > 0.01
        {
            println!(
                "  🟡 Con tu pago actual (${:.0}), aún regalas ${:.2} en intereses.",
                corte.pago_realizado, est.dinero_regalado_al_banco_actual
            );
        }

        println!();
        println!("  ⭐ RECOMENDACIÓN:");
        if est.monto_corta_intereses <= corte.pago_realizado * 1.5 {
            println!(
                "  {}  Paga ${:.2} este mes y CORTAS los intereses de un solo golpe.",
                "→".green().bold(),
                est.monto_corta_intereses
            );
            println!(
                "  {}  Después solo necesitas pagar lo que compres cada mes (sin interés).",
                "→".green().bold()
            );
        } else {
            println!(
                "  →  Lo ideal: pagar ${:.2} para cortar los intereses.",
                est.monto_corta_intereses
            );
            let plan_doble = &est.plan_actual;
            if plan_doble.meses_para_liquidar <= 3 {
                println!(
                    "  →  Con tu pago actual (${:.0}/mes) terminas en {} meses.",
                    corte.pago_realizado, plan_doble.meses_para_liquidar
                );
            } else {
                // Sugerir un monto intermedio que liquide en ~3 meses
                let monto_3m = corte.saldo_al_corte / 3.0 * 1.05; // +5% para cubrir intereses
                println!(
                    "  →  Si no puedes pagar todo, intenta ${:.0}/mes → ~3 meses para liquidar.",
                    monto_3m
                );
            }
            println!("  →  Mientras más rápido pagues, menos intereses acumulas.");
        }

        // Si pago_minimo ni siquiera cubre intereses
        if !est.pago_cubre_intereses {
            println!();
            println!(
                "  {} ALERTA: El pago mínimo NO cubre ni los intereses.",
                "⛔ DEUDA CRECIENDO".red().bold()
            );
            println!(
                "     Necesitas pagar mínimo ${:.2}/mes solo para que no crezca.",
                corte.intereses_cobrados
            );
        }
    } else {
        println!("  ✅ No tienes intereses pendientes. Sigue así.");
        println!(
            "  → Para mantenerlo: paga ${:.2} antes de la fecha límite.",
            corte.pago_no_intereses
        );
    }

    // ═══════════════════════════════════════════════════════
    //  SECCIÓN 7: Tabla mes a mes (opcional)
    // ═══════════════════════════════════════════════════════
    println!();
    let opciones_sig = &[
        "📋  Ver tabla mes a mes (con estrategia actual)",
        "📋  Ver tabla mes a mes (pagando para no generar intereses)",
        "💾  Guardar y volver",
    ];

    match menu("¿Quieres ver el desglose mes a mes?", opciones_sig) {
        Some(0) => {
            mostrar_tabla_mensual(&analisis.deuda, corte.pago_realizado, &analisis);
        }
        Some(1) => {
            mostrar_tabla_mensual(&analisis.deuda, est.monto_corta_intereses, &analisis);
        }
        _ => {}
    }

    // Guardar la deuda analizada
    state.asesor.analisis_deudas.push(analisis.deuda.clone());

    // Registro automático del corte bancario
    let resumen_corte = format!(
        "Saldo ${:.2}, tasa {:.2}%/mes ({:.1}%/año), intereses ${:.2}",
        corte.saldo_al_corte,
        analisis.tasa_mensual_calculada * 100.0,
        analisis.tasa_anual_calculada * 100.0,
        corte.intereses_cobrados
    );
    let id = state.asesor.siguiente_id();
    let hoy_reg = Local::now().format("%Y-%m-%d").to_string();
    let hora_reg = Local::now().format("%H:%M").to_string();
    let reg = RegistroAsesor::nuevo(
        id,
        &hoy_reg,
        &hora_reg,
        &nombre,
        &resumen_corte,
        vec![
            "tarjeta".into(),
            "intereses".into(),
            "banco".into(),
            nombre.clone(),
        ],
        TipoRegistro::CorteBancario {
            corte: corte.clone(),
            tasa_mensual: analisis.tasa_mensual_calculada,
            tasa_anual: analisis.tasa_anual_calculada,
            saldo_que_genero_interes: analisis.saldo_que_genero_interes,
            pago_a_capital: analisis.pago_a_capital,
            pago_a_interes: analisis.pago_a_interes,
            monto_corta_intereses: analisis.estrategia.monto_corta_intereses,
        },
    );
    state.asesor.registros.push(reg);

    // Guardar en memoria
    let contenido = format!(
        "Corte bancario: {} — Saldo ${:.2} — Tasa calculada {:.2}% mensual ({:.1}% anual) — Intereses cobrados ${:.2}",
        nombre,
        corte.saldo_al_corte,
        analisis.tasa_mensual_calculada * 100.0,
        analisis.tasa_anual_calculada * 100.0,
        corte.intereses_cobrados
    );
    let recuerdo = omniplanner::memoria::Recuerdo::new(
        contenido,
        vec![
            "tarjeta".into(),
            "intereses".into(),
            "banco".into(),
            nombre.clone(),
        ],
    )
    .con_origen("asesor", &nombre);
    state.memoria.agregar_recuerdo(recuerdo);

    pausa();
}

fn mostrar_tabla_mensual(
    deuda: &omniplanner::ml::AnalisisDeuda,
    monto_pago: f64,
    analisis: &omniplanner::ml::advisor::AnalisisCorte,
) {
    let proy = deuda.proyeccion_mensual(monto_pago, 60);
    println!();
    println!(
        "  Proyección pagando ${:.2}/mes — Tasa: {:.2}%/mes",
        monto_pago,
        analisis.tasa_mensual_calculada * 100.0
    );
    println!(
        "  {:<5} {:>10} {:>10} {:>12} {:>12} {:>14}",
        "Mes", "Pago", "Interés", "A capital", "Saldo", "Int. acum."
    );
    println!("  {}", "─".repeat(65));
    for f in &proy {
        println!(
            "  {:<5} ${:>8.2} ${:>8.2} ${:>10.2} ${:>10.2} ${:>12.2}",
            f.mes, f.pago, f.interes, f.abono_capital, f.saldo_restante, f.intereses_acumulados
        );
    }
    if let Some(last) = proy.last() {
        if last.saldo_restante > 0.01 {
            println!();
            println!(
                "  ⚠️ Después de 60 meses aún deberías ${:.2}",
                last.saldo_restante
            );
        }
    }
    println!("  {}", "─".repeat(65));
    pausa();
}

// ══════════════════════════════════════════════════════════════
//  PRESUPUESTO BASE CERO — cada dólar tiene un destino
// ══════════════════════════════════════════════════════════════

fn menu_presupuesto_cero(state: &mut AppState) {
    let mes_actual = Local::now().format("%Y-%m").to_string();

    // Si no hay plantilla, guiar directo a crearla
    if state.presupuesto.plantilla.is_none() {
        limpiar();
        println!(
            "{}",
            "╔══════════════════════════════════════════════════════════╗".cyan()
        );
        println!(
            "{}",
            "║  📊 P R E S U P U E S T O   B A S E   C E R O         ║"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "║  Cada dólar tiene un destino — Saldo final = $0        ║".cyan()
        );
        println!(
            "{}",
            "╚══════════════════════════════════════════════════════════╝".cyan()
        );
        println!();
        println!("  👋 Vamos a configurar tu presupuesto por primera vez.");
        println!("  💡 Solo necesitas hacerlo una vez — después cada mes se genera solo.");
        println!();
        if Confirm::new()
            .with_prompt("  ¿Crear tu plantilla de presupuesto ahora?")
            .default(true)
            .interact()
            .unwrap_or(false)
        {
            crear_plantilla_manual(state);
        } else {
            return;
        }
    }

    // Si hay plantilla pero no hay mes actual, generarlo automáticamente
    if state.presupuesto.plantilla.is_some() && state.presupuesto.mes_actual(&mes_actual).is_none()
    {
        if let Some(plantilla) = &state.presupuesto.plantilla {
            let nuevo = plantilla.generar_mes(&mes_actual);
            println!(
                "\n  ✅ Presupuesto de {} generado automáticamente ({} líneas).",
                mes_actual,
                nuevo.lineas.len()
            );
            state.presupuesto.meses.push(nuevo);
        }
    }

    loop {
        limpiar();
        println!(
            "{}",
            "╔══════════════════════════════════════════════════════════╗".cyan()
        );
        println!(
            "{}",
            "║  📊 P R E S U P U E S T O   B A S E   C E R O         ║"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "║  Cada dólar tiene un destino — Saldo final = $0        ║".cyan()
        );
        println!(
            "{}",
            "╚══════════════════════════════════════════════════════════╝".cyan()
        );
        println!();

        // Mostrar resumen del mes actual
        if let Some(pres) = state.presupuesto.mes_actual(&mes_actual) {
            mostrar_resumen_presupuesto_cero(pres);
        }
        println!();

        let opciones = &[
            "✅  Marcar pagos realizados",
            "✏️   Editar monto de una línea",
            "➕  Agregar línea al mes",
            "📊  Ver presupuesto completo",
            "🔧  Editar plantilla base",
            "🔄  Regenerar mes desde plantilla",
            "📆  Ver otro mes",
            "🔙  Volver",
        ];

        match menu("¿Qué hacer?", opciones) {
            Some(0) => marcar_pagos(state, &mes_actual),
            Some(1) => editar_monto_linea(state, &mes_actual),
            Some(2) => agregar_linea_mes(state, &mes_actual),
            Some(3) => {
                ver_presupuesto_completo(state, &mes_actual);
                pausa();
            }
            Some(4) => crear_plantilla_manual(state),
            Some(5) => generar_presupuesto_mes(state, &mes_actual),
            Some(6) => {
                let mes = pedir_texto_opcional("Mes (YYYY-MM, ej: 2026-03)");
                if !mes.is_empty() {
                    ver_presupuesto_completo(state, &mes);
                    pausa();
                }
            }
            _ => return,
        }
    }
}

fn mostrar_resumen_presupuesto_cero(pres: &PresupuestoMensual) {
    let res = pres.resumen();

    println!("  📅 Mes: {}", pres.mes.bold());
    println!();
    println!(
        "    💵 Ingresos:         {}",
        format!("${:>10.2}", res.ingresos).green()
    );
    println!(
        "    🏠 Gastos fijos:    -{}  ({:.0}%)",
        format!("${:>10.2}", res.gastos_fijos),
        res.pct_fijos
    );
    println!(
        "    🛒 Gastos variables:-{}  ({:.0}%)",
        format!("${:>10.2}", res.gastos_variables),
        res.pct_variables
    );
    println!(
        "    💳 Pagos deuda:     -{}  ({:.0}%)",
        format!("${:>10.2}", res.pagos_deuda),
        res.pct_deuda
    );
    println!(
        "    🏦 Ahorro:          -{}  ({:.0}%)",
        format!("${:>10.2}", res.ahorro),
        res.pct_ahorro
    );
    println!("    ─────────────────────────────");

    match &res.salud {
        SaludPresupuesto::Perfecto => {
            println!("    ✅  Saldo: $0.00 — ¡Cada dólar asignado!");
        }
        SaludPresupuesto::SobraDinero(s) => {
            println!("    🟡 Sobran ${:.2} — asígnalos a ahorro o deuda.", s);
        }
        SaludPresupuesto::FaltaDinero(f) => {
            println!("    🔴 Faltan ${:.2} — recorta gastos.", f);
        }
    }

    // Progreso de pagos
    if res.egresos > 0.0 {
        let pct_pagado = res.pagado / res.egresos * 100.0;
        let barra_len = 30;
        let lleno = ((pct_pagado / 100.0) * barra_len as f64) as usize;
        let vacio = barra_len - lleno;
        println!();
        println!(
            "    Pagado: [{}{}] {:.0}%  (${:.2} de ${:.2})",
            "█".repeat(lleno).green(),
            "░".repeat(vacio),
            pct_pagado,
            res.pagado,
            res.egresos
        );
    }

    for alerta in &res.alertas {
        println!("    {}", alerta.yellow());
    }
}

fn generar_presupuesto_mes(state: &mut AppState, mes: &str) {
    if state.presupuesto.mes_actual(mes).is_some() {
        println!("  ⚠️ Ya existe un presupuesto para {}. ¿Regenerar?", mes);
        if !Confirm::new()
            .with_prompt("  Esto reemplazará el existente")
            .default(false)
            .interact()
            .unwrap_or(false)
        {
            return;
        }
        // Eliminar el existente
        state.presupuesto.meses.retain(|m| m.mes != mes);
    }

    if let Some(plantilla) = &state.presupuesto.plantilla {
        let nuevo = plantilla.generar_mes(mes);
        println!(
            "  ✅ Presupuesto generado para {} con {} líneas.",
            mes,
            nuevo.lineas.len()
        );
        state.presupuesto.meses.push(nuevo);
    } else {
        println!(
            "  {} No hay plantilla. Crea una primero (manual o desde Excel).",
            "✗".red()
        );
    }
    pausa();
}

fn crear_plantilla_manual(state: &mut AppState) {
    limpiar();
    separador("🔧 CREAR PLANTILLA DE PRESUPUESTO");

    let mut lineas = Vec::new();

    println!("  💵 Primero tus ingresos (sueldos, etc.):");
    loop {
        let nombre = match pedir_texto(&format!("Ingreso {} (vacío=siguiente)", lineas.len() + 1))
        {
            Some(n) => n,
            None => break,
        };
        let monto = pedir_f64("  Monto $", 0.0);
        if monto <= 0.0 {
            break;
        }
        let fecha = pedir_texto_opcional("  Día del mes que llega (ej: 1, 15)");
        lineas.push(presupuesto_cero::LineaPlantilla {
            nombre,
            categoria: Categoria::Ingreso,
            monto_default: monto,
            fecha_limite: fecha,
            saldo_total_deuda: None,
        });
    }

    println!();
    println!("  🏠 Gastos FIJOS (casa, carro, seguros, servicios):");
    loop {
        let nombre = match pedir_texto(&format!(
            "Gasto fijo {} (vacío=siguiente)",
            lineas
                .iter()
                .filter(|l| l.categoria == Categoria::GastoFijo)
                .count()
                + 1
        )) {
            Some(n) => n,
            None => break,
        };
        let monto = pedir_f64("  Monto $", 0.0);
        if monto <= 0.0 {
            break;
        }
        let fecha = pedir_texto_opcional("  Día límite de pago (ej: 5, 15)");
        lineas.push(presupuesto_cero::LineaPlantilla {
            nombre,
            categoria: Categoria::GastoFijo,
            monto_default: monto,
            fecha_limite: fecha,
            saldo_total_deuda: None,
        });
    }

    println!();
    println!("  🛒 Gastos VARIABLES (comida, gasolina, entretenimiento):");
    loop {
        let nombre = match pedir_texto(&format!(
            "Gasto variable {} (vacío=siguiente)",
            lineas
                .iter()
                .filter(|l| l.categoria == Categoria::GastoVariable)
                .count()
                + 1
        )) {
            Some(n) => n,
            None => break,
        };
        let monto = pedir_f64("  Monto $", 0.0);
        if monto <= 0.0 {
            break;
        }
        lineas.push(presupuesto_cero::LineaPlantilla {
            nombre,
            categoria: Categoria::GastoVariable,
            monto_default: monto,
            fecha_limite: String::new(),
            saldo_total_deuda: None,
        });
    }

    println!();
    println!("  💳 Pagos de DEUDA (tarjetas, préstamos):");
    loop {
        let nombre = match pedir_texto(&format!(
            "Deuda {} (vacío=siguiente)",
            lineas
                .iter()
                .filter(|l| l.categoria == Categoria::PagoDeuda)
                .count()
                + 1
        )) {
            Some(n) => n,
            None => break,
        };
        let saldo_total = pedir_f64("  Saldo TOTAL de la deuda $", 0.0);
        let monto = pedir_f64("  Pago mensual que harás $", 0.0);
        if monto <= 0.0 {
            break;
        }
        if saldo_total > 0.0 {
            let meses_restantes = (saldo_total / monto).ceil() as u32;
            println!(
                "    📊 Con ${:.0}/mes pagarás ${:.2} en ~{} meses ({:.1} años)",
                monto,
                saldo_total,
                meses_restantes,
                meses_restantes as f64 / 12.0
            );
        }
        let fecha = pedir_texto_opcional("  Día límite de pago");
        lineas.push(presupuesto_cero::LineaPlantilla {
            nombre,
            categoria: Categoria::PagoDeuda,
            monto_default: monto,
            fecha_limite: fecha,
            saldo_total_deuda: if saldo_total > 0.0 {
                Some(saldo_total)
            } else {
                None
            },
        });
    }

    println!();
    println!("  🏦 AHORRO (savings, fondo de emergencia):");
    loop {
        let nombre = match pedir_texto(&format!(
            "Ahorro {} (vacío=terminar)",
            lineas
                .iter()
                .filter(|l| l.categoria == Categoria::Ahorro)
                .count()
                + 1
        )) {
            Some(n) => n,
            None => break,
        };
        let monto = pedir_f64("  Monto $", 0.0);
        if monto <= 0.0 {
            break;
        }
        lineas.push(presupuesto_cero::LineaPlantilla {
            nombre,
            categoria: Categoria::Ahorro,
            monto_default: monto,
            fecha_limite: String::new(),
            saldo_total_deuda: None,
        });
    }

    if lineas.is_empty() {
        println!("  {} No se agregó nada.", "✗".red());
        pausa();
        return;
    }

    // Calcular saldo con la plantilla
    let total_ing: f64 = lineas
        .iter()
        .filter(|l| l.categoria == Categoria::Ingreso)
        .map(|l| l.monto_default)
        .sum();
    let total_eg: f64 = lineas
        .iter()
        .filter(|l| l.categoria != Categoria::Ingreso)
        .map(|l| l.monto_default)
        .sum();
    let saldo = total_ing - total_eg;

    println!();
    println!("  Resumen de la plantilla:");
    println!("    Ingresos: ${:.2}", total_ing);
    println!("    Egresos:  ${:.2}", total_eg);
    if saldo.abs() < 0.01 {
        println!("    ✅ Saldo: $0.00 — ¡Perfecto, cada dólar asignado!");
    } else if saldo > 0.0 {
        println!(
            "    🟡 Sobran ${:.2} — podrías asignarlos a ahorro o deuda",
            saldo
        );
    } else {
        println!(
            "    🔴 Faltan ${:.2} — necesitas recortar o agregar ingreso",
            -saldo
        );
    }

    state.presupuesto.plantilla = Some(PlantillaPresupuesto {
        nombre: "Mi presupuesto".into(),
        lineas,
    });
    println!();
    println!("  ✅ Plantilla guardada.");
    pausa();
}

fn marcar_pagos(state: &mut AppState, mes: &str) {
    let pres = match state.presupuesto.mes_actual_mut(mes) {
        Some(p) => p,
        None => {
            println!(
                "  {} No hay presupuesto para {}. Genera uno primero.",
                "✗".red(),
                mes
            );
            pausa();
            return;
        }
    };

    limpiar();
    separador("✅ MARCAR PAGOS REALIZADOS");

    let pendientes: Vec<usize> = pres
        .lineas
        .iter()
        .enumerate()
        .filter(|(_, l)| !l.pagado && l.categoria != Categoria::Ingreso)
        .map(|(i, _)| i)
        .collect();

    if pendientes.is_empty() {
        println!("  ✅ ¡Todos los pagos están al día!");
        pausa();
        return;
    }

    let _nombres: Vec<String> = pendientes
        .iter()
        .map(|&i| {
            let l = &pres.lineas[i];
            format!(
                "{} {} — ${:.2}{}",
                l.categoria.emoji(),
                l.nombre,
                l.monto,
                if l.fecha_limite.is_empty() {
                    String::new()
                } else {
                    format!(" (día {})", l.fecha_limite)
                }
            )
        })
        .collect();

    println!("  💰 Selecciona los pagos que ya realizaste:");

    // Marcar uno a uno
    loop {
        let pendientes_now: Vec<usize> = pres
            .lineas
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.pagado && l.categoria != Categoria::Ingreso)
            .map(|(i, _)| i)
            .collect();

        if pendientes_now.is_empty() {
            println!("  ✅ ¡Todo pagado!");
            break;
        }

        let nombres_now: Vec<String> = pendientes_now
            .iter()
            .map(|&i| {
                let l = &pres.lineas[i];
                format!("{} {} — ${:.2}", l.categoria.emoji(), l.nombre, l.monto,)
            })
            .collect();
        let mut opciones: Vec<&str> = nombres_now.iter().map(|s| s.as_str()).collect();
        opciones.push("✅ Listo, volver");

        match menu("¿Qué pagaste?", &opciones) {
            Some(i) if i < pendientes_now.len() => {
                let idx = pendientes_now[i];
                pres.lineas[idx].pagado = true;
                println!(
                    "  {} {} marcado como pagado",
                    "✓".green(),
                    pres.lineas[idx].nombre
                );
            }
            _ => break,
        }
    }
    pausa();
}

fn agregar_linea_mes(state: &mut AppState, mes: &str) {
    let pres = match state.presupuesto.mes_actual_mut(mes) {
        Some(p) => p,
        None => {
            println!("  {} No hay presupuesto para {}.", "✗".red(), mes);
            pausa();
            return;
        }
    };

    let nombre = match pedir_texto("Nombre del concepto") {
        Some(n) => n,
        None => return,
    };
    let monto = pedir_f64("Monto $", 0.0);
    if monto <= 0.0 {
        return;
    }

    let cats = &[
        "💵 Ingreso",
        "🏠 Gasto Fijo",
        "🛒 Gasto Variable",
        "💳 Pago de Deuda",
        "🏦 Ahorro",
    ];
    let cat = match menu("Categoría", cats) {
        Some(0) => Categoria::Ingreso,
        Some(1) => Categoria::GastoFijo,
        Some(2) => Categoria::GastoVariable,
        Some(3) => Categoria::PagoDeuda,
        Some(4) => Categoria::Ahorro,
        _ => return,
    };

    let saldo_deuda = if cat == Categoria::PagoDeuda {
        let s = pedir_f64("  Saldo TOTAL de la deuda (0=no aplica) $", 0.0);
        if s > 0.0 {
            Some(s)
        } else {
            None
        }
    } else {
        None
    };

    pres.agregar(LineaPresupuesto {
        nombre: nombre.clone(),
        categoria: cat,
        monto,
        pagado: false,
        fecha_limite: String::new(),
        notas: String::new(),
        saldo_total_deuda: saldo_deuda,
    });

    println!("  ✅ '{}' agregado: ${:.2}", nombre, monto);
    pausa();
}

fn editar_monto_linea(state: &mut AppState, mes: &str) {
    let pres = match state.presupuesto.mes_actual_mut(mes) {
        Some(p) => p,
        None => {
            println!("  {} No hay presupuesto para {}.", "✗".red(), mes);
            pausa();
            return;
        }
    };

    if pres.lineas.is_empty() {
        println!("  {} Presupuesto vacío.", "✗".red());
        pausa();
        return;
    }

    let nombres: Vec<String> = pres
        .lineas
        .iter()
        .map(|l| {
            format!(
                "{} {} — ${:.2}{}",
                l.categoria.emoji(),
                l.nombre,
                l.monto,
                if l.pagado { " ✅" } else { "" }
            )
        })
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    if let Some(idx) = menu("¿Cuál editar?", &refs) {
        let nuevo = pedir_f64(
            &format!(
                "Nuevo monto para '{}' (actual: ${:.2})",
                pres.lineas[idx].nombre, pres.lineas[idx].monto
            ),
            pres.lineas[idx].monto,
        );
        if nuevo > 0.0 {
            pres.lineas[idx].monto = nuevo;
            println!("  ✅ Actualizado a ${:.2}", nuevo);
        }
    }
    pausa();
}

fn ver_presupuesto_completo(state: &AppState, mes: &str) {
    let pres = match state.presupuesto.mes_actual(mes) {
        Some(p) => p,
        None => {
            println!("  {} No hay presupuesto para {}.", "✗".red(), mes);
            return;
        }
    };

    limpiar();
    println!(
        "{}",
        "╔══════════════════════════════════════════════════════════════════╗".cyan()
    );
    println!("  📊 PRESUPUESTO BASE CERO — {}", mes.bold());
    println!(
        "{}",
        "╠══════════════════════════════════════════════════════════════════╣".cyan()
    );

    let categorias = [
        Categoria::Ingreso,
        Categoria::GastoFijo,
        Categoria::GastoVariable,
        Categoria::PagoDeuda,
        Categoria::Ahorro,
    ];

    let mut _running_total: f64 = 0.0;

    for cat in &categorias {
        let items = pres.por_categoria(cat);
        if items.is_empty() {
            continue;
        }

        let subtotal: f64 = items.iter().map(|l| l.monto).sum();
        let pagados = items.iter().filter(|l| l.pagado).count();

        println!();
        println!(
            "  {} {} ({}/{} pagados) — ${:.2}",
            cat.emoji(),
            cat.nombre().bold(),
            pagados,
            items.len(),
            subtotal
        );
        println!("  ─────────────────────────────────────────────────");

        for item in &items {
            let estado = if item.pagado {
                "✅".to_string()
            } else {
                "⬜".to_string()
            };
            let fecha_str = if item.fecha_limite.is_empty() {
                String::new()
            } else {
                format!(" (día {})", item.fecha_limite)
            };
            println!(
                "    {} {:<30} ${:>10.2}{}",
                estado,
                item.nombre,
                item.monto,
                fecha_str.dimmed()
            );
            // Mostrar info de saldo total para deudas
            if item.categoria == Categoria::PagoDeuda {
                if let Some(saldo) = item.saldo_total_deuda {
                    let meses_rest = if item.monto > 0.0 {
                        (saldo / item.monto).ceil() as u32
                    } else {
                        0
                    };
                    println!(
                        "      📋 Saldo total: ${:.2} — ~{} meses restantes ({:.1} años)",
                        saldo,
                        meses_rest,
                        meses_rest as f64 / 12.0
                    );
                }
            }
        }

        if *cat == Categoria::Ingreso {
            _running_total += subtotal;
        } else {
            _running_total -= subtotal;
        }
    }

    println!();
    println!(
        "{}",
        "╠══════════════════════════════════════════════════════════════════╣".cyan()
    );

    let res = pres.resumen();
    println!("  💵 Ingresos:           ${:>10.2}", res.ingresos);
    println!("  💸 Total egresos:     -${:>10.2}", res.egresos);
    println!("  ─────────────────────────────────────────");

    match &res.salud {
        SaludPresupuesto::Perfecto => {
            println!("  ✅ SALDO: $0.00 — ¡PERFECTO! Cada dólar tiene destino.");
        }
        SaludPresupuesto::SobraDinero(s) => {
            println!(
                "  🟡 SOBRAN: ${:.2} — Asígnalos a ahorro o pago extra de deuda",
                s
            );
        }
        SaludPresupuesto::FaltaDinero(f) => {
            println!(
                "  🔴 FALTAN: ${:.2} — Recorta gastos variables o aumenta ingreso",
                f
            );
        }
    }

    // Barra de progreso
    if res.egresos > 0.0 {
        let pct = res.pagado / res.egresos * 100.0;
        let barra_len = 40;
        let lleno = ((pct / 100.0) * barra_len as f64) as usize;
        let vacio = barra_len - lleno;
        println!();
        println!(
            "  Progreso: [{}{}] {:.0}%  (${:.2} pagado / ${:.2})",
            "█".repeat(lleno).green(),
            "░".repeat(vacio),
            pct,
            res.pagado,
            res.egresos
        );
    }

    // Alertas
    if !res.alertas.is_empty() {
        println!();
        for alerta in &res.alertas {
            println!("  {}", alerta);
        }
    }

    // Resumen de deudas totales
    let deudas = pres.info_deudas();
    if !deudas.is_empty() {
        println!();
        println!("  💳 RESUMEN DE DEUDAS:");
        println!("  ─────────────────────────────────────────────────");
        let mut total_deuda = 0.0f64;
        let mut total_pago_mes = 0.0f64;
        for (nombre, pago, saldo, meses) in &deudas {
            total_deuda += saldo;
            total_pago_mes += pago;
            println!(
                "    • {:<25} Saldo: ${:>10.2}  Pago: ${:>7.2}/mes  ~{} meses",
                nombre, saldo, pago, meses
            );
        }
        println!("    ─────────────────────────────────────────────");
        println!("    Deuda total:  ${:.2}", total_deuda);
        println!("    Pago total/mes: ${:.2}", total_pago_mes);
        if total_pago_mes > 0.0 {
            let meses_global = (total_deuda / total_pago_mes).ceil() as u32;
            println!(
                "    Libre de deuda en ~{} meses ({:.1} años)",
                meses_global,
                meses_global as f64 / 12.0
            );
        }
    }

    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════════════╝".cyan()
    );
}

// ── Comparación rápida A vs B ──

fn menu_asesor_comparacion(state: &mut AppState) {
    limpiar();
    separador("⚖️  COMPARACIÓN RÁPIDA");

    println!("  Compara dos opciones rápidamente:\n");

    let titulo = match pedir_texto("¿Qué estás decidiendo?") {
        Some(t) => t,
        None => return,
    };

    let opcion_a = match pedir_texto("Opción A (nombre)") {
        Some(o) => o,
        None => return,
    };
    let costo_a = pedir_f64("Costo / inversión de A ($)", 0.0);
    let beneficio_a = pedir_texto_opcional("Beneficio o resultado de A");

    let opcion_b = match pedir_texto("Opción B (nombre)") {
        Some(o) => o,
        None => return,
    };
    let costo_b = pedir_f64("Costo / inversión de B ($)", 0.0);
    let beneficio_b = pedir_texto_opcional("Beneficio o resultado de B");

    let comp = ComparacionRapida::nueva(
        &titulo,
        &opcion_a,
        costo_a,
        &beneficio_a,
        &opcion_b,
        costo_b,
        &beneficio_b,
    );

    // Mostrar resultado
    println!();
    println!(
        "{}",
        "  ╔════════════════════════════════════════════════════╗".cyan()
    );
    println!("  ║  ⚖️ {}", comp.titulo.bold());
    println!(
        "{}",
        "  ╠════════════════════════════════════════════════════╣".cyan()
    );
    println!(
        "  ║  {} {:<20} │ {} {:<20}",
        "A:".yellow().bold(),
        comp.opcion_a,
        "B:".yellow().bold(),
        comp.opcion_b
    );
    println!(
        "  ║  Costo: ${:<17.2} │ Costo: ${:<17.2}",
        comp.costo_a, comp.costo_b
    );
    println!(
        "  ║  {}                │ {}",
        if comp.beneficio_a.len() > 20 {
            &comp.beneficio_a[..20]
        } else {
            &comp.beneficio_a
        },
        if comp.beneficio_b.len() > 20 {
            &comp.beneficio_b[..20]
        } else {
            &comp.beneficio_b
        }
    );
    println!(
        "{}",
        "  ╠════════════════════════════════════════════════════╣".cyan()
    );
    println!("  ║");
    if comp.diferencia.abs() > 0.01 {
        println!("  ║  💰 Diferencia: ${:.2}", comp.diferencia.abs());
    }
    println!("  ║  📌 {}", comp.recomendacion.green());
    println!("  ║");
    println!(
        "{}",
        "  ╚════════════════════════════════════════════════════╝".cyan()
    );

    // Buscar decisiones similares previas
    let similares = state.asesor.diccionario.buscar_similares(&titulo);
    if !similares.is_empty() {
        println!();
        println!("  🔍 Decisiones similares previas:");
        for a in similares.iter().take(3) {
            println!(
                "    {} {} — {} ({}) {}",
                a.impacto.emoji(),
                a.accion,
                a.categoria,
                a.fecha,
                a.monto
                    .map(|m| format!("${:.2}", m))
                    .unwrap_or_default()
                    .dimmed()
            );
        }
    }

    state.asesor.comparaciones.push(comp.clone());

    // Registro automático
    let id = state.asesor.siguiente_id();
    let hoy = Local::now().format("%Y-%m-%d").to_string();
    let hora = Local::now().format("%H:%M").to_string();
    let comp_titulo = comp.titulo.clone();
    let comp_rec = comp.recomendacion.clone();
    let reg = RegistroAsesor::nuevo(
        id,
        &hoy,
        &hora,
        &comp_titulo,
        &comp_rec,
        vec!["comparacion".into(), titulo.clone()],
        TipoRegistro::Comparacion(comp),
    );
    state.asesor.registros.push(reg);

    pausa();
}

// ── Matriz de decisión multi-criterio ──

fn menu_asesor_matriz(state: &mut AppState) {
    loop {
        limpiar();
        separador("🧮 MATRICES DE DECISIÓN");

        if !state.asesor.matrices.is_empty() {
            for (i, m) in state.asesor.matrices.iter().enumerate() {
                let mejor = m
                    .mejor_opcion()
                    .map(|(nombre, score)| format!("→ {} ({:.1}/10)", nombre, score))
                    .unwrap_or_default();
                println!(
                    "  {}. {} ({} opciones, {} criterios) {}",
                    (i + 1).to_string().cyan(),
                    m.titulo,
                    m.opciones.len(),
                    m.criterios.len(),
                    mejor.green()
                );
            }
            println!();
        }

        let opciones = &[
            "➕  Nueva matriz de decisión",
            "📊  Ver detalle de una matriz",
            "🗑️   Eliminar matriz",
            "🔙  Volver",
        ];

        match menu("Acción", opciones) {
            Some(0) => nueva_matriz_decision(state),
            Some(1) => ver_matriz_decision(state),
            Some(2) => {
                if state.asesor.matrices.is_empty() {
                    println!("  No hay matrices.");
                    pausa();
                    continue;
                }
                let nombres: Vec<String> = state
                    .asesor
                    .matrices
                    .iter()
                    .map(|m| m.titulo.clone())
                    .collect();
                let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
                if let Some(idx) = menu("¿Cuál eliminar?", &refs) {
                    state.asesor.matrices.remove(idx);
                    println!("  {} Eliminada", "✓".green());
                    pausa();
                }
            }
            _ => return,
        }
    }
}

fn nueva_matriz_decision(state: &mut AppState) {
    separador("➕ Nueva Matriz de Decisión");

    let titulo = match pedir_texto("¿Qué estás decidiendo? (ej: ¿Qué laptop comprar?)") {
        Some(t) => t,
        None => return,
    };

    let hoy = Local::now().format("%Y-%m-%d").to_string();
    let mut matriz = MatrizDecision::nueva(&titulo, &hoy);

    // Agregar criterios
    println!();
    println!("  📐 Define los criterios de evaluación y su peso (0.0 a 1.0)");
    println!("  💡 Ej: Precio (0.4), Calidad (0.3), Rapidez (0.3)");
    println!();

    loop {
        let nombre = match pedir_texto(&format!(
            "Criterio {} (vacío=terminar)",
            matriz.criterios.len() + 1
        )) {
            Some(n) => n,
            None => break,
        };
        let peso = pedir_f64("  Peso (0.0-1.0)", 0.5);
        matriz.agregar_criterio(&nombre, peso);
        println!(
            "  {} Criterio '{}' (peso: {:.2})",
            "✓".green(),
            nombre,
            peso
        );
    }

    if matriz.criterios.is_empty() {
        println!("  {} Necesitas al menos un criterio.", "✗".red());
        pausa();
        return;
    }

    // Agregar opciones
    println!();
    println!("  📋 Ahora define las opciones a comparar:");
    loop {
        let nombre = match pedir_texto(&format!(
            "Opción {} (vacío=terminar)",
            matriz.opciones.len() + 1
        )) {
            Some(n) => n,
            None => break,
        };
        matriz.agregar_opcion(&nombre);
    }

    if matriz.opciones.len() < 2 {
        println!("  {} Necesitas al menos 2 opciones.", "✗".red());
        pausa();
        return;
    }

    // Puntuar cada opción en cada criterio
    println!();
    println!("  📊 Puntúa cada opción en cada criterio (0-10):");
    for (i, opcion) in matriz.opciones.clone().iter().enumerate() {
        println!();
        println!("  → {}", opcion.bold());
        for (j, criterio) in matriz.criterios.clone().iter().enumerate() {
            let valor = pedir_f64(&format!("    {} (0-10)", criterio.nombre), 5.0);
            matriz.set_valor(i, j, valor);
        }
    }

    // Mostrar resultados
    mostrar_matriz_resultado(&matriz);

    // Registro automático
    let mejor_str = matriz
        .mejor_opcion()
        .map(|(n, s)| format!("{} ({:.1}/10)", n, s))
        .unwrap_or_default();
    let resumen_m = format!(
        "{} opciones, {} criterios → {}",
        matriz.opciones.len(),
        matriz.criterios.len(),
        mejor_str
    );
    let id = state.asesor.siguiente_id();
    let hoy = Local::now().format("%Y-%m-%d").to_string();
    let hora = Local::now().format("%H:%M").to_string();
    let reg = RegistroAsesor::nuevo(
        id,
        &hoy,
        &hora,
        &titulo,
        &resumen_m,
        vec!["decision".into(), "matriz".into(), titulo.clone()],
        TipoRegistro::MatrizDecision(matriz.clone()),
    );
    state.asesor.registros.push(reg);

    state.asesor.matrices.push(matriz);
    pausa();
}

fn ver_matriz_decision(state: &AppState) {
    if state.asesor.matrices.is_empty() {
        println!("  No hay matrices.");
        pausa();
        return;
    }
    let nombres: Vec<String> = state
        .asesor
        .matrices
        .iter()
        .map(|m| m.titulo.clone())
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
    if let Some(idx) = menu("¿Cuál ver?", &refs) {
        mostrar_matriz_resultado(&state.asesor.matrices[idx]);
        pausa();
    }
}

fn mostrar_matriz_resultado(m: &MatrizDecision) {
    println!();
    println!("  🧮 {} ({})", m.titulo.bold(), m.fecha.dimmed());

    // Header
    print!("  {:<20}", "");
    for c in &m.criterios {
        print!(" {:>10}", format!("{}({:.1})", c.nombre, c.peso));
    }
    println!(" {:>10}", "TOTAL".bold());
    println!("  {}", "─".repeat(20 + m.criterios.len() * 11 + 11));

    let puntuaciones = m.puntuaciones();
    let mejor = m
        .mejor_opcion()
        .map(|(nombre, _)| nombre)
        .unwrap_or_default();

    for (i, (opcion, score)) in puntuaciones.iter().enumerate() {
        let marca = if *opcion == mejor { " ⭐" } else { "" };
        print!("  {:<20}", opcion);
        for j in 0..m.criterios.len() {
            print!(" {:>10.1}", m.valores[i][j]);
        }
        println!(" {:>10}{}", format!("{:.2}", score).green().bold(), marca);
    }
    println!("  {}", "─".repeat(20 + m.criterios.len() * 11 + 11));

    if let Some((nombre, score)) = m.mejor_opcion() {
        println!();
        println!(
            "  ⭐ Recomendación: {} ({:.1}/10)",
            nombre.green().bold(),
            score
        );
    }
}

// ── Presupuesto mensual ──

fn menu_asesor_presupuesto(state: &mut AppState) {
    loop {
        limpiar();
        separador("💰 PRESUPUESTO MENSUAL");

        let pres = &state.asesor.presupuesto;
        let ingresos = pres.ingreso_mensual();
        let gastos = pres.gasto_mensual();
        let balance = pres.balance_mensual();

        if !pres.ingresos.is_empty() || !pres.gastos.is_empty() {
            println!("  📊 Resumen:");
            println!("    Ingresos:  {}", format!("${:.2}/mes", ingresos).green());
            println!("    Gastos:    {}", format!("${:.2}/mes", gastos).red());
            let balance_str = if balance >= 0.0 {
                format!("${:.2}/mes", balance).green().bold().to_string()
            } else {
                format!("-${:.2}/mes", balance.abs())
                    .red()
                    .bold()
                    .to_string()
            };
            println!("    Balance:   {}", balance_str);

            // Gastos fijos vs variables
            let fijos = pres.gastos_fijos_mensual();
            let variables = pres.gastos_variables_mensual();
            if gastos > 0.0 {
                println!();
                println!(
                    "    Fijos:     ${:.2} ({:.0}%)",
                    fijos,
                    fijos / gastos * 100.0
                );
                println!(
                    "    Variables: ${:.2} ({:.0}%)",
                    variables,
                    variables / gastos * 100.0
                );
            }

            // Por categoría
            let por_cat = pres.gastos_por_categoria();
            if !por_cat.is_empty() {
                println!();
                println!("    📂 Por categoría:");
                for (cat, monto) in &por_cat {
                    let pct = monto / gastos * 100.0;
                    let barra = "█".repeat((pct / 5.0).ceil() as usize);
                    println!(
                        "      {:<15} ${:>8.2} ({:>4.1}%) {}",
                        cat,
                        monto,
                        pct,
                        barra.cyan()
                    );
                }
            }
        } else {
            println!(
                "  {}",
                "(vacío — agrega tus ingresos y gastos para ver el panorama completo)".dimmed()
            );
        }
        println!();

        let opciones = &[
            "💵  Agregar ingreso",
            "💸  Agregar gasto",
            "🎯  Definir meta de ahorro",
            "📋  Ver todos los movimientos",
            "🗑️   Eliminar movimiento",
            "🔙  Volver",
        ];

        match menu("Acción", opciones) {
            Some(0) => agregar_movimiento(state, true),
            Some(1) => agregar_movimiento(state, false),
            Some(2) => agregar_meta_ahorro(state),
            Some(3) => ver_movimientos(state),
            Some(4) => eliminar_movimiento(state),
            _ => return,
        }
    }
}

fn agregar_movimiento(state: &mut AppState, es_ingreso: bool) {
    let tipo_str = if es_ingreso { "ingreso" } else { "gasto" };

    let concepto = match pedir_texto(&format!("Concepto del {}", tipo_str)) {
        Some(c) => c,
        None => return,
    };
    let monto = pedir_f64("Monto ($)", 0.0);
    if monto <= 0.0 {
        println!("  {} El monto debe ser positivo", "✗".red());
        pausa();
        return;
    }

    let freq = match menu("Frecuencia", FrecuenciaPago::todas()) {
        Some(i) => FrecuenciaPago::desde_indice(i),
        None => return,
    };

    let categoria = pedir_texto_opcional("Categoría (ej: Vivienda, Transporte, Comida)");
    let categoria = if categoria.is_empty() {
        if es_ingreso {
            "Ingreso".to_string()
        } else {
            "General".to_string()
        }
    } else {
        categoria
    };

    let fijo = Confirm::new()
        .with_prompt("  ¿Es un gasto/ingreso fijo?")
        .default(true)
        .interact()
        .unwrap_or(true);

    let mov = Movimiento {
        concepto: concepto.clone(),
        monto,
        frecuencia: freq,
        categoria,
        fijo,
    };

    if es_ingreso {
        state.asesor.presupuesto.ingresos.push(mov);
    } else {
        state.asesor.presupuesto.gastos.push(mov);
    }

    println!(
        "  {} {} '{}' (${:.2}) agregado",
        "✓".green(),
        if es_ingreso { "Ingreso" } else { "Gasto" },
        concepto,
        monto
    );
    pausa();
}

fn agregar_meta_ahorro(state: &mut AppState) {
    let nombre = match pedir_texto("Nombre de la meta (ej: Fondo emergencia, Viaje)") {
        Some(n) => n,
        None => return,
    };
    let objetivo = pedir_f64("Monto objetivo ($)", 0.0);
    let ahorrado = pedir_f64("¿Cuánto llevas ahorrado? ($)", 0.0);
    let fecha = pedir_texto_opcional("Fecha meta (opcional, ej: 2026-12-31)");

    state.asesor.presupuesto.metas.push(MetaAhorro {
        nombre: nombre.clone(),
        objetivo,
        ahorrado,
        fecha_meta: fecha,
    });

    println!("  {} Meta '{}' creada", "✓".green(), nombre);
    pausa();
}

fn ver_movimientos(state: &AppState) {
    let pres = &state.asesor.presupuesto;

    if !pres.ingresos.is_empty() {
        println!();
        println!("  💵 Ingresos:");
        for (i, m) in pres.ingresos.iter().enumerate() {
            println!(
                "    {}. {} — ${:.2} ({}) [{}] {}",
                i + 1,
                m.concepto,
                m.monto,
                m.frecuencia.nombre(),
                m.categoria,
                if m.fijo { "fijo" } else { "variable" }
            );
        }
    }

    if !pres.gastos.is_empty() {
        println!();
        println!("  💸 Gastos:");
        for (i, m) in pres.gastos.iter().enumerate() {
            println!(
                "    {}. {} — ${:.2} ({}) [{}] {}",
                i + 1,
                m.concepto,
                m.monto,
                m.frecuencia.nombre(),
                m.categoria,
                if m.fijo { "fijo" } else { "variable" }
            );
        }
    }

    if !pres.metas.is_empty() {
        println!();
        println!("  🎯 Metas de ahorro:");
        for m in &pres.metas {
            let pct = if m.objetivo > 0.0 {
                m.ahorrado / m.objetivo * 100.0
            } else {
                0.0
            };
            let barra_len = (pct / 5.0).ceil() as usize;
            let barra = format!(
                "{}{}",
                "█".repeat(barra_len.min(20)),
                "░".repeat(20_usize.saturating_sub(barra_len))
            );
            println!(
                "    🎯 {} — ${:.2} / ${:.2} ({:.0}%) {}",
                m.nombre,
                m.ahorrado,
                m.objetivo,
                pct,
                barra.cyan()
            );
            if !m.fecha_meta.is_empty() {
                println!("       Fecha meta: {}", m.fecha_meta);
            }
        }
    }

    pausa();
}

fn eliminar_movimiento(state: &mut AppState) {
    let tipos = &["Eliminar un ingreso", "Eliminar un gasto", "Cancelar"];
    match menu("¿Qué eliminar?", tipos) {
        Some(0) => {
            if state.asesor.presupuesto.ingresos.is_empty() {
                println!("  No hay ingresos.");
                pausa();
                return;
            }
            let nombres: Vec<String> = state
                .asesor
                .presupuesto
                .ingresos
                .iter()
                .map(|m| format!("{} — ${:.2}", m.concepto, m.monto))
                .collect();
            let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
            if let Some(idx) = menu("¿Cuál?", &refs) {
                state.asesor.presupuesto.ingresos.remove(idx);
                println!("  {} Eliminado", "✓".green());
                pausa();
            }
        }
        Some(1) => {
            if state.asesor.presupuesto.gastos.is_empty() {
                println!("  No hay gastos.");
                pausa();
                return;
            }
            let nombres: Vec<String> = state
                .asesor
                .presupuesto
                .gastos
                .iter()
                .map(|m| format!("{} — ${:.2}", m.concepto, m.monto))
                .collect();
            let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
            if let Some(idx) = menu("¿Cuál?", &refs) {
                state.asesor.presupuesto.gastos.remove(idx);
                println!("  {} Eliminado", "✓".green());
                pausa();
            }
        }
        _ => {}
    }
}

// ── Proyecciones de ahorro ──

fn menu_asesor_proyecciones(state: &mut AppState) {
    limpiar();
    separador("📈 PROYECCIONES");

    let pres = &state.asesor.presupuesto;
    let balance = pres.balance_mensual();

    if pres.ingresos.is_empty() && pres.gastos.is_empty() {
        println!("  ⚠️ Configura tu presupuesto primero para ver proyecciones.");
        pausa();
        return;
    }

    println!(
        "  Balance mensual actual: {}",
        if balance >= 0.0 {
            format!("${:.2}", balance).green().to_string()
        } else {
            format!("-${:.2}", balance.abs()).red().to_string()
        }
    );
    println!();

    let meses = pedir_usize("¿Cuántos meses proyectar?", 12);

    // Proyección de ahorro
    let proyeccion = pres.proyeccion_ahorro(meses as u32);
    println!();
    println!("  📈 Proyección de ahorro acumulado:");
    println!("  {:<8} {:>14}", "Mes", "Ahorro acumulado");
    println!("  {}", "─".repeat(24));
    for (mes, acumulado) in &proyeccion {
        let color = if *acumulado >= 0.0 {
            format!("${:.2}", acumulado).green().to_string()
        } else {
            format!("-${:.2}", acumulado.abs()).red().to_string()
        };
        println!("  {:<8} {:>14}", format!("Mes {}", mes), color);
    }

    // Metas
    let metas_info = pres.meses_para_metas();
    if !metas_info.is_empty() {
        println!();
        println!("  🎯 Tiempo estimado para alcanzar metas:");
        for (nombre, faltante, meses_est) in &metas_info {
            if *meses_est == 0 {
                println!(
                    "    ⚠️  {} — Faltante: ${:.2} (balance insuficiente)",
                    nombre, faltante
                );
            } else {
                let anios = *meses_est / 12;
                let meses_r = *meses_est % 12;
                let tiempo = if anios > 0 {
                    format!("{} año(s) y {} mes(es)", anios, meses_r)
                } else {
                    format!("{} mes(es)", meses_r)
                };
                println!(
                    "    ✅ {} — Faltante: ${:.2} → {} para alcanzarla",
                    nombre,
                    faltante,
                    tiempo.green()
                );
            }
        }
    }

    // Deudas pendientes
    if !state.asesor.analisis_deudas.is_empty() {
        println!();
        println!("  💳 Deudas registradas:");
        for d in &state.asesor.analisis_deudas {
            let (meses_min, _, total_min) = d.simular_pagos(d.pago_minimo);
            println!(
                "    {} — Saldo: ${:.2} | Mínimo: ${:.2}/mes → {}m, total ${:.2}",
                d.nombre, d.saldo_total, d.pago_minimo, meses_min, total_min
            );
            if balance > d.pago_minimo {
                let pago_sugerido = balance.min(d.saldo_total);
                let (meses_s, _, total_s) = d.simular_pagos(pago_sugerido);
                println!(
                    "    💡 Con tu balance podrías pagar ${:.2}/mes → {}m, total ${:.2} (ahorras ${:.2})",
                    pago_sugerido,
                    meses_s,
                    total_s,
                    total_min - total_s
                );
            }
        }
    }

    // Registro automático de la proyección
    let id = state.asesor.siguiente_id();
    let hoy = Local::now().format("%Y-%m-%d").to_string();
    let hora = Local::now().format("%H:%M").to_string();
    let resumen_proy = format!(
        "Balance ${:.2}/mes, {} meses, acumulado final ${:.2}",
        balance,
        meses,
        proyeccion.last().map(|(_, v)| *v).unwrap_or(0.0)
    );
    let reg = RegistroAsesor::nuevo(
        id,
        &hoy,
        &hora,
        &format!("Proyección a {} meses", meses),
        &resumen_proy,
        vec!["proyeccion".into(), "ahorro".into()],
        TipoRegistro::ProyeccionAhorro {
            balance_mensual: balance,
            meses: meses as u32,
            proyeccion: proyeccion.clone(),
        },
    );
    state.asesor.registros.push(reg);

    pausa();
}

// ── Registrar acción / decisión ──

fn menu_asesor_registrar_accion(state: &mut AppState) {
    limpiar();
    separador("📝 REGISTRAR ACCIÓN");

    let accion = match pedir_texto("¿Qué acción o decisión tomaste?") {
        Some(a) => a,
        None => return,
    };
    let categoria = pedir_texto_opcional("Categoría (ej: finanzas, salud, proyecto, compra)");
    let categoria = if categoria.is_empty() {
        "general".to_string()
    } else {
        categoria
    };

    let impacto = match menu("¿Qué impacto tuvo?", ImpactoAccion::todas()) {
        Some(i) => ImpactoAccion::desde_indice(i),
        None => return,
    };

    let monto_str = pedir_texto_opcional("Monto involucrado ($ o vacío si no aplica)");
    let monto = monto_str.parse::<f64>().ok();

    let notas = pedir_texto_opcional("Notas adicionales");

    let hoy = Local::now().format("%Y-%m-%d").to_string();

    state
        .asesor
        .diccionario
        .registrar(&accion, &categoria, impacto.clone(), &hoy, monto, &notas);

    // Registro automático
    let id = state.asesor.siguiente_id();
    let hora = Local::now().format("%H:%M").to_string();
    let resumen_acc = format!(
        "{} — {} {}",
        categoria,
        impacto.nombre(),
        monto.map(|m| format!("${:.2}", m)).unwrap_or_default()
    );
    let reg = RegistroAsesor::nuevo(
        id,
        &hoy,
        &hora,
        &accion,
        &resumen_acc,
        vec!["accion".into(), categoria.clone()],
        TipoRegistro::Accion(omniplanner::ml::advisor::AccionRegistrada {
            accion: accion.clone(),
            categoria: categoria.clone(),
            impacto,
            fecha: hoy.clone(),
            monto,
            notas: notas.clone(),
        }),
    );
    state.asesor.registros.push(reg);

    println!("  {} Acción registrada", "✓".green());

    // Registrar en memoria del diccionario neuronal también
    let palabras: Vec<String> = accion
        .split_whitespace()
        .map(|s| s.to_lowercase())
        .collect();
    state
        .memoria
        .diccionario
        .registrar("asesor", &hoy, &accion, &palabras, &notas);

    pausa();
}

// ── Rastreador de deudas multi-cuenta con diagnóstico ──

fn menu_asesor_rastreador(state: &mut AppState) {
    loop {
        limpiar();
        println!(
            "{}",
            "╔══════════════════════════════════════════════════════════════╗".cyan()
        );
        println!(
            "{}",
            "║  🔎 R A S T R E A D O R   D E   D E U D A S              ║"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "║  Seguimiento multi-cuenta, diagnóstico y simulación       ║".cyan()
        );
        println!(
            "{}",
            "╚══════════════════════════════════════════════════════════════╝".cyan()
        );
        println!();

        let rast = &state.asesor.rastreador;
        if rast.deudas.is_empty() {
            println!("  📌 No hay deudas registradas en el rastreador.");
            println!("  💡 Agrega tus deudas con saldo, tasa y pagos mensuales.");
        } else {
            println!("  📊 Estado actual del portafolio de deudas:");
            println!();
            println!(
                "  {:<25} {:>12} {:>8} {:>12} {:>8}",
                "Cuenta", "Saldo", "Tasa%", "Pago", "Meses"
            );
            println!("  {}", "─".repeat(70));
            for d in &rast.deudas {
                let status = if d.activa { "" } else { " ✅" };
                if d.es_pago_corriente() {
                    println!(
                        "  {:<25} {:>12} {:>8} {:>12} {:>6} 🔒",
                        if d.nombre.len() > 24 {
                            format!("{}…", &d.nombre[..23])
                        } else {
                            d.nombre.clone()
                        },
                        "corriente",
                        "0.0%",
                        format!("${:.2}/mes", d.pago_minimo),
                        d.historial.len()
                    );
                } else {
                    let tipo = if d.obligatoria { " 🔒" } else { "" };
                    let gracia = if d.meses_gracia > 0 {
                        format!(" 🧊{}m", d.meses_gracia)
                    } else {
                        String::new()
                    };
                    let tasa_display = if d.meses_gracia > 0 {
                        format!("0→{:.1}%", d.tasa_anual)
                    } else {
                        format!("{:.1}%", d.tasa_anual)
                    };
                    println!(
                        "  {:<25} {:>12} {:>8} {:>12} {:>6}{}{}{}",
                        if d.nombre.len() > 24 {
                            format!("{}…", &d.nombre[..23])
                        } else {
                            d.nombre.clone()
                        },
                        format!("${:.2}", d.saldo_actual()),
                        tasa_display,
                        format!("${:.2}", d.pago_minimo),
                        d.historial.len(),
                        status,
                        tipo,
                        gracia
                    );
                }
            }
            println!("  {}", "─".repeat(70));
            let total = rast.deuda_total_actual();
            let activas = rast.deudas_activas().len();
            println!(
                "  Total: {}  ({} activas de {})",
                format!("${:.2}", total).red().bold(),
                activas,
                rast.deudas.len()
            );
            if !rast.ingresos.is_empty() {
                println!("  {}", "Ingresos:".green().bold());
                for ing in &rast.ingresos {
                    println!(
                        "    • {} — {} ({})",
                        ing.concepto,
                        format!("${:.2}", ing.monto).green(),
                        ing.frecuencia.nombre()
                    );
                }
                println!(
                    "    Total mensual: {}",
                    format!("${:.2}", rast.ingreso_mensual_total())
                        .green()
                        .bold()
                );
            }
        }
        println!();

        let opciones = &[
            "➕  Agregar nueva deuda",
            "📅  Registrar mes de pago (a una deuda)",
            "�  Revisar deuda individual (análisis predatorio + pagos sugeridos)",
            "📊  Diagnóstico completo (errores + recomendaciones)",
            "📈  Simulación: ¿qué hubiera pasado si...?",
            "🗺️   Simulación: camino a la libertad financiera",
            "📋  Tabla de aporte mínimo (¿cuánto necesito para salir en X meses?)",
            "✏️   Editar pago de un mes",
            "⚙️   Ajustar tasa de interés",
            "💵  Configurar ingresos",
            "📥  Exportar CSV del rastreador",
            "📂  Importar desde CSV (Excel convertido)",
            "🔧  Gestionar deudas (activar/desactivar, obligatoria)",
            "🗑️   Eliminar una deuda",
            "🔙  Volver",
        ];

        match menu("¿Qué hacer?", opciones) {
            Some(0) => rastreador_agregar_deuda(state),
            Some(1) => rastreador_registrar_mes(state),
            Some(2) => rastreador_revisar_deuda_individual(state),
            Some(3) => rastreador_diagnostico(state),
            Some(4) => rastreador_simulacion(state),
            Some(5) => rastreador_simulacion_libertad(state),
            Some(6) => rastreador_tabla_aporte_minimo(state),
            Some(7) => rastreador_editar_pago(state),
            Some(8) => rastreador_ajustar_tasa(state),
            Some(9) => rastreador_ingreso(state),
            Some(10) => rastreador_exportar(state),
            Some(11) => rastreador_importar_csv(state),
            Some(12) => rastreador_gestionar_deudas(state),
            Some(13) => rastreador_eliminar(state),
            _ => return,
        }
    }
}

fn rastreador_revisar_deuda_individual(state: &AppState) {
    let deudas_con_interes: Vec<(usize, &omniplanner::ml::advisor::DeudaRastreada)> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .enumerate()
        .filter(|(_, d)| d.activa && !d.es_pago_corriente() && d.saldo_actual() > 0.01)
        .collect();

    if deudas_con_interes.is_empty() {
        println!("  No hay deudas activas para revisar.");
        pausa();
        return;
    }

    loop {
        limpiar();
        println!(
            "{}",
            "╔══════════════════════════════════════════════════════════════╗".cyan()
        );
        println!(
            "{}",
            "║  🔍 REVISIÓN INDIVIDUAL DE DEUDAS                         ║"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "║  Selecciona una deuda para ver análisis detallado          ║".cyan()
        );
        println!(
            "{}",
            "╚══════════════════════════════════════════════════════════════╝".cyan()
        );
        println!();

        // Resumen rápido con indicadores
        println!(
            "  {:<22} {:>11} {:>7} {:>9} {:>10} {:>10} {:>10} Estado",
            "Cuenta", "Saldo", "Tasa%", "Int/mes", "Pago mín", "Sugerido", "A capital"
        );
        println!("  {}", "─".repeat(100));

        let mut opciones_menu: Vec<String> = Vec::new();
        let mut total_sugerido: f64 = 0.0;
        for (_, d) in deudas_con_interes.iter() {
            let saldo = d.saldo_actual();
            let tasa_mensual = d.tasa_efectiva() / 100.0 / 12.0;
            let interes_mensual = saldo * tasa_mensual;
            let es_predatoria = d.pago_minimo < interes_mensual && d.tasa_anual > 0.01;

            // Regla: pagar el DOBLE del mínimo o al menos +75%, lo que sea mayor
            let pago_sugerido = if d.tasa_anual >= 20.0 {
                (d.pago_minimo * 2.0)
                    .max(d.pago_minimo * 1.75)
                    .max(interes_mensual * 2.0)
            } else if d.tasa_anual > 0.01 {
                d.pago_minimo * 1.75
            } else {
                d.pago_minimo
            };
            total_sugerido += pago_sugerido;

            let a_capital_min = d.pago_minimo - interes_mensual;
            let _a_capital_sug = pago_sugerido - interes_mensual;

            let estado = if es_predatoria {
                "⛔ CRECE".red().bold().to_string()
            } else if d.tasa_anual >= 20.0 {
                "⚠️  PREDATORIA".yellow().bold().to_string()
            } else if interes_mensual > 0.01 && a_capital_min < interes_mensual * 0.3 {
                "⚠️  Lenta".yellow().to_string()
            } else if d.tasa_anual < 0.01 {
                "✅ Sin int.".green().to_string()
            } else {
                "✅ Bajando".green().to_string()
            };

            let nombre_corto = if d.nombre.len() > 21 {
                format!("{}…", &d.nombre[..20])
            } else {
                d.nombre.clone()
            };

            let capital_str = if a_capital_min < 0.0 {
                format!("-${:.0}", a_capital_min.abs()).red().to_string()
            } else {
                format!("${:.0}", a_capital_min).to_string()
            };

            println!(
                "  {:<22} {:>11} {:>6.1}% {:>9} {:>10} {:>10} {:>10} {}",
                nombre_corto,
                format!("${:.2}", saldo),
                d.tasa_anual,
                format!("${:.0}", interes_mensual),
                format!("${:.0}", d.pago_minimo),
                format!("${:.0}", pago_sugerido).green(),
                capital_str,
                estado
            );

            let tag = if es_predatoria {
                " ⛔ CRECE"
            } else if d.tasa_anual >= 20.0 {
                " ⚠️"
            } else {
                ""
            };
            opciones_menu.push(format!("{}  ${:.2}{}", d.nombre, saldo, tag));
        }
        println!("  {}", "─".repeat(100));

        // Totales
        let total_saldo: f64 = deudas_con_interes
            .iter()
            .map(|(_, d)| d.saldo_actual())
            .sum();
        let total_interes: f64 = deudas_con_interes
            .iter()
            .map(|(_, d)| d.saldo_actual() * d.tasa_efectiva() / 100.0 / 12.0)
            .sum();
        let total_minimos: f64 = deudas_con_interes.iter().map(|(_, d)| d.pago_minimo).sum();

        println!(
            "  {:<22} {:>11} {:>7} {:>9} {:>10} {:>10}",
            "TOTALES",
            format!("${:.2}", total_saldo).red().bold(),
            "",
            format!("${:.0}", total_interes).red(),
            format!("${:.0}", total_minimos).yellow(),
            format!("${:.0}", total_sugerido).green().bold()
        );
        println!();

        // Warning box siempre visible
        println!(
            "  {}",
            "┌──────────────────────────────────────────────────────────────────┐".yellow()
        );
        println!(
            "  {} ⚠️  REGLA DE ORO: Pagar SIEMPRE el DOBLE del mínimo o +75%{}  {}",
            "│".yellow(),
            " ".repeat(5),
            "│".yellow()
        );
        println!(
            "  {} El pago mínimo es una TRAMPA — solo alimenta intereses{}     {}",
            "│".yellow(),
            " ".repeat(5),
            "│".yellow()
        );
        println!(
            "  {}",
            "├──────────────────────────────────────────────────────────────────┤".yellow()
        );
        // Show each card's minimum as warning
        for (_, d) in &deudas_con_interes {
            if d.tasa_anual >= 20.0 {
                let int_m = d.saldo_actual() * d.tasa_efectiva() / 100.0 / 12.0;
                let sug = (d.pago_minimo * 2.0)
                    .max(d.pago_minimo * 1.75)
                    .max(int_m * 2.0);
                let crece = if d.pago_minimo < int_m {
                    " ⛔ CRECE"
                } else {
                    ""
                };
                println!(
                    "  {} {:<20} mín: ${:<8.0} → sugerido: ${:<8.0} (int: ${:.0}/mes){}{}",
                    "│".yellow(),
                    d.nombre,
                    d.pago_minimo,
                    sug,
                    int_m,
                    crece,
                    format!("{:>width$}│", "", width = 1).yellow()
                );
            }
        }
        println!(
            "  {}",
            "└──────────────────────────────────────────────────────────────────┘".yellow()
        );

        if total_interes > total_minimos * 0.4 {
            println!();
            println!(
                "  🚨 De los ${:.0} en pagos mínimos, ${:.0} ({:.0}%) se va SOLO a intereses.",
                total_minimos,
                total_interes,
                (total_interes / total_minimos) * 100.0
            );
            println!(
                "     Pagando los sugeridos (${:.0}/mes), más dinero iría a reducir la deuda.",
                total_sugerido
            );
        }
        println!();

        opciones_menu.push("🔙  Volver".to_string());
        let opciones_ref: Vec<&str> = opciones_menu.iter().map(|s| s.as_str()).collect();

        match menu("¿Qué deuda deseas revisar?", &opciones_ref) {
            Some(i) if i < deudas_con_interes.len() => {
                let (_, deuda) = deudas_con_interes[i];
                mostrar_analisis_deuda_individual(deuda);
            }
            _ => return,
        }
    }
}

fn mostrar_analisis_deuda_individual(d: &omniplanner::ml::advisor::DeudaRastreada) {
    let saldo = d.saldo_actual();
    let tasa_mensual = d.tasa_efectiva() / 100.0 / 12.0;
    let interes_mensual = saldo * tasa_mensual;
    let es_predatoria = d.pago_minimo < interes_mensual && d.tasa_anual > 0.01;
    let pago_para_empatar = interes_mensual * 1.005;
    // Regla de oro: doble del mínimo o +75%, lo que sea mayor; nunca menos que 2x el interés
    let pago_sugerido = if d.tasa_anual >= 20.0 {
        (d.pago_minimo * 2.0)
            .max(d.pago_minimo * 1.75)
            .max(interes_mensual * 2.0)
    } else if d.tasa_anual > 0.01 {
        d.pago_minimo * 1.75
    } else {
        d.pago_minimo
    };

    loop {
        limpiar();

        // ── Encabezado ──
        if es_predatoria {
            println!(
                "{}",
                "╔══════════════════════════════════════════════════════════════╗".red()
            );
            println!(
                "{}",
                format!("║  ⛔ DEUDA PREDATORIA: {:<38}║", d.nombre)
                    .red()
                    .bold()
            );
            println!(
                "{}",
                "║  El pago mínimo NO cubre los intereses — la deuda CRECE    ║".red()
            );
            println!(
                "{}",
                "╚══════════════════════════════════════════════════════════════╝".red()
            );
        } else if d.tasa_anual >= 20.0 {
            println!(
                "{}",
                "╔══════════════════════════════════════════════════════════════╗".yellow()
            );
            println!(
                "{}",
                format!("║  ⚠️  TASA PREDATORIA: {:<37}║", d.nombre)
                    .yellow()
                    .bold()
            );
            println!(
                "{}",
                "║  Tasa ≥20% — cada mes que pase es dinero regalado al banco  ║".yellow()
            );
            println!(
                "{}",
                "╚══════════════════════════════════════════════════════════════╝".yellow()
            );
        } else {
            println!(
                "{}",
                "╔══════════════════════════════════════════════════════════════╗".cyan()
            );
            println!(
                "{}",
                format!("║  🔍 ANÁLISIS: {:<45}║", d.nombre).cyan().bold()
            );
            println!(
                "{}",
                "╚══════════════════════════════════════════════════════════════╝".cyan()
            );
        }

        // ── WARNING: Pago mínimo siempre visible ──
        println!();
        println!(
            "  {}",
            "┌──────────────────────────────────────────────────────────────┐".yellow()
        );
        println!(
            "  {}  ⚠️  PAGO MÍNIMO:  {}    ←  esto es lo que pide el banco{}",
            "│".yellow(),
            format!("${:.2}", d.pago_minimo).red().bold(),
            format!("{:>width$}│", "", width = 3).yellow()
        );
        println!(
            "  {}  💰 PAGO SUGERIDO: {}    ←  mínimo para avanzar de verdad{}",
            "│".yellow(),
            format!("${:.2}", pago_sugerido).green().bold(),
            format!("{:>width$}│", "", width = 1).yellow()
        );
        if es_predatoria {
            println!(
                "  {}  🛑 PARA EMPATAR:  {}    ←  solo para que DEJE de crecer{}",
                "│".yellow(),
                format!("${:.2}", pago_para_empatar).yellow().bold(),
                format!("{:>width$}│", "", width = 1).yellow()
            );
        }
        println!(
            "  {}",
            "└──────────────────────────────────────────────────────────────┘".yellow()
        );

        // ── Sección 1: Radiografía ──
        println!();
        println!("  📋 RADIOGRAFÍA DE LA DEUDA");
        println!("  {}", "─".repeat(60));
        println!(
            "  Saldo actual:           {}",
            format!("${:.2}", saldo).red().bold()
        );
        println!(
            "  Tasa anual:             {}  (todas las tarjetas al 30% son predatorias)",
            format!("{:.1}%", d.tasa_anual).red()
        );
        println!("  Tasa mensual:           {:.2}%", tasa_mensual * 100.0);
        println!(
            "  Intereses que genera:   {} cada mes",
            format!("${:.2}", interes_mensual).red().bold()
        );
        println!(
            "  Intereses al año:       {} — dinero regalado al banco",
            format!("${:.2}", interes_mensual * 12.0).red()
        );
        println!(
            "  Pago mínimo del banco:  {} ← NO pagues solo esto",
            format!("${:.2}", d.pago_minimo).yellow()
        );
        println!(
            "  Pago sugerido (×2/+75%):{}  ← MÍNIMO recomendado",
            format!("${:.2}", pago_sugerido).green().bold()
        );

        if es_predatoria {
            let deficit = interes_mensual - d.pago_minimo;
            println!();
            println!(
                "  ⛔ ALERTA CRÍTICA: Pagando el mínimo de ${:.2}, la deuda SUBE ${:.2}/mes",
                d.pago_minimo, deficit
            );
            println!(
                "    → En 12 meses habrás pagado ${:.2} y la deuda habrá SUBIDO",
                d.pago_minimo * 12.0
            );
            println!(
                "    → Necesitas pagar al menos {} para que deje de crecer",
                format!("${:.2}", pago_para_empatar).yellow().bold()
            );
            println!(
                "    → Con el sugerido de {} empezarías a reducirla de verdad",
                format!("${:.2}", pago_sugerido).green().bold()
            );
        } else if d.tasa_anual > 0.01 {
            let a_capital_min = d.pago_minimo - interes_mensual;
            let a_capital_sug = pago_sugerido - interes_mensual;
            let pct_interes = (interes_mensual / d.pago_minimo) * 100.0;
            println!();
            println!("  Pagando el mínimo de ${:.2}:", d.pago_minimo);
            println!(
                "    → ${:.2} ({:.0}%) se va a intereses (dinero regalado al banco)",
                interes_mensual, pct_interes
            );
            println!(
                "    → ${:.2} ({:.0}%) reduce tu deuda realmente",
                a_capital_min,
                100.0 - pct_interes
            );
            println!();
            println!(
                "  Pagando el sugerido de {}:",
                format!("${:.2}", pago_sugerido).green()
            );
            println!(
                "    → ${:.2} iría a capital — {:.1}× más rápido que con el mínimo",
                a_capital_sug,
                if a_capital_min > 0.01 {
                    a_capital_sug / a_capital_min
                } else {
                    0.0
                }
            );
        }

        // ── Sección 2: Tabla comparativa de pagos ──
        println!();
        println!("  💰 COMPARACIÓN DE PAGOS — ¿Cuánto debería pagar?");
        println!("  {}", "─".repeat(60));

        // Generar opciones: mínimo, empatar, sugerido, doble, triple, por meses
        let mut montos: Vec<(String, f64)> = Vec::new();

        montos.push(("⛔ Pago mínimo (trampa)".to_string(), d.pago_minimo));

        if es_predatoria {
            montos.push((
                "🛑 Para detener crecimiento".to_string(),
                pago_para_empatar.ceil(),
            ));
        }

        // Pago sugerido (+75% / doble)
        montos.push(("💰 SUGERIDO (×2 / +75%)".to_string(), pago_sugerido));

        // Calcular montos estratégicos
        let opciones_monto = [("Triple del mínimo", d.pago_minimo * 3.0)];
        for (nombre, monto) in &opciones_monto {
            if *monto > pago_sugerido + 10.0
                && !montos.iter().any(|(_, m)| (*m - *monto).abs() < 10.0)
            {
                montos.push((nombre.to_string(), *monto));
            }
        }

        // Pago para salir en X meses (búsqueda simple)
        for target_meses in [12u32, 24, 36, 48] {
            let pago_necesario = calcular_pago_para_meses(saldo, tasa_mensual, target_meses);
            if pago_necesario > d.pago_minimo
                && pago_necesario < saldo
                && !montos
                    .iter()
                    .any(|(_, m)| (*m - pago_necesario).abs() < 10.0)
            {
                montos.push((
                    format!("Liquidar en {} meses", target_meses),
                    pago_necesario,
                ));
            }
        }

        // Pago total (liquidar ya)
        montos.push(("Pago total (liquidar ya)".to_string(), saldo));

        // Ordenar por monto
        montos.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        // Tabla
        println!(
            "  ┌──────────────────────────────┬──────────┬────────┬──────────────┬──────────────┬─────────────┐"
        );
        println!(
            "  │ {:<28} │ {:>8} │ {:>6} │ {:>12} │ {:>12} │ {:>11} │",
            "Estrategia", "Pago/mes", "Meses", "Intereses", "Total pagado", "Costo extra"
        );
        println!(
            "  ├──────────────────────────────┼──────────┼────────┼──────────────┼──────────────┼─────────────┤"
        );

        let mut resultados: Vec<(String, f64, u32, f64, f64)> = Vec::new();
        for (nombre, monto) in &montos {
            let (meses, total_int, total_pag) = simular_pagos_simple(saldo, tasa_mensual, *monto);
            resultados.push((nombre.clone(), *monto, meses, total_int, total_pag));
        }

        let costo_minimo = resultados
            .last()
            .map(|(_, _, _, _, tp)| *tp)
            .unwrap_or(saldo);

        for (nombre, monto, meses, total_int, total_pag) in &resultados {
            let costo_extra = total_pag - costo_minimo;
            let meses_str = if *meses >= 600 {
                "∞".to_string()
            } else {
                format!("{}", meses)
            };

            let nombre_corto = if nombre.len() > 28 {
                format!("{}…", &nombre[..27])
            } else {
                nombre.clone()
            };

            // Indicador visual
            let indicador = if *meses >= 600 {
                " ⛔"
            } else if *meses > 60 {
                " ⚠️ "
            } else if *meses <= 24 {
                " ✅"
            } else {
                ""
            };

            println!(
                "  │ {:<28} │ {:>8} │ {:>5}{} │ {:>12} │ {:>12} │ {:>11} │",
                nombre_corto,
                format!("${:.0}", monto),
                meses_str,
                if indicador.is_empty() { " " } else { indicador },
                format!("${:.2}", total_int),
                format!("${:.2}", total_pag),
                if costo_extra > 0.5 {
                    format!("+${:.0}", costo_extra)
                } else {
                    "—".to_string()
                }
            );
        }

        println!(
            "  └──────────────────────────────┴──────────┴────────┴──────────────┴──────────────┴─────────────┘"
        );

        println!();
        println!("  💡 \"Costo extra\" = cuánto más pagas en total vs liquidar de inmediato.");
        println!("     Cada dólar en esa columna es dinero regalado al banco.");

        // ── Sección 3: Historial ──
        if !d.historial.is_empty() {
            println!();
            println!("  📅 HISTORIAL DE PAGOS REGISTRADOS");
            println!("  {}", "─".repeat(60));
            println!(
                "  {:<12} {:>12} {:>10} {:>10} {:>10} {:>12}",
                "Mes", "Saldo ini.", "Pago", "Interés", "Cargos", "Saldo fin."
            );
            println!("  {}", "─".repeat(68));
            for m in &d.historial {
                println!(
                    "  {:<12} {:>12} {:>10} {:>10} {:>10} {:>12}",
                    m.mes,
                    format!("${:.2}", m.saldo_inicio),
                    format!("${:.2}", m.pago),
                    format!("${:.2}", m.intereses),
                    format!("${:.2}", m.nuevos_cargos),
                    format!("${:.2}", m.saldo_final)
                );
            }
            println!("  {}", "─".repeat(68));
            let total_pagado: f64 = d.historial.iter().map(|m| m.pago).sum();
            let total_interes: f64 = d.historial.iter().map(|m| m.intereses).sum();
            println!(
                "  Total pagado: {}  |  Total en intereses: {}  |  Eficiencia: {:.0}%",
                format!("${:.2}", total_pagado).green(),
                format!("${:.2}", total_interes).red(),
                if total_pagado > 0.01 {
                    ((total_pagado - total_interes) / total_pagado) * 100.0
                } else {
                    0.0
                }
            );
        }

        // ── Sub-menú ──
        println!();
        let sub_opciones = &[
            "📊  Ver proyección mes a mes con un monto específico",
            "�  Ver proyección con el pago SUGERIDO",
            "🔙  Volver al listado de deudas",
        ];

        match menu("¿Qué deseas hacer?", sub_opciones) {
            Some(0) => {
                let monto = pedir_f64("Monto de pago mensual a proyectar ($)", pago_sugerido);
                let max_m = pedir_f64("¿Cuántos meses proyectar? (máx)", 60.0) as u32;
                mostrar_proyeccion_individual(d, monto, max_m);
            }
            Some(1) => {
                mostrar_proyeccion_individual(d, pago_sugerido, 60);
            }
            _ => return,
        }
    }
}

/// Calcula el pago mensual fijo necesario para liquidar una deuda en X meses.
fn calcular_pago_para_meses(saldo: f64, tasa_mensual: f64, meses: u32) -> f64 {
    if tasa_mensual < 0.0001 {
        return saldo / meses as f64;
    }
    // Fórmula de amortización: P = S * [r(1+r)^n] / [(1+r)^n - 1]
    let r = tasa_mensual;
    let n = meses as f64;
    let factor = r * (1.0 + r).powf(n);
    let denom = (1.0 + r).powf(n) - 1.0;
    if denom.abs() < 0.0001 {
        return saldo / meses as f64;
    }
    (saldo * factor / denom).ceil()
}

/// Simula pagos fijos mensuales y devuelve (meses, total_intereses, total_pagado).
fn simular_pagos_simple(saldo_inicial: f64, tasa_mensual: f64, monto: f64) -> (u32, f64, f64) {
    let mut saldo = saldo_inicial;
    let mut total_int = 0.0;
    let mut total_pag = 0.0;
    let mut meses = 0u32;

    while saldo > 0.01 && meses < 600 {
        let interes = saldo * tasa_mensual;
        total_int += interes;
        saldo += interes;
        let pago = monto.min(saldo);
        saldo -= pago;
        total_pag += pago;
        meses += 1;
    }
    (meses, total_int, total_pag)
}

/// Muestra proyección mes a mes para una deuda con un monto de pago dado.
fn mostrar_proyeccion_individual(
    d: &omniplanner::ml::advisor::DeudaRastreada,
    monto: f64,
    max_meses: u32,
) {
    let saldo_ini = d.saldo_actual();
    let tasa_mensual = d.tasa_efectiva() / 100.0 / 12.0;
    let interes_mes1 = saldo_ini * tasa_mensual;

    limpiar();
    separador(&format!(
        "📊 PROYECCIÓN: {} — pagando ${:.2}/mes",
        d.nombre, monto
    ));

    if monto <= interes_mes1 && d.tasa_anual > 0.01 {
        println!();
        println!(
            "  ⛔ Con ${:.2}/mes NO cubres los intereses de ${:.2}/mes.",
            monto, interes_mes1
        );
        println!("  La deuda crecerá indefinidamente. Necesitas pagar más.");
        println!();
    }

    println!();
    println!(
        "  {:<5} {:>12} {:>10} {:>12} {:>12} {:>14}",
        "Mes", "Saldo", "Pago", "→ Interés", "→ Capital", "Int. acum."
    );
    println!("  {}", "─".repeat(70));

    let mut saldo = saldo_ini;
    let mut int_acum = 0.0;

    for mes in 1..=max_meses {
        if saldo < 0.01 {
            break;
        }
        let interes = saldo * tasa_mensual;
        int_acum += interes;
        saldo += interes;
        let pago = monto.min(saldo);
        let a_capital = pago - interes;
        saldo -= pago;
        if saldo < 0.01 {
            saldo = 0.0;
        }

        // Colorear: rojo si a_capital negativo, verde si positivo
        let capital_str = if a_capital < 0.0 {
            format!("-${:.2}", a_capital.abs()).red().to_string()
        } else {
            format!("${:.2}", a_capital).green().to_string()
        };

        println!(
            "  {:<5} {:>12} {:>10} {:>12} {:>12} {:>14}",
            mes,
            format!("${:.2}", saldo),
            format!("${:.2}", pago),
            format!("${:.2}", interes),
            capital_str,
            format!("${:.2}", int_acum)
        );

        if saldo < 0.01 {
            println!();
            println!(
                "  🎉 ¡Deuda liquidada en {} meses! Total intereses: ${:.2}",
                mes, int_acum
            );
            break;
        }
    }

    if saldo > 0.01 {
        println!("  {}", "─".repeat(70));
        println!(
            "  Después de {} meses: Saldo restante ${:.2}  |  Intereses pagados: ${:.2}",
            max_meses, saldo, int_acum
        );
    }

    println!();
    pausa();
}

fn rastreador_agregar_deuda(state: &mut AppState) {
    limpiar();
    separador("➕ AGREGAR DEUDA AL RASTREADOR");

    let nombre = match pedir_texto("Nombre de la cuenta (ej: Discover, BOFA, Renta, Seguro)") {
        Some(n) => n,
        None => return,
    };

    // Preguntar tipo PRIMERO — el flujo cambia según la respuesta
    let tipos_deuda = &[
        "💳  Tarjeta de crédito / línea de crédito",
        "🏠  Préstamo con interés (mortgage, carro, préstamo personal)",
        "🧊  Compra diferida a meses sin intereses (Dell, Best Buy, etc.)",
        "🔒  Pago corriente / fijo (renta, seguro, suscripción — sin intereses, se paga completo)",
    ];
    let tipo = match menu("Tipo de deuda", tipos_deuda) {
        Some(t) => t,
        _ => return,
    };

    let (saldo, tasa, pago_min, es_obligatoria);
    let mut meses_gracia: usize = 0;

    match tipo {
        3 => {
            // Pago corriente: renta, seguro, suscripción — tasa 0, pago = monto completo
            es_obligatoria = true;
            tasa = 0.0;
            pago_min = pedir_f64("Monto mensual fijo ($)", 0.0);
            saldo = pago_min;

            println!();
            println!(
                "    🔒 Pago corriente: ${:.2}/mes — sin intereses, se paga en su totalidad.",
                pago_min
            );
        }
        2 => {
            // Compra diferida a meses sin intereses
            es_obligatoria = false;
            saldo = pedir_f64("Saldo actual ($)", 0.0);
            tasa = pedir_f64(
                "Tasa de interés ANUAL que aplica DESPUÉS del periodo gratis (%) (ej: 29.99)",
                0.0,
            );
            println!();
            println!("  ¿Cuántos meses SIN INTERESES te quedan?");
            println!("  (Ej: si compraste a 12 meses y ya van 6, quedan 6)");
            meses_gracia = pedir_f64("Meses restantes sin intereses", 0.0).max(0.0) as usize;

            let pago_sugerido = if meses_gracia > 0 {
                saldo / meses_gracia as f64
            } else {
                25.0
            };
            pago_min = pedir_f64(
                &format!(
                    "Pago mínimo mensual (${:.2} para liquidar en el plazo)",
                    pago_sugerido
                ),
                pago_sugerido,
            );

            println!();
            println!(
                "    🧊 {} meses restantes a 0% — después aplica {:.1}% anual.",
                meses_gracia, tasa
            );
            if pago_min * meses_gracia as f64 >= saldo - 0.01 {
                println!(
                    "    ✅ Con ${:.2}/mes la liquidas antes de que empiecen intereses. ✓",
                    pago_min
                );
            } else {
                println!(
                    "    ⚠️ ¡Cuidado! Con ${:.2}/mes quedarán ${:.2} cuando empiecen intereses al {:.1}%.",
                    pago_min,
                    saldo - (pago_min * meses_gracia as f64),
                    tasa
                );
            }
        }
        1 => {
            // Préstamo con interés (mortgage, carro) — obligatoria, con tasa
            es_obligatoria = true;
            saldo = pedir_f64("Saldo actual del préstamo ($)", 0.0);
            tasa = pedir_f64("Tasa de interés ANUAL (%) (ej: 6.5)", 0.0);
            pago_min = pedir_f64("Pago mensual ($)", 0.0);

            println!("    🔒 Préstamo fijo — el diagnóstico alertará si falla un pago.");
        }
        _ => {
            // Tarjeta de crédito — no obligatoria, con tasa y pago mínimo
            es_obligatoria = false;
            saldo = pedir_f64("Saldo actual ($)", 0.0);
            tasa = pedir_f64("Tasa de interés ANUAL (%) (ej: 24.99)", 0.0);
            pago_min = pedir_f64("Pago mínimo mensual ($)", 25.0);
        }
    }

    let mut deuda = DeudaRastreada::nueva(&nombre, tasa, pago_min);
    deuda.obligatoria = es_obligatoria;
    deuda.meses_gracia = meses_gracia;

    // Solo ofrecer historial para deudas con saldo real (no pagos corrientes)
    if tipo != 3 {
        let cargar_hist = Confirm::new()
            .with_prompt("  ¿Quieres cargar meses anteriores de pago?")
            .default(false)
            .interact()
            .unwrap_or(false);

        if cargar_hist {
            println!();
            println!("  📅 Ingresa los datos mes por mes (vacío para terminar).");
            let mut saldo_actual = saldo;

            loop {
                let mes = pedir_texto_opcional(&format!(
                    "Mes {} (ej: Ene 2021, vacío=terminar)",
                    deuda.historial.len() + 1
                ));
                if mes.is_empty() {
                    break;
                }

                let saldo_inicio = pedir_f64(
                    &format!("  Saldo al inicio del mes (${:.2} sugerido)", saldo_actual),
                    saldo_actual,
                );
                let pago = pedir_f64("  Pago realizado ($)", 0.0);
                let cargos = pedir_f64("  Nuevos cargos/compras ($)", 0.0);

                deuda.registrar_mes(&mes, saldo_inicio, pago, cargos);
                saldo_actual = deuda.saldo_actual();

                println!(
                    "    {} {} — Saldo final: ${:.2}",
                    "✓".green(),
                    mes,
                    saldo_actual
                );
            }
        } else {
            let hoy = Local::now().format("%b %Y").to_string();
            deuda.registrar_mes(&hoy, saldo, 0.0, 0.0);
        }
    } else {
        // Pago corriente: registrar un mes con su monto como saldo
        let hoy = Local::now().format("%b %Y").to_string();
        deuda.registrar_mes(&hoy, saldo, 0.0, 0.0);
    }

    println!();
    let sufijo = if tipo == 3 {
        "/mes (pago corriente)".to_string()
    } else if deuda.meses_gracia > 0 {
        format!(" (🧊 {} meses sin intereses)", deuda.meses_gracia)
    } else {
        String::new()
    };
    println!(
        "  {} '{}' agregada — ${:.2}{}",
        "✓".green(),
        nombre,
        deuda.saldo_actual(),
        sufijo
    );

    state.asesor.rastreador.agregar_deuda(deuda);
    pausa();
}

fn rastreador_registrar_mes(state: &mut AppState) {
    if state.asesor.rastreador.deudas.is_empty() {
        println!("  Sin deudas registradas.");
        pausa();
        return;
    }

    let nombres: Vec<String> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .map(|d| {
            format!(
                "{} — ${:.2}{}",
                d.nombre,
                d.saldo_actual(),
                if d.activa { "" } else { " ✅ (pagada)" }
            )
        })
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    if let Some(idx) = menu("¿A cuál deuda registrar pago?", &refs) {
        let d = &state.asesor.rastreador.deudas[idx];
        let es_corriente = d.es_pago_corriente();
        let saldo_act = d.saldo_actual();
        let pago_min = d.pago_minimo;

        let mes = pedir_texto_opcional("Mes (ej: Mar 2024, vacío=mes actual)");
        let mes = if mes.is_empty() {
            Local::now().format("%b %Y").to_string()
        } else {
            mes
        };

        if es_corriente {
            // Pago corriente: el saldo siempre es el monto fijo, se paga completo
            let pago = pedir_f64(
                &format!("Pago realizado (${:.2} = monto completo)", pago_min),
                pago_min,
            );
            state.asesor.rastreador.deudas[idx].registrar_mes(&mes, pago_min, pago, 0.0);
            println!();
            if (pago - pago_min).abs() < 0.01 {
                println!("  ✅ {} — Pago corriente ${:.2} registrado ✓", mes, pago);
            } else {
                println!(
                    "  ⚠️ {} — Pagaste ${:.2} de ${:.2} (faltaron ${:.2})",
                    mes,
                    pago,
                    pago_min,
                    (pago_min - pago).max(0.0)
                );
            }
        } else {
            let saldo_inicio = pedir_f64(
                &format!("Saldo al inicio (${:.2} sugerido)", saldo_act),
                saldo_act,
            );
            let pago = pedir_f64("Pago realizado ($)", 0.0);
            let cargos = pedir_f64("Nuevos cargos/compras ($)", 0.0);

            state.asesor.rastreador.deudas[idx].registrar_mes(&mes, saldo_inicio, pago, cargos);

            let nuevo_saldo = state.asesor.rastreador.deudas[idx].saldo_actual();
            println!();
            if nuevo_saldo < saldo_act {
                println!(
                    "  ✅ {} — Saldo: ${:.2} → ${:.2} (bajó ${:.2})",
                    mes,
                    saldo_act,
                    nuevo_saldo,
                    saldo_act - nuevo_saldo
                );
            } else {
                println!(
                    "  ⚠️ {} — Saldo: ${:.2} → ${:.2} (subió ${:.2})",
                    mes,
                    saldo_act,
                    nuevo_saldo,
                    nuevo_saldo - saldo_act
                );
            }
        }
        pausa();
    }
}

fn rastreador_diagnostico(state: &AppState) {
    if state.asesor.rastreador.deudas.is_empty() {
        println!("  Sin deudas registradas.");
        pausa();
        return;
    }

    limpiar();
    separador("📊 DIAGNÓSTICO COMPLETO DE DEUDAS");

    let diag = state.asesor.rastreador.diagnosticar();

    // Resumen general
    println!();
    println!("  ┌─────────────────────────────────────────┐");
    println!("  │         RESUMEN GENERAL                  │");
    println!("  ├─────────────────────────────────────────┤");
    println!(
        "  │  Deuda inicial total:  {:>16}  │",
        format!("${:.2}", diag.deuda_inicial_total)
    );
    println!(
        "  │  Deuda actual total:   {:>16}  │",
        format!("${:.2}", diag.deuda_final_total)
    );

    let cambio_str = if diag.cambio_neto > 0.0 {
        format!("+${:.2} ⛔", diag.cambio_neto)
    } else if diag.cambio_neto < 0.0 {
        format!("-${:.2} ✅", diag.cambio_neto.abs())
    } else {
        "Sin cambio".to_string()
    };
    println!("  │  Cambio neto:          {:>16}  │", cambio_str);
    println!(
        "  │  Total pagado:         {:>16}  │",
        format!("${:.2}", diag.total_pagado)
    );
    println!(
        "  │  Intereses estimados:  {:>16}  │",
        format!("${:.2}", diag.total_intereses_estimados)
    );
    println!(
        "  │  Nuevos cargos:        {:>16}  │",
        format!("${:.2}", diag.total_nuevos_cargos)
    );
    println!(
        "  │  Meses analizados:     {:>16}  │",
        diag.meses_analizados
    );
    println!("  └─────────────────────────────────────────┘");

    // Resumen por deuda
    println!();
    println!("  📋 Desglose por cuenta:");
    println!();
    println!(
        "  {:<22} {:>10} {:>10} {:>10} {:>10} Tendencia",
        "Cuenta", "Inicio", "Actual", "Pagado", "Cargos"
    );
    println!("  {}", "─".repeat(85));
    for r in &diag.resumen_por_deuda {
        println!(
            "  {:<22} {:>10} {:>10} {:>10} {:>10} {}",
            if r.nombre.len() > 21 {
                format!("{}…", &r.nombre[..20])
            } else {
                r.nombre.clone()
            },
            format!("${:.0}", r.saldo_inicial),
            format!("${:.0}", r.saldo_final),
            format!("${:.0}", r.total_pagado),
            format!("${:.0}", r.total_cargos),
            r.tendencia
        );
    }
    println!("  {}", "─".repeat(85));

    // Errores detectados (solo los más graves)
    let errores_graves: Vec<_> = diag
        .errores
        .iter()
        .filter(|e| {
            matches!(
                e.error,
                omniplanner::ml::advisor::ErrorPago::SiguioUsandoTarjeta
                    | omniplanner::ml::advisor::ErrorPago::NoPagoNada
                    | omniplanner::ml::advisor::ErrorPago::PagoInsuficiente
            )
        })
        .collect();

    if !errores_graves.is_empty() {
        println!();
        println!(
            "  ⚠️  {} errores/advertencias detectados:",
            errores_graves.len()
        );
        println!();
        // Mostrar máximo 15 errores para no saturar
        for (i, e) in errores_graves.iter().take(15).enumerate() {
            println!(
                "    {}. {} {} [{}] {}",
                i + 1,
                e.error.emoji(),
                e.deuda,
                e.mes,
                e.nota
            );
        }
        if errores_graves.len() > 15 {
            println!("    ... y {} más", errores_graves.len() - 15);
        }
    }

    // Recomendaciones
    if !diag.recomendaciones.is_empty() {
        println!();
        println!("  💡 RECOMENDACIONES:");
        println!();
        for rec in &diag.recomendaciones {
            println!("    {}", rec);
        }
    }

    pausa();
}

fn rastreador_simulacion(state: &AppState) {
    if state.asesor.rastreador.deudas.is_empty() {
        println!("  Sin deudas registradas.");
        pausa();
        return;
    }

    let nombres: Vec<String> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .map(|d| format!("{} — ${:.2}", d.nombre, d.saldo_actual()))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    if let Some(idx) = menu("¿Simular cuál deuda?", &refs) {
        let d = &state.asesor.rastreador.deudas[idx];
        if d.historial.is_empty() {
            println!("  Esta deuda no tiene historial aún.");
            pausa();
            return;
        }

        limpiar();
        separador(&format!("📈 SIMULACIÓN: {}", d.nombre));

        println!("  🔄 Real vs Alternativa");
        println!();

        let pago_alt = pedir_f64(
            "¿Cuánto hubieras querido pagar por mes? ($)",
            d.pago_minimo * 2.0,
        );

        let alt = d.simular_alternativa(pago_alt);

        // Mostrar tabla comparativa
        println!();
        println!(
            "  {:<10} {:>12} {:>10} {:>12} {:>10}",
            "Mes", "Real", "Pago.R", "Alternativa", "Pago.A"
        );
        println!("  {}", "─".repeat(60));

        let max_filas = d.historial.len().max(alt.len());
        for i in 0..max_filas {
            let real = d.historial.get(i);
            let sim = alt.get(i);
            println!(
                "  {:<10} {:>12} {:>10} {:>12} {:>10}",
                real.map(|m| m.mes.as_str()).unwrap_or("-"),
                real.map(|m| format!("${:.2}", m.saldo_final))
                    .unwrap_or_default(),
                real.map(|m| format!("${:.2}", m.pago)).unwrap_or_default(),
                sim.map(|m| format!("${:.2}", m.saldo_final))
                    .unwrap_or_default(),
                sim.map(|m| format!("${:.2}", m.pago)).unwrap_or_default(),
            );
        }
        println!("  {}", "─".repeat(60));

        let real_final = d.historial.last().map(|m| m.saldo_final).unwrap_or(0.0);
        let alt_final = alt.last().map(|m| m.saldo_final).unwrap_or(0.0);
        let real_pagado: f64 = d.historial.iter().map(|m| m.pago).sum();
        let alt_pagado: f64 = alt.iter().map(|m| m.pago).sum();

        println!();
        println!(
            "  Saldo final REAL:        {}",
            format!("${:.2}", real_final).red()
        );
        println!(
            "  Saldo final ALTERNATIVO: {}",
            if alt_final < real_final {
                format!("${:.2}", alt_final).green().to_string()
            } else {
                format!("${:.2}", alt_final).red().to_string()
            }
        );
        println!(
            "  Diferencia:              {}",
            format!("${:.2} menos", (real_final - alt_final).max(0.0)).green()
        );
        println!();
        println!(
            "  Total pagado REAL: ${:.2}  |  ALTERNATIVO: ${:.2}",
            real_pagado, alt_pagado
        );

        pausa();
    }
}

fn rastreador_simulacion_libertad(state: &AppState) {
    let deudas_reales: Vec<&DeudaRastreada> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .filter(|d| d.activa && !d.es_pago_corriente() && d.saldo_actual() > 0.01)
        .collect();

    let pagos_corrientes: Vec<&DeudaRastreada> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .filter(|d| d.activa && d.es_pago_corriente())
        .collect();

    if deudas_reales.is_empty() {
        println!("  No hay deudas activas (con saldo) para simular.");
        if !pagos_corrientes.is_empty() {
            println!(
                "  (Tienes {} pago(s) corriente(s) pero esos no se liquidan.)",
                pagos_corrientes.len()
            );
        }
        pausa();
        return;
    }

    limpiar();
    separador("🗺️  SIMULACIÓN: CAMINO A LA LIBERTAD FINANCIERA");

    let deuda_total: f64 = deudas_reales.iter().map(|d| d.saldo_actual()).sum();
    let ingreso_mensual = state.asesor.rastreador.ingreso_mensual_total();
    let minimos_deudas: f64 = deudas_reales.iter().map(|d| d.pago_minimo).sum();
    let total_corrientes: f64 = pagos_corrientes.iter().map(|d| d.pago_minimo).sum();

    // Mostrar pagos corrientes (gastos fijos)
    if !pagos_corrientes.is_empty() {
        println!();
        println!("  🔒 Pagos corrientes (se restan del presupuesto):");
        for d in &pagos_corrientes {
            println!(
                "     • {} — {}/mes",
                d.nombre,
                format!("${:.2}", d.pago_minimo).yellow()
            );
        }
        println!(
            "     Total gastos fijos: {}",
            format!("${:.2}", total_corrientes).yellow()
        );
    }

    // Mostrar deudas reales
    println!();
    println!("  📋 Deudas a liquidar: {}", deudas_reales.len());
    for d in &deudas_reales {
        let tag = if d.obligatoria { " 🔒" } else { "" };
        let gracia_tag = if d.meses_gracia > 0 {
            format!(" 🧊 {}m a 0%", d.meses_gracia)
        } else {
            String::new()
        };
        println!(
            "     • {} — Saldo: {} | Pago: ${:.2} | Tasa: {:.1}%{}{}",
            d.nombre,
            format!("${:.2}", d.saldo_actual()).red(),
            d.pago_minimo,
            d.tasa_anual,
            tag,
            gracia_tag,
        );
    }
    println!();
    println!(
        "  Deuda total:         {}",
        format!("${:.2}", deuda_total).red()
    );
    println!(
        "  Ingreso mensual:     {}",
        format!("${:.2}", ingreso_mensual).green()
    );
    if total_corrientes > 0.0 {
        println!(
            "  Gastos fijos:       -{}",
            format!("${:.2}", total_corrientes).yellow()
        );
        println!(
            "  Disponible p/deudas: {}",
            format!("${:.2}", (ingreso_mensual - total_corrientes).max(0.0)).cyan()
        );
    }
    println!(
        "  Pago mínimo deudas:  {}",
        format!("${:.2}", minimos_deudas).yellow()
    );
    println!();

    // Elegir estrategia
    let estrategias = &[
        "❄️  Avalancha (paga primero la tasa más alta — ahorra más en intereses)",
        "⛄ Bola de nieve (paga primero el saldo más bajo — victorias rápidas)",
    ];
    let bola_nieve = match menu("¿Qué estrategia usar?", estrategias) {
        Some(1) => true,
        Some(0) => false,
        _ => return,
    };

    // Monto mensual (incluye gastos fijos + deudas)
    let minimo_necesario = minimos_deudas + total_corrientes;
    let sugerido = if ingreso_mensual > minimo_necesario * 1.5 {
        minimo_necesario * 1.5
    } else {
        minimo_necesario
    };
    let presupuesto = pedir_f64(
        "¿Cuánto puedes destinar al mes en TOTAL? (deudas + gastos fijos) ($)",
        sugerido,
    );

    if presupuesto < minimo_necesario {
        println!();
        println!(
            "  ⚠️ El presupuesto (${:.2}) es menor que lo necesario (${:.2} fijos + ${:.2} mínimos).",
            presupuesto,
            total_corrientes,
            minimos_deudas
        );
        println!("  No se podrán cubrir todos los pagos.");
        println!();
    }

    let sim = state
        .asesor
        .rastreador
        .simular_libertad(presupuesto, bola_nieve);

    if sim.meses.is_empty() {
        println!("  No hay nada que simular.");
        pausa();
        return;
    }

    limpiar();
    separador(&format!(
        "📊 PLAN DE LIBERTAD — {} | ${:.2}/mes",
        sim.estrategia, sim.presupuesto_mensual
    ));

    // Mostrar gastos fijos descontados
    if !sim.gastos_fijos.is_empty() {
        println!();
        println!(
            "  🔒 Gastos fijos descontados: {} ({}/mes)",
            sim.gastos_fijos
                .iter()
                .map(|(n, m)| format!("{} ${:.0}", n, m))
                .collect::<Vec<_>>()
                .join(", "),
            format!("${:.2}", sim.total_gastos_fijos).yellow()
        );
        println!(
            "  💰 Presupuesto efectivo para deudas: {}/mes",
            format!("${:.2}", sim.presupuesto_mensual - sim.total_gastos_fijos).green()
        );
    }

    // Nombres de deudas
    let nombres: Vec<String> = if let Some(primer_mes) = sim.meses.first() {
        primer_mes.saldos.iter().map(|(n, _)| n.clone()).collect()
    } else {
        Vec::new()
    };

    // ═══════════════════════════════════════════════════════════
    // TABLA DE AMORTIZACIÓN DETALLADA — mes a mes, deuda por deuda
    // ═══════════════════════════════════════════════════════════
    println!();
    println!(
        "  {}",
        "══════════════════════════════════════════════════════════════════════".cyan()
    );
    println!(
        "  {}",
        "  TABLA DE AMORTIZACIÓN — Distribución de pagos mes a mes"
            .cyan()
            .bold()
    );
    println!(
        "  {}",
        "══════════════════════════════════════════════════════════════════════".cyan()
    );

    for mes in &sim.meses {
        println!();
        // Header del mes
        let pago_total_mes: f64 = mes.pagos.iter().map(|(_, p)| *p).sum();
        let interes_total_mes: f64 = mes.intereses.iter().map(|(_, i)| *i).sum();

        println!(
            "  ┌─── {} ──────────────────────────────────────────────┐",
            format!("MES {}", mes.mes_numero).bold()
        );

        // Línea 1: Presupuesto efectivo con detalle de liberados
        if mes.liberado_de_liquidadas > 0.01 {
            println!(
                "  │  Presupuesto: {} (base ${:.2} + {} liberados)",
                format!("${:.2}", mes.presupuesto_efectivo).green().bold(),
                mes.presupuesto_efectivo - mes.liberado_de_liquidadas,
                format!("${:.2}", mes.liberado_de_liquidadas).green(),
            );
        } else {
            println!(
                "  │  Presupuesto: {}",
                format!("${:.2}", mes.presupuesto_efectivo),
            );
        }

        // Línea 2: Pagos, intereses, deuda restante, sobrante
        println!(
            "  │  Pagos: {}  │  Intereses: {}  │  Deuda restante: {}{}",
            format!("${:.2}", pago_total_mes).green(),
            format!("${:.2}", interes_total_mes).red(),
            if mes.deuda_total < 0.01 {
                "$0.00".green().bold().to_string()
            } else {
                format!("${:.2}", mes.deuda_total)
            },
            if mes.sobrante > 0.01 {
                format!(
                    "  │  Sin asignar: {}",
                    format!("${:.2}", mes.sobrante).yellow()
                )
            } else {
                String::new()
            }
        );
        println!("  ├──────────────────────┬────────────┬────────────┬──────────────┤");
        println!(
            "  │ {:<20} │ {:>10} │ {:>10} │ {:>12} │",
            "Deuda", "Pago", "Interés", "Saldo"
        );
        println!("  ├──────────────────────┼────────────┼────────────┼──────────────┤");

        for (nombre, saldo) in &mes.saldos {
            let pago = mes
                .pagos
                .iter()
                .find(|(n, _)| n == nombre)
                .map(|(_, p)| *p)
                .unwrap_or(0.0);
            let interes = mes
                .intereses
                .iter()
                .find(|(n, _)| n == nombre)
                .map(|(_, i)| *i)
                .unwrap_or(0.0);

            let nombre_corto = if nombre.len() > 20 {
                format!("{}…", &nombre[..19])
            } else {
                nombre.clone()
            };

            if *saldo < 0.01 && pago < 0.01 {
                // Ya liquidada en un mes anterior
                println!(
                    "  │ {:<20} │ {:>10} │ {:>10} │ {:>12} │",
                    nombre_corto, "—", "—", "✅ $0.00"
                );
            } else if mes.liquidadas_este_mes.contains(nombre) {
                // Se liquidó ESTE mes
                println!(
                    "  │ {} │ {} │ {} │ {} │",
                    format!("{:<20}", nombre_corto).green().bold(),
                    format!("{:>10}", format!("${:.2}", pago)).green().bold(),
                    if interes > 0.01 {
                        format!("{:>10}", format!("${:.2}", interes))
                            .red()
                            .to_string()
                    } else {
                        format!("{:>10}", "$0.00")
                    },
                    format!("{:>12}", "🎉 $0.00").green().bold()
                );
            } else {
                // Deuda activa con pago
                let pago_str = if pago > 0.01 {
                    format!("${:.2}", pago)
                } else {
                    "$0.00".to_string()
                };
                let interes_str = if interes > 0.01 {
                    format!("${:.2}", interes)
                } else {
                    "$0.00".to_string()
                };
                println!(
                    "  │ {:<20} │ {:>10} │ {} │ {:>12} │",
                    nombre_corto,
                    pago_str,
                    if interes > 0.01 {
                        format!("{:>10}", interes_str).red().to_string()
                    } else {
                        format!("{:>10}", interes_str)
                    },
                    format!("${:.2}", saldo)
                );
            }
        }

        println!("  └──────────────────────┴────────────┴────────────┴──────────────┘");

        // Evento de liquidación
        if !mes.liquidadas_este_mes.is_empty() {
            for nombre in &mes.liquidadas_este_mes {
                let pago_final = mes
                    .pagos
                    .iter()
                    .find(|(n, _)| n == nombre)
                    .map(|(_, p)| *p)
                    .unwrap_or(0.0);
                println!(
                    "  {}",
                    format!(
                        "  🎉 ¡{} LIQUIDADA! → ${:.2}/mes liberados para las demás deudas.",
                        nombre.to_uppercase(),
                        pago_final
                    )
                    .green()
                    .bold()
                );
            }
        }
    }

    // ═══════════════════════════════════════════════════════════
    // RESUMEN FINAL
    // ═══════════════════════════════════════════════════════════
    println!();
    let total_meses = sim.meses.len();
    let anios = total_meses / 12;
    let meses_rest = total_meses % 12;
    let tiempo = if anios > 0 && meses_rest > 0 {
        format!("{} año(s) y {} mes(es)", anios, meses_rest)
    } else if anios > 0 {
        format!("{} año(s)", anios)
    } else {
        format!("{} mes(es)", meses_rest)
    };

    println!(
        "  {}",
        "══════════════════════════════════════════════════════════════════════".yellow()
    );
    println!(
        "  {}",
        "  👑  ¡LIBERTAD FINANCIERA ALCANZADA!  👑".green().bold()
    );
    println!(
        "  {}",
        "══════════════════════════════════════════════════════════════════════".yellow()
    );
    println!();
    println!("  ⏱️  Tiempo total:        {}", tiempo.green().bold());
    println!(
        "  💰 Total pagado:        {}",
        format!("${:.2}", sim.total_pagado).cyan()
    );
    println!(
        "  📈 Total en intereses:  {}",
        format!("${:.2}", sim.total_intereses).red()
    );
    println!(
        "  💵 Capital real pagado: {}",
        format!("${:.2}", sim.total_pagado - sim.total_intereses).green()
    );

    // Resumen por deuda: total pagado e intereses por cada una
    println!();
    println!("  {}", "  📋 RESUMEN POR DEUDA".cyan().bold());
    println!("  ┌──────────────────────┬────────────┬────────────┬────────────┬──────────┐");
    println!(
        "  │ {:<20} │ {:>10} │ {:>10} │ {:>10} │ {:>8} │",
        "Deuda", "Pagado", "Intereses", "Capital", "Mes liq."
    );
    println!("  ├──────────────────────┼────────────┼────────────┼────────────┼──────────┤");
    for nombre in &nombres {
        let total_pago_deuda: f64 = sim
            .meses
            .iter()
            .flat_map(|m| m.pagos.iter())
            .filter(|(n, _)| n == nombre)
            .map(|(_, p)| *p)
            .sum();
        let total_int_deuda: f64 = sim
            .meses
            .iter()
            .flat_map(|m| m.intereses.iter())
            .filter(|(n, _)| n == nombre)
            .map(|(_, i)| *i)
            .sum();
        let mes_liq = sim
            .orden_liquidacion
            .iter()
            .find(|(n, _)| n == nombre)
            .map(|(_, m)| format!("{}", m))
            .unwrap_or_else(|| "—".to_string());
        let nombre_corto = if nombre.len() > 20 {
            format!("{}…", &nombre[..19])
        } else {
            nombre.clone()
        };
        println!(
            "  │ {:<20} │ {:>10} │ {} │ {:>10} │ {:>8} │",
            nombre_corto,
            format!("${:.2}", total_pago_deuda),
            format!("{:>10}", format!("${:.2}", total_int_deuda)).red(),
            format!("${:.2}", total_pago_deuda - total_int_deuda),
            mes_liq
        );
    }
    println!("  └──────────────────────┴────────────┴────────────┴────────────┴──────────┘");

    // Orden de liquidación
    println!();
    println!("  {}", "  🗺️  ORDEN DE LIQUIDACIÓN".cyan().bold());
    for (i, (nombre, mes)) in sim.orden_liquidacion.iter().enumerate() {
        let emoji = if i == sim.orden_liquidacion.len() - 1 {
            "👑"
        } else {
            "✅"
        };
        let meses_txt = if *mes == 1 {
            "1 mes".to_string()
        } else {
            format!("{} meses", mes)
        };
        println!(
            "     {} {}. {} — liquidada en {} (mes {})",
            emoji,
            i + 1,
            nombre,
            meses_txt,
            mes
        );
    }
    println!();

    // Preguntar si desea exportar a Excel
    if Confirm::new()
        .with_prompt("¿Deseas exportar este reporte a Excel?")
        .default(true)
        .interact()
        .unwrap_or(false)
    {
        match exportar_simulacion_excel(&sim, &nombres) {
            Ok(ruta) => {
                println!();
                println!("  ✅ Reporte exportado a: {}", ruta.green().bold());
                println!("  Puedes abrirlo en Excel e imprimirlo.");
            }
            Err(e) => {
                println!();
                println!("  ❌ Error al exportar: {}", e);
            }
        }
    }

    pausa();
}

fn rastreador_tabla_aporte_minimo(state: &AppState) {
    let deudas_reales: Vec<&DeudaRastreada> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .filter(|d| d.activa && !d.es_pago_corriente() && d.saldo_actual() > 0.01)
        .collect();

    if deudas_reales.is_empty() {
        println!("  No hay deudas activas para proyectar.");
        pausa();
        return;
    }

    limpiar();
    separador("📊 TABLA DE APORTE MÍNIMO MENSUAL — ¿Cuánto necesitas para salir de deudas?");

    let deuda_total: f64 = deudas_reales.iter().map(|d| d.saldo_actual()).sum();
    let ingreso_mensual = state.asesor.rastreador.ingreso_mensual_total();
    let minimos: f64 = deudas_reales.iter().map(|d| d.pago_minimo).sum();

    println!();
    println!(
        "  Deuda total:     {}",
        format!("${:.2}", deuda_total).red().bold()
    );
    println!(
        "  Ingreso mensual: {}",
        format!("${:.2}", ingreso_mensual).green()
    );
    println!("  Mínimos deudas:  {}", format!("${:.2}", minimos).yellow());
    println!();

    // Elegir estrategia
    let estrategias = &[
        "❄️  Avalancha (tasa más alta primero)",
        "⛄ Bola de nieve (saldo más bajo primero)",
    ];
    let bola_nieve = match menu("¿Qué estrategia usar?", estrategias) {
        Some(1) => true,
        Some(0) => false,
        _ => return,
    };

    // Calcular máximo de meses con pagos mínimos
    let max_meses_default = match state.asesor.rastreador.meses_para_salir(
        minimos
            + state
                .asesor
                .rastreador
                .deudas
                .iter()
                .filter(|d| d.activa && d.es_pago_corriente())
                .map(|d| d.pago_minimo)
                .sum::<f64>(),
        bola_nieve,
    ) {
        Some(m) if m > 0 => m.min(120),
        _ => 60,
    };

    let max_meses = pedir_f64(
        "¿Hasta cuántos meses mostrar? (máx referencia con pago mínimo)",
        max_meses_default as f64,
    ) as usize;

    let min_meses = pedir_f64("¿Desde cuántos meses? (mínimo agresivo)", 1.0) as usize;

    if min_meses > max_meses || min_meses == 0 {
        println!("  Rango inválido.");
        pausa();
        return;
    }

    println!();
    println!("  ⏳ Calculando proyecciones... (esto puede tomar unos segundos)");
    println!();

    let tabla = state
        .asesor
        .rastreador
        .tabla_aporte_minimo(max_meses, min_meses, bola_nieve);

    if tabla.is_empty() {
        println!("  No se pudo calcular ninguna proyección.");
        pausa();
        return;
    }

    limpiar();
    let nombre_est = if bola_nieve {
        "Bola de nieve"
    } else {
        "Avalancha"
    };
    separador(&format!(
        "📊 TABLA DE APORTE MÍNIMO — {} | Deuda: ${:.2}",
        nombre_est, deuda_total
    ));

    println!();
    println!("  💡 Esta tabla muestra cuánto necesitas aportar como mínimo cada mes");
    println!("     para salir de deudas en el número de meses indicado.");
    println!("     Úsala como referencia para saber cuánto debes ganar o destinar.");
    println!();

    // Encabezados de la tabla
    println!(
        "  ┌──────────┬──────────────────┬──────────────────┬──────────────────┬────────────────┐"
    );
    println!(
        "  │ {:>8} │ {:>16} │ {:>16} │ {:>16} │ {:>14} │",
        "Meses", "Aporte/mes", "Total pagado", "Intereses", "Ahorro vs max"
    );
    println!(
        "  ├──────────┼──────────────────┼──────────────────┼──────────────────┼────────────────┤"
    );

    // El mayor total pagado (más meses = más intereses) para calcular ahorro
    let max_total = tabla.first().map(|(_, _, tp, _)| *tp).unwrap_or(0.0);

    let mut prev_aporte = 0.0f64;
    for (meses, aporte, total_pagado, total_intereses) in &tabla {
        let ahorro = max_total - total_pagado;
        let delta = if prev_aporte > 0.01 {
            aporte - prev_aporte
        } else {
            0.0
        };
        let delta_str = if delta.abs() > 0.01 {
            format!(" (+${:.0})", delta)
        } else {
            String::new()
        };

        // Colorear según accesibilidad
        let aporte_str = format!("${:.2}", aporte);
        let aporte_display = if ingreso_mensual > 0.01 && *aporte <= ingreso_mensual {
            format!("{:>16}", aporte_str).green().to_string()
        } else if ingreso_mensual > 0.01 && *aporte <= ingreso_mensual * 1.2 {
            format!("{:>16}", aporte_str).yellow().to_string()
        } else {
            format!("{:>16}", aporte_str).red().to_string()
        };

        println!(
            "  │ {:>6}m  │ {} │ {:>16} │ {:>16} │ {:>14} │",
            meses,
            aporte_display,
            format!("${:.2}", total_pagado),
            format!("${:.2}", total_intereses),
            if ahorro > 0.01 {
                format!("${:.2}", ahorro)
            } else {
                "—".to_string()
            }
        );

        if !delta_str.is_empty() {
            println!(
                "  │          │ {:>16} │                  │                  │                │",
                delta_str
            );
        }

        prev_aporte = *aporte;
    }
    println!(
        "  └──────────┴──────────────────┴──────────────────┴──────────────────┴────────────────┘"
    );

    // Resumen
    println!();
    if let Some((meses_max, aporte_min, _, int_max)) = tabla.first() {
        if let Some((meses_min, aporte_max, _, int_min)) = tabla.last() {
            println!(
                "  📌 Con {} puedes salir en {}m (máximo interés: {})",
                format!("${:.2}/mes", aporte_min).yellow(),
                meses_max,
                format!("${:.2}", int_max).red()
            );
            println!(
                "  🚀 Con {} sales en solo {}m (interés: {})",
                format!("${:.2}/mes", aporte_max).green().bold(),
                meses_min,
                format!("${:.2}", int_min).red()
            );
            let ahorro_total = int_max - int_min;
            if ahorro_total > 0.01 {
                println!(
                    "  💰 Diferencia en intereses: {} — ¡eso te ahorras pagando más rápido!",
                    format!("${:.2}", ahorro_total).green().bold()
                );
            }
        }
    }

    // Indicar qué es viable con ingreso actual
    if ingreso_mensual > 0.01 {
        println!();
        let viables: Vec<_> = tabla
            .iter()
            .filter(|(_, aporte, _, _)| *aporte <= ingreso_mensual)
            .collect();
        if let Some((meses_rapido, aporte_rapido, _, _)) = viables.last() {
            println!(
                "  ✅ Con tu ingreso actual ({}) lo más rápido viable es {}m aportando {}",
                format!("${:.2}", ingreso_mensual).green(),
                meses_rapido,
                format!("${:.2}/mes", aporte_rapido).green().bold()
            );
        } else {
            println!(
                "  ⚠️  Tu ingreso actual ({}) no alcanza para ninguna opción.",
                format!("${:.2}", ingreso_mensual).red()
            );
            if let Some((_, aporte_min, _, _)) = tabla.first() {
                println!(
                    "     Necesitas al menos {} para el plan más lento.",
                    format!("${:.2}/mes", aporte_min).yellow()
                );
            }
        }
    }

    println!();
    pausa();
}

fn exportar_simulacion_excel(
    sim: &SimulacionLibertad,
    nombres: &[String],
) -> Result<String, String> {
    let carpeta = dirs::document_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("OmniPlanner");
    std::fs::create_dir_all(&carpeta).map_err(|e| format!("No se pudo crear carpeta: {}", e))?;

    let fecha = chrono::Local::now().format("%Y-%m-%d_%H%M%S");
    let archivo = carpeta.join(format!("simulacion_deudas_{}.xlsx", fecha));

    let mut wb = Workbook::new();

    // ── Formatos ──
    let fmt_titulo = Format::new()
        .set_bold()
        .set_font_size(14)
        .set_align(FormatAlign::Center);
    let fmt_header = Format::new()
        .set_bold()
        .set_font_size(11)
        .set_border(FormatBorder::Thin)
        .set_background_color("4472C4")
        .set_font_color("FFFFFF")
        .set_align(FormatAlign::Center);
    let fmt_dinero = Format::new()
        .set_num_format("$#,##0.00")
        .set_border(FormatBorder::Thin);
    let fmt_dinero_rojo = Format::new()
        .set_num_format("$#,##0.00")
        .set_border(FormatBorder::Thin)
        .set_font_color("FF0000");
    let fmt_dinero_verde = Format::new()
        .set_num_format("$#,##0.00")
        .set_border(FormatBorder::Thin)
        .set_font_color("008000");
    let fmt_celda = Format::new()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center);
    let fmt_celda_izq = Format::new().set_border(FormatBorder::Thin);
    let fmt_evento = Format::new().set_bold().set_font_color("008000");
    let fmt_seccion = Format::new()
        .set_bold()
        .set_font_size(12)
        .set_background_color("D9E2F3");

    // ════════════════════════════════════════════
    //  HOJA 1: Amortización mes a mes
    // ════════════════════════════════════════════
    let ws = wb.add_worksheet();
    ws.set_name("Amortización").map_err(|e| e.to_string())?;

    // Título
    ws.merge_range(0, 0, 0, 4, "", &fmt_titulo)
        .map_err(|e| e.to_string())?;
    ws.write_string_with_format(
        0,
        0,
        format!(
            "Plan de Libertad Financiera — {} | ${:.2}/mes",
            sim.estrategia, sim.presupuesto_mensual
        ),
        &fmt_titulo,
    )
    .map_err(|e| e.to_string())?;

    // Info general
    let mut row = 2u32;
    ws.write_string(row, 0, "Presupuesto mensual:")
        .map_err(|e| e.to_string())?;
    ws.write_number_with_format(row, 1, sim.presupuesto_mensual, &fmt_dinero)
        .map_err(|e| e.to_string())?;
    row += 1;
    ws.write_string(row, 0, "Gastos fijos:")
        .map_err(|e| e.to_string())?;
    ws.write_number_with_format(row, 1, sim.total_gastos_fijos, &fmt_dinero)
        .map_err(|e| e.to_string())?;
    if !sim.gastos_fijos.is_empty() {
        let detalle: String = sim
            .gastos_fijos
            .iter()
            .map(|(n, m)| format!("{} ${:.2}", n, m))
            .collect::<Vec<_>>()
            .join(", ");
        ws.write_string(row, 2, &detalle)
            .map_err(|e| e.to_string())?;
    }
    row += 1;
    ws.write_string(row, 0, "Disponible para deudas:")
        .map_err(|e| e.to_string())?;
    ws.write_number_with_format(
        row,
        1,
        sim.presupuesto_mensual - sim.total_gastos_fijos,
        &fmt_dinero_verde,
    )
    .map_err(|e| e.to_string())?;
    row += 2;

    // Tabla de amortización
    for mes in &sim.meses {
        ws.merge_range(row, 0, row, 4, "", &fmt_seccion)
            .map_err(|e| e.to_string())?;
        let pago_total: f64 = mes.pagos.iter().map(|(_, p)| *p).sum();
        let int_total: f64 = mes.intereses.iter().map(|(_, i)| *i).sum();
        ws.write_string_with_format(
            row,
            0,
            format!(
                "MES {}  |  Pagos: ${:.2}  |  Intereses: ${:.2}  |  Deuda restante: ${:.2}",
                mes.mes_numero, pago_total, int_total, mes.deuda_total
            ),
            &fmt_seccion,
        )
        .map_err(|e| e.to_string())?;
        row += 1;

        ws.write_string_with_format(row, 0, "Deuda", &fmt_header)
            .map_err(|e| e.to_string())?;
        ws.write_string_with_format(row, 1, "Pago", &fmt_header)
            .map_err(|e| e.to_string())?;
        ws.write_string_with_format(row, 2, "Interés", &fmt_header)
            .map_err(|e| e.to_string())?;
        ws.write_string_with_format(row, 3, "Saldo", &fmt_header)
            .map_err(|e| e.to_string())?;
        ws.write_string_with_format(row, 4, "Evento", &fmt_header)
            .map_err(|e| e.to_string())?;
        row += 1;

        for (nombre, saldo) in &mes.saldos {
            let pago = mes
                .pagos
                .iter()
                .find(|(n, _)| n == nombre)
                .map(|(_, p)| *p)
                .unwrap_or(0.0);
            let interes = mes
                .intereses
                .iter()
                .find(|(n, _)| n == nombre)
                .map(|(_, i)| *i)
                .unwrap_or(0.0);

            ws.write_string_with_format(row, 0, nombre, &fmt_celda_izq)
                .map_err(|e| e.to_string())?;
            ws.write_number_with_format(row, 1, pago, &fmt_dinero_verde)
                .map_err(|e| e.to_string())?;
            ws.write_number_with_format(
                row,
                2,
                interes,
                if interes > 0.01 {
                    &fmt_dinero_rojo
                } else {
                    &fmt_dinero
                },
            )
            .map_err(|e| e.to_string())?;
            ws.write_number_with_format(row, 3, *saldo, &fmt_dinero)
                .map_err(|e| e.to_string())?;

            if mes.liquidadas_este_mes.contains(nombre) {
                ws.write_string_with_format(row, 4, "LIQUIDADA", &fmt_evento)
                    .map_err(|e| e.to_string())?;
            } else if *saldo < 0.01 && pago < 0.01 {
                ws.write_string_with_format(row, 4, "ya liquidada", &fmt_celda)
                    .map_err(|e| e.to_string())?;
            }
            row += 1;
        }
        row += 1;
    }

    ws.set_column_width(0, 22).map_err(|e| e.to_string())?;
    ws.set_column_width(1, 14).map_err(|e| e.to_string())?;
    ws.set_column_width(2, 14).map_err(|e| e.to_string())?;
    ws.set_column_width(3, 14).map_err(|e| e.to_string())?;
    ws.set_column_width(4, 14).map_err(|e| e.to_string())?;

    // ════════════════════════════════════════════
    //  HOJA 2: Resumen
    // ════════════════════════════════════════════
    let ws2 = wb.add_worksheet();
    ws2.set_name("Resumen").map_err(|e| e.to_string())?;

    ws2.merge_range(0, 0, 0, 4, "", &fmt_titulo)
        .map_err(|e| e.to_string())?;
    ws2.write_string_with_format(0, 0, "Resumen — Plan de Libertad Financiera", &fmt_titulo)
        .map_err(|e| e.to_string())?;

    let mut r = 2u32;
    ws2.write_string(r, 0, "Estrategia:")
        .map_err(|e| e.to_string())?;
    ws2.write_string(r, 1, &sim.estrategia)
        .map_err(|e| e.to_string())?;
    r += 1;
    ws2.write_string(r, 0, "Meses totales:")
        .map_err(|e| e.to_string())?;
    ws2.write_number_with_format(r, 1, sim.meses.len() as f64, &fmt_celda)
        .map_err(|e| e.to_string())?;
    r += 1;
    ws2.write_string(r, 0, "Total pagado:")
        .map_err(|e| e.to_string())?;
    ws2.write_number_with_format(r, 1, sim.total_pagado, &fmt_dinero)
        .map_err(|e| e.to_string())?;
    r += 1;
    ws2.write_string(r, 0, "Total intereses:")
        .map_err(|e| e.to_string())?;
    ws2.write_number_with_format(r, 1, sim.total_intereses, &fmt_dinero_rojo)
        .map_err(|e| e.to_string())?;
    r += 1;
    ws2.write_string(r, 0, "Capital pagado:")
        .map_err(|e| e.to_string())?;
    ws2.write_number_with_format(
        r,
        1,
        sim.total_pagado - sim.total_intereses,
        &fmt_dinero_verde,
    )
    .map_err(|e| e.to_string())?;
    r += 2;

    ws2.write_string_with_format(r, 0, "Deuda", &fmt_header)
        .map_err(|e| e.to_string())?;
    ws2.write_string_with_format(r, 1, "Total pagado", &fmt_header)
        .map_err(|e| e.to_string())?;
    ws2.write_string_with_format(r, 2, "Intereses", &fmt_header)
        .map_err(|e| e.to_string())?;
    ws2.write_string_with_format(r, 3, "Capital", &fmt_header)
        .map_err(|e| e.to_string())?;
    ws2.write_string_with_format(r, 4, "Mes liquidación", &fmt_header)
        .map_err(|e| e.to_string())?;
    r += 1;

    for nombre in nombres {
        let total_pago: f64 = sim
            .meses
            .iter()
            .flat_map(|m| m.pagos.iter())
            .filter(|(n, _)| n == nombre)
            .map(|(_, p)| *p)
            .sum();
        let total_int: f64 = sim
            .meses
            .iter()
            .flat_map(|m| m.intereses.iter())
            .filter(|(n, _)| n == nombre)
            .map(|(_, i)| *i)
            .sum();
        let mes_liq = sim
            .orden_liquidacion
            .iter()
            .find(|(n, _)| n == nombre)
            .map(|(_, m)| *m as f64);

        ws2.write_string_with_format(r, 0, nombre, &fmt_celda_izq)
            .map_err(|e| e.to_string())?;
        ws2.write_number_with_format(r, 1, total_pago, &fmt_dinero)
            .map_err(|e| e.to_string())?;
        ws2.write_number_with_format(r, 2, total_int, &fmt_dinero_rojo)
            .map_err(|e| e.to_string())?;
        ws2.write_number_with_format(r, 3, total_pago - total_int, &fmt_dinero_verde)
            .map_err(|e| e.to_string())?;
        if let Some(m) = mes_liq {
            ws2.write_number_with_format(r, 4, m, &fmt_celda)
                .map_err(|e| e.to_string())?;
        } else {
            ws2.write_string_with_format(r, 4, "—", &fmt_celda)
                .map_err(|e| e.to_string())?;
        }
        r += 1;
    }

    r += 1;
    ws2.write_string_with_format(r, 0, "Orden de liquidación", &fmt_seccion)
        .map_err(|e| e.to_string())?;
    r += 1;
    for (i, (nombre, mes)) in sim.orden_liquidacion.iter().enumerate() {
        ws2.write_string_with_format(r, 0, format!("{}. {}", i + 1, nombre), &fmt_celda_izq)
            .map_err(|e| e.to_string())?;
        ws2.write_string(r, 1, format!("Mes {}", mes))
            .map_err(|e| e.to_string())?;
        r += 1;
    }

    ws2.set_column_width(0, 22).map_err(|e| e.to_string())?;
    ws2.set_column_width(1, 16).map_err(|e| e.to_string())?;
    ws2.set_column_width(2, 14).map_err(|e| e.to_string())?;
    ws2.set_column_width(3, 14).map_err(|e| e.to_string())?;
    ws2.set_column_width(4, 18).map_err(|e| e.to_string())?;

    // Guardar
    wb.save(&archivo)
        .map_err(|e| format!("Error guardando Excel: {}", e))?;

    Ok(archivo.to_string_lossy().to_string())
}

fn rastreador_editar_pago(state: &mut AppState) {
    if state.asesor.rastreador.deudas.is_empty() {
        println!("  Sin deudas registradas.");
        pausa();
        return;
    }

    let nombres: Vec<String> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .map(|d| format!("{} ({} meses)", d.nombre, d.historial.len()))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    if let Some(idx) = menu("¿Editar cuál deuda?", &refs) {
        let d = &state.asesor.rastreador.deudas[idx];
        if d.historial.is_empty() {
            println!("  No hay meses registrados.");
            pausa();
            return;
        }

        let meses: Vec<String> = d
            .historial
            .iter()
            .map(|m| {
                format!(
                    "{} — Saldo: ${:.2}, Pago: ${:.2}, Cargos: ${:.2}",
                    m.mes, m.saldo_inicio, m.pago, m.nuevos_cargos
                )
            })
            .collect();
        let refs_m: Vec<&str> = meses.iter().map(|s| s.as_str()).collect();

        if let Some(midx) = menu("¿Cuál mes editar?", &refs_m) {
            let actual = &d.historial[midx];
            println!();
            println!("  Datos actuales: {}", actual.mes);
            println!("    Saldo inicio: ${:.2}", actual.saldo_inicio);
            println!("    Pago: ${:.2}", actual.pago);
            println!("    Nuevos cargos: ${:.2}", actual.nuevos_cargos);
            println!();

            let nuevo_pago = pedir_f64(
                &format!("Nuevo pago (actual ${:.2})", actual.pago),
                actual.pago,
            );
            let nuevos_cargos = pedir_f64(
                &format!("Nuevos cargos (actual ${:.2})", actual.nuevos_cargos),
                actual.nuevos_cargos,
            );

            // Recalcular desde este mes en adelante
            let tasa_anual = state.asesor.rastreador.deudas[idx].tasa_anual;
            let saldo_inicio = state.asesor.rastreador.deudas[idx].historial[midx].saldo_inicio;

            // Actualizar este mes
            let tasa_mensual = tasa_anual / 100.0 / 12.0;
            let saldo_despues = (saldo_inicio - nuevo_pago).max(0.0);
            let intereses = saldo_despues * tasa_mensual;
            let saldo_final = saldo_despues + intereses + nuevos_cargos;

            state.asesor.rastreador.deudas[idx].historial[midx].pago = nuevo_pago;
            state.asesor.rastreador.deudas[idx].historial[midx].nuevos_cargos = nuevos_cargos;
            state.asesor.rastreador.deudas[idx].historial[midx].intereses = intereses;
            state.asesor.rastreador.deudas[idx].historial[midx].saldo_final =
                if saldo_final < 0.01 { 0.0 } else { saldo_final };

            // Recalcular meses siguientes
            let mut saldo = if saldo_final < 0.01 { 0.0 } else { saldo_final };
            let len = state.asesor.rastreador.deudas[idx].historial.len();
            for i in (midx + 1)..len {
                state.asesor.rastreador.deudas[idx].historial[i].saldo_inicio = saldo;
                let pago_i = state.asesor.rastreador.deudas[idx].historial[i].pago;
                let cargos_i = state.asesor.rastreador.deudas[idx].historial[i].nuevos_cargos;
                let sd = (saldo - pago_i).max(0.0);
                let int_i = sd * tasa_mensual;
                let sf = sd + int_i + cargos_i;
                state.asesor.rastreador.deudas[idx].historial[i].intereses = int_i;
                state.asesor.rastreador.deudas[idx].historial[i].saldo_final =
                    if sf < 0.01 { 0.0 } else { sf };
                saldo = if sf < 0.01 { 0.0 } else { sf };
            }

            println!(
                "  {} Mes actualizado y saldos recalculados. Nuevo saldo final: ${:.2}",
                "✓".green(),
                state.asesor.rastreador.deudas[idx].saldo_actual()
            );
            pausa();
        }
    }
}

fn rastreador_ajustar_tasa(state: &mut AppState) {
    if state.asesor.rastreador.deudas.is_empty() {
        println!("  Sin deudas registradas.");
        pausa();
        return;
    }

    let nombres: Vec<String> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .map(|d| format!("{} — tasa actual: {:.1}% anual", d.nombre, d.tasa_anual))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    if let Some(idx) = menu("¿A cuál deuda ajustar la tasa?", &refs) {
        let nombre = state.asesor.rastreador.deudas[idx].nombre.clone();
        let actual = state.asesor.rastreador.deudas[idx].tasa_anual;
        println!();
        println!(
            "  {} — Tasa actual: {:.2}% anual ({:.2}% mensual)",
            nombre,
            actual,
            actual / 12.0
        );
        let nueva = pedir_f64("Nueva tasa anual (%) (ej: 24.99)", actual);
        state.asesor.rastreador.deudas[idx].tasa_anual = nueva;
        println!(
            "  {} Tasa de '{}' actualizada a {:.2}%",
            "✓".green(),
            nombre,
            nueva
        );
    }
    pausa();
}

fn rastreador_ingreso(state: &mut AppState) {
    state.asesor.rastreador.migrar_ingreso_legacy();
    loop {
        limpiar();
        separador("💵 INGRESOS");

        let rast = &state.asesor.rastreador;
        if rast.ingresos.is_empty() {
            println!("  No hay ingresos registrados.");
        } else {
            for (i, ing) in rast.ingresos.iter().enumerate() {
                println!(
                    "  {}. {} — {} ({})",
                    i + 1,
                    ing.concepto,
                    format!("${:.2}", ing.monto).green(),
                    ing.frecuencia.nombre()
                );
            }
            println!();
            println!(
                "  Total mensual: {}",
                format!("${:.2}", rast.ingreso_mensual_total())
                    .green()
                    .bold()
            );
        }
        println!();

        let opciones = &[
            "➕  Agregar ingreso",
            "✏️   Editar ingreso",
            "🗑️   Eliminar ingreso",
            "🔙  Volver",
        ];
        match menu("¿Qué hacer?", opciones) {
            Some(0) => rastreador_agregar_ingreso(state),
            Some(1) => rastreador_editar_ingreso(state),
            Some(2) => rastreador_eliminar_ingreso(state),
            _ => return,
        }
    }
}

fn pedir_frecuencia(prompt: &str) -> Option<FrecuenciaPago> {
    let frecuencias = &[
        "Semanal",
        "Quincenal",
        "Mensual",
        "Trimestral",
        "Semestral",
        "Anual",
    ];
    match menu(prompt, frecuencias) {
        Some(0) => Some(FrecuenciaPago::Semanal),
        Some(1) => Some(FrecuenciaPago::Quincenal),
        Some(2) => Some(FrecuenciaPago::Mensual),
        Some(3) => Some(FrecuenciaPago::Trimestral),
        Some(4) => Some(FrecuenciaPago::Semestral),
        Some(5) => Some(FrecuenciaPago::Anual),
        _ => None,
    }
}

fn rastreador_agregar_ingreso(state: &mut AppState) {
    let concepto = match pedir_texto("Concepto (ej: Sueldo empresa X, Freelance, Renta)") {
        Some(c) => c,
        None => return,
    };
    let freq = match pedir_frecuencia("¿Cada cuánto recibes este ingreso?") {
        Some(f) => f,
        None => return,
    };
    let monto = pedir_f64("Monto ($)", 0.0);
    if monto <= 0.0 {
        println!("  {} El monto debe ser mayor a 0.", "✗".red());
        pausa();
        return;
    }
    state.asesor.rastreador.ingresos.push(IngresoRastreado {
        concepto: concepto.clone(),
        monto,
        frecuencia: freq.clone(),
    });
    println!(
        "  {} Ingreso agregado: {} — ${:.2} ({})",
        "✓".green(),
        concepto,
        monto,
        freq.nombre()
    );
    pausa();
}

fn rastreador_editar_ingreso(state: &mut AppState) {
    if state.asesor.rastreador.ingresos.is_empty() {
        println!("  No hay ingresos para editar.");
        pausa();
        return;
    }
    let nombres: Vec<String> = state
        .asesor
        .rastreador
        .ingresos
        .iter()
        .enumerate()
        .map(|(i, ing)| {
            format!(
                "{}. {} — ${:.2} ({})",
                i + 1,
                ing.concepto,
                ing.monto,
                ing.frecuencia.nombre()
            )
        })
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
    let idx = match menu("¿Cuál editar?", &refs) {
        Some(i) => i,
        None => return,
    };

    let ing = &state.asesor.rastreador.ingresos[idx];
    let concepto_actual = ing.concepto.clone();
    let monto_actual = ing.monto;

    let nuevo_concepto = pedir_texto_opcional(&format!(
        "Concepto (actual: {}, vacío=mantener)",
        concepto_actual
    ));
    let freq = pedir_frecuencia("Nueva frecuencia (Esc=mantener)");
    let nuevo_monto = pedir_f64("Nuevo monto ($)", monto_actual);

    let ing = &mut state.asesor.rastreador.ingresos[idx];
    if !nuevo_concepto.is_empty() {
        ing.concepto = nuevo_concepto;
    }
    if let Some(f) = freq {
        ing.frecuencia = f;
    }
    ing.monto = nuevo_monto;
    println!(
        "  {} Ingreso actualizado: {} — ${:.2} ({})",
        "✓".green(),
        ing.concepto,
        ing.monto,
        ing.frecuencia.nombre()
    );
    pausa();
}

fn rastreador_eliminar_ingreso(state: &mut AppState) {
    if state.asesor.rastreador.ingresos.is_empty() {
        println!("  No hay ingresos para eliminar.");
        pausa();
        return;
    }
    let nombres: Vec<String> = state
        .asesor
        .rastreador
        .ingresos
        .iter()
        .enumerate()
        .map(|(i, ing)| {
            format!(
                "{}. {} — ${:.2} ({})",
                i + 1,
                ing.concepto,
                ing.monto,
                ing.frecuencia.nombre()
            )
        })
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
    let idx = match menu("¿Cuál eliminar?", &refs) {
        Some(i) => i,
        None => return,
    };
    let eliminado = state.asesor.rastreador.ingresos.remove(idx);
    println!(
        "  {} Ingreso '{}' eliminado.",
        "✓".green(),
        eliminado.concepto
    );
    pausa();
}

fn rastreador_exportar(state: &AppState) {
    if state.asesor.rastreador.deudas.is_empty() {
        println!("  Sin deudas registradas.");
        pausa();
        return;
    }

    let opciones = &[
        "📊  Exportar resumen global (todas las deudas)",
        "📋  Exportar historial de una deuda",
        "🔙  Cancelar",
    ];

    match menu("¿Qué exportar?", opciones) {
        Some(0) => {
            let csv = state.asesor.rastreador.csv_resumen_global();
            let dir = omniplanner::ml::advisor::AlmacenAsesor::dir_exportacion();
            let ruta = dir.join("rastreador_resumen.csv");
            match std::fs::write(&ruta, &csv) {
                Ok(()) => {
                    println!();
                    println!("  ✅ CSV exportado: {}", ruta.display().to_string().green());
                }
                Err(e) => println!("  {} Error: {}", "✗".red(), e),
            }
            pausa();
        }
        Some(1) => {
            let nombres: Vec<String> = state
                .asesor
                .rastreador
                .deudas
                .iter()
                .map(|d| format!("{} ({} meses)", d.nombre, d.historial.len()))
                .collect();
            let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

            if let Some(idx) = menu("¿Cuál deuda exportar?", &refs) {
                let nombre = &state.asesor.rastreador.deudas[idx].nombre;
                let csv = state.asesor.rastreador.csv_historial_deuda(nombre);
                let dir = omniplanner::ml::advisor::AlmacenAsesor::dir_exportacion();
                let archivo = format!(
                    "rastreador_{}.csv",
                    nombre
                        .chars()
                        .filter(|c| c.is_alphanumeric() || *c == ' ')
                        .collect::<String>()
                        .replace(' ', "_")
                );
                let ruta = dir.join(archivo);
                match std::fs::write(&ruta, &csv) {
                    Ok(()) => {
                        println!();
                        println!("  ✅ CSV exportado: {}", ruta.display().to_string().green());
                    }
                    Err(e) => println!("  {} Error: {}", "✗".red(), e),
                }
                pausa();
            }
        }
        _ => {}
    }
}

fn rastreador_importar_csv(state: &mut AppState) {
    limpiar();
    separador("📂 IMPORTAR DEUDAS");

    println!("  📋 Arrastra tu archivo Excel (.xlsx) o CSV aquí:");
    println!("  💡 También puedes escribir la ruta manualmente.");
    println!();

    let ruta = match pedir_texto("Ruta del archivo (arrastra aquí)") {
        Some(r) => {
            // Limpiar formato de arrastrar en Windows: & 'ruta' → ruta
            let limpio = r.trim();
            let limpio = limpio.strip_prefix("& ").unwrap_or(limpio);
            let limpio = limpio.trim_matches('\'').trim_matches('"').trim();
            limpio.to_string()
        }
        None => return,
    };

    // Si es Excel, convertir automáticamente con Python
    let csv_path =
        if ruta.to_lowercase().ends_with(".xlsx") || ruta.to_lowercase().ends_with(".xls") {
            println!();
            println!("  🔄 Detectado archivo Excel. Convirtiendo a CSV...");

            // Ruta temporal para el CSV generado
            let csv_temp = std::env::temp_dir().join("omniplanner_import.csv");

            // Buscar el script de conversión
            let script = if let Ok(exe) = std::env::current_exe() {
                let base = exe
                    .parent()
                    .and_then(|p| p.parent())
                    .and_then(|p| p.parent())
                    .unwrap_or_else(|| std::path::Path::new("."));
                let s = base.join("tools").join("excel_a_csv.py");
                if s.exists() {
                    s
                } else {
                    std::path::PathBuf::from("tools").join("excel_a_csv.py")
                }
            } else {
                std::path::PathBuf::from("tools").join("excel_a_csv.py")
            };

            // Intentar varias ubicaciones del script
            let script_path = if script.exists() {
                script
            } else {
                // Intentar relativo al directorio de trabajo
                let cwd_script = std::path::PathBuf::from("tools").join("excel_a_csv.py");
                if cwd_script.exists() {
                    cwd_script
                } else {
                    // Ruta absoluta del proyecto
                    std::path::PathBuf::from(
                        r"C:\Users\elxav\proyectos\omniplanner\tools\excel_a_csv.py",
                    )
                }
            };

            if !script_path.exists() {
                println!(
                    "  {} No se encontró el script de conversión: {}",
                    "✗".red(),
                    script_path.display()
                );
                println!("  Asegúrate de que existe: tools/excel_a_csv.py");
                pausa();
                return;
            }

            let resultado = std::process::Command::new("python")
                .arg(&script_path)
                .arg(&ruta)
                .arg(csv_temp.to_str().unwrap_or("omniplanner_import.csv"))
                .output();

            match resultado {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);

                    if !stdout.is_empty() {
                        for line in stdout.lines() {
                            println!("    {}", line);
                        }
                    }

                    if !output.status.success() {
                        println!("  {} Error al convertir Excel:", "✗".red());
                        if !stderr.is_empty() {
                            for line in stderr.lines() {
                                println!("    {}", line);
                            }
                        }
                        pausa();
                        return;
                    }

                    if !csv_temp.exists() {
                        println!("  {} No se generó el archivo CSV.", "✗".red());
                        pausa();
                        return;
                    }

                    println!("  ✅ Conversión exitosa.");
                    csv_temp.to_string_lossy().to_string()
                }
                Err(e) => {
                    println!("  {} No se pudo ejecutar Python: {}", "✗".red(), e);
                    println!("  Asegúrate de tener Python instalado con: pip install openpyxl");
                    pausa();
                    return;
                }
            }
        } else {
            ruta
        };

    println!();

    match omniplanner::ml::advisor::RastreadorDeudas::importar_csv(&csv_path) {
        Ok(importado) => {
            let n_deudas = importado.deudas.len();
            let n_meses: usize = importado.deudas.iter().map(|d| d.historial.len()).sum();

            println!();
            println!(
                "  ✅ Importación exitosa: {} cuentas, {} registros",
                n_deudas, n_meses
            );
            println!();

            // Mostrar resumen de lo importado
            for d in &importado.deudas {
                let si = d.historial.first().map(|m| m.saldo_inicio).unwrap_or(0.0);
                let sf = d.saldo_actual();
                let tendencia = if sf > si + 100.0 {
                    "📈 Creció".red().to_string()
                } else if sf < si * 0.5 {
                    "📉 Bajó mucho".green().to_string()
                } else {
                    "➡️ Estable".to_string()
                };
                println!(
                    "    {:<20} ${:>10.2} → ${:>10.2}  ({} meses) {}",
                    d.nombre,
                    si,
                    sf,
                    d.historial.len(),
                    tendencia
                );
            }
            println!();

            if !state.asesor.rastreador.deudas.is_empty() {
                let opciones_merge = &[
                    "🔄  Reemplazar todo (borrar datos actuales)",
                    "➕  Agregar a las existentes (merge)",
                    "❌  Cancelar",
                ];
                match menu(
                    "Ya tienes deudas en el rastreador. ¿Qué hacer?",
                    opciones_merge,
                ) {
                    Some(0) => {
                        state.asesor.rastreador = importado;
                        println!("  {} Datos reemplazados.", "✓".green());
                    }
                    Some(1) => {
                        for d in importado.deudas {
                            // Si ya existe una deuda con el mismo nombre, reemplazarla
                            if let Some(pos) = state
                                .asesor
                                .rastreador
                                .deudas
                                .iter()
                                .position(|x| x.nombre == d.nombre)
                            {
                                state.asesor.rastreador.deudas[pos] = d;
                            } else {
                                state.asesor.rastreador.deudas.push(d);
                            }
                        }
                        println!("  {} Datos combinados.", "✓".green());
                    }
                    _ => {
                        println!("  Importación cancelada.");
                    }
                }
            } else {
                state.asesor.rastreador = importado;
                println!("  {} Listo. Ahora puedes ver el diagnóstico.", "✓".green());
            }

            println!();
            println!("  💡 Tip: Ajusta las tasas de interés de cada cuenta");
            println!("    para un diagnóstico más preciso.");
        }
        Err(e) => {
            println!();
            println!("  {} Error: {}", "✗".red(), e);
        }
    }
    pausa();
}

fn rastreador_gestionar_deudas(state: &mut AppState) {
    loop {
        limpiar();
        separador("🔀 GESTIONAR DEUDAS");

        if state.asesor.rastreador.deudas.is_empty() {
            println!("  Sin deudas registradas.");
            pausa();
            return;
        }

        // Mostrar tabla con estado actual
        println!(
            "  {:<4} {:<25} {:>10} {:>8} {:>10}  Estado",
            "#", "Deuda", "Saldo", "Tasa%", "Pago mín"
        );
        println!("  {}", "─".repeat(78));

        for (i, d) in state.asesor.rastreador.deudas.iter().enumerate() {
            let estado = if !d.activa {
                "⏸️  INACTIVA".to_string()
            } else if d.es_pago_corriente() {
                "🔒 Corriente".to_string()
            } else if d.obligatoria {
                "🔒 Obligatoria".to_string()
            } else {
                "📋 Normal".to_string()
            };

            let nombre_corto = if d.nombre.len() > 24 {
                format!("{}…", &d.nombre[..23])
            } else {
                d.nombre.clone()
            };

            let saldo_str = if d.es_pago_corriente() {
                "corriente".to_string()
            } else {
                format!("${:.2}", d.saldo_actual())
            };

            println!(
                "  {:<4} {:<25} {:>10} {:>7}% {:>10}  {}",
                format!("{}.", i + 1),
                nombre_corto,
                saldo_str,
                format!("{:.1}", d.tasa_anual),
                format!("${:.2}", d.pago_minimo),
                estado
            );
        }
        println!("  {}", "─".repeat(78));
        println!();

        let acciones = &[
            "⏸️   Activar / Desactivar una deuda (excluir de simulación)",
            "🔒  Cambiar a Obligatoria / Normal (prioridad de pago)",
            "🔙  Volver",
        ];

        match menu("¿Qué quieres hacer?", acciones) {
            Some(0) => {
                // Toggle activa/inactiva
                let nombres: Vec<String> = state
                    .asesor
                    .rastreador
                    .deudas
                    .iter()
                    .enumerate()
                    .map(|(i, d)| {
                        let estado = if d.activa { "ACTIVA" } else { "INACTIVA" };
                        format!(
                            "{}. {} — ${:.2} [{}]",
                            i + 1,
                            d.nombre,
                            d.saldo_actual(),
                            estado
                        )
                    })
                    .collect();
                let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

                if let Some(idx) = menu("¿Cuál deuda cambiar?", &refs) {
                    let d = &mut state.asesor.rastreador.deudas[idx];
                    let nuevo_estado = !d.activa;
                    let accion = if nuevo_estado {
                        "ACTIVADA ✅"
                    } else {
                        "DESACTIVADA ⏸️"
                    };
                    d.activa = nuevo_estado;
                    println!();
                    println!("  {} '{}' ahora está {}", "✓".green(), d.nombre, accion);
                    if !nuevo_estado {
                        println!(
                            "  {}",
                            "  → No aparecerá en simulaciones ni diagnósticos.".dimmed()
                        );
                    }
                    state.guardar().ok();
                    pausa();
                }
            }
            Some(1) => {
                // Toggle obligatoria/normal
                let deudas_no_corrientes: Vec<(usize, String)> = state
                    .asesor
                    .rastreador
                    .deudas
                    .iter()
                    .enumerate()
                    .filter(|(_, d)| !d.es_pago_corriente())
                    .map(|(i, d)| {
                        let tipo = if d.obligatoria {
                            "🔒 OBLIGATORIA"
                        } else {
                            "📋 Normal"
                        };
                        (
                            i,
                            format!("{} — ${:.2} [{}]", d.nombre, d.saldo_actual(), tipo),
                        )
                    })
                    .collect();

                if deudas_no_corrientes.is_empty() {
                    println!("  No hay deudas editables (solo pagos corrientes).");
                    pausa();
                    continue;
                }

                let labels: Vec<&str> = deudas_no_corrientes
                    .iter()
                    .map(|(_, s)| s.as_str())
                    .collect();

                if let Some(sel) = menu("¿Cuál deuda cambiar?", &labels) {
                    let real_idx = deudas_no_corrientes[sel].0;
                    let d = &mut state.asesor.rastreador.deudas[real_idx];
                    let nueva_prioridad = !d.obligatoria;
                    let accion = if nueva_prioridad {
                        "OBLIGATORIA 🔒 (se paga primero en simulación)"
                    } else {
                        "NORMAL 📋 (participa en avalancha/bola de nieve)"
                    };
                    d.obligatoria = nueva_prioridad;
                    println!();
                    println!("  {} '{}' ahora es {}", "✓".green(), d.nombre, accion);
                    state.guardar().ok();
                    pausa();
                }
            }
            _ => return,
        }
    }
}

fn rastreador_eliminar(state: &mut AppState) {
    if state.asesor.rastreador.deudas.is_empty() {
        println!("  Sin deudas registradas.");
        pausa();
        return;
    }

    let nombres: Vec<String> = state
        .asesor
        .rastreador
        .deudas
        .iter()
        .map(|d| format!("{} — ${:.2}", d.nombre, d.saldo_actual()))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    if let Some(idx) = menu("¿Cuál deuda eliminar?", &refs) {
        let nombre = state.asesor.rastreador.deudas[idx].nombre.clone();
        if Confirm::new()
            .with_prompt(format!(
                "  ¿Eliminar '{}'? Se perderá todo el historial.",
                nombre
            ))
            .default(false)
            .interact()
            .unwrap_or(false)
        {
            state.asesor.rastreador.deudas.remove(idx);
            println!("  {} '{}' eliminada", "✓".green(), nombre);
        }
    }
    pausa();
}

// ── Historial unificado y exportación ──

fn menu_asesor_historial(state: &mut AppState) {
    loop {
        limpiar();
        println!(
            "{}",
            "╔══════════════════════════════════════════════════════════╗".cyan()
        );
        println!(
            "{}",
            "║   📂 H I S T O R I A L   Y   E X P O R T A C I Ó N   ║"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "║   Todos tus análisis guardados — exporta, busca, revisa║".cyan()
        );
        println!(
            "{}",
            "╚══════════════════════════════════════════════════════════╝".cyan()
        );
        println!();

        let n = state.asesor.registros.len();
        if n == 0 {
            println!(
                "  📌 No hay registros aún. Cada análisis que hagas se guardará automáticamente."
            );
            println!();

            // Aún mostrar acciones del diccionario antiguo si hay
            let dic = &state.asesor.diccionario;
            if !dic.acciones.is_empty() {
                println!(
                    "  📝 {} acciones registradas (historial previo):",
                    dic.acciones.len()
                );
                let total = dic.acciones.len();
                let inicio = total.saturating_sub(5);
                for a in &dic.acciones[inicio..] {
                    println!(
                        "    {} [{}] {} — {}",
                        a.impacto.emoji(),
                        a.fecha,
                        a.accion,
                        a.categoria.dimmed()
                    );
                }
            }
            pausa();
            return;
        }

        // ── Resumen por tipo ──
        let mut conteo_tipo: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for r in &state.asesor.registros {
            *conteo_tipo.entry(r.tipo_nombre.clone()).or_default() += 1;
        }
        println!("  📊 {} registros guardados:", n);
        for (tipo, cnt) in &conteo_tipo {
            println!("     • {} ({})", tipo, cnt);
        }
        println!();

        // Últimos 5 registros
        println!("  📋 Últimos registros:");
        let inicio = n.saturating_sub(5);
        for r in &state.asesor.registros[inicio..] {
            println!(
                "    {} #{:<4} [{}] {} — {}",
                r.datos.emoji(),
                r.id,
                r.fecha,
                r.titulo,
                r.resumen.chars().take(50).collect::<String>().dimmed()
            );
        }
        println!();

        let opciones = &[
            "📋  Ver todos los registros",
            "🔍  Buscar en registros",
            "📄  Ver detalle de un registro",
            "📊  Filtrar por tipo",
            "📥  Exportar TODO a CSV (Excel)",
            "📥  Exportar TODO a texto (imprimir)",
            "📥  Exportar UN registro a CSV",
            "📥  Exportar UN registro a texto",
            "🗑️   Eliminar un registro",
            "🔙  Volver",
        ];

        match menu("¿Qué hacer?", opciones) {
            Some(0) => historial_ver_todos(state),
            Some(1) => historial_buscar(state),
            Some(2) => historial_ver_detalle(state),
            Some(3) => historial_filtrar_tipo(state),
            Some(4) => historial_exportar_csv_todo(state),
            Some(5) => historial_exportar_texto_todo(state),
            Some(6) => historial_exportar_csv_uno(state),
            Some(7) => historial_exportar_texto_uno(state),
            Some(8) => historial_eliminar(state),
            _ => return,
        }
    }
}

fn historial_ver_todos(state: &AppState) {
    limpiar();
    separador("📋 TODOS LOS REGISTROS");

    if state.asesor.registros.is_empty() {
        println!("  Sin registros.");
        pausa();
        return;
    }

    println!(
        "  {:<5} {:<12} {:<6} {:<22} {:<30} Resumen",
        "ID", "Fecha", "Hora", "Tipo", "Título"
    );
    println!("  {}", "─".repeat(100));

    for r in &state.asesor.registros {
        println!(
            "  {:<5} {:<12} {:<6} {:<22} {:<30} {}",
            r.id,
            r.fecha,
            r.hora,
            r.tipo_nombre,
            if r.titulo.len() > 28 {
                format!("{}…", &r.titulo[..27])
            } else {
                r.titulo.clone()
            },
            r.resumen.chars().take(40).collect::<String>().dimmed()
        );
    }
    println!("  {}", "─".repeat(100));
    println!("  Total: {} registros", state.asesor.registros.len());
    pausa();
}

fn historial_buscar(state: &AppState) {
    let texto = pedir_texto_opcional("Buscar (texto en título, resumen o etiquetas)");
    if texto.is_empty() {
        return;
    }

    let resultados = state.asesor.buscar_registros(&texto);
    if resultados.is_empty() {
        println!("  No se encontraron registros con '{}'.", texto);
        pausa();
        return;
    }

    println!();
    println!("  🔍 {} resultado(s) para '{}':", resultados.len(), texto);
    println!();
    for r in &resultados {
        println!(
            "    {} #{} [{}] {} — {}",
            r.datos.emoji(),
            r.id,
            r.fecha,
            r.titulo.bold(),
            r.resumen.dimmed()
        );
    }
    pausa();
}

fn historial_ver_detalle(state: &AppState) {
    if state.asesor.registros.is_empty() {
        println!("  Sin registros.");
        pausa();
        return;
    }

    let nombres: Vec<String> = state
        .asesor
        .registros
        .iter()
        .map(|r| format!("#{} [{}] {} — {}", r.id, r.fecha, r.tipo_nombre, r.titulo))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    if let Some(idx) = menu("¿Cuál registro ver?", &refs) {
        let reg = &state.asesor.registros[idx];
        limpiar();
        println!("{}", reg.detalle_texto());
        pausa();
    }
}

fn historial_filtrar_tipo(state: &AppState) {
    let tipos: Vec<String> = {
        let mut t: Vec<String> = state
            .asesor
            .registros
            .iter()
            .map(|r| r.tipo_nombre.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        t.sort();
        t
    };

    if tipos.is_empty() {
        println!("  Sin registros.");
        pausa();
        return;
    }

    let refs: Vec<&str> = tipos.iter().map(|s| s.as_str()).collect();
    if let Some(idx) = menu("¿Filtrar por qué tipo?", &refs) {
        let filtrados = state.asesor.filtrar_por_tipo(&tipos[idx]);
        println!();
        println!(
            "  📊 {} registro(s) de tipo '{}':",
            filtrados.len(),
            tipos[idx]
        );
        println!();
        for r in &filtrados {
            println!(
                "    {} #{} [{}] {} — {}",
                r.datos.emoji(),
                r.id,
                r.fecha,
                r.titulo.bold(),
                r.resumen.dimmed()
            );
        }
        pausa();
    }
}

fn historial_exportar_csv_todo(state: &AppState) {
    match state.asesor.exportar_resumen_csv() {
        Ok(ruta) => {
            println!();
            println!("  ✅ CSV exportado: {}", ruta.display().to_string().green());
            println!("  💡 Ábrelo en Excel, Google Sheets o cualquier hoja de cálculo.");
        }
        Err(e) => println!("  {} {}", "✗".red(), e),
    }
    pausa();
}

fn historial_exportar_texto_todo(state: &AppState) {
    match state.asesor.exportar_reporte_texto() {
        Ok(ruta) => {
            println!();
            println!(
                "  ✅ Reporte exportado: {}",
                ruta.display().to_string().green()
            );
            println!("  🖨️ Listo para imprimir o revisar en cualquier editor.");
        }
        Err(e) => println!("  {} {}", "✗".red(), e),
    }
    pausa();
}

fn historial_exportar_csv_uno(state: &AppState) {
    if state.asesor.registros.is_empty() {
        println!("  Sin registros.");
        pausa();
        return;
    }

    let nombres: Vec<String> = state
        .asesor
        .registros
        .iter()
        .map(|r| format!("#{} [{}] {} — {}", r.id, r.fecha, r.tipo_nombre, r.titulo))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    if let Some(idx) = menu("¿Exportar cuál registro a CSV?", &refs) {
        let id = state.asesor.registros[idx].id;
        match state.asesor.exportar_registro_csv(id) {
            Ok(ruta) => {
                println!();
                println!(
                    "  ✅ CSV detallado exportado: {}",
                    ruta.display().to_string().green()
                );
            }
            Err(e) => println!("  {} {}", "✗".red(), e),
        }
        pausa();
    }
}

fn historial_exportar_texto_uno(state: &AppState) {
    if state.asesor.registros.is_empty() {
        println!("  Sin registros.");
        pausa();
        return;
    }

    let nombres: Vec<String> = state
        .asesor
        .registros
        .iter()
        .map(|r| format!("#{} [{}] {} — {}", r.id, r.fecha, r.tipo_nombre, r.titulo))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    if let Some(idx) = menu("¿Exportar cuál registro a texto?", &refs) {
        let id = state.asesor.registros[idx].id;
        match state.asesor.exportar_registro_texto(id) {
            Ok(ruta) => {
                println!();
                println!(
                    "  ✅ Reporte exportado: {}",
                    ruta.display().to_string().green()
                );
            }
            Err(e) => println!("  {} {}", "✗".red(), e),
        }
        pausa();
    }
}

fn historial_eliminar(state: &mut AppState) {
    if state.asesor.registros.is_empty() {
        println!("  Sin registros.");
        pausa();
        return;
    }

    let nombres: Vec<String> = state
        .asesor
        .registros
        .iter()
        .map(|r| format!("#{} [{}] {} — {}", r.id, r.fecha, r.tipo_nombre, r.titulo))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    if let Some(idx) = menu("¿Cuál registro eliminar?", &refs) {
        let titulo = state.asesor.registros[idx].titulo.clone();
        if Confirm::new()
            .with_prompt(format!(
                "  ¿Eliminar #{}? '{}'",
                state.asesor.registros[idx].id, titulo
            ))
            .default(false)
            .interact()
            .unwrap_or(false)
        {
            state.asesor.registros.remove(idx);
            println!("  {} Registro eliminado", "✓".green());
        }
    }
    pausa();
}

// ══════════════════════════════════════════════════════════════
//  Menú compacto ML/NLP avanzado (las herramientas técnicas)
// ══════════════════════════════════════════════════════════════

fn menu_ml_nlp_avanzado(state: &mut AppState) {
    loop {
        limpiar();
        separador("🤖 ML / NLP AVANZADO");
        println!(
            "  {}",
            "Herramientas técnicas de Machine Learning y NLP".dimmed()
        );
        println!();

        let opciones = &[
            "🤖  Machine Learning (modelos, datasets, algoritmos)",
            "🗣️   Procesamiento de Lenguaje Natural (NLP)",
            "🔙  Volver",
        ];

        match menu("¿Qué abrir?", opciones) {
            Some(0) => menu_ml(state),
            Some(1) => menu_nlp(state),
            _ => return,
        }
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
            "💡  Asesor Inteligente (Decisiones y Finanzas)",
            "🤖  ML/NLP Avanzado (Herramientas técnicas)",
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
            Some(9) => menu_asesor(&mut state),
            Some(10) => menu_ml_nlp_avanzado(&mut state),
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
