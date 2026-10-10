import { RemoteResource } from '$lib/remote-resource.svelte'
import type { Bucket, StorageListing } from '$lib/types'
import { UploadQueue, type UploadTarget } from './upload-queue.svelte'

export type BucketInfo = { bucket: Bucket; publicOrigin: string } | null
export interface StorageBrowserAdapter {
  info(bucket: string, signal: AbortSignal): Promise<BucketInfo>
  list(target: UploadTarget, offset: number, signal: AbortSignal): Promise<StorageListing>
  remove(bucket: string, name: string): Promise<unknown>
  upload(file: File, target: UploadTarget, replace: boolean, progress: (loaded: number, total: number) => void): Promise<unknown>
  isConflict(error: unknown): boolean
}

/** Owns folder paging and mutation refreshes against captured destinations. */
export class StorageBrowser {
  readonly infoResource = new RemoteResource<BucketInfo>()
  readonly filesResource = new RemoteResource<StorageListing>()
  readonly uploads: UploadQueue
  private target?: UploadTarget

  constructor(private adapter: StorageBrowserAdapter) {
    this.uploads = new UploadQueue(
      (file, target, replace, progress) => adapter.upload(file, target, replace, progress),
      error => adapter.isConflict(error),
    )
  }

  async open(target: UploadTarget) {
    const bucketChanged = this.target?.bucket !== target.bucket
    if (this.matches(target)) return
    this.target = { ...target }
    this.filesResource.clear()
    if (bucketChanged) this.infoResource.clear()
    await Promise.all([this.load(), ...(bucketChanged ? [this.loadInfo()] : [])])
  }

  loadInfo() {
    const bucket = this.target?.bucket
    if (!bucket) return Promise.resolve(false)
    return this.infoResource.load(signal => this.adapter.info(bucket, signal))
  }

  load(more = false) {
    if (!this.target || (more && this.filesResource.loading)) return Promise.resolve(false)
    const target = { ...this.target }
    const previous = more ? this.filesResource.data : null
    const objects = [...(previous?.objects ?? [])]
    const folders = [...(previous?.folders ?? [])]
    return this.filesResource.load(async signal => {
      const page = await this.adapter.list(target, Math.max(objects.length, folders.length), signal)
      return { ...page, objects: [...objects, ...page.objects], folders: [...folders, ...page.folders] }
    })
  }

  async send(files: File[], replace = false, destination = this.target) {
    if (!destination) return undefined
    const result = await this.uploads.send(files, destination, replace)
    if (result && this.matches(result.target)) await this.refresh()
    return result
  }

  async remove(name: string) {
    if (!this.target) return
    const target = { ...this.target }
    await this.adapter.remove(target.bucket, name)
    if (this.matches(target)) await this.refresh()
  }

  private matches(target: UploadTarget) {
    return this.target?.bucket === target.bucket && this.target.prefix === target.prefix
  }
  private refresh() { return Promise.all([this.load(), this.loadInfo()]) }
  cancel() {
    this.target = undefined
    this.filesResource.cancel()
    this.infoResource.cancel()
  }
}
