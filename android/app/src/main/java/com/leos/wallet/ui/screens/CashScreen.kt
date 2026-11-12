package com.leos.wallet.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Tab
import androidx.compose.material3.TabRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.leos.wallet.CashOut
import com.leos.wallet.WalletUiState
import com.leos.wallet.WalletViewModel
import com.leos.wallet.ui.components.AmountField
import com.leos.wallet.ui.components.LeosButton
import com.leos.wallet.ui.components.LeosField
import com.leos.wallet.ui.components.MessageBanner
import com.leos.wallet.ui.components.PinField
import com.leos.wallet.ui.components.SectionCard
import com.leos.wallet.ui.components.formatPeso
import kotlinx.coroutines.launch
import java.math.BigDecimal

/**
 * Cash in and cash out.
 *
 * Cash in works the way it does at a real agent: the customer pays the
 * agent, gets a code, and enters it here. Cash out is the other way
 * round: the app reserves the money and gives the customer a reference to
 * show at the agent.
 */
@Composable
fun CashScreen(
    state: WalletUiState,
    viewModel: WalletViewModel,
) {
    var tab by remember { mutableIntStateOf(0) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(20.dp),
    ) {
        Text(
            "Cash",
            style = MaterialTheme.typography.headlineSmall,
            fontWeight = FontWeight.SemiBold,
        )
        Spacer(Modifier.height(4.dp))
        Text(
            "Move money in and out of your wallet through a partner agent.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        Spacer(Modifier.height(16.dp))

        state.error?.let {
            MessageBanner(message = it, isError = true, onDismiss = viewModel::dismissMessage)
            Spacer(Modifier.height(12.dp))
        }

        state.notice?.let {
            MessageBanner(message = it, isError = false, onDismiss = viewModel::dismissMessage)
            Spacer(Modifier.height(12.dp))
        }

        TabRow(selectedTabIndex = tab) {
            Tab(
                selected = tab == 0,
                onClick = { tab = 0 },
                text = { Text("Cash in") },
            )
            Tab(
                selected = tab == 1,
                onClick = { tab = 1 },
                text = { Text("Cash out") },
            )
        }

        Spacer(Modifier.height(20.dp))

        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState()),
        ) {
            if (tab == 0) {
                CashInTab(state = state, viewModel = viewModel)
            } else {
                CashOutTab(state = state, viewModel = viewModel)
            }
        }
    }
}

/** Redeeming a cash in voucher. */
@Composable
private fun CashInTab(
    state: WalletUiState,
    viewModel: WalletViewModel,
) {
    var code by remember { mutableStateOf("") }
    var error by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()

    SectionCard(title = "Enter your voucher code") {
        Text(
            "Pay cash at any Leos agent and they will give you a code. Enter it here to top up your wallet.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(16.dp))

        LeosField(
            value = code,
            onValueChange = { code = it.uppercase().take(12); error = null },
            label = "Voucher code",
            placeholder = "ABCD2345",
            isError = error != null,
            supportingText = error,
        )

        Spacer(Modifier.height(16.dp))

        LeosButton(
            text = "Credit my wallet",
            busy = state.busy,
            enabled = code.isNotBlank(),
            onClick = {
                scope.launch {
                    viewModel.redeemVoucher(code) {
                        code = ""
                    }
                }
            },
        )
    }
}

/** Asking for cash. */
@Composable
private fun CashOutTab(
    state: WalletUiState,
    viewModel: WalletViewModel,
) {
    var amount by remember { mutableStateOf("") }
    var pin by remember { mutableStateOf("") }
    var amountError by remember { mutableStateOf<String?>(null) }
    var pinError by remember { mutableStateOf<String?>(null) }
    var payout by remember { mutableStateOf<CashOut?>(null) }
    val scope = rememberCoroutineScope()

    if (payout != null) {
        SectionCard(title = "Collect your cash") {
            Text(
                "Show this reference at any Leos partner agent.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(12.dp))
            Text(
                payout!!.reference,
                style = MaterialTheme.typography.headlineSmall,
                fontWeight = FontWeight.Bold,
            )
            Spacer(Modifier.height(8.dp))
            Text(
                "Amount to collect: ${formatPeso(payout!!.amount)}",
                style = MaterialTheme.typography.bodyLarge,
            )
            if (payout!!.fee > BigDecimal.ZERO) {
                Text(
                    "Service fee: ${formatPeso(payout!!.fee)}",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Spacer(Modifier.height(16.dp))
            LeosButton(text = "Done", onClick = { payout = null })
        }
        return
    }

    SectionCard(title = "How much do you need?") {
        AmountField(
            value = amount,
            onValueChange = { amount = it; amountError = null },
            isError = amountError != null,
            supportingText = amountError,
        )

        Spacer(Modifier.height(14.dp))

        PinField(
            value = pin,
            onValueChange = { pin = it; pinError = null },
            supportingText = pinError,
        )

        Spacer(Modifier.height(16.dp))

        LeosButton(
            text = "Request cash",
            busy = state.busy,
            enabled = amount.isNotBlank() && pin.length >= 4,
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
                            viewModel.cashOut(value, pin) { result -> payout = result }
                        }
                    }
                }
            },
        )

        Spacer(Modifier.height(8.dp))

        Text(
            "A service fee of LC 15.00 applies to every cash out.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
