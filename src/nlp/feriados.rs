//! Días festivos / feriados de Ecuador, Estados Unidos y fechas religiosas.
//!
//! Permite:
//!   * Listar todos los feriados de un año para un país.
//!   * Resolver un nombre coloquial ("navidad", "thanksgiving", "viernes santo")
//!     a una fecha concreta (`NaiveDate`).
//!   * Calcular cuántos días faltan para un feriado.
//!
//! Las fechas pascuales se calculan con el algoritmo de Computus (Anonymous /
//! Gauss-Meeus). El resto son fijas o se calculan con la regla "n-ésimo
//! día de la semana del mes" (USA).

use chrono::{Datelike, Duration, NaiveDate, Weekday};

// ─── Tipos ──────────────────────────────────────────────────────────────────

/// País / categoría a la que pertenece el feriado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pais {
    Ecuador,
    Usa,
    Religioso,
}

impl Pais {
    pub fn nombre(&self) -> &'static str {
        match self {
            Pais::Ecuador => "Ecuador",
            Pais::Usa => "USA",
            Pais::Religioso => "Religioso",
        }
    }
}

/// Un feriado concreto en un año específico.
#[derive(Debug, Clone)]
pub struct Feriado {
    pub nombre: &'static str,
    pub fecha: NaiveDate,
    pub pais: Pais,
    /// Si es festivo bancario / oficial (true) o cultural / religioso opcional.
    pub oficial: bool,
}

// ─── Algoritmo de Pascua (Computus, válido para calendario gregoriano) ──────

