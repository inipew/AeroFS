/**
 * Universal path utility functions for VFS and UI
 */

export function normalizePath(path: string): string {
  if (!path || path === '.' || path === '') return '/';
  
  // Replace backslashes with forward slashes
  let clean = path.replace(/\\/g, '/');
  
  // Collapse multiple slashes
  clean = clean.replace(/\/+/g, '/');
  
  // Ensure leading slash
  if (!clean.startsWith('/')) {
    clean = '/' + clean;
  }
  
  // Remove trailing slash unless root
  if (clean.length > 1 && clean.endsWith('/')) {
    clean = clean.slice(0, -1);
  }
  
  return clean;
}

export function joinPath(parent: string, child: string): string {
  const cleanParent = normalizePath(parent);
  const cleanChild = child.replace(/^[/\\]+/, '').replace(/[/\\]+$/, '');
  
  if (cleanParent === '/') {
    return `/${cleanChild}`;
  }
  if (!cleanChild) {
    return cleanParent;
  }
  return `${cleanParent}/${cleanChild}`;
}

export function parentPath(path: string): string {
  const normalized = normalizePath(path);
  if (normalized === '/') return '/';
  
  const lastSlashIndex = normalized.lastIndexOf('/');
  if (lastSlashIndex <= 0) return '/';
  
  return normalized.substring(0, lastSlashIndex);
}

export function basename(path: string): string {
  const normalized = normalizePath(path);
  if (normalized === '/') return '';
  const lastSlashIndex = normalized.lastIndexOf('/');
  return normalized.substring(lastSlashIndex + 1);
}

export function dirname(path: string): string {
  return parentPath(path);
}

export function isRoot(path: string): string | boolean {
  return normalizePath(path) === '/';
}

export function getPathSegments(path: string): { name: string; path: string }[] {
  const normalized = normalizePath(path);
  if (normalized === '/') {
    return [{ name: 'Root', path: '/' }];
  }
  
  const parts = normalized.split('/').filter(Boolean);
  const segments: { name: string; path: string }[] = [{ name: 'Root', path: '/' }];
  
  let current = '';
  for (const part of parts) {
    current += '/' + part;
    segments.push({
      name: part,
      path: current,
    });
  }
  
  return segments;
}
