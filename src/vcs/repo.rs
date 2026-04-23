//! Source control sobre archivos estilo Git minimalista.
//!
//! Guarda el historial bajo `.omnivcs/` dentro del directorio de trabajo.
//! Hash de contenido SHA-256 (idéntico al usado por [`super::DataVcs`]).
//!
//! Layout del repositorio:
//!
//! ```text
//! .omnivcs/
//! ├── HEAD                ← "ref: refs/heads/<rama>"
//! ├── index.json          ← staging: path → hash
//! ├── objects/
//! │   └── <hash>          ← blobs de contenido (bytes tal cual)
//! ├── commits/
//! │   └── <id>.json       ← metadata + árbol {path → hash}
//! └── refs/heads/<rama>   ← último commit id de la rama
//! ```
//!
//! No pretende ser compatible con git; es un VCS local autocontenido
//! para los datos del propio omniplanner.

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const DIR_REPO: &str = ".omnivcs";
pub const ARCHIVO_IGNORE: &str = ".omniignore";

/// Errores del source control.
#[derive(Debug)]
pub enum ErrorVcs {
    Io(io::Error),
    Json(serde_json::Error),
    NoEsRepo,
    YaEsRepo,
    RutaFueraDelRepo,
    RamaInexistente(String),
    RamaYaExiste(String),
    CommitInexistente(String),
    IndexCorrupto(String),
    HeadCorrupto,
}

impl std::fmt::Display for ErrorVcs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "E/S: {e}"),
            Self::Json(e) => write!(f, "JSON: {e}"),
            Self::NoEsRepo => write!(f, "el directorio no es un repositorio omnivcs"),
            Self::YaEsRepo => write!(f, "el directorio ya es un repositorio omnivcs"),
            Self::RutaFueraDelRepo => write!(f, "la ruta está fuera del repositorio"),
            Self::RamaInexistente(r) => write!(f, "rama inexistente: {r}"),
            Self::RamaYaExiste(r) => write!(f, "la rama ya existe: {r}"),
            Self::CommitInexistente(c) => write!(f, "commit inexistente: {c}"),
            Self::IndexCorrupto(m) => write!(f, "index corrupto: {m}"),
            Self::HeadCorrupto => write!(f, "HEAD corrupto"),
        }
    }
}

impl std::error::Error for ErrorVcs {}

impl From<io::Error> for ErrorVcs {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for ErrorVcs {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

pub type Resultado<T> = Result<T, ErrorVcs>;

/// Commit persistido en disco.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitRepo {
    pub id: String,
    pub mensaje: String,
    pub autor: String,
    pub timestamp: NaiveDateTime,
    pub padre_id: Option<String>,
    /// path relativo (con `/` como separador) → hash del blob.
    pub arbol: BTreeMap<String, String>,
}

/// Estado del workdir vs HEAD + index.
#[derive(Debug, Default, Clone, Serialize)]
pub struct EstadoRepo {
    pub rama_actual: String,
    pub head: Option<String>,
    /// Rutas cuyo contenido en workdir difiere del index (modificados no staged).
    pub modificados: Vec<String>,
    /// Rutas staged para el próximo commit (difieren de HEAD).
    pub staged: Vec<String>,
    /// Rutas en workdir que no están en el index ni en HEAD.
    pub sin_seguimiento: Vec<String>,
    /// Rutas presentes en HEAD o index pero borradas del workdir.
    pub borrados: Vec<String>,
}

/// Repositorio. No cachea nada: cada operación lee de disco.
pub struct Repo {
    raiz: PathBuf,
}

impl Repo {
    /// Inicializa un repo nuevo en `raiz`. Crea la estructura y rama `main`.
    pub fn init(raiz: impl AsRef<Path>) -> Resultado<Self> {
        let raiz = raiz.as_ref().to_path_buf();
        let dir = raiz.join(DIR_REPO);
        if dir.exists() {
            return Err(ErrorVcs::YaEsRepo);
        }
        fs::create_dir_all(dir.join("objects"))?;
        fs::create_dir_all(dir.join("commits"))?;
        fs::create_dir_all(dir.join("refs").join("heads"))?;
        fs::write(dir.join("HEAD"), "ref: refs/heads/main")?;
        fs::write(dir.join("index.json"), "{}")?;
        // refs/heads/main queda sin crear hasta el primer commit.
        Ok(Self { raiz })
    }

