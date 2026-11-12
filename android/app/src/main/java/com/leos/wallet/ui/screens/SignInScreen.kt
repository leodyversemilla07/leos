package com.leos.wallet.ui.screens

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.leos.wallet.WalletUiState
import com.leos.wallet.WalletViewModel
import com.leos.wallet.ui.components.LeosButton
import com.leos.wallet.ui.components.MessageBanner
import com.leos.wallet.ui.components.PhoneField
import com.leos.wallet.ui.components.PinField
import kotlinx.coroutines.launch

/** Signing in to a wallet that is already on this device. */
@Composable
fun SignInScreen(
    viewModel: WalletViewModel,
    state: WalletUiState,
) {
    var phone by remember { mutableStateOf(viewModel.registeredPhone() ?: "") }
    var pin by remember { mutableStateOf("") }
    var pinError by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Spacer(Modifier.height(72.dp))

        Text(
            "Leos",
            style = MaterialTheme.typography.headlineLarge,
            fontWeight = FontWeight.Bold,
            color = MaterialTheme.colorScheme.primary,
        )
        Spacer(Modifier.height(8.dp))
        Text(
            "Sign in to your wallet",
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )

        Spacer(Modifier.height(32.dp))

        state.error?.let {
            MessageBanner(message = it, isError = true, onDismiss = viewModel::dismissMessage)
            Spacer(Modifier.height(16.dp))
        }

        PhoneField(value = phone, onValueChange = { phone = it })
        Spacer(Modifier.height(12.dp))
        PinField(
            value = pin,
            onValueChange = { pin = it; pinError = null },
            isError = pinError != null,
            supportingText = pinError,
        )

        Spacer(Modifier.height(24.dp))

        LeosButton(
            text = "Sign in",
            busy = state.busy,
            enabled = phone.isNotBlank() && pin.length >= 4,
            onClick = {
                scope.launch {
                    val (valid, reason) = viewModel.checkPin(pin)
                    if (!valid) {
                        pinError = reason
                    } else {
                        viewModel.signIn(phone, pin)
                    }
                }
            },
        )

        Spacer(Modifier.height(12.dp))

        TextButton(onClick = { viewModel.signOut() }) {
            Text("Use a different wallet")
        }
    }
}
