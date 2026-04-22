package com.omniplanner.app.screens

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Payment
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.omniplanner.app.OmniBridge
import kotlinx.serialization.json.*

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PresupuestoScreen() {
    var tabIndex by remember { mutableIntStateOf(0) }
    val tabs = listOf("Presupuesto", "Deudas")

    Column {
        TabRow(selectedTabIndex = tabIndex) {
            tabs.forEachIndexed { i, title ->
                Tab(selected = tabIndex == i, onClick = { tabIndex = i },
                    text = { Text(title) })
            }
        }
        when (tabIndex) {
            0 -> PresupuestoTab()
            1 -> DeudasTab()
        }
    }
}

// ═══════════════════════════════════════════════════════════
//  Tab 1: Presupuesto Base Cero
// ═══════════════════════════════════════════════════════════

@Composable
fun PresupuestoTab() {
    var meses by remember { mutableStateOf<List<JsonObject>>(emptyList()) }
    var mesSeleccionado by remember { mutableStateOf<String?>(null) }
    var lineas by remember { mutableStateOf<List<JsonObject>>(emptyList()) }
    var showDialog by remember { mutableStateOf(false) }
    var editLinea by remember { mutableStateOf<JsonObject?>(null) }

    fun cargarResumen() {
        val r = OmniBridge.presupuestoResumen()
        if (r.ok) {
            meses = r.data?.jsonObject?.get("meses")?.jsonArray?.map { it.jsonObject } ?: emptyList()
        }
    }

    fun cargarDetalle(mes: String) {
        val r = OmniBridge.presupuestoDetalle(mes)
        if (r.ok) {
            lineas = r.data?.jsonObject?.get("lineas")?.jsonArray?.map { it.jsonObject } ?: emptyList()
        }
    }

    LaunchedEffect(Unit) { cargarResumen() }

    Scaffold(
        floatingActionButton = {
            FloatingActionButton(onClick = { editLinea = null; showDialog = true }) {
                Icon(Icons.Default.Add, "Agregar")
            }
        }
    ) { padding ->
        Column(modifier = Modifier.padding(padding).padding(16.dp)) {
            if (mesSeleccionado != null) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    IconButton(onClick = {
                        mesSeleccionado = null; lineas = emptyList(); cargarResumen()
                    }) { Icon(Icons.Default.ArrowBack, "Volver") }
                    Text("Detalle: $mesSeleccionado", style = MaterialTheme.typography.headlineSmall)
                }
                Spacer(Modifier.height(8.dp))
                if (lineas.isEmpty()) {
                    Text("Sin movimientos este mes", style = MaterialTheme.typography.bodyLarge)
                }
                LazyColumn(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    itemsIndexed(lineas) { _, linea ->
                        val nombre = linea["nombre"]?.jsonPrimitive?.content ?: ""
                        val monto = linea["monto"]?.jsonPrimitive?.double ?: 0.0
                        val cat = linea["categoria"]?.jsonPrimitive?.content ?: ""
                        val indice = linea["indice"]?.jsonPrimitive?.int ?: 0
                        val pagado = linea["pagado"]?.jsonPrimitive?.boolean ?: false
                        val fechaLimite = linea["fecha_limite"]?.jsonPrimitive?.content ?: ""
                        val notas = linea["notas"]?.jsonPrimitive?.content ?: ""
                        val saldoDeuda = linea["saldo_total_deuda"]?.jsonPrimitive?.doubleOrNull
                        val esIngreso = cat == "ingreso"

                        Card(modifier = Modifier.fillMaxWidth()) {
                            Column(modifier = Modifier.padding(12.dp)) {
                                Row(verticalAlignment = Alignment.CenterVertically) {
                                    Column(modifier = Modifier.weight(1f)) {
                                        Text(nombre, style = MaterialTheme.typography.titleSmall)
                                        Text(cat.replace("_", " ").replaceFirstChar { it.uppercase() },
                                            style = MaterialTheme.typography.labelSmall,
                                            color = MaterialTheme.colorScheme.onSurfaceVariant)
                                    }
                                    Text("${if (esIngreso) "+" else "-"}$${"%.2f".format(monto)}",
                                        style = MaterialTheme.typography.titleSmall,
                                        color = if (esIngreso) MaterialTheme.colorScheme.primary
                                                else MaterialTheme.colorScheme.error)
                                    IconButton(onClick = { editLinea = linea; showDialog = true }) {
                                        Icon(Icons.Default.Edit, "Editar")
                                    }
                                    IconButton(onClick = {
                                        OmniBridge.presupuestoEliminarLinea(mesSeleccionado!!, indice)
                                        cargarDetalle(mesSeleccionado!!)
                                    }) {
                                        Icon(Icons.Default.Delete, "Eliminar",
                                            tint = MaterialTheme.colorScheme.error)
                                    }
                                }
                                if (pagado) Text("\u2705 Pagado", style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.primary)
                                if (fechaLimite.isNotBlank()) Text("\uD83D\uDCC5 Fecha l\u00edmite: $fechaLimite",
                                    style = MaterialTheme.typography.labelSmall)
                                if (saldoDeuda != null) Text("\uD83D\uDCB3 Saldo deuda: $${"%.2f".format(saldoDeuda)}",
                                    style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.error)
                                if (notas.isNotBlank()) Text("\uD83D\uDCDD $notas",
                                    style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                    }
                }
            } else {
                Text("Presupuesto", style = MaterialTheme.typography.headlineMedium)
                Spacer(Modifier.height(12.dp))
                if (meses.isEmpty()) {
                    Text("Sin datos. Toca + para agregar.", style = MaterialTheme.typography.bodyLarge)
                }
                LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(meses) { mes ->
                        val label = mes["mes"]?.jsonPrimitive?.content ?: ""
                        val ingresos = mes["ingresos"]?.jsonPrimitive?.double ?: 0.0
                        val gastos = mes["gastos"]?.jsonPrimitive?.double ?: 0.0
                        val balance = mes["balance"]?.jsonPrimitive?.double ?: 0.0
                        Card(modifier = Modifier.fillMaxWidth().clickable {
                            mesSeleccionado = label; cargarDetalle(label)
                        }) {
                            Column(modifier = Modifier.padding(16.dp)) {
                                Text(label, style = MaterialTheme.typography.titleMedium)
                                Spacer(Modifier.height(4.dp))
                                Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                                    Text("Ingresos: $${"%.2f".format(ingresos)}",
                                        color = MaterialTheme.colorScheme.primary)
                                    Text("Gastos: $${"%.2f".format(gastos)}",
                                        color = MaterialTheme.colorScheme.error)
                                }
                                Text("Balance: $${"%.2f".format(balance)}",
                                    style = MaterialTheme.typography.titleSmall,
                                    color = if (balance >= 0) MaterialTheme.colorScheme.primary
                                            else MaterialTheme.colorScheme.error)
                                Text("Toca para ver detalle \u2192",
                                    style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                    }
                }
            }
        }
    }

    if (showDialog) {
        GastoDialog(
            editLinea = editLinea,
            onDismiss = { showDialog = false; editLinea = null },
            onSave = { nombre, monto, categoria, pagado, fechaLimite, notas, saldoDeuda ->
                if (editLinea != null && mesSeleccionado != null) {
                    val idx = editLinea!!["indice"]?.jsonPrimitive?.int ?: 0
                    OmniBridge.presupuestoActualizarLinea(
                        mes = mesSeleccionado!!, indice = idx,
                        nombre = nombre, monto = monto, categoria = categoria,
                        pagado = pagado, fechaLimite = fechaLimite, notas = notas,
                        saldoTotalDeuda = saldoDeuda)
                } else {
                    OmniBridge.presupuestoAgregar(nombre = nombre, monto = monto,
                        categoria = categoria, pagado = pagado, fechaLimite = fechaLimite,
                        notas = notas, saldoTotalDeuda = saldoDeuda)
                }
                if (mesSeleccionado != null) cargarDetalle(mesSeleccionado!!)
                cargarResumen()
                showDialog = false; editLinea = null
            }
        )
    }
}

// ═══════════════════════════════════════════════════════════
//  Tab 2: Rastreador de Deudas
// ═══════════════════════════════════════════════════════════

@Composable
fun DeudasTab() {
    var deudas by remember { mutableStateOf<List<JsonObject>>(emptyList()) }
    var ingresos by remember { mutableStateOf<List<JsonObject>>(emptyList()) }
    var ingresoMensual by remember { mutableDoubleStateOf(0.0) }
    var deudaTotal by remember { mutableDoubleStateOf(0.0) }

    var showDeudaDialog by remember { mutableStateOf(false) }
    var editDeuda by remember { mutableStateOf<JsonObject?>(null) }
    var showIngresoDialog by remember { mutableStateOf(false) }
    var pagoDeuda by remember { mutableStateOf<JsonObject?>(null) }

    // Qué estamos agregando: "deuda" o "ingreso"
    var addMenuExpanded by remember { mutableStateOf(false) }

    fun cargar() {
        val r = OmniBridge.deudasListar()
        if (r.ok) {
            val data = r.data?.jsonObject
            deudas = data?.get("deudas")?.jsonArray?.map { it.jsonObject } ?: emptyList()
            ingresos = data?.get("ingresos")?.jsonArray?.map { it.jsonObject } ?: emptyList()
            ingresoMensual = data?.get("ingreso_mensual")?.jsonPrimitive?.double ?: 0.0
            deudaTotal = data?.get("deuda_total")?.jsonPrimitive?.double ?: 0.0
        }
    }

    LaunchedEffect(Unit) { cargar() }

    Scaffold(
        floatingActionButton = {
            Box {
                FloatingActionButton(onClick = { addMenuExpanded = true }) {
                    Icon(Icons.Default.Add, "Agregar")
                }
                DropdownMenu(expanded = addMenuExpanded,
                    onDismissRequest = { addMenuExpanded = false }) {
                    DropdownMenuItem(
                        text = { Text("\uD83D\uDCB5 Agregar ingreso") },
                        onClick = { addMenuExpanded = false; showIngresoDialog = true }
                    )
                    DropdownMenuItem(
                        text = { Text("\uD83D\uDCB3 Agregar deuda") },
                        onClick = { addMenuExpanded = false; editDeuda = null; showDeudaDialog = true }
                    )
                }
            }
        }
    ) { padding ->
        LazyColumn(
            modifier = Modifier.padding(padding).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            // ── Resumen ──
            item {
                Text("Rastreador de Deudas", style = MaterialTheme.typography.headlineMedium)
                Spacer(Modifier.height(4.dp))
                Card(modifier = Modifier.fillMaxWidth(),
                    colors = CardDefaults.cardColors(
                        containerColor = MaterialTheme.colorScheme.primaryContainer)) {
                    Column(modifier = Modifier.padding(12.dp)) {
                        Text("Ingreso mensual: $${"%.2f".format(ingresoMensual)}",
                            style = MaterialTheme.typography.titleSmall,
                            color = MaterialTheme.colorScheme.onPrimaryContainer)
                        Text("Deuda total: $${"%.2f".format(deudaTotal)}",
                            style = MaterialTheme.typography.titleSmall,
                            color = MaterialTheme.colorScheme.error)
                        if (ingresoMensual > 0) {
                            val ratio = (deudaTotal / ingresoMensual * 100)
                            Text("Ratio deuda/ingreso: ${"%.0f".format(ratio)}%",
                                style = MaterialTheme.typography.bodySmall)
                        }
                    }
                }
            }

            // ── Sección: Ingresos ──
            item {
                Spacer(Modifier.height(8.dp))
                Text("\uD83D\uDCB5 Ingresos", style = MaterialTheme.typography.titleMedium)
            }

            if (ingresos.isEmpty()) {
                item {
                    Text("Sin ingresos registrados. Agrega tus fuentes de ingreso.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }

            items(ingresos) { ingreso ->
                val concepto = ingreso["concepto"]?.jsonPrimitive?.content ?: ""
                val monto = ingreso["monto"]?.jsonPrimitive?.double ?: 0.0
                val freq = ingreso["frecuencia"]?.jsonPrimitive?.content ?: "mensual"
                val montoMensual = ingreso["monto_mensual"]?.jsonPrimitive?.double ?: monto
                val indice = ingreso["indice"]?.jsonPrimitive?.int ?: 0

                Card(modifier = Modifier.fillMaxWidth()) {
                    Row(modifier = Modifier.padding(12.dp),
                        verticalAlignment = Alignment.CenterVertically) {
                        Column(modifier = Modifier.weight(1f)) {
                            Text(concepto, style = MaterialTheme.typography.titleSmall)
                            Text("$${"%.2f".format(monto)} $freq" +
                                    if (freq != "mensual") " (\u2248$${"%.2f".format(montoMensual)}/mes)" else "",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.primary)
                        }
                        IconButton(onClick = {
                            OmniBridge.ingresoEliminar(indice); cargar()
                        }) {
                            Icon(Icons.Default.Delete, "Eliminar",
                                tint = MaterialTheme.colorScheme.error)
                        }
                    }
                }
            }

            // ── Sección: Deudas ──
            item {
                Spacer(Modifier.height(8.dp))
                Text("\uD83D\uDCB3 Deudas", style = MaterialTheme.typography.titleMedium)
            }

            if (deudas.isEmpty()) {
                item {
                    Text("Sin deudas registradas.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }

            items(deudas) { deuda ->
                val nombre = deuda["nombre"]?.jsonPrimitive?.content ?: ""
                val tipo = deuda["tipo"]?.jsonPrimitive?.content ?: ""
                val tasa = deuda["tasa_anual"]?.jsonPrimitive?.double ?: 0.0
                val pagoMin = deuda["pago_minimo"]?.jsonPrimitive?.double ?: 0.0
                val saldo = deuda["saldo_actual"]?.jsonPrimitive?.double ?: 0.0
                val activa = deuda["activa"]?.jsonPrimitive?.boolean ?: true
                val indice = deuda["indice"]?.jsonPrimitive?.int ?: 0
                val enganche = deuda["enganche"]?.jsonPrimitive?.double ?: 0.0

                val tipoLabel = when(tipo) {
                    "deuda" -> if (tasa >= 0.01) "\uD83D\uDCB3 Deuda con inter\u00e9s" else "\uD83D\uDCB0 Deuda"
                    "pago_corriente" -> "\uD83D\uDD04 Pago corriente"
                    else -> "\uD83D\uDCB0 Deuda"
                }

                Card(modifier = Modifier.fillMaxWidth(),
                    colors = CardDefaults.cardColors(
                        containerColor = if (!activa) MaterialTheme.colorScheme.surfaceVariant
                        else MaterialTheme.colorScheme.surface)) {
                    Column(modifier = Modifier.padding(12.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Column(modifier = Modifier.weight(1f)) {
                                Text(nombre, style = MaterialTheme.typography.titleSmall)
                                Text(tipoLabel, style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                            IconButton(onClick = { editDeuda = deuda; showDeudaDialog = true }) {
                                Icon(Icons.Default.Edit, "Editar")
                            }
                            if (tipo != "pago_corriente") {
                                IconButton(onClick = { pagoDeuda = deuda }) {
                                    Icon(Icons.Default.Payment, "Registrar pago")
                                }
                            }
                            IconButton(onClick = {
                                OmniBridge.deudaEliminar(indice); cargar()
                            }) {
                                Icon(Icons.Default.Delete, "Eliminar",
                                    tint = MaterialTheme.colorScheme.error)
                            }
                        }
                        // Campos específicos por tipo de deuda
                        when(tipo) {
                            "deuda" -> {
                                if (tasa >= 0.01) {
                                    Text("Tasa inter\u00e9s: ${"%.1f".format(tasa)}% anual (compuesto, fija)",
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.tertiary)
                                    val interesMensual = saldo * tasa / 100.0 / 12.0
                                    if (interesMensual > 0.01) Text("Inter\u00e9s mensual: $${"%.2f".format(interesMensual)}",
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.error)
                                }
                                if (enganche > 0.01) Text("Enganche pagado: $${"%.2f".format(enganche)}",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.primary)
                                Text("Pago m\u00ednimo: $${"%.2f".format(pagoMin)}/mes",
                                    style = MaterialTheme.typography.bodySmall)
                                if (saldo > 0) {
                                    Text("Saldo: $${"%.2f".format(saldo)}",
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.error)
                                    if (pagoMin > 0 && tasa >= 0.01) {
                                        val tasaMes = tasa / 100.0 / 12.0
                                        val mesesEst = if (pagoMin > saldo * tasaMes) {
                                            kotlin.math.ceil(
                                                -kotlin.math.ln(1.0 - saldo * tasaMes / pagoMin) /
                                                kotlin.math.ln(1.0 + tasaMes)
                                            ).toInt()
                                        } else { 999 }
                                        if (mesesEst < 999) Text("\u2248$mesesEst meses para liquidar",
                                            style = MaterialTheme.typography.bodySmall,
                                            color = MaterialTheme.colorScheme.onSurfaceVariant)
                                        else Text("\u26A0 El pago no cubre intereses \u2014 la deuda crece",
                                            style = MaterialTheme.typography.bodySmall,
                                            color = MaterialTheme.colorScheme.error)
                                    }
                                }
                            }
                            "pago_corriente" -> {
                                Text("Monto mensual fijo: $${"%.2f".format(pagoMin)}",
                                    style = MaterialTheme.typography.bodySmall)
                            }
                            else -> {
                                Text("Pago: $${"%.2f".format(pagoMin)}/mes",
                                    style = MaterialTheme.typography.bodySmall)
                                if (saldo > 0) Text("Saldo: $${"%.2f".format(saldo)}",
                                    style = MaterialTheme.typography.bodySmall)
                            }
                        }
                        if (!activa) Text("\u2705 Liquidada",
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.primary)
                    }
                }
            }

            // Spacer final para que el FAB no tape
            item { Spacer(Modifier.height(72.dp)) }
        }
    }

    if (showDeudaDialog) {
        DeudaDialog(
            editDeuda = editDeuda,
            onDismiss = { showDeudaDialog = false; editDeuda = null },
            onSave = { nombre, tasa, pagoMin, oblig, saldoIni, eng ->
                if (editDeuda != null) {
                    val idx = editDeuda!!["indice"]?.jsonPrimitive?.int ?: 0
                    OmniBridge.deudaActualizar(idx, nombre = nombre, tasaAnual = tasa,
                        pagoMinimo = pagoMin, obligatoria = oblig)
                } else {
                    OmniBridge.deudaAgregar(nombre, tasa, pagoMin, oblig, saldoIni, eng)
                }
                cargar(); showDeudaDialog = false; editDeuda = null
            }
        )
    }

    if (showIngresoDialog) {
        IngresoDialog(
            onDismiss = { showIngresoDialog = false },
            onSave = { concepto, monto, frecuencia ->
                OmniBridge.ingresoAgregar(concepto, monto, frecuencia)
                cargar(); showIngresoDialog = false
            }
        )
    }

    if (pagoDeuda != null) {
        PagoDialog(
            deuda = pagoDeuda!!,
            onDismiss = { pagoDeuda = null },
            onPago = { idx, monto, cargos ->
                OmniBridge.deudaRegistrarPago(idx, monto, cargos)
                cargar(); pagoDeuda = null
            }
        )
    }
}

// ═══════════════════════════════════════════════════════════
//  Diálogos
// ═══════════════════════════════════════════════════════════

@Composable
fun GastoDialog(
    editLinea: JsonObject?, onDismiss: () -> Unit,
    onSave: (String, Double, String, Boolean, String, String, Double?) -> Unit
) {
    val isEdit = editLinea != null
    var nombre by remember(editLinea) { mutableStateOf(editLinea?.get("nombre")?.jsonPrimitive?.content ?: "") }
    var monto by remember(editLinea) { mutableStateOf(editLinea?.get("monto")?.jsonPrimitive?.double?.let { "%.2f".format(it) } ?: "") }
    var categoria by remember(editLinea) { mutableStateOf(editLinea?.get("categoria")?.jsonPrimitive?.content ?: "gasto_variable") }
    var pagado by remember(editLinea) { mutableStateOf(editLinea?.get("pagado")?.jsonPrimitive?.boolean ?: false) }
    var fechaLimite by remember(editLinea) { mutableStateOf(editLinea?.get("fecha_limite")?.jsonPrimitive?.content ?: "") }
    var notas by remember(editLinea) { mutableStateOf(editLinea?.get("notas")?.jsonPrimitive?.content ?: "") }
    var saldoDeuda by remember(editLinea) { mutableStateOf(editLinea?.get("saldo_total_deuda")?.jsonPrimitive?.doubleOrNull?.let { "%.2f".format(it) } ?: "") }

    val categorias = listOf("ingreso", "gasto_fijo", "gasto_variable", "pago_deuda", "ahorro")

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(if (isEdit) "Editar movimiento" else "Agregar movimiento") },
        text = {
            Column(modifier = Modifier.verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(value = nombre, onValueChange = { nombre = it },
                    label = { Text("Concepto") }, singleLine = true, modifier = Modifier.fillMaxWidth())
                OutlinedTextField(value = monto, onValueChange = { monto = it },
                    label = { Text("Monto") }, singleLine = true, modifier = Modifier.fillMaxWidth())
                Text("Categor\u00eda:", style = MaterialTheme.typography.bodySmall)
                Column {
                    categorias.forEach { cat ->
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            RadioButton(selected = categoria == cat, onClick = { categoria = cat })
                            Text(cat.replace("_", " ").replaceFirstChar { it.uppercase() },
                                modifier = Modifier.padding(start = 4.dp))
                        }
                    }
                }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = pagado, onCheckedChange = { pagado = it })
                    Text("Pagado", modifier = Modifier.padding(start = 4.dp))
                }
                OutlinedTextField(value = fechaLimite, onValueChange = { fechaLimite = it },
                    label = { Text("Fecha l\u00edmite (d\u00eda del mes)") }, singleLine = true,
                    modifier = Modifier.fillMaxWidth())
                OutlinedTextField(value = notas, onValueChange = { notas = it },
                    label = { Text("Notas") }, modifier = Modifier.fillMaxWidth(), minLines = 2)
                if (categoria == "pago_deuda") {
                    OutlinedTextField(value = saldoDeuda, onValueChange = { saldoDeuda = it },
                        label = { Text("Saldo total de la deuda") }, singleLine = true,
                        modifier = Modifier.fillMaxWidth())
                }
            }
        },
        confirmButton = {
            TextButton(onClick = {
                val m = monto.toDoubleOrNull()
                if (nombre.isNotBlank() && m != null)
                    onSave(nombre, m, categoria, pagado, fechaLimite, notas,
                        if (categoria == "pago_deuda") saldoDeuda.toDoubleOrNull() else null)
            }, enabled = nombre.isNotBlank() && monto.toDoubleOrNull() != null) {
                Text(if (isEdit) "Guardar" else "Agregar")
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancelar") } }
    )
}

@Composable
fun IngresoDialog(onDismiss: () -> Unit, onSave: (String, Double, String) -> Unit) {
    var concepto by remember { mutableStateOf("") }
    var monto by remember { mutableStateOf("") }
    var frecuencia by remember { mutableStateOf("mensual") }

    val frecuencias = listOf(
        "semanal" to "Semanal",
        "quincenal" to "Quincenal",
        "mensual" to "Mensual",
        "trimestral" to "Trimestral",
        "semestral" to "Semestral",
        "anual" to "Anual",
        "una_vez" to "Una vez"
    )

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Agregar ingreso") },
        text = {
            Column(modifier = Modifier.verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(value = concepto, onValueChange = { concepto = it },
                    label = { Text("Concepto (ej: Sueldo, freelance)") },
                    singleLine = true, modifier = Modifier.fillMaxWidth())
                OutlinedTextField(value = monto, onValueChange = { monto = it },
                    label = { Text("Monto") }, singleLine = true,
                    modifier = Modifier.fillMaxWidth())
                Text("Frecuencia:", style = MaterialTheme.typography.bodySmall)
                Column {
                    frecuencias.forEach { (key, label) ->
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            RadioButton(selected = frecuencia == key,
                                onClick = { frecuencia = key })
                            Text(label, modifier = Modifier.padding(start = 4.dp))
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = {
                val m = monto.toDoubleOrNull()
                if (concepto.isNotBlank() && m != null) onSave(concepto, m, frecuencia)
            }, enabled = concepto.isNotBlank() && monto.toDoubleOrNull() != null) {
                Text("Agregar")
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancelar") } }
    )
}

@Composable
fun DeudaDialog(
    editDeuda: JsonObject?, onDismiss: () -> Unit,
    onSave: (String, Double, Double, Boolean, Double, Double) -> Unit
) {
    val isEdit = editDeuda != null
    var nombre by remember(editDeuda) { mutableStateOf(editDeuda?.get("nombre")?.jsonPrimitive?.content ?: "") }
    var tasaAnual by remember(editDeuda) { mutableStateOf(editDeuda?.get("tasa_anual")?.jsonPrimitive?.double?.let { "%.2f".format(it) } ?: "") }
    var pagoMinimo by remember(editDeuda) { mutableStateOf(editDeuda?.get("pago_minimo")?.jsonPrimitive?.double?.let { "%.2f".format(it) } ?: "") }
    var obligatoria by remember(editDeuda) { mutableStateOf(editDeuda?.get("obligatoria")?.jsonPrimitive?.boolean ?: false) }
    var saldoInicial by remember(editDeuda) { mutableStateOf(editDeuda?.get("saldo_actual")?.jsonPrimitive?.double?.let { "%.2f".format(it) } ?: "") }
    var enganche by remember(editDeuda) { mutableStateOf(editDeuda?.get("enganche")?.jsonPrimitive?.double?.takeIf { it > 0.0 }?.let { "%.2f".format(it) } ?: "") }

    // Tipo: 0 = deuda con interés, 1 = pago corriente
    var tipoSeleccion by remember(editDeuda) {
        val tipo = editDeuda?.get("tipo")?.jsonPrimitive?.content ?: ""
        mutableStateOf(if (tipo == "pago_corriente") 1 else 0)
    }

    val tipos = listOf(
        "\uD83D\uDCB3 Deuda con inter\u00e9s" to "Tarjeta, pr\u00e9stamo \u2014 inter\u00e9s compuesto fijo",
        "\uD83D\uDD04 Pago corriente" to "Renta, seguro, suscripci\u00f3n"
    )

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(if (isEdit) "Editar deuda" else "Agregar deuda") },
        text = {
            Column(modifier = Modifier.verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(8.dp)) {

                OutlinedTextField(value = nombre, onValueChange = { nombre = it },
                    label = { Text("Nombre de la cuenta") }, singleLine = true,
                    modifier = Modifier.fillMaxWidth())

                if (!isEdit) {
                    Text("Tipo:", style = MaterialTheme.typography.bodySmall)
                    Column {
                        tipos.forEachIndexed { i, (label, desc) ->
                            Row(verticalAlignment = Alignment.CenterVertically,
                                modifier = Modifier.clickable {
                                    tipoSeleccion = i
                                    when(i) {
                                        0 -> { obligatoria = false }
                                        1 -> { obligatoria = true; tasaAnual = "0"; enganche = "" }
                                    }
                                }) {
                                RadioButton(selected = tipoSeleccion == i,
                                    onClick = null)
                                Column(modifier = Modifier.padding(start = 4.dp)) {
                                    Text(label, style = MaterialTheme.typography.bodyMedium)
                                    Text(desc, style = MaterialTheme.typography.labelSmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                                }
                            }
                        }
                    }
                }

                // Campos según tipo
                if (tipoSeleccion == 0) {
                    OutlinedTextField(value = tasaAnual, onValueChange = { tasaAnual = it },
                        label = { Text("Tasa inter\u00e9s anual fija (%) ej: 24.99") },
                        singleLine = true, modifier = Modifier.fillMaxWidth())
                    Text("Inter\u00e9s compuesto \u2014 no var\u00eda durante el pr\u00e9stamo",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                }

                OutlinedTextField(value = pagoMinimo, onValueChange = { pagoMinimo = it },
                    label = { Text(if (tipoSeleccion == 1) "Monto mensual" else "Pago m\u00ednimo mensual") },
                    singleLine = true, modifier = Modifier.fillMaxWidth())

                if (tipoSeleccion == 0) {
                    OutlinedTextField(value = saldoInicial, onValueChange = { saldoInicial = it },
                        label = { Text("Monto total de la deuda") }, singleLine = true,
                        modifier = Modifier.fillMaxWidth())

                    OutlinedTextField(value = enganche, onValueChange = { enganche = it },
                        label = { Text("Enganche / pago inicial (opcional)") },
                        singleLine = true, modifier = Modifier.fillMaxWidth())
                    val eng = enganche.toDoubleOrNull() ?: 0.0
                    val sal = saldoInicial.toDoubleOrNull() ?: 0.0
                    if (eng > 0.0 && sal > eng) {
                        Text("Saldo pendiente tras enganche: $${"%.2f".format(sal - eng)}",
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.primary)
                    }

                    if (!isEdit) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Checkbox(checked = obligatoria, onCheckedChange = { obligatoria = it })
                            Text("Obligatoria (hipoteca, carro)", style = MaterialTheme.typography.bodySmall)
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = {
                val pm = pagoMinimo.toDoubleOrNull()
                if (nombre.isNotBlank() && pm != null)
                    onSave(nombre, tasaAnual.toDoubleOrNull() ?: 0.0, pm,
                        obligatoria, saldoInicial.toDoubleOrNull() ?: 0.0,
                        enganche.toDoubleOrNull() ?: 0.0)
            }, enabled = nombre.isNotBlank() && pagoMinimo.toDoubleOrNull() != null) {
                Text(if (isEdit) "Guardar" else "Agregar")
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancelar") } }
    )
}

@Composable
fun PagoDialog(deuda: JsonObject, onDismiss: () -> Unit,
               onPago: (Int, Double, Double) -> Unit) {
    val nombre = deuda["nombre"]?.jsonPrimitive?.content ?: ""
    val saldo = deuda["saldo_actual"]?.jsonPrimitive?.double ?: 0.0
    val indice = deuda["indice"]?.jsonPrimitive?.int ?: 0
    var montoPago by remember { mutableStateOf("") }
    var nuevosCargos by remember { mutableStateOf("") }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Registrar pago: $nombre") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("Saldo actual: $${"%.2f".format(saldo)}",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.error)
                OutlinedTextField(value = montoPago, onValueChange = { montoPago = it },
                    label = { Text("Monto del pago") }, singleLine = true,
                    modifier = Modifier.fillMaxWidth())
                OutlinedTextField(value = nuevosCargos, onValueChange = { nuevosCargos = it },
                    label = { Text("Nuevos cargos (compras, etc.)") }, singleLine = true,
                    modifier = Modifier.fillMaxWidth())
            }
        },
        confirmButton = {
            TextButton(onClick = {
                val p = montoPago.toDoubleOrNull()
                if (p != null) onPago(indice, p, nuevosCargos.toDoubleOrNull() ?: 0.0)
            }, enabled = montoPago.toDoubleOrNull() != null) { Text("Registrar") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancelar") } }
    )
}
