export interface DirectoryPage<T> {
  entries: T[];
  total_count?: number | null;
}

/** Preserve backend page ordering exactly when materializing loaded entries. */
export function flattenDirectoryPages<T>(pages?: readonly DirectoryPage<T>[]): T[] {
  return pages?.flatMap((page) => page.entries) ?? [];
}

/**
 * Directory total is query metadata, not page-local metadata. Use the first
 * page as the authoritative value so loading more pages cannot change it.
 */
export function getDirectoryTotalCount<T>(
  pages?: readonly DirectoryPage<T>[]
): number | undefined {
  const total = pages?.[0]?.total_count;
  return total == null ? undefined : total;
}

export function directoryCountLabel(loaded: number, total?: number | null): string {
  const itemLabel = (count: number) => `${count.toLocaleString()} ${count === 1 ? 'item' : 'items'}`;
  if (total == null || total <= loaded) {
    return itemLabel(total ?? loaded);
  }
  return `${loaded.toLocaleString()} loaded · ${itemLabel(total)}`;
}
