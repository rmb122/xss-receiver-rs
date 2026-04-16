import type { Ref } from 'vue'

export function expandAllGroups(groupHeaders: Ref<Record<string, any>>) {
  Object.values(groupHeaders.value).forEach((header) => {
    if (!header.isGroupOpen(header.item)) {
      header.toggleGroup(header.item)
    }
  })
}
