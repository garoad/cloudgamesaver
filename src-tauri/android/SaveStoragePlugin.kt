package com.garoad.cloudgamesaver

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import androidx.activity.result.ActivityResult
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import kotlin.concurrent.thread

@InvokeArg
class SaveFileArgs {
    lateinit var treeUri: String
    var relativePath: String = ""
    var mode: String = ""
}

@TauriPlugin
class SaveStoragePlugin(private val activity: Activity) : Plugin(activity) {
    private val resolver get() = activity.contentResolver

    private data class Document(val uri: Uri, val name: String, val mime: String, val modified: Long)

    @Command
    fun pickTree(invoke: Invoke) {
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).addFlags(
            Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION or
                Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION or Intent.FLAG_GRANT_PREFIX_URI_PERMISSION
        )
        startActivityForResult(invoke, intent, "treeSelected")
    }

    @ActivityCallback
    fun treeSelected(invoke: Invoke, result: ActivityResult) {
        if (result.resultCode != Activity.RESULT_OK) {
            invoke.resolve(JSObject().put("uri", null))
            return
        }
        try {
            val data = result.data ?: error("폴더를 선택하지 않았습니다.")
            val uri = data.data ?: error("폴더를 선택하지 않았습니다.")
            val flags = data.flags and (Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
            resolver.takePersistableUriPermission(uri, flags)
            root(uri.toString())
            invoke.resolve(JSObject().put("uri", uri.toString()))
        } catch (error: Exception) {
            invoke.reject(error.message ?: "폴더 권한을 저장하지 못했습니다.")
        }
    }

    private fun root(value: String): Uri {
        val tree = Uri.parse(value)
        require(tree.scheme == "content" && DocumentsContract.isTreeUri(tree)) { "세이브 폴더를 다시 선택해주세요." }
        require(resolver.persistedUriPermissions.any { it.uri == tree && it.isReadPermission && it.isWritePermission }) {
            "세이브 폴더 접근 권한이 없습니다. 폴더를 다시 선택해주세요."
        }
        val uri = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
        require(document(uri).mime == DocumentsContract.Document.MIME_TYPE_DIR) { "세이브 폴더가 존재하지 않습니다." }
        return uri
    }

    private fun document(uri: Uri): Document {
        resolver.query(uri, arrayOf(
            DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            DocumentsContract.Document.COLUMN_MIME_TYPE,
            DocumentsContract.Document.COLUMN_LAST_MODIFIED
        ), null, null, null)?.use { cursor ->
            if (cursor.moveToFirst()) return Document(uri, cursor.getString(0), cursor.getString(1), cursor.getLong(2))
        }
        error("세이브 파일 정보를 읽지 못했습니다.")
    }

    private fun children(parent: Uri): List<Document> {
        val childrenUri = DocumentsContract.buildChildDocumentsUriUsingTree(parent, DocumentsContract.getDocumentId(parent))
        val result = mutableListOf<Document>()
        val cursor = resolver.query(childrenUri, arrayOf(
            DocumentsContract.Document.COLUMN_DOCUMENT_ID,
            DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            DocumentsContract.Document.COLUMN_MIME_TYPE,
            DocumentsContract.Document.COLUMN_LAST_MODIFIED
        ), null, null, null) ?: error("세이브 폴더 목록을 읽지 못했습니다.")
        cursor.use {
            while (it.moveToNext()) {
                val uri = DocumentsContract.buildDocumentUriUsingTree(parent, it.getString(0))
                result.add(Document(uri, it.getString(1), it.getString(2), it.getLong(3)))
            }
        }
        return result
    }

    private fun safeName(name: String) {
        require(name.isNotEmpty() && name != "." && name != ".." &&
            !name.contains('/') && !name.contains('\\') && !name.contains('\u0000')) { "안전하지 않은 세이브 파일 이름입니다." }
    }

    private fun resolve(args: SaveFileArgs, create: Boolean): Uri? {
        val parts = args.relativePath.split('/')
        parts.forEach(::safeName)
        var parent = root(args.treeUri)
        for ((index, name) in parts.withIndex()) {
            val isDirectory = index < parts.lastIndex
            val found = children(parent).firstOrNull { it.name == name }
            if (found != null) {
                require((found.mime == DocumentsContract.Document.MIME_TYPE_DIR) == isDirectory) { "세이브 파일과 폴더 이름이 충돌합니다: $name" }
                parent = found.uri
            } else if (create) {
                parent = DocumentsContract.createDocument(resolver, parent,
                    if (isDirectory) DocumentsContract.Document.MIME_TYPE_DIR else "application/octet-stream", name)
                    ?: error("세이브 파일을 생성하지 못했습니다: $name")
            } else return null
        }
        return parent
    }

    private fun runIo(invoke: Invoke, action: () -> JSObject) {
        thread(name = "save-storage") {
            try { invoke.resolve(action()) }
            catch (error: Exception) { invoke.reject(error.message ?: "세이브 폴더에 접근하지 못했습니다.") }
        }
    }

    @Command
    fun validateTree(invoke: Invoke) = runIo(invoke) {
        val args = invoke.parseArgs(SaveFileArgs::class.java)
        root(args.treeUri)
        JSObject()
    }

    @Command
    fun listFiles(invoke: Invoke) = runIo(invoke) {
        val args = invoke.parseArgs(SaveFileArgs::class.java)
        val files = JSArray()
        val visited = mutableSetOf<String>()
        fun walk(parent: Uri, prefix: String) {
            require(visited.add(parent.toString())) { "세이브 폴더가 순환 참조합니다." }
            for (child in children(parent)) {
                if (child.name.equals(".DS_Store", ignoreCase = true)) continue
                safeName(child.name)
                val relative = prefix + child.name
                if (child.mime == DocumentsContract.Document.MIME_TYPE_DIR) walk(child.uri, "$relative/")
                else files.put(JSObject().put("relativePath", relative))
            }
        }
        walk(root(args.treeUri), "")
        JSObject().put("files", files)
    }

    @Command
    fun statFile(invoke: Invoke) = runIo(invoke) {
        val args = invoke.parseArgs(SaveFileArgs::class.java)
        val uri = resolve(args, false)
        val result = JSObject().put("exists", uri != null)
        if (uri != null) result.put("modifiedMillis", document(uri).modified)
        result
    }

    @Command
    fun openFile(invoke: Invoke) = runIo(invoke) {
        val args = invoke.parseArgs(SaveFileArgs::class.java)
        require(args.mode == "r" || args.mode == "rwt") { "지원하지 않는 파일 접근 방식입니다." }
        val uri = resolve(args, args.mode == "rwt") ?: error("세이브 파일이 존재하지 않습니다.")
        val descriptor = resolver.openFileDescriptor(uri, args.mode) ?: error("세이브 파일을 열지 못했습니다.")
        // Rust is in the same process. Transfer ownership instead of copying
        // save contents through JSON or trying to convert content URIs to paths.
        val fd = descriptor.use { it.detachFd() }
        JSObject().put("fd", fd)
    }
}
