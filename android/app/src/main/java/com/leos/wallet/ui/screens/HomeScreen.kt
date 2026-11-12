package com.leos.wallet.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AccountBalance
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.ArrowDownward
import androidx.compose.material.icons.filled.ArrowUpward
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.Payments
import androidx.compose.material.icons.filled.QrCodeScanner
import androidx.compose.material.icons.filled.Send
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.leos.wallet.Transaction
import com.leos.wallet.WalletUiState
import com.leos.wallet.ui.components.EmptyState
import com.leos.wallet.ui.components.SIMULATION_BADGE
import com.leos.wallet.ui.components.formatPeso
import java.math.BigDecimal

/** The main wallet screen. */
@Composable
fun HomeScreen(
    state: WalletUiState,
    onSend: () -> Unit,
    onScan: () -> Unit,
    onCashIn: () -> Unit,
    onCashOut: () -> Unit,
    onTopUp: () -> Unit,
    onSeeAll: () -> Unit,
    onProfile: () -> Unit,
) {
    LazyColumn(
        modifier = Modifier.fillMaxWidth(),
        contentPadding = androidx.compose.foundation.layout.PaddingValues(
            start = 20.dp, end = 20.dp, top = 16.dp, bottom = 28.dp,
        ),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        item {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(Modifier.weight(1f)) {
                    Text(
                        greeting(),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Text(
                        state.user?.displayName ?: "Your wallet",
                        style = MaterialTheme.typography.titleLarge,
                        fontWeight = FontWeight.SemiBold,
                    )
                }
                Box(
                    modifier = Modifier
                        .size(42.dp)
                        .clip(CircleShape)
                        .background(MaterialTheme.colorScheme.primaryContainer)
                        .clickable { onProfile() },
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        state.user?.displayName?.take(1)?.uppercase() ?: "L",
                        style = MaterialTheme.typography.titleMedium,
                        color = MaterialTheme.colorScheme.onPrimaryContainer,
                        fontWeight = FontWeight.Bold,
                    )
                }
            }
        }

        item {
            BalanceCard(balance = state.balance)
        }

        item {
            Text(
                SIMULATION_BADGE,
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }

        item {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                QuickAction(
                    icon = Icons.Default.Send,
                    label = "Send",
                    modifier = Modifier.weight(1f),
                    onClick = onSend,
                )
                QuickAction(
                    icon = Icons.Default.QrCodeScanner,
                    label = "Scan",
                    modifier = Modifier.weight(1f),
                    onClick = onScan,
                )
                QuickAction(
                    icon = Icons.Default.Add,
                    label = "Cash in",
                    modifier = Modifier.weight(1f),
                    onClick = onCashIn,
                )
            }
            Spacer(Modifier.height(10.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                QuickAction(
                    icon = Icons.Default.AccountBalance,
                    label = "Cash out",
                    modifier = Modifier.weight(1f),
                    onClick = onCashOut,
                )
                QuickAction(
                    icon = Icons.Default.Payments,
                    label = "Top up",
                    modifier = Modifier.weight(1f),
                    onClick = onTopUp,
                )
                QuickAction(
                    icon = Icons.Default.History,
                    label = "History",
                    modifier = Modifier.weight(1f),
                    onClick = onSeeAll,
                )
            }
        }

        item {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    "Recent activity",
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.weight(1f),
                )
                Text(
                    "See all",
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.clickable { onSeeAll() },
                )
            }
        }

        if (state.transactions.isEmpty()) {
            item {
                EmptyState(
                    title = "No activity yet",
                    subtitle = "Money you send and receive will show up here.",
                )
            }
        } else {
            items(state.transactions.take(6), key = { it.id }) { transaction ->
                TransactionRow(transaction)
            }
        }
    }
}