    /// Abre un repo existente. Busca `.omnivcs` en `raiz` (sin ascender).
    pub fn open(raiz: impl AsRef<Path>) -> Resultado<Self> {
        let raiz = raiz.as_ref().to_path_buf();
        if !raiz.join(DIR_REPO).is_dir() {
            return Err(ErrorVcs::NoEsRepo);
        }
        Ok(Self { raiz })
    }

    /// Busca un repo subiendo por los padres de `desde`.
    pub fn discover(desde: impl AsRef<Path>) -> Resultado<Self> {
        let mut p = desde.as_ref().to_path_buf();
        loop {
            if p.join(DIR_REPO).is_dir() {
                return Ok(Self { raiz: p });
            }
            if !p.pop() {
                return Err(ErrorVcs::NoEsRepo);
            }
        }
    }

    pub fn raiz(&self) -> &Path {
        &self.raiz
    }

    fn dir_repo(&self) -> PathBuf {
        self.raiz.join(DIR_REPO)
    }

    // ── HEAD / ramas ──────────────────────────────────────────

    /// Nombre de la rama actual (o `None` si HEAD está en estado detached).
    pub fn rama_actual(&self) -> Resultado<String> {
        let head = fs::read_to_string(self.dir_repo().join("HEAD"))?;
        let head = head.trim();
        let prefijo = "ref: refs/heads/";
        if let Some(rama) = head.strip_prefix(prefijo) {
            Ok(rama.to_string())
        } else {
            Err(ErrorVcs::HeadCorrupto)
        }
    }

    /// Id del último commit de la rama actual, si existe.
    pub fn head_commit_id(&self) -> Resultado<Option<String>> {
        let rama = self.rama_actual()?;
        let ref_path = self.dir_repo().join("refs").join("heads").join(&rama);
        if !ref_path.exists() {
            return Ok(None);
        }
        let id = fs::read_to_string(ref_path)?.trim().to_string();
        if id.is_empty() { Ok(None) } else { Ok(Some(id)) }
    }

    fn escribir_head_rama(&self, rama: &str) -> Resultado<()> {
        fs::write(
            self.dir_repo().join("HEAD"),
            format!("ref: refs/heads/{rama}"),
        )?;
        Ok(())
    }

    fn escribir_ref_rama(&self, rama: &str, commit_id: &str) -> Resultado<()> {
        fs::write(
            self.dir_repo().join("refs").join("heads").join(rama),
            commit_id,
        )?;
        Ok(())
    }

