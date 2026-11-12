package com.leos.wallet.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.leos.wallet.WalletUiState
import com.leos.wallet.ui.components.EmptyState

/** The filters the history can be narrowed by. */
private enum class HistoryFilter(val label: String) {
    ALL("All"),
    IN("Money in"),
    OUT("Money out"),
}

/** The full transaction history. */
@Composable
fun HistoryScreen(state: WalletUiState) {
    var filter by remember { mutableStateOf(HistoryFilter.ALL) }

    val transactions = remember(state.transactions, filter) {
        when (filter) {
            HistoryFilter.ALL -> state.transactions
            HistoryFilter.IN -> state.transactions.filter { it.incoming }
            HistoryFilter.OUT -> state.transactions.filter { !it.incoming }
        }
    }

    Column(modifier = Modifier.fillMaxSize()) {
        Column(Modifier.padding(start = 20.dp, end = 20.dp, top = 16.dp)) {
            Text(
                "History",
                style = MaterialTheme.typography.headlineSmall,
                fontWeight = FontWeight.SemiBold,
            )
            Spacer(Modifier.height(12.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                HistoryFilter.entries.forEach { option ->
                    FilterChip(
                        selected = filter == option,
                        onClick = { filter = option },
                        label = { Text(option.label) },
                    )
                }
            }
        }

        Spacer(Modifier.height(8.dp))

        if (transactions.isEmpty()) {
            EmptyState(
                title = "Nothing here yet",
                subtitle = "Transactions you make will be listed here.",
            )
        } else {
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(start = 20.dp, end = 20.dp, bottom = 24.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                items(transactions, key = { it.id }) { transaction ->
                    TransactionRow(transaction)
                }
            }
        }
    }
}
