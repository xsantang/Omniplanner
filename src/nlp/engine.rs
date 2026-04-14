use super::conversation::GestorConversaciones;
use super::feedback::{SistemaFeedback, Valoracion};
use super::intent::{CategoriaIntencion, ClasificadorIntencion};
use super::knowledge::BaseConocimiento;
use super::sentiment::AnalizadorSentimiento;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ══════════════════════════════════════════════════════════════
//  Motor NLP Central — orquesta todos los componentes
//  Combina reglas + datos, gestiona ambigüedad, multi-turno
// ══════════════════════════════════════════════════════════════

/// Resultado completo de procesar un input del usuario
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResultadoNLP {
    pub texto_original: String,
    pub respuesta: String,
    pub intencion: String,
    pub confianza_intencion: f64,
    pub sentimiento: String,
    pub score_sentimiento: f64,
    pub entidades: Vec<(String, String)>,
    pub fuente_conocimiento: Option<String>,
    pub ambigua: bool,
    pub sugerencias: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigNLP {
    pub umbral_confianza: f64,    // mínimo para actuar sin clarificar
    pub max_resultados_kb: usize, // máx. resultados de knowledge base
    pub usar_sentimiento: bool,
    pub usar_conocimiento: bool,
    pub usar_feedback: bool,
    pub modo_verbose: bool,
    pub idioma_preferido: String, // "es" o "en"
}

impl Default for ConfigNLP {
    fn default() -> Self {
        Self {
            umbral_confianza: 0.35,
            max_resultados_kb: 3,
            usar_sentimiento: true,
            usar_conocimiento: true,
            usar_feedback: true,
            modo_verbose: false,
            idioma_preferido: "es".to_string(),
        }
    }
}

/// Datos de entrenamiento predefinidos (diversos)
#[derive(Clone, Debug)]
pub struct DatosEntrenamiento;

impl DatosEntrenamiento {
    /// Datos de sentimiento: (texto, score -1 a 1)
    pub fn sentimiento_es() -> Vec<(&'static str, f64)> {
        vec![
            ("me encanta este proyecto es genial", 0.9),
            ("excelente trabajo bien hecho", 0.85),
            ("esto es maravilloso me siento feliz", 0.9),
            ("buen avance sigue asi", 0.7),
            ("no esta mal pero puede mejorar", 0.2),
            ("es aceptable normal", 0.0),
            ("no me gusta nada esto", -0.8),
            ("terrible experiencia muy frustrado", -0.9),
            ("esto es un desastre total", -0.95),
            ("estoy preocupado por el resultado", -0.5),
            ("que aburrido no sirve para nada", -0.7),
            ("pesimo servicio nunca mas", -0.9),
            ("super contento con los resultados", 0.85),
            ("todo perfecto gracias", 0.8),
            ("increible lo logre", 0.9),
            ("que horror no funciona", -0.85),
            ("odio perder el tiempo asi", -0.8),
            ("buenas noticias todo avanza", 0.7),
            ("lamentablemente no pudimos", -0.6),
            ("estoy motivado a seguir", 0.7),
        ]
    }

    /// Datos de intención: (texto, categoría)
    pub fn intenciones_es() -> Vec<(&'static str, CategoriaIntencion)> {
        vec![
            ("crear nueva tarea para hoy", CategoriaIntencion::Crear),
            ("agregar evento a la agenda", CategoriaIntencion::Crear),
            ("quiero añadir un proyecto nuevo", CategoriaIntencion::Crear),
            ("nuevo canvas de planificacion", CategoriaIntencion::Crear),
            ("programar reunion para mañana", CategoriaIntencion::Crear),
            ("mostrar todas mis tareas", CategoriaIntencion::Listar),
            ("ver la lista de pendientes", CategoriaIntencion::Listar),
            ("listar todos los proyectos", CategoriaIntencion::Listar),
            (
                "dame todas las tareas completadas",
                CategoriaIntencion::Listar,
            ),
            ("editar la tarea del lunes", CategoriaIntencion::Modificar),
            ("cambiar la fecha del evento", CategoriaIntencion::Modificar),
            ("modificar la prioridad", CategoriaIntencion::Modificar),
            (
                "actualizar el titulo de la tarea",
                CategoriaIntencion::Modificar,
            ),
            ("eliminar esa tarea", CategoriaIntencion::Eliminar),
            ("borrar el evento de ayer", CategoriaIntencion::Eliminar),
            ("quitar la tarea vieja", CategoriaIntencion::Eliminar),
            ("buscar tareas urgentes", CategoriaIntencion::Buscar),
            ("encontrar el proyecto de rust", CategoriaIntencion::Buscar),
            ("hola como estas", CategoriaIntencion::Saludo),
            ("buenos dias", CategoriaIntencion::Saludo),
            ("adios hasta luego", CategoriaIntencion::Despedida),
            ("chao nos vemos", CategoriaIntencion::Despedida),
            ("muchas gracias", CategoriaIntencion::Agradecimiento),
            ("si eso es correcto", CategoriaIntencion::Afirmacion),
            ("no para nada", CategoriaIntencion::Negacion),
            ("ayuda como funciona esto", CategoriaIntencion::Ayuda),
            ("que opciones tengo", CategoriaIntencion::Ayuda),
            (
                "configurar las preferencias",
                CategoriaIntencion::Configurar,
            ),
            ("exportar mis datos", CategoriaIntencion::Exportar),
            ("que es una tarea", CategoriaIntencion::Consultar),
            (
                "cuando es mi siguiente evento",
                CategoriaIntencion::Consultar,
            ),
            ("como agrego algo", CategoriaIntencion::Consultar),
        ]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MotorNLP {
    pub sentimiento: AnalizadorSentimiento,
    pub intencion: ClasificadorIntencion,
    pub conocimiento: BaseConocimiento,
    pub conversaciones: GestorConversaciones,
    pub feedback: SistemaFeedback,
    pub config: ConfigNLP,
    pub estadisticas: EstadisticasMotor,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EstadisticasMotor {
    pub consultas_procesadas: usize,
    pub respuestas_exitosas: usize,
    pub ambiguedades_detectadas: usize,
    pub feedback_recibido: usize,
    pub sentimientos_por_tipo: HashMap<String, usize>,
    pub intenciones_por_tipo: HashMap<String, usize>,
}

impl MotorNLP {
    pub fn nuevo() -> Self {
        Self {
            sentimiento: AnalizadorSentimiento::nuevo(),
            intencion: ClasificadorIntencion::nuevo(),
            conocimiento: BaseConocimiento::nueva(),
            conversaciones: GestorConversaciones::nuevo(),
            feedback: SistemaFeedback::nuevo(),
            config: ConfigNLP::default(),
            estadisticas: EstadisticasMotor::default(),
        }
    }

    /// Procesar input del usuario — pipeline completo
    pub fn procesar(&mut self, texto: &str) -> ResultadoNLP {
        self.estadisticas.consultas_procesadas += 1;

        // 1. Análisis de sentimiento
        let sent = if self.config.usar_sentimiento {
            self.sentimiento.analizar(texto)
        } else {
            super::sentiment::ResultadoSentimiento {
                polaridad: super::sentiment::Polaridad::Neutro,
                score: 0.0,
                confianza: 0.0,
                emociones: HashMap::new(),
                palabras_clave: Vec::new(),
            }
        };

        // Estadísticas
        *self
            .estadisticas
            .sentimientos_por_tipo
            .entry(sent.polaridad.nombre().to_string())
            .or_insert(0) += 1;

        // 2. Clasificación de intención
        let intent = self.intencion.clasificar(texto);
        let ambigua = self.intencion.es_ambigua(&intent);

        *self
            .estadisticas
            .intenciones_por_tipo
            .entry(intent.categoria.nombre().to_string())
            .or_insert(0) += 1;

        if ambigua {
            self.estadisticas.ambiguedades_detectadas += 1;
        }

        // 3. Consultar base de conocimiento
        let fuente_kb = if self.config.usar_conocimiento {
            let resultados = self
                .conocimiento
                .buscar(texto, self.config.max_resultados_kb);
            if let Some(mejor) = resultados.first() {
                if mejor.relevancia > 0.15 {
                    Some(mejor.entrada.titulo.clone())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // 4. Generar respuesta
        let respuesta = self.generar_respuesta(texto, &intent.categoria, ambigua, &fuente_kb);

        // 5. Registrar en conversación
        if self.conversaciones.conversacion_activa.is_none() {
            self.conversaciones.nueva_conversacion();
        }
        if let Some(conv) = self.conversaciones.activa_mut() {
            let entidades: HashMap<String, String> = intent
                .entidades
                .iter()
                .map(|e| (e.tipo.clone(), e.valor.clone()))
                .collect();
            conv.turno_usuario(
                texto,
                Some(intent.categoria.clone()),
                Some(sent.polaridad.clone()),
                entidades,
            );
            conv.turno_sistema(&respuesta);
        }

        // 6. Ajustar por feedback previo
        let respuesta = if self.config.usar_feedback {
            self.ajustar_por_feedback(respuesta, &intent.categoria)
        } else {
            respuesta
        };

        self.estadisticas.respuestas_exitosas += 1;

        // Sugerencias contextuales
        let sugerencias = self.generar_sugerencias(&intent.categoria);

        // Entidades
        let entidades: Vec<(String, String)> = intent
            .entidades
            .iter()
            .map(|e| (e.tipo.clone(), e.valor.clone()))
            .collect();

        ResultadoNLP {
            texto_original: texto.to_string(),
            respuesta,
            intencion: intent.categoria.nombre().to_string(),
            confianza_intencion: intent.confianza,
            sentimiento: sent.polaridad.nombre().to_string(),
            score_sentimiento: sent.score,
            entidades,
            fuente_conocimiento: fuente_kb,
            ambigua,
            sugerencias,
        }
    }

    fn generar_respuesta(
        &mut self,
        texto: &str,
        intencion: &CategoriaIntencion,
        ambigua: bool,
        fuente_kb: &Option<String>,
    ) -> String {
        // Intentar respuesta de conversación primero
        if let Some(conv) = self.conversaciones.activa_mut() {
            let resp_conv = conv.decidir_respuesta(intencion, ambigua);

            // Si la intención es consulta y hay KB, enriquecer
            if matches!(
                intencion,
                CategoriaIntencion::Consultar
                    | CategoriaIntencion::Ayuda
                    | CategoriaIntencion::Buscar
            ) {
                if let Some(resp_kb) = self.conocimiento.generar_respuesta(texto) {
                    return format!("{}\n\n📚 {}", resp_conv, resp_kb);
                }
            }

            return resp_conv;
        }

        // Fallback: usando base de conocimiento
        if fuente_kb.is_some() {
            if let Some(resp) = self.conocimiento.generar_respuesta(texto) {
                return resp;
            }
        }

        "No estoy seguro de cómo ayudar con eso. ¿Puedes dar más detalles?".to_string()
    }

    fn ajustar_por_feedback(&self, respuesta: String, intencion: &CategoriaIntencion) -> String {
        let componente = format!("intencion_{}", intencion.nombre());
        let ajuste = self.feedback.obtener_ajuste(&componente);

        if ajuste < -0.3 {
            // El sistema ha recibido mucho feedback negativo para esta intención
            format!(
                "{}\n\n💡 (Estoy mejorando en este tipo de respuestas. Tu feedback ayuda.)",
                respuesta
            )
        } else {
            respuesta
        }
    }

    fn generar_sugerencias(&self, intencion: &CategoriaIntencion) -> Vec<String> {
        match intencion {
            CategoriaIntencion::Saludo => vec![
                "Puedes pedirme crear tareas".to_string(),
                "Pregúntame sobre tus pendientes".to_string(),
                "Intenta: 'listar tareas'".to_string(),
            ],
            CategoriaIntencion::Crear => vec![
                "Puedes crear: tareas, eventos, canvas, diagramas".to_string(),
                "Especifica una fecha para agendar".to_string(),
            ],
            CategoriaIntencion::Ayuda => vec![
                "Módulos disponibles: Tareas, Agenda, Canvas, ML, NLP".to_string(),
                "Escribe 'listar tareas' o 'crear tarea'".to_string(),
            ],
            CategoriaIntencion::Desconocido => vec![
                "Intenta ser más específico".to_string(),
                "Puedo ayudar con: tareas, agenda, canvas y más".to_string(),
                "Escribe 'ayuda' para ver opciones".to_string(),
            ],
            _ => Vec::new(),
        }
    }

    /// Registrar feedback del usuario
    pub fn registrar_feedback(
        &mut self,
        consulta: &str,
        respuesta: &str,
        valoracion: Valoracion,
        comentario: Option<String>,
    ) {
        self.feedback
            .registrar(consulta, respuesta, valoracion, comentario, "general");
        self.estadisticas.feedback_recibido += 1;
    }

    /// Entrenar todos los modelos con datos predefinidos
    pub fn entrenar_completo(&mut self) {
        println!("\n  🧠 Entrenando modelo de sentimiento...");
        let datos_sent = DatosEntrenamiento::sentimiento_es();
        self.sentimiento.entrenar_ml(&datos_sent, 100, 0.05);

        println!("\n  🧠 Entrenando clasificador de intención...");
        let datos_intent = DatosEntrenamiento::intenciones_es();
        self.intencion.entrenar(&datos_intent, 100, 0.1);

        println!("\n  ✅ Entrenamiento completo.");
    }

    /// Entrenar solo sentimiento
    pub fn entrenar_sentimiento(&mut self, datos: &[(&str, f64)], epocas: usize, lr: f64) {
        self.sentimiento.entrenar_ml(datos, epocas, lr);
    }

    /// Entrenar solo intención
    pub fn entrenar_intencion(
        &mut self,
        datos: &[(&str, CategoriaIntencion)],
        epocas: usize,
        lr: f64,
    ) {
        self.intencion.entrenar(datos, epocas, lr);
    }

    /// Nueva conversación
    pub fn nueva_conversacion(&mut self) -> String {
        self.conversaciones.nueva_conversacion()
    }

    /// Agregar conocimiento
    pub fn agregar_conocimiento(
        &mut self,
        titulo: &str,
        contenido: &str,
        categoria: &str,
        etiquetas: &[String],
    ) -> String {
        self.conocimiento
            .agregar(titulo, contenido, categoria, etiquetas)
    }

    /// Resumen completo del motor
    pub fn resumen(&self) {
        println!("\n  ╔══════════════════════════════════════╗");
        println!("  ║    🧠 Motor NLP — Estado General     ║");
        println!("  ╚══════════════════════════════════════╝\n");

        self.sentimiento.resumen();
        println!();
        self.intencion.resumen();
        println!();
        self.conocimiento.resumen();
        println!();
        self.feedback.resumen();

        println!("\n  Estadísticas Generales");
        println!("  ──────────────────────");
        println!(
            "    Consultas procesadas: {}",
            self.estadisticas.consultas_procesadas
        );
        println!(
            "    Respuestas exitosas: {}",
            self.estadisticas.respuestas_exitosas
        );
        println!(
            "    Ambigüedades: {}",
            self.estadisticas.ambiguedades_detectadas
        );
        println!(
            "    Feedback recibido: {}",
            self.estadisticas.feedback_recibido
        );
        println!("    Conversaciones: {}", self.conversaciones.total());

        if !self.estadisticas.intenciones_por_tipo.is_empty() {
            println!("    Intenciones:");
            let mut sorted: Vec<_> = self.estadisticas.intenciones_por_tipo.iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(a.1));
            for (intent, count) in sorted.iter().take(5) {
                println!("      {}: {}", intent, count);
            }
        }

        if !self.estadisticas.sentimientos_por_tipo.is_empty() {
            println!("    Sentimientos:");
            for (sent, count) in &self.estadisticas.sentimientos_por_tipo {
                println!("      {}: {}", sent, count);
            }
        }
    }
}

impl Default for MotorNLP {
    fn default() -> Self {
        Self::nuevo()
    }
}
