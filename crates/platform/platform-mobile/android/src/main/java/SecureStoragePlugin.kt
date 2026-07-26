/** 本文件使用 Android Keystore 管理 AES-GCM 密钥，并向 Tauri 暴露安全存储与后台同步调度入口。 */
package com.nexus.platform.mobile

import android.app.Activity
import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import androidx.work.BackoffPolicy
import androidx.work.Constraints
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkRequest
import androidx.work.workDataOf
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.security.KeyStore
import java.util.concurrent.TimeUnit
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

internal const val KEYSTORE_PROVIDER = "AndroidKeyStore"
internal const val KEY_ALIAS = "nexus.platform.mobile.secure-storage.v1"
internal const val PREFERENCES_NAME = "nexus_secure_storage"
internal const val TRANSFORMATION = "AES/GCM/NoPadding"
internal const val PERIODIC_SYNC_WORK = "nexus.orbit.e2e-periodic-sync"
internal const val IMMEDIATE_SYNC_WORK = "nexus.orbit.e2e-immediate-sync"
internal const val SETTINGS_PATH_INPUT = "settingsPath"
internal const val CACHE_PATH_INPUT = "cachePath"

@InvokeArg
class StoreArgs {
    lateinit var key: String
    lateinit var value: String
}

@InvokeArg
class KeyArgs {
    lateinit var key: String
}

@InvokeArg
class BackgroundSyncArgs {
    lateinit var settingsPath: String
    lateinit var cachePath: String
}

/**
 * 共享安全存储实现。
 *
 * Worker 只在任务执行期间取得解密后的字节，并在调用 Rust 后立即清零；Keystore 主密钥始终不可导出。
 */
internal object SecureStorageVault {
    /** 写入经 Keystore AES-GCM 加密后的原始字节。 */
    fun store(context: Context, key: String, value: ByteArray): Boolean {
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, getOrCreateKey())
        val encrypted = cipher.doFinal(value)
        val envelope =
            "${Base64.encodeToString(cipher.iv, Base64.NO_WRAP)}." +
                Base64.encodeToString(encrypted, Base64.NO_WRAP)
        return preferences(context).edit().putString(key, envelope).commit()
    }

    /** 读取并解密指定条目；密文失效时删除条目并返回空值。 */
    fun load(context: Context, key: String): ByteArray? {
        val preferences = preferences(context)
        val envelope = preferences.getString(key, null) ?: return null
        return try {
            val parts = envelope.split(".", limit = 2)
            require(parts.size == 2) { "密文封装格式无效" }
            val cipher = Cipher.getInstance(TRANSFORMATION)
            cipher.init(
                Cipher.DECRYPT_MODE,
                getOrCreateKey(),
                GCMParameterSpec(128, Base64.decode(parts[0], Base64.NO_WRAP)),
            )
            cipher.doFinal(Base64.decode(parts[1], Base64.NO_WRAP))
        } catch (_: Exception) {
            // 锁屏凭据变化或系统使密钥失效时，旧密文无法恢复，必须要求设备重新配对。
            preferences.edit().remove(key).commit()
            null
        }
    }

    /** 删除指定安全存储条目并返回此前是否存在。 */
    fun delete(context: Context, key: String): Boolean {
        val preferences = preferences(context)
        val existed = preferences.contains(key)
        check(preferences.edit().remove(key).commit()) { "无法提交 Android 安全存储删除操作" }
        return existed
    }

    /** 返回应用私有偏好区，避免安全密文被其他应用读取。 */
    private fun preferences(context: Context) =
        context.getSharedPreferences(PREFERENCES_NAME, Context.MODE_PRIVATE)

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