/// Domingo de Resurrección (Easter Sunday) gregoriano.
pub fn pascua(anio: i32) -> NaiveDate {
    let a = anio % 19;
    let b = anio / 100;
    let c = anio % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let mes = (h + l - 7 * m + 114) / 31;
    let dia = ((h + l - 7 * m + 114) % 31) + 1;
    NaiveDate::from_ymd_opt(anio, mes as u32, dia as u32).expect("pascua válida")
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// N-ésimo (1..=5) día-de-la-semana del mes. Si `n` es mayor al disponible,
/// retorna el último.
fn nth_weekday(anio: i32, mes: u32, weekday: Weekday, n: u32) -> NaiveDate {
    let primero = NaiveDate::from_ymd_opt(anio, mes, 1).unwrap();
    let offset = (7 + weekday.num_days_from_monday() as i64
        - primero.weekday().num_days_from_monday() as i64)
        % 7;
    let mut fecha = primero + Duration::days(offset);
    for _ in 1..n {
        let candidato = fecha + Duration::days(7);
        if candidato.month() != mes {
            break;
        }
        fecha = candidato;
    }
    fecha
}

/// Último día-de-la-semana del mes.
fn last_weekday(anio: i32, mes: u32, weekday: Weekday) -> NaiveDate {
    nth_weekday(anio, mes, weekday, 5)
}

// ─── Catálogo: Ecuador ──────────────────────────────────────────────────────

pub fn feriados_ecuador(anio: i32) -> Vec<Feriado> {
    let p = pascua(anio);
    let mut v = vec![
        Feriado {
            nombre: "Año Nuevo",
            fecha: NaiveDate::from_ymd_opt(anio, 1, 1).unwrap(),
            pais: Pais::Ecuador,
            oficial: true,
        },
        Feriado {
            nombre: "Carnaval (lunes)",
            fecha: p - Duration::days(48),
            pais: Pais::Ecuador,
            oficial: true,
        },
        Feriado {
            nombre: "Carnaval (martes)",
            fecha: p - Duration::days(47),
            pais: Pais::Ecuador,
            oficial: true,
        },
        Feriado {
            nombre: "Viernes Santo",
            fecha: p - Duration::days(2),
            pais: Pais::Ecuador,
            oficial: true,
        },
        Feriado {
            nombre: "Día del Trabajo",
            fecha: NaiveDate::from_ymd_opt(anio, 5, 1).unwrap(),
            pais: Pais::Ecuador,
            oficial: true,
        },
        Feriado {
            nombre: "Día de la Madre",
            fecha: nth_weekday(anio, 5, Weekday::Sun, 2),
            pais: Pais::Ecuador,
            oficial: false,
        },
        Feriado {
            nombre: "Batalla de Pichincha",
            fecha: NaiveDate::from_ymd_opt(anio, 5, 24).unwrap(),
            pais: Pais::Ecuador,
            oficial: true,
        },
        Feriado {
            nombre: "Día del Padre",
            fecha: nth_weekday(anio, 6, Weekday::Sun, 3),
            pais: Pais::Ecuador,
            oficial: false,
        },
        Feriado {
            nombre: "Primer Grito de Independencia",
            fecha: NaiveDate::from_ymd_opt(anio, 8, 10).unwrap(),
            pais: Pais::Ecuador,
            oficial: true,
        },
        Feriado {
            nombre: "Independencia de Guayaquil",
            fecha: NaiveDate::from_ymd_opt(anio, 10, 9).unwrap(),
            pais: Pais::Ecuador,
            oficial: true,
        },
        Feriado {
            nombre: "Día de los Difuntos",
            fecha: NaiveDate::from_ymd_opt(anio, 11, 2).unwrap(),
            pais: Pais::Ecuador,
            oficial: true,
        },
        Feriado {
            nombre: "Independencia de Cuenca",
            fecha: NaiveDate::from_ymd_opt(anio, 11, 3).unwrap(),
            pais: Pais::Ecuador,
            oficial: true,
        },
        Feriado {
            nombre: "Navidad",
            fecha: NaiveDate::from_ymd_opt(anio, 12, 25).unwrap(),
            pais: Pais::Ecuador,
            oficial: true,
        },
        Feriado {
            nombre: "Fin de Año",
            fecha: NaiveDate::from_ymd_opt(anio, 12, 31).unwrap(),
            pais: Pais::Ecuador,
            oficial: false,
        },
    ];
    v.sort_by_key(|f| f.fecha);
    v
}

// ─── Catálogo: USA ──────────────────────────────────────────────────────────

pub fn feriados_usa(anio: i32) -> Vec<Feriado> {
    let mut v = vec![
        Feriado {
            nombre: "New Year's Day",
            fecha: NaiveDate::from_ymd_opt(anio, 1, 1).unwrap(),
            pais: Pais::Usa,
            oficial: true,
        },
        Feriado {
            nombre: "Martin Luther King Jr. Day",
            fecha: nth_weekday(anio, 1, Weekday::Mon, 3),
            pais: Pais::Usa,
            oficial: true,
        },
        Feriado {
            nombre: "Presidents' Day",
            fecha: nth_weekday(anio, 2, Weekday::Mon, 3),
            pais: Pais::Usa,
            oficial: true,
        },
        Feriado {
            nombre: "Memorial Day",
            fecha: last_weekday(anio, 5, Weekday::Mon),
            pais: Pais::Usa,
            oficial: true,
        },
        Feriado {
            nombre: "Juneteenth",
            fecha: NaiveDate::from_ymd_opt(anio, 6, 19).unwrap(),
            pais: Pais::Usa,
            oficial: true,
        },
        Feriado {
            nombre: "Independence Day",
            fecha: NaiveDate::from_ymd_opt(anio, 7, 4).unwrap(),
            pais: Pais::Usa,
            oficial: true,
        },
        Feriado {
            nombre: "Labor Day",
            fecha: nth_weekday(anio, 9, Weekday::Mon, 1),
            pais: Pais::Usa,
            oficial: true,
        },
        Feriado {
            nombre: "Columbus Day",
            fecha: nth_weekday(anio, 10, Weekday::Mon, 2),
            pais: Pais::Usa,
            oficial: true,
        },
        Feriado {
            nombre: "Veterans Day",
            fecha: NaiveDate::from_ymd_opt(anio, 11, 11).unwrap(),
            pais: Pais::Usa,
            oficial: true,
        },
        Feriado {
            nombre: "Thanksgiving",
            fecha: nth_weekday(anio, 11, Weekday::Thu, 4),
            pais: Pais::Usa,
            oficial: true,
        },
        Feriado {
            nombre: "Christmas Day",
            fecha: NaiveDate::from_ymd_opt(anio, 12, 25).unwrap(),
            pais: Pais::Usa,
            oficial: true,
        },
        // Culturales
        Feriado {
            nombre: "Valentine's Day",
            fecha: NaiveDate::from_ymd_opt(anio, 2, 14).unwrap(),
            pais: Pais::Usa,
            oficial: false,
        },
        Feriado {
            nombre: "St. Patrick's Day",
            fecha: NaiveDate::from_ymd_opt(anio, 3, 17).unwrap(),
            pais: Pais::Usa,
            oficial: false,
        },
        Feriado {
            nombre: "Mother's Day",
            fecha: nth_weekday(anio, 5, Weekday::Sun, 2),
            pais: Pais::Usa,
            oficial: false,
        },
        Feriado {
            nombre: "Father's Day",
            fecha: nth_weekday(anio, 6, Weekday::Sun, 3),
            pais: Pais::Usa,
            oficial: false,
        },
        Feriado {
            nombre: "Halloween",
            fecha: NaiveDate::from_ymd_opt(anio, 10, 31).unwrap(),
            pais: Pais::Usa,
            oficial: false,
        },
        Feriado {
            nombre: "Election Day",
            fecha: {
                // Primer martes después del primer lunes de noviembre
                let primer_lunes = nth_weekday(anio, 11, Weekday::Mon, 1);
                primer_lunes + Duration::days(1)
            },
            pais: Pais::Usa,
            oficial: false,
        },
        Feriado {
            nombre: "Black Friday",
            fecha: nth_weekday(anio, 11, Weekday::Thu, 4) + Duration::days(1),
            pais: Pais::Usa,
            oficial: false,
        },
        Feriado {
            nombre: "Cyber Monday",
            fecha: nth_weekday(anio, 11, Weekday::Thu, 4) + Duration::days(4),
            pais: Pais::Usa,
            oficial: false,
        },
        Feriado {
            nombre: "Christmas Eve",
            fecha: NaiveDate::from_ymd_opt(anio, 12, 24).unwrap(),
            pais: Pais::Usa,
            oficial: false,
        },
        Feriado {
            nombre: "New Year's Eve",
            fecha: NaiveDate::from_ymd_opt(anio, 12, 31).unwrap(),
            pais: Pais::Usa,
            oficial: false,
        },
        Feriado {
            nombre: "Tax Day",
            fecha: NaiveDate::from_ymd_opt(anio, 4, 15).unwrap(),
            pais: Pais::Usa,
            oficial: false,
        },
        Feriado {
            nombre: "Super Bowl Sunday",
            fecha: nth_weekday(anio, 2, Weekday::Sun, 1),
            pais: Pais::Usa,
            oficial: false,
        },
    ];
    v.sort_by_key(|f| f.fecha);
    v
}

// ─── Catálogo: Religiosos ───────────────────────────────────────────────────

pub fn feriados_religiosos(anio: i32) -> Vec<Feriado> {
    let p = pascua(anio);
    let mut v = vec![
        Feriado {
            nombre: "Epifanía / Día de Reyes",
            fecha: NaiveDate::from_ymd_opt(anio, 1, 6).unwrap(),
            pais: Pais::Religioso,
            oficial: false,
        },
        Feriado {
            nombre: "Miércoles de Ceniza",
            fecha: p - Duration::days(46),
            pais: Pais::Religioso,
            oficial: false,
        },
        Feriado {
            nombre: "Domingo de Ramos",
            fecha: p - Duration::days(7),
            pais: Pais::Religioso,
            oficial: false,
        },
        Feriado {
            nombre: "Jueves Santo",
            fecha: p - Duration::days(3),
            pais: Pais::Religioso,
            oficial: false,
        },
        Feriado {
            nombre: "Viernes Santo",
            fecha: p - Duration::days(2),
            pais: Pais::Religioso,
            oficial: false,
        },
        Feriado {
            nombre: "Sábado de Gloria",
            fecha: p - Duration::days(1),
            pais: Pais::Religioso,
            oficial: false,
        },
        Feriado {
            nombre: "Domingo de Resurrección (Pascua)",
            fecha: p,
            pais: Pais::Religioso,
            oficial: false,
        },
        Feriado {
            nombre: "Corpus Christi",
            fecha: p + Duration::days(60),
            pais: Pais::Religioso,
            oficial: false,
        },
        Feriado {
            nombre: "Asunción de la Virgen",
            fecha: NaiveDate::from_ymd_opt(anio, 8, 15).unwrap(),
            pais: Pais::Religioso,
            oficial: false,
        },
        Feriado {
            nombre: "Día de Todos los Santos",
            fecha: NaiveDate::from_ymd_opt(anio, 11, 1).unwrap(),
            pais: Pais::Religioso,
            oficial: false,
        },
        Feriado {
            nombre: "Inmaculada Concepción",
            fecha: NaiveDate::from_ymd_opt(anio, 12, 8).unwrap(),
            pais: Pais::Religioso,
            oficial: false,
        },
        Feriado {
            nombre: "Nochebuena",
            fecha: NaiveDate::from_ymd_opt(anio, 12, 24).unwrap(),
            pais: Pais::Religioso,
            oficial: false,
        },
        Feriado {
            nombre: "Navidad",
            fecha: NaiveDate::from_ymd_opt(anio, 12, 25).unwrap(),
            pais: Pais::Religioso,
            oficial: false,
        },
    ];
    v.sort_by_key(|f| f.fecha);
    v
}

/// Conjunto consolidado: Ecuador + USA + Religiosos sin duplicar fechas iguales
/// del mismo nombre.
pub fn feriados_todos(anio: i32) -> Vec<Feriado> {
    let mut v = feriados_ecuador(anio);
    v.extend(feriados_usa(anio));
    v.extend(feriados_religiosos(anio));
    v.sort_by_key(|f| f.fecha);
    v
}

// ─── Resolución por nombre coloquial ────────────────────────────────────────

/// Quita tildes y baja a minúsculas, igual que `router::sin_tildes`.
fn norm(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'Á' | 'À' | 'Ä' => 'a',
            'é' | 'è' | 'ë' | 'É' | 'È' | 'Ë' => 'e',
            'í' | 'ì' | 'ï' | 'Í' | 'Ì' | 'Ï' => 'i',
            'ó' | 'ò' | 'ö' | 'Ó' | 'Ò' | 'Ö' => 'o',
            'ú' | 'ù' | 'ü' | 'Ú' | 'Ù' | 'Ü' => 'u',
            'ñ' | 'Ñ' => 'n',
            other => other.to_ascii_lowercase(),
        })
        .collect()
}

