package com.omniplanner.app

import kotlinx.serialization.json.*

/**
 * Puente JNI hacia libomniplanner.so (Rust FFI).
 * Toda la comunicación es JSON string in → JSON string out.
 */
object OmniBridge {

    init {
        System.loadLibrary("omniplanner")
    }

    // ── JNI nativas ──────────────────────────────────────────
    private external fun omni_command(jsonRequest: String): String

    // ── API pública ──────────────────────────────────────────

    data class Response(
        val ok: Boolean,
        val data: JsonElement? = null,
        val error: String? = null
    )

    fun call(action: String, params: Map<String, JsonElement> = emptyMap()): Response {
        val request = buildJsonObject {
            put("action", action)
            put("params", JsonObject(params))
        }.toString()

        val raw = omni_command(request)
        val json = Json.parseToJsonElement(raw).jsonObject

        return Response(
            ok = json["ok"]?.jsonPrimitive?.boolean ?: false,
            data = json["data"],
            error = json["error"]?.jsonPrimitive?.contentOrNull
        )
    }

    // ── Helpers tipados ──────────────────────────────────────

    fun init(dataDir: String): Response =
        call("init", mapOf("data_dir" to JsonPrimitive(dataDir)))

    fun guardar(): Response = call("guardar")

    fun dashboard(): Response = call("dashboard")

    // Tareas
    fun tareasListar(): Response = call("tareas_listar")

    fun tareaCrear(titulo: String, descripcion: String = "", fecha: String = "",
                   prioridad: String = "media"): Response =
        call("tarea_crear", mapOf(
            "titulo" to JsonPrimitive(titulo),
            "descripcion" to JsonPrimitive(descripcion),
            "fecha" to JsonPrimitive(fecha),
            "prioridad" to JsonPrimitive(prioridad)
        ))

    fun tareaActualizar(id: String, estado: String? = null, titulo: String? = null): Response {
        val p = mutableMapOf<String, JsonElement>("id" to JsonPrimitive(id))
        estado?.let { p["estado"] = JsonPrimitive(it) }
        titulo?.let { p["titulo"] = JsonPrimitive(it) }
        return call("tarea_actualizar", p)
    }

    fun tareaEliminar(id: String): Response =
        call("tarea_eliminar", mapOf("id" to JsonPrimitive(id)))

    // Agenda
    fun agendaHoy(): Response = call("agenda_hoy")

    fun agendaMes(mes: Int? = null, anio: Int? = null): Response {
        val p = mutableMapOf<String, JsonElement>()
        mes?.let { p["mes"] = JsonPrimitive(it) }
        anio?.let { p["anio"] = JsonPrimitive(it) }
        return call("agenda_mes", p)
    }

    fun eventoCrear(titulo: String, fecha: String, hora: String = "09:00",
                    tipo: String = "recordatorio"): Response =
        call("evento_crear", mapOf(
            "titulo" to JsonPrimitive(titulo),
            "fecha" to JsonPrimitive(fecha),
            "hora" to JsonPrimitive(hora),
            "tipo" to JsonPrimitive(tipo)
        ))

    fun eventoEliminar(id: String): Response =
        call("evento_eliminar", mapOf("id" to JsonPrimitive(id)))

    fun eventoActualizar(id: String, titulo: String? = null, fecha: String? = null,
                         hora: String? = null, descripcion: String? = null): Response {
        val p = mutableMapOf<String, JsonElement>("id" to JsonPrimitive(id))
        titulo?.let { p["titulo"] = JsonPrimitive(it) }
        fecha?.let { p["fecha"] = JsonPrimitive(it) }
        hora?.let { p["hora"] = JsonPrimitive(it) }
        descripcion?.let { p["descripcion"] = JsonPrimitive(it) }
        return call("evento_actualizar", p)
    }

    // Presupuesto
    fun presupuestoResumen(): Response = call("presupuesto_resumen")

    fun presupuestoAgregar(nombre: String, monto: Double, categoria: String,
                           mes: String = "", pagado: Boolean = false,
                           fechaLimite: String = "", notas: String = "",
                           saldoTotalDeuda: Double? = null): Response {
        val p = mutableMapOf<String, JsonElement>(
            "nombre" to JsonPrimitive(nombre),
            "monto" to JsonPrimitive(monto),
            "categoria" to JsonPrimitive(categoria),
            "pagado" to JsonPrimitive(pagado),
            "fecha_limite" to JsonPrimitive(fechaLimite),
            "notas" to JsonPrimitive(notas)
        )
        if (mes.isNotEmpty()) p["mes"] = JsonPrimitive(mes)
        saldoTotalDeuda?.let { p["saldo_total_deuda"] = JsonPrimitive(it) }
        return call("presupuesto_agregar", p)
    }

    fun presupuestoDetalle(mes: String): Response =
        call("presupuesto_detalle", mapOf("mes" to JsonPrimitive(mes)))

