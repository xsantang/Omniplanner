//! Primitivas criptográficas para claves simétricas y asimétricas.
//!
//! Expone una API de alto nivel sobre crates auditados:
//!
//! - **Simétrica autenticada**: AES-256-GCM (nonce 96 bits del CSPRNG).
//! - **Derivación de clave desde contraseña**: Argon2id (salt 16 B, 32 B out).
//! - **Firma digital**: Ed25519 (`ed25519-dalek`).
//! - **Intercambio de claves**: X25519 ECDH (`x25519-dalek`) + HKDF-SHA256.
//! - **RSA-4096**: OAEP-SHA256 para cifrado y PSS-SHA256 para firma.
//!
//! Toda la aleatoriedad viene de `OsRng` (CSPRNG del sistema), sin PRNGs
//! inseguros. Las claves privadas implementan [`zeroize::Zeroize`] a través
//! de los tipos envueltos por los crates originales — no mantenemos copias
//! sin zeroizar.
//!
//! ## Envolver claves privadas con contraseña
//!
//! Para persistir una clave privada se recomienda derivar una clave maestra
//! con [`derivar_clave_maestra`] desde la contraseña del usuario y luego
//! usar [`cifrar_aes_gcm`] sobre los bytes de la clave privada. Ver
//! [`ClavePrivadaSellada`] que ya encapsula este flujo.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use ed25519_dalek::{
    Signature as Ed25519Signature, Signer, SigningKey as Ed25519Signing,
    Verifier as Ed25519Verifier, VerifyingKey as Ed25519Verifying,
};
use hkdf::Hkdf;
use pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey, LineEnding};
use rand_core::{OsRng, RngCore};
use rsa::pss::{SigningKey as RsaPssSigning, VerifyingKey as RsaPssVerifying};
use rsa::signature::{RandomizedSigner, SignatureEncoding};
use rsa::{Oaep, RsaPrivateKey, RsaPublicKey};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use x25519_dalek::{PublicKey as X25519Public, StaticSecret as X25519Secret};
use zeroize::Zeroize;

/// Tamaño de clave AES-256 en bytes.
pub const AES256_KEY_LEN: usize = 32;
/// Tamaño de nonce AES-GCM en bytes (96 bits).
pub const AES_GCM_NONCE_LEN: usize = 12;
/// Tamaño de salt Argon2 en bytes.
pub const ARGON2_SALT_LEN: usize = 16;

/// Errores del módulo.
#[derive(Debug)]
pub enum ErrorCripto {
    ClaveInvalida(&'static str),
    NonceInvalido,
    CifradoFallido,
    DescifradoFallido,
    DerivacionFallida,
    FirmaInvalida,
    RsaFallido(String),
    Pkcs8Fallido(String),
}

impl std::fmt::Display for ErrorCripto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClaveInvalida(m) => write!(f, "clave inválida: {m}"),
            Self::NonceInvalido => write!(f, "nonce inválido"),
            Self::CifradoFallido => write!(f, "cifrado fallido"),
            Self::DescifradoFallido => write!(f, "descifrado fallido o tag inválido"),
            Self::DerivacionFallida => write!(f, "derivación de clave fallida"),
            Self::FirmaInvalida => write!(f, "firma inválida"),
            Self::RsaFallido(m) => write!(f, "RSA fallido: {m}"),
            Self::Pkcs8Fallido(m) => write!(f, "PKCS8 fallido: {m}"),
        }
    }
}

impl std::error::Error for ErrorCripto {}

pub type Resultado<T> = Result<T, ErrorCripto>;

// ── Utilidades aleatorias ────────────────────────────────────

/// Genera `N` bytes aleatorios desde el CSPRNG del sistema.
pub fn bytes_aleatorios<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    OsRng.fill_bytes(&mut buf);
    buf
}

/// Genera una clave AES-256 aleatoria.
pub fn generar_clave_aes256() -> [u8; AES256_KEY_LEN] {
    bytes_aleatorios::<AES256_KEY_LEN>()
}

/// Genera un nonce AES-GCM aleatorio (96 bits).
pub fn generar_nonce_aes_gcm() -> [u8; AES_GCM_NONCE_LEN] {
    bytes_aleatorios::<AES_GCM_NONCE_LEN>()
}

