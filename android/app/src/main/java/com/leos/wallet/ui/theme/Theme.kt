package com.leos.wallet.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

/** The brand green of the wallet. */
val LeosGreen = Color(0xFF0B6E4F)
val LeosGreenDark = Color(0xFF00382A)
val LeosGreenLight = Color(0xFF58D6A8)
val LeosMint = Color(0xFFB4F1DA)
val LeosGold = Color(0xFF8A6A00)

private val LightColors = lightColorScheme(
    primary = LeosGreen,
    onPrimary = Color.White,
    primaryContainer = LeosMint,
    onPrimaryContainer = LeosGreenDark,
    secondary = Color(0xFF4C635A),
    onSecondary = Color.White,
    secondaryContainer = Color(0xFFCFE9DC),
    onSecondaryContainer = Color(0xFF082019),
    tertiary = LeosGold,
    onTertiary = Color.White,
    tertiaryContainer = Color(0xFFFFE08C),
    onTertiaryContainer = Color(0xFF2A1F00),
    background = Color(0xFFF6FBF7),
    onBackground = Color(0xFF171D1A),
    surface = Color.White,
    onSurface = Color(0xFF171D1A),
    surfaceVariant = Color(0xFFDBE5DE),
    onSurfaceVariant = Color(0xFF3F4945),
    outline = Color(0xFF6F7975),
    error = Color(0xFFBA1A1A),
    onError = Color.White,
    errorContainer = Color(0xFFFFDAD6),
    onErrorContainer = Color(0xFF410002),
)

private val DarkColors = darkColorScheme(
    primary = LeosGreenLight,
    onPrimary = Color(0xFF00382A),
    primaryContainer = LeosGreenDark,
    onPrimaryContainer = LeosMint,
    secondary = Color(0xFFB3CCBF),
    onSecondary = Color(0xFF1E352C),
    tertiary = Color(0xFFEDC448),
    onTertiary = Color(0xFF3F2E00),
    background = Color(0xFF0E1512),
    onBackground = Color(0xFFDFE4E0),
    surface = Color(0xFF151C19),
    onSurface = Color(0xFFDFE4E0),
    surfaceVariant = Color(0xFF3F4945),
    onSurfaceVariant = Color(0xFFBFC9C3),
    outline = Color(0xFF899390),
    error = Color(0xFFFFB4AB),
    onError = Color(0xFF690005),
)

/** The wallet's theme. */
@Composable
fun LeosTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    MaterialTheme(
        colorScheme = if (darkTheme) DarkColors else LightColors,
        content = content,
    )
}
