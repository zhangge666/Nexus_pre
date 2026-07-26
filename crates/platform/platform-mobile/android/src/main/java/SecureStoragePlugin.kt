/** 本文件使用 Android Keystore 管理 AES-GCM 密钥，并将密文保存到应用私有偏好区。 */
package com.nexus.platform.mobile

import android.app.Activity
import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

private const val KEYSTORE_PROVIDER = "AndroidKeyStore"
private const val KEY_ALIAS = "nexus.platform.mobile.secure-storage.v1"
private const val PREFERENCES_NAME = "nexus_secure_storage"
private const val TRANSFORMATION = "AES/GCM/NoPadding"

@InvokeArg
class StoreArgs {
    lateinit var key: String
    lateinit var value: String
}

@InvokeArg
class KeyArgs {
    lateinit var key: String
}

@TauriPlugin
class SecureStoragePlugin(private val activity: Activity) : Plugin(activity) {
    private val preferences by lazy {
        activity.getSharedPreferences(PREFERENCES_NAME, Context.MODE_PRIVATE)
    }

    /** 写入经 Keystore AES-GCM 加密后的 Base64 数据。 */
    @Command
    fun store(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(StoreArgs::class.java)
            val cipher = Cipher.getInstance(TRANSFORMATION)
            cipher.init(Cipher.ENCRYPT_MODE, getOrCreateKey())
            val encrypted = cipher.doFinal(Base64.decode(args.value, Base64.NO_WRAP))
            val envelope = "${Base64.encodeToString(cipher.iv, Base64.NO_WRAP)}.${Base64.encodeToString(encrypted, Base64.NO_WRAP)}"
            if (!preferences.edit().putString(args.key, envelope).commit()) {
                invoke.reject("无法提交 Android 安全存储")
                return
            }
            invoke.resolve()
        } catch (error: Exception) {
            invoke.reject("Android 安全存储写入失败：${error.message}")
        }
    }

    /** 读取并解密指定条目；不存在时返回 null。 */
    @Command
    fun load(invoke: Invoke) {
        val args = try {
            invoke.parseArgs(KeyArgs::class.java)
        } catch (error: Exception) {
            invoke.reject("Android 安全存储参数无效：${error.message}")
            return
        }
        try {
            val envelope = preferences.getString(args.key, null)
            val result = JSObject()
            if (envelope == null) {
                result.put("value", null)
                invoke.resolve(result)
                return
            }
            val parts = envelope.split(".", limit = 2)
            require(parts.size == 2) { "密文封装格式无效" }
            val cipher = Cipher.getInstance(TRANSFORMATION)
            cipher.init(
                Cipher.DECRYPT_MODE,
                getOrCreateKey(),
                GCMParameterSpec(128, Base64.decode(parts[0], Base64.NO_WRAP)),
            )
            val decrypted = cipher.doFinal(Base64.decode(parts[1], Base64.NO_WRAP))
            result.put("value", Base64.encodeToString(decrypted, Base64.NO_WRAP))
            invoke.resolve(result)
        } catch (error: Exception) {
            // 锁屏凭据变化或系统使密钥失效时，旧密文已经无法恢复；清理后要求重新配对。
            preferences.edit().remove(args.key).commit()
            invoke.resolve(JSObject().apply { put("value", null) })
        }
    }

    /** 删除指定条目并返回此前是否存在。 */
    @Command
    fun delete(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(KeyArgs::class.java)
            val existed = preferences.contains(args.key)
            if (!preferences.edit().remove(args.key).commit()) {
                invoke.reject("无法提交 Android 安全存储删除操作")
                return
            }
            invoke.resolve(JSObject().apply { put("deleted", existed) })
        } catch (error: Exception) {
            invoke.reject("Android 安全存储删除失败：${error.message}")
        }
    }

    /** 读取既有 Keystore 密钥，首次运行时创建不可导出的 AES-256 密钥。 */
    private fun getOrCreateKey(): SecretKey {
        val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER).apply { load(null) }
        (keyStore.getKey(KEY_ALIAS, null) as? SecretKey)?.let { return it }

        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE_PROVIDER)
        generator.init(
            KeyGenParameterSpec.Builder(
                KEY_ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build(),
        )
        return generator.generateKey()
    }
}
