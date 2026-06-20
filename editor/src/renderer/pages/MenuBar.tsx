import React, { useCallback, useEffect, useRef, useState } from 'react';
import { rpc } from '../api';
import { useTranslation } from '../i18n';
import {
  contextMenuClass,
  contextMenuItemClass,
  toolButtonClass,
  toolbarExtrasClass,
  toolbarGroupClass,
  toolbarGroupRelativeClass,
  toolbarSeparatorClass,
  toolbarSelectClass,
} from '../uiClasses';
import {
  IconSave, IconUndo, IconRedo, IconPlay, IconMove, IconRotate, IconScale, IconView,
  IconX, IconChevronDown, IconChevronRight, IconPlus, IconMenu,
} from '../icons';

// ─── Shared dropdown hook ──────────────────────────────────────────────────

function useDropdown() {
  const [openMenu, setOpenMenu] = useState<string | null>(null);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!openMenu) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpenMenu(null);
      }
    };
    window.addEventListener('mousedown', handler);
    return () => window.removeEventListener('mousedown', handler);
  }, [openMenu]);

  return { openMenu, setOpenMenu, ref };
}

// ─── Menu Item types ───────────────────────────────────────────────────────

interface MenuItem {
  label?: string;
  shortcut?: string;
  disabled?: boolean;
  action?: () => void;
  divider?: boolean;
  submenu?: MenuItem[];
}

interface MenuDef {
  label: string;
  items: MenuItem[];
}

const cx = (...classes: Array<string | false | null | undefined>) => classes.filter(Boolean).join(' ');

const menuBarClass =
  'flex h-[26px] min-h-[26px] select-none items-center border-b border-[var(--border)] bg-[var(--bg-surface)] px-1 z-50';

const menuClass = 'relative';

const menuTriggerClass = (active: boolean) =>
  cx(
    'cursor-pointer rounded-[3px] border-0 bg-transparent px-2.5 py-[3px] font-sans text-xs text-[var(--text-secondary)] transition-[background,color] duration-[var(--transition-fast)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]',
    active && 'bg-[var(--bg-hover)] text-[var(--text-primary)]',
  );

const menuDropdownBaseClass =
  'absolute z-[1000] min-w-[200px] rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--bg-surface)] py-1 shadow-[var(--shadow-lg)] animate-[fadeIn_100ms_ease]';

const menuDropdownClass = cx(menuDropdownBaseClass, 'left-0 top-full');
const submenuDropdownClass = cx(menuDropdownBaseClass, 'left-full top-0');

const menuItemClass = (disabled?: boolean) =>
  cx(
    'flex w-full cursor-pointer items-center justify-between border-0 bg-transparent px-3.5 py-[5px] text-left font-sans text-xs text-[var(--text-primary)] transition-[background] duration-[var(--transition-fast)] hover:bg-[var(--bg-hover)]',
    disabled && 'cursor-default opacity-35 hover:bg-transparent',
  );

const submenuItemClass = cx(menuItemClass(false), 'relative gap-1');
const menuItemLabelClass = 'flex-1';
const menuShortcutClass = 'ml-6 font-mono text-[10px] text-[var(--text-muted)]';
const menuDividerClass = 'mx-2 my-1 h-px bg-[var(--border)]';

// ─── MenuBar Component ─────────────────────────────────────────────────────

interface MenuBarProps {
  menus: MenuDef[];
  onCloseProject: () => void;
}

