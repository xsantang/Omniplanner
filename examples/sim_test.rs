use omniplanner::storage::AppState;

fn main() {
    let state = AppState::cargar().expect("Error cargando datos");
    let rast = &state.asesor.rastreador;

    println!("=== DEUDAS ===");
    for d in &rast.deudas {
        if !d.activa {
            continue;
        }
        let tipo = if d.es_pago_corriente() {
            "CORRIENTE"
        } else if d.es_pago_fijo() {
            "FIJO"
        } else {
            "DEUDA"
        };
        println!(
            "  {} | ${:.2} | {:.0}% | min ${} | {}",
            d.nombre,
            d.saldo_actual(),
            d.tasa_anual,
            d.pago_minimo,
            tipo
        );
    }

    let sim = rast.simular_libertad(3500.0, false);
    println!("\n=== AVALANCHA $3500/mes ===");
    println!(
        "Gastos fijos: ${:.2} ({})",
        sim.total_gastos_fijos,
        sim.gastos_fijos
            .iter()
            .map(|(n, m)| format!("{} ${}", n, m))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "Para deudas: ${:.2}\n",
        sim.presupuesto_mensual - sim.total_gastos_fijos
    );

    for mes in &sim.meses {
        let pagos_total: f64 = mes.pagos.iter().map(|(_, p)| *p).sum();
        println!(
            "--- MES {} --- (deuda: ${:.2}, pagos: ${:.2})",
            mes.mes_numero, mes.deuda_total, pagos_total
        );
        for (nombre, saldo) in &mes.saldos {
            let pago = mes
                .pagos
                .iter()
                .find(|(n, _)| n == nombre)
                .map(|(_, p)| *p)
                .unwrap_or(0.0);
            let interes = mes
                .intereses
                .iter()
                .find(|(n, _)| n == nombre)
                .map(|(_, i)| *i)
                .unwrap_or(0.0);
            if *saldo < 0.01 && pago < 0.01 {
                continue;
            }
            let liq = if mes.liquidadas_este_mes.contains(nombre) {
                " << LIQUIDADA"
            } else {
                ""
            };
            println!(
                "  {} | pago: ${:.2} | int: ${:.2} | saldo: ${:.2}{}",
                nombre, pago, interes, saldo, liq
            );
        }
    }
    println!(
        "\nTotal: {} meses | pagado: ${:.2} | intereses: ${:.2}",
        sim.meses.len(),
        sim.total_pagado,
        sim.total_intereses
    );
    for (n, m) in &sim.orden_liquidacion {
        println!("  {} -> mes {}", n, m);
    }
}
