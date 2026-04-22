package com.omniplanner.app.screens

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.omniplanner.app.OmniBridge
import kotlinx.serialization.json.*

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AgendaScreen() {
    var eventos by remember { mutableStateOf<List<JsonObject>>(emptyList()) }
    var fechaLabel by remember { mutableStateOf("") }
    var showCrear by remember { mutableStateOf(false) }
    var editando by remember { mutableStateOf<JsonObject?>(null) }

    fun cargar() {
        val r = OmniBridge.agendaHoy()
        if (r.ok) {
            val d = r.data?.jsonObject
            fechaLabel = d?.get("fecha")?.jsonPrimitive?.content ?: ""
            eventos = d?.get("eventos")?.jsonArray?.map { it.jsonObject } ?: emptyList()
        }
    }

    LaunchedEffect(Unit) { cargar() }

    Scaffold(
        floatingActionButton = {
            FloatingActionButton(onClick = { showCrear = true }) {
                Icon(Icons.Default.Add, "Agregar")
            }
        }
    ) { padding ->
        Column(modifier = Modifier.padding(padding).padding(16.dp)) {
            Text("Agenda", style = MaterialTheme.typography.headlineMedium)
            Text("Hoy: $fechaLabel", style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(12.dp))

            if (eventos.isEmpty()) {
                Text("Sin eventos hoy. Toca + para crear uno.",
                    style = MaterialTheme.typography.bodyLarge)
            }

            LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                items(eventos, key = { it["id"]?.jsonPrimitive?.content ?: "" }) { ev ->
                    val id = ev["id"]?.jsonPrimitive?.content ?: ""
                    val titulo = ev["titulo"]?.jsonPrimitive?.content ?: ""
                    val hora = ev["hora_inicio"]?.jsonPrimitive?.content ?: ""
                    val tipo = ev["tipo"]?.jsonPrimitive?.content ?: ""
                    val desc = ev["descripcion"]?.jsonPrimitive?.content ?: ""

                    Card(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { editando = ev }
                    ) {
                        Row(
                            modifier = Modifier.padding(12.dp),
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            // Hora badge
                            Surface(
                                color = MaterialTheme.colorScheme.primaryContainer,
                                shape = MaterialTheme.shapes.small
                            ) {
                                Text(
                                    hora.take(5),
                                    modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp),
                                    style = MaterialTheme.typography.labelMedium
                                )
                            }
                            Spacer(Modifier.width(12.dp))
                            Column(modifier = Modifier.weight(1f)) {
                                Text(titulo, style = MaterialTheme.typography.titleSmall)
                                if (desc.isNotBlank()) {
                                    Text(desc, style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                        maxLines = 1)
                                }
                                Text(tipo, style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                            IconButton(onClick = { editando = ev }) {
                                Icon(Icons.Default.Edit, "Editar",
                                    tint = MaterialTheme.colorScheme.primary)
                            }
                            IconButton(onClick = {
                                OmniBridge.eventoEliminar(id)
                                cargar()
                            }) {
                                Icon(Icons.Default.Delete, "Eliminar",
                                    tint = MaterialTheme.colorScheme.error)
                            }
                        }
                    }
                }
            }
        }
    }

    if (showCrear) {
        EventoDialog(
            titulo = "Nuevo evento",
            onDismiss = { showCrear = false },
            onSave = { tit, fecha, hora, tipo ->
                OmniBridge.eventoCrear(tit, fecha, hora, tipo)
                cargar()
                showCrear = false
            }
        )
    }

    editando?.let { ev ->
        val id = ev["id"]?.jsonPrimitive?.content ?: ""
        val fechaEvt = ev["fecha"]?.jsonPrimitive?.content ?: ""
        val horaEvt = ev["hora_inicio"]?.jsonPrimitive?.content ?: ""
        EventoDialog(
            titulo = "Editar evento",
            initialTitulo = ev["titulo"]?.jsonPrimitive?.content ?: "",
            initialFecha = fechaEvt,
            initialHora = horaEvt.take(5),
            initialTipo = ev["tipo"]?.jsonPrimitive?.content ?: "recordatorio",
            onDismiss = { editando = null },
            onSave = { tit, fecha, hora, _ ->
                OmniBridge.eventoActualizar(id, titulo = tit, fecha = fecha, hora = hora)
                cargar()
                editando = null
            }
        )
    }
}

@Composable
fun EventoDialog(
    titulo: String,
    initialTitulo: String = "",
    initialFecha: String = "",
    initialHora: String = "09:00",
    initialTipo: String = "recordatorio",
    onDismiss: () -> Unit,
    onSave: (String, String, String, String) -> Unit
) {
    var tit by remember { mutableStateOf(initialTitulo) }
    var fecha by remember { mutableStateOf(initialFecha) }
    var hora by remember { mutableStateOf(initialHora) }
    var tipo by remember { mutableStateOf(initialTipo) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(titulo) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    value = tit, onValueChange = { tit = it },
                    label = { Text("Título") }, singleLine = true,
                    modifier = Modifier.fillMaxWidth()
                )
                OutlinedTextField(
                    value = fecha, onValueChange = { fecha = it },
                    label = { Text("Fecha (YYYY-MM-DD)") }, singleLine = true,
                    modifier = Modifier.fillMaxWidth()
                )
                OutlinedTextField(
                    value = hora, onValueChange = { hora = it },
                    label = { Text("Hora (HH:MM)") }, singleLine = true,
                    modifier = Modifier.fillMaxWidth()
                )
                Text("Tipo:", style = MaterialTheme.typography.bodySmall)
                Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    listOf("recordatorio", "reunion", "cita", "pago").forEach { t ->
                        FilterChip(
                            selected = tipo == t,
                            onClick = { tipo = t },
                            label = { Text(t.replaceFirstChar { it.uppercase() }) }
                        )
                    }
                }
            }
        },
        confirmButton = {
            TextButton(
                onClick = { if (tit.isNotBlank()) onSave(tit, fecha, hora, tipo) },
                enabled = tit.isNotBlank()
            ) { Text("Guardar") }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Cancelar") }
        }
    )
}