@TauriPlugin
class SecureStoragePlugin(private val activity: Activity) : Plugin(activity) {
    /** 写入经 Keystore AES-GCM 加密后的 Base64 数据。 */
    @Command
    fun store(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(StoreArgs::class.java)
            val value = Base64.decode(args.value, Base64.NO_WRAP)
            try {
                if (!SecureStorageVault.store(activity, args.key, value)) {
                    invoke.reject("无法提交 Android 安全存储")
                    return
                }
            } finally {
                value.fill(0)
            }
            invoke.resolve()
        } catch (error: Exception) {
            invoke.reject("Android 安全存储写入失败：${error.message}")
        }
    }

    /** 读取并解密指定条目；不存在或密钥失效时返回 null。 */
    @Command
    fun load(invoke: Invoke) {
        val args = try {
            invoke.parseArgs(KeyArgs::class.java)
        } catch (error: Exception) {
            invoke.reject("Android 安全存储参数无效：${error.message}")
            return
        }
        try {
            val decrypted = SecureStorageVault.load(activity, args.key)
            val result = JSObject()
            if (decrypted == null) {
                result.put("value", null)
            } else {
                try {
                    result.put("value", Base64.encodeToString(decrypted, Base64.NO_WRAP))
                } finally {
                    decrypted.fill(0)
                }
            }
            invoke.resolve(result)
        } catch (error: Exception) {
            invoke.reject("Android 安全存储读取失败：${error.message}")
        }
    }

    /** 删除指定条目并返回此前是否存在。 */
    @Command
    fun delete(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(KeyArgs::class.java)
            val existed = SecureStorageVault.delete(activity, args.key)
            invoke.resolve(JSObject().apply { put("deleted", existed) })
        } catch (error: Exception) {
            invoke.reject("Android 安全存储删除失败：${error.message}")
        }
    }

    /** 注册联网约束下的唯一周期任务，并用相同参数替换一次即时同步任务。 */
    @Command
    fun scheduleBackgroundSync(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(BackgroundSyncArgs::class.java)
            require(args.settingsPath.isNotBlank()) { "设置路径不能为空" }
            require(args.cachePath.isNotBlank()) { "缓存路径不能为空" }
            val input =
                workDataOf(
                    SETTINGS_PATH_INPUT to args.settingsPath,
                    CACHE_PATH_INPUT to args.cachePath,
                )
            val constraints =
                Constraints.Builder()
                    .setRequiredNetworkType(NetworkType.CONNECTED)
                    .build()
            val periodic =
                PeriodicWorkRequestBuilder<OrbitBackgroundSyncWorker>(15, TimeUnit.MINUTES)
                    .setInputData(input)
                    .setConstraints(constraints)
                    .setBackoffCriteria(
                        BackoffPolicy.EXPONENTIAL,
                        WorkRequest.MIN_BACKOFF_MILLIS,
                        TimeUnit.MILLISECONDS,
                    )
                    .build()
            val immediate =
                OneTimeWorkRequestBuilder<OrbitBackgroundSyncWorker>()
                    .setInputData(input)
                    .setConstraints(constraints)
                    .setBackoffCriteria(
                        BackoffPolicy.EXPONENTIAL,
                        WorkRequest.MIN_BACKOFF_MILLIS,
                        TimeUnit.MILLISECONDS,
                    )
                    .build()
            val workManager = WorkManager.getInstance(activity.applicationContext)
            workManager.enqueueUniquePeriodicWork(
                PERIODIC_SYNC_WORK,
                ExistingPeriodicWorkPolicy.UPDATE,
                periodic,
            )
            workManager.enqueueUniqueWork(
                IMMEDIATE_SYNC_WORK,
                ExistingWorkPolicy.REPLACE,
                immediate,
            )
            invoke.resolve()
        } catch (error: Exception) {
            invoke.reject("Android 后台同步调度失败：${error.message}")
        }
    }

    /** 取消周期任务和仍在排队的一次性任务。 */
    @Command
    fun cancelBackgroundSync(invoke: Invoke) {
        try {
            val workManager = WorkManager.getInstance(activity.applicationContext)
            workManager.cancelUniqueWork(PERIODIC_SYNC_WORK)
            workManager.cancelUniqueWork(IMMEDIATE_SYNC_WORK)
            invoke.resolve()
        } catch (error: Exception) {
            invoke.reject("Android 后台同步取消失败：${error.message}")
        }
    }
}
