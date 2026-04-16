use super::SyncConfig;

const GIST_API: &str = "https://api.github.com/gists";

/// Sube data.json + README.md a un Gist privado (crea o actualiza).
/// Retorna el gist_id.
pub fn gist_push(config: &SyncConfig, json: &str) -> Result<String, String> {
    if config.gist_token.is_empty() {
        return Err("No hay token de GitHub configurado.".to_string());
    }

    // Generar resumen legible
    let resumen = generar_resumen_md(json);

    if config.gist_id.is_empty() {
        gist_crear(&config.gist_token, json, &resumen)
    } else {
        gist_actualizar(&config.gist_token, &config.gist_id, json, &resumen)
    }
}

/// Descarga data.json desde un Gist.
pub fn gist_pull(config: &SyncConfig) -> Result<String, String> {
    if config.gist_token.is_empty() {
        return Err("No hay token de GitHub configurado.".to_string());
    }
    if config.gist_id.is_empty() {
        return Err("No hay Gist vinculado. Haz push primero o ingresa un Gist ID.".to_string());
    }

    let url = format!("{}/{}", GIST_API, config.gist_id);

    let resp = ureq::get(&url)
        .set("Authorization", &format!("Bearer {}", config.gist_token))
        .set("User-Agent", "OmniPlanner")
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Error descargando Gist: {}", e))?;

    let body: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("Error parseando respuesta: {}", e))?;

    body["files"]["omniplanner_data.json"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or("No se encontró omniplanner_data.json en el Gist.".to_string())
}

/// Busca gists del usuario que contengan omniplanner_data.json.
pub fn gist_buscar(token: &str) -> Result<Option<String>, String> {
    let resp = ureq::get(GIST_API)
        .set("Authorization", &format!("Bearer {}", token))
        .set("User-Agent", "OmniPlanner")
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Error listando Gists: {}", e))?;

    let gists: Vec<serde_json::Value> = resp
        .into_json()
        .map_err(|e| format!("Error parseando respuesta: {}", e))?;

    for gist in &gists {
        if let Some(files) = gist["files"].as_object() {
            if files.contains_key("omniplanner_data.json") {
                if let Some(id) = gist["id"].as_str() {
                    return Ok(Some(id.to_string()));
                }
            }
        }
    }

    Ok(None)
}

// ── Funciones internas ──────────────────────────────────────

fn gist_crear(token: &str, json: &str, resumen: &str) -> Result<String, String> {
    let payload = serde_json::json!({
        "description": "OmniPlanner - datos sincronizados",
        "public": false,
        "files": {
            "omniplanner_data.json": {
                "content": json
            },
            "README.md": {
                "content": resumen
            }
        }
    });

    let resp = ureq::post(GIST_API)
        .set("Authorization", &format!("Bearer {}", token))
        .set("User-Agent", "OmniPlanner")
        .set("Accept", "application/vnd.github+json")
        .send_json(payload)
        .map_err(|e| format!("Error creando Gist: {}", e))?;

    let body: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("Error parseando respuesta: {}", e))?;

    body["id"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or("No se obtuvo ID del Gist creado.".to_string())
}

fn gist_actualizar(
    token: &str,
    gist_id: &str,
    json: &str,
    resumen: &str,
) -> Result<String, String> {
    let url = format!("{}/{}", GIST_API, gist_id);

    let payload = serde_json::json!({
        "files": {
            "omniplanner_data.json": {
                "content": json
            },
            "README.md": {
                "content": resumen
            }
        }
    });

    let resp = ureq::request("PATCH", &url)
        .set("Authorization", &format!("Bearer {}", token))
        .set("User-Agent", "OmniPlanner")
        .set("Accept", "application/vnd.github+json")
        .send_json(payload)
        .map_err(|e| format!("Error actualizando Gist: {}", e))?;

    let body: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("Error parseando respuesta: {}", e))?;

    body["id"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or("No se obtuvo ID del Gist actualizado.".to_string())
}

// ── Generación de resumen legible ───────────────────────────

