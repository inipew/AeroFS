export interface FileTypeMeta {
  category: 'image' | 'video' | 'audio' | 'archive' | 'rust' | 'config' | 'database' | 'binary' | 'code' | 'pdf' | 'doc' | 'other';
  label: string;
  badgeBg: string;
  badgeText: string;
  badgeBorder: string;
  cardBg: string;
  iconBg: string;
  iconColor: string;
  symbol: string;
}

export function getFileExt(entry: { name: string }): string {
  return entry.name.split('.').pop()?.toLowerCase() || '';
}

export function isArchiveFile(entry: { name: string }): boolean {
  const name = entry.name.toLowerCase();
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

export function isTextOrCode(entry: { name: string }): boolean {
  if (isArchiveFile(entry)) return false;
  if (entry.name.startsWith('.')) return true; // All dotfiles are editable config/code/text
  const ext = getFileExt(entry);
  const textExts = [
    'txt', 'md', 'log', 'env', 'json', 'yaml', 'yml', 'toml', 'xml', 'csv', 'tsv',
    'rs', 'ts', 'js', 'jsx', 'tsx', 'vue', 'html', 'css', 'scss', 'sass', 'less',
    'py', 'sh', 'bash', 'zsh', 'fish', 'c', 'cpp', 'h', 'hpp', 'go', 'java', 'kt',
    'php', 'rb', 'pl', 'lua', 'sql', 'conf', 'cfg', 'ini', 'properties', 'dockerfile',
    'lock', 'mod', 'sum', 'gradle', 'service', 'gitignore', 'gitattributes', 'npmrc',
    'bashrc', 'profile', 'zshrc', 'vimrc', 'eslintrc', 'prettierrc'
  ];
  return textExts.includes(ext);
}

export function getFileTypeMeta(file: { name: string }): FileTypeMeta {
  const name = file.name.toLowerCase();
  const ext = getFileExt(file);

  // 1. Rust / Cargo
  if (ext === 'rs' || ext === 'rlib' || name === 'cargo.toml' || name === 'cargo.lock') {
    return {
      category: 'rust',
      label: ext === 'rlib' ? 'RLIB' : (ext === 'rs' ? 'RUST' : 'CARGO'),
      badgeBg: 'bg-orange-500/15 dark:bg-orange-500/25',
      badgeText: 'text-orange-600 dark:text-orange-400',
      badgeBorder: 'border-orange-500/30 dark:border-orange-500/40',
      cardBg: 'from-orange-500/10 to-transparent',
      iconBg: 'bg-orange-500/10 dark:bg-orange-500/20',
      iconColor: 'text-orange-500 dark:text-orange-400',
      symbol: '🦀',
    };
  }

  // 2. Database
  if (['db', 'sqlite', 'sqlite3', 'sql', 'db-wal', 'db-shm'].includes(ext) || name.endsWith('.db')) {
    return {
      category: 'database',
      label: ext.toUpperCase() || 'DB',
      badgeBg: 'bg-purple-500/15 dark:bg-purple-500/25',
      badgeText: 'text-purple-600 dark:text-purple-400',
      badgeBorder: 'border-purple-500/30 dark:border-purple-500/40',
      cardBg: 'from-purple-500/10 to-transparent',
      iconBg: 'bg-purple-500/10 dark:bg-purple-500/20',
      iconColor: 'text-purple-500 dark:text-purple-400',
      symbol: '🗄️',
    };
  }

  // 3. Config / Data
  if (['toml', 'json', 'yaml', 'yml', 'xml', 'env', 'ini', 'conf', 'cfg', 'properties'].includes(ext) || name.startsWith('.env')) {
    return {
      category: 'config',
      label: ext.toUpperCase() || 'CONF',
      badgeBg: 'bg-emerald-500/15 dark:bg-emerald-500/25',
      badgeText: 'text-emerald-600 dark:text-emerald-400',
      badgeBorder: 'border-emerald-500/30 dark:border-emerald-500/40',
      cardBg: 'from-emerald-500/10 to-transparent',
      iconBg: 'bg-emerald-500/10 dark:bg-emerald-500/20',
      iconColor: 'text-emerald-500 dark:text-emerald-400',
      symbol: '⚙️',
    };
  }

  // 4. Executables / Binaries / Linux ELF / Dynamic Libs / Object Files
  if (['d', 'o', 'so', 'a', 'dll', 'dylib', 'bin', 'exe', 'out', 'wasm'].includes(ext) || !ext) {
    return {
      category: 'binary',
      label: (ext || 'BIN').toUpperCase(),
      badgeBg: 'bg-cyan-500/15 dark:bg-cyan-500/25',
      badgeText: 'text-cyan-600 dark:text-cyan-400',
      badgeBorder: 'border-cyan-500/30 dark:border-cyan-500/40',
      cardBg: 'from-cyan-500/10 to-transparent',
      iconBg: 'bg-cyan-500/10 dark:bg-cyan-500/20',
      iconColor: 'text-cyan-500 dark:text-cyan-400',
      symbol: '⚡',
    };
  }

  // 5. Archives
  if (isArchiveFile(file)) {
    return {
      category: 'archive',
      label: ext.toUpperCase() || 'ZIP',
      badgeBg: 'bg-amber-500/15 dark:bg-amber-500/25',
      badgeText: 'text-amber-600 dark:text-amber-400',
      badgeBorder: 'border-amber-500/30 dark:border-amber-500/40',
      cardBg: 'from-amber-500/10 to-transparent',
      iconBg: 'bg-amber-500/10 dark:bg-amber-500/20',
      iconColor: 'text-amber-500 dark:text-amber-400',
      symbol: '📦',
    };
  }

  // 6. Source Code
  if (['ts', 'js', 'vue', 'jsx', 'tsx', 'py', 'go', 'c', 'cpp', 'h', 'hpp', 'java', 'kt', 'php', 'rb', 'sh', 'bash', 'css', 'html', 'scss'].includes(ext)) {
    return {
      category: 'code',
      label: ext.toUpperCase(),
      badgeBg: 'bg-blue-500/15 dark:bg-blue-500/25',
      badgeText: 'text-blue-600 dark:text-blue-400',
      badgeBorder: 'border-blue-500/30 dark:border-blue-500/40',
      cardBg: 'from-blue-500/10 to-transparent',
      iconBg: 'bg-blue-500/10 dark:bg-blue-500/20',
      iconColor: 'text-blue-500 dark:text-blue-400',
      symbol: '📄',
    };
  }

  // 7. Documents (PDF, MD, TXT, DOC)
  if (ext === 'pdf') {
    return {
      category: 'pdf',
      label: 'PDF',
      badgeBg: 'bg-red-500/15 dark:bg-red-500/25',
      badgeText: 'text-red-600 dark:text-red-400',
      badgeBorder: 'border-red-500/30 dark:border-red-500/40',
      cardBg: 'from-red-500/10 to-transparent',
      iconBg: 'bg-red-500/10 dark:bg-red-500/20',
      iconColor: 'text-red-500 dark:text-red-400',
      symbol: '📕',
    };
  }

  if (['md', 'txt', 'rtf', 'log'].includes(ext)) {
    return {
      category: 'doc',
      label: ext.toUpperCase() || 'TXT',
      badgeBg: 'bg-slate-500/15 dark:bg-slate-500/25',
      badgeText: 'text-slate-600 dark:text-slate-400',
      badgeBorder: 'border-slate-500/30 dark:border-slate-500/40',
      cardBg: 'from-slate-500/10 to-transparent',
      iconBg: 'bg-slate-500/10 dark:bg-slate-500/20',
      iconColor: 'text-slate-500 dark:text-slate-400',
      symbol: '📝',
    };
  }

  // Fallback / Other
  return {
    category: 'other',
    label: (ext || 'FILE').slice(0, 4).toUpperCase(),
    badgeBg: 'bg-gray-500/15 dark:bg-slate-700/40',
    badgeText: 'text-gray-700 dark:text-slate-300',
    badgeBorder: 'border-gray-300 dark:border-slate-700',
    cardBg: 'from-gray-500/10 to-transparent',
    iconBg: 'bg-gray-100 dark:bg-slate-800',
    iconColor: 'text-gray-500 dark:text-slate-400',
    symbol: '📄',
  };
}
