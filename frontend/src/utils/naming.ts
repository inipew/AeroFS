/**
 * Resolves naming conflicts by appending an incrementing counter:
 * 'file.txt' -> 'file (1).txt' -> 'file (2).txt'
 */
export function generateConflictResolvedName(fileName: string, existingNames: string[]): string {
  const existingSet = new Set(existingNames);
  if (!existingSet.has(fileName)) {
    return fileName;
  }

  const dotIdx = fileName.lastIndexOf('.');
  let count = 1;
  let candidate =
    dotIdx > 0
      ? `${fileName.substring(0, dotIdx)} (${count})${fileName.substring(dotIdx)}`
      : `${fileName} (${count})`;

  while (existingSet.has(candidate)) {
    count++;
    candidate =
      dotIdx > 0
        ? `${fileName.substring(0, dotIdx)} (${count})${fileName.substring(dotIdx)}`
        : `${fileName} (${count})`;
  }

  return candidate;
}