    /// Lista todas las ramas existentes.
    pub fn listar_ramas(&self) -> Resultado<Vec<String>> {
        let dir = self.dir_repo().join("refs").join("heads");
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut ramas = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                if let Some(n) = entry.file_name().to_str() {
                    ramas.push(n.to_string());
                }
            }
        }
        ramas.sort();
        Ok(ramas)
    }

    /// Crea rama nueva apuntando al commit actual y hace checkout.
    pub fn crear_rama(&self, nombre: &str) -> Resultado<()> {
        let ref_path = self.dir_repo().join("refs").join("heads").join(nombre);
        if ref_path.exists() {
            return Err(ErrorVcs::RamaYaExiste(nombre.to_string()));
        }
        if let Some(id) = self.head_commit_id()? {
            fs::write(&ref_path, id)?;
        } else {
            // Repo vacío: se crea la rama al primer commit.
            // Guardamos ref vacío para marcar su existencia lógica.
            fs::write(&ref_path, "")?;
        }
        self.escribir_head_rama(nombre)?;
        Ok(())
    }

    /// Cambia de rama. Reemplaza el workdir por el árbol de esa rama y
    /// limpia el index. No valida cambios sin guardar (llamar a
    /// [`Repo::estado`] antes si se desea).
    pub fn cambiar_rama(&self, nombre: &str) -> Resultado<()> {
        let ref_path = self.dir_repo().join("refs").join("heads").join(nombre);
        if !ref_path.exists() {
            return Err(ErrorVcs::RamaInexistente(nombre.to_string()));
        }
        self.escribir_head_rama(nombre)?;
        // Reescribir workdir desde el árbol del head (si existe).
        let id = self.head_commit_id()?;
        if let Some(id) = id {
            let commit = self.leer_commit(&id)?;
            self.materializar_arbol(&commit.arbol)?;
            self.escribir_index(&commit.arbol)?;
        } else {
            // rama vacía: index vacío, workdir sin tocar.
            self.escribir_index(&BTreeMap::new())?;
        }
        Ok(())
    }

    // ── Index / staging ───────────────────────────────────────

    fn leer_index(&self) -> Resultado<BTreeMap<String, String>> {
        let path = self.dir_repo().join("index.json");
        let raw = fs::read_to_string(path)?;
        let idx: BTreeMap<String, String> = serde_json::from_str(&raw)
            .map_err(|e| ErrorVcs::IndexCorrupto(e.to_string()))?;
        Ok(idx)
    }

    fn escribir_index(&self, idx: &BTreeMap<String, String>) -> Resultado<()> {
        let path = self.dir_repo().join("index.json");
        fs::write(path, serde_json::to_string_pretty(idx)?)?;
        Ok(())
    }

    /// Añade un archivo o directorio al staging. Si `path` es un directorio,
    /// añade recursivamente respetando `.omniignore`.
    pub fn add(&self, path: impl AsRef<Path>) -> Resultado<usize> {
        let absoluto = self.resolver_absoluto(path.as_ref())?;
        let mut idx = self.leer_index()?;
        let ignore = self.cargar_ignore()?;
        let mut contador = 0usize;

        if absoluto.is_dir() {
            for rel in self.listar_archivos_rel(&absoluto, &ignore)? {
                let abs = self.raiz.join(&rel);
                let hash = self.almacenar_blob(&abs)?;
                idx.insert(rel, hash);
                contador += 1;
            }
        } else if absoluto.is_file() {
            let rel = self.ruta_relativa(&absoluto)?;
            if self.esta_ignorada(&rel, &ignore) {
                return Ok(0);
            }
            let hash = self.almacenar_blob(&absoluto)?;
            idx.insert(rel, hash);
            contador = 1;
        } else {
            // Archivo borrado: quitarlo del index.
            let rel = self.ruta_relativa(&absoluto)?;
            if idx.remove(&rel).is_some() {
                contador = 1;
            }
        }

        self.escribir_index(&idx)?;
        Ok(contador)
    }

    /// Quita del index (unstage). No toca el workdir.
    pub fn unstage(&self, path: impl AsRef<Path>) -> Resultado<bool> {
        let absoluto = self.resolver_absoluto(path.as_ref())?;
        let rel = self.ruta_relativa(&absoluto)?;
        let mut idx = self.leer_index()?;
        let tenia = idx.remove(&rel).is_some();
        self.escribir_index(&idx)?;
        Ok(tenia)
    }

    // ── Commit / log ──────────────────────────────────────────

    /// Crea un commit con el index actual. Devuelve el id.
    pub fn commit(&self, mensaje: &str, autor: &str) -> Resultado<String> {
        let arbol = self.leer_index()?;
        let padre_id = self.head_commit_id()?;
        let id = Uuid::new_v4().to_string()[..12].to_string();
        let commit = CommitRepo {
            id: id.clone(),
            mensaje: mensaje.to_string(),
            autor: autor.to_string(),
            timestamp: chrono::Local::now().naive_local(),
            padre_id,
            arbol,
        };
        let path = self.dir_repo().join("commits").join(format!("{id}.json"));
        fs::write(path, serde_json::to_string_pretty(&commit)?)?;
        let rama = self.rama_actual()?;
        self.escribir_ref_rama(&rama, &id)?;
        Ok(id)
    }

    /// Lee un commit por id.
    pub fn leer_commit(&self, id: &str) -> Resultado<CommitRepo> {
        let path = self.dir_repo().join("commits").join(format!("{id}.json"));
        if !path.exists() {
            return Err(ErrorVcs::CommitInexistente(id.to_string()));
        }
        let raw = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&raw)?)
    }

    /// Historial lineal desde HEAD hacia atrás por `padre_id`.
    pub fn log(&self) -> Resultado<Vec<CommitRepo>> {
        let mut out = Vec::new();
        let mut actual = self.head_commit_id()?;
        while let Some(id) = actual {
            let c = self.leer_commit(&id)?;
            actual = c.padre_id.clone();
            out.push(c);
        }
        Ok(out)
    }

    // ── Estado / diff ─────────────────────────────────────────

    pub fn estado(&self) -> Resultado<EstadoRepo> {
        let rama_actual = self.rama_actual()?;
        let head = self.head_commit_id()?;
        let arbol_head = match &head {
            Some(id) => self.leer_commit(id)?.arbol,
            None => BTreeMap::new(),
        };
        let index = self.leer_index()?;
        let ignore = self.cargar_ignore()?;

        let mut modificados = Vec::new();
        let mut staged = Vec::new();
        let mut sin_seguimiento = Vec::new();
        let mut borrados = Vec::new();

        // staged: index difiere de head
        let todas: std::collections::HashSet<&String> =
            index.keys().chain(arbol_head.keys()).collect();
        for path in &todas {
            match (index.get(*path), arbol_head.get(*path)) {
                (Some(hi), Some(hh)) if hi != hh => staged.push((*path).clone()),
                (Some(_), None) => staged.push((*path).clone()),
                (None, Some(_)) => borrados.push((*path).clone()),
                _ => {}
            }
        }

        // modificados / sin seguimiento respecto al workdir
        let archivos_wd = self.listar_archivos_rel(&self.raiz, &ignore)?;
        let set_wd: std::collections::HashSet<&String> = archivos_wd.iter().collect();
        for rel in &archivos_wd {
            let abs = self.raiz.join(rel);
            let hash_wd = hash_archivo(&abs)?;
            match index.get(rel) {
                Some(h) if *h == hash_wd => {}
                Some(_) => modificados.push(rel.clone()),
                None => {
                    if !arbol_head.contains_key(rel) {
                        sin_seguimiento.push(rel.clone());
                    } else {
                        // En HEAD pero no en index: tratado como borrado del index.
                        // El usuario tendrá que `add` para stagear la restauración
                        // o `checkout` para traerlo. Lo marcamos como modificado.
                        modificados.push(rel.clone());
                    }
                }
            }
        }

        // Archivos en index que ya no están en workdir → borrados
        for rel in index.keys() {
            if !set_wd.contains(rel) && !borrados.contains(rel) {
                borrados.push(rel.clone());
            }
        }

        staged.sort();
        modificados.sort();
        sin_seguimiento.sort();
        borrados.sort();
        staged.dedup();
        modificados.dedup();
        borrados.dedup();

        Ok(EstadoRepo {
            rama_actual,
            head,
            modificados,
            staged,
            sin_seguimiento,
            borrados,
        })
    }

    /// Diff textual entre dos commits (línea a línea, formato unificado simple).
    pub fn diff(&self, id_a: &str, id_b: &str) -> Resultado<String> {
        let a = self.leer_commit(id_a)?;
        let b = self.leer_commit(id_b)?;
        let mut out = String::new();
        let todas: std::collections::BTreeSet<&String> =
            a.arbol.keys().chain(b.arbol.keys()).collect();
        for path in todas {
            match (a.arbol.get(path), b.arbol.get(path)) {
                (Some(ha), Some(hb)) if ha == hb => {}
                (Some(ha), Some(hb)) => {
                    out.push_str(&format!("~ {path}\n"));
                    out.push_str(&diff_lineas(
                        &self.leer_blob_texto(ha)?,
                        &self.leer_blob_texto(hb)?,
                    ));
                }
                (Some(_), None) => out.push_str(&format!("- {path}\n")),
                (None, Some(_)) => out.push_str(&format!("+ {path}\n")),
                (None, None) => {}
            }
        }
        Ok(out)
    }

    // ── Restauración / checkout de archivos ──────────────────

    /// Sobreescribe `path` con su contenido en `commit_id`.
    pub fn restaurar(&self, path: impl AsRef<Path>, commit_id: &str) -> Resultado<()> {
        let rel = self.ruta_relativa(&self.resolver_absoluto(path.as_ref())?)?;
        let commit = self.leer_commit(commit_id)?;
        let hash = commit
            .arbol
            .get(&rel)
            .ok_or_else(|| ErrorVcs::IndexCorrupto(format!("{rel} no existe en {commit_id}")))?;
        let bytes = fs::read(self.dir_repo().join("objects").join(hash))?;
        let destino = self.raiz.join(&rel);
        if let Some(p) = destino.parent() {
            fs::create_dir_all(p)?;
        }
        fs::write(destino, bytes)?;
        Ok(())
    }

    fn materializar_arbol(&self, arbol: &BTreeMap<String, String>) -> Resultado<()> {
        for (rel, hash) in arbol {
            let bytes = fs::read(self.dir_repo().join("objects").join(hash))?;
            let destino = self.raiz.join(rel);
            if let Some(p) = destino.parent() {
                fs::create_dir_all(p)?;
            }
            fs::write(destino, bytes)?;
        }
        Ok(())
    }

    // ── Objects ───────────────────────────────────────────────

    fn almacenar_blob(&self, path: &Path) -> Resultado<String> {
        let bytes = fs::read(path)?;
        let hash = hash_bytes(&bytes);
        let dir_obj = self.dir_repo().join("objects");
        let destino = dir_obj.join(&hash);
        if !destino.exists() {
            fs::write(destino, bytes)?;
        }
        Ok(hash)
    }

    fn leer_blob_texto(&self, hash: &str) -> Resultado<String> {
        let bytes = fs::read(self.dir_repo().join("objects").join(hash))?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    // ── Utilidades de paths e ignore ──────────────────────────

    fn resolver_absoluto(&self, p: &Path) -> Resultado<PathBuf> {
        let abs = if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.raiz.join(p)
        };
        Ok(abs)
    }

    fn ruta_relativa(&self, abs: &Path) -> Resultado<String> {
        let rel = abs
            .strip_prefix(&self.raiz)
            .map_err(|_| ErrorVcs::RutaFueraDelRepo)?;
        let s = rel
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches('/')
            .to_string();
        Ok(s)
    }

    fn cargar_ignore(&self) -> Resultado<Vec<String>> {
        let mut patrones = vec![DIR_REPO.to_string()];
        let f = self.raiz.join(ARCHIVO_IGNORE);
        if f.is_file() {
            for l in fs::read_to_string(f)?.lines() {
                let l = l.trim();
                if !l.is_empty() && !l.starts_with('#') {
                    patrones.push(l.to_string());
                }
            }
        }
        Ok(patrones)
    }

    fn esta_ignorada(&self, rel: &str, patrones: &[String]) -> bool {
        for p in patrones {
            // Soporte mínimo de globs:
            //   - "nombre"           → coincide con "nombre" exacto o
            //                          cualquier ruta dentro de "nombre/"
            //   - "*.ext"            → coincide con rutas que terminen en ".ext"
            //   - "prefijo*"         → coincide con rutas que empiecen en "prefijo"
            if let Some(ext) = p.strip_prefix('*') {
                if rel.ends_with(ext) {
                    return true;
                }
            } else if let Some(pre) = p.strip_suffix('*') {
                if rel.starts_with(pre) {
                    return true;
                }
            } else if rel == p || rel.starts_with(&format!("{p}/")) {
                return true;
            }
        }
        false
    }

    fn listar_archivos_rel(
        &self,
        desde: &Path,
        ignore: &[String],
    ) -> Resultado<Vec<String>> {
        let mut out = Vec::new();
        self.recorrer(desde, ignore, &mut out)?;
        out.sort();
        Ok(out)
    }

    fn recorrer(&self, dir: &Path, ignore: &[String], out: &mut Vec<String>) -> Resultado<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let rel = self.ruta_relativa(&path)?;
            if self.esta_ignorada(&rel, ignore) {
                continue;
            }
            let ft = entry.file_type()?;
            if ft.is_dir() {
                self.recorrer(&path, ignore, out)?;
            } else if ft.is_file() {
                out.push(rel);
            }
        }
        Ok(())
    }
}

