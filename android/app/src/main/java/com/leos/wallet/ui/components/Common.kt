package com.leos.wallet.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.math.BigDecimal
import java.text.NumberFormat
import java.util.Locale

/** The primary action of a screen. */
@Composable
fun LeosButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    busy: Boolean = false,
) {
    Button(
        onClick = onClick,
        modifier = modifier
            .fillMaxWidth()
            .height(54.dp),
        enabled = enabled && !busy,
        shape = RoundedCornerShape(16.dp),
        colors = ButtonDefaults.buttonColors(
            containerColor = MaterialTheme.colorScheme.primary,
            contentColor = MaterialTheme.colorScheme.onPrimary,
        ),
    ) {
        if (busy) {
            CircularProgressIndicator(
                modifier = Modifier.size(22.dp),
                strokeWidth = 2.dp,
                color = MaterialTheme.colorScheme.onPrimary,
            )
        } else {
            Text(text, fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
        }
    }
}

/** A labelled text field. */
@Composable
fun LeosField(
    value: String,
    onValueChange: (String) -> Unit,
    label: String,
    modifier: Modifier = Modifier,
    placeholder: String? = null,
    keyboardType: KeyboardType = KeyboardType.Text,
    singleLine: Boolean = true,
    isError: Boolean = false,
    supportingText: String? = null,
    trailing: @Composable (() -> Unit)? = null,
) {
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        label = { Text(label) },
        placeholder = placeholder?.let { { Text(it) } },
        modifier = modifier.fillMaxWidth(),
        singleLine = singleLine,
        isError = isError,
        supportingText = supportingText?.let { { Text(it) } },
        trailingIcon = trailing,
        shape = RoundedCornerShape(14.dp),
        keyboardOptions = KeyboardOptions(keyboardType = keyboardType),
    )
}

/** A field for an amount in Leos Credits. */
@Composable
fun AmountField(
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    label: String = "Amount",
    isError: Boolean = false,
    supportingText: String? = null,
) {
    LeosField(
        value = value,
        onValueChange = { raw ->
            // Only digits and a single decimal point are allowed through.
            val filtered = raw.filter { it.isDigit() || it == '.' }
            val parts = filtered.split(".")
            val cleaned = when (parts.size) {
                0 -> ""
                1 -> parts[0]
                else -> "${parts[0]}.${parts[1].take(2)}"
            }
            onValueChange(cleaned)
        },
        label = label,
        modifier = modifier,
        placeholder = "0.00",
        keyboardType = KeyboardType.Decimal,
        isError = isError,
        supportingText = supportingText,
        trailing = {
            Text(
                CURRENCY_SYMBOL,
                style = MaterialTheme.typography.titleMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        },
    )
}

/** A field for a mobile number. */
@Composable
fun PhoneField(
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    label: String = "Mobile number",
    isError: Boolean = false,
    supportingText: String? = null,
) {
    LeosField(
        value = value,
        onValueChange = { raw -> onValueChange(raw.filter { it.isDigit() || it == '+' }) },
        label = label,
        modifier = modifier,
        placeholder = "917 123 4567",
        keyboardType = KeyboardType.Phone,
        isError = isError,
        supportingText = supportingText,
    )
}

/** A field for the wallet PIN. */
@Composable
fun PinField(
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    label: String = "PIN",
    isError: Boolean = false,
    supportingText: String? = null,
) {
    LeosField(
        value = value,
        onValueChange = { raw -> onValueChange(raw.filter { it.isDigit() }.take(6)) },
        label = label,
        modifier = modifier,
        keyboardType = KeyboardType.NumberPassword,
        isError = isError,
        supportingText = supportingText,
        trailing = {
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                repeat(4) { index ->
                    Box(
                        modifier = Modifier
                            .size(10.dp)
                            .clip(CircleShape)
                            .background(
                                if (index < value.length) {
                                    MaterialTheme.colorScheme.primary
                                } else {
                                    MaterialTheme.colorScheme.surfaceVariant
                                },
                            ),
                    )
                }
            }
        },
    )
}

