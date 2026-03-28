use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime};
use colored::Colorize;
use dialoguer::{Input, Select, Confirm};

use omniplanner::agenda::{Evento, HorarioEscritura, TipoEvento};
use omniplanner::canvas::{Canvas, Punto, Trazo};
use omniplanner::diagrams::{Diagrama, Nodo, TipoConexion, TipoDiagrama, TipoNodo};
use omniplanner::mapper::{Codificacion, EsquemaMapa, Mapper};
use omniplanner::memoria::Recuerdo;
use omniplanner::storage::AppState;
use omniplanner::tasks::{Prioridad, Task, TaskStatus};

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

fn pedir_texto(prompt: &str) -> String {
    Input::new()
        .with_prompt(format!("  {}", prompt))
        .interact_text()
        .unwrap_or_default()
}

fn pedir_texto_opcional(prompt: &str) -> String {
    Input::new()
        .with_prompt(format!("  {}", prompt))
        .default(String::new())
        .allow_empty(true)
        .interact_text()
        .unwrap_or_default()
}

fn pedir_fecha(prompt: &str) -> Option<NaiveDate> {
    loop {
        let s = pedir_texto_opcional(&format!("{} (YYYY-MM-DD, vacío=cancelar)", prompt));
        if s.is_empty() { return None; }
        match NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
            Ok(f) => return Some(f),
            Err(_) => {
                println!("    {} Formato inválido. Ejemplo: 2026-03-28", "✗".red());
            }
        }
    }
}

fn pedir_hora(prompt: &str) -> Option<NaiveTime> {
    loop {
        let s = pedir_texto_opcional(&format!("{} (HH:MM, vacío=cancelar)", prompt));
        if s.is_empty() { return None; }
        match NaiveTime::parse_from_str(&s, "%H:%M") {
            Ok(h) => return Some(h),
            Err(_) => {
                println!("    {} Formato inválido. Ejemplo: 14:30", "✗".red());
            }
        }
    }
}

fn menu(titulo: &str, opciones: &[&str]) -> usize {
    println!();
    Select::new()
        .with_prompt(format!("  {}", titulo).bold().to_string())
        .items(opciones)
        .default(0)
        .interact()
        .unwrap_or(0)
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
            println!("    📌 {}{} {} ({})", e.hora_inicio.format("%H:%M"), fin, e.titulo, e.tipo);
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
            "✅ Cambiar estado",
            "🔔 Programar follow-up",
            "🏷️  Agregar etiqueta / recordar",
            "🗑️  Eliminar tarea",
            "← Volver al menú",
        ];

        match menu("¿Qué deseas hacer?", opciones) {
            0 => nueva_tarea(state),
            1 => cambiar_estado_tarea(state),
            2 => follow_up_tarea(state),
            3 => recordar_tarea(state),
            4 => eliminar_tarea(state),
            _ => return,
        }
    }
}