/// Genera un README.md con el resumen de datos de OmniPlanner
fn generar_resumen_md(json: &str) -> String {
    let data: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return "# OmniPlanner\n\nError generando resumen.".to_string(),
    };

    let mut md = String::with_capacity(4096);
    md.push_str("# 📋 OmniPlanner\n\n");

    // Timestamp
    if let Some(ts) = data["ultima_modificacion"].as_i64() {
        if ts > 0 {
            let dt = chrono::DateTime::from_timestamp(ts, 0);
            if let Some(dt) = dt {
                let local = dt.with_timezone(&chrono::Local);
                md.push_str(&format!(
                    "**Última actualización:** {}\n\n",
                    local.format("%d/%m/%Y %H:%M")
                ));
            }
        }
    }

    // ── Tareas ──
    if let Some(tareas) = data["tasks"]["tareas"].as_array() {
        if !tareas.is_empty() {
            md.push_str("---\n## ✅ Tareas\n\n");

            // Agrupar por estado
            let mut pendientes = Vec::new();
            let mut en_progreso = Vec::new();
            let mut completadas = Vec::new();

            for t in tareas {
                let titulo = t["titulo"].as_str().unwrap_or("Sin título");
                let estado = t["estado"].as_str().unwrap_or("");
                let prioridad = t["prioridad"].as_str().unwrap_or("");
                let fecha = t["fecha"].as_str().unwrap_or("");
                let desc = t["descripcion"].as_str().unwrap_or("");

                let icono_prio = match prioridad {
                    "Urgente" => "🔴",
                    "Alta" => "🟠",
                    "Media" => "🟡",
                    _ => "🟢",
                };

                let linea = if desc.is_empty() {
                    format!("{} **{}** — {} `{}`", icono_prio, titulo, fecha, prioridad)
                } else {
                    format!(
                        "{} **{}** — {} `{}`\n  > {}",
                        icono_prio, titulo, fecha, prioridad, desc
                    )
                };

                match estado {
                    "EnProgreso" => en_progreso.push(linea),
                    "Completada" => completadas.push(linea),
                    "Cancelada" => {}
                    _ => pendientes.push(linea),
                }
            }

            if !en_progreso.is_empty() {
                md.push_str("### 🔄 En Progreso\n");
                for l in &en_progreso {
                    md.push_str(&format!("- {}\n", l));
                }
                md.push('\n');
            }
            if !pendientes.is_empty() {
                md.push_str("### 📌 Pendientes\n");
                for l in &pendientes {
                    md.push_str(&format!("- {}\n", l));
                }
                md.push('\n');
            }
            if !completadas.is_empty() {
                md.push_str(&format!(
                    "<details><summary>✅ Completadas ({})</summary>\n\n",
                    completadas.len()
                ));
                for l in &completadas {
                    md.push_str(&format!("- {}\n", l));
                }
                md.push_str("\n</details>\n\n");
            }
        }
    }

    // ── Agenda / Eventos ──
    if let Some(eventos) = data["agenda"]["eventos"].as_array() {
        if !eventos.is_empty() {
            md.push_str("---\n## 📅 Agenda\n\n");

            // Ordenar por fecha (mostrar los más recientes/próximos)
            let mut evs: Vec<&serde_json::Value> = eventos.iter().collect();
            evs.sort_by(|a, b| {
                let fa = a["fecha"].as_str().unwrap_or("");
                let fb = b["fecha"].as_str().unwrap_or("");
                fa.cmp(fb).reverse()
            });

            let mostrar = evs.len().min(20);
            md.push_str("| Fecha | Hora | Evento | Tipo |\n|-------|------|--------|------|\n");
            for ev in &evs[..mostrar] {
                let titulo = ev["titulo"].as_str().unwrap_or("Sin título");
                let fecha = ev["fecha"].as_str().unwrap_or("");
                let hora = ev["hora_inicio"].as_str().unwrap_or("");
                let tipo = ev["tipo"].as_str().unwrap_or("");
                md.push_str(&format!(
                    "| {} | {} | **{}** | {} |\n",
                    fecha, hora, titulo, tipo
                ));
            }
            if evs.len() > 20 {
                md.push_str(&format!("\n*...y {} eventos más*\n", evs.len() - 20));
            }
            md.push('\n');
        }
    }

    // ── Presupuesto / Deudas ──
    if let Some(meses) = data["presupuesto"]["meses"].as_array() {
        if !meses.is_empty() {
            md.push_str("---\n## 💰 Presupuesto\n\n");
            // Mostrar el mes más reciente
            if let Some(ultimo) = meses.last() {
                let mes = ultimo["mes"].as_str().unwrap_or("?");
                md.push_str(&format!("### {}\n\n", mes));

                if let Some(lineas) = ultimo["lineas"].as_array() {
                    if !lineas.is_empty() {
                        md.push_str("| Categoría | Presupuesto | Gastado |\n|-----------|-------------|----------|\n");
                        for l in lineas {
                            let cat = l["categoria"].as_str().unwrap_or("");
                            let pres = l["presupuesto"].as_f64().unwrap_or(0.0);
                            let gast = l["gastado"].as_f64().unwrap_or(0.0);
                            let icono = if gast > pres { "🔴" } else { "🟢" };
                            md.push_str(&format!(
                                "| {} {} | ${:.2} | ${:.2} |\n",
                                icono, cat, pres, gast
                            ));
                        }
                        md.push('\n');
                    }
                }

                if let Some(deudas) = ultimo["deudas"].as_array() {
                    if !deudas.is_empty() {
                        md.push_str("#### Deudas\n\n");
                        md.push_str("| Deuda | Saldo | Pago Min | Tasa |\n|-------|-------|----------|------|\n");
                        for d in deudas {
                            let nombre = d["nombre"].as_str().unwrap_or("");
                            let saldo = d["saldo_actual"].as_f64().unwrap_or(0.0);
                            let pago = d["pago_minimo"].as_f64().unwrap_or(0.0);
                            let tasa = d["tasa_interes"].as_f64().unwrap_or(0.0);
                            md.push_str(&format!(
                                "| {} | ${:.2} | ${:.2} | {:.1}% |\n",
                                nombre,
                                saldo,
                                pago,
                                tasa * 100.0
                            ));
                        }
                        md.push('\n');
                    }
                }
            }
        }
    }

    // ── Canvas ──
    if let Some(canvases) = data["canvases"].as_array() {
        if !canvases.is_empty() {
            md.push_str("---\n## 🎨 Canvas\n\n");
            for c in canvases {
                let nombre = c["nombre"].as_str().unwrap_or("Sin nombre");
                let elems = c["elementos"].as_array().map(|a| a.len()).unwrap_or(0);
                md.push_str(&format!("- **{}** ({} elementos)\n", nombre, elems));
            }
            md.push('\n');
        }
    }

    // ── Diagramas ──
    if let Some(diagramas) = data["diagramas"].as_array() {
        if !diagramas.is_empty() {
            md.push_str("---\n## 📊 Diagramas\n\n");
            for d in diagramas {
                let nombre = d["nombre"].as_str().unwrap_or("Sin nombre");
                let tipo = d["tipo"].as_str().unwrap_or("");
                let nodos = d["nodos"].as_array().map(|a| a.len()).unwrap_or(0);
                md.push_str(&format!("- **{}** ({}, {} nodos)\n", nombre, tipo, nodos));
            }
            md.push('\n');
        }
    }

    // ── Memoria ──
    if let Some(recuerdos) = data["memoria"]["recuerdos"].as_array() {
        if !recuerdos.is_empty() {
            md.push_str("---\n## 🧠 Memoria\n\n");
            let mostrar = recuerdos.len().min(10);
            for r in &recuerdos[recuerdos.len() - mostrar..] {
                let contenido = r["contenido"].as_str().unwrap_or("");
                let fecha = r["creado"].as_str().unwrap_or("").get(..10).unwrap_or("");
                md.push_str(&format!("- {} — *{}*\n", contenido, fecha));
            }
            if recuerdos.len() > 10 {
                md.push_str(&format!(
                    "\n*...y {} recuerdos más*\n",
                    recuerdos.len() - 10
                ));
            }
            md.push('\n');
        }
    }

    md.push_str("---\n*Generado automáticamente por OmniPlanner*\n");

    md
}
