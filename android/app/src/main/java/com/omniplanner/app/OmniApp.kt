package com.omniplanner.app

import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.omniplanner.app.screens.*

enum class Screen(val route: String, val label: String, val icon: ImageVector) {
    Dashboard("dashboard", "Inicio", Icons.Default.Dashboard),
    Tareas("tareas", "Tareas", Icons.Default.CheckCircle),
    Agenda("agenda", "Agenda", Icons.Default.CalendarMonth),
    Presupuesto("presupuesto", "Dinero", Icons.Default.AccountBalance),
    Contras("contras", "Claves", Icons.Default.Lock),
    Memoria("memoria", "Memoria", Icons.Default.Psychology),
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun OmniApp() {
    val navController = rememberNavController()
    val current by navController.currentBackStackEntryAsState()
    val currentRoute = current?.destination?.route

    MaterialTheme(
        colorScheme = darkColorScheme()
    ) {
        Scaffold(
            topBar = {
                TopAppBar(
                    title = { Text("Omniplanner") },
                    colors = TopAppBarDefaults.topAppBarColors(
                        containerColor = MaterialTheme.colorScheme.primaryContainer
                    )
                )
            },
            bottomBar = {
                NavigationBar {
                    Screen.entries.forEach { screen ->
                        NavigationBarItem(
                            icon = { Icon(screen.icon, contentDescription = screen.label) },
                            label = { Text(screen.label) },
                            selected = currentRoute == screen.route,
                            onClick = {
                                if (currentRoute != screen.route) {
                                    navController.navigate(screen.route) {
                                        popUpTo(Screen.Dashboard.route) { saveState = true }
                                        launchSingleTop = true
                                        restoreState = true
                                    }
                                }
                            }
                        )
                    }
                }
            }
        ) { padding ->
            NavHost(
                navController = navController,
                startDestination = Screen.Dashboard.route,
                modifier = Modifier.padding(padding)
            ) {
                composable(Screen.Dashboard.route) {
                    DashboardScreen(onNavigate = { route ->
                        navController.navigate(route) {
                            popUpTo(Screen.Dashboard.route) { saveState = true }
                            launchSingleTop = true
                            restoreState = true
                        }
                    })
                }
                composable(Screen.Tareas.route) { TareasScreen() }
                composable(Screen.Agenda.route) { AgendaScreen() }
                composable(Screen.Presupuesto.route) { PresupuestoScreen() }
                composable(Screen.Contras.route) { ContrasScreen() }
                composable(Screen.Memoria.route) { MemoriaScreen() }
            }
        }
    }
}
