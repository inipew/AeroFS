import { describe, expect, it } from 'bun:test';
import { streamUpload, streamDownload } from '../src/services/transfer/fetchStream';
import { normalizeApiError } from '../src/utils/errorNormalizer';

describe('fetchStream error normalization', () => {
  it('normalizes streamUpload non-ok HTTP error with Unified Error Envelope', async () => {
    const errorPayload = {
      error: {
        code: 'CONFLICT',
        category: 'conflict',
        message: 'Destination already exists',
        retryable: false,
        user_action: 'Specify another file name',
        details: null,
      },
    };

    const originalFetch = globalThis.fetch;
    try {
      globalThis.fetch = async () => {
        return new Response(JSON.stringify(errorPayload), {
          status: 409,
          statusText: 'Conflict',
          headers: { 'Content-Type': 'application/json' },
        });
      };

      const file = new File(['sample content'], 'test.txt', { type: 'text/plain' });
      const controller = new AbortController();

      let caughtErr: any = null;
      try {
        await streamUpload('http://localhost:8080/upload', file, controller.signal);
      } catch (err) {
        caughtErr = err;
      }

      expect(caughtErr).not.toBeNull();
      expect(caughtErr.message).toBe('Destination already exists');

      // Test integration with normalizeApiError
      const norm = normalizeApiError(caughtErr);
      expect(norm.kind).toBe('conflict');
      expect(norm.code).toBe('CONFLICT');
      expect(norm.category).toBe('conflict');
      expect(norm.retryable).toBe(false);
      expect(norm.statusCode).toBe(409);
    } finally {
      globalThis.fetch = originalFetch;
    }
  });

  it('normalizes streamDownload non-ok HTTP error with Unified Error Envelope', async () => {
    const errorPayload = {
      error: {
        code: 'NOT_FOUND',
        category: 'not_found',
        message: 'Requested file does not exist',
        retryable: false,
        user_action: 'Verify resource path',
        details: null,
      },
    };

    const originalFetch = globalThis.fetch;
    try {
      globalThis.fetch = async () => {
        return new Response(JSON.stringify(errorPayload), {
          status: 404,
          statusText: 'Not Found',
          headers: { 'Content-Type': 'application/json' },
        });
      };

      const controller = new AbortController();

      let caughtErr: any = null;
      try {
        await streamDownload('http://localhost:8080/download', controller.signal);
      } catch (err) {
        caughtErr = err;
      }

      expect(caughtErr).not.toBeNull();
      expect(caughtErr.message).toBe('Requested file does not exist');

      const norm = normalizeApiError(caughtErr);
      expect(norm.kind).toBe('not_found');
      expect(norm.code).toBe('NOT_FOUND');
      expect(norm.category).toBe('not_found');
      expect(norm.statusCode).toBe(404);
    } finally {
      globalThis.fetch = originalFetch;
    }
  });

  it('resolveApiUrl correctly handles relative and absolute paths without duplicating /api/v1', async () => {
    const { resolveApiUrl } = await import('../src/api/files');

    // Absolute URL is returned as-is
    expect(resolveApiUrl('https://example.com/api/upload')).toBe('https://example.com/api/upload');
    expect(resolveApiUrl('http://127.0.0.1:8080/file')).toBe('http://127.0.0.1:8080/file');

    // Path that already includes /api/v1 must NOT be duplicated
    const resolvedSessionUrl = resolveApiUrl('/api/v1/connections/local/uploads/job-123/content');
    expect(resolvedSessionUrl).not.toContain('/api/v1/api/v1');
    expect(resolvedSessionUrl).toContain('/api/v1/connections/local/uploads/job-123/content');

    // Path that does NOT include /api/v1 gets prefixed with /api/v1
    const resolvedContentUrl = resolveApiUrl('/connections/local/files/content');
    expect(resolvedContentUrl).toContain('/api/v1/connections/local/files/content');
  });
});

