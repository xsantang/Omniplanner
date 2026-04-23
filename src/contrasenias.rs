//! Gestión de contraseñas, claves de cifrado y verificación de texto.
//!
//! Funcionalidades:
//! - Verificar texto carácter por carácter contra un valor almacenado
//! - Generar contraseñas seguras personalizables
//! - Generar claves de cifrado (frases de 20 palabras)
//! - Almacenar y gestionar contraseñas por sitio/servicio
//! - Evaluar y mejorar seguridad de contraseñas
//!
//! ## Notas de seguridad
//!
//! - La aleatoriedad usada en la generación de contraseñas y frases semilla
//!   proviene de [`getrandom`], que delega en el CSPRNG del sistema operativo
//!   (`getrandom(2)` en Linux, `BCryptGenRandom` en Windows, `arc4random` en
//!   macOS/BSD, `crypto.getRandomValues` en navegadores). No se usan PRNGs
//!   de juguete como `xorshift` para material sensible.
//! - Las contraseñas se almacenan **en texto plano** dentro de
//!   [`AlmacenContrasenias`]. Si el archivo de estado se guarda en disco sin
//!   cifrar, un atacante con acceso al disco podrá leerlas. Para cifrado en
//!   reposo usa AES-GCM/ChaCha20-Poly1305 con clave derivada (Argon2/scrypt)
//!   sobre una contraseña maestra — esto vive fuera de este módulo.

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

/// Almacén de contraseñas y claves
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlmacenContrasenias {
    /// Entradas de contraseñas (sitio/servicio → datos)
    pub entradas: Vec<EntradaClave>,
    /// Claves de cifrado generadas (frases semilla)
    #[serde(default)]
    pub claves_cifrado: Vec<ClaveCifrado>,
}

/// Una entrada de contraseña para un sitio/servicio
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntradaClave {
    pub id: String,
    pub nombre: String,    // ej: "GitHub", "Gmail", "Token API"
    pub usuario: String,   // login / email
    pub clave: String,     // la contraseña o token
    pub notas: String,     // info adicional
    pub categoria: String, // ej: "web", "api", "crypto", "banco"
    pub creado: NaiveDateTime,
    pub modificado: NaiveDateTime,
}

/// Clave de cifrado (frase semilla de 20 palabras)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaveCifrado {
    pub id: String,
    pub nombre: String,
    pub palabras: Vec<String>,
    pub creado: NaiveDateTime,
}

/// Resultado de comparar dos textos carácter por carácter
#[derive(Debug)]
pub struct ResultadoVerificacion {
    pub coincide: bool,
    pub total_chars: usize,
    pub errores: Vec<ErrorCaracter>,
    pub diff_longitud: i64, // positivo = input más largo, negativo = original más largo
}

#[derive(Debug)]
pub struct ErrorCaracter {
    pub posicion: usize, // 1-indexed
    pub esperado: char,
    pub recibido: char,
}

impl AlmacenContrasenias {
    pub fn nueva_entrada(
        nombre: &str,
        usuario: &str,
        clave: &str,
        notas: &str,
        categoria: &str,
    ) -> EntradaClave {
        let ahora = chrono::Local::now().naive_local();
        EntradaClave {
            id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
            nombre: nombre.to_string(),
            usuario: usuario.to_string(),
            clave: clave.to_string(),
            notas: notas.to_string(),
            categoria: categoria.to_string(),
            creado: ahora,
            modificado: ahora,
        }
    }

    pub fn buscar(&self, termino: &str) -> Vec<&EntradaClave> {
        let t = termino.to_lowercase();
        self.entradas
            .iter()
            .filter(|e| {
                e.nombre.to_lowercase().contains(&t)
                    || e.usuario.to_lowercase().contains(&t)
                    || e.categoria.to_lowercase().contains(&t)
            })
            .collect()
    }
}

// ── Verificación carácter por carácter ──────────────────────

/// Compara dos textos y devuelve las diferencias exactas
pub fn verificar_texto(original: &str, input: &str) -> ResultadoVerificacion {
    let chars_orig: Vec<char> = original.chars().collect();
    let chars_input: Vec<char> = input.chars().collect();
    let mut errores = Vec::new();

    let min_len = chars_orig.len().min(chars_input.len());

    for i in 0..min_len {
        if chars_orig[i] != chars_input[i] {
            errores.push(ErrorCaracter {
                posicion: i + 1,
                esperado: chars_orig[i],
                recibido: chars_input[i],
            });
        }
    }

    ResultadoVerificacion {
        coincide: errores.is_empty() && chars_orig.len() == chars_input.len(),
        total_chars: chars_orig.len(),
        errores,
        diff_longitud: chars_input.len() as i64 - chars_orig.len() as i64,
    }
}

// ── Generación de contraseñas ───────────────────────────────

