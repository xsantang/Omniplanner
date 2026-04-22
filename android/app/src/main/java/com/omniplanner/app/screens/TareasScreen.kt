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
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import com.omniplanner.app.OmniBridge
import kotlinx.serialization.json.*

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TareasScreen() {
    var tareas by remember { mutableStateOf<List<JsonObject>>(emptyList()) }
    var showCrear by remember { mutableStateOf(false) }
    var editando by remember { mutableStateOf<JsonObject?>(null) }

    fun cargar() {
        val r = OmniBridge.tareasListar()
        if (r.ok) {
            tareas = r.data?.jsonArray?.map { it.jsonObject } ?: emptyList()
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
            Text("Tareas", style = MaterialTheme.typography.headlineMedium)
            if (tareas.isNotEmpty()) {
                val completadas = tareas.count {
                    it["estado"]?.jsonPrimitive?.content == "Completada"
                }
                Text("$completadas/${tareas.size} completadas",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            Spacer(Modifier.height(12.dp))

            if (tareas.isEmpty()) {
                Text("No hay tareas. Toca + para crear una.",
                    style = MaterialTheme.typography.bodyLarge)
            }

            LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                items(tareas, key = { it["id"]?.jsonPrimitive?.content ?: "" }) { tarea ->
                    val id = tarea["id"]?.jsonPrimitive?.content ?: ""
                    val titulo = tarea["titulo"]?.jsonPrimitive?.content ?: ""
                    val estado = tarea["estado"]?.jsonPrimitive?.content ?: ""
                    val prioridad = tarea["prioridad"]?.jsonPrimitive?.content ?: ""
                    val desc = tarea["descripcion"]?.jsonPrimitive?.content ?: ""
                    val completada = estado == "Completada"

                    Card(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { editando = tarea }
                    ) {
                        Row(
                            modifier = Modifier.padding(12.dp),
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Checkbox(
                                checked = completada,
                                onCheckedChange = {
                                    val nuevo = if (completada) "pendiente" else "completada"
                                    OmniBridge.tareaActualizar(id, estado = nuevo)
                                    cargar()
                                }
                            )
                            Column(modifier = Modifier.weight(1f)) {
                                Text(
                                    titulo,
                                    style = MaterialTheme.typography.titleSmall,
                                    textDecoration = if (completada) TextDecoration.LineThrough else TextDecoration.None,
                                    color = if (completada) MaterialTheme.colorScheme.onSurfaceVariant
                                            else MaterialTheme.colorScheme.onSurface
                                )
                                if (desc.isNotBlank()) {
                                    Text(desc, style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                        maxLines = 1)
                                }
                                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                    val prioColor = when (prioridad) {
                                        "urgente" -> MaterialTheme.colorScheme.error
                                        "alta" -> MaterialTheme.colorScheme.tertiary
                                        else -> MaterialTheme.colorScheme.onSurfaceVariant
                                    }
                                    Text(prioridad, style = MaterialTheme.typography.labelSmall, color = prioColor)
                                    Text("·", style = MaterialTheme.typography.labelSmall)
                                    Text(estado, style = MaterialTheme.typography.labelSmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                                }
                            }
                            IconButton(onClick = { editando = tarea }) {
                                Icon(Icons.Default.Edit, "Editar",
                                    tint = MaterialTheme.colorScheme.primary)
                            }
                            IconButton(onClick = {
                                OmniBridge.tareaEliminar(id)
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
        TareaDialog(
            titulo = "Nueva tarea",
            onDismiss = { showCrear = false },
            onSave = { tit, desc, prio ->
                OmniBridge.tareaCrear(tit, desc, prioridad = prio)
                cargar()
                showCrear = false
            }
        )
    }

    editando?.let { tarea ->
        val id = tarea["id"]?.jsonPrimitive?.content ?: ""
        TareaDialog(
            titulo = "Editar tarea",
            initialTitulo = tarea["titulo"]?.jsonPrimitive?.content ?: "",
            initialDesc = tarea["descripcion"]?.jsonPrimitive?.content ?: "",
            initialPrioridad = tarea["prioridad"]?.jsonPrimitive?.content ?: "media",
            onDismiss = { editando = null },
            onSave = { tit, desc, prio ->
                OmniBridge.tareaActualizar(id, titulo = tit)
                // Actualizar descripcion requiere otro param - update titulo only for now
                cargar()
                editando = null
            }
        )
    }
}

@Composable
fun TareaDialog(
    titulo: String,
    initialTitulo: String = "",
    initialDesc: String = "",
    initialPrioridad: String = "media",
    onDismiss: () -> Unit,
    onSave: (String, String, String) -> Unit
) {
    var tit by remember { mutableStateOf(initialTitulo) }
    var desc by remember { mutableStateOf(initialDesc) }
    var prioridad by remember { mutableStateOf(initialPrioridad) }

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
                    value = desc, onValueChange = { desc = it },
                    label = { Text("Descripción") },
                    modifier = Modifier.fillMaxWidth(), maxLines = 3
                )
                Text("Prioridad:", style = MaterialTheme.typography.bodySmall)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf("baja", "media", "alta", "urgente").forEach { p ->
                        FilterChip(
                            selected = prioridad == p,
                            onClick = { prioridad = p },
                            label = { Text(p.replaceFirstChar { it.uppercase() }) }
                        )
                    }
                }
            }
        },
        confirmButton = {
            TextButton(
                onClick = { if (tit.isNotBlank()) onSave(tit, desc, prioridad) },
                enabled = tit.isNotBlank()
            ) { Text("Guardar") }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Cancelar") }
        }
    )
}