fn nueva_tarea(state: &mut AppState) {
    separador("➕ Nueva tarea");
    let titulo = pedir_texto("Título");
    let desc = pedir_texto_opcional("Descripción (opcional)");
    let fecha = match pedir_fecha("Fecha") { Some(f) => f, None => return };
    let hora = match pedir_hora("Hora") { Some(h) => h, None => return };

    let prioridades = &["Baja", "Media", "Alta", "⚠ Urgente"];
    let pi = menu("Prioridad", prioridades);
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

fn cambiar_estado_tarea(state: &mut AppState) {
    if state.tasks.tareas.is_empty() {
        println!("  {}", "No hay tareas.".yellow());
        pausa();
        return;
    }

    let nombres: Vec<String> = state.tasks.tareas.iter()
        .map(|t| format!("{} - {} [{}]", t.id, t.titulo, t.estado))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();

    let idx = menu("Selecciona la tarea", &refs);
    let estados = &["Pendiente", "En Progreso", "Completada", "Cancelada"];
    let ei = menu("Nuevo estado", estados);

    let nuevo = match ei {
        0 => TaskStatus::Pendiente,
        1 => TaskStatus::EnProgreso,
        2 => TaskStatus::Completada,
        3 => TaskStatus::Cancelada,
        _ => return,
    };

    state.tasks.tareas[idx].cambiar_estado(nuevo);
    println!("  {} {}", "✓".green().bold(), state.tasks.tareas[idx]);
    pausa();
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

    let idx = menu("¿A cuál tarea?", &refs);
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

    let idx = menu("¿Cuál tarea?", &refs);
    let palabras = pedir_texto("Palabras clave para recordar (separadas por coma)");
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

    let idx = menu("¿Cuál eliminar?", &refs);
    let nombre = state.tasks.tareas[idx].titulo.clone();

    if Confirm::new().with_prompt(format!("  ¿Eliminar '{}'?", nombre)).default(false).interact().unwrap_or(false) {
        state.tasks.tareas.remove(idx);
        println!("  {} Tarea eliminada", "✓".green());
    }
    pausa();
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
                println!("  📌 {} | {}{} | {} ({})",
                    e.id.dimmed(),
                    e.hora_inicio.format("%H:%M"), fin,
                    e.titulo,
                    e.tipo
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
            "🗑️  Eliminar evento",
            "🏷️  Recordar evento",
            "← Volver al menú",
        ];

        match menu("¿Qué deseas hacer?", opciones) {
            0 => nuevo_evento(state),
            1 => nuevo_horario(state),
            2 => eliminar_evento(state),
            3 => recordar_evento(state),
            _ => return,
        }
    }
}