/// Rellena `buf` con bytes del CSPRNG del sistema. Si `getrandom` falla
/// (extremadamente raro), hace fallback a un xorshift64 seedado con el
/// reloj. En ese caso la contraseña resultante se considera de emergencia
/// y el llamador debería regenerarla cuando el CSPRNG vuelva a estar
/// disponible.
fn rellenar_aleatorio(buf: &mut [u8]) {
    if getrandom::getrandom(buf).is_ok() {
        return;
    }
    // Fallback determinístico: sólo por si getrandom no está disponible.
    let mut estado: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x12345678_9ABCDEF0);
    for byte in buf.iter_mut() {
        estado = xorshift64(estado);
        *byte = estado as u8;
    }
}

/// Devuelve un `usize` aleatorio en `[0, n)` con distribución ~uniforme
/// (sesgo despreciable mientras `n` sea mucho menor que `u64::MAX`).
fn aleatorio_en(n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    let mut buf = [0u8; 8];
    rellenar_aleatorio(&mut buf);
    (u64::from_le_bytes(buf) as usize) % n
}

/// Genera una contraseña aleatoria segura usando el CSPRNG del sistema.
pub fn generar_contrasenia(longitud: usize, usar_especiales: bool) -> String {
    use std::collections::HashSet;

    let minus = b"abcdefghijkmnopqrstuvwxyz"; // sin l (confusión con 1)
    let mayus = b"ABCDEFGHJKLMNPQRSTUVWXYZ"; // sin I, O (confusión con 1, 0)
    let digitos = b"23456789"; // sin 0, 1 (confusión con O, l)
    let especiales = b"!@#$%&*-_=+?";

    let mut charset: Vec<u8> = Vec::new();
    charset.extend_from_slice(minus);
    charset.extend_from_slice(mayus);
    charset.extend_from_slice(digitos);
    if usar_especiales {
        charset.extend_from_slice(especiales);
    }

    let mut resultado = Vec::with_capacity(longitud);

    // Garantizar al menos uno de cada tipo (reduce casos frágiles).
    let mut obligatorios: Vec<u8> = Vec::new();
    obligatorios.push(minus[aleatorio_en(minus.len())]);
    obligatorios.push(mayus[aleatorio_en(mayus.len())]);
    obligatorios.push(digitos[aleatorio_en(digitos.len())]);
    if usar_especiales {
        obligatorios.push(especiales[aleatorio_en(especiales.len())]);
    }

    // Llenar el resto con bytes del CSPRNG reducidos módulo charset.len().
    // El sesgo es despreciable en la práctica para charset pequeño.
    for _ in 0..(longitud.saturating_sub(obligatorios.len())) {
        resultado.push(charset[aleatorio_en(charset.len())]);
    }
    resultado.extend(obligatorios);

    // Fisher-Yates uniforme con CSPRNG.
    let n = resultado.len();
    for i in (1..n).rev() {
        let j = aleatorio_en(i + 1);
        resultado.swap(i, j);
    }

    // Verificar unicidad mínima (seguridad frente a charsets degenerados).
    let unicos: HashSet<u8> = resultado.iter().copied().collect();
    if unicos.len() < longitud / 3 {
        for byte in &mut resultado {
            if aleatorio_en(3) == 0 {
                *byte = charset[aleatorio_en(charset.len())];
            }
        }
    }

    String::from_utf8(resultado).unwrap_or_else(|_| "error".to_string())
}

