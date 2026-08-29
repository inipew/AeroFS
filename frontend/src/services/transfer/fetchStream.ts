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
  let loaded = 0;

  let body: BodyInit;

  if (onProgress && typeof TransformStream !== 'undefined') {
    // Wrap in a TransformStream to intercept bytes as they pass through
    const { readable, writable } = new TransformStream<Uint8Array, Uint8Array>({
      transform(chunk, controller) {
        loaded += chunk.byteLength;
        onProgress(loaded, total);
        controller.enqueue(chunk);
      },
    });

    // Pipe file stream into the transform
    file.stream().pipeTo(writable, { signal }).catch(() => {
      // AbortError is expected on cancellation — ignore
    });

    body = readable;
  } else {
    // Fallback: stream without progress tracking
    body = file.stream();
  }

  const response = await fetch(url, {
    method: 'PUT',
    body,
    signal,
    // Required for streaming request body in Chromium-based browsers
    // @ts-expect-error — duplex is not yet in TypeScript's RequestInit types
    duplex: 'half',
    headers: {
      'Content-Type': file.type || 'application/octet-stream',
      'Content-Length': String(total),
    },
  });

  if (!response.ok) {
    const errText = await response.text().catch(() => response.statusText);
    throw new Error(`Upload failed: HTTP ${response.status} — ${errText}`);
  }
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
    throw new Error(`Download failed: HTTP ${response.status}`);
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