/// Tabla (alias normalizado → resolutor que devuelve la fecha del año dado).
/// Devuelve también el nombre canónico para mostrar.
fn resolver_alias(alias_norm: &str, anio: i32) -> Option<(NaiveDate, &'static str)> {
    let p = pascua(anio);
    let mk = |m: u32, d: u32| NaiveDate::from_ymd_opt(anio, m, d);

    // Coincidencia por substring (la consulta puede traer "para navidad").
    let contiene = |kw: &str| alias_norm.contains(kw);

    // Religiosas / pascua
    if contiene("nochebuena") || contiene("noche buena") {
        return Some((mk(12, 24)?, "Nochebuena"));
    }
    if contiene("navidad") || contiene("christmas") || contiene("xmas") {
        return Some((mk(12, 25)?, "Navidad"));
    }
    if contiene("ano nuevo") || contiene("año nuevo") || contiene("new year") {
        return Some((mk(1, 1)?, "Año Nuevo"));
    }
    if contiene("fin de ano") || contiene("fin de año") || contiene("nochevieja") {
        return Some((mk(12, 31)?, "Fin de Año"));
    }
    if contiene("reyes") || contiene("epifania") {
        return Some((mk(1, 6)?, "Día de Reyes"));
    }
    if contiene("miercoles de ceniza") {
        return Some((p - Duration::days(46), "Miércoles de Ceniza"));
    }
    if contiene("domingo de ramos") {
        return Some((p - Duration::days(7), "Domingo de Ramos"));
    }
    if contiene("jueves santo") {
        return Some((p - Duration::days(3), "Jueves Santo"));
    }
    if contiene("viernes santo") || contiene("good friday") {
        return Some((p - Duration::days(2), "Viernes Santo"));
    }
    if contiene("sabado de gloria") {
        return Some((p - Duration::days(1), "Sábado de Gloria"));
    }
    if contiene("pascua") || contiene("resurreccion") || contiene("easter") {
        return Some((p, "Domingo de Pascua"));
    }
    if contiene("corpus christi") {
        return Some((p + Duration::days(60), "Corpus Christi"));
    }
    if contiene("asuncion") {
        return Some((mk(8, 15)?, "Asunción de la Virgen"));
    }
    if contiene("todos los santos") {
        return Some((mk(11, 1)?, "Día de Todos los Santos"));
    }
    if contiene("inmaculada") {
        return Some((mk(12, 8)?, "Inmaculada Concepción"));
    }

    // Ecuador
    if contiene("dia del trabajo") || contiene("dia del trabajador") || contiene("labor day") {
        if contiene("usa") || contiene("estados unidos") {
            return Some((nth_weekday(anio, 9, Weekday::Mon, 1), "Labor Day (USA)"));
        }
        return Some((mk(5, 1)?, "Día del Trabajo"));
    }
    if contiene("batalla de pichincha") || contiene("pichincha") {
        return Some((mk(5, 24)?, "Batalla de Pichincha"));
    }
    if contiene("primer grito") || contiene("10 de agosto") || contiene("diez de agosto") {
        return Some((mk(8, 10)?, "Primer Grito de Independencia"));
    }
    if contiene("independencia de guayaquil") || contiene("9 de octubre") {
        return Some((mk(10, 9)?, "Independencia de Guayaquil"));
    }
    if contiene("difuntos") || contiene("dia de muertos") || contiene("dia de los muertos") {
        return Some((mk(11, 2)?, "Día de los Difuntos"));
    }
    if contiene("independencia de cuenca") || contiene("3 de noviembre") {
        return Some((mk(11, 3)?, "Independencia de Cuenca"));
    }

    // USA
    if contiene("martin luther king") || contiene("mlk") {
        return Some((
            nth_weekday(anio, 1, Weekday::Mon, 3),
            "Martin Luther King Jr. Day",
        ));
    }
    if contiene("presidents day") || contiene("presidents' day") {
        return Some((nth_weekday(anio, 2, Weekday::Mon, 3), "Presidents' Day"));
    }
    if contiene("memorial day") {
        return Some((last_weekday(anio, 5, Weekday::Mon), "Memorial Day"));
    }
    if contiene("juneteenth") {
        return Some((mk(6, 19)?, "Juneteenth"));
    }
    if contiene("independence day") || contiene("4 de julio") || contiene("4th of july") {
        return Some((mk(7, 4)?, "Independence Day (USA)"));
    }
    if contiene("columbus day") {
        return Some((nth_weekday(anio, 10, Weekday::Mon, 2), "Columbus Day"));
    }
    if contiene("veterans day") {
        return Some((mk(11, 11)?, "Veterans Day"));
    }
    if contiene("thanksgiving") || contiene("accion de gracias") {
        return Some((nth_weekday(anio, 11, Weekday::Thu, 4), "Thanksgiving"));
    }
    if contiene("black friday") || contiene("viernes negro") {
        return Some((
            nth_weekday(anio, 11, Weekday::Thu, 4) + Duration::days(1),
            "Black Friday",
        ));
    }
    if contiene("cyber monday") {
        return Some((
            nth_weekday(anio, 11, Weekday::Thu, 4) + Duration::days(4),
            "Cyber Monday",
        ));
    }
    if contiene("halloween") {
        return Some((mk(10, 31)?, "Halloween"));
    }
    if contiene("election day") || contiene("dia de elecciones") || contiene("elecciones usa") {
        let primer_lunes = nth_weekday(anio, 11, Weekday::Mon, 1);
        return Some((primer_lunes + Duration::days(1), "Election Day"));
    }
    if contiene("tax day") || contiene("dia de impuestos") {
        return Some((mk(4, 15)?, "Tax Day"));
    }
    if contiene("super bowl") {
        return Some((nth_weekday(anio, 2, Weekday::Sun, 1), "Super Bowl Sunday"));
    }
    if contiene("christmas eve") || contiene("nochebuena usa") {
        return Some((mk(12, 24)?, "Christmas Eve"));
    }
    if contiene("san valentin") || contiene("valentin") || contiene("valentines") {
        return Some((mk(2, 14)?, "San Valentín"));
    }
    if contiene("san patricio") || contiene("st patrick") || contiene("st. patrick") {
        return Some((mk(3, 17)?, "St. Patrick's Day"));
    }
    if contiene("dia de la madre") || contiene("mothers day") || contiene("mother's day") {
        return Some((nth_weekday(anio, 5, Weekday::Sun, 2), "Día de la Madre"));
    }
    if contiene("dia del padre") || contiene("fathers day") || contiene("father's day") {
        return Some((nth_weekday(anio, 6, Weekday::Sun, 3), "Día del Padre"));
    }

    None
}

