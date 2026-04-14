use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ══════════════════════════════════════════════════════════════
//  Sistema de Feedback — aprendizaje por retroalimentación
//  Recoge valoraciones del usuario y ajusta pesos del sistema
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Feedback {
    pub id: usize,
    pub consulta_original: String,
    pub respuesta_dada: String,
    pub valoracion: Valoracion,
    pub comentario: Option<String>,
    pub componente: String, // "sentimiento", "intencion", "conocimiento", "respuesta"
    pub timestamp: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Valoracion {
    MuyBuena, //  2
    Buena,    //  1
    Neutral,  //  0
    Mala,     // -1
    MuyMala,  // -2
}

impl Valoracion {
    pub fn score(&self) -> f64 {
        match self {
            Self::MuyBuena => 1.0,
            Self::Buena => 0.5,
            Self::Neutral => 0.0,
            Self::Mala => -0.5,
            Self::MuyMala => -1.0,
        }
    }

    pub fn nombre(&self) -> &str {
        match self {
            Self::MuyBuena => "Muy buena ⭐⭐⭐",
            Self::Buena => "Buena ⭐⭐",
            Self::Neutral => "Neutral ⭐",
            Self::Mala => "Mala 👎",
            Self::MuyMala => "Muy mala 👎👎",
        }
    }

    pub fn desde_indice(i: usize) -> Self {
        match i {
            0 => Self::MuyBuena,
            1 => Self::Buena,
            2 => Self::Neutral,
            3 => Self::Mala,
            _ => Self::MuyMala,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EstadisticasFeedback {
    pub total: usize,
    pub promedio: f64,
    pub distribucion: HashMap<String, usize>,
    pub satisfaccion: f64, // 0-100%
    pub tendencia: f64,    // comparando últimas 10 vs anteriores
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SistemaFeedback {
    pub feedbacks: Vec<Feedback>,
    /// Ajustes aprendidos: componente.clave → ajuste
    pub ajustes: HashMap<String, f64>,
    siguiente_id: usize,
}

impl SistemaFeedback {
    pub fn nuevo() -> Self {
        Self {
            feedbacks: Vec::new(),
            ajustes: HashMap::new(),
            siguiente_id: 1,
        }
    }

    /// Registrar feedback del usuario
    pub fn registrar(
        &mut self,
        consulta: &str,
        respuesta: &str,
        valoracion: Valoracion,
        comentario: Option<String>,
        componente: &str,
    ) -> usize {
        let id = self.siguiente_id;
        self.siguiente_id += 1;

        let feedback = Feedback {
            id,
            consulta_original: consulta.to_string(),
            respuesta_dada: respuesta.to_string(),
            valoracion,
            comentario,
            componente: componente.to_string(),
            timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        };

        self.feedbacks.push(feedback);

        // Actualizar ajustes automáticos
        self.recalcular_ajustes();

        id
    }

    /// Recalcular ajustes basados en feedback acumulado
    fn recalcular_ajustes(&mut self) {
        // Agrupar por componente
        let mut por_componente: HashMap<String, Vec<f64>> = HashMap::new();
        for fb in &self.feedbacks {
            por_componente
                .entry(fb.componente.clone())
                .or_default()
                .push(fb.valoracion.score());
        }

        for (componente, scores) in &por_componente {
            if scores.is_empty() {
                continue;
            }

            let promedio: f64 = scores.iter().sum::<f64>() / scores.len() as f64;

            // Ajuste: si feedback es consistentemente malo, penalizar; si bueno, reforzar
            // Usamos una media ponderada exponencial (más recientes pesan más)
            let mut suma_ponderada = 0.0;
            let mut peso_total = 0.0;
            for (i, &score) in scores.iter().enumerate() {
                let peso = (1.0 + i as f64 * 0.1).min(3.0); // recientes pesan más
                suma_ponderada += score * peso;
                peso_total += peso;
            }
            let ajuste = if peso_total > 0.0 {
                suma_ponderada / peso_total
            } else {
                promedio
            };

            self.ajustes.insert(componente.clone(), ajuste);
        }
    }

    /// Obtener ajuste para un componente
    pub fn obtener_ajuste(&self, componente: &str) -> f64 {
        self.ajustes.get(componente).copied().unwrap_or(0.0)
    }

    /// ¿El componente necesita mejora?
    pub fn necesita_mejora(&self, componente: &str) -> bool {
        self.obtener_ajuste(componente) < -0.2
    }

    /// Estadísticas generales
    pub fn estadisticas(&self) -> EstadisticasFeedback {
        let total = self.feedbacks.len();
        if total == 0 {
            return EstadisticasFeedback {
                total: 0,
                promedio: 0.0,
                distribucion: HashMap::new(),
                satisfaccion: 50.0,
                tendencia: 0.0,
            };
        }

        let scores: Vec<f64> = self
            .feedbacks
            .iter()
            .map(|f| f.valoracion.score())
            .collect();
        let promedio = scores.iter().sum::<f64>() / total as f64;

        let mut distribucion = HashMap::new();
        for fb in &self.feedbacks {
            *distribucion
                .entry(fb.valoracion.nombre().to_string())
                .or_insert(0) += 1;
        }

        // Satisfacción: % de feedbacks positivos (>0)
        let positivos = scores.iter().filter(|&&s| s > 0.0).count();
        let satisfaccion = (positivos as f64 / total as f64) * 100.0;

        // Tendencia: comparar últimas 10 con anteriores
        let tendencia = if total >= 20 {
            let ultimas: f64 = scores[total - 10..].iter().sum::<f64>() / 10.0;
            let anteriores: f64 = scores[total - 20..total - 10].iter().sum::<f64>() / 10.0;
            ultimas - anteriores
        } else if total >= 10 {
            let mitad = total / 2;
            let recientes: f64 = scores[mitad..].iter().sum::<f64>() / (total - mitad) as f64;
            let antiguos: f64 = scores[..mitad].iter().sum::<f64>() / mitad as f64;
            recientes - antiguos
        } else {
            0.0
        };

        EstadisticasFeedback {
            total,
            promedio,
            distribucion,
            satisfaccion,
            tendencia,
        }
    }

    /// Estadísticas por componente
    pub fn estadisticas_componente(&self, componente: &str) -> EstadisticasFeedback {
        let filtrado: Vec<&Feedback> = self
            .feedbacks
            .iter()
            .filter(|f| f.componente == componente)
            .collect();

        let total = filtrado.len();
        if total == 0 {
            return EstadisticasFeedback {
                total: 0,
                promedio: 0.0,
                distribucion: HashMap::new(),
                satisfaccion: 50.0,
                tendencia: 0.0,
            };
        }

        let scores: Vec<f64> = filtrado.iter().map(|f| f.valoracion.score()).collect();
        let promedio = scores.iter().sum::<f64>() / total as f64;

        let mut distribucion = HashMap::new();
        for fb in &filtrado {
            *distribucion
                .entry(fb.valoracion.nombre().to_string())
                .or_insert(0) += 1;
        }

        let positivos = scores.iter().filter(|&&s| s > 0.0).count();
        let satisfaccion = (positivos as f64 / total as f64) * 100.0;

        EstadisticasFeedback {
            total,
            promedio,
            distribucion,
            satisfaccion,
            tendencia: 0.0,
        }
    }

    pub fn resumen(&self) {
        println!("  Sistema de Feedback");
        println!("  ───────────────────");
        println!("    Feedbacks recibidos: {}", self.feedbacks.len());

        if !self.feedbacks.is_empty() {
            let stats = self.estadisticas();
            println!("    Satisfacción: {:.1}%", stats.satisfaccion);
            println!("    Promedio: {:.2}", stats.promedio);
            if stats.tendencia.abs() > 0.05 {
                let dir = if stats.tendencia > 0.0 {
                    "↑ mejorando"
                } else {
                    "↓ empeorando"
                };
                println!("    Tendencia: {} ({:.2})", dir, stats.tendencia);
            }
        }

        if !self.ajustes.is_empty() {
            println!("    Ajustes activos:");
            for (comp, ajuste) in &self.ajustes {
                let estado = if *ajuste > 0.2 {
                    "✅"
                } else if *ajuste < -0.2 {
                    "⚠️"
                } else {
                    "➖"
                };
                println!("      {} {}: {:.2}", estado, comp, ajuste);
            }
        }
    }
}

impl Default for SistemaFeedback {
    fn default() -> Self {
        Self::nuevo()
    }
}
