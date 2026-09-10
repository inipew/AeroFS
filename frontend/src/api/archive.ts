import { apiClient } from './client';
import { getApiBaseUrl } from './files';
import type { components } from './generated/openapi';

export type VirtualArchiveEntry = components['schemas']['VirtualArchiveEntry'];
export type ArchiveResponse = components['schemas']['ArchiveResponse'];
export type ArchiveOverwriteMode = components['schemas']['ArchiveOverwriteMode'];

/**
 * List virtual contents inside an archive without full extraction
 */
export async function listArchiveEntriesApi(
  connectionId: string,
  archivePath: string,
  subpath: string = ''
): Promise<VirtualArchiveEntry[]> {
  const params = new URLSearchParams({
    archive_path: archivePath,
    subpath,
  });
  const res = await apiClient.get<VirtualArchiveEntry[]>(
    `/connections/${connectionId}/archive/entries?${params.toString()}`
  );
  return res.data;
}

/**
 * Get read/stream URL for an entry inside an archive
 */
export function getArchiveEntryReadUrl(
  connectionId: string,
  archivePath: string,
  entryPath: string
): string {
  const params = new URLSearchParams({
    archive_path: archivePath,
    entry_path: entryPath,
  });
  return `${getApiBaseUrl()}/connections/${connectionId}/archive/read?${params.toString()}`;
}

/**
 * Fetch text content of an entry inside an archive (for code/text preview)
 */
export async function readArchiveEntryTextApi(
  connectionId: string,
  archivePath: string,
  entryPath: string
): Promise<string> {
  const params = new URLSearchParams({
    archive_path: archivePath,
    entry_path: entryPath,
  });
  const res = await apiClient.get<string>(
    `/connections/${connectionId}/archive/read?${params.toString()}`,
    { responseType: 'text' }
  );
  return res.data;
}

/**
 * Extract selected entries from an archive into a destination directory
 */
export async function extractSelectedArchiveApi(
  connectionId: string,
  archivePath: string,
  destinationDir: string,
  entries: string[],
  overwriteMode?: ArchiveOverwriteMode
): Promise<ArchiveResponse> {
  const res = await apiClient.post<ArchiveResponse>(
    `/connections/${connectionId}/archive/extract-selected`,
    {
      archive_path: archivePath,
      destination_dir: destinationDir,
      entries,
      overwrite_mode: overwriteMode,
    }
  );
  return res.data;
}

/**
 * Compress files into an archive
 */
export async function compressFilesApi(
  connectionId: string,
  basePath: string,
  relativePaths: string[],
  destinationFile: string,
  format?: string
): Promise<ArchiveResponse> {
  const res = await apiClient.post<ArchiveResponse>(
    `/connections/${connectionId}/archive/compress`,
    {
      base_path: basePath,
      relative_paths: relativePaths,
      destination_file: destinationFile,
      format,
    }
  );
  return res.data;
}

/**
 * Full archive extract
 */
export async function extractArchiveApi(
  connectionId: string,
  archivePath: string,
  destinationDir: string,
  format?: string,
  overwriteMode?: ArchiveOverwriteMode
): Promise<ArchiveResponse> {
  const res = await apiClient.post<ArchiveResponse>(
    `/connections/${connectionId}/archive/extract`,
    {
      archive_path: archivePath,
      destination_dir: destinationDir,
      format,
      overwrite_mode: overwriteMode,
    }
  );
  return res.data;
}