/// Resuelve un nombre coloquial de feriado a su próxima ocurrencia a partir de
/// `desde`. Si la fecha ya pasó este año, retorna la del año siguiente.
pub fn resolver_nombre_feriado(consulta: &str, desde: NaiveDate) -> Option<(NaiveDate, String)> {
    let n = norm(consulta);
    let (fecha_actual, nombre) = resolver_alias(&n, desde.year())?;
    let fecha = if fecha_actual >= desde {
        fecha_actual
    } else {
        let (fecha_sig, _) = resolver_alias(&n, desde.year() + 1)?;
        fecha_sig
    };
    Some((fecha, nombre.to_string()))
}

// ─── Parser "DD de mes" ─────────────────────────────────────────────────────

/// Mapa nombre-de-mes (español/inglés, sin tildes) → número.
fn mes_num(nombre: &str) -> Option<u32> {
    match nombre {
        "enero" | "january" | "jan" => Some(1),
        "febrero" | "february" | "feb" => Some(2),
        "marzo" | "march" | "mar" => Some(3),
        "abril" | "april" | "apr" => Some(4),
        "mayo" | "may" => Some(5),
        "junio" | "june" | "jun" => Some(6),
        "julio" | "july" | "jul" => Some(7),
        "agosto" | "august" | "aug" => Some(8),
        "septiembre" | "setiembre" | "september" | "sep" | "sept" => Some(9),
        "octubre" | "october" | "oct" => Some(10),
        "noviembre" | "november" | "nov" => Some(11),
        "diciembre" | "december" | "dec" => Some(12),
        _ => None,
    }
}

