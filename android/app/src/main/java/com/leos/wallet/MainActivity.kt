package com.leos.wallet

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import com.leos.wallet.ui.LeosApp
import com.leos.wallet.ui.theme.LeosTheme

/**
 * The single activity of the wallet.
 *
 * Everything the user sees is Compose, and the wallet's behaviour lives in
 * the Rust core underneath.
 */
class MainActivity : ComponentActivity() {

    private val viewModel: WalletViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        setContent {
            LeosTheme {
                LeosApp(viewModel)
            }
        }
    }
}
