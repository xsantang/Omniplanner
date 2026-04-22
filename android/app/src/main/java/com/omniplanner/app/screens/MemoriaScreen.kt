package com.omniplanner.app.screens

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.omniplanner.app.OmniBridge
import kotlinx.serialization.json.*

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MemoriaScreen() {
    var recuerdos by remember { mutableStateOf<List<JsonObject>>(emptyList()) }
    var showCrear by remember { mutableStateOf(false) }
    var viendo by remember { mutableStateOf<JsonObject?>(null) }

    fun cargar() {
        val r = OmniBridge.memoriaListar()
        if (r.ok) {
            recuerdos = r.data?.jsonArray?.map { it.jsonObject } ?: emptyList()
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
            Text("Memoria", style = MaterialTheme.typography.headlineMedium)
            if (recuerdos.isNotEmpty()) {
                Text("${recuerdos.size} recuerdos",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            Spacer(Modifier.height(12.dp))

            if (recuerdos.isEmpty()) {
                Text("Sin recuerdos. Toca + para agregar uno.",
                    style = MaterialTheme.typography.bodyLarge)
            }

            LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                items(recuerdos, key = { it["id"]?.jsonPrimitive?.content ?: "" }) { rec ->
                    val id = rec["id"]?.jsonPrimitive?.content ?: ""
                    val contenido = rec["contenido"]?.jsonPrimitive?.content ?: ""
                    val palabras = rec["palabras_clave"]?.jsonArray
                        ?.mapNotNull { it.jsonPrimitive.contentOrNull }
                        ?: emptyList()
                    val fecha = rec["creado"]?.jsonPrimitive?.content ?: ""

                    Card(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { viendo = rec }
                    ) {
                        Row(
                            modifier = Modifier.padding(12.dp),
                            verticalAlignment = Alignment.Top
                        ) {
                            Column(modifier = Modifier.weight(1f)) {
                                Text(
                                    contenido.take(80) + if (contenido.length > 80) "..." else "",
                                    style = MaterialTheme.typography.bodyMedium
                                )
                                Spacer(Modifier.height(4.dp))
                                if (palabras.isNotEmpty()) {
                                    Text(
                                        palabras.take(5).joinToString(" · "),
                                        style = MaterialTheme.typography.labelSmall,
                                        color = MaterialTheme.colorScheme.primary
                                    )
                                }
                                Text(
                                    fecha.take(10),
                                    style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                            }
                            IconButton(onClick = {
                                OmniBridge.memoriaEliminar(id)
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
        AgregarRecuerdoDialog(
            onDismiss = { showCrear = false },
            onAdd = { contenido, palabras ->
                OmniBridge.memoriaAgregar(contenido, palabras)
                cargar()
                showCrear = false
            }
        )
    }

    viendo?.let { rec ->
        val contenido = rec["contenido"]?.jsonPrimitive?.content ?: ""
        val palabras = rec["palabras_clave"]?.jsonArray
            ?.mapNotNull { it.jsonPrimitive.contentOrNull } ?: emptyList()
        val fecha = rec["creado"]?.jsonPrimitive?.content ?: ""

        AlertDialog(
            onDismissRequest = { viendo = null },
            title = { Text("Recuerdo") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(contenido, style = MaterialTheme.typography.bodyMedium)
                    if (palabras.isNotEmpty()) {
                        Text("Palabras clave: ${palabras.joinToString(", ")}",
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.primary)
                    }
                    Text("Creado: $fecha",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            },
            confirmButton = {
                TextButton(onClick = { viendo = null }) { Text("Cerrar") }
            }
        )
    }
}

@Composable
fun AgregarRecuerdoDialog(onDismiss: () -> Unit, onAdd: (String, List<String>) -> Unit) {
    var contenido by remember { mutableStateOf("") }
    var palabras by remember { mutableStateOf("") }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Nuevo recuerdo") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    value = contenido, onValueChange = { contenido = it },
                    label = { Text("Contenido") },
                    modifier = Modifier.fillMaxWidth(), maxLines = 5
                )
                OutlinedTextField(
                    value = palabras, onValueChange = { palabras = it },
                    label = { Text("Palabras clave (separadas por coma)") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth()
                )
            }
        },
        confirmButton = {
            TextButton(
                onClick = {
                    if (contenido.isNotBlank()) {
                        val lista = palabras.split(",").map { it.trim() }.filter { it.isNotBlank() }
                        onAdd(contenido, lista)
                    }
                },
                enabled = contenido.isNotBlank()
            ) { Text("Agregar") }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Cancelar") }
        }
    )
}