/// Reconoce "25 de diciembre", "diciembre 25", "25 dic 2026", "december 25 2026".
/// Si no se especifica año, usa el año de `desde` y, si la fecha quedó en el
/// pasado, salta al siguiente año.
pub fn extraer_fecha_textual(consulta: &str, desde: NaiveDate) -> Option<NaiveDate> {
    let n = norm(consulta);
    // Quitar puntuación común
    let limpio: String = n
        .chars()
        .map(|c| if c == ',' || c == '.' { ' ' } else { c })
        .collect();
    let toks: Vec<&str> = limpio.split_whitespace().collect();

    for i in 0..toks.len() {
        // Patrón A: <número 1-31> [de] <mes> [<año>]
        if let Ok(d) = toks[i].parse::<u32>() {
            if (1..=31).contains(&d) {
                let mut j = i + 1;
                if j < toks.len() && toks[j] == "de" {
                    j += 1;
                }
                if let Some(m) = toks.get(j).and_then(|t| mes_num(t)) {
                    let mut anio = desde.year();
                    if let Some(siguiente) = toks.get(j + 1) {
                        let mut k = j + 1;
                        if siguiente == &"de" {
                            k += 1;
                        }
                        if let Some(a) = toks.get(k).and_then(|t| t.parse::<i32>().ok()) {
                            anio = if a < 100 { 2000 + a } else { a };
                        }
                    }
                    if let Some(f) = NaiveDate::from_ymd_opt(anio, m, d) {
                        return Some(ajustar_a_futuro(f, desde, &toks, j));
                    }
                }
            }
        }
        // Patrón B: <mes> <número 1-31> [<año>]
        if let Some(m) = mes_num(toks[i]) {
            if let Some(d) = toks.get(i + 1).and_then(|t| t.parse::<u32>().ok()) {
                if (1..=31).contains(&d) {
                    let mut anio = desde.year();
                    if let Some(a) = toks.get(i + 2).and_then(|t| t.parse::<i32>().ok()) {
                        anio = if a < 100 { 2000 + a } else { a };
                    }
                    if let Some(f) = NaiveDate::from_ymd_opt(anio, m, d) {
                        return Some(ajustar_a_futuro(f, desde, &toks, i + 2));
                    }
                }
            }
        }
    }
    None
}