// ── Helpers libres ───────────────────────────────────────────

fn hash_bytes(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

fn hash_archivo(path: &Path) -> Resultado<String> {
    let bytes = fs::read(path)?;
    Ok(hash_bytes(&bytes))
}

/// Diff línea a línea muy simple (sin LCS) entre dos textos.
fn diff_lineas(a: &str, b: &str) -> String {
    let la: Vec<&str> = a.lines().collect();
    let lb: Vec<&str> = b.lines().collect();
    let max = la.len().max(lb.len());
    let mut out = String::new();
    for i in 0..max {
        let x = la.get(i).copied().unwrap_or("");
        let y = lb.get(i).copied().unwrap_or("");
        if x != y {
            if !x.is_empty() {
                out.push_str(&format!("  - {x}\n"));
            }
            if !y.is_empty() {
                out.push_str(&format!("  + {y}\n"));
            }
        }
    }
    out
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static CONTADOR: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir(nombre: &str) -> PathBuf {
        let n = CONTADOR.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        let p = std::env::temp_dir().join(format!("omnivcs_{nombre}_{pid}_{n}"));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn escribir(p: &Path, contenido: &str) {
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, contenido).unwrap();
    }

    #[test]
    fn init_y_abrir() {
        let d = temp_dir("init");
        let repo = Repo::init(&d).unwrap();
        assert_eq!(repo.raiz(), d);
        assert!(matches!(Repo::init(&d), Err(ErrorVcs::YaEsRepo)));

        let repo2 = Repo::open(&d).unwrap();
        assert_eq!(repo2.rama_actual().unwrap(), "main");
        assert!(repo2.head_commit_id().unwrap().is_none());
    }

    #[test]
    fn add_commit_log() {
        let d = temp_dir("addcommit");
        let repo = Repo::init(&d).unwrap();
        escribir(&d.join("a.txt"), "hola");
        escribir(&d.join("sub/b.txt"), "mundo");
        repo.add(".").unwrap();

        let id = repo.commit("inicial", "yo").unwrap();
        let log = repo.log().unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].id, id);
        assert_eq!(log[0].arbol.len(), 2);
        assert!(log[0].arbol.contains_key("a.txt"));
        assert!(log[0].arbol.contains_key("sub/b.txt"));

        escribir(&d.join("a.txt"), "hola v2");
        repo.add("a.txt").unwrap();
        let id2 = repo.commit("v2", "yo").unwrap();
        assert_eq!(repo.log().unwrap().len(), 2);
        assert_eq!(repo.log().unwrap()[0].id, id2);
        assert_eq!(repo.log().unwrap()[0].padre_id.as_deref(), Some(id.as_str()));
    }

    #[test]
    fn estado_detecta_cambios() {
        let d = temp_dir("estado");
        let repo = Repo::init(&d).unwrap();
        escribir(&d.join("x.txt"), "v1");
        repo.add(".").unwrap();
        repo.commit("c1", "a").unwrap();

        escribir(&d.join("x.txt"), "v2");
        escribir(&d.join("nuevo.txt"), "nuevo");

        let e = repo.estado().unwrap();
        assert!(e.modificados.contains(&"x.txt".to_string()));
        assert!(e.sin_seguimiento.contains(&"nuevo.txt".to_string()));
        assert!(e.staged.is_empty());

        repo.add("x.txt").unwrap();
        let e = repo.estado().unwrap();
        assert!(e.staged.contains(&"x.txt".to_string()));
        assert!(!e.modificados.contains(&"x.txt".to_string()));
    }

    #[test]
    fn ramas_y_checkout() {
        let d = temp_dir("ramas");
        let repo = Repo::init(&d).unwrap();
        escribir(&d.join("f.txt"), "main-v1");
        repo.add(".").unwrap();
        repo.commit("c1", "a").unwrap();

        repo.crear_rama("feature").unwrap();
        assert_eq!(repo.rama_actual().unwrap(), "feature");
        escribir(&d.join("f.txt"), "feature-v2");
        repo.add(".").unwrap();
        repo.commit("feat c2", "a").unwrap();

        assert!(repo.listar_ramas().unwrap().contains(&"feature".to_string()));
        assert!(repo.listar_ramas().unwrap().contains(&"main".to_string()));

        repo.cambiar_rama("main").unwrap();
        assert_eq!(fs::read_to_string(d.join("f.txt")).unwrap(), "main-v1");

        repo.cambiar_rama("feature").unwrap();
        assert_eq!(fs::read_to_string(d.join("f.txt")).unwrap(), "feature-v2");

        assert!(matches!(
            repo.cambiar_rama("noexiste"),
            Err(ErrorVcs::RamaInexistente(_))
        ));
    }

    #[test]
    fn diff_entre_commits() {
        let d = temp_dir("diff");
        let repo = Repo::init(&d).unwrap();
        escribir(&d.join("t.txt"), "linea1\nlinea2\n");
        repo.add(".").unwrap();
        let a = repo.commit("c1", "u").unwrap();

        escribir(&d.join("t.txt"), "linea1\nmodificada\n");
        escribir(&d.join("extra.txt"), "nuevo");
        repo.add(".").unwrap();
        let b = repo.commit("c2", "u").unwrap();

        let d1 = repo.diff(&a, &b).unwrap();
        assert!(d1.contains("~ t.txt"));
        assert!(d1.contains("linea2"));
        assert!(d1.contains("modificada"));
        assert!(d1.contains("+ extra.txt"));
    }

    #[test]
    fn restaurar_archivo() {
        let d = temp_dir("restaurar");
        let repo = Repo::init(&d).unwrap();
        escribir(&d.join("f.txt"), "original");
        repo.add(".").unwrap();
        let c1 = repo.commit("c1", "u").unwrap();

        escribir(&d.join("f.txt"), "cambiado");
        repo.restaurar("f.txt", &c1).unwrap();
        assert_eq!(fs::read_to_string(d.join("f.txt")).unwrap(), "original");
    }

    #[test]
    fn omniignore_excluye_rutas() {
        let d = temp_dir("ignore");
        let repo = Repo::init(&d).unwrap();
        fs::write(d.join(ARCHIVO_IGNORE), "target\n*.log\n").unwrap();
        escribir(&d.join("src.rs"), "ok");
        escribir(&d.join("target/big.bin"), "no");
        escribir(&d.join("app.log"), "ignorado");
        repo.add(".").unwrap();
        let idx = repo.leer_index().unwrap();
        assert!(idx.contains_key("src.rs"));
        assert!(!idx.keys().any(|k| k.starts_with("target/")));
        assert!(!idx.contains_key("app.log"));
    }
}