    fun presupuestoActualizarLinea(mes: String, indice: Int,
                                    nombre: String? = null, monto: Double? = null,
                                    categoria: String? = null, pagado: Boolean? = null,
                                    fechaLimite: String? = null, notas: String? = null,
                                    saldoTotalDeuda: Double? = null): Response {
        val p = mutableMapOf<String, JsonElement>(
            "mes" to JsonPrimitive(mes),
            "indice" to JsonPrimitive(indice)
        )
        nombre?.let { p["nombre"] = JsonPrimitive(it) }
        monto?.let { p["monto"] = JsonPrimitive(it) }
        categoria?.let { p["categoria"] = JsonPrimitive(it) }
        pagado?.let { p["pagado"] = JsonPrimitive(it) }
        fechaLimite?.let { p["fecha_limite"] = JsonPrimitive(it) }
        notas?.let { p["notas"] = JsonPrimitive(it) }
        saldoTotalDeuda?.let { p["saldo_total_deuda"] = JsonPrimitive(it) }
        return call("presupuesto_actualizar_linea", p)
    }

    fun presupuestoEliminarLinea(mes: String, indice: Int): Response =
        call("presupuesto_eliminar_linea", mapOf(
            "mes" to JsonPrimitive(mes),
            "indice" to JsonPrimitive(indice)
        ))

    // Contraseñas
    fun contrasListar(): Response = call("contras_listar")

    fun contrasGuardar(nombre: String, usuario: String, clave: String): Response =
        call("contras_guardar", mapOf(
            "nombre" to JsonPrimitive(nombre),
            "usuario" to JsonPrimitive(usuario),
            "clave" to JsonPrimitive(clave)
        ))

    fun contrasGenerar(longitud: Int = 20): Response =
        call("contras_generar", mapOf("longitud" to JsonPrimitive(longitud)))

    fun contrasEliminar(id: String): Response =
        call("contras_eliminar", mapOf("id" to JsonPrimitive(id)))

    fun contrasActualizar(id: String, nombre: String? = null, usuario: String? = null,
                          clave: String? = null): Response {
        val p = mutableMapOf<String, JsonElement>("id" to JsonPrimitive(id))
        nombre?.let { p["nombre"] = JsonPrimitive(it) }
        usuario?.let { p["usuario"] = JsonPrimitive(it) }
        clave?.let { p["clave"] = JsonPrimitive(it) }
        return call("contras_actualizar", p)
    }

    // Memoria
    fun memoriaListar(): Response = call("memoria_listar")

    fun memoriaAgregar(contenido: String, palabras: List<String> = emptyList()): Response =
        call("memoria_agregar", mapOf(
            "contenido" to JsonPrimitive(contenido),
            "palabras_clave" to JsonArray(palabras.map { JsonPrimitive(it) })
        ))

    fun memoriaEliminar(id: String): Response =
        call("memoria_eliminar", mapOf("id" to JsonPrimitive(id)))

    // Rastreador de Deudas
    fun deudasListar(): Response = call("deudas_listar")

    fun deudaAgregar(nombre: String, tasaAnual: Double = 0.0, pagoMinimo: Double,
                     obligatoria: Boolean = false,
                     saldoInicial: Double = 0.0,
                     enganche: Double = 0.0): Response =
        call("deuda_agregar", mapOf(
            "nombre" to JsonPrimitive(nombre),
            "tasa_anual" to JsonPrimitive(tasaAnual),
            "pago_minimo" to JsonPrimitive(pagoMinimo),
            "obligatoria" to JsonPrimitive(obligatoria),
            "saldo_inicial" to JsonPrimitive(saldoInicial),
            "enganche" to JsonPrimitive(enganche)
        ))

    fun deudaActualizar(indice: Int, nombre: String? = null, tasaAnual: Double? = null,
                        pagoMinimo: Double? = null, obligatoria: Boolean? = null,
                        activa: Boolean? = null): Response {
        val p = mutableMapOf<String, JsonElement>("indice" to JsonPrimitive(indice))
        nombre?.let { p["nombre"] = JsonPrimitive(it) }
        tasaAnual?.let { p["tasa_anual"] = JsonPrimitive(it) }
        pagoMinimo?.let { p["pago_minimo"] = JsonPrimitive(it) }
        obligatoria?.let { p["obligatoria"] = JsonPrimitive(it) }
        activa?.let { p["activa"] = JsonPrimitive(it) }
        return call("deuda_actualizar", p)
    }

    fun deudaEliminar(indice: Int): Response =
        call("deuda_eliminar", mapOf("indice" to JsonPrimitive(indice)))

    fun deudaRegistrarPago(indice: Int, pago: Double, nuevosCargos: Double = 0.0): Response =
        call("deuda_registrar_pago", mapOf(
            "indice" to JsonPrimitive(indice),
            "pago" to JsonPrimitive(pago),
            "nuevos_cargos" to JsonPrimitive(nuevosCargos)
        ))

    // Ingresos (Rastreador)
    fun ingresoAgregar(concepto: String, monto: Double, frecuencia: String = "mensual"): Response =
        call("ingreso_agregar", mapOf(
            "concepto" to JsonPrimitive(concepto),
            "monto" to JsonPrimitive(monto),
            "frecuencia" to JsonPrimitive(frecuencia)
        ))

    fun ingresoEliminar(indice: Int): Response =
        call("ingreso_eliminar", mapOf("indice" to JsonPrimitive(indice)))
}
