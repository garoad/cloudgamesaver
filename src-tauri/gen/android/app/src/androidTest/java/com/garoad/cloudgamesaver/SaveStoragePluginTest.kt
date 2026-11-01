package com.garoad.cloudgamesaver

import android.net.Uri
import android.os.ParcelFileDescriptor
import android.provider.DocumentsContract
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.tauri.plugin.Invoke
import com.fasterxml.jackson.annotation.JsonAutoDetect
import com.fasterxml.jackson.annotation.PropertyAccessor
import com.fasterxml.jackson.databind.DeserializationFeature
import com.fasterxml.jackson.databind.ObjectMapper
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

@RunWith(AndroidJUnit4::class)
class SaveStoragePluginTest {
    @Test
    fun persistedTreeSupportsNestedBinaryWritesReadsAndSafeEnumeration() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val tree = context.contentResolver.persistedUriPermissions.firstOrNull {
            it.isReadPermission && it.isWritePermission &&
                DocumentsContract.getTreeDocumentId(it.uri) == "primary:Documents/CloudGameSaverTest"
        }?.uri ?: error("Create Documents/CloudGameSaverTest and select it in the app before running this test.")
        val testDirectory = "native-test-${System.currentTimeMillis()}"
        // Tauri exits its process when the last activity is destroyed. Keep
        // the activity alive until the instrumentation result is delivered.
        val scenario = ActivityScenario.launch(MainActivity::class.java)
        run {
            lateinit var plugin: SaveStoragePlugin
            scenario.onActivity { plugin = SaveStoragePlugin(it) }
            val relative = "$testDirectory/nested/slot.sav"
            fun args(path: String = relative, mode: String = "") = JSONObject()
                .put("treeUri", tree.toString()).put("relativePath", path).put("mode", mode)

            try {
                call(args(""), plugin::validateTree)
                assertFalse(call(args(), plugin::statFile).getBoolean("exists"))
                val original = ByteArray(8193) { (it % 256).toByte() }
                val fd = call(args(mode = "rwt"), plugin::openFile).getInt("fd")
                ParcelFileDescriptor.AutoCloseOutputStream(ParcelFileDescriptor.adoptFd(fd)).use { it.write(original) }
                val readFd = call(args(mode = "r"), plugin::openFile).getInt("fd")
                val restored = ParcelFileDescriptor.AutoCloseInputStream(ParcelFileDescriptor.adoptFd(readFd)).use { it.readBytes() }
                assertArrayEquals(original, restored)
                assertTrue(call(args(), plugin::statFile).getLong("modifiedMillis") > 0)

                // Opening with rwt must truncate an existing larger save file.
                val shortSave = byteArrayOf(0, 127, -128, -1)
                val overwriteFd = call(args(mode = "rwt"), plugin::openFile).getInt("fd")
                ParcelFileDescriptor.AutoCloseOutputStream(ParcelFileDescriptor.adoptFd(overwriteFd)).use { it.write(shortSave) }
                val rereadFd = call(args(mode = "r"), plugin::openFile).getInt("fd")
                assertArrayEquals(shortSave,
                    ParcelFileDescriptor.AutoCloseInputStream(ParcelFileDescriptor.adoptFd(rereadFd)).use { it.readBytes() })

                val ignoredFd = call(args("$testDirectory/.DS_Store", "rwt"), plugin::openFile).getInt("fd")
                ParcelFileDescriptor.adoptFd(ignoredFd).close()
                val files = call(args(""), plugin::listFiles).getJSONArray("files")
                val names = (0 until files.length()).map { files.getJSONObject(it).getString("relativePath") }
                assertTrue(names.contains(relative))
                assertFalse(names.any { it.endsWith(".DS_Store") })
                for (unsafe in listOf("../outside.sav", "$testDirectory/../../outside.sav", "/outside.sav")) {
                    assertNotNull(callError(args(unsafe), plugin::openFile))
                }
            } finally {
                val root = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
                val children = DocumentsContract.buildChildDocumentsUriUsingTree(root, DocumentsContract.getDocumentId(root))
                context.contentResolver.query(children, arrayOf(
                    DocumentsContract.Document.COLUMN_DOCUMENT_ID, DocumentsContract.Document.COLUMN_DISPLAY_NAME
                ), null, null, null)?.use { cursor ->
                    while (cursor.moveToNext()) {
                        if (cursor.getString(1) == testDirectory) {
                            DocumentsContract.deleteDocument(context.contentResolver,
                                DocumentsContract.buildDocumentUriUsingTree(tree, cursor.getString(0)))
                        }
                    }
                }
            }
        }
    }

    private fun invoke(args: JSONObject, command: (Invoke) -> Unit): Pair<Long, String> {
        val latch = CountDownLatch(1)
        var response: Pair<Long, String>? = null
        val mapper = ObjectMapper().disable(DeserializationFeature.FAIL_ON_UNKNOWN_PROPERTIES)
            .setVisibility(PropertyAccessor.FIELD, JsonAutoDetect.Visibility.ANY)
        command(Invoke(1, "storage-test", 0, 1, { callback, data ->
            response = callback to data
            latch.countDown()
        }, args.toString(), mapper))
        assertTrue("Native storage command timed out", latch.await(30, TimeUnit.SECONDS))
        return requireNotNull(response)
    }

    private fun call(args: JSONObject, command: (Invoke) -> Unit): JSONObject {
        val (callback, data) = invoke(args, command)
        assertEquals(data, 0L, callback)
        return JSONObject(data)
    }

    private fun callError(args: JSONObject, command: (Invoke) -> Unit): String? {
        val (callback, data) = invoke(args.put("mode", "rwt"), command)
        return if (callback == 1L) data else null
    }
}
