package com.leos.wallet.ui.screens

import androidx.biometric.BiometricManager
import androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_STRONG
import androidx.biometric.BiometricManager.Authenticators.DEVICE_CREDENTIAL
import androidx.biometric.BiometricPrompt
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Fingerprint
import androidx.compose.material3.Icon
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
import androidx.compose.ui.platform.LocalContext
import androidx.fragment.app.FragmentActivity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.leos.wallet.WalletUiState
import com.leos.wallet.WalletViewModel
import com.leos.wallet.ui.components.LeosButton
import com.leos.wallet.ui.components.MessageBanner
import com.leos.wallet.ui.components.PinField
import kotlinx.coroutines.launch
import java.util.concurrent.Executor

/**
 * The lock screen.
 *
 * The wallet is closed when the app opens, and it is opened either with the
 * PIN or with the fingerprint or face the phone already knows.
 */
@Composable
fun LockScreen(
    viewModel: WalletViewModel,
    state: WalletUiState,
) {
    var pin by remember { mutableStateOf("") }
    var pinError by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    val activity = context as? FragmentActivity

    val biometricManager = BiometricManager.from(context)
    val biometricAvailable = biometricManager.canAuthenticate(
        BIOMETRIC_STRONG or DEVICE_CREDENTIAL
    ) == BiometricManager.BIOMETRIC_SUCCESS

    fun unlockWithBiometrics() {
        val host = activity ?: return
        val executor: Executor = androidx.core.content.ContextCompat.getMainExecutor(context)
        val prompt = BiometricPrompt(
            host,
            executor,
            object : BiometricPrompt.AuthenticationCallback() {
                override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                    scope.launch {
                        val phone = viewModel.registeredPhone()
                        val secret = viewModel.storedPinSecret()
                        if (phone == null || secret == null) {
                            viewModel.fail("This device cannot unlock the wallet with biometrics yet.")
                        } else {
                            viewModel.unlockWithSecret(phone, secret)
                        }
                    }
                }
            },
        )

        prompt.authenticate(
            BiometricPrompt.PromptInfo.Builder()
                .setTitle("Unlock Leos")
                .setSubtitle("Confirm it is you to open your wallet")
                .setAllowedAuthenticators(BIOMETRIC_STRONG or DEVICE_CREDENTIAL)
                .build()
        )
    }

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
            "Enter your PIN to open your wallet",
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )

        Spacer(Modifier.height(32.dp))

        state.error?.let {
            MessageBanner(message = it, isError = true, onDismiss = viewModel::dismissMessage)
            Spacer(Modifier.height(16.dp))
        }

        PinField(
            value = pin,
            onValueChange = { pin = it; pinError = null },
            isError = pinError != null,
            supportingText = pinError,
        )

        Spacer(Modifier.height(24.dp))

        LeosButton(
            text = "Unlock",
            busy = state.busy,
            enabled = pin.length >= 4,
            onClick = {
                scope.launch {
                    val (valid, reason) = viewModel.checkPin(pin)
                    if (!valid) {
                        pinError = reason
                    } else {
                        viewModel.unlock(pin)
                    }
                }
            },
        )

        if (biometricAvailable) {
            Spacer(Modifier.height(8.dp))
            TextButton(onClick = { unlockWithBiometrics() }) {
                Icon(
                    Icons.Default.Fingerprint,
                    contentDescription = null,
                    modifier = Modifier.size(20.dp),
                )
                Spacer(Modifier.size(8.dp))
                Text("Use biometrics")
            }
        }
    }
}
