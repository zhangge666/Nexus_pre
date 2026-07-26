/** 本文件实现无需启动 Activity 或 WebView 的 Android WorkManager 端到端增量同步入口。 */
package com.nexus.platform.mobile

import android.content.Context
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.File
import java.nio.charset.StandardCharsets

private const val REMOTE_TOKEN_STORAGE_KEY = "orbit.remote-access-token"
private const val CACHE_KEY_STORAGE_KEY = "orbit.offline-cache-key"
private const val ROOT_KEY_STORAGE_KEY = "orbit.e2e-root-key"
private const val DEVICE_ID_STORAGE_KEY = "orbit.e2e-device-id"
private const val DEVICE_IDENTITY_STORAGE_KEY = "orbit.e2e-device-identity"
private const val E2E_SYNC_MODE = "e2e_cloud"

/**
 * 由 WorkManager 唤醒 Rust 增量同步。
 *
 * Kotlin 只读取调度参数和 Keystore 信封，不实现协议、签名、加密信封或版本合并。
 */
class OrbitBackgroundSyncWorker(
    appContext: Context,
    parameters: WorkerParameters,
) : CoroutineWorker(appContext, parameters) {
    /** 在联网约束满足后执行一次同步；临时失败交由 WorkManager 指数退避。 */
    override suspend fun doWork(): Result =
        withContext(Dispatchers.IO) {
            val settingsPath = inputData.getString(SETTINGS_PATH_INPUT)
            val cachePath = inputData.getString(CACHE_PATH_INPUT)
            if (settingsPath.isNullOrBlank() || cachePath.isNullOrBlank()) {
                return@withContext Result.failure()
            }
            val endpoint = readRelayEndpoint(settingsPath) ?: return@withContext Result.success()
            val secrets =
                listOf(
                    SecureStorageVault.load(applicationContext, REMOTE_TOKEN_STORAGE_KEY),
                    SecureStorageVault.load(applicationContext, CACHE_KEY_STORAGE_KEY),
                    SecureStorageVault.load(applicationContext, ROOT_KEY_STORAGE_KEY),
                    SecureStorageVault.load(applicationContext, DEVICE_ID_STORAGE_KEY),
                    SecureStorageVault.load(applicationContext, DEVICE_IDENTITY_STORAGE_KEY),
                )
            if (secrets.any { it == null }) {
                secrets.filterNotNull().forEach { it.fill(0) }
                return@withContext Result.success()
            }
            val token = checkNotNull(secrets[0])
            val cacheKey = checkNotNull(secrets[1])
            val rootKey = checkNotNull(secrets[2])
            val deviceId = checkNotNull(secrets[3])
            val identity = checkNotNull(secrets[4])

            try {
                val synced =
                    BackgroundSyncNative.runSync(
                        endpoint,
                        token,
                        cachePath,
                        cacheKey,
                        rootKey,
                        String(deviceId, StandardCharsets.UTF_8),
                        identity,
                    )
                if (synced) Result.success() else Result.retry()
            } catch (_: Throwable) {
                Result.retry()
            } finally {
                token.fill(0)
                cacheKey.fill(0)
                rootKey.fill(0)
                deviceId.fill(0)
                identity.fill(0)
            }
        }

    /** 从非敏感设置文件读取已启用的 E2E Relay 地址。 */
    private fun readRelayEndpoint(settingsPath: String): String? {
        return try {
            val sync = JSONObject(File(settingsPath).readText()).optJSONObject("sync") ?: return null
            if (sync.optString("mode") != E2E_SYNC_MODE) {
                return null
            }
            sync.optString("relayEndpoint").trim().ifEmpty { null }
        } catch (_: Exception) {
            null
        }
    }
}

/** 加载 Orbit Rust 动态库并暴露不依赖 Tauri Activity 的 JNI 同步函数。 */
private object BackgroundSyncNative {
    init {
        System.loadLibrary("orbit_app_lib")
    }

    /** 返回本轮上传、拉取和游标确认是否全部成功。 */
    @JvmStatic
    external fun runSync(
        endpoint: String,
        token: ByteArray,
        cachePath: String,
        cacheKey: ByteArray,
        rootKey: ByteArray,
        deviceId: String,
        identityPkcs8: ByteArray,
    ): Boolean
}