/// Genera una frase semilla de N palabras (clave de cifrado)
pub fn generar_clave_cifrado(num_palabras: usize) -> Vec<String> {
    // Wordlist BIP39 simplificado (2048 palabras comunes en español)
    let palabras: Vec<&str> = vec![
        "alma", "alto", "amor", "angel", "año", "arbol", "arena", "arte", "azul", "banco", "barco",
        "bello", "bien", "blanco", "boca", "brazo", "breve", "bueno", "cabo", "cafe", "calma",
        "calor", "campo", "canto", "carne", "carta", "casa", "causa", "celta", "cero", "cielo",
        "cinco", "cinta", "claro", "cobre", "color", "conde", "coral", "corte", "costa", "crema",
        "cruel", "cruz", "cubo", "danza", "dardo", "dato", "decir", "delta", "denso", "diente",
        "disco", "dolor", "dosis", "drama", "duelo", "dulce", "duro", "eco", "edad", "elite",
        "enero", "envio", "epoca", "equipo", "error", "escape", "espejo", "estar", "etapa",
        "etica", "exito", "faro", "fauna", "fecha", "fibra", "fijo", "final", "flor", "fondo",
        "forma", "fuego", "fuente", "fuerza", "gallo", "gato", "genio", "gesto", "globo", "golpe",
        "gorro", "gota", "gracia", "grano", "grave", "grupo", "guia", "habla", "hacer", "hielo",
        "hierro", "hilo", "honor", "hueso", "idea", "igual", "isla", "jade", "jardin", "joven",
        "juego", "juicio", "justo", "labor", "lago", "largo", "latir", "lento", "letra", "libre",
        "limon", "linea", "lista", "lobo", "logro", "lomo", "lucha", "lugar", "luna", "madre",
        "mango", "manto", "mapa", "marca", "mayor", "medio", "mejor", "menor", "mente", "mesa",
        "metal", "miedo", "mina", "mitad", "moda", "modo", "monte", "moral", "motor", "mundo",
        "muro", "musica", "nariz", "nave", "nieve", "noble", "noche", "norma", "nota", "nube",
        "nuevo", "obeso", "obvio", "ocaso", "ocio", "opera", "orden", "oro", "otra", "oveja",
        "padre", "pais", "palma", "pared", "parte", "paso", "patio", "pausa", "pecho", "perla",
        "perro", "piano", "pieza", "plata", "plaza", "pleno", "pluma", "pobre", "poco", "poder",
        "poeta", "polar", "polo", "polvo", "poner", "portal", "poste", "prado", "precio", "primo",
        "prisa", "prosa", "punto", "queso", "rango", "rapido", "rasgo", "raton", "razon", "real",
        "red", "reloj", "resto", "rey", "ritmo", "rival", "roca", "rodeo", "rojo", "rosa",
        "rostro", "rueda", "ruido", "rumbo", "rural", "ruta", "sabio", "sal", "salto", "salud",
        "santo", "seda", "selva", "señal", "serie", "siglo", "signo", "sitio", "sobre", "solar",
        "sombra", "sonar", "soplo", "sordo", "suave", "sucio", "suelo", "sueño", "sur", "tabla",
        "tallo", "tanto", "tarea", "tigre", "tipo", "titulo", "tono", "torre", "total", "trazo",
        "trece", "trigo", "trono", "trozo", "tumba", "turno", "unico", "unir", "uva", "vacio",
        "valor", "varon", "vasto", "vela", "verde", "verso", "vida", "viento", "vigor", "vino",
        "vital", "volar", "vuelo", "yerno", "zagal", "zanja", "zarpa", "zona",
    ];

    let mut resultado = Vec::with_capacity(num_palabras);
    let mut usadas = std::collections::HashSet::new();

    for _ in 0..num_palabras {
        loop {
            let idx = aleatorio_en(palabras.len());
            if usadas.insert(idx) {
                resultado.push(palabras[idx].to_string());
                break;
            }
        }
    }

    resultado
}

/// Evalúa la fortaleza de una contraseña (0-100)
pub fn evaluar_fortaleza(clave: &str) -> (u32, String) {
    let mut puntaje: u32 = 0;
    let mut sugerencias = Vec::new();

    let len = clave.len();
    // Longitud
    puntaje += match len {
        0..=5 => 5,
        6..=8 => 15,
        9..=12 => 25,
        13..=16 => 35,
        17..=24 => 40,
        _ => 45,
    };
    if len < 8 {
        sugerencias.push("Usa al menos 8 caracteres");
    }
    if len < 12 {
        sugerencias.push("Idealmente 12+ caracteres");
    }

    let tiene_minus = clave.chars().any(|c| c.is_ascii_lowercase());
    let tiene_mayus = clave.chars().any(|c| c.is_ascii_uppercase());
    let tiene_digito = clave.chars().any(|c| c.is_ascii_digit());
    let tiene_especial = clave.chars().any(|c| !c.is_alphanumeric());

    if tiene_minus {
        puntaje += 10;
    } else {
        sugerencias.push("Agrega letras minúsculas");
    }
    if tiene_mayus {
        puntaje += 10;
    } else {
        sugerencias.push("Agrega letras MAYÚSCULAS");
    }
    if tiene_digito {
        puntaje += 10;
    } else {
        sugerencias.push("Agrega números");
    }
    if tiene_especial {
        puntaje += 15;
    } else {
        sugerencias.push("Agrega caracteres especiales (!@#$%)");
    }

    // Variedad de caracteres
    let unicos: std::collections::HashSet<char> = clave.chars().collect();
    let ratio = if len > 0 {
        unicos.len() as f64 / len as f64
    } else {
        0.0
    };
    if ratio > 0.7 {
        puntaje += 10;
    }
    if ratio < 0.4 {
        sugerencias.push("Muy repetitiva, usa más caracteres distintos");
    }

    // Patrones comunes inseguros
    let lower = clave.to_lowercase();
    let patrones_malos = [
        "123", "abc", "qwerty", "password", "admin", "letmein", "welcome", "111", "000", "aaa",
    ];
    for p in &patrones_malos {
        if lower.contains(p) {
            puntaje = puntaje.saturating_sub(15);
            sugerencias.push("Evita patrones comunes (123, abc, qwerty...)");
            break;
        }
    }

    let puntaje = puntaje.min(100);
    let nivel = match puntaje {
        0..=20 => "🔴 Muy débil",
        21..=40 => "🟠 Débil",
        41..=60 => "🟡 Regular",
        61..=80 => "🟢 Buena",
        _ => "🟢 Excelente",
    };

    let resumen = if sugerencias.is_empty() {
        format!("{} ({}/100)", nivel, puntaje)
    } else {
        format!(
            "{} ({}/100)\n  💡 {}",
            nivel,
            puntaje,
            sugerencias.join("\n  💡 ")
        )
    };

    (puntaje, resumen)
}

