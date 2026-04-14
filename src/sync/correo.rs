use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};

use super::SyncConfig;
use crate::agenda::Evento;
use crate::tasks::Task;

pub fn enviar_correo(config: &SyncConfig, asunto: &str, cuerpo: &str) -> Result<(), String> {
    if !config.email_configurado() {
        return Err("Email no configurado".to_string());
    }

    let email = Message::builder()
        .from(
            config
                .email_remitente
                .parse()
                .map_err(|e| format!("Remitente inválido: {}", e))?,
        )
        .to(config
            .email_destinatario
            .parse()
            .map_err(|e| format!("Destinatario inválido: {}", e))?)
        .subject(asunto)
        .body(cuerpo.to_string())
        .map_err(|e| format!("Error construyendo email: {}", e))?;

    let creds = Credentials::new(config.smtp_usuario.clone(), config.smtp_password.clone());

    let mailer = SmtpTransport::relay(&config.smtp_server)
        .map_err(|e| format!("Error conectando SMTP: {}", e))?
        .credentials(creds)
        .build();

    mailer
        .send(&email)
        .map_err(|e| format!("Error enviando email: {}", e))?;

    Ok(())
}

pub fn enviar_recordatorio_tarea(config: &SyncConfig, tarea: &Task) -> Result<(), String> {
    let asunto = format!("Recordatorio: {}", tarea.titulo);
    let cuerpo = format!(
        "\
═══════════════════════════════════════
  OMNIPLANNER — Recordatorio de Tarea
═══════════════════════════════════════

  Título:      {}
  Descripción: {}
  Fecha:       {}
  Hora:        {}
  Prioridad:   {}
  Estado:      {}
{}
───────────────────────────────────────
Enviado por OmniPlanner",
        tarea.titulo,
        if tarea.descripcion.is_empty() {
            "(sin descripción)"
        } else {
            &tarea.descripcion
        },
        tarea.fecha.format("%d/%m/%Y"),
        tarea.hora.format("%H:%M"),
        tarea.prioridad,
        tarea.estado,
        tarea
            .follow_up
            .map(|f| format!("  Follow-up:   {}", f.format("%d/%m/%Y %H:%M")))
            .unwrap_or_default(),
    );

    enviar_correo(config, &asunto, &cuerpo)
}

pub fn enviar_recordatorio_evento(config: &SyncConfig, evento: &Evento) -> Result<(), String> {
    let fin = evento
        .hora_fin
        .map(|h| format!(" - {}", h.format("%H:%M")))
        .unwrap_or_default();

    let asunto = format!("Recordatorio: {}", evento.titulo);
    let cuerpo = format!(
        "\
═══════════════════════════════════════
  OMNIPLANNER — Recordatorio de Evento
═══════════════════════════════════════

  Título:      {}
  Descripción: {}
  Tipo:        {}
  Fecha:       {}
  Hora:        {}{}

───────────────────────────────────────
Enviado por OmniPlanner",
        evento.titulo,
        if evento.descripcion.is_empty() {
            "(sin descripción)"
        } else {
            &evento.descripcion
        },
        evento.tipo,
        evento.fecha.format("%d/%m/%Y"),
        evento.hora_inicio.format("%H:%M"),
        fin,
    );

    enviar_correo(config, &asunto, &cuerpo)
}

pub fn enviar_resumen_diario(
    config: &SyncConfig,
    tareas: &[&Task],
    eventos: &[&Evento],
    follow_ups: &[&Task],
) -> Result<(), String> {
    let hoy = chrono::Local::now().format("%d/%m/%Y");
    let asunto = format!("OmniPlanner — Resumen del día {}", hoy);

    let mut cuerpo = format!(
        "\
═══════════════════════════════════════
  OMNIPLANNER — Resumen del Día
  {}
═══════════════════════════════════════
",
        hoy
    );

    if !tareas.is_empty() {
        cuerpo.push_str(&format!("\nTAREAS DE HOY ({})\n", tareas.len()));
        for t in tareas {
            let icono = match t.estado {
                crate::tasks::TaskStatus::Completada => "[OK]",
                crate::tasks::TaskStatus::EnProgreso => "[>>]",
                crate::tasks::TaskStatus::Cancelada => "[XX]",
                crate::tasks::TaskStatus::Pendiente => "[  ]",
            };
            cuerpo.push_str(&format!(
                "  {} {} — {} [{}]\n",
                icono,
                t.hora.format("%H:%M"),
                t.titulo,
                t.prioridad
            ));
        }
    }

    if !eventos.is_empty() {
        cuerpo.push_str(&format!("\nEVENTOS DE HOY ({})\n", eventos.len()));
        for e in eventos {
            let fin = e
                .hora_fin
                .map(|h| format!("-{}", h.format("%H:%M")))
                .unwrap_or_default();
            cuerpo.push_str(&format!(
                "  > {}{} {} ({})\n",
                e.hora_inicio.format("%H:%M"),
                fin,
                e.titulo,
                e.tipo
            ));
        }
    }

    if !follow_ups.is_empty() {
        cuerpo.push_str(&format!("\nFOLLOW-UPS PENDIENTES ({})\n", follow_ups.len()));
        for t in follow_ups {
            if let Some(fu) = t.follow_up {
                cuerpo.push_str(&format!(
                    "  -> {} — {} [{}]\n",
                    t.titulo,
                    fu.format("%H:%M"),
                    t.prioridad
                ));
            }
        }
    }

    if tareas.is_empty() && eventos.is_empty() && follow_ups.is_empty() {
        cuerpo.push_str("\n  Día libre — sin compromisos pendientes\n");
    }

    cuerpo.push_str("\n───────────────────────────────────────\nEnviado por OmniPlanner\n");

    enviar_correo(config, &asunto, &cuerpo)
}

pub fn enviar_follow_up(config: &SyncConfig, tarea: &Task, mensaje: &str) -> Result<(), String> {
    let asunto = format!("Follow-up: {}", tarea.titulo);
    let cuerpo = format!(
        "\
═══════════════════════════════════════
  OMNIPLANNER — Follow-up
═══════════════════════════════════════

  Tarea:       {}
  Descripción: {}
  Estado:      {}
  Prioridad:   {}
  Fecha:       {}

  Mensaje:
  {}

───────────────────────────────────────
Enviado por OmniPlanner",
        tarea.titulo,
        if tarea.descripcion.is_empty() {
            "(sin descripción)"
        } else {
            &tarea.descripcion
        },
        tarea.estado,
        tarea.prioridad,
        tarea.fecha.format("%d/%m/%Y"),
        mensaje,
    );

    enviar_correo(config, &asunto, &cuerpo)
}
