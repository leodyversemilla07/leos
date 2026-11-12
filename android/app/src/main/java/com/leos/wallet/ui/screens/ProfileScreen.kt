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
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.leos.wallet.WalletUiState
import com.leos.wallet.ui.components.MessageBanner
import com.leos.wallet.ui.components.SIMULATION_NOTICE
import com.leos.wallet.ui.components.SectionCard
import com.leos.wallet.ui.components.formatPeso

/** The account details and the way out of the wallet. */
@Composable
fun ProfileScreen(
    state: WalletUiState,
    onSignOut: () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(20.dp),
    ) {
        Text(
            "Account",
            style = MaterialTheme.typography.headlineSmall,
            fontWeight = FontWeight.SemiBold,
        )

        Spacer(Modifier.height(20.dp))

        SectionCard(title = "Your wallet") {
            Detail("Name", state.user?.displayName ?: "—")
            Detail("Mobile number", state.user?.phone ?: "—")
            Detail("Status", state.user?.status?.replaceFirstChar { it.uppercase() } ?: "—")
            Detail("Balance", formatPeso(state.balance))
        }

        Spacer(Modifier.height(20.dp))

        SectionCard(title = "About") {
            MessageBanner(message = SIMULATION_NOTICE, isError = false)
            Spacer(Modifier.height(12.dp))
            Detail("App", "Leos")
            Detail("Version", "0.1.0")
            Detail("Currency", state.currency)
            Text(
                "Your PIN never leaves this phone. It is stretched into a secret with Argon2id and only that secret is sent to Leos.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 12.dp),
            )
        }

        Spacer(Modifier.height(24.dp))

        OutlinedButton(
            onClick = onSignOut,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text("Sign out")
        }
    }
}

/** A label and a value. */
@Composable
private fun Detail(label: String, value: String) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 6.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(
            label,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            value,
            style = MaterialTheme.typography.bodyMedium,
            fontWeight = FontWeight.Medium,
        )
    }
}
