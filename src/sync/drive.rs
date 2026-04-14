use super::SyncConfig;

const DRIVE_UPLOAD_URL: &str = "https://www.googleapis.com/upload/drive/v3/files";
const DRIVE_API_URL: &str = "https://www.googleapis.com/drive/v3/files";
const BOUNDARY: &str = "omniplanner_boundary_2026";

/// Sube data.json a Google Drive (crea o actualiza).
/// Retorna el file ID de Drive.
pub fn drive_push(config: &SyncConfig, json: &str) -> Result<String, String> {
    let token = config
        .google_access_token
        .as_ref()
        .ok_or("No autenticado con Google. Re-autentica primero.")?;

    if config.drive_file_id.is_empty() {
        drive_crear(token, json)
    } else {
        drive_actualizar(token, &config.drive_file_id, json)
    }
}

/// Descarga data.json desde Google Drive.
pub fn drive_pull(config: &SyncConfig) -> Result<String, String> {
    let token = config
        .google_access_token
        .as_ref()
        .ok_or("No autenticado con Google. Re-autentica primero.")?;

    if config.drive_file_id.is_empty() {
        return Err("No hay archivo de Drive configurado. Haz push primero.".to_string());
    }

    let url = format!("{}/{}?alt=media", DRIVE_API_URL, config.drive_file_id);

    let resp = ureq::get(&url)
        .set("Authorization", &format!("Bearer {}", token))
        .call()
        .map_err(|e| format!("Error descargando de Drive: {}", e))?;

    resp.into_string()
        .map_err(|e| format!("Error leyendo respuesta de Drive: {}", e))
}

/// Busca si ya existe un archivo omniplanner_data.json en Drive.
/// Útil para recuperar el file_id en un dispositivo nuevo.
pub fn drive_buscar(config: &SyncConfig) -> Result<Option<String>, String> {
    let token = config
        .google_access_token
        .as_ref()
        .ok_or("No autenticado con Google.")?;

    let url = format!(
        "{}?q=name%3D'omniplanner_data.json'+and+trashed%3Dfalse&fields=files(id,name,modifiedTime)&spaces=appDataFolder,drive",
        DRIVE_API_URL
    );

    let resp = ureq::get(&url)
        .set("Authorization", &format!("Bearer {}", token))
        .call()
        .map_err(|e| format!("Error buscando en Drive: {}", e))?;

    let body: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("Error parseando respuesta: {}", e))?;

    if let Some(files) = body["files"].as_array() {
        if let Some(first) = files.first() {
            return Ok(first["id"].as_str().map(|s| s.to_string()));
        }
    }

    Ok(None)
}

// ── Funciones internas ──────────────────────────────────────

fn drive_crear(token: &str, json: &str) -> Result<String, String> {
    // Multipart upload: metadata + contenido
    let metadata = serde_json::json!({
        "name": "omniplanner_data.json",
        "mimeType": "application/json"
    });

    let body = format!(
        "--{boundary}\r\n\
         Content-Type: application/json; charset=UTF-8\r\n\r\n\
         {metadata}\r\n\
         --{boundary}\r\n\
         Content-Type: application/json\r\n\r\n\
         {content}\r\n\
         --{boundary}--",
        boundary = BOUNDARY,
        metadata = metadata,
        content = json
    );

    let resp = ureq::post(&format!("{}?uploadType=multipart", DRIVE_UPLOAD_URL))
        .set("Authorization", &format!("Bearer {}", token))
        .set(
            "Content-Type",
            &format!("multipart/related; boundary={}", BOUNDARY),
        )
        .send_string(&body)
        .map_err(|e| format!("Error subiendo a Drive: {}", e))?;

    let result: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("Error parseando respuesta: {}", e))?;

    result["id"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or("No se obtuvo ID del archivo en Drive".to_string())
}

fn drive_actualizar(token: &str, file_id: &str, json: &str) -> Result<String, String> {
    let metadata = serde_json::json!({
        "name": "omniplanner_data.json",
        "mimeType": "application/json"
    });

    let body = format!(
        "--{boundary}\r\n\
         Content-Type: application/json; charset=UTF-8\r\n\r\n\
         {metadata}\r\n\
         --{boundary}\r\n\
         Content-Type: application/json\r\n\r\n\
         {content}\r\n\
         --{boundary}--",
        boundary = BOUNDARY,
        metadata = metadata,
        content = json
    );

    let url = format!("{}/{}?uploadType=multipart", DRIVE_UPLOAD_URL, file_id);

    let resp = ureq::request("PATCH", &url)
        .set("Authorization", &format!("Bearer {}", token))
        .set(
            "Content-Type",
            &format!("multipart/related; boundary={}", BOUNDARY),
        )
        .send_string(&body)
        .map_err(|e| format!("Error actualizando en Drive: {}", e))?;

    let result: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("Error parseando respuesta: {}", e))?;

    result["id"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or("No se obtuvo ID del archivo actualizado".to_string())
}
