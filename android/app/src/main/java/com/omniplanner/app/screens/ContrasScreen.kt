package com.omniplanner.app.screens

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.unit.dp
import com.omniplanner.app.OmniBridge
import kotlinx.serialization.json.*

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ContrasScreen() {
    var entradas by remember { mutableStateOf<List<JsonObject>>(emptyList()) }
    var showCrear by remember { mutableStateOf(false) }
    var editando by remember { mutableStateOf<JsonObject?>(null) }
    var generada by remember { mutableStateOf<String?>(null) }
    val clipboard = LocalClipboardManager.current

    fun cargar() {
        val r = OmniBridge.contrasListar()
        if (r.ok) {
            entradas = r.data?.jsonObject?.get("entradas")?.jsonArray?.map { it.jsonObject } ?: emptyList()
        }
    }

    LaunchedEffect(Unit) { cargar() }

    Scaffold(
        floatingActionButton = {
            Column(horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(8.dp)) {
                FloatingActionButton(
                    onClick = {
                        val r = OmniBridge.contrasGenerar(20)
                        if (r.ok) {
                            generada = r.data?.jsonObject?.get("passwords")?.jsonArray
                                ?.firstOrNull()?.jsonObject?.get("password")?.jsonPrimitive?.content
                        }
                    },
                    containerColor = MaterialTheme.colorScheme.secondary
                ) {
                    Icon(Icons.Default.Refresh, "Generar")
                }
                FloatingActionButton(onClick = { showCrear = true }) {
                    Icon(Icons.Default.Add, "Agregar")
                }
            }
        }
    ) { padding ->
        Column(modifier = Modifier.padding(padding).padding(16.dp)) {
            Text("Contraseñas", style = MaterialTheme.typography.headlineMedium)
            Spacer(Modifier.height(12.dp))

            generada?.let { pass ->
                Card(
                    modifier = Modifier.fillMaxWidth(),
                    colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.tertiaryContainer)
                ) {
                    Row(
                        modifier = Modifier.padding(12.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Column(modifier = Modifier.weight(1f)) {
                            Text("Generada:", style = MaterialTheme.typography.labelSmall)
                            Text(pass, style = MaterialTheme.typography.bodyMedium)
                        }
                        IconButton(onClick = {
                            clipboard.setText(AnnotatedString(pass))
                        }) {
                            Icon(Icons.Default.ContentCopy, "Copiar")
                        }
                        IconButton(onClick = { generada = null }) {
                            Icon(Icons.Default.Close, "Cerrar")
                        }
                    }
                }
                Spacer(Modifier.height(12.dp))
            }

            if (entradas.isEmpty()) {
                Text("Sin contraseñas guardadas. Toca + para agregar.",
                    style = MaterialTheme.typography.bodyLarge)
            }

            LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                items(entradas, key = { it["id"]?.jsonPrimitive?.content ?: "" }) { entrada ->
                    val id = entrada["id"]?.jsonPrimitive?.content ?: ""
                    val nombre = entrada["nombre"]?.jsonPrimitive?.content ?: ""
                    val usuario = entrada["usuario"]?.jsonPrimitive?.content ?: ""
                    val categoria = entrada["categoria"]?.jsonPrimitive?.content ?: ""

                    Card(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { editando = entrada }
                    ) {
                        Row(
                            modifier = Modifier.padding(12.dp),
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Icon(Icons.Default.Key, "Clave",
                                modifier = Modifier.padding(end = 12.dp),
                                tint = MaterialTheme.colorScheme.primary)
                            Column(modifier = Modifier.weight(1f)) {
                                Text(nombre, style = MaterialTheme.typography.titleSmall)
                                if (usuario.isNotBlank()) {
                                    Text(usuario, style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                                }
                                if (categoria.isNotBlank()) {
                                    Text(categoria, style = MaterialTheme.typography.labelSmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                                }
                            }
                            IconButton(onClick = { editando = entrada }) {
                                Icon(Icons.Default.Edit, "Editar",
                                    tint = MaterialTheme.colorScheme.primary)
                            }
                            IconButton(onClick = {
                                OmniBridge.contrasEliminar(id)
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
        ContraDialog(
            titulo = "Guardar contraseña",
            onDismiss = { showCrear = false },
            onSave = { nombre, usuario, clave ->
                OmniBridge.contrasGuardar(nombre, usuario, clave)
                cargar()
                showCrear = false
            }
        )
    }

    editando?.let { entrada ->
        val id = entrada["id"]?.jsonPrimitive?.content ?: ""
        ContraDialog(
            titulo = "Editar contraseña",
            initialNombre = entrada["nombre"]?.jsonPrimitive?.content ?: "",
            initialUsuario = entrada["usuario"]?.jsonPrimitive?.content ?: "",
            onDismiss = { editando = null },
            onSave = { nombre, usuario, clave ->
                if (clave.isNotBlank()) {
                    OmniBridge.contrasActualizar(id, nombre = nombre, usuario = usuario, clave = clave)
                } else {
                    OmniBridge.contrasActualizar(id, nombre = nombre, usuario = usuario)
                }
                cargar()
                editando = null
            }
        )
    }
}

@Composable
fun ContraDialog(
    titulo: String,
    initialNombre: String = "",
    initialUsuario: String = "",
    onDismiss: () -> Unit,
    onSave: (String, String, String) -> Unit
) {
    var nombre by remember { mutableStateOf(initialNombre) }
    var usuario by remember { mutableStateOf(initialUsuario) }
    var clave by remember { mutableStateOf("") }
    val isEdit = initialNombre.isNotBlank()

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(titulo) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    value = nombre, onValueChange = { nombre = it },
                    label = { Text("Servicio") }, singleLine = true,
                    modifier = Modifier.fillMaxWidth()
                )
                OutlinedTextField(
                    value = usuario, onValueChange = { usuario = it },
                    label = { Text("Usuario") }, singleLine = true,
                    modifier = Modifier.fillMaxWidth()
                )
                OutlinedTextField(
                    value = clave, onValueChange = { clave = it },
                    label = { Text(if (isEdit) "Nueva contraseña (vacío = no cambiar)" else "Contraseña") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth()
                )
            }
        },
        confirmButton = {
            TextButton(
                onClick = { if (nombre.isNotBlank()) onSave(nombre, usuario, clave) },
                enabled = nombre.isNotBlank() && (isEdit || clave.isNotBlank())
            ) { Text("Guardar") }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Cancelar") }
        }
    )
}
