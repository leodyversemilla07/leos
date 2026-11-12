package com.leos.wallet.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.leos.wallet.Challenge
import com.leos.wallet.WalletUiState
import com.leos.wallet.WalletViewModel
import com.leos.wallet.ui.components.LeosButton
import com.leos.wallet.ui.components.SIMULATION_NOTICE
import com.leos.wallet.ui.components.LeosField
import com.leos.wallet.ui.components.MessageBanner
import com.leos.wallet.ui.components.PhoneField
import com.leos.wallet.ui.components.PinField
import kotlinx.coroutines.launch

/** The steps of making a wallet. */
private enum class OnboardingStep { PHONE, CODE, PIN }

/**
 * Makes a wallet: the mobile number, the passcode that arrives by text,
 * and the PIN that protects it.
 */
@Composable
fun OnboardingScreen(
    viewModel: WalletViewModel,
    state: WalletUiState,
) {
    var step by remember { mutableStateOf(OnboardingStep.PHONE) }
    var phone by remember { mutableStateOf("") }
    var code by remember { mutableStateOf("") }
    var pin by remember { mutableStateOf("") }
    var pinAgain by remember { mutableStateOf("") }
    var challenge by remember { mutableStateOf<Challenge?>(null) }
    var phoneError by remember { mutableStateOf<String?>(null) }
    var codeError by remember { mutableStateOf<String?>(null) }
    var pinError by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(24.dp),
    ) {
        Spacer(Modifier.height(24.dp))

        if (step != OnboardingStep.PHONE) {
            IconButton(onClick = { step = OnboardingStep.PHONE }) {
                Icon(Icons.Default.ArrowBack, contentDescription = "Back")
            }
        }

        Spacer(Modifier.height(8.dp))

        Text(
            "Leos",
            style = MaterialTheme.typography.headlineLarge,
            fontWeight = FontWeight.Bold,
            color = MaterialTheme.colorScheme.primary,
        )
        Spacer(Modifier.height(4.dp))
        Text(
            when (step) {
                OnboardingStep.PHONE -> "A wallet for everyday payments in the Philippines."
                OnboardingStep.CODE -> "We sent a six digit code to $phone."
                OnboardingStep.PIN -> "Choose a PIN to protect your wallet."
            },
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        Spacer(Modifier.height(20.dp))

        MessageBanner(message = SIMULATION_NOTICE, isError = false)

        Spacer(Modifier.height(20.dp))

        state.error?.let {
            MessageBanner(message = it, isError = true, onDismiss = viewModel::dismissMessage)
            Spacer(Modifier.height(16.dp))
        }

        when (step) {
            OnboardingStep.PHONE -> {
                PhoneField(
                    value = phone,
                    onValueChange = { phone = it; phoneError = null },
                    isError = phoneError != null,
                    supportingText = phoneError,
                )
                Spacer(Modifier.height(12.dp))
                Text(
                    "We will text you a one time code to confirm the number is yours.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(Modifier.height(24.dp))
                LeosButton(
                    text = "Send code",
                    busy = state.busy,
                    enabled = phone.isNotBlank(),
                    onClick = {
                        scope.launch {
                            val normalized = viewModel.normalizePhone(phone)
                            if (normalized == null) {
                                phoneError = "That does not look like a Philippine mobile number."
                            } else {
                                viewModel.startRegistration(normalized.first) { result ->
                                    challenge = result
                                    step = OnboardingStep.CODE
                                }
                            }
                        }
                    },
                )
            }

            OnboardingStep.CODE -> {
                LeosField(
                    value = code,
                    onValueChange = { code = it.filter { c -> c.isDigit() }.take(6); codeError = null },
                    label = "One time code",
                    placeholder = "000000",
                    keyboardType = KeyboardType.NumberPassword,
                    isError = codeError != null,
                    supportingText = codeError
                        ?: challenge?.devCode?.let { "Development code: $it" },
                )
                Spacer(Modifier.height(24.dp))
                LeosButton(
                    text = "Continue",
                    busy = state.busy,
                    enabled = code.length == 6,
                    onClick = {
                        if (code.length != 6) {
                            codeError = "The code has six digits."
                        } else {
                            step = OnboardingStep.PIN
                        }
                    },
                )
            }

            OnboardingStep.PIN -> {
                PinField(
                    value = pin,
                    onValueChange = { pin = it; pinError = null },
                    supportingText = pinError,
                )
                Spacer(Modifier.height(12.dp))
                PinField(
                    value = pinAgain,
                    onValueChange = { pinAgain = it; pinError = null },
                    label = "Confirm PIN",
                    supportingText = pinError,
                )
                Spacer(Modifier.height(24.dp))
                LeosButton(
                    text = "Create wallet",
                    busy = state.busy,
                    enabled = pin.length >= 4 && pinAgain.isNotEmpty(),
                    onClick = {
                        scope.launch {
                            val (valid, reason) = viewModel.checkPin(pin)
                            when {
                                !valid -> pinError = reason
                                pin != pinAgain -> pinError = "The two PINs do not match."
                                else -> viewModel.register(phone, code, pin) {
                                    // The wallet is open.
                                }
                            }
                        }
                    },
                )
            }
        }

        Spacer(Modifier.height(32.dp))
    }
}
