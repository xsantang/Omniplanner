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
            // ── Crear ────────────────────────────────────────────────
            ("crear nueva tarea para hoy", CategoriaIntencion::Crear),
            ("agregar evento a la agenda", CategoriaIntencion::Crear),
            ("quiero añadir un proyecto nuevo", CategoriaIntencion::Crear),
            ("nuevo canvas de planificacion", CategoriaIntencion::Crear),
            ("programar reunion para mañana", CategoriaIntencion::Crear),
            // ── Listar ───────────────────────────────────────────────
            ("mostrar todas mis tareas", CategoriaIntencion::Listar),
            ("ver la lista de pendientes", CategoriaIntencion::Listar),
            ("listar todos los proyectos", CategoriaIntencion::Listar),
            (
                "dame todas las tareas completadas",
                CategoriaIntencion::Listar,
            ),
            // ── Modificar ────────────────────────────────────────────
            ("editar la tarea del lunes", CategoriaIntencion::Modificar),
            ("cambiar la fecha del evento", CategoriaIntencion::Modificar),
            ("modificar la prioridad", CategoriaIntencion::Modificar),
            (
                "actualizar el titulo de la tarea",
                CategoriaIntencion::Modificar,
            ),
            // ── Eliminar ─────────────────────────────────────────────
            ("eliminar esa tarea", CategoriaIntencion::Eliminar),
            ("borrar el evento de ayer", CategoriaIntencion::Eliminar),
            ("quitar la tarea vieja", CategoriaIntencion::Eliminar),
            // ── Buscar ───────────────────────────────────────────────
            ("buscar tareas urgentes", CategoriaIntencion::Buscar),
            ("encontrar el proyecto de rust", CategoriaIntencion::Buscar),
            // ── Sociales ─────────────────────────────────────────────
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

    /// Banco de entrenamiento expandido — cobre miles de frases mediante
    /// expansión programática de plantillas con sinónimos y conjugaciones.
    /// Principio "10 millones de permutaciones": cada plantilla con sus slots
    /// genera combinaciones que el modelo aprende como un único intent.
    pub fn intenciones_expandidas() -> Vec<(String, CategoriaIntencion)> {
        let mut out: Vec<(String, CategoriaIntencion)> = Vec::new();

        // ── RegistrarGasto ────────────────────────────────────────────────
        let verbos_gasto = [
            "gaste",
            "pague",
            "compre",
            "desembolse",
            "erogue",
            "gaste un total de",
            "pague en total",
        ];
        let montos = [
            "50",
            "100",
            "200",
            "500",
            "1000",
            "50 pesos",
            "cien pesos",
            "200 dolares",
            "500 soles",
            "$50",
            "$100",
            "$200",
        ];
        let categorias_gasto = [
            "comida",
            "gasolina",
            "combustible",
            "renta",
            "luz",
            "agua",
            "internet",
            "telefono",
            "celular",
            "uber",
            "medicamentos",
            "farmacia",
            "ropa",
            "zapatos",
            "gym",
            "gimnasio",
            "netflix",
            "streaming",
            "supermercado",
            "mercado",
            "restaurante",
            "cafe",
            "desayuno",
            "almuerzo",
            "cena",
            "transporte",
            "taxi",
            "bus",
            "metro",
        ];
        let tiempos = [
            "hoy",
            "ayer",
            "esta manana",
            "anoche",
            "el lunes",
            "el martes",
            "esta semana",
            "el fin de semana",
            "hace un rato",
            "hace dos dias",
            "ayer por la tarde",
        ];
        for v in &verbos_gasto {
            for m in &montos {
                for c in &categorias_gasto {
                    out.push((
                        format!("{} {} en {}", v, m, c),
                        CategoriaIntencion::RegistrarGasto,
                    ));
                }
            }
        }
        for v in &verbos_gasto {
            for c in &categorias_gasto {
                for t in &tiempos {
                    out.push((
                        format!("{} en {} {}", v, c, t),
                        CategoriaIntencion::RegistrarGasto,
                    ));
                }
            }
        }
        let extras_gasto = [
            "registrar gasto de 300 en materiales",
            "anotar gasto de comida",
            "nuevo gasto transporte",
            "apuntar gasto de 80 en gasolina",
            "me costo 150 el mantenimiento del carro",
            "pague la renta este mes",
            "compre medicamentos por 200",
            "gaste en el supermercado hoy",
            "registra que gaste 500 en ropa",
            "anota 300 de gasto en herramientas",
        ];
        out.extend(
            extras_gasto
                .iter()
                .map(|s| (s.to_string(), CategoriaIntencion::RegistrarGasto)),
        );

        // ── RegistrarIngreso ──────────────────────────────────────────────
        let verbos_ingreso = [
            "recibi",
            "cobre",
            "me pagaron",
            "entre",
            "deposito",
            "llego dinero",
            "ingreso",
            "me deposito",
        ];
        let fuentes_ingreso = [
            "sueldo",
            "nomina",
            "salario",
            "pago de cliente",
            "trabajo freelance",
            "proyecto",
            "venta",
            "comision",
            "bono",
            "aguinaldo",
            "prestamo",
            "transferencia",
            "efectivo",
        ];
        for v in &verbos_ingreso {
            for f in &fuentes_ingreso {
                out.push((
                    format!("{} {} hoy", v, f),
                    CategoriaIntencion::RegistrarIngreso,
                ));
                out.push((
                    format!("{} por {} esta semana", v, f),
                    CategoriaIntencion::RegistrarIngreso,
                ));
            }
        }
        for v in &verbos_ingreso {
            for m in &montos {
                out.push((
                    format!("{} {} de ingreso", v, m),
                    CategoriaIntencion::RegistrarIngreso,
                ));
            }
        }
        let extras_ingreso = [
            "registrar ingreso mensual",
            "nuevo ingreso de sueldo",
            "me pago el cliente hoy",
            "cobre el proyecto esta semana",
            "recibi pago de 5000",
            "llego el deposito del trabajo",
            "me cayeron 3000 de la venta",
            "ingreso extra por freelance",
        ];
        out.extend(
            extras_ingreso
                .iter()
                .map(|s| (s.to_string(), CategoriaIntencion::RegistrarIngreso)),
        );

        // ── ConsultarGastos ───────────────────────────────────────────────
        let frases_consultar_gastos = [
            "cuanto gaste este mes",
            "cuanto llevo gastado",
            "mis gastos del mes",
            "ver gastos de hoy",
            "resumen de gastos",
            "gastos de esta semana",
            "en que gaste mas",
            "cuanto gaste en comida",
            "balance del mes",
            "cuanto va de gastos",
            "mostrar mis gastos",
            "cuanto llevo de gastos",
            "cuanto he gastado en total",
            "gastos por categoria",
            "desglose de gastos",
            "que tanto gaste esta semana",
            "cuanto gaste ayer",
            "gastos de hoy",
            "resumen gastos mensuales",
            "ver mis egresos",
            "cuanto en gasolina este mes",
            "cuanto gaste en comida esta semana",
            "total gastado",
            "mis egresos",
            "en que se va el dinero",
            "donde se va mi dinero",
            "mis gastos por categoria",
        ];
        out.extend(
            frases_consultar_gastos
                .iter()
                .map(|s| (s.to_string(), CategoriaIntencion::ConsultarGastos)),
        );

        // ── ConsultarDeudas ───────────────────────────────────────────────
        let frases_deudas = [
            "cuanto debo en total",
            "mis deudas",
            "ver mis deudas",
            "resumen de deudas",
            "que deudas tengo",
            "cuanto debo",
            "cuantas deudas tengo",
            "lista de deudas",
            "deudas activas",
            "deudas pendientes",
            "saldo de mis deudas",
            "total adeudado",
            "cuanto me falta pagar",
            "que debo pagar",
            "mis prestamos",
            "estado de mis deudas",
            "deuda con el banco",
            "cuanto debo al banco",
            "tarjeta de credito saldo",
            "saldo tarjeta",
            "deuda tarjeta",
            "cuanto debo de credito",
            "mis obligaciones financieras",
            "cuanto me queda por pagar",
            "resumen deudas pendientes",
            "creditos activos",
            "prestamos vigentes",
            "deudas vigentes",
        ];
        out.extend(
            frases_deudas
                .iter()
                .map(|s| (s.to_string(), CategoriaIntencion::ConsultarDeudas)),
        );

        // ── PedirSugerenciaPago ───────────────────────────────────────────
        let frases_sug_pago = [
            "que pago primero",
            "a que deuda abonar mas",
            "como pago mis deudas",
            "recomiendame un plan de pagos",
            "estrategia para pagar deudas",
            "metodo bola de nieve",
            "metodo avalancha deudas",
            "cual deuda pagar antes",
            "cuanto abonar a cada deuda",
            "plan de liquidacion de deudas",
            "como salir de deudas",
            "cual es la mejor estrategia para pagar",
            "que conviene pagar antes",
            "prioridad de pagos",
            "sugerencia para mis deudas",
            "como organizar mis pagos",
            "dame un plan de pagos",
            "que deuda tiene mas interes",
            "optimizar pagos de deudas",
            "reducir deudas rapidamente",
        ];
        out.extend(
            frases_sug_pago
                .iter()
                .map(|s| (s.to_string(), CategoriaIntencion::PedirSugerenciaPago)),
        );

        // ── ResumenFinanciero ─────────────────────────────────────────────
        let frases_resumen = [
            "como voy financieramente",
            "estado financiero",
            "resumen financiero",
            "situacion financiera",
            "como estan mis finanzas",
            "panorama de mis finanzas",
            "dashboard financiero",
            "como voy este mes",
            "resumen de todo",
            "estado general de mis finanzas",
            "como voy con el dinero",
            "cuanto tengo disponible",
            "saldo disponible",
            "balance general",
            "en que estoy parado",
            "cuanta plata me queda",
            "cuanto dinero tengo libre",
            "resumen de ingresos y gastos",
            "flujo de dinero",
            "mis ingresos vs gastos",
            "balance ingresos egresos",
            "como estan mis numeros",
            "estado de cuenta personal",
        ];
        out.extend(
            frases_resumen
                .iter()
                .map(|s| (s.to_string(), CategoriaIntencion::ResumenFinanciero)),
        );

        // ── AgendarPago ───────────────────────────────────────────────────
        let frases_agendar_pago = [
            "agendar pago de la luz el 15",
            "recordarme pagar el telefono",
            "programar pago de renta para el primero",
            "agenda el pago del internet el dia 20",
            "recordar pagar la tarjeta el 10",
            "agendar pago mensual del seguro",
            "poner recordatorio de pago",
            "programar el pago del prestamo",
            "que no se me olvide pagar el gas",
            "recordatorio para pagar renta",
        ];
        out.extend(
            frases_agendar_pago
                .iter()
                .map(|s| (s.to_string(), CategoriaIntencion::AgendarPago)),
        );

        // ── ConsultarTareas ───────────────────────────────────────────────
        let frases_tareas = [
            "que tareas tengo pendientes",
            "mis pendientes de hoy",
            "tareas para hoy",
            "que debo hacer hoy",
            "lista de tareas",
            "tareas vencidas",
            "tareas urgentes",
            "pendientes de esta semana",
            "que se me paso",
            "tareas atrasadas",
            "ver mis tareas",
            "cuantas tareas tengo",
            "tareas de alta prioridad",
            "que tengo para hacer",
            "mis actividades pendientes",
            "pendientes sin completar",
            "tareas sin terminar",
        ];
        out.extend(
            frases_tareas
                .iter()
                .map(|s| (s.to_string(), CategoriaIntencion::ConsultarTareas)),
        );

        // ── CrearTarea ────────────────────────────────────────────────────
        let frases_crear_tarea = [
            "agregar tarea estudiar para manana",
            "nueva tarea comprar víveres",
            "crear tarea revisar contrato",
            "anota que tengo que llamar al cliente",
            "recordar que hay que entregar el informe",
            "apunta pendiente hacer la declaracion",
            "tengo que ir al banco manana",
            "hay que comprar materiales para la obra",
            "crea pendiente enviar factura al cliente",
            "agregar recordatorio de cita medica",
        ];
        out.extend(
            frases_crear_tarea
                .iter()
                .map(|s| (s.to_string(), CategoriaIntencion::CrearTarea)),
        );

        // ── ConsultarObras ────────────────────────────────────────────────
        let frases_obras = [
            "como van mis obras",
            "estado de mis obras",
            "que obras tengo activas",
            "cuantas obras activas",
            "lista de obras",
            "mis proyectos de construccion",
            "obras en curso",
            "ver obras activas",
            "avance de mis obras",
            "porcentaje de avance de las obras",
            "en que etapa estan mis obras",
            "que proyectos tengo",
            "resumen de obras",
            "obras sin terminar",
            "cuantos proyectos activos tengo",
            "que obra esta mas avanzada",
            "estado de los proyectos",
            "obras pendientes de terminar",
            "progreso de las obras",
            "obras con retraso",
            "cuantos pasos completos tiene la obra",
            "resumen de proyectos activos",
        ];
        out.extend(
            frases_obras
                .iter()
                .map(|s| (s.to_string(), CategoriaIntencion::ConsultarObras)),
        );

        // ── SaldoObra ─────────────────────────────────────────────────────
        let frases_saldo_obra = [
            "cuanto saldo tiene la obra",
            "desembolsos de la obra",
            "cuanto dinero queda en el proyecto",
            "presupuesto de la obra",
            "cuanto hemos gastado en la obra",
            "cuanto llevamos gastado en construccion",
            "saldo disponible para la obra",
            "gastos de la obra",
            "cuanto dinero tiene la obra",
            "presupuesto restante obra",
            "cuanto se ha ejecutado del presupuesto",
            "cuanto falta ejecutar",
            "monto gastado en la obra",
            "ejecucion presupuestal obra",
            "saldo partidas presupuesto obra",
            "cuanto queda del presupuesto",
        ];
        out.extend(
            frases_saldo_obra
                .iter()
                .map(|s| (s.to_string(), CategoriaIntencion::SaldoObra)),
        );

        // ── AlertasObras ──────────────────────────────────────────────────
        let frases_alertas_obras = [
            "hay alertas en mis obras",
            "alguna obra con problemas",
            "obras retrasadas",
            "pasos vencidos en obras",
            "que obras tienen alerta",
            "problemas en mis proyectos",
            "obras con retraso",
            "alertas de construccion",
            "que paso esta vencido",
            "pasos atrasados en la obra",
            "alguna obra en riesgo",
            "obras que necesitan atencion",
            "alertas criticas de obras",
            "que obras estan retrasadas",
        ];
        out.extend(
            frases_alertas_obras
                .iter()
                .map(|s| (s.to_string(), CategoriaIntencion::AlertasObras)),
        );

        // ── ConsultarCobranzas ────────────────────────────────────────────
        let frases_cobranzas = [
            "que me deben",
            "cuanto me deben en total",
            "cuentas por cobrar",
            "clientes que deben",
            "facturas pendientes de cobro",
            "cartera por cobrar",
            "cobranzas pendientes",
            "cuanto tengo por cobrar",
            "quien me debe dinero",
            "cuentas sin cobrar",
            "facturas vencidas por cobrar",
            "cuantos clientes me deben",
            "total por cobrar",
            "resumen de cobranzas",
            "estado de cobranzas",
            "cuanto me adeudan en total",
            "deudores pendientes",
            "cobros pendientes",
            "que clientes tienen facturas vencidas",
            "cuanto deben mis clientes",
            "facturas sin pagar de clientes",
        ];
        out.extend(
            frases_cobranzas
                .iter()
                .map(|s| (s.to_string(), CategoriaIntencion::ConsultarCobranzas)),
        );

        // ── ResumenEmpresa ────────────────────────────────────────────────
        let frases_empresa = [
            "como va la empresa",
            "estado del negocio",
            "resumen empresarial",
            "dashboard empresa",
            "panorama del negocio",
            "como va mi negocio",
            "propuestas activas",
            "mis propuestas",
            "cuantas propuestas tengo",
            "casos abiertos",
            "mis casos activos",
            "proveedores registrados",
            "resumen de la empresa",
            "estado general del negocio",
            "que propuestas tengo vigentes",
            "cuantos contratos activos",
            "cartera de clientes activa",
            "negocios en curso",
            "proyectos empresariales",
            "mis contratos activos",
        ];
        out.extend(
            frases_empresa
                .iter()
                .map(|s| (s.to_string(), CategoriaIntencion::ResumenEmpresa)),
        );

        // ── GuiaSiguientePaso ─────────────────────────────────────────────
        let frases_guia = [
            "que sigue en la obra",
            "cual es el siguiente paso",
            "que debo hacer ahora en el proyecto",
            "que falta por hacer",
            "proxima actividad de la obra",
            "siguiente etapa del proyecto",
            "guia de siguiente paso",
            "que continua en la obra",
            "cual es la proxima tarea de la obra",
            "que actividad sigue",
            "donde quedamos en la obra",
            "siguiente hito del proyecto",
            "que paso viene despues",
            "que me falta completar",
            "actividades pendientes del proyecto",
            "en que paso estamos",
        ];
        out.extend(
            frases_guia
                .iter()
                .map(|s| (s.to_string(), CategoriaIntencion::GuiaSiguientePaso)),
        );

        // ── Ayuda ─────────────────────────────────────────────────────────
        let frases_ayuda = [
            "que puedo hacer",
            "como funciona el asistente",
            "ayuda con los comandos",
            "instrucciones de uso",
            "que comandos tienes",
            "que eres capaz de hacer",
            "para que sirves",
            "muéstrame las opciones",
            "no se que preguntar",
            "que sabes hacer",
        ];
        out.extend(
            frases_ayuda
                .iter()
                .map(|s| (s.to_string(), CategoriaIntencion::Ayuda)),
        );

        // ── Saludos y sociales ────────────────────────────────────────────
        let frases_saludo = [
            "hola buenos dias",
            "buenas tardes",
            "buenas noches",
            "que tal",
            "como estas",
            "saludos",
            "hey hola",
            "buen dia",
        ];
        out.extend(
            frases_saludo
                .iter()
                .map(|s| (s.to_string(), CategoriaIntencion::Saludo)),
        );

        out
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
        let mut motor = Self {
            sentimiento: AnalizadorSentimiento::nuevo(),
            intencion: ClasificadorIntencion::nuevo(),
            conocimiento: BaseConocimiento::nueva(),
            conversaciones: GestorConversaciones::nuevo(),
            feedback: SistemaFeedback::nuevo(),
            config: ConfigNLP::default(),
            estadisticas: EstadisticasMotor::default(),
        };
        // Auto-entrenar con los datos predefinidos para que los modelos
        // estén activos desde el primer uso, sin necesidad de entrenamiento manual.
        motor.entrenar_silencioso();
        motor
    }

    /// Entrenamiento silencioso (sin output) — llamado automáticamente en nuevo()
    /// y al cargar un estado sin modelos entrenados.
    pub fn entrenar_silencioso(&mut self) {
        let datos_sent = DatosEntrenamiento::sentimiento_es();
        self.sentimiento
            .entrenar_ml_silencioso(&datos_sent, 150, 0.05);
        // Combinar datos base (estáticos) con banco expandido (dinámico)
        let datos_base = DatosEntrenamiento::intenciones_es();
        let datos_base_refs: Vec<(&str, CategoriaIntencion)> =
            datos_base.iter().map(|(s, c)| (*s, c.clone())).collect();
        self.intencion
            .entrenar_silencioso(&datos_base_refs, 50, 0.1);
        // Segunda pasada con banco expandido para cubrir miles de frases
        let datos_expandidos = DatosEntrenamiento::intenciones_expandidas();
        let datos_exp_refs: Vec<(&str, CategoriaIntencion)> = datos_expandidos
            .iter()
            .map(|(s, c)| (s.as_str(), c.clone()))
            .collect();
        self.intencion
            .entrenar_silencioso(&datos_exp_refs, 20, 0.05);
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
        self.feedback.registrar(
            consulta,
            respuesta,
            valoracion.clone(),
            comentario,
            "general",
        );
        self.estadisticas.feedback_recibido += 1;

        // Aprendizaje por refuerzo: feedback positivo → confirma clasificación actual
        // y la añade al historial para reentrenamiento incremental.
        if matches!(valoracion, Valoracion::Buena | Valoracion::MuyBuena) {
            let intent = self.intencion.clasificar(consulta);
            if intent.confianza > 0.45 {
                self.intencion.registrar(consulta, intent.categoria);
                // Cada 5 ejemplos positivos nuevos, reentrenar incrementalmente
                if self.intencion.historial.len().is_multiple_of(5) {
                    self.intencion.reentrenar_incremental(10, 0.05);
                }
            }
            // Refuerzo de sentimiento: la consulta aprobada tiene sentimiento positivo
            let sent = self.sentimiento.analizar(consulta);
            let score_confirmado = sent.score;
            self.sentimiento
                .reentrenar_incremental(&[(consulta, score_confirmado)], 5, 0.02);
        }
    }

    /// Corregir la intención de una consulta pasada (aprendizaje supervisado).
    /// Llama a esto cuando el usuario indica cuál era la intención correcta.
    pub fn corregir_intencion(
        &mut self,
        consulta: &str,
        intent_correcto: super::intent::CategoriaIntencion,
    ) {
        self.intencion.registrar(consulta, intent_correcto);
        if self.intencion.historial.len().is_multiple_of(3) {
            self.intencion.reentrenar_incremental(15, 0.08);
        }
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