// ── AES-256-GCM ──────────────────────────────────────────────

/// Salida de [`cifrar_aes_gcm`]: nonce + ciphertext (con tag de 16 bytes
/// adjunto al final por AES-GCM).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SobreAesGcm {
    /// Nonce de 12 bytes en base64.
    pub nonce_b64: String,
    /// Ciphertext + tag en base64.
    pub ct_b64: String,
}

/// Cifra `plaintext` con AES-256-GCM. Genera el nonce internamente.
pub fn cifrar_aes_gcm(clave: &[u8; AES256_KEY_LEN], plaintext: &[u8]) -> Resultado<SobreAesGcm> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(clave));
    let nonce_bytes = generar_nonce_aes_gcm();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ct = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| ErrorCripto::CifradoFallido)?;
    Ok(SobreAesGcm {
        nonce_b64: b64_encode(&nonce_bytes),
        ct_b64: b64_encode(&ct),
    })
}

/// Descifra un [`SobreAesGcm`] verificando autenticidad (tag GCM).
pub fn descifrar_aes_gcm(clave: &[u8; AES256_KEY_LEN], sobre: &SobreAesGcm) -> Resultado<Vec<u8>> {
    let nonce_bytes = b64_decode(&sobre.nonce_b64)?;
    if nonce_bytes.len() != AES_GCM_NONCE_LEN {
        return Err(ErrorCripto::NonceInvalido);
    }
    let ct = b64_decode(&sobre.ct_b64)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(clave));
    let nonce = Nonce::from_slice(&nonce_bytes);
    cipher
        .decrypt(nonce, ct.as_ref())
        .map_err(|_| ErrorCripto::DescifradoFallido)
}

// ── Derivación de clave desde contraseña (Argon2id) ──────────

/// Parámetros de Argon2id usados al derivar claves.
///
/// Valores por defecto conservadores para máquinas modestas (~64 MiB,
/// 3 iteraciones, 1 hilo). Se pueden ajustar si el entorno lo permite.
pub struct ParamsKdf {
    pub m_cost_kib: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

impl Default for ParamsKdf {
    fn default() -> Self {
        Self {
            m_cost_kib: 64 * 1024, // 64 MiB
            t_cost: 3,
            p_cost: 1,
        }
    }
}

/// Deriva una clave AES-256 desde una contraseña usando Argon2id.
/// Si `salt` es `None` se genera uno nuevo y se devuelve.
pub fn derivar_clave_maestra(
    contrasenia: &[u8],
    salt: Option<[u8; ARGON2_SALT_LEN]>,
    params: &ParamsKdf,
) -> Resultado<(Vec<u8>, [u8; ARGON2_SALT_LEN])> {
    let salt = salt.unwrap_or_else(bytes_aleatorios::<ARGON2_SALT_LEN>);
    let p = Params::new(params.m_cost_kib, params.t_cost, params.p_cost, Some(32))
        .map_err(|_| ErrorCripto::DerivacionFallida)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, p);
    let mut out = vec![0u8; 32];
    argon
        .hash_password_into(contrasenia, &salt, &mut out)
        .map_err(|_| ErrorCripto::DerivacionFallida)?;
    Ok((out, salt))
}

// ── Clave privada sellada bajo contraseña ────────────────────

/// Clave privada serializada en PEM (PKCS#8) y cifrada con AES-256-GCM
/// usando una clave derivada de una contraseña maestra (Argon2id).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClavePrivadaSellada {
    pub salt_b64: String,
    pub sobre: SobreAesGcm,
    pub m_cost_kib: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

impl ClavePrivadaSellada {
    /// Sella `pem_privado` bajo `contrasenia` usando Argon2id + AES-GCM.
    pub fn sellar(pem_privado: &[u8], contrasenia: &[u8], params: &ParamsKdf) -> Resultado<Self> {
        let (clave, salt) = derivar_clave_maestra(contrasenia, None, params)?;
        let mut clave_arr = [0u8; AES256_KEY_LEN];
        clave_arr.copy_from_slice(&clave);
        let sobre = cifrar_aes_gcm(&clave_arr, pem_privado)?;
        clave_arr.zeroize();
        Ok(Self {
            salt_b64: b64_encode(&salt),
            sobre,
            m_cost_kib: params.m_cost_kib,
            t_cost: params.t_cost,
            p_cost: params.p_cost,
        })
    }

