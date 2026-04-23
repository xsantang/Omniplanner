# omniplanner

Suite todo-en-uno en **Rust** para productividad personal y finanzas: tareas,
agenda, diagramas, gestor de contraseñas y un **asesor financiero con
rastreador de deudas** que incluye planificador editable de libertad
financiera.

Un único crate compila a tres objetivos:

| Objetivo | Feature      | Artefacto                         |
|----------|--------------|-----------------------------------|
| Desktop  | `desktop`    | Binario CLI `omniplanner`         |
| Android  | `android`    | `libomniplanner.so` (JNI)         |
| Web      | `web`        | `omniplanner.wasm` (wasm-bindgen) |

---

## Requisitos

- Rust stable (edición 2021). Instalable con [rustup](https://rustup.rs/).
- Para WASM: `rustup target add wasm32-unknown-unknown`.
- Para Android: toolchain NDK configurada (fuera del alcance de este README).

## Compilación

```powershell
# Desktop (por defecto, incluye el binario CLI):
cargo build --release

# Ejecutar la CLI:
cargo run --release

# Sólo la librería para Android:
cargo build --release --no-default-features --features android

# WebAssembly:
cargo build --release --target wasm32-unknown-unknown `
    --no-default-features --features web
```

## Tests

```powershell
cargo test                      # lib + integración
cargo test --test advisor_rules # sólo reglas del asesor
```

Chequeos rápidos en los tres objetivos:

```powershell
cargo clippy --all-targets -- -D warnings
cargo clippy --lib --no-default-features --features android -- -D warnings
cargo clippy --lib --target wasm32-unknown-unknown `
    --no-default-features --features web -- -D warnings
```

---

## Arquitectura

```
src/
├── main.rs          CLI (sólo con feature "desktop")
├── lib.rs           Punto de entrada de la librería
├── cli/
│   └── rastreador.rs  Subcomandos CLI del rastreador de deudas
├── ml/
│   ├── mod.rs
│   └── advisor.rs     Dominio financiero: deudas, planes, simulaciones
├── contrasenias.rs    Gestor de contraseñas + generación CSPRNG
├── cripto.rs          Cripto simétrica (AES-256-GCM) y asimétrica (Ed25519, X25519, RSA-4096)
├── storage.rs         Persistencia JSON
├── ffi.rs             Bindings JNI (Android)
└── wasm.rs            Bindings wasm-bindgen (Web)
tests/
└── advisor_rules.rs   Tests de integración de reglas del asesor
```

### Módulo `ml::advisor`

El cerebro financiero del proyecto. Tipos destacados:

- **`DeudaRastreada`** — deuda individual con historial mensual, métodos
  `evaluar_pago_mes`, `estado_ui`, `simular_liquidacion`, `ahorro_por_pago_extra`.
- **`DecisionPago`** — enum con las 4 políticas del motor de reglas:
  `Aceptar`, `AceptarConAviso`, `PedirDobleConfirmacion`, `Bloquear`.
- **`EstadoDeudaUi`** — estado visual: `Liquidada`, `AlDia`,
  `EnTrampaIntereses`, `Vencida{monto_vencido}`.
- **`EstrategiaLibertad`** — `Avalancha` (mayor tasa primero),
  `BolaNieve` (menor saldo primero), `Pesos(...)` (reparto personalizado).
- **`AjusteMensualLibertad`** — sobreescritura manual de un pago
  `(mes, nombre_deuda, pago_forzado)`.
- **`SimulacionLibertad`** + **`ComparacionPlanes`** — ejecución del plan y
  comparador de dos planes lado a lado.

Flujo del planificador editable de libertad financiera
(`rastreador_simulacion_libertad` en [src/cli/rastreador.rs](src/cli/rastreador.rs)):

1. Se genera una simulación inicial (estrategia avalancha o bola de nieve).
2. El usuario entra al editor y puede: ver tabla mes×deuda, cambiar
   estrategia, mover recursos entre deudas en un mes concreto, fijar un
   pago específico, limpiar ajustes o comparar el plan editado contra el
   original.
3. El plan resultante se exporta a Excel.

### Módulo `contrasenias`

- Generación de contraseñas y frases semilla usando **`getrandom`** (CSPRNG
  del sistema operativo: `BCryptGenRandom` en Windows, `getrandom(2)` en
  Linux, `arc4random` en macOS/BSD, `crypto.getRandomValues` en navegadores).
- Fallback determinista (xorshift64) sólo si `getrandom` falla.
- Evaluador de fortaleza (0–100) con sugerencias.
- **Aviso de seguridad**: las entradas se guardan en texto plano. Para
  cifrado en reposo usa el módulo [`cripto`](#módulo-cripto) con
  `ClavePrivadaSellada` o un `SobreAesGcm` sobre el JSON.

### Módulo `cripto`

API de alto nivel sobre crates auditados:

| Primitiva            | Crate          | Uso recomendado                                |
|----------------------|----------------|------------------------------------------------|
| AES-256-GCM          | `aes-gcm`      | Cifrado simétrico autenticado                   |
| Argon2id             | `argon2`       | Derivar clave desde contraseña maestra          |
| Ed25519              | `ed25519-dalek`| Firma digital rápida (32 B clave, 64 B firma)   |
| X25519 ECDH + HKDF   | `x25519-dalek` | Intercambio → clave AES-256 compartida          |
| RSA-4096 (OAEP/PSS)  | `rsa`          | Interop legacy; cifrado y firma                 |
| Sobre híbrido RSA+AES| —              | Cifrar mensajes largos con clave pública RSA    |

Ejemplo — cifrar un archivo con contraseña maestra:

```rust
use omniplanner::cripto::{cifrar_aes_gcm, derivar_clave_maestra, ParamsKdf};

let contrasenia = b"mi frase super secreta";
let (clave_vec, salt) = derivar_clave_maestra(contrasenia, None, &ParamsKdf::default())?;
let mut clave = [0u8; 32];
clave.copy_from_slice(&clave_vec);
let sobre = cifrar_aes_gcm(&clave, b"datos sensibles")?;
// Persistir: salt + sobre.nonce_b64 + sobre.ct_b64
```

Todas las claves privadas pueden sellarse con
[`ClavePrivadaSellada::sellar`] (Argon2id + AES-GCM) antes de
persistirlas en disco.

---

## Uso rápido (CLI)

```powershell
cargo run --release
```

El menú principal expone los módulos: tareas, agenda, contraseñas,
asesor financiero, etc. Dentro del asesor, `Rastreador de deudas`
habilita el flujo de registro mensual y el planificador editable.

## Licencia

Sin licencia pública definida. Uso interno.
