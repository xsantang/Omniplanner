//! Tests de integración de las reglas del asesor financiero.
//!
//! Cubren escenarios realistas (multi-deuda, tramos de atraso, tasas altas,
//! pagos parciales y el planificador editable de libertad) sobre la API
//! pública reexportada en `omniplanner::ml::*`.

use omniplanner::ml::{
    AjusteMensualLibertad, DecisionPago, DeudaRastreada, EstadoDeudaUi, EstrategiaLibertad,
    MesPago, RastreadorDeudas,
};

// ──────────────────────────────────────────────────────────────
//  Helpers
// ──────────────────────────────────────────────────────────────

/// Crea una deuda con un saldo inicial sembrado en el historial.
/// Permite que `saldo_actual()` devuelva el valor deseado sin tener
/// que llamar `registrar_mes` repetidamente.
fn deuda(nombre: &str, saldo: f64, tasa_anual: f64, pago_min: f64) -> DeudaRastreada {
    let mut d = DeudaRastreada::nueva(nombre, tasa_anual, pago_min);
    d.historial.push(MesPago {
        mes: "2026-01".into(),
        saldo_inicio: saldo,
        pago: 0.0,
        pago_escrow: 0.0,
        nuevos_cargos: 0.0,
        intereses: 0.0,
        saldo_final: saldo,
    });
    d.activa = saldo > 0.01;
    d
}

// ──────────────────────────────────────────────────────────────
//  DecisionPago — reglas de validación de pago
// ──────────────────────────────────────────────────────────────

#[test]
fn pago_negativo_se_bloquea() {
    let d = deuda("Visa", 1000.0, 24.0, 60.0);
    let decision = d.evaluar_pago_mes(-10.0, 0.0, 1000.0);
    assert!(matches!(decision, DecisionPago::Bloquear(_)));
    assert!(decision.esta_bloqueado());
}

#[test]
fn pago_nan_se_bloquea() {
    let d = deuda("Visa", 1000.0, 24.0, 60.0);
    let decision = d.evaluar_pago_mes(f64::NAN, 0.0, 1000.0);
    assert!(matches!(decision, DecisionPago::Bloquear(_)));
}

#[test]
fn pago_mayor_10x_exigible_pide_doble_confirmacion() {
    let d = deuda("Visa", 2000.0, 24.0, 60.0);
    // 10× el pago mínimo (60) = 600. Un pago de 1500 supera el umbral.
    let decision = d.evaluar_pago_mes(1500.0, 0.0, 2000.0);
    assert!(decision.requiere_confirmacion());
}

#[test]
fn pago_normal_se_acepta_sin_alertas() {
    let d = deuda("Visa", 2000.0, 24.0, 60.0);
    // Pago igual al mínimo exigible + un poco más: caso normal.
    let decision = d.evaluar_pago_mes(80.0, 0.0, 2000.0);
    assert!(decision.es_aceptado());
    assert!(!decision.requiere_confirmacion());
}

#[test]
fn decision_bloqueada_no_es_aceptada() {
    let d = deuda("Visa", 1000.0, 24.0, 60.0);
    let decision = d.evaluar_pago_mes(f64::NAN, 0.0, 1000.0);
    assert!(!decision.es_aceptado());
    assert!(!decision.mensaje().is_empty());
}

// ──────────────────────────────────────────────────────────────
//  EstadoDeudaUi — clasificación visual
// ──────────────────────────────────────────────────────────────

#[test]
fn deuda_liquidada_reporta_estado_liquidada() {
    let mut d = deuda("Visa", 0.0, 24.0, 60.0);
    d.activa = false;
    assert_eq!(d.estado_ui(), EstadoDeudaUi::Liquidada);
}

// ──────────────────────────────────────────────────────────────
//  Ahorro por pago extra y ranking
// ──────────────────────────────────────────────────────────────

#[test]
fn pago_extra_mayor_da_mayor_ahorro() {
    let d = deuda("Visa", 3000.0, 24.0, 100.0);
    let a_50 = d.ahorro_por_pago_extra(50.0).expect("ahorro 50");
    let a_150 = d.ahorro_por_pago_extra(150.0).expect("ahorro 150");
    assert!(a_150.intereses_ahorrados > a_50.intereses_ahorrados);
    assert!(a_150.meses_ahorrados >= a_50.meses_ahorrados);
}

#[test]
fn ranking_ordena_por_ahorro_descendente() {
    let mut r = RastreadorDeudas::default();
    r.agregar_deuda(deuda("Hipoteca", 5000.0, 6.0, 100.0));
    r.agregar_deuda(deuda("Tarjeta", 3000.0, 28.0, 90.0));
    r.agregar_deuda(deuda("Auto", 8000.0, 10.0, 200.0));

    let rec = r.mejor_destino_pago_extra(100.0).expect("rec");
    // La primera del ranking es la recomendación.
    assert_eq!(rec.ranking[0].0, rec.nombre_deuda);
    // El ranking está ordenado descendente por intereses ahorrados.
    for pair in rec.ranking.windows(2) {
        assert!(pair[0].1.intereses_ahorrados >= pair[1].1.intereses_ahorrados);
    }
}

