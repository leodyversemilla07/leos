package com.leos.wallet.ui.screens

import android.annotation.SuppressLint
import androidx.core.content.ContextCompat
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.QrCodeScanner
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.MultiFormatReader
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.common.HybridBinarizer
import com.leos.wallet.Merchant
import com.leos.wallet.WalletUiState
import com.leos.wallet.WalletViewModel
import com.leos.wallet.ui.components.AmountField
import com.leos.wallet.ui.components.LeosButton
import com.leos.wallet.ui.components.MessageBanner
import com.leos.wallet.ui.components.PinField
import com.leos.wallet.ui.components.formatPeso
import kotlinx.coroutines.launch
import java.math.BigDecimal
import java.util.concurrent.Executors

/**
 * Scanning a merchant QR code and paying it.
 *
 * The camera is decoded on the device, and the QR is read by the Rust core
 * before anything is sent to the server, so a code that is not a Leos QR
 * never reaches the network.
 */
@Composable
fun ScanScreen(
    state: WalletUiState,
    viewModel: WalletViewModel,
    onDone: () -> Unit,
) {
    var scanned by remember { mutableStateOf<String?>(null) }
    var merchant by remember { mutableStateOf<Merchant?>(null) }
    var amount by remember { mutableStateOf("") }
    var pin by remember { mutableStateOf("") }
    var amountError by remember { mutableStateOf<String?>(null) }
    var pinError by remember { mutableStateOf<String?>(null) }
    var merchants by remember { mutableStateOf<List<Merchant>>(emptyList()) }
    val scope = rememberCoroutineScope()

    LaunchedEffect(Unit) {
        merchants = viewModel.merchants()
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(20.dp),
    ) {
        Text(
            "Scan to pay",
            style = MaterialTheme.typography.headlineSmall,
            fontWeight = FontWeight.SemiBold,
        )
        Spacer(Modifier.height(4.dp))
        Text(
            "Point the camera at the merchant's QR code.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        Spacer(Modifier.height(16.dp))

        state.error?.let {
            MessageBanner(message = it, isError = true, onDismiss = viewModel::dismissMessage)
            Spacer(Modifier.height(12.dp))
        }

        if (scanned == null) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(320.dp)
                    .clip(RoundedCornerShape(20.dp))
                    .background(Color(0xFF101614)),
                contentAlignment = Alignment.Center,
            ) {
                QrScanner(
                    modifier = Modifier.fillMaxSize(),
                    onScanned = { payload ->
                        if (scanned == null) {
                            scanned = payload
                            scope.launch {
                                val parsed = viewModel.parseQr(payload)
                                if (parsed != null) {
                                    merchant = Merchant(
                                        id = parsed.merchantId ?: "",
                                        name = parsed.merchantName ?: "Merchant",
                                        city = parsed.merchantCity ?: "",
                                        qrPayload = payload,
                                    )
                                    parsed.amount?.let { amount = it }
                                }
                            }
                        }
                    },
                )
                Box(
                    modifier = Modifier
                        .size(220.dp)
                        .border(2.dp, Color.White.copy(alpha = 0.85f), RoundedCornerShape(20.dp)),
                )
            }

            Spacer(Modifier.height(20.dp))

            Text(
                "Or choose a merchant",
                style = MaterialTheme.typography.titleSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(8.dp))

            LazyColumn(
                modifier = Modifier.height(180.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                items(merchants) { item ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(14.dp))
                            .background(MaterialTheme.colorScheme.surface)
                            .clickable {
                                scanned = item.qrPayload
                                merchant = item
                            }
                            .padding(14.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Icon(
                            Icons.Default.QrCodeScanner,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.primary,
                        )
                        Spacer(Modifier.width(12.dp))
                        Column(Modifier.weight(1f)) {
                            Text(item.name, style = MaterialTheme.typography.bodyLarge)
                            Text(
                                item.city,
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                }
            }
        } else {
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

            Spacer(Modifier.height(24.dp))

            LeosButton(
                text = "Pay",
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
                                viewModel.payMerchant(scanned!!, value, pin, onDone)
                            }
                        }
                    }
                },
            )

            Spacer(Modifier.height(8.dp))

            Text(
                "Scanned: ${merchant?.name ?: "merchant"}",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.clickable { scanned = null; merchant = null; amount = "" },
            )
        }
    }
}

/**
 * The camera preview that looks for QR codes.
 *
 * Frames are decoded on a background thread and the first code that is
 * found is handed over once.
 */
@SuppressLint("UnsafeOptInUsageError")
@Composable
private fun QrScanner(
    modifier: Modifier = Modifier,
    onScanned: (String) -> Unit,
) {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    val executor = remember { Executors.newSingleThreadExecutor() }
    val found = remember { mutableStateOf(false) }
    val reader = remember {
        MultiFormatReader().apply {
            setHints(
                mapOf(
                    DecodeHintType.POSSIBLE_FORMATS to
                        listOf(BarcodeFormat.QR_CODE),
                    DecodeHintType.TRY_HARDER to true,
                )
            )
        }
    }

    DisposableEffect(Unit) {
        onDispose { executor.shutdown() }
    }

    AndroidView(
        modifier = modifier,
        factory = { previewContext ->
            val previewView = PreviewView(previewContext).apply {
                scaleType = PreviewView.ScaleType.FILL_CENTER
                implementationMode = PreviewView.ImplementationMode.COMPATIBLE
            }

            val providerFuture = ProcessCameraProvider.getInstance(previewContext)
            providerFuture.addListener({
                val provider = providerFuture.get()

                val preview = Preview.Builder().build().also {
                    it.setSurfaceProvider(previewView.surfaceProvider)
                }

                val analysis = ImageAnalysis.Builder()
                    .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                    .build()

                analysis.setAnalyzer(executor) { image ->
                    if (found.value) {
                        image.close()
                        return@setAnalyzer
                    }

                    val media = image.image
                    if (media != null) {
                        val plane = media.planes[0]
                        val buffer = plane.buffer
                        val data = ByteArray(buffer.remaining())
                        buffer.get(data)

                        val source = PlanarYUVLuminanceSource(
                            data,
                            plane.rowStride,
                            media.height,
                            0,
                            0,
                            minOf(plane.rowStride, media.width),
                            media.height,
                            false,
                        )

                        try {
                            val result = reader.decodeWithState(
                                BinaryBitmap(HybridBinarizer(source))
                            )
                            if (result != null) {
                                found.value = true
                                onScanned(result.text)
                            }
                        } catch (_: Exception) {
                            // No code in this frame.
                        } finally {
                            reader.reset()
                        }
                    }
                    image.close()
                }

                provider.unbindAll()
                provider.bindToLifecycle(
                    lifecycleOwner,
                    CameraSelector.DEFAULT_BACK_CAMERA,
                    preview,
                    analysis,
                )
            }, ContextCompat.getMainExecutor(previewContext))

            previewView
        },
    )
}