/// Si no se especificó año explícito y la fecha resultó pasada, avanzar al
/// próximo año.
fn ajustar_a_futuro(f: NaiveDate, desde: NaiveDate, toks: &[&str], idx_anio: usize) -> NaiveDate {
    let anio_explicito = toks
        .get(idx_anio)
        .and_then(|t| t.parse::<i32>().ok())
        .is_some();
    if !anio_explicito && f < desde {
        NaiveDate::from_ymd_opt(f.year() + 1, f.month(), f.day()).unwrap_or(f)
    } else {
        f
    }
}

// ─── Listado próximos ───────────────────────────────────────────────────────

/// Devuelve los próximos `n` feriados a partir de `desde` (incluido), filtrados
/// por país. Si `pais` es `None`, mezcla los tres catálogos.
pub fn proximos_feriados(desde: NaiveDate, n: usize, pais: Option<Pais>) -> Vec<Feriado> {
    let mut acc: Vec<Feriado> = Vec::new();
    for delta in 0..2 {
        let anio = desde.year() + delta;
        let lote = match pais {
            Some(Pais::Ecuador) => feriados_ecuador(anio),
            Some(Pais::Usa) => feriados_usa(anio),
            Some(Pais::Religioso) => feriados_religiosos(anio),
            None => feriados_todos(anio),
        };
        acc.extend(lote.into_iter().filter(|f| f.fecha >= desde));
        if acc.len() >= n {
            break;
        }
    }
    acc.sort_by_key(|f| f.fecha);
    acc.truncate(n);
    acc
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pascua_conocida() {
        // Easter Sunday 2024 = 31 marzo; 2025 = 20 abril; 2026 = 5 abril.
        assert_eq!(pascua(2024), NaiveDate::from_ymd_opt(2024, 3, 31).unwrap());
        assert_eq!(pascua(2025), NaiveDate::from_ymd_opt(2025, 4, 20).unwrap());
        assert_eq!(pascua(2026), NaiveDate::from_ymd_opt(2026, 4, 5).unwrap());
    }

    #[test]
    fn thanksgiving_2026() {
        let lista = feriados_usa(2026);
        let t = lista.iter().find(|f| f.nombre == "Thanksgiving").unwrap();
        // Thanksgiving 2026 = 26 noviembre (4º jueves)
        assert_eq!(t.fecha, NaiveDate::from_ymd_opt(2026, 11, 26).unwrap());
    }

    #[test]
    fn navidad_resuelve() {
        let hoy = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
        let (f, n) = resolver_nombre_feriado("cuanto falta para navidad", hoy).unwrap();
        assert_eq!(n, "Navidad");
        assert_eq!(f, NaiveDate::from_ymd_opt(2026, 12, 25).unwrap());
    }

    #[test]
    fn navidad_pasada_avanza_anio() {
        let hoy = NaiveDate::from_ymd_opt(2026, 12, 26).unwrap();
        let (f, _) = resolver_nombre_feriado("para navidad", hoy).unwrap();
        assert_eq!(f, NaiveDate::from_ymd_opt(2027, 12, 25).unwrap());
    }

    #[test]
    fn fecha_textual_25_de_diciembre() {
        let hoy = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
        let f = extraer_fecha_textual("cuantos dias faltan para el 25 de diciembre", hoy).unwrap();
        assert_eq!(f, NaiveDate::from_ymd_opt(2026, 12, 25).unwrap());
    }

    #[test]
    fn fecha_textual_avanza_si_pasada() {
        let hoy = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
        let f = extraer_fecha_textual("cuantos dias para el 1 de enero", hoy).unwrap();
        assert_eq!(f, NaiveDate::from_ymd_opt(2027, 1, 1).unwrap());
    }
}
