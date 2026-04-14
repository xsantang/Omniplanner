use omniplanner::ml::SimulacionLibertad;
use omniplanner::storage::AppState;
use rust_xlsxwriter::{Format, FormatAlign, FormatBorder, Workbook};

fn main() {
    let state = AppState::cargar().expect("Error cargando datos");
    let rast = &state.asesor.rastreador;

    let sim = rast.simular_libertad(3500.0, false);
    let nombres: Vec<String> = if let Some(m) = sim.meses.first() {
        m.saldos.iter().map(|(n, _)| n.clone()).collect()
    } else {
        vec![]
    };

    println!("Simulacion {} meses, exportando...", sim.meses.len());
    match exportar_test(&sim, &nombres) {
        Ok(ruta) => println!("Excel guardado en: {}", ruta),
        Err(e) => println!("Error: {}", e),
    }
}

fn exportar_test(sim: &SimulacionLibertad, nombres: &[String]) -> Result<String, String> {
    let carpeta = dirs::document_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("OmniPlanner");
    std::fs::create_dir_all(&carpeta).map_err(|e| e.to_string())?;
    let archivo = carpeta.join("test_export.xlsx");

    let mut wb = Workbook::new();
    let fmt_header = Format::new().set_bold().set_border(FormatBorder::Thin)
        .set_background_color("4472C4").set_font_color("FFFFFF");
    let fmt_dinero = Format::new().set_num_format("$#,##0.00").set_border(FormatBorder::Thin);

    let ws = wb.add_worksheet();
    ws.set_name("Amortización").map_err(|e| e.to_string())?;
    ws.write_string_with_format(0, 0, "Deuda", &fmt_header).map_err(|e| e.to_string())?;
    ws.write_string_with_format(0, 1, "Pago", &fmt_header).map_err(|e| e.to_string())?;
    ws.write_string_with_format(0, 2, "Interés", &fmt_header).map_err(|e| e.to_string())?;
    ws.write_string_with_format(0, 3, "Saldo", &fmt_header).map_err(|e| e.to_string())?;

    let mut row = 1u32;
    for mes in &sim.meses {
        for (nombre, saldo) in &mes.saldos {
            let pago = mes.pagos.iter().find(|(n,_)| n == nombre).map(|(_,p)| *p).unwrap_or(0.0);
            let interes = mes.intereses.iter().find(|(n,_)| n == nombre).map(|(_,i)| *i).unwrap_or(0.0);
            ws.write_string(row, 0, &format!("M{} {}", mes.mes_numero, nombre)).map_err(|e| e.to_string())?;
            ws.write_number_with_format(row, 1, pago, &fmt_dinero).map_err(|e| e.to_string())?;
            ws.write_number_with_format(row, 2, interes, &fmt_dinero).map_err(|e| e.to_string())?;
            ws.write_number_with_format(row, 3, *saldo, &fmt_dinero).map_err(|e| e.to_string())?;
            row += 1;
        }
    }

    ws.set_column_width(0, 25).map_err(|e| e.to_string())?;
    ws.set_column_width(1, 14).map_err(|e| e.to_string())?;
    ws.set_column_width(2, 14).map_err(|e| e.to_string())?;
    ws.set_column_width(3, 14).map_err(|e| e.to_string())?;

    wb.save(&archivo).map_err(|e| e.to_string())?;
    Ok(archivo.to_string_lossy().to_string())
}
