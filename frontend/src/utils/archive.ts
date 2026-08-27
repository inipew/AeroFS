/**
 * Utility helpers for archive format checking
 */
export function isArchiveFile(item: { name: string } | string): boolean {
  const name = typeof item === 'string' ? item.toLowerCase() : item.name.toLowerCase();
  return (
    name.endsWith('.zip') ||
    name.endsWith('.tar') ||
    name.endsWith('.tar.gz') ||
    name.endsWith('.tgz') ||
    name.endsWith('.tar.bz2') ||
    name.endsWith('.tbz2') ||
    name.endsWith('.tar.xz') ||
    name.endsWith('.txz') ||
    name.endsWith('.7z') ||
    name.endsWith('.rar') ||
    name.endsWith('.gz') ||
    name.endsWith('.bz2') ||
    name.endsWith('.xz')
  );
}

export function getArchiveExtension(name: string): string {
  const n = name.toLowerCase();
  if (n.endsWith('.tar.gz')) return 'tar.gz';
  if (n.endsWith('.tgz')) return 'tgz';
  if (n.endsWith('.tar.bz2')) return 'tar.bz2';
  if (n.endsWith('.tar.xz')) return 'tar.xz';
  return n.split('.').pop() || 'zip';
}
