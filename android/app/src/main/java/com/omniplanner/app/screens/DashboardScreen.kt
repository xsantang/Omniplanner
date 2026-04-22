package com.omniplanner.app.screens

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.dp
import com.omniplanner.app.OmniBridge
import kotlinx.serialization.json.*

@Composable
fun DashboardScreen(onNavigate: (String) -> Unit = {}) {
    var data by remember { mutableStateOf<JsonObject?>(null) }
    var error by remember { mutableStateOf<String?>(null) }

    fun cargar() {
        val r = OmniBridge.dashboard()
        if (r.ok) data = r.data?.jsonObject else error = r.error
    }

    LaunchedEffect(Unit) { cargar() }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Text("Dashboard", style = MaterialTheme.typography.headlineMedium)

        error?.let {
            Text(it, color = MaterialTheme.colorScheme.error)
            return@Column
        }

        data?.let { d ->
            val fecha = d["fecha"]?.jsonPrimitive?.content ?: ""
            Text(fecha, style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant)

            NavCard("Tareas", Icons.Default.CheckCircle, "tareas", onNavigate) {
                val pendientes = d["tareas_pendientes"]?.jsonPrimitive?.int ?: 0
                val hoy = d["tareas_hoy"]?.jsonPrimitive?.int ?: 0
                Text("$pendientes pendientes · $hoy para hoy")
            }

            val pres = d["presupuesto"]?.jsonObject
            NavCard("Dinero", Icons.Default.AccountBalance, "presupuesto", onNavigate) {
                val ing = pres?.get("ingresos")?.jsonPrimitive?.double ?: 0.0
                val gas = pres?.get("gastos")?.jsonPrimitive?.double ?: 0.0
                val bal = pres?.get("balance")?.jsonPrimitive?.double ?: 0.0
                Text("Ingresos: $${"%.2f".format(ing)}  ·  Gastos: $${"%.2f".format(gas)}")
                Text(
                    "Balance: $${"%.2f".format(bal)}",
                    color = if (bal >= 0) MaterialTheme.colorScheme.primary
                            else MaterialTheme.colorScheme.error
                )
            }

            val eventosHoy = d["eventos_hoy"]?.jsonArray ?: JsonArray(emptyList())
            NavCard("Agenda", Icons.Default.CalendarMonth, "agenda", onNavigate) {
                if (eventosHoy.isEmpty()) {
                    Text("Sin eventos hoy", style = MaterialTheme.typography.bodySmall)
                } else {
                    eventosHoy.take(3).forEach { ev ->
                        val obj = ev.jsonObject
                        Text("${obj["hora"]?.jsonPrimitive?.content ?: ""} — ${obj["titulo"]?.jsonPrimitive?.content ?: ""}")
                    }
                    if (eventosHoy.size > 3) Text("+${eventosHoy.size - 3} más...")
                }
            }

            NavCard("Claves", Icons.Default.Lock, "contras", onNavigate) {
                Text("${d["contrasenias"]?.jsonPrimitive?.int ?: 0} guardadas")
            }

            NavCard("Memoria", Icons.Default.Psychology, "memoria", onNavigate) {
                Text("${d["memoria"]?.jsonPrimitive?.int ?: 0} recuerdos")
            }
        } ?: run {
            Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
                CircularProgressIndicator()
            }
        }
    }
}

@Composable
fun NavCard(
    title: String,
    icon: ImageVector,
    route: String,
    onNavigate: (String) -> Unit,
    content: @Composable ColumnScope.() -> Unit
) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onNavigate(route) },
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceVariant
        )
    ) {
        Row(
            modifier = Modifier.padding(16.dp),
            verticalAlignment = Alignment.Top
        ) {
            Icon(icon, title, modifier = Modifier.padding(end = 12.dp, top = 2.dp),
                tint = MaterialTheme.colorScheme.primary)
            Column(modifier = Modifier.weight(1f)) {
                Text(title, style = MaterialTheme.typography.titleMedium)
                Spacer(Modifier.height(4.dp))
                content()
            }
            Icon(Icons.Default.ChevronRight, "Ir",
                tint = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}
