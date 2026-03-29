use super::SyncConfig;

const GIST_API: &str = "https://api.github.com/gists";

/// Sube data.json a un Gist privado (crea o actualiza).
/// Retorna el gist_id.
pub fn gist_push(config: &SyncConfig, json: &str) -> Result<String, String> {
    if config.gist_token.is_empty() {
        return Err("No hay token de GitHub configurado.".to_string());
    }

    if config.gist_id.is_empty() {
        gist_crear(&config.gist_token, json)
    } else {
        gist_actualizar(&config.gist_token, &config.gist_id, json)
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

fn gist_crear(token: &str, json: &str) -> Result<String, String> {
    let payload = serde_json::json!({
        "description": "OmniPlanner - datos sincronizados",
        "public": false,
        "files": {
            "omniplanner_data.json": {
                "content": json
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

fn gist_actualizar(token: &str, gist_id: &str, json: &str) -> Result<String, String> {
    let url = format!("{}/{}", GIST_API, gist_id);

    let payload = serde_json::json!({
        "files": {
            "omniplanner_data.json": {
                "content": json
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
