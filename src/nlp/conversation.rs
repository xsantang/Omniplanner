use super::intent::CategoriaIntencion;
use super::sentiment::Polaridad;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ══════════════════════════════════════════════════════════════
//  Conversación Multi-turno — contexto y gestión de diálogo
//  Mantiene historial, contexto activo y estado del diálogo
// ══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Turno {
    pub id: usize,
    pub rol: Rol,
    pub texto: String,
    pub intencion: Option<CategoriaIntencion>,
    pub sentimiento: Option<Polaridad>,
    pub timestamp: String,
    pub entidades: HashMap<String, String>, // entidades extraídas
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Rol {
    Usuario,
    Sistema,
}

impl Rol {
    pub fn nombre(&self) -> &str {
        match self {
            Self::Usuario => "Usuario",
            Self::Sistema => "Sistema",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum EstadoDialogo {
    Inicio,
    Esperando,    // esperando input del usuario
    Procesando,   // procesando solicitud
    Confirmando,  // pidiendo confirmación
    Clarificando, // pidiendo clarificación
    Completado,   // acción completada
    Error,
}

impl EstadoDialogo {
    pub fn nombre(&self) -> &str {
        match self {
            Self::Inicio => "Inicio",
            Self::Esperando => "Esperando",
            Self::Procesando => "Procesando",
            Self::Confirmando => "Confirmando",
            Self::Clarificando => "Clarificando",
            Self::Completado => "Completado",
            Self::Error => "Error",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContextoConversacion {
    pub tema_actual: Option<String>,
    pub intencion_activa: Option<CategoriaIntencion>,
    pub entidades_acumuladas: HashMap<String, String>,
    pub sentimiento_general: f64, // promedio -1.0 a 1.0
    pub turnos_en_tema: usize,
    pub preguntas_pendientes: Vec<String>,
}

impl Default for ContextoConversacion {
    fn default() -> Self {
        Self {
            tema_actual: None,
            intencion_activa: None,
            entidades_acumuladas: HashMap::new(),
            sentimiento_general: 0.0,
            turnos_en_tema: 0,
            preguntas_pendientes: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Conversacion {
    pub id: String,
    pub turnos: Vec<Turno>,
    pub estado: EstadoDialogo,
    pub contexto: ContextoConversacion,
    pub siguiente_turno: usize,
    /// Respuestas predefinidas por estado/intención
    pub plantillas: HashMap<String, Vec<String>>,
}

impl Conversacion {
    pub fn nueva(id: &str) -> Self {
        let mut conv = Self {
            id: id.to_string(),
            turnos: Vec::new(),
            estado: EstadoDialogo::Inicio,
            contexto: ContextoConversacion::default(),
            siguiente_turno: 1,
            plantillas: HashMap::new(),
        };
        conv.cargar_plantillas();
        conv
    }

    fn cargar_plantillas(&mut self) {
        let plantillas: &[(&str, &[&str])] = &[
            (
                "saludo",
                &[
                    "¡Hola! ¿En qué puedo ayudarte hoy?",
                    "¡Buenos días! Estoy aquí para asistirte.",
                    "¡Hey! ¿Qué necesitas?",
                    "¡Bienvenido de vuelta! ¿Cómo te puedo ayudar?",
                ],
            ),
            (
                "despedida",
                &[
                    "¡Hasta luego! Que tengas un buen día.",
                    "¡Chao! Si necesitas algo más, aquí estaré.",
                    "¡Nos vemos! Buena suerte con tus tareas.",
                ],
            ),
            (
                "agradecimiento",
                &[
                    "¡De nada! Estoy para ayudar.",
                    "¡Con gusto! ¿Hay algo más?",
                    "¡No hay de qué! Si necesitas algo más, dime.",
                ],
            ),
            (
                "no_entiendo",
                &[
                    "No estoy seguro de entender. ¿Podrías reformularlo?",
                    "Hmm, no capté bien eso. ¿Puedes ser más específico?",
                    "¿Podrías darme más detalles sobre lo que necesitas?",
                    "No estoy seguro a qué te refieres. ¿Me das más contexto?",
                ],
            ),
            (
                "confirmacion",
                &[
                    "¿Estás seguro de que quieres hacer eso?",
                    "¿Confirmas esta acción?",
                    "Antes de proceder, ¿esto es correcto?",
                ],
            ),
            (
                "error",
                &[
                    "Hubo un problema. ¿Intentamos de nuevo?",
                    "Algo salió mal. ¿Puedo ayudar de otra forma?",
                    "Encontré un error procesando tu solicitud.",
                ],
            ),
            (
                "exito",
                &[
                    "¡Listo! Se completó exitosamente.",
                    "¡Hecho! ¿Necesitas algo más?",
                    "Perfecto, ya está listo.",
                ],
            ),
            (
                "clarificar_crear",
                &[
                    "¿Qué quieres crear? Puedo ayudarte con tareas, eventos, canvas o diagramas.",
                    "¿Qué tipo de elemento deseas crear?",
                ],
            ),
            (
                "clarificar_buscar",
                &[
                    "¿Qué estás buscando exactamente?",
                    "¿Puedes darme más detalles sobre lo que quieres buscar?",
                ],
            ),
            (
                "sentimiento_negativo",
                &[
                    "Noto que algo te preocupa. ¿Puedo ayudar de alguna forma?",
                    "Entiendo que puede ser frustrante. Veamos cómo resolver esto.",
                    "Lamento la dificultad. Intentemos otra forma.",
                ],
            ),
        ];

        for &(clave, opciones) in plantillas {
            self.plantillas.insert(
                clave.to_string(),
                opciones.iter().map(|s| s.to_string()).collect(),
            );
        }
    }

    /// Agregar turno del usuario
    pub fn turno_usuario(
        &mut self,
        texto: &str,
        intencion: Option<CategoriaIntencion>,
        sentimiento: Option<Polaridad>,
        entidades: HashMap<String, String>,
    ) -> usize {
        let id = self.siguiente_turno;
        self.siguiente_turno += 1;

        let turno = Turno {
            id,
            rol: Rol::Usuario,
            texto: texto.to_string(),
            intencion: intencion.clone(),
            sentimiento: sentimiento.clone(),
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
            entidades: entidades.clone(),
        };

        self.turnos.push(turno);

        // Actualizar contexto
        if let Some(intent) = &intencion {
            self.contexto.intencion_activa = Some(intent.clone());
        }
        for (k, v) in &entidades {
            self.contexto
                .entidades_acumuladas
                .insert(k.clone(), v.clone());
        }
        if let Some(sent) = &sentimiento {
            let val = match sent {
                Polaridad::MuyPositivo => 1.0,
                Polaridad::Positivo => 0.5,
                Polaridad::Neutro => 0.0,
                Polaridad::Negativo => -0.5,
                Polaridad::MuyNegativo => -1.0,
            };
            let n = self.turnos.iter().filter(|t| t.rol == Rol::Usuario).count() as f64;
            self.contexto.sentimiento_general =
                (self.contexto.sentimiento_general * (n - 1.0) + val) / n;
        }

        self.contexto.turnos_en_tema += 1;
        id
    }

    /// Agregar respuesta del sistema
    pub fn turno_sistema(&mut self, texto: &str) -> usize {
        let id = self.siguiente_turno;
        self.siguiente_turno += 1;

        let turno = Turno {
            id,
            rol: Rol::Sistema,
            texto: texto.to_string(),
            intencion: None,
            sentimiento: None,
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
            entidades: HashMap::new(),
        };

        self.turnos.push(turno);
        id
    }

    /// Obtener respuesta de plantilla
    pub fn respuesta_plantilla(&self, clave: &str) -> String {
        if let Some(opciones) = self.plantillas.get(clave) {
            // Usar turno actual como pseudo-random selector
            let idx = self.siguiente_turno % opciones.len();
            opciones[idx].clone()
        } else {
            "No tengo una respuesta para eso.".to_string()
        }
    }

    /// Decidir respuesta automática basada en intención y contexto
    pub fn decidir_respuesta(&mut self, intencion: &CategoriaIntencion, ambigua: bool) -> String {
        // Si sentimiento muy negativo, empatizar primero
        if self.contexto.sentimiento_general < -0.6 {
            let empatia = self.respuesta_plantilla("sentimiento_negativo");
            self.estado = EstadoDialogo::Esperando;
            return empatia;
        }

        match intencion {
            CategoriaIntencion::Saludo => {
                self.estado = EstadoDialogo::Esperando;
                self.contexto.tema_actual = None;
                self.contexto.turnos_en_tema = 0;
                self.respuesta_plantilla("saludo")
            }
            CategoriaIntencion::Despedida => {
                self.estado = EstadoDialogo::Completado;
                self.respuesta_plantilla("despedida")
            }
            CategoriaIntencion::Agradecimiento => {
                self.estado = EstadoDialogo::Esperando;
                self.respuesta_plantilla("agradecimiento")
            }
            CategoriaIntencion::Crear => {
                if ambigua {
                    self.estado = EstadoDialogo::Clarificando;
                    self.respuesta_plantilla("clarificar_crear")
                } else {
                    self.estado = EstadoDialogo::Confirmando;
                    format!(
                        "Voy a crear eso. {}",
                        self.respuesta_plantilla("confirmacion")
                    )
                }
            }
            CategoriaIntencion::Eliminar => {
                self.estado = EstadoDialogo::Confirmando;
                self.respuesta_plantilla("confirmacion")
            }
            CategoriaIntencion::Buscar => {
                if ambigua {
                    self.estado = EstadoDialogo::Clarificando;
                    self.respuesta_plantilla("clarificar_buscar")
                } else {
                    self.estado = EstadoDialogo::Procesando;
                    "Buscando...".to_string()
                }
            }
            CategoriaIntencion::Afirmacion => {
                if self.estado == EstadoDialogo::Confirmando {
                    self.estado = EstadoDialogo::Procesando;
                    "Entendido, procediendo.".to_string()
                } else {
                    self.estado = EstadoDialogo::Esperando;
                    "Ok. ¿Qué más necesitas?".to_string()
                }
            }
            CategoriaIntencion::Negacion => {
                if self.estado == EstadoDialogo::Confirmando {
                    self.estado = EstadoDialogo::Esperando;
                    "Cancelado. ¿Algo más?".to_string()
                } else {
                    self.estado = EstadoDialogo::Esperando;
                    "Entendido. ¿Puedo ayudar con algo diferente?".to_string()
                }
            }
            CategoriaIntencion::Desconocido => {
                self.estado = EstadoDialogo::Clarificando;
                self.respuesta_plantilla("no_entiendo")
            }
            _ => {
                if ambigua {
                    self.estado = EstadoDialogo::Clarificando;
                    self.respuesta_plantilla("no_entiendo")
                } else {
                    self.estado = EstadoDialogo::Procesando;
                    format!("Procesando tu solicitud de '{}'...", intencion.nombre())
                }
            }
        }
    }

    /// Obtener últimos N turnos como contexto
    pub fn ultimos_turnos(&self, n: usize) -> &[Turno] {
        let inicio = self.turnos.len().saturating_sub(n);
        &self.turnos[inicio..]
    }

    /// Cambiar de tema
    pub fn cambiar_tema(&mut self, nuevo_tema: &str) {
        self.contexto.tema_actual = Some(nuevo_tema.to_string());
        self.contexto.turnos_en_tema = 0;
        self.contexto.preguntas_pendientes.clear();
    }

    pub fn total_turnos(&self) -> usize {
        self.turnos.len()
    }

    pub fn resumen(&self) {
        println!("  Conversación: {}", self.id);
        println!("  ────────────────────");
        println!("    Turnos: {}", self.turnos.len());
        let turnos_usuario = self.turnos.iter().filter(|t| t.rol == Rol::Usuario).count();
        let turnos_sistema = self.turnos.iter().filter(|t| t.rol == Rol::Sistema).count();
        println!(
            "    Usuario: {} | Sistema: {}",
            turnos_usuario, turnos_sistema
        );
        println!("    Estado: {}", self.estado.nombre());
        if let Some(tema) = &self.contexto.tema_actual {
            println!("    Tema: {}", tema);
        }
        println!(
            "    Sentimiento general: {:.2}",
            self.contexto.sentimiento_general
        );
        println!(
            "    Entidades: {}",
            self.contexto.entidades_acumuladas.len()
        );
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GestorConversaciones {
    pub conversaciones: Vec<Conversacion>,
    pub conversacion_activa: Option<String>,
    siguiente_id: usize,
}

impl GestorConversaciones {
    pub fn nuevo() -> Self {
        Self {
            conversaciones: Vec::new(),
            conversacion_activa: None,
            siguiente_id: 1,
        }
    }

    pub fn nueva_conversacion(&mut self) -> String {
        let id = format!("conv_{:04}", self.siguiente_id);
        self.siguiente_id += 1;
        let conv = Conversacion::nueva(&id);
        self.conversaciones.push(conv);
        self.conversacion_activa = Some(id.clone());
        id
    }

    pub fn activa(&self) -> Option<&Conversacion> {
        if let Some(id) = &self.conversacion_activa {
            self.conversaciones.iter().find(|c| c.id == *id)
        } else {
            None
        }
    }

    pub fn activa_mut(&mut self) -> Option<&mut Conversacion> {
        if let Some(id) = &self.conversacion_activa {
            let id = id.clone();
            self.conversaciones.iter_mut().find(|c| c.id == id)
        } else {
            None
        }
    }

    pub fn total(&self) -> usize {
        self.conversaciones.len()
    }
}

impl Default for GestorConversaciones {
    fn default() -> Self {
        Self::nuevo()
    }
}
