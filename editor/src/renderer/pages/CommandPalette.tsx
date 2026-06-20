import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from '../i18n';

// ─── Types ──────────────────────────────────────────────────────────────────

interface CommandDef {
  id: string;
  label: string;
  shortcut?: string;
  category: string;
  action?: () => void;
  disabled?: boolean;
}

interface CommandPaletteProps {
  isOpen: boolean;
  onClose: () => void;
  commands: CommandDef[];
}

const overlayClass = 'fixed inset-0 z-[10000] flex justify-center bg-black/50 pt-[15vh] backdrop-blur-[4px] animate-[fadeIn_100ms_ease]';
const paletteClass = 'flex max-h-[420px] w-[560px] flex-col overflow-hidden rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--bg-surface)] shadow-[var(--shadow-lg)] animate-[slideUp_120ms_ease]';
const searchRowClass = 'border-b border-[var(--border)] px-3 py-2.5';
const searchInputClass = 'w-full rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 py-1.5 text-sm text-[var(--text-primary)] outline-none focus:border-[var(--accent)]';
const listClass = 'flex-1 overflow-y-auto py-1';
const groupClass = 'mb-0.5';
const categoryClass = 'px-3.5 pt-1.5 pb-0.5 text-[10px] font-semibold uppercase tracking-[0.06em] text-[var(--text-muted)]';
const itemLabelClass = 'flex-1';
const shortcutClass = 'ml-4 font-[var(--font-mono)] text-[11px] text-[var(--text-muted)] group-hover:text-white/70 group-[.selected]:text-white/70';
const emptyClass = 'p-6 text-center text-[13px] text-[var(--text-muted)]';

function itemClass(selected: boolean, disabled?: boolean): string {
  return [
    'group flex w-full cursor-pointer items-center justify-between border-0 bg-transparent px-3.5 py-1.5 text-left font-[var(--font-sans)] text-[13px] text-[var(--text-primary)] hover:bg-[var(--accent)] hover:text-white',
    selected ? 'selected bg-[var(--accent)] text-white' : '',
    disabled ? 'pointer-events-none opacity-35' : '',
  ].filter(Boolean).join(' ');
}

// ─── Component ──────────────────────────────────────────────────────────────

export default function CommandPalette({ isOpen, onClose, commands }: CommandPaletteProps) {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  // Filter commands by query (case-insensitive match on label, category, or shortcut)
  const filtered = query.trim()
    ? commands.filter(c =>
        c.label.toLowerCase().includes(query.toLowerCase()) ||
        c.category.toLowerCase().includes(query.toLowerCase()) ||
        (c.shortcut && c.shortcut.toLowerCase().includes(query.toLowerCase()))
      )
    : commands;

  // Reset state on open
  useEffect(() => {
    if (isOpen) {
      setQuery('');
      setSelectedIndex(0);
      // Focus input after a tick so the DOM is ready
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [isOpen]);

  // Clamp selected index to filtered range
  const safeIndex = Math.min(selectedIndex, Math.max(0, filtered.length - 1));

  // Scroll selected item into view
  useEffect(() => {
    if (!listRef.current) return;
    const items = listRef.current.querySelectorAll('[data-command-palette-item]');
    const selected = items[safeIndex] as HTMLElement | undefined;
    if (selected) {
      selected.scrollIntoView({ block: 'nearest' });
    }
  }, [safeIndex, filtered.length]);

  // Keyboard navigation
  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        setSelectedIndex(i => Math.min(i + 1, filtered.length - 1));
        break;
      case 'ArrowUp':
        e.preventDefault();
        setSelectedIndex(i => Math.max(i - 1, 0));
        break;
      case 'Enter':
        e.preventDefault();
        if (filtered[safeIndex]?.action) {
          filtered[safeIndex].action!();
          onClose();
        }
        break;
      case 'Escape':
        e.preventDefault();
        onClose();
        break;
    }
  }, [filtered, safeIndex, onClose]);

  // Execute on click
  const handleItemClick = useCallback((cmd: CommandDef) => {
    cmd.action?.();
    onClose();
  }, [onClose]);

  if (!isOpen) return null;

  // Group filtered commands by category
  const grouped: Record<string, CommandDef[]> = {};
  for (const cmd of filtered) {
    const cat = cmd.category;
    if (!grouped[cat]) grouped[cat] = [];
    grouped[cat].push(cmd);
  }

  let itemIndex = -1;

  return (
    <div className={overlayClass} onClick={onClose}>
      <div className={paletteClass} onClick={e => e.stopPropagation()}>
        <div className={searchRowClass}>
          <input
            ref={inputRef}
            className={searchInputClass}
            type="text"
            placeholder={t('cmd_search_placeholder')}
            value={query}
            onChange={e => {
              setQuery(e.target.value);
              setSelectedIndex(0);
            }}
            onKeyDown={handleKeyDown}
          />
        </div>
        <div ref={listRef} className={listClass}>
          {Object.entries(grouped).map(([category, items]) => (
            <div key={category} className={groupClass}>
              <div className={categoryClass}>{category}</div>
              {items.map(cmd => {
                itemIndex++;
                const idx = itemIndex;
                return (
                  <button
                    key={cmd.id}
                    className={itemClass(idx === safeIndex, cmd.disabled)}
                    data-command-palette-item
                    disabled={cmd.disabled}
                    onMouseEnter={() => setSelectedIndex(idx)}
                    onClick={() => handleItemClick(cmd)}
                  >
                    <span className={itemLabelClass}>{cmd.label}</span>
                    {cmd.shortcut && (
                      <span className={shortcutClass}>{cmd.shortcut}</span>
                    )}
                  </button>
                );
              })}
            </div>
          ))}
          {filtered.length === 0 && (
            <p className={emptyClass}>{t('cmd_no_results')}</p>
          )}
        </div>
      </div>
    </div>
  );
}

