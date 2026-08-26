import { ref } from 'vue'
import { chunkedUpload, FileTooLargeError, getFileBytes } from '@/api/file'
import { formatFileSize } from '@/utils/format'
import { showErrorToast, showSuccessToast } from '@/utils/toast'

interface RouteWithHandler {
  handler: string
}

interface HandlerFile {
  name: string
  bytes: Uint8Array<ArrayBuffer>
  path: string
}

export function useHandlerFileEditor() {
  const handlerEditorDialog = ref(false)
  const editingHandlerFile = ref<HandlerFile>({
    name: '',
    bytes: new Uint8Array(0),
    path: '',
  })
  const savingHandler = ref(false)

  async function openHandlerEditor(route: RouteWithHandler) {
    if (!route.handler) {
      showErrorToast('handler 路径为空')
      return
    }

    const filename = route.handler.split('/').pop() || route.handler
    let bytes: Uint8Array<ArrayBuffer>
    try {
      bytes = await getFileBytes(route.handler)
    } catch (error) {
      if (error instanceof FileTooLargeError) {
        showErrorToast(`文件过大 (${formatFileSize(error.size)}), 无法在线编辑, 请下载后查看`)
        return
      }
      throw error
    }

    editingHandlerFile.value = {
      name: filename,
      bytes,
      path: route.handler,
    }
    handlerEditorDialog.value = true
  }

  async function saveHandlerFile(bytes: Uint8Array<ArrayBuffer>, closeAfterSave: boolean) {
    savingHandler.value = true
    try {
      await chunkedUpload(editingHandlerFile.value.path, new Blob([bytes]))
      editingHandlerFile.value.bytes = bytes
      showSuccessToast('保存成功')
      if (closeAfterSave) {
        handlerEditorDialog.value = false
      }
    } finally {
      savingHandler.value = false
    }
  }

  function handleSaveHandlerFile(bytes: Uint8Array<ArrayBuffer>) {
    return saveHandlerFile(bytes, false)
  }

  function handleSaveHandlerFileAndClose(bytes: Uint8Array<ArrayBuffer>) {
    return saveHandlerFile(bytes, true)
  }

  return {
    handlerEditorDialog,
    editingHandlerFile,
    savingHandler,
    openHandlerEditor,
    handleSaveHandlerFile,
    handleSaveHandlerFileAndClose,
  }
}
