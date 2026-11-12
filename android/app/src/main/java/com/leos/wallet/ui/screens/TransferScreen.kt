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
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.leos.wallet.WalletUiState
import com.leos.wallet.WalletViewModel
import com.leos.wallet.ui.components.AmountField
import com.leos.wallet.ui.components.LeosButton
import com.leos.wallet.ui.components.LeosField
import com.leos.wallet.ui.components.MessageBanner
import com.leos.wallet.ui.components.PhoneField
import com.leos.wallet.ui.components.PinField
import kotlinx.coroutines.launch
import java.math.BigDecimal

/** Sending money to another Leos wallet. */
@Composable
fun TransferScreen(
    state: WalletUiState,
    viewModel: WalletViewModel,
    onDone: () -> Unit,
) {
    var phone by remember { mutableStateOf("") }
    var amount by remember { mutableStateOf("") }
    var note by remember { mutableStateOf("") }
    var pin by remember { mutableStateOf("") }
    var phoneError by remember { mutableStateOf<String?>(null) }
    var amountError by remember { mutableStateOf<String?>(null) }
    var pinError by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(20.dp),
    ) {
        Text(
            "Send money",
            style = MaterialTheme.typography.headlineSmall,
            fontWeight = FontWeight.SemiBold,
        )
        Spacer(Modifier.height(4.dp))
        Text(
            "Money arrives in the recipient's wallet straight away.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        Spacer(Modifier.height(24.dp))

        state.error?.let {
            MessageBanner(message = it, isError = true, onDismiss = viewModel::dismissMessage)
            Spacer(Modifier.height(16.dp))
        }

        PhoneField(
            value = phone,
            onValueChange = { phone = it; phoneError = null },
            label = "Recipient's mobile number",
            isError = phoneError != null,
            supportingText = phoneError,
        )

        Spacer(Modifier.height(14.dp))

        AmountField(
            value = amount,
            onValueChange = { amount = it; amountError = null },
            isError = amountError != null,
            supportingText = amountError,
        )

        Spacer(Modifier.height(14.dp))

        LeosField(
            value = note,
            onValueChange = { note = it.take(80) },
            label = "Note (optional)",
            placeholder = "What is this for?",
        )

        Spacer(Modifier.height(14.dp))

        PinField(
            value = pin,
            onValueChange = { pin = it; pinError = null },
            supportingText = pinError,
        )

        Spacer(Modifier.height(24.dp))

        LeosButton(
            text = "Send",
            busy = state.busy,
            enabled = phone.isNotBlank() && amount.isNotBlank() && pin.length >= 4,
            onClick = {
                val value = amount.toBigDecimalOrNull()
                when {
                    value == null || value <= BigDecimal.ZERO ->
                        amountError = "Enter an amount greater than zero."
                    pin.length < 4 -> pinError = "Enter your PIN."
                    else -> scope.launch {
                        val (valid, reason) = viewModel.checkPin(pin)
                        if (!valid) {
                            pinError = reason
                        } else {
                            viewModel.transfer(phone, value, note.ifBlank { null }, pin, onDone)
                        }
                    }
                }
            },
        )
    }
}