// ─── Default command definitions ────────────────────────────────────────────

export function buildDefaultCommands(t: (key: string) => string, actions: {
  handleUndo?: () => void;
  handleRedo?: () => void;
  handleSaveScene?: () => void;
  handleSaveSceneAs?: () => void;
  handleOpenScene?: () => void;
  handleCloseProject?: () => void;
  handleCreateEmpty?: () => void;
  handleCreateCamera?: () => void;
  handleCreateLight?: () => void;
  handleToggleHierarchy?: () => void;
  handleToggleInspector?: () => void;
  handleToggleConsole?: () => void;
  handleToggleProject?: () => void;
  handleImportAsset?: () => void;
  handleAbout?: () => void;
  handleKeyboardShortcuts?: () => void;
  handleDocumentation?: () => void;
  handleReportIssue?: () => void;
}, hasProject: boolean): CommandDef[] {
  return [
    // File
    { id: 'open-scene',     label: t('menu_open_scene'),    shortcut: 'Ctrl+O',       category: t('menu_file'),       action: actions.handleOpenScene,     disabled: !hasProject },
    { id: 'save',           label: t('menu_save'),           shortcut: 'Ctrl+S',       category: t('menu_file'),       action: actions.handleSaveScene,     disabled: !hasProject },
    { id: 'save-as',        label: t('menu_save_as'),        shortcut: 'Ctrl+Shift+S', category: t('menu_file'),       action: actions.handleSaveSceneAs,   disabled: !hasProject },
    { id: 'close-project',  label: t('menu_close_project'),  shortcut: '',             category: t('menu_file'),       action: actions.handleCloseProject,  disabled: !hasProject },
    // Edit
    { id: 'undo',           label: t('menu_undo'),           shortcut: 'Ctrl+Z',       category: t('menu_edit'),       action: actions.handleUndo },
    { id: 'redo',           label: t('menu_redo'),           shortcut: 'Ctrl+Y',       category: t('menu_edit'),       action: actions.handleRedo },
    // GameObject
    { id: 'create-empty',   label: t('menu_create_empty'),   shortcut: 'Ctrl+Shift+N', category: t('menu_gameobject'), action: actions.handleCreateEmpty,   disabled: !hasProject },
    { id: 'create-camera',  label: t('menu_create_camera'),  shortcut: '',             category: t('menu_gameobject'), action: actions.handleCreateCamera,  disabled: !hasProject },
    { id: 'create-light',   label: t('menu_create_light'),   shortcut: '',             category: t('menu_gameobject'), action: actions.handleCreateLight,   disabled: !hasProject },
    // View
    { id: 'toggle-hierarchy',  label: t('menu_toggle_hierarchy'),  shortcut: '', category: t('menu_window'), action: actions.handleToggleHierarchy,  disabled: !hasProject },
    { id: 'toggle-inspector',  label: t('menu_toggle_inspector'),  shortcut: '', category: t('menu_window'), action: actions.handleToggleInspector,  disabled: !hasProject },
    { id: 'toggle-console',    label: t('menu_toggle_console'),    shortcut: '', category: t('menu_window'), action: actions.handleToggleConsole,    disabled: !hasProject },
    { id: 'toggle-project',    label: t('menu_toggle_project'),    shortcut: '', category: t('menu_window'), action: actions.handleToggleProject,    disabled: !hasProject },
    // Assets
    { id: 'import-asset',   label: t('menu_import_asset'),   shortcut: '', category: t('menu_assets'), action: actions.handleImportAsset, disabled: !hasProject },
    // Help
    { id: 'command-palette', label: 'Command Palette',        shortcut: 'Ctrl+Shift+K', category: t('menu_help'), action: actions.handleKeyboardShortcuts },
    { id: 'about',           label: t('menu_about'),          shortcut: '',             category: t('menu_help'), action: actions.handleAbout },
    { id: 'documentation',   label: t('menu_documentation'),  shortcut: '',             category: t('menu_help'), action: actions.handleDocumentation },
    { id: 'report-issue',    label: t('menu_report_issue'),   shortcut: '',             category: t('menu_help'), action: actions.handleReportIssue },
  ];
}