fn nuevo_evento(state: &mut AppState) {
    separador("📌 Nuevo evento");
    let titulo = pedir_texto("Título");
    let desc = pedir_texto_opcional("Descripción (opcional)");

    let tipos = &["Reunión", "Recordatorio", "Follow-Up", "Cita", "Otro"];
    let ti = menu("Tipo de evento", tipos);
    let tipo = match ti {
        0 => TipoEvento::Reunion,
        1 => TipoEvento::Recordatorio,
        2 => TipoEvento::FollowUp,
        3 => TipoEvento::Cita,
        _ => TipoEvento::Otro("Otro".to_string()),
    };

    let fecha = match pedir_fecha("Fecha") { Some(f) => f, None => return };
    let hora = match pedir_hora("Hora inicio") { Some(h) => h, None => return };

    let tiene_fin = Confirm::new()
        .with_prompt("  ¿Tiene hora de fin?")
        .default(true)
        .interact()
        .unwrap_or(false);
    let hora_fin = if tiene_fin { pedir_hora("Hora fin") } else { None };

    let tags = pedir_texto_opcional("Palabras clave (opcional, separadas por coma)");

    let evento = Evento::new(titulo.clone(), desc, tipo, fecha, hora, hora_fin);

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
    let di = menu("Día de la semana", dias);
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

    let idx = menu("¿Cuál eliminar?", &refs);
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

    let idx = menu("¿Cuál evento?", &refs);
    let palabras = pedir_texto("Palabras clave para recordar (separadas por coma)");
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
//  Módulo: CANVAS
// ══════════════════════════════════════════════════════════════

fn menu_canvas(state: &mut AppState) {
    loop {
        limpiar();
        separador("✏️  CANVAS — Escritura a mano");

        if !state.canvases.is_empty() {
            for c in &state.canvases {
                println!("  🖼️  [{}] {} ({}x{}) — {} trazos", c.id.dimmed(), c.nombre, c.ancho, c.alto, c.trazos.len());
            }
        } else {
            println!("  {}", "(sin canvas — crea tu primer lienzo)".dimmed());
        }

        let opciones = &[
            "🖼️  Nuevo canvas",
            "✏️  Dibujar trazo",
            "🔍 Reconocer escritura",
            "💾 Exportar a SVG",
            "← Volver al menú",
        ];

        match menu("¿Qué deseas hacer?", opciones) {
            0 => nuevo_canvas(state),
            1 => dibujar_trazo(state),
            2 => reconocer_canvas(state),
            3 => exportar_canvas(state),
            _ => return,
        }
    }
}

fn nuevo_canvas(state: &mut AppState) {
    separador("🖼️  Nuevo canvas");
    let nombre = pedir_texto("Nombre");
    let ancho: u32 = Input::new()
        .with_prompt("  Ancho (px)")
        .default(800u32)
        .interact_text()
        .unwrap_or(800);
    let alto: u32 = Input::new()
        .with_prompt("  Alto (px)")
        .default(600u32)
        .interact_text()
        .unwrap_or(600);

    let c = Canvas::new(nombre.clone(), ancho, alto);
    println!("  {} [{}] {} ({}x{})", "✓ Canvas creado:".green().bold(), c.id, nombre, ancho, alto);
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
        .map(|c| format!("[{}] {} ({} trazos)", c.id, c.nombre, c.trazos.len()))
        .collect();
    let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
    Some(menu("Selecciona canvas", &refs))
}

fn dibujar_trazo(state: &mut AppState) {
    let idx = match seleccionar_canvas(state) { Some(i) => i, None => return };
    println!("  Ingresa puntos como: x,y ; x,y ; x,y");
    let puntos_str = pedir_texto("Puntos");
    let mut trazo = Trazo::new("#000000".to_string(), 2.0);
    for par in puntos_str.split(';') {
        let coords: Vec<&str> = par.split(',').collect();
        if coords.len() >= 2 {
            if let (Ok(x), Ok(y)) = (coords[0].trim().parse::<f64>(), coords[1].trim().parse::<f64>()) {
                trazo.agregar_punto(Punto { x, y, presion: 1.0, timestamp_ms: 0 });
            }
        }
    }
    let n = trazo.puntos.len();
    state.canvases[idx].agregar_trazo(trazo);
    println!("  {} {} puntos agregados", "✓".green(), n);
    pausa();
}

fn reconocer_canvas(state: &mut AppState) {
    let idx = match seleccionar_canvas(state) { Some(i) => i, None => return };
    let resultados = state.canvases[idx].reconocer_escritura();
    if resultados.is_empty() {
        println!("  {}", "No se reconoció escritura.".yellow());
    } else {
        separador("🔍 Reconocimiento");
        for r in resultados {
            println!("  {}", r);
        }
    }
    pausa();
}

fn exportar_canvas(state: &AppState) {
    let idx = match seleccionar_canvas(state) { Some(i) => i, None => return };
    let salida = pedir_texto("Archivo de salida (ej: dibujo.svg)");
    let svg = state.canvases[idx].exportar_svg();
    match std::fs::write(&salida, &svg) {
        Ok(_) => println!("  {} Exportado a '{}'", "✓".green(), salida),
        Err(e) => println!("  {} {}", "✗".red(), e),
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
            0 => nuevo_diagrama(state),
            1 => agregar_nodo(state),
            2 => conectar_nodos(state),
            3 => ver_mermaid(state),
            4 => ver_pseudo(state),
            5 => validar_diagrama(state),
            6 => recordar_diagrama(state),
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
    Some(menu("Selecciona diagrama", &refs))
}

fn nuevo_diagrama(state: &mut AppState) {
    separador("📊 Nuevo diagrama");
    let nombre = pedir_texto("Nombre");
    let tipos = &["Diagrama de Flujo", "Algoritmo", "Proceso", "Flujo de Datos", "Libre"];
    let ti = menu("Tipo", tipos);
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
    let ni = menu("Tipo de nodo", tipos_nodo);
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

    let etiqueta = pedir_texto("Etiqueta del nodo");
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
    let ni = menu("Tipo de nodo", tipos_nodo);
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

    let etiqueta = pedir_texto("Etiqueta del nodo");
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
    let oi = menu("Origen", &refs);
    println!("  Selecciona el nodo DESTINO:");
    let di = menu("Destino", &refs);

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

    let oi = menu("Origen", &refs);
    let di = menu("Destino", &refs);
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
    let palabras = pedir_texto("Palabras clave (separadas por coma)");
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
            0 => {
                let mensaje = pedir_texto("Mensaje del commit");
                let autor = pedir_texto_opcional("Autor");
                let autor = if autor.is_empty() { "usuario".to_string() } else { autor };
                let datos = serde_json::to_string(&state.tasks).unwrap_or_default();
                let id = state.vcs.commit(datos, mensaje.clone(), autor);
                println!("  {} Commit [{}]: {}", "✓".green(), id, mensaje);
                pausa();
            }
            1 => {
                let nombre = pedir_texto("Nombre de la nueva rama");
                if state.vcs.crear_rama(nombre.clone()) {
                    println!("  {} Rama '{}' creada y activada", "✓".green(), nombre);
                } else {
                    println!("  {} La rama '{}' ya existe", "✗".red(), nombre);
                }
                pausa();
            }
            2 => {
                let ramas: Vec<String> = state.vcs.ramas.iter().map(|r| r.nombre.clone()).collect();
                let refs: Vec<&str> = ramas.iter().map(|s| s.as_str()).collect();
                let idx = menu("Selecciona rama", &refs);
                state.vcs.cambiar_rama(&ramas[idx]);
                println!("  {} Cambiado a '{}'", "✓".green(), ramas[idx]);
                pausa();
            }
            3 => {
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
            0 => {
                let texto = pedir_texto("Texto a codificar");
                let formatos = &["Base64", "Hexadecimal", "Binario"];
                let fi = menu("Formato", formatos);
                let cod = match fi {
                    0 => Codificacion::Base64,
                    1 => Codificacion::Hex,
                    _ => Codificacion::Binario,
                };
                let resultado = Mapper::codificar(&texto, &cod);
                println!("\n  {} → {}", formatos[fi].cyan(), resultado.green().bold());
                pausa();
            }
            1 => {
                let hex = pedir_texto("Texto en hexadecimal");
                match Mapper::decodificar_hex(&hex) {
                    Some(texto) => println!("  {} → {}", "hex".cyan(), texto.green().bold()),
                    None => println!("  {} Formato hex inválido", "✗".red()),
                }
                pausa();
            }
            2 => {
                let nombre = pedir_texto("Nombre del esquema");
                let cods = &["UTF-8", "JSON", "CSV", "Base64", "Hex", "Binario"];
                let ei = menu("Codificación de entrada", cods);
                let si = menu("Codificación de salida", cods);
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
            3 => {
                if state.mapper.esquemas.is_empty() {
                    println!("  {}", "No hay esquemas.".yellow());
                    pausa();
                    continue;
                }
                let nombres: Vec<String> = state.mapper.esquemas.iter().map(|e| format!("[{}] {}", e.id, e.nombre)).collect();
                let refs: Vec<&str> = nombres.iter().map(|s| s.as_str()).collect();
                let idx = menu("Esquema", &refs);
                let origen = pedir_texto("Campo origen");
                let destino = pedir_texto("Campo destino");
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
        separador("🧠 MEMORIA — Buscar y conectar todo");

        if !state.memoria.recuerdos.is_empty() {
            let mut palabras: Vec<&String> = state.memoria.palabras_clave();
            palabras.sort();
            palabras.dedup();
            println!("  Palabras clave: {}", palabras.iter().map(|p| p.cyan().to_string()).collect::<Vec<_>>().join(", "));
            println!("  {} recuerdos guardados", state.memoria.recuerdos.len());
        } else {
            println!("  {}", "(sin recuerdos — agrega palabras clave a tus items)".dimmed());
        }

        if !state.memoria.enlaces.is_empty() {
            println!("  {} enlaces entre módulos", state.memoria.enlaces.len());
        }

        let opciones = &[
            "🔍 Buscar por palabra clave",
            "🔗 Enlazar dos elementos",
            "📝 Crear recuerdo libre",
            "📋 Ver todos los recuerdos",
            "← Volver al menú",
        ];

        match menu("¿Qué deseas hacer?", opciones) {
            0 => buscar_memoria(state),
            1 => enlazar_elementos(state),
            2 => crear_recuerdo(state),
            3 => ver_recuerdos(state),
            _ => return,
        }
    }
}

fn buscar_memoria(state: &AppState) {
    let consulta = pedir_texto("¿Qué buscas?");
    let resultados = state.memoria.buscar(&consulta);

    if resultados.is_empty() {
        // Buscar también en títulos de tareas, eventos, diagramas
        println!();
        let mut encontrado = false;

        for t in &state.tasks.tareas {
            if t.titulo.to_lowercase().contains(&consulta.to_lowercase())
                || t.descripcion.to_lowercase().contains(&consulta.to_lowercase())
                || t.etiquetas.iter().any(|e| e.to_lowercase().contains(&consulta.to_lowercase()))
            {
                println!("  📋 Tarea: {} [{}] — {}", t.titulo, t.id, t.estado);
                encontrado = true;
            }
        }

        for e in &state.agenda.eventos {
            if e.titulo.to_lowercase().contains(&consulta.to_lowercase())
                || e.descripcion.to_lowercase().contains(&consulta.to_lowercase())
            {
                println!("  📅 Evento: {} [{}] — {}", e.titulo, e.id, e.tipo);
                encontrado = true;
            }
        }

        for d in &state.diagramas {
            if d.nombre.to_lowercase().contains(&consulta.to_lowercase()) {
                println!("  📊 Diagrama: {} [{}] — {}", d.nombre, d.id, d.tipo);
                encontrado = true;
            }
        }

        for c in &state.canvases {
            if c.nombre.to_lowercase().contains(&consulta.to_lowercase()) {
                println!("  ✏️  Canvas: {} [{}]", c.nombre, c.id);
                encontrado = true;
            }
        }

        if !encontrado {
            println!("  {}", "No se encontraron resultados.".yellow());
        }
    } else {
        separador("Resultados");
        for r in resultados {
            let origen = match (&r.modulo_origen, &r.item_id) {
                (Some(m), Some(id)) => format!(" ({} [{}])", m, id),
                _ => String::new(),
            };
            println!("  🧠 {}{}", r.contenido, origen.dimmed());
            println!("     Palabras: {}", r.palabras_clave.join(", ").cyan());

            // Mostrar enlaces relacionados
            if let (Some(modulo), Some(id)) = (&r.modulo_origen, &r.item_id) {
                let enlaces = state.memoria.enlaces_de(modulo, id);
                for e in enlaces {
                    println!("     🔗 {} [{}] ↔ {} [{}] ({})",
                        e.origen_modulo, e.origen_id, e.destino_modulo, e.destino_id, e.relacion);
                }
            }
        }
    }
    pausa();
}

fn enlazar_elementos(state: &mut AppState) {
    let modulos = &["📋 Tarea", "📅 Evento", "📊 Diagrama", "✏️  Canvas"];

    println!("  Selecciona el PRIMER elemento:");
    let m1 = menu("Módulo origen", modulos);
    let (mod1, id1) = seleccionar_item_de_modulo(state, m1);
    if id1.is_empty() { return; }

    println!("  Selecciona el SEGUNDO elemento:");
    let m2 = menu("Módulo destino", modulos);
    let (mod2, id2) = seleccionar_item_de_modulo(state, m2);
    if id2.is_empty() { return; }

    let relacion = pedir_texto("Relación (ej: 'necesita', 'depende de', 'parte de')");

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
            let i = menu("Selecciona", &refs);
            ("tarea".to_string(), state.tasks.tareas[i].id.clone())
        }
        1 => {
            if state.agenda.eventos.is_empty() { println!("  Sin eventos."); return (String::new(), String::new()); }
            let items: Vec<String> = state.agenda.eventos.iter().map(|e| format!("{} - {}", e.id, e.titulo)).collect();
            let refs: Vec<&str> = items.iter().map(|s| s.as_str()).collect();
            let i = menu("Selecciona", &refs);
            ("evento".to_string(), state.agenda.eventos[i].id.clone())
        }
        2 => {
            if state.diagramas.is_empty() { println!("  Sin diagramas."); return (String::new(), String::new()); }
            let items: Vec<String> = state.diagramas.iter().map(|d| format!("{} - {}", d.id, d.nombre)).collect();
            let refs: Vec<&str> = items.iter().map(|s| s.as_str()).collect();
            let i = menu("Selecciona", &refs);
            ("diagrama".to_string(), state.diagramas[i].id.clone())
        }
        3 => {
            if state.canvases.is_empty() { println!("  Sin canvases."); return (String::new(), String::new()); }
            let items: Vec<String> = state.canvases.iter().map(|c| format!("{} - {}", c.id, c.nombre)).collect();
            let refs: Vec<&str> = items.iter().map(|s| s.as_str()).collect();
            let i = menu("Selecciona", &refs);
            ("canvas".to_string(), state.canvases[i].id.clone())
        }
        _ => (String::new(), String::new()),
    }
}

fn crear_recuerdo(state: &mut AppState) {
    separador("📝 Nuevo recuerdo");
    let contenido = pedir_texto("¿Qué quieres recordar?");
    let palabras = pedir_texto("Palabras clave (separadas por coma)");
    let tags: Vec<String> = palabras.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();

    let vincular = Confirm::new()
        .with_prompt("  ¿Vincular a un elemento existente?")
        .default(false)
        .interact()
        .unwrap_or(false);

    let mut recuerdo = Recuerdo::new(contenido, tags);

    if vincular {
        let modulos = &["📋 Tarea", "📅 Evento", "📊 Diagrama", "✏️  Canvas"];
        let mi = menu("¿De qué módulo?", modulos);
        let (modulo, id) = seleccionar_item_de_modulo(state, mi);
        if !id.is_empty() {
            recuerdo = recuerdo.con_origen(&modulo, &id);
        }
    }

    state.memoria.agregar_recuerdo(recuerdo);
    println!("  {} Recuerdo guardado", "🧠".to_string());
    pausa();
}

fn ver_recuerdos(state: &AppState) {
    if state.memoria.recuerdos.is_empty() {
        println!("  {}", "Sin recuerdos guardados.".dimmed());
        pausa();
        return;
    }

    separador("Todos los recuerdos");
    for r in &state.memoria.recuerdos {
        let origen = match (&r.modulo_origen, &r.item_id) {
            (Some(m), Some(id)) => format!(" → {} [{}]", m, id),
            _ => String::new(),
        };
        println!("  🧠 {}{}", r.contenido, origen.dimmed());
        println!("     🏷️  {}", r.palabras_clave.join(", ").cyan());
    }
    pausa();
}

// ══════════════════════════════════════════════════════════════
//  MAIN — Menú principal interactivo
// ══════════════════════════════════════════════════════════════

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
            "❌  Salir",
        ];

        match menu("¿Qué módulo quieres usar?", opciones) {
            0 => menu_tareas(&mut state),
            1 => menu_agenda(&mut state),
            2 => menu_canvas(&mut state),
            3 => menu_diagramas(&mut state),
            4 => menu_versiones(&mut state),
            5 => menu_mapeo(&mut state),
            6 => menu_memoria(&mut state),
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