export function MenuBar({ menus, onCloseProject }: MenuBarProps) {
  const { openMenu, setOpenMenu, ref } = useDropdown();

  return (
    <div className={menuBarClass} ref={ref}>
      {menus.map((menu) => (
        <div key={menu.label} className={menuClass}>
          <button
            className={menuTriggerClass(openMenu === menu.label)}
            onClick={() => setOpenMenu(openMenu === menu.label ? null : menu.label)}
            onMouseEnter={() => openMenu && setOpenMenu(menu.label)}
          >
            {menu.label}
          </button>
          {openMenu === menu.label && (
            <div className={menuDropdownClass}>
              {menu.items.map((item, i) => {
                if (item.divider) {
                  return <div key={i} className={menuDividerClass} />;
                }
                if (item.submenu) {
                  return (
                    <SubmenuItem key={i} item={item} depth={0} onClose={() => setOpenMenu(null)} />
                  );
                }
                return (
                  <button
                    key={i}
                    className={menuItemClass(item.disabled)}
                    disabled={item.disabled}
                    onClick={() => {
                      item.action?.();
                      setOpenMenu(null);
                    }}
                  >
                    <span className={menuItemLabelClass}>{item.label}</span>
                    {item.shortcut && <span className={menuShortcutClass}>{item.shortcut}</span>}
                  </button>
                );
              })}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}

// ─── Submenu item with nested dropdown ─────────────────────────────────────

function SubmenuItem({ item, depth, onClose }: { item: MenuItem; depth: number; onClose: () => void }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    window.addEventListener('mousedown', handler);
    return () => window.removeEventListener('mousedown', handler);
  }, [open]);

  return (
    <div
      ref={ref}
      className={submenuItemClass}
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(false)}
      onClick={() => setOpen(!open)}
    >
      <span className={menuItemLabelClass}>{item.label}</span>
      <IconChevronRight size={12} />
      {open && item.submenu && (
        <div className={submenuDropdownClass}>
          {item.submenu.map((sub, i) => (
            sub.divider ? (
              <div key={i} className={menuDividerClass} />
            ) : (
              <button
                key={i}
                className={menuItemClass(sub.disabled)}
                disabled={sub.disabled}
                onClick={() => {
                  sub.action?.();
                  onClose();
                }}
              >
                <span className={menuItemLabelClass}>{sub.label}</span>
                {sub.shortcut && <span className={menuShortcutClass}>{sub.shortcut}</span>}
              </button>
            )
          ))}
        </div>
      )}
    </div>
  );
}

// ─── Toolbar Extras ────────────────────────────────────────────────────────

export type TransformTool = 'view' | 'move' | 'rotate' | 'scale';

interface ToolbarExtrasProps {
  activeTool: TransformTool;
  onToolChange: (tool: TransformTool) => void;
  space: 'global' | 'local';
  onSpaceChange: (space: 'global' | 'local') => void;
  moveSnap: number;
  onMoveSnapChange: (snap: number) => void;
  angleSnap: number;
  onAngleSnapChange: (snap: number) => void;
}

export function ToolbarExtras({
  activeTool,
  onToolChange,
  space,
  onSpaceChange,
  moveSnap,
  onMoveSnapChange,
  angleSnap,
  onAngleSnapChange,
}: ToolbarExtrasProps) {
  const { t } = useTranslation();
  const [showSnap, setShowSnap] = useState(false);

  const tools: { key: TransformTool; icon: React.ReactNode; label: string; shortcut: string }[] = [
    { key: 'view',   icon: <IconView size={16} />,   label: t('tool_view'),   shortcut: 'Q' },
    { key: 'move',   icon: <IconMove size={16} />,   label: t('tool_move'),   shortcut: 'W' },
    { key: 'rotate', icon: <IconRotate size={16} />,  label: t('tool_rotate'), shortcut: 'E' },
    { key: 'scale',  icon: <IconScale size={16} />,  label: t('tool_scale'),  shortcut: 'R' },
  ];

  return (
    <div className={toolbarExtrasClass}>
      {/* Transform tools */}
      <div className={toolbarGroupClass}>
        {tools.map((tool) => (
          <button
            key={tool.key}
            className={toolButtonClass({ size: 'icon', active: activeTool === tool.key })}
            onClick={() => onToolChange(tool.key)}
            title={`${tool.label} (${tool.shortcut})`}
          >
            {tool.icon}
          </button>
        ))}
      </div>

      <div className={toolbarSeparatorClass} />

      {/* Transform space */}
      <div className={toolbarGroupClass}>
        <button
          className={toolButtonClass({ size: 'sm', active: space === 'global' })}
          onClick={() => onSpaceChange(space === 'global' ? 'local' : 'global')}
          title={t('tool_toggle_space')}
        >
          {space === 'global' ? t('tool_global') : t('tool_local')}
        </button>
      </div>

      <div className={toolbarSeparatorClass} />

      {/* Snap */}
      <div className={toolbarGroupRelativeClass}>
        <button
          className={toolButtonClass({ size: 'sm', active: showSnap })}
          onClick={() => setShowSnap(!showSnap)}
          title={t('tool_snap')}
        >
          <IconChevronDown size={10} /> {t('tool_snap')}
        </button>
        {showSnap && (
          <div className={`${contextMenuClass} absolute left-0 top-full z-[100] w-[180px]`}>
            <div className={`${contextMenuItemClass} gap-2 px-2 py-1`}>
              <span className="min-w-[60px] text-[11px]">{t('snap_move')}</span>
              <select value={moveSnap} onChange={e => onMoveSnapChange(Number(e.target.value))} className={toolbarSelectClass}>
                <option value={0.1}>0.1</option>
                <option value={0.25}>0.25</option>
                <option value={0.5}>0.5</option>
                <option value={1}>1</option>
              </select>
            </div>
            <div className={`${contextMenuItemClass} gap-2 px-2 py-1`}>
              <span className="min-w-[60px] text-[11px]">{t('snap_angle')}</span>
              <select value={angleSnap} onChange={e => onAngleSnapChange(Number(e.target.value))} className={toolbarSelectClass}>
                <option value={5}>5°</option>
                <option value={15}>15°</option>
                <option value={45}>45°</option>
              </select>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

// ─── Build menu structure from shell state ─────────────────────────────────

export interface EditorMenuActions {
  handleUndo: () => void;
  handleRedo: () => void;
  handleSaveScene: () => void;
  handleSaveSceneAs: () => void;
  handleOpenScene: () => void;
  handleCloseProject: () => void;
  handleCreateEmpty: () => void;
  handleCreateCamera: () => void;
  handleCreateLight: () => void;
  addComponent: (type: string) => void;
  handleImportAsset?: () => void;
  handleReimportAll?: () => void;
  handleProjectSettings?: () => void;
  handleToggleHierarchy?: () => void;
  handleToggleInspector?: () => void;
  handleToggleConsole?: () => void;
  handleToggleProject?: () => void;
  handleKeyboardShortcuts?: () => void;
  handleDocumentation?: () => void;
  handleReportIssue?: () => void;
  handleAbout?: () => void;
}

export interface EditorPanelStates {
  leftCollapsed?: boolean;
  rightCollapsed?: boolean;
  bottomCollapsed?: boolean;
}

export function buildEditorMenus(
  t: (key: string) => string,
  shellState: { can_undo: boolean; can_redo: boolean; scene_dirty: boolean; has_project: boolean } | null,
  actions: EditorMenuActions,
  panelStates?: EditorPanelStates,
): MenuDef[] {
  const hp = !!shellState?.has_project;
  const dirty = !!shellState?.scene_dirty;

  return [
    {
      label: t('menu_file'),
      items: [
        { label: t('menu_open_scene'),   shortcut: 'Ctrl+O',  disabled: !hp, action: actions.handleOpenScene },
        { label: t('menu_save'),          shortcut: 'Ctrl+S',  disabled: !dirty, action: actions.handleSaveScene },
        { label: t('menu_save_as'),       shortcut: 'Ctrl+Shift+S', disabled: !hp, action: actions.handleSaveSceneAs },
        { divider: true },
        { label: t('menu_close_project'), shortcut: '',        disabled: !hp, action: actions.handleCloseProject },
      ],
    },
    {
      label: t('menu_edit'),
      items: [
        { label: t('menu_undo'), shortcut: 'Ctrl+Z', disabled: !shellState?.can_undo, action: actions.handleUndo },
        { label: t('menu_redo'), shortcut: 'Ctrl+Y', disabled: !shellState?.can_redo, action: actions.handleRedo },
      ],
    },
    {
      label: t('menu_gameobject'),
      items: [
        { label: t('menu_create_empty'),  shortcut: 'Ctrl+Shift+N', disabled: !hp, action: actions.handleCreateEmpty },
        { divider: true },
        { label: t('menu_create_camera'), shortcut: '', disabled: !hp, action: actions.handleCreateCamera },
        { label: t('menu_create_light'),  shortcut: '', disabled: !hp, action: actions.handleCreateLight },
      ],
    },
    {
      label: t('menu_component'),
      items: [
        ...['Camera', 'MeshRenderer', 'Light', 'Rigidbody', 'Collider', 'AudioSource', 'Script'].map((comp) => ({
          label: comp,
          disabled: !hp,
          action: () => actions.addComponent(comp),
        })),
      ],
    },
    {
      label: t('menu_assets'),
      items: [
        { label: t('menu_import_asset'), shortcut: '', disabled: !hp, action: actions.handleImportAsset },
        { label: t('menu_reimport_all'), shortcut: '', disabled: !hp, action: actions.handleReimportAll },
        { divider: true },
        { label: t('menu_project_settings'), shortcut: '', disabled: !hp, action: actions.handleProjectSettings },
      ],
    },
    {
      label: t('menu_window'),
      items: [
        { label: t('menu_toggle_hierarchy'), shortcut: '', disabled: !hp, action: actions.handleToggleHierarchy },
        { label: t('menu_toggle_inspector'), shortcut: '', disabled: !hp, action: actions.handleToggleInspector },
        { divider: true },
        { label: t('menu_toggle_console'), shortcut: '', disabled: !hp, action: actions.handleToggleConsole },
        { label: t('menu_toggle_project'), shortcut: '', disabled: !hp, action: actions.handleToggleProject },
      ],
    },
    {
      label: t('menu_help'),
      items: [
        { label: t('menu_about'), shortcut: '', action: actions.handleAbout },
        { divider: true },
        { label: t('menu_keyboard_shortcuts'), shortcut: 'Ctrl+Shift+K', action: actions.handleKeyboardShortcuts },
        { divider: true },
        { label: t('menu_documentation'), shortcut: '', action: actions.handleDocumentation },
        { label: t('menu_report_issue'), shortcut: '', action: actions.handleReportIssue },
      ],
    },
  ];
}