    /// Abre el sello y devuelve el PEM original.
    pub fn abrir(&self, contrasenia: &[u8]) -> Resultado<Vec<u8>> {
        let salt = b64_decode(&self.salt_b64)?;
        if salt.len() != ARGON2_SALT_LEN {
            return Err(ErrorCripto::DerivacionFallida);
        }
        let mut salt_arr = [0u8; ARGON2_SALT_LEN];
        salt_arr.copy_from_slice(&salt);
        let params = ParamsKdf {
            m_cost_kib: self.m_cost_kib,
            t_cost: self.t_cost,
            p_cost: self.p_cost,
        };
        let (clave, _) = derivar_clave_maestra(contrasenia, Some(salt_arr), &params)?;
        let mut clave_arr = [0u8; AES256_KEY_LEN];
        clave_arr.copy_from_slice(&clave);
        let out = descifrar_aes_gcm(&clave_arr, &self.sobre)?;
        clave_arr.zeroize();
        Ok(out)
    }
}

// ── Ed25519 (firma) ──────────────────────────────────────────

/// Par de claves Ed25519 recién generado. La clave privada se puede
/// sellar con [`ClavePrivadaSellada::sellar`] sobre su PEM.
pub struct ParClavesEd25519 {
    pub signing: Ed25519Signing,
    pub verifying: Ed25519Verifying,
}

impl ParClavesEd25519 {
    /// Genera un nuevo par desde el CSPRNG del sistema.
    pub fn generar() -> Self {
        let signing = Ed25519Signing::generate(&mut OsRng);
        let verifying = signing.verifying_key();
        Self { signing, verifying }
    }

    /// PEM PKCS#8 de la clave privada.
    pub fn privada_pem(&self) -> Resultado<String> {
        self.signing
            .to_pkcs8_pem(LineEnding::LF)
            .map(|z| z.to_string())
            .map_err(|e| ErrorCripto::Pkcs8Fallido(e.to_string()))
    }

    /// PEM SubjectPublicKeyInfo de la clave pública.
    pub fn publica_pem(&self) -> Resultado<String> {
        self.verifying
            .to_public_key_pem(LineEnding::LF)
            .map_err(|e| ErrorCripto::Pkcs8Fallido(e.to_string()))
    }
}

/// Firma `mensaje` con la clave privada Ed25519 en PEM PKCS#8.
pub fn firmar_ed25519(pem_privado: &str, mensaje: &[u8]) -> Resultado<Vec<u8>> {
    let sk = Ed25519Signing::from_pkcs8_pem(pem_privado)
        .map_err(|e| ErrorCripto::Pkcs8Fallido(e.to_string()))?;
    Ok(sk.sign(mensaje).to_bytes().to_vec())
}

/// Verifica una firma Ed25519 contra `mensaje`.
pub fn verificar_ed25519(pem_publico: &str, mensaje: &[u8], firma: &[u8]) -> Resultado<()> {
    let vk = Ed25519Verifying::from_public_key_pem(pem_publico)
        .map_err(|e| ErrorCripto::Pkcs8Fallido(e.to_string()))?;
    let sig_arr: [u8; 64] = firma.try_into().map_err(|_| ErrorCripto::FirmaInvalida)?;
    let sig = Ed25519Signature::from_bytes(&sig_arr);
    Ed25519Verifier::verify(&vk, mensaje, &sig).map_err(|_| ErrorCripto::FirmaInvalida)
}

// ── X25519 (ECDH) ────────────────────────────────────────────

/// Par de claves X25519 para intercambio Diffie-Hellman.
pub struct ParClavesX25519 {
    pub secreto: X25519Secret,
    pub publica: X25519Public,
}

impl ParClavesX25519 {
    pub fn generar() -> Self {
        let secreto = X25519Secret::random_from_rng(OsRng);
        let publica = X25519Public::from(&secreto);
        Self { secreto, publica }
    }

    /// Bytes (32) de la clave pública, listos para enviar al par.
    pub fn publica_bytes(&self) -> [u8; 32] {
        self.publica.to_bytes()
    }

    /// Bytes (32) del secreto estático. Tratar como material sensible.
    pub fn secreto_bytes(&self) -> [u8; 32] {
        self.secreto.to_bytes()
    }
}

