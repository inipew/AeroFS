import { describe, expect, it } from 'bun:test';
import type { VirtualArchiveEntry } from '../src/api/archive';

describe('Archive Viewer Invariants', () => {
  describe('Active Search Filtering', () => {
    const mockEntries: VirtualArchiveEntry[] = [
      { name: 'index.html', path: 'src/index.html', kind: 'file', size: 1024, compressed_size: 512 },
      { name: 'main.ts', path: 'src/main.ts', kind: 'file', size: 2048, compressed_size: 1024 },
      { name: 'logo.svg', path: 'assets/logo.svg', kind: 'file', size: 500, compressed_size: 250 },
      { name: 'components', path: 'src/components', kind: 'directory', size: 0, compressed_size: 0 },
    ];

    function filterEntries(entries: VirtualArchiveEntry[], query: string): VirtualArchiveEntry[] {
      const q = query.trim().toLowerCase();
      if (!q) return entries;
      return entries.filter(
        (entry) =>
          entry.name.toLowerCase().includes(q) ||
          entry.path.toLowerCase().includes(q)
      );
    }

    it('returns all entries when query is empty', () => {
      expect(filterEntries(mockEntries, '')).toHaveLength(4);
      expect(filterEntries(mockEntries, '   ')).toHaveLength(4);
    });

    it('filters entries matching name case-insensitively', () => {
      const results = filterEntries(mockEntries, 'INDEX');
      expect(results).toHaveLength(1);
      expect(results[0].name).toBe('index.html');
    });

    it('filters entries matching subpath', () => {
      const results = filterEntries(mockEntries, 'assets/');
      expect(results).toHaveLength(1);
      expect(results[0].name).toBe('logo.svg');
    });

    it('returns empty array when no entries match', () => {
      const results = filterEntries(mockEntries, 'nonexistent');
      expect(results).toHaveLength(0);
    });
  });

  describe('HTML Entity Sanitization in Search Highlighting', () => {
    function escapeHtml(str: string): string {
      return str
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#039;');
    }

    function highlightMatch(text: string, query: string): string {
      const cleanText = escapeHtml(text);
      if (!query.trim()) return cleanText;
      const cleanQuery = escapeHtml(query.trim());
      const regex = new RegExp(`(${cleanQuery.replace(/[-/\\^$*+?.()|[\]{}]/g, '\\$&')})`, 'gi');
      return cleanText.replace(regex, '<mark class="bg-amber-400/30 text-amber-900 dark:text-amber-200 rounded px-0.5 font-semibold">$1</mark>');
    }

    it('escapes dangerous HTML characters in text and query', () => {
      const dangerousText = '<img src=x onerror=alert(1)>';
      const highlighted = highlightMatch(dangerousText, 'img');
      expect(highlighted).not.toContain('<img');
      expect(highlighted).toContain('&lt;<mark class="bg-amber-400/30 text-amber-900 dark:text-amber-200 rounded px-0.5 font-semibold">img</mark>');
      expect(highlighted).toContain('&gt;');
    });

    it('escapes quotes and ampersands properly', () => {
      const text = 'test & "quotes" \'single\'';
      const clean = escapeHtml(text);
      expect(clean).toBe('test &amp; &quot;quotes&quot; &#039;single&#039;');
    });
  });

  describe('Monotonic Sequence Token (Text Preview Race Condition Guard)', () => {
    it('only commits result from the latest request token', async () => {
      let currentRequestId = 0;
      let displayedContent = '';

      async function triggerPreviewFetch(reqDelayMs: number, contentToReturn: string) {
        const reqId = ++currentRequestId;
        await new Promise((resolve) => setTimeout(resolve, reqDelayMs));
        if (reqId === currentRequestId) {
          displayedContent = contentToReturn;
        }
      }

      // Fire request 1 (slow, 50ms)
      const p1 = triggerPreviewFetch(50, 'Content 1 (Stale)');
      // Fire request 2 immediately after (fast, 10ms)
      const p2 = triggerPreviewFetch(10, 'Content 2 (Fresh)');

      await Promise.all([p1, p2]);

      // Request 1 must NOT overwrite Request 2 despite resolving later
      expect(displayedContent).toBe('Content 2 (Fresh)');
    });
  });

  describe('Path Normalization on Extract', () => {
    function computeExtractTarget(
      destinationDir: string,
      createSubfolder: boolean,
      archiveName: string
    ): { cleanDest: string; target: string } {
      const cleanDest = destinationDir.trim().replace(/\/+$/, '') || '/';
      let target = cleanDest;
      if (createSubfolder) {
        const subName = archiveName.replace(/\.(zip|tar\.gz|tgz|tar\.bz2|tar\.xz|tar|7z|rar)$/i, '');
        target = target === '/' ? `/${subName}` : `${target}/${subName}`;
      }
      return { cleanDest, target };
    }

    it('normalizes trailing slashes properly', () => {
      const res = computeExtractTarget('/var/www/html///', false, 'bundle.tar.gz');
      expect(res.cleanDest).toBe('/var/www/html');
      expect(res.target).toBe('/var/www/html');
    });

    it('normalizes root path cleanly without subfolder', () => {
      const res = computeExtractTarget('///', false, 'test.zip');
      expect(res.cleanDest).toBe('/');
      expect(res.target).toBe('/');
    });

    it('appends subfolder stripping archive extensions correctly', () => {
      const res1 = computeExtractTarget('/downloads/', true, 'my-archive.tar.gz');
      expect(res1.target).toBe('/downloads/my-archive');

      const res2 = computeExtractTarget('/', true, 'project.zip');
      expect(res2.target).toBe('/project');

      const res3 = computeExtractTarget('/data', true, 'backup.tar.xz');
      expect(res3.target).toBe('/data/backup');
    });
  });

  describe('Compression Metrics & Ratio Calculation', () => {
    function calculateMetrics(entries: { size?: number; compressed_size?: number }[]) {
      const totalUnpackedSize = entries.reduce((acc, e) => acc + (e.size || 0), 0);
      const totalCompressedSize = entries.reduce((acc, e) => acc + (e.compressed_size || e.size || 0), 0);
      let ratio = 0;
      if (totalUnpackedSize > 0 && totalCompressedSize < totalUnpackedSize) {
        ratio = Math.round(((totalUnpackedSize - totalCompressedSize) / totalUnpackedSize) * 100);
      }
      return { totalUnpackedSize, totalCompressedSize, ratio };
    }

    it('computes 50% ratio when compressed size is half', () => {
      const entries = [
        { size: 1000, compressed_size: 500 },
        { size: 2000, compressed_size: 1000 },
      ];
      const res = calculateMetrics(entries);
      expect(res.totalUnpackedSize).toBe(3000);
      expect(res.totalCompressedSize).toBe(1500);
      expect(res.ratio).toBe(50);
    });

    it('returns 0% ratio when compressed size is greater than or equal to uncompressed', () => {
      const entries = [
        { size: 100, compressed_size: 120 },
      ];
      const res = calculateMetrics(entries);
      expect(res.ratio).toBe(0);
    });
  });
});
