export type UploadTarget = { bucket: string; prefix: string }
export type UploadItem = { name: string; loaded: number; total: number; status: 'queued' | 'sending' | 'done' | 'error' | 'conflict' }
type Adapter = (file: File, target: UploadTarget, replace: boolean, progress: (loaded: number, total: number) => void) => Promise<unknown>

/** Owns upload ordering and captures the destination before navigation can change it. */
export class UploadQueue {
  items = $state<UploadItem[]>([])
  progress = $state<{ done: number; total: number } | null>(null)
  constructor(private upload: Adapter, private isConflict: (error: unknown) => boolean) {}
  clear() { if (!this.progress) this.items = [] }

  async send(files: File[], destination: UploadTarget, replace = false) {
    if (this.progress || !files.length) return undefined
    const target = { ...destination }
    const conflicts: File[] = []
    const errors: { file: File; error: unknown }[] = []
    let sent = 0
    this.progress = { done: 0, total: files.length }
    this.items = files.map(file => ({ name: file.name, loaded: 0, total: file.size, status: 'queued' }))
    try {
      for (const [index, file] of files.entries()) {
        const item = this.items[index]
        item.status = 'sending'
        try {
          await this.upload(file, target, replace, (loaded, total) => { item.loaded = loaded; item.total = total })
          item.status = 'done'
          item.loaded = file.size
          sent++
        } catch (error) {
          if (this.isConflict(error)) { conflicts.push(file); item.status = 'conflict' }
          else { errors.push({ file, error }); item.status = 'error' }
        }
        this.progress = { done: index + 1, total: files.length }
      }
      return { target, sent, conflicts, errors }
    } finally { this.progress = null }
  }
}