#[test]
fn tarjeta_tasa_alta_gana_sobre_hipoteca_tasa_baja() {
    let mut r = RastreadorDeudas::default();
    r.agregar_deuda(deuda("Hipoteca", 5000.0, 6.0, 100.0));
    r.agregar_deuda(deuda("Tarjeta", 3000.0, 28.0, 90.0));

    let rec = r.mejor_destino_pago_extra(100.0).expect("rec");
    assert_eq!(rec.nombre_deuda, "Tarjeta");
}

// ──────────────────────────────────────────────────────────────
//  Planificador editable de libertad
// ──────────────────────────────────────────────────────────────

fn tres_tarjetas() -> RastreadorDeudas {
    let mut r = RastreadorDeudas::default();
    r.agregar_deuda(deuda("Visa", 2500.0, 24.0, 70.0));
    r.agregar_deuda(deuda("Amex", 1500.0, 20.0, 50.0));
    r.agregar_deuda(deuda("Disc", 800.0, 18.0, 35.0));
    r
}

#[test]
fn avalancha_vs_bola_nieve_ambos_liquidan() {
    let r = tres_tarjetas();
    let aval = r.simular_libertad(400.0, false);
    let bola = r.simular_libertad(400.0, true);
    assert!(!aval.meses.is_empty());
    assert!(!bola.meses.is_empty());
    // Ambos deben dejar deuda total < 0.01 al final.
    assert!(aval.meses.last().unwrap().deuda_total < 0.01);
    assert!(bola.meses.last().unwrap().deuda_total < 0.01);
}

#[test]
fn avalancha_suele_ahorrar_mas_intereses() {
    let r = tres_tarjetas();
    let aval = r.simular_libertad(400.0, false);
    let bola = r.simular_libertad(400.0, true);
    // Avalancha prioriza tasa alta → total_intereses <= bola de nieve.
    assert!(aval.total_intereses <= bola.total_intereses + 0.01);
}

#[test]
fn pesos_personalizados_liquidan_y_reportan_nombre() {
    let r = tres_tarjetas();
    let estrategia = EstrategiaLibertad::pesos_normalizados(vec![
        ("Visa".into(), 2.0),
        ("Amex".into(), 1.0),
        ("Disc".into(), 1.0),
    ]);
    let sim = r.simular_libertad_editado(400.0, &estrategia, &[]);
    assert_eq!(sim.estrategia, "Pesos personalizados");
    assert!(!sim.meses.is_empty());
    assert!(sim.meses.last().unwrap().deuda_total < 0.01);
}

#[test]
fn ajuste_manual_se_respeta_y_sobrante_se_redirige() {
    let r = tres_tarjetas();
    // Forzar Visa a recibir solo el mínimo (70) en el mes 1,
    // el sobrante debería ir a Amex/Disc según avalancha.
    let ajustes = vec![AjusteMensualLibertad::nuevo(1, "Visa", 70.0)];
    let sim = r.simular_libertad_editado(400.0, &EstrategiaLibertad::Avalancha, &ajustes);
    let mes1 = &sim.meses[0];
    let pago_visa = mes1.pagos.iter().find(|(n, _)| n == "Visa").unwrap().1;
    assert!((pago_visa - 70.0).abs() < 0.01);
    let total_mes1: f64 = mes1.pagos.iter().map(|(_, p)| *p).sum();
    // Se gastó (casi) todo el presupuesto: sobrante pequeño.
    assert!(total_mes1 > 350.0);
}

#[test]
fn comparacion_planes_expone_metricas_coherentes() {
    let r = tres_tarjetas();
    let base = r.simular_libertad(400.0, false);
    let alt = r.simular_libertad_editado(400.0, &EstrategiaLibertad::BolaNieve, &[]);
    let cmp = RastreadorDeudas::comparar_planes(&base, &alt);
    assert_eq!(cmp.meses_base, base.meses.len());
    assert_eq!(cmp.meses_alternativa, alt.meses.len());
    // La diferencia de meses es consistente con los valores crudos.
    assert_eq!(
        cmp.diferencia_meses,
        (alt.meses.len() as i32) - (base.meses.len() as i32)
    );
}

#[test]
fn presupuesto_insuficiente_no_liquida_pero_no_panica() {
    let r = tres_tarjetas();
    // $100 < suma de mínimos (70+50+35=155).
    let sim = r.simular_libertad(100.0, false);
    // O se queda sin terminar (<600 meses) o termina con deuda > 0.
    if let Some(ultimo) = sim.meses.last() {
        // Si terminó al límite, o sigue con saldo, no debe panicar.
        assert!(ultimo.deuda_total >= 0.0);
    }
}

#[test]
fn pago_extra_a_deuda_sin_saldo_da_none() {
    let d = deuda("Pagada", 0.0, 24.0, 60.0);
    assert!(d.ahorro_por_pago_extra(100.0).is_none());
}