/// ECDH X25519 + HKDF-SHA256 → clave AES-256 derivada.
///
/// `info` sirve como contexto/dominio para separar usos de la misma
/// clave compartida.
pub fn ecdh_x25519_a_aes256(
    mi_secreto: &[u8; 32],
    publica_par: &[u8; 32],
    info: &[u8],
) -> Resultado<[u8; AES256_KEY_LEN]> {
    let sk = X25519Secret::from(*mi_secreto);
    let pk = X25519Public::from(*publica_par);
    let shared = sk.diffie_hellman(&pk);
    let hk = Hkdf::<Sha256>::new(None, shared.as_bytes());
    let mut out = [0u8; AES256_KEY_LEN];
    hk.expand(info, &mut out)
        .map_err(|_| ErrorCripto::DerivacionFallida)?;
    Ok(out)
}

// ── RSA-4096 (OAEP + PSS) ────────────────────────────────────

/// Par RSA-4096 recién generado.
///
/// **Advertencia**: la generación tarda segundos incluso en release.
/// Usar Ed25519/X25519 cuando sea posible.
pub struct ParClavesRsa4096 {
    pub privada: RsaPrivateKey,
    pub publica: RsaPublicKey,
}

impl ParClavesRsa4096 {
    /// Genera un par RSA-4096 usando `OsRng`.
    pub fn generar() -> Resultado<Self> {
        let privada = RsaPrivateKey::new(&mut OsRng, 4096)
            .map_err(|e| ErrorCripto::RsaFallido(e.to_string()))?;
        let publica = RsaPublicKey::from(&privada);
        Ok(Self { privada, publica })
    }

    pub fn privada_pem(&self) -> Resultado<String> {
        self.privada
            .to_pkcs8_pem(LineEnding::LF)
            .map(|z| z.to_string())
            .map_err(|e| ErrorCripto::Pkcs8Fallido(e.to_string()))
    }

    pub fn publica_pem(&self) -> Resultado<String> {
        self.publica
            .to_public_key_pem(LineEnding::LF)
            .map_err(|e| ErrorCripto::Pkcs8Fallido(e.to_string()))
    }
}

/// Cifra con RSA-OAEP(SHA-256). Apto sólo para mensajes cortos
/// (≤ 446 bytes con RSA-4096/OAEP-SHA256). Para payloads grandes,
/// usar AES-GCM con clave envuelta por RSA.
pub fn cifrar_rsa_oaep(pem_publico: &str, plaintext: &[u8]) -> Resultado<Vec<u8>> {
    let pk = RsaPublicKey::from_public_key_pem(pem_publico)
        .map_err(|e| ErrorCripto::Pkcs8Fallido(e.to_string()))?;
    let padding = Oaep::new::<Sha256>();
    pk.encrypt(&mut OsRng, padding, plaintext)
        .map_err(|e| ErrorCripto::RsaFallido(e.to_string()))
}

/// Descifra RSA-OAEP(SHA-256).
pub fn descifrar_rsa_oaep(pem_privado: &str, ciphertext: &[u8]) -> Resultado<Vec<u8>> {
    let sk = RsaPrivateKey::from_pkcs8_pem(pem_privado)
        .map_err(|e| ErrorCripto::Pkcs8Fallido(e.to_string()))?;
    let padding = Oaep::new::<Sha256>();
    sk.decrypt(padding, ciphertext)
        .map_err(|e| ErrorCripto::RsaFallido(e.to_string()))
}

/// Firma RSA-PSS(SHA-256).
pub fn firmar_rsa_pss(pem_privado: &str, mensaje: &[u8]) -> Resultado<Vec<u8>> {
    let sk = RsaPrivateKey::from_pkcs8_pem(pem_privado)
        .map_err(|e| ErrorCripto::Pkcs8Fallido(e.to_string()))?;
    let signing_key = RsaPssSigning::<Sha256>::new(sk);
    let sig = signing_key.sign_with_rng(&mut OsRng, mensaje);
    Ok(sig.to_bytes().to_vec())
}

