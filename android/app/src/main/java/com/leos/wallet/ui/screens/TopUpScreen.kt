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
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.leos.wallet.Topup
import com.leos.wallet.WalletUiState
import com.leos.wallet.WalletViewModel
import com.leos.wallet.ui.components.AmountField
import com.leos.wallet.ui.components.LeosButton
import com.leos.wallet.ui.components.MessageBanner
import com.leos.wallet.ui.components.SectionCard
import com.leos.wallet.ui.components.formatPeso
import kotlinx.coroutines.launch
import java.math.BigDecimal

/**
 * Topping up from a bank transfer.
 *
 * The wallet gives the customer a reference to quote to their bank. The
 * money is credited once the bank confirms the transfer.
 */
@Composable
fun TopUpScreen(
    state: WalletUiState,
    viewModel: WalletViewModel,
) {
    var amount by remember { mutableStateOf("") }
    var amountError by remember { mutableStateOf<String?>(null) }
    var pending by remember { mutableStateOf<Topup?>(null) }
    var history by remember { mutableStateOf<List<Topup>>(emptyList()) }
    val scope = rememberCoroutineScope()

    LaunchedEffect(Unit) {
        history = viewModel.topups()
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(20.dp),
    ) {
        Text(
            "Top up",
            style = MaterialTheme.typography.headlineSmall,
            fontWeight = FontWeight.SemiBold,
        )
        Spacer(Modifier.height(4.dp))
        Text(
            "Transfer from your bank using the reference below.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        Spacer(Modifier.height(20.dp))

        state.error?.let {
            MessageBanner(message = it, isError = true, onDismiss = viewModel::dismissMessage)
            Spacer(Modifier.height(12.dp))
        }

        val current = pending
        if (current != null) {
            SectionCard(title = "Transfer this amount") {
                Text(
                    "Amount",
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Text(
                    formatPeso(current.amount),
                    style = MaterialTheme.typography.headlineMedium,
                    fontWeight = FontWeight.Bold,
                )

                Spacer(Modifier.height(16.dp))

                Text(
                    "Reference",
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Text(
                    current.reference,
                    style = MaterialTheme.typography.titleLarge,
                    fontWeight = FontWeight.Bold,
                )

                Spacer(Modifier.height(16.dp))

                Text(
                    "Send the transfer to ${current.bankCode}, account ${current.bankAccount}, and quote the reference exactly. The wallet is credited as soon as the bank confirms.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )

                Spacer(Modifier.height(16.dp))

                LeosButton(text = "Start another top up", onClick = { pending = null })
            }
        } else {
            SectionCard(title = "How much are you topping up?") {
                AmountField(
                    value = amount,
                    onValueChange = { amount = it; amountError = null },
                    isError = amountError != null,
                    supportingText = amountError,
                )

                Spacer(Modifier.height(16.dp))

                LeosButton(
                    text = "Get a reference",
                    busy = state.busy,
                    enabled = amount.isNotBlank(),
                    onClick = {
                        val value = amount.toBigDecimalOrNull()
                        if (value == null || value <= BigDecimal.ZERO) {
                            amountError = "Enter an amount greater than zero."
                        } else {
                            scope.launch {
                                viewModel.createTopup(value) { topup ->
                                    pending = topup
                                    scope.launch { history = viewModel.topups() }
                                }
                            }
                        }
                    },
                )
            }
        }

        if (history.isNotEmpty()) {
            Spacer(Modifier.height(20.dp))
            Text(
                "Your top ups",
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
            )
            Spacer(Modifier.height(8.dp))

            history.forEach { topup ->
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(vertical = 10.dp),
                    verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(
                            formatPeso(topup.amount),
                            style = MaterialTheme.typography.bodyLarge,
                            fontWeight = FontWeight.Medium,
                        )
                        Text(
                            topup.reference,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    Text(
                        topup.status.replace('_', ' '),
                        style = MaterialTheme.typography.labelLarge,
                        color = if (topup.status == "credited") {
                            MaterialTheme.colorScheme.primary
                        } else {
                            MaterialTheme.colorScheme.onSurfaceVariant
                        },
                    )
                }
                HorizontalDivider()
            }
        }
    }
}
