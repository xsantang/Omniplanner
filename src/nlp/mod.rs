//! Procesamiento de Lenguaje Natural en español — pipeline completo.
//!
//! Tokenización, análisis de sentimiento, clasificación de intención,
//! base de conocimiento, gestión de diálogos multi-turno y feedback.

pub mod conversation;
pub mod engine;
pub mod feedback;
pub mod intent;
pub mod knowledge;
pub mod sentiment;
pub mod tokenizer;

// Re-exports principales
pub use conversation::{
    ContextoConversacion, Conversacion, EstadoDialogo, GestorConversaciones, Rol, Turno,
};
pub use engine::{ConfigNLP, DatosEntrenamiento, EstadisticasMotor, MotorNLP, ResultadoNLP};
pub use feedback::{EstadisticasFeedback, Feedback, SistemaFeedback, Valoracion};
pub use intent::{CategoriaIntencion, ClasificadorIntencion, Entidad, Intencion};
pub use knowledge::{BaseConocimiento, EntradaConocimiento, ResultadoBusqueda, TipoRelacion};
pub use sentiment::{AnalizadorSentimiento, Polaridad, ResultadoSentimiento};
pub use tokenizer::{Token, Tokenizer, WordEmbeddings};

use serde::{Deserialize, Serialize};

/// Estado persistente del módulo NLP
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlmacenNLP {
    pub motor: MotorNLP,
}

impl AlmacenNLP {
    pub fn nuevo() -> Self {
        Self {
            motor: MotorNLP::nuevo(),
        }
    }
}

impl Default for AlmacenNLP {
    fn default() -> Self {
        Self::nuevo()
    }
}