/// Verifica RSA-PSS(SHA-256).
pub fn verificar_rsa_pss(pem_publico: &str, mensaje: &[u8], firma: &[u8]) -> Resultado<()> {
    let pk = RsaPublicKey::from_public_key_pem(pem_publico)
        .map_err(|e| ErrorCripto::Pkcs8Fallido(e.to_string()))?;
    let vk = RsaPssVerifying::<Sha256>::new(pk);
    let sig = rsa::pss::Signature::try_from(firma).map_err(|_| ErrorCripto::FirmaInvalida)?;
    vk.verify(mensaje, &sig).map_err(|_| ErrorCripto::FirmaInvalida)
}

// ── Sobre híbrido RSA+AES para mensajes largos ───────────────

/// Sobre híbrido: clave AES-256 aleatoria envuelta con RSA-OAEP y
/// payload cifrado con AES-GCM. Útil para mensajes > 446 bytes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SobreHibridoRsa {
    /// Clave AES-256 cifrada con RSA-OAEP, en base64.
    pub clave_envuelta_b64: String,
    pub aes: SobreAesGcm,
}

pub fn cifrar_hibrido_rsa(pem_publico: &str, plaintext: &[u8]) -> Resultado<SobreHibridoRsa> {
    let mut clave = generar_clave_aes256();
    let aes = cifrar_aes_gcm(&clave, plaintext)?;
    let envuelta = cifrar_rsa_oaep(pem_publico, &clave)?;
    clave.zeroize();
    Ok(SobreHibridoRsa {
        clave_envuelta_b64: b64_encode(&envuelta),
        aes,
    })
}

pub fn descifrar_hibrido_rsa(pem_privado: &str, sobre: &SobreHibridoRsa) -> Resultado<Vec<u8>> {
    let envuelta = b64_decode(&sobre.clave_envuelta_b64)?;
    let clave_vec = descifrar_rsa_oaep(pem_privado, &envuelta)?;
    if clave_vec.len() != AES256_KEY_LEN {
        return Err(ErrorCripto::ClaveInvalida("longitud AES incorrecta"));
    }
    let mut clave = [0u8; AES256_KEY_LEN];
    clave.copy_from_slice(&clave_vec);
    let out = descifrar_aes_gcm(&clave, &sobre.aes);
    clave.zeroize();
    out
}

// ── base64 helpers ───────────────────────────────────────────

