package com.leos.wallet.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AccountCircle
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.ReceiptLong
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.navigation.NavDestination.Companion.hierarchy
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.leos.wallet.WalletPhase
import com.leos.wallet.WalletUiState
import com.leos.wallet.WalletViewModel
import com.leos.wallet.ui.screens.CashScreen
import com.leos.wallet.ui.screens.HistoryScreen
import com.leos.wallet.ui.screens.HomeScreen
import com.leos.wallet.ui.screens.LockScreen
import com.leos.wallet.ui.screens.OnboardingScreen
import com.leos.wallet.ui.screens.ProfileScreen
import com.leos.wallet.ui.screens.ScanScreen
import com.leos.wallet.ui.screens.SignInScreen
import com.leos.wallet.ui.screens.TopUpScreen
import com.leos.wallet.ui.screens.TransferScreen

/** The screens of the open wallet. */
object Routes {
    /** The wallet itself. */
    const val HOME = "home"

    /** The full transaction history. */
    const val HISTORY = "history"

    /** The account settings. */
    const val PROFILE = "profile"

    /** Sending money to another wallet. */
    const val TRANSFER = "transfer"

    /** Scanning a merchant QR code. */
    const val SCAN = "scan"

    /** Cash in and cash out. */
    const val CASH = "cash"

    /** Topping up from a bank transfer. */
    const val TOPUP = "topup"
}

/**
 * The root of the app.
 *
 * The phase decides which part of the app is on screen: making a wallet,
 * signing in, unlocking, or the wallet itself.
 */
@Composable
fun LeosApp(viewModel: WalletViewModel) {
    val state = viewModel.state

    Surface(
        modifier = Modifier.fillMaxSize(),
        color = MaterialTheme.colorScheme.background,
    ) {
        when (state.phase) {
            WalletPhase.LOADING -> Box(
                modifier = Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center,
            ) {
                CircularProgressIndicator()
            }

            WalletPhase.ONBOARDING -> OnboardingScreen(viewModel, state)
            WalletPhase.SIGN_IN -> SignInScreen(viewModel, state)
            WalletPhase.LOCKED -> LockScreen(viewModel, state)
            WalletPhase.READY -> WalletNavigation(viewModel, state)
        }
    }
}

/** The navigation of the open wallet. */
@Composable
private fun WalletNavigation(
    viewModel: WalletViewModel,
    state: WalletUiState,
) {
    val navController = rememberNavController()
    val backStackEntry by navController.currentBackStackEntryAsState()
    val currentDestination = backStackEntry?.destination

    val bottomItems = listOf(
        BottomItem(Routes.HOME, "Home", Icons.Default.Home),
        BottomItem(Routes.HISTORY, "History", Icons.Default.ReceiptLong),
        BottomItem(Routes.PROFILE, "Account", Icons.Default.AccountCircle),
    )

    val showBottomBar = bottomItems.any { item ->
        currentDestination?.hierarchy?.any { it.route == item.route } == true
    }

    Scaffold(
        bottomBar = {
            if (showBottomBar) {
                NavigationBar {
                    bottomItems.forEach { item ->
                        val selected =
                            currentDestination?.hierarchy?.any { it.route == item.route } == true
                        NavigationBarItem(
                            selected = selected,
                            onClick = {
                                navController.navigate(item.route) {
                                    popUpTo(navController.graph.findStartDestination().id) {
                                        saveState = true
                                    }
                                    launchSingleTop = true
                                    restoreState = true
                                }
                            },
                            icon = { Icon(item.icon, contentDescription = item.label) },
                            label = { Text(item.label) },
                        )
                    }
                }
            }
        },
    ) { padding ->
        NavHost(
            navController = navController,
            startDestination = Routes.HOME,
            modifier = Modifier.padding(padding),
        ) {
            composable(Routes.HOME) {
                HomeScreen(
                    state = state,
                    onSend = { navController.navigate(Routes.TRANSFER) },
                    onScan = { navController.navigate(Routes.SCAN) },
                    onCashIn = { navController.navigate(Routes.CASH) },
                    onCashOut = { navController.navigate(Routes.CASH) },
                    onTopUp = { navController.navigate(Routes.TOPUP) },
                    onSeeAll = { navController.navigate(Routes.HISTORY) },
                    onProfile = { navController.navigate(Routes.PROFILE) },
                )
            }

            composable(Routes.HISTORY) {
                HistoryScreen(state = state)
            }

            composable(Routes.PROFILE) {
                ProfileScreen(
                    state = state,
                    onSignOut = viewModel::signOut,
                )
            }

            composable(Routes.TRANSFER) {
                TransferScreen(
                    state = state,
                    viewModel = viewModel,
                    onDone = { navController.popBackStack() },
                )
            }

            composable(Routes.SCAN) {
                ScanScreen(
                    state = state,
                    viewModel = viewModel,
                    onDone = { navController.popBackStack() },
                )
            }

            composable(Routes.CASH) {
                CashScreen(
                    state = state,
                    viewModel = viewModel,
                )
            }

            composable(Routes.TOPUP) {
                TopUpScreen(
                    state = state,
                    viewModel = viewModel,
                )
            }
        }
    }
}

/** One entry of the bottom navigation. */
private data class BottomItem(
    val route: String,
    val label: String,
    val icon: ImageVector,
)