/** A PIN shown as dots, used on the confirmation screen. */
@Composable
fun PinDots(length: Int = 4, filled: Int = 0, modifier: Modifier = Modifier) {
    Row(
        modifier = modifier,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        repeat(length) { index ->
            Box(
                modifier = Modifier
                    .size(14.dp)
                    .clip(CircleShape)
                    .background(
                        if (index < filled) {
                            MaterialTheme.colorScheme.primary
                        } else {
                            MaterialTheme.colorScheme.surfaceVariant
                        },
                    ),
            )
        }
    }
}

/** A banner showing an error or a confirmation. */
@Composable
fun MessageBanner(
    message: String,
    isError: Boolean,
    modifier: Modifier = Modifier,
    onDismiss: (() -> Unit)? = null,
) {
    val background =
        if (isError) MaterialTheme.colorScheme.errorContainer else MaterialTheme.colorScheme.primaryContainer
    val foreground =
        if (isError) MaterialTheme.colorScheme.onErrorContainer else MaterialTheme.colorScheme.onPrimaryContainer

    Row(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(14.dp))
            .background(background)
            .padding(14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            message,
            modifier = Modifier.weight(1f),
            color = foreground,
            style = MaterialTheme.typography.bodyMedium,
        )
        if (onDismiss != null) {
            Spacer(Modifier.width(8.dp))
            Text(
                "Dismiss",
                modifier = Modifier
                    .clip(RoundedCornerShape(8.dp))
                    .padding(horizontal = 8.dp, vertical = 4.dp),
                color = foreground,
                style = MaterialTheme.typography.labelLarge,
            )
        }
    }
}

/** A titled card used to group content. */
@Composable
fun SectionCard(
    title: String,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    Column(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(20.dp))
            .background(MaterialTheme.colorScheme.surface)
            .border(
                width = 1.dp,
                color = MaterialTheme.colorScheme.surfaceVariant,
                shape = RoundedCornerShape(20.dp),
            )
            .padding(18.dp),
    ) {
        Text(
            title,
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.SemiBold,
            color = MaterialTheme.colorScheme.onSurface,
        )
        Spacer(Modifier.height(12.dp))
        content()
    }
}

/**
 * The currency of the wallet.
 *
 * Leos holds Leos Credits, a currency that exists only inside this
 * project, so the amounts on screen can never be mistaken for real money.
 * Keep this in step with `crates/leos-core/src/currency.rs`.
 */
const val CURRENCY_SYMBOL = "LC"
const val CURRENCY_NAME = "Leos Credits"
const val CURRENCY_CODE = "LCO"

/**
 * The notice that says what this is.
 *
 * Keep this in step with `crates/leos-core/src/currency.rs`.
 */
const val SIMULATION_NOTICE =
    "Leos is a simulation. It holds no real money, makes no real payments, and is not a " +
        "registered financial service."
const val SIMULATION_BADGE = "Simulation — no real money"

/** The locale the amounts are formatted in. */
private val PHILIPPINE_LOCALE = Locale("en", "PH")

/** The empty state of a list. */
@Composable
fun EmptyState(
    title: String,
    subtitle: String,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            title,
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onSurface,
        )
        Spacer(Modifier.height(6.dp))
        Text(
            subtitle,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
    }
}

/** Formats an amount for display, e.g. `LC 1,234.56`. */
fun formatPeso(amount: BigDecimal): String {
    val formatter = NumberFormat.getNumberInstance(PHILIPPINE_LOCALE).apply {
        minimumFractionDigits = 2
        maximumFractionDigits = 2
    }
    return "$CURRENCY_SYMBOL " + formatter.format(amount)
}

/** Formats a peso amount without the symbol. */
fun formatAmount(amount: BigDecimal): String {
    return NumberFormat.getNumberInstance(PHILIPPINE_LOCALE).apply {
        minimumFractionDigits = 2
        maximumFractionDigits = 2
    }.format(amount)
}

/** A neutral colour used for outgoing transactions. */
val OutgoingColor: Color = Color(0xFFB3261E)
