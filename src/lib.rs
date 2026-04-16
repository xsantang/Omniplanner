//! # Omniplanner
//!
//! Aplicación CLI de productividad personal todo-en-uno escrita en Rust puro.
//!
//! ## Módulos
//!
//! | Módulo | Descripción |
//! |--------|-------------|
//! | [`tasks`] | Gestión de tareas con prioridades, etiquetas y follow-ups |
//! | [`agenda`] | Calendario, eventos recurrentes y horarios |
//! | [`canvas`] | Board visual de ideas, notas e imágenes |
//! | [`diagrams`] | Diagramas de flujo con export a Mermaid/pseudocódigo |
//! | [`mapper`] | Codificación/decodificación (Base64, Hex, Binario) |
//! | [`vcs`] | Control de versiones Git-like con SHA-256 |
//! | [`memoria`] | Sistema de memoria neuronal y búsqueda unificada |
//! | [`storage`] | Persistencia de estado a JSON |
//! | [`sync`] | Sincronización (Google Calendar, Drive, GitHub Gist, Email) |
//! | [`ml`] | Machine Learning: ANN, SVM, DNN, CNN, RNN, RL, k-fold CV |
//! | [`nlp`] | NLP: tokenización, sentimiento, intención, diálogos |

pub mod agenda;
pub mod canvas;
pub mod contrasenias;
pub mod diagrams;
pub mod mapper;
pub mod memoria;
pub mod ml;
pub mod nlp;
pub mod storage;
pub mod sync;
pub mod tasks;
pub mod vcs;