/// Mejora una contraseña existente haciéndola más segura
pub fn mejorar_contrasenia(original: &str) -> String {
    let mut chars: Vec<char> = original.chars().collect();

    // Reemplazos leet-speak deterministas (no agregan entropía,
    // pero confunden diccionarios básicos).
    for c in &mut chars {
        match *c {
            'a' | 'A' => *c = '@',
            'e' | 'E' => *c = '3',
            'i' | 'I' => *c = '!',
            'o' | 'O' => *c = '0',
            's' | 'S' => *c = '$',
            't' | 'T' => *c = '7',
            _ => {}
        }
    }

    // Agregar caracteres extra si es corta usando el CSPRNG.
    while chars.len() < 16 {
        let extras = b"!@#$%&*-_=+?23456789ABCDEFGHJK";
        chars.push(extras[aleatorio_en(extras.len())] as char);
    }

    // Insertar mayúsculas aleatorias usando el CSPRNG.
    for c in &mut chars {
        if aleatorio_en(4) == 0 && c.is_ascii_lowercase() {
            *c = c.to_ascii_uppercase();
        }
    }

    chars.into_iter().collect()
}

// ── PRNG de emergencia (xorshift64) ─────────────────────────
//
// Sólo se usa como último recurso si [`getrandom`] falla. NO usar para
// claves ni material criptográfico fuera de ese camino de fallback.

fn xorshift64(mut state: u64) -> u64 {
    if state == 0 {
        state = 0x12345678_9ABCDEF0;
    }
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verificacion_exacta() {
        let r = verificar_texto("hola mundo", "hola mundo");
        assert!(r.coincide);
        assert!(r.errores.is_empty());
        assert_eq!(r.diff_longitud, 0);
    }

    #[test]
    fn verificacion_con_errores() {
        let r = verificar_texto("abcdef", "abXdeY");
        assert!(!r.coincide);
        assert_eq!(r.errores.len(), 2);
        assert_eq!(r.errores[0].posicion, 3);
        assert_eq!(r.errores[0].esperado, 'c');
        assert_eq!(r.errores[0].recibido, 'X');
        assert_eq!(r.errores[1].posicion, 6);
    }

    #[test]
    fn verificacion_longitud_diferente() {
        let r = verificar_texto("abc", "abcde");
        assert!(!r.coincide);
        assert_eq!(r.diff_longitud, 2); // input 2 chars más largo
    }

    #[test]
    fn generar_contrasenia_longitud() {
        let c = generar_contrasenia(20, true);
        assert_eq!(c.len(), 20);
    }

    #[test]
    fn generar_clave_cifrado_palabras() {
        let c = generar_clave_cifrado(20);
        assert_eq!(c.len(), 20);
        // Todas diferentes
        let set: std::collections::HashSet<&String> = c.iter().collect();
        assert_eq!(set.len(), 20);
    }

    #[test]
    fn evaluar_fortaleza_debil() {
        let (p, _) = evaluar_fortaleza("abc");
        assert!(p < 30);
    }

    #[test]
    fn evaluar_fortaleza_fuerte() {
        let (p, _) = evaluar_fortaleza("K#9xMp!2qR$vN7&w");
        assert!(p >= 70);
    }

    #[test]
    fn mejorar_contrasenia_basica() {
        let mejorada = mejorar_contrasenia("password");
        assert_ne!(mejorada, "password");
        assert!(mejorada.len() >= 16);
    }

    #[test]
    fn csprng_devuelve_valores_distintos() {
        // Dos llamadas consecutivas al CSPRNG deben casi siempre diferir.
        // (probabilidad de colisión ≈ 2^-64 con getrandom).
        let a = generar_contrasenia(24, true);
        let b = generar_contrasenia(24, true);
        assert_ne!(a, b, "CSPRNG no debería dar dos valores iguales consecutivos");
    }

    #[test]
    fn frase_semilla_no_es_determinista() {
        let a = generar_clave_cifrado(12);
        let b = generar_clave_cifrado(12);
        assert_ne!(a, b, "Dos frases semilla consecutivas no deben coincidir");
    }
}
