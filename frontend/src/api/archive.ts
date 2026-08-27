import { apiClient } from './client';

export interface VirtualArchiveEntry {
  name: string;
  path: string;
  kind: 'file' | 'directory';
  size: number;
  compressed_size?: number;
  modified_at?: string;
}

export interface ArchiveResponse {
  success: boolean;
  message: string;
  entries_count?: number;
}

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
  return `/api/v1/connections/${connectionId}/archive/read?${params.toString()}`;
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
  entries: string[]
): Promise<ArchiveResponse> {
  const res = await apiClient.post<ArchiveResponse>(
    `/connections/${connectionId}/archive/extract-selected`,
    {
      archive_path: archivePath,
      destination_dir: destinationDir,
      entries,
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
  format?: string
): Promise<ArchiveResponse> {
  const res = await apiClient.post<ArchiveResponse>(
    `/connections/${connectionId}/archive/extract`,
    {
      archive_path: archivePath,
      destination_dir: destinationDir,
      format,
    }
  );
  return res.data;
}
