import { describe, expect, it, beforeEach } from 'bun:test';
import { createPinia, setActivePinia } from 'pinia';
import { generateConflictResolvedName } from '../src/utils/naming';

describe('Workspace Clipboard & Conflict Duplication', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it('generates conflict resolved duplicate name with counter', () => {
    const existing = ['report.pdf', 'report (1).pdf', 'image.png'];
    expect(generateConflictResolvedName('report.pdf', existing)).toBe('report (2).pdf');
    expect(generateConflictResolvedName('image.png', existing)).toBe('image (1).png');
    expect(generateConflictResolvedName('unknown.txt', existing)).toBe('unknown.txt');
  });

  it('differentiates between cut (no-op skip) and copy (auto-duplicate) in same directory', () => {
    function resolvePasteDestPath(opts: {
      isCut: boolean;
      sourceConnectionId: string;
      targetConnectionId: string;
      sourceFilePath: string;
      targetDir: string;
      existingFiles: string[];
    }): { skipped: boolean; destPath: string } {
      const fileName = opts.sourceFilePath.split('/').pop() || 'file';
      const initialDestPath = opts.targetDir === '/' ? `/${fileName}` : `${opts.targetDir}/${fileName}`;

      if (
        opts.isCut &&
        opts.sourceConnectionId === opts.targetConnectionId &&
        opts.sourceFilePath === initialDestPath
      ) {
        return { skipped: true, destPath: initialDestPath };
      }

      if (
        !opts.isCut &&
        opts.sourceConnectionId === opts.targetConnectionId &&
        opts.sourceFilePath === initialDestPath
      ) {
        const dupName = generateConflictResolvedName(fileName, opts.existingFiles);
        const dupDestPath = opts.targetDir === '/' ? `/${dupName}` : `${opts.targetDir}/${dupName}`;
        return { skipped: false, destPath: dupDestPath };
      }

      return { skipped: false, destPath: initialDestPath };
    }

    // Cut into same directory: skipped
    const cutResult = resolvePasteDestPath({
      isCut: true,
      sourceConnectionId: 'local',
      targetConnectionId: 'local',
      sourceFilePath: '/docs/report.pdf',
      targetDir: '/docs',
      existingFiles: ['report.pdf'],
    });
    expect(cutResult.skipped).toBe(true);

    // Copy into same directory: automatically creates duplicate file
    const copyResult = resolvePasteDestPath({
      isCut: false,
      sourceConnectionId: 'local',
      targetConnectionId: 'local',
      sourceFilePath: '/docs/report.pdf',
      targetDir: '/docs',
      existingFiles: ['report.pdf'],
    });
    expect(copyResult.skipped).toBe(false);
    expect(copyResult.destPath).toBe('/docs/report (1).pdf');
  });
});