/** The card that shows the balance. */
@Composable
private fun BalanceCard(balance: BigDecimal) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(24.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.primary),
    ) {
        Column(Modifier.padding(24.dp)) {
            Text(
                "Available balance",
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onPrimary.copy(alpha = 0.8f),
            )
            Spacer(Modifier.height(8.dp))
            Text(
                formatPeso(balance),
                style = MaterialTheme.typography.displaySmall,
                fontWeight = FontWeight.Bold,
                color = MaterialTheme.colorScheme.onPrimary,
            )
            Spacer(Modifier.height(4.dp))
            Text(
                "Leos Credits (LCO)",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onPrimary.copy(alpha = 0.7f),
            )
        }
    }
}

/** A tile that starts one of the wallet's actions. */
@Composable
private fun QuickAction(
    icon: ImageVector,
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier
            .clip(RoundedCornerShape(18.dp))
            .background(MaterialTheme.colorScheme.surface)
            .clickable { onClick() }
            .padding(vertical = 16.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Box(
            modifier = Modifier
                .size(40.dp)
                .clip(CircleShape)
                .background(MaterialTheme.colorScheme.primaryContainer),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                icon,
                contentDescription = label,
                tint = MaterialTheme.colorScheme.onPrimaryContainer,
                modifier = Modifier.size(20.dp),
            )
        }
        Spacer(Modifier.height(8.dp))
        Text(
            label,
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurface,
        )
    }
}

/** One row of the transaction history. */
@Composable
fun TransactionRow(transaction: Transaction) {
    val incoming = transaction.incoming
    val icon = when (transaction.type) {
        "transfer" -> Icons.Default.Send
        "cash_in" -> Icons.Default.ArrowDownward
        "cash_out" -> Icons.Default.AccountBalance
        "merchant_pay" -> Icons.Default.QrCodeScanner
        "top_up" -> Icons.Default.Payments
        else -> Icons.Default.History
    }

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(16.dp))
            .background(MaterialTheme.colorScheme.surface)
            .padding(14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            modifier = Modifier
                .size(40.dp)
                .clip(CircleShape)
                .background(
                    if (incoming) {
                        MaterialTheme.colorScheme.primaryContainer
                    } else {
                        MaterialTheme.colorScheme.surfaceVariant
                    },
                ),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                icon,
                contentDescription = null,
                modifier = Modifier.size(20.dp),
                tint = if (incoming) {
                    MaterialTheme.colorScheme.onPrimaryContainer
                } else {
                    MaterialTheme.colorScheme.onSurfaceVariant
                },
            )
        }

        Spacer(Modifier.width(12.dp))

        Column(Modifier.weight(1f)) {
            Text(
                transaction.counterpartyName
                    ?: transaction.counterpartyPhone
                    ?: titleFor(transaction),
                style = MaterialTheme.typography.bodyLarge,
                fontWeight = FontWeight.Medium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                transaction.note ?: transaction.reference,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }

        Spacer(Modifier.width(8.dp))

        Column(horizontalAlignment = Alignment.End) {
            Text(
                (if (incoming) "+" else "−") + formatPeso(transaction.amount),
                style = MaterialTheme.typography.bodyLarge,
                fontWeight = FontWeight.SemiBold,
                color = if (incoming) {
                    MaterialTheme.colorScheme.primary
                } else {
                    MaterialTheme.colorScheme.onSurface
                },
            )
            if (transaction.fee > BigDecimal.ZERO) {
                Text(
                    "fee " + formatPeso(transaction.fee),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

/** The name of a transaction when there is no counterparty. */
private fun titleFor(transaction: Transaction): String {
    return when (transaction.type) {
        "transfer" -> "Transfer"
        "cash_in" -> "Cash in"
        "cash_out" -> "Cash out"
        "merchant_pay" -> "Merchant payment"
        "top_up" -> "Top up"
        else -> "Transaction"
    }
}

/** A greeting for the current time of day. */
private fun greeting(): String {
    val hour = java.util.Calendar.getInstance().get(java.util.Calendar.HOUR_OF_DAY)
    return when {
        hour < 12 -> "Good morning"
        hour < 18 -> "Good afternoon"
        else -> "Good evening"
    }
}
