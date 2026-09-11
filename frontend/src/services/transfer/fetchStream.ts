/**
 * fetchStream.ts — Native Fetch Streams API primitives for large file transfers.
 *
 * Used instead of Axios for:
 *   - Presigned S3 PUT uploads (ReadableStream body)
 *   - Large file downloads with byte-accurate progress tracking
 *   - Streaming archive operations
 *
 * Axios limitations that motivate this:
 *   - No native ReadableStream body support on upload
 *   - XHR-based progress less accurate for presigned PUT
 *   - `duplex: 'half'` required for streaming upload not yet supported in Axios
 */

import { normalizeFetchError } from '../../utils/errorNormalizer';

export interface ProgressCallback {
  (loaded: number, total: number): void;
}

// ── Upload ───────────────────────────────────────────────────────────────────

/**
 * Upload a File to a presigned URL using native fetch + ReadableStream.
 * Sends the file body as a streaming PUT — no full-file buffering.
 *
 * @param url       Presigned PUT URL
 * @param file      File object from <input type="file">
 * @param signal    AbortController signal for cancellation
 * @param onProgress  Called repeatedly with (loadedBytes, totalBytes)
 */
export async function streamUpload(
  url: string,
  file: File,
  signal: AbortSignal,
  onProgress?: ProgressCallback
): Promise<void> {
  const total = file.size;

  if (typeof XMLHttpRequest !== 'undefined') {
    return new Promise<void>((resolve, reject) => {
      if (signal.aborted) {
        return reject(new DOMException('The operation was aborted', 'AbortError'));
      }

      const xhr = new XMLHttpRequest();
      xhr.open('PUT', url, true);
      xhr.withCredentials = true;

      const contentType = file.type || 'application/octet-stream';
      xhr.setRequestHeader('Content-Type', contentType);

      if (onProgress) {
        xhr.upload.onprogress = (event) => {
          const loaded = event.lengthComputable ? event.loaded : event.loaded;
          const tot = event.lengthComputable ? event.total : total;
          onProgress(loaded, tot);
        };
      }

      const abortHandler = () => {
        xhr.abort();
        reject(new DOMException('The operation was aborted', 'AbortError'));
      };
      signal.addEventListener('abort', abortHandler, { once: true });

      xhr.onload = async () => {
        signal.removeEventListener('abort', abortHandler);
        if (xhr.status >= 200 && xhr.status < 300) {
          if (onProgress) onProgress(total, total);
          resolve();
        } else {
          const headers = new Headers();
          const rawHeaders = xhr.getAllResponseHeaders();
          if (rawHeaders) {
            rawHeaders.split('\r\n').forEach((line) => {
              const parts = line.split(': ');
              const key = parts.shift();
              const val = parts.join(': ');
              if (key) headers.append(key, val);
            });
          }
          const resp = new Response(xhr.responseText, {
            status: xhr.status,
            statusText: xhr.statusText,
            headers,
          });
          try {
            const norm = await normalizeFetchError(resp);
            const err = new Error(norm.message);
            (err as any).normalizedError = norm;
            (err as any).status = norm.statusCode;
            (err as any).code = norm.code;
            (err as any).category = norm.category;
            (err as any).response = {
              status: norm.statusCode,
              statusText: xhr.statusText,
              data: norm.details,
            };
            reject(err);
          } catch (e) {
            reject(e);
          }
        }
      };

      xhr.onerror = () => {
        signal.removeEventListener('abort', abortHandler);
        const err = new Error('Network error during upload');
        (err as any).status = 0;
        reject(err);
      };

      xhr.ontimeout = () => {
        signal.removeEventListener('abort', abortHandler);
        const err = new Error('Upload request timed out');
        (err as any).status = 0;
        reject(err);
      };

      xhr.send(file);
    });
  }

  // Fallback for non-browser environments (e.g. Bun/Node test runners)
  const response = await fetch(url, {
    method: 'PUT',
    body: file,
    signal,
    credentials: 'include',
    headers: {
      'Content-Type': file.type || 'application/octet-stream',
    },
  });

  if (!response.ok) {
    const norm = await normalizeFetchError(response);
    const err = new Error(norm.message);
    (err as any).normalizedError = norm;
    (err as any).status = norm.statusCode;
    (err as any).code = norm.code;
    (err as any).category = norm.category;
    (err as any).response = {
      status: norm.statusCode,
      statusText: response.statusText,
      data: norm.details,
    };
    throw err;
  }

  if (onProgress) onProgress(total, total);
}

// ── Download ─────────────────────────────────────────────────────────────────

/**
 * Download a file from a URL using native fetch streaming.
 * Returns a Blob progressively, calling onProgress as bytes arrive.
 *
 * @param url       Direct download URL or presigned GET URL
 * @param signal    AbortController signal for cancellation
 * @param onProgress  Called repeatedly with (loadedBytes, totalBytes or 0 if unknown)
 * @returns         Blob of the downloaded file
 */
export async function streamDownload(
  url: string,
  signal: AbortSignal,
  onProgress?: ProgressCallback
): Promise<Blob> {
  const response = await fetch(url, { signal });

  if (!response.ok) {
    const norm = await normalizeFetchError(response);
    const err = new Error(norm.message);
    (err as any).normalizedError = norm;
    (err as any).status = norm.statusCode;
    (err as any).code = norm.code;
    (err as any).category = norm.category;
    (err as any).response = {
      status: norm.statusCode,
      statusText: response.statusText,
      data: norm.details,
    };
    throw err;
  }

  const contentLength = response.headers.get('Content-Length');
  const total = contentLength ? parseInt(contentLength, 10) : 0;
  let loaded = 0;

  const reader = response.body?.getReader();
  if (!reader) {
    // Fallback for environments without streaming body
    return response.blob();
  }

  const chunks: Uint8Array<ArrayBuffer>[] = [];

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    if (value) {
      // Copy into a plain ArrayBuffer (not SharedArrayBuffer) for Blob compatibility
      const ab = value.buffer.slice(value.byteOffset, value.byteOffset + value.byteLength) as ArrayBuffer;
      chunks.push(new Uint8Array(ab));
      loaded += value.byteLength;
      onProgress?.(loaded, total);
    }
  }

  return new Blob(chunks);
}

/**
 * Trigger a browser download from a Blob, with a given filename.
 * Works cross-browser without requiring an anchor element in the DOM.
 */
export function triggerBlobDownload(blob: Blob, filename: string): void {
  const objectUrl = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = objectUrl;
  anchor.download = filename;
  anchor.style.display = 'none';
  document.body.appendChild(anchor);
  anchor.click();
  document.body.removeChild(anchor);
  // Revoke after a short delay so the download initiates
  setTimeout(() => URL.revokeObjectURL(objectUrl), 5000);
}