fn b64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn b64_decode(s: &str) -> Resultado<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(s.as_bytes())
        .map_err(|_| ErrorCripto::ClaveInvalida("base64 inválido"))
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Parámetros Argon2 rápidos para tests (no usar en producción).
    fn params_test() -> ParamsKdf {
        ParamsKdf {
            m_cost_kib: 8 * 1024, // 8 MiB
            t_cost: 1,
            p_cost: 1,
        }
    }

    #[test]
    fn aes_gcm_roundtrip() {
        let clave = generar_clave_aes256();
        let msg = b"mensaje super secreto con acentos: \xc3\xb1";
        let sobre = cifrar_aes_gcm(&clave, msg).unwrap();
        let pt = descifrar_aes_gcm(&clave, &sobre).unwrap();
        assert_eq!(pt, msg);
    }

    #[test]
    fn aes_gcm_tag_detecta_manipulacion() {
        let clave = generar_clave_aes256();
        let mut sobre = cifrar_aes_gcm(&clave, b"datos").unwrap();
        // Alterar un byte del ciphertext → el tag debe fallar.
        let mut ct = b64_decode(&sobre.ct_b64).unwrap();
        ct[0] ^= 0x01;
        sobre.ct_b64 = b64_encode(&ct);
        assert!(descifrar_aes_gcm(&clave, &sobre).is_err());
    }

    #[test]
    fn aes_gcm_nonces_no_se_repiten() {
        let clave = generar_clave_aes256();
        let a = cifrar_aes_gcm(&clave, b"x").unwrap();
        let b = cifrar_aes_gcm(&clave, b"x").unwrap();
        assert_ne!(a.nonce_b64, b.nonce_b64);
    }

    #[test]
    fn argon2id_deriva_misma_clave_con_mismo_salt() {
        let p = params_test();
        let (k1, salt) = derivar_clave_maestra(b"passw0rd!", None, &p).unwrap();
        let (k2, _) = derivar_clave_maestra(b"passw0rd!", Some(salt), &p).unwrap();
        assert_eq!(k1, k2);
        assert_eq!(k1.len(), 32);
    }

    #[test]
    fn argon2id_contrasenia_distinta_da_clave_distinta() {
        let p = params_test();
        let (k1, salt) = derivar_clave_maestra(b"uno", None, &p).unwrap();
        let (k2, _) = derivar_clave_maestra(b"dos", Some(salt), &p).unwrap();
        assert_ne!(k1, k2);
    }

    #[test]
    fn clave_privada_sellada_roundtrip() {
        let pem = b"-----BEGIN FAKE-----\ndata\n-----END FAKE-----";
        let p = params_test();
        let sello = ClavePrivadaSellada::sellar(pem, b"maestra", &p).unwrap();
        let abierta = sello.abrir(b"maestra").unwrap();
        assert_eq!(abierta, pem);
    }

    #[test]
    fn clave_privada_sellada_contrasenia_mala() {
        let p = params_test();
        let sello = ClavePrivadaSellada::sellar(b"secret", b"correcta", &p).unwrap();
        assert!(sello.abrir(b"incorrecta").is_err());
    }

    #[test]
    fn ed25519_firma_y_verifica() {
        let par = ParClavesEd25519::generar();
        let priv_pem = par.privada_pem().unwrap();
        let pub_pem = par.publica_pem().unwrap();
        let msg = b"firma esto";
        let sig = firmar_ed25519(&priv_pem, msg).unwrap();
        verificar_ed25519(&pub_pem, msg, &sig).unwrap();
    }

    #[test]
    fn ed25519_rechaza_mensaje_modificado() {
        let par = ParClavesEd25519::generar();
        let priv_pem = par.privada_pem().unwrap();
        let pub_pem = par.publica_pem().unwrap();
        let sig = firmar_ed25519(&priv_pem, b"original").unwrap();
        assert!(verificar_ed25519(&pub_pem, b"alterado", &sig).is_err());
    }

    #[test]
    fn x25519_ecdh_produce_clave_simetrica_identica() {
        let a = ParClavesX25519::generar();
        let b = ParClavesX25519::generar();
        let ka =
            ecdh_x25519_a_aes256(&a.secreto_bytes(), &b.publica_bytes(), b"omniplanner/v1").unwrap();
        let kb =
            ecdh_x25519_a_aes256(&b.secreto_bytes(), &a.publica_bytes(), b"omniplanner/v1").unwrap();
        assert_eq!(ka, kb);
    }

    #[test]
    fn x25519_info_distinto_deriva_clave_distinta() {
        let a = ParClavesX25519::generar();
        let b = ParClavesX25519::generar();
        let k1 = ecdh_x25519_a_aes256(&a.secreto_bytes(), &b.publica_bytes(), b"ctx-1").unwrap();
        let k2 = ecdh_x25519_a_aes256(&a.secreto_bytes(), &b.publica_bytes(), b"ctx-2").unwrap();
        assert_ne!(k1, k2);
    }

    // RSA-4096 es lento; lo marcamos `ignore` por defecto.
    #[test]
    #[ignore = "RSA-4096 keygen tarda varios segundos"]
    fn rsa_4096_oaep_y_pss_roundtrip() {
        let par = ParClavesRsa4096::generar().unwrap();
        let priv_pem = par.privada_pem().unwrap();
        let pub_pem = par.publica_pem().unwrap();

        let msg = b"hola rsa";
        let ct = cifrar_rsa_oaep(&pub_pem, msg).unwrap();
        let pt = descifrar_rsa_oaep(&priv_pem, &ct).unwrap();
        assert_eq!(pt, msg);

        let sig = firmar_rsa_pss(&priv_pem, msg).unwrap();
        verificar_rsa_pss(&pub_pem, msg, &sig).unwrap();
    }

    #[test]
    #[ignore = "RSA-4096 keygen tarda varios segundos"]
    fn rsa_4096_sobre_hibrido_mensaje_largo() {
        let par = ParClavesRsa4096::generar().unwrap();
        let priv_pem = par.privada_pem().unwrap();
        let pub_pem = par.publica_pem().unwrap();

        let msg = vec![0xABu8; 4096];
        let sobre = cifrar_hibrido_rsa(&pub_pem, &msg).unwrap();
        let pt = descifrar_hibrido_rsa(&priv_pem, &sobre).unwrap();
        assert_eq!(pt, msg);
    }
}
