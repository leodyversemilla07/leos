package com.leos.wallet

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Storage for the secrets the app keeps on the device.
 *
 * Everything here is encrypted with an AES key that lives in the Android
 * keystore and never leaves the device, so the values are safe even if the
 * preferences file is readable.
 *
 * Two of the stored values matter more than the rest:
 *
 * * the *device secret*, a random value mixed into the PIN derivation, and
 * * the *client salt*, which salts the PIN derivation as well.
 *
 * Together they mean a PIN lifted from one device cannot unlock another,
 * and the server cannot work backwards from a verifier to the PIN.
 */
class SecureStore(context: Context) {

    private val preferences =
        context.applicationContext.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)

    /** Stores a value, encrypted. */
    fun put(name: String, value: String) {
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, key())
        val encrypted = cipher.doFinal(value.toByteArray(Charsets.UTF_8))

        val payload = ByteArray(cipher.iv.size + encrypted.size)
        System.arraycopy(cipher.iv, 0, payload, 0, cipher.iv.size)
        System.arraycopy(encrypted, 0, payload, cipher.iv.size, encrypted.size)

        preferences.edit()
            .putString(name, Base64.encodeToString(payload, Base64.NO_WRAP))
            .apply()
    }

    /** Reads a value, or null when it is not there. */
    fun get(name: String): String? {
        val stored = preferences.getString(name, null) ?: return null
        return runCatching {
            val payload = Base64.decode(stored, Base64.NO_WRAP)
            val iv = payload.copyOfRange(0, GCM_IV_LENGTH)
            val body = payload.copyOfRange(GCM_IV_LENGTH, payload.size)

            val cipher = Cipher.getInstance(TRANSFORMATION)
            cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(GCM_TAG_LENGTH, iv))
            String(cipher.doFinal(body), Charsets.UTF_8)
        }.getOrNull()
    }

    /** Removes a value. */
    fun remove(name: String) {
        preferences.edit().remove(name).apply()
    }

    /** Whether a value is stored. */
    fun contains(name: String): Boolean = preferences.contains(name)

    /**
     * The random secret that binds the PIN derivation to this device.
     *
     * It is generated once and never leaves the device.
     */
    fun deviceSecret(): String {
        return get(KEY_DEVICE_SECRET) ?: generate(KEY_DEVICE_SECRET, SECRET_LENGTH)
    }

    /** The salt used when stretching the PIN. */
    fun salt(): String {
        return get(KEY_SALT) ?: generate(KEY_SALT, SALT_LENGTH)
    }

    private fun generate(name: String, length: Int): String {
        val bytes = ByteArray(length)
        java.security.SecureRandom().nextBytes(bytes)
        val value = bytes.joinToString("") { "%02x".format(it) }
        put(name, value)
        return value
    }

    private fun key(): SecretKey {
        val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER).apply { load(null) }
        (keyStore.getEntry(KEY_ALIAS, null) as? KeyStore.SecretKeyEntry)?.let {
            return it.secretKey
        }

        val generator =
            KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE_PROVIDER)
        generator.init(
            KeyGenParameterSpec.Builder(
                KEY_ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setRandomizedEncryptionRequired(true)
                .build(),
        )
        return generator.generateKey()
    }

    private companion object {
        const val PREFERENCES = "leos_secure_store"
        const val KEYSTORE_PROVIDER = "AndroidKeyStore"
        const val KEY_ALIAS = "leos_device_key"
        const val TRANSFORMATION = "AES/GCM/NoPadding"
        const val GCM_IV_LENGTH = 12
        const val GCM_TAG_LENGTH = 128
        const val SECRET_LENGTH = 32
        const val SALT_LENGTH = 16

        const val KEY_DEVICE_SECRET = "device_secret"
        const val KEY_SALT = "client_salt"
    }
}