// ══════════════════════════════════════════════════════════
//  Tests
// ══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenizer_basico() {
        let tokens = Tokenizer::tokenizar("¡Hola mundo! Esto es una PRUEBA.");
        assert!(tokens.len() >= 4);
        assert_eq!(tokens[0].texto, "hola");
    }

    #[test]
    fn test_tokenizer_stopwords() {
        let limpios = Tokenizer::tokenizar_limpio("el gato de la casa");
        // "el", "de", "la" son stopwords
        assert!(limpios.contains(&"gato".to_string()));
        assert!(limpios.contains(&"casa".to_string()));
        assert!(!limpios.contains(&"el".to_string()));
    }

    #[test]
    fn test_tokenizer_stem() {
        // El stemmer español quita sufijos comunes
        let s1 = Tokenizer::stem("corriendo");
        assert!(s1.len() < "corriendo".len(), "Stem debería ser más corto");
        let s2 = Tokenizer::stem("jugando");
        assert!(s2.len() < "jugando".len());
        let s3 = Tokenizer::stem("felicidad");
        assert!(s3.len() < "felicidad".len());
    }

    #[test]
    fn test_tokenizer_ngrams() {
        let ngrams = Tokenizer::ngrams(
            &["hola".to_string(), "mundo".to_string(), "feliz".to_string()],
            2,
        );
        assert!(ngrams.contains(&"hola mundo".to_string()));
        assert!(ngrams.contains(&"mundo feliz".to_string()));
        assert_eq!(ngrams.len(), 2);
    }

    #[test]
    fn test_tokenizer_levenshtein() {
        assert_eq!(Tokenizer::levenshtein("gato", "gato"), 0);
        assert_eq!(Tokenizer::levenshtein("gato", "pato"), 1);
        assert_eq!(Tokenizer::levenshtein("", "abc"), 3);
    }

    #[test]
    fn test_sentimiento_positivo() {
        let analizador = AnalizadorSentimiento::nuevo();
        let res = analizador.analizar("Excelente trabajo, estoy muy feliz");
        assert!(
            res.score > 0.2,
            "Score positivo esperado, got {}",
            res.score
        );
        assert!(
            res.polaridad == Polaridad::Positivo || res.polaridad == Polaridad::MuyPositivo,
            "Polaridad positiva esperada"
        );
    }

    #[test]
    fn test_sentimiento_negativo() {
        let analizador = AnalizadorSentimiento::nuevo();
        let res = analizador.analizar("Terrible, todo es un desastre horrible");
        assert!(
            res.score < -0.2,
            "Score negativo esperado, got {}",
            res.score
        );
        assert!(
            res.polaridad == Polaridad::Negativo || res.polaridad == Polaridad::MuyNegativo,
            "Polaridad negativa esperada"
        );
    }

    #[test]
    fn test_sentimiento_negacion() {
        let analizador = AnalizadorSentimiento::nuevo();
        let res_pos = analizador.analizar("es bueno");
        let res_neg = analizador.analizar("no es bueno");
        // La negación debería reducir el score
        assert!(
            res_neg.score < res_pos.score,
            "Negación debería reducir score"
        );
    }

    #[test]
    fn test_sentimiento_emociones() {
        let analizador = AnalizadorSentimiento::nuevo();
        let res = analizador.analizar("estoy muy feliz y alegre hoy");
        assert!(!res.emociones.is_empty(), "Debería detectar emociones");
        assert!(
            res.emociones.contains_key("alegria"),
            "Debería detectar alegría"
        );
    }

    #[test]
    fn test_sentimiento_entrenamiento_ml() {
        let mut analizador = AnalizadorSentimiento::nuevo();
        let datos = vec![
            ("esto es genial increible", 0.9),
            ("muy malo terrible", -0.9),
            ("normal nada especial", 0.0),
        ];
        analizador.entrenar_ml(&datos, 50, 0.05);
        assert!(analizador.entrenado_ml);
    }

    #[test]
    fn test_intencion_crear() {
        let clasificador = ClasificadorIntencion::nuevo();
        let intent = clasificador.clasificar("crear nueva tarea urgente");
        assert_eq!(intent.categoria, CategoriaIntencion::Crear);
        assert!(intent.confianza > 0.0);
    }

    #[test]
    fn test_intencion_saludo() {
        let clasificador = ClasificadorIntencion::nuevo();
        let intent = clasificador.clasificar("hola buenos días");
        assert_eq!(intent.categoria, CategoriaIntencion::Saludo);
    }

    #[test]
    fn test_intencion_listar() {
        let clasificador = ClasificadorIntencion::nuevo();
        let intent = clasificador.clasificar("listar todas las tareas pendientes");
        assert_eq!(intent.categoria, CategoriaIntencion::Listar);
    }

    #[test]
    fn test_intencion_entidades() {
        let clasificador = ClasificadorIntencion::nuevo();
        let intent = clasificador.clasificar("crear tarea urgente para mañana");
        let tiene_fecha = intent.entidades.iter().any(|e| e.tipo == "fecha");
        let tiene_prioridad = intent.entidades.iter().any(|e| e.tipo == "prioridad");
        assert!(tiene_fecha || tiene_prioridad, "Debería detectar entidades");
    }

    #[test]
    fn test_intencion_ambigua() {
        let clasificador = ClasificadorIntencion::nuevo();
        let intent = clasificador.clasificar("xyz abc 123");
        assert!(
            clasificador.es_ambigua(&intent),
            "Input sin sentido debería ser ambiguo"
        );
    }

    #[test]
    fn test_intencion_entrenamiento() {
        let mut clasificador = ClasificadorIntencion::nuevo();
        let datos = vec![
            ("crear tarea", CategoriaIntencion::Crear),
            ("listar todo", CategoriaIntencion::Listar),
            ("hola", CategoriaIntencion::Saludo),
        ];
        clasificador.entrenar(&datos, 30, 0.1);
        assert!(clasificador.entrenado_ml);
    }

    #[test]
    fn test_knowledge_base_buscar() {
        let mut kb = BaseConocimiento::nueva();
        kb.agregar(
            "Rust",
            "Lenguaje de programación seguro y rápido",
            "Programación",
            &["rust".into(), "lenguaje".into()],
        );
        let resultados = kb.buscar("rust programación", 3);
        assert!(!resultados.is_empty(), "Debería encontrar resultado");
        assert!(resultados[0].entrada.titulo == "Rust");
    }

    #[test]
    fn test_knowledge_base_categorias() {
        let kb = BaseConocimiento::nueva();
        // La base viene con conocimiento precargado
        assert!(kb.total_entradas() > 0, "Debería tener entradas base");
        assert!(!kb.categorias.is_empty(), "Debería tener categorías");
    }

    #[test]
    fn test_knowledge_base_relaciones() {
        let mut kb = BaseConocimiento::nueva();
        let id1 = kb.agregar("A", "Concepto A", "Test", &[]);
        let id2 = kb.agregar("B", "Concepto B", "Test", &[]);
        kb.agregar_relacion(&id1, &id2, TipoRelacion::Relacionado, 0.8);
        let rel = kb.obtener_relacionadas(&id1);
        assert_eq!(rel.len(), 1);
    }

    #[test]
    fn test_conversacion_multi_turno() {
        let mut conv = Conversacion::nueva("test_conv");
        assert_eq!(conv.total_turnos(), 0);

        conv.turno_usuario(
            "hola",
            Some(CategoriaIntencion::Saludo),
            None,
            std::collections::HashMap::new(),
        );
        conv.turno_sistema("¡Hola! ¿En qué puedo ayudar?");
        assert_eq!(conv.total_turnos(), 2);

        conv.turno_usuario(
            "crear tarea",
            Some(CategoriaIntencion::Crear),
            None,
            std::collections::HashMap::new(),
        );
        conv.turno_sistema("¿Qué tarea quieres crear?");
        assert_eq!(conv.total_turnos(), 4);

        // Verificar contexto
        assert_eq!(
            conv.contexto.intencion_activa,
            Some(CategoriaIntencion::Crear)
        );
    }

    #[test]
    fn test_conversacion_sentimiento_general() {
        let mut conv = Conversacion::nueva("test_sent");
        conv.turno_usuario(
            "genial",
            None,
            Some(Polaridad::MuyPositivo),
            std::collections::HashMap::new(),
        );
        conv.turno_usuario(
            "horrible",
            None,
            Some(Polaridad::MuyNegativo),
            std::collections::HashMap::new(),
        );
        // Promedio de 1.0 y -1.0 = 0.0
        assert!((conv.contexto.sentimiento_general).abs() < 0.1);
    }

    #[test]
    fn test_conversacion_decidir_respuesta() {
        let mut conv = Conversacion::nueva("test_resp");
        let resp = conv.decidir_respuesta(&CategoriaIntencion::Saludo, false);
        assert!(!resp.is_empty());
        assert!(
            resp.contains("Hola")
                || resp.contains("Buenos")
                || resp.contains("Hey")
                || resp.contains("Bienvenido")
        );
    }

    #[test]
    fn test_feedback_basico() {
        let mut fb = SistemaFeedback::nuevo();
        fb.registrar("hola", "¡Hola!", Valoracion::Buena, None, "general");
        fb.registrar(
            "crear",
            "Error",
            Valoracion::Mala,
            Some("no funcionó".into()),
            "crear",
        );

        let stats = fb.estadisticas();
        assert_eq!(stats.total, 2);
    }

    #[test]
    fn test_feedback_ajustes() {
        let mut fb = SistemaFeedback::nuevo();
        // Varios feedbacks negativos para un componente
        for _ in 0..5 {
            fb.registrar("test", "resp", Valoracion::Mala, None, "componente_x");
        }
        assert!(
            fb.necesita_mejora("componente_x"),
            "Debería necesitar mejora"
        );

        // Varios positivos para otro
        for _ in 0..5 {
            fb.registrar("test", "resp", Valoracion::Buena, None, "componente_y");
        }
        assert!(
            !fb.necesita_mejora("componente_y"),
            "No debería necesitar mejora"
        );
    }

    #[test]
    fn test_motor_nlp_pipeline() {
        let mut motor = MotorNLP::nuevo();
        let resultado = motor.procesar("hola como estas");

        assert!(!resultado.respuesta.is_empty());
        // "hola como estas" contiene "como"→Consultar y "hola"→Saludo
        // Ambas son válidas; lo importante es que se procese
        assert!(!resultado.intencion.is_empty());
        assert!(!resultado.texto_original.is_empty());
    }

    #[test]
    fn test_motor_nlp_sentimiento_integrado() {
        let mut motor = MotorNLP::nuevo();
        let res = motor.procesar("estoy muy feliz con el resultado excelente");
        assert!(res.score_sentimiento > 0.0, "Debería ser positivo");
    }

    #[test]
    fn test_motor_nlp_conocimiento() {
        let mut motor = MotorNLP::nuevo();
        let res = motor.procesar("que es machine learning");
        // Debería encontrar algo en la KB sobre ML
        assert!(!res.respuesta.is_empty());
    }

    #[test]
    fn test_motor_nlp_conversacion_multi_turno() {
        let mut motor = MotorNLP::nuevo();
        motor.procesar("hola");
        motor.procesar("crear una tarea nueva");
        motor.procesar("si confirmo");

        assert_eq!(motor.estadisticas.consultas_procesadas, 3);
        assert!(motor.conversaciones.total() >= 1);
    }

    #[test]
    fn test_motor_nlp_feedback() {
        let mut motor = MotorNLP::nuevo();
        let res = motor.procesar("hola");
        motor.registrar_feedback("hola", &res.respuesta, Valoracion::Buena, None);
        assert_eq!(motor.estadisticas.feedback_recibido, 1);
    }

    #[test]
    fn test_motor_nlp_entrenamiento_completo() {
        let mut motor = MotorNLP::nuevo();
        motor.entrenar_completo();
        assert!(motor.sentimiento.entrenado_ml);
        assert!(motor.intencion.entrenado_ml);
    }

    #[test]
    fn test_word_embeddings() {
        let mut emb = WordEmbeddings::nuevo(10);
        let corpus = vec![
            "el gato come pescado",
            "el perro come carne",
            "el gato duerme mucho",
        ];
        emb.entrenar(&corpus, 2, 3, 0.05);
        assert!(!emb.vocab_index.is_empty());
        let vec = emb.vector("gato");
        assert!(vec.is_some());
    }

    #[test]
    fn test_tokenizer_tfidf() {
        let mut tok = Tokenizer::new();
        tok.entrenar_vocabulario(&["el gato come pescado", "el perro come carne"]);
        let tfidf = tok.tfidf("el gato come");
        assert!(!tfidf.is_empty());
    }

    #[test]
    fn test_similitud_coseno() {
        let sim = Tokenizer::similitud_coseno(&[1.0, 0.0, 1.0], &[1.0, 0.0, 1.0]);
        assert!((sim - 1.0).abs() < 0.01, "Vectores iguales → sim≈1.0");

        let sim2 = Tokenizer::similitud_coseno(&[1.0, 0.0], &[0.0, 1.0]);
        assert!(sim2.abs() < 0.01, "Vectores ortogonales → sim≈0.0");
    }

    #[test]
    fn test_jaccard() {
        let j = Tokenizer::jaccard(
            &["hola".to_string(), "mundo".to_string()],
            &["hola".to_string(), "mundo".to_string()],
        );
        assert!((j - 1.0).abs() < 0.01);

        let j2 = Tokenizer::jaccard(
            &["a".to_string(), "b".to_string()],
            &["c".to_string(), "d".to_string()],
        );
        assert!(j2.abs() < 0.01);
    }

    #[test]
    fn test_gestor_conversaciones() {
        let mut gestor = GestorConversaciones::nuevo();
        let id = gestor.nueva_conversacion();
        assert!(!id.is_empty());
        assert!(gestor.activa().is_some());
        assert_eq!(gestor.total(), 1);
    }
}
