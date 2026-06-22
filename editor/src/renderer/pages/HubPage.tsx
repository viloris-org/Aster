import React, { useState, useCallback, useEffect, useMemo } from 'react';
import { rpc, selectProjectLocation } from '../api';
import { useTranslation } from '../i18n';
import {
  buttonClass,
  badgeClass,
  formErrorClass,
  formGroupClass,
  formInputClass,
  formLabelClass,
  formSelectClass,
  hubEmptyClass,
  hubEmptyIconClass,
  hubEmptyTextClass,
  hubEmptyTitleClass,
  installBadgesClass,
  installCardClass,
  installIconClass,
  installInfoClass,
  installPathClass,
  installVersionClass,
  locationInputRowClass,
  modalBodyClass,
  modalClass,
  modalCloseButtonClass,
  modalFooterClass,
  modalHeaderClass,
  modalOverlayClass,
  modalTitleClass,
  projectAvatarClass,
  projectCardClass,
  projectFolderButtonClass,
  projectInfoClass,
  projectMetaClass,
  projectMetaDotClass,
  projectNameClass,
  projectPathClass,
  settingsDescClass,
  settingsInputClass,
  settingsLabelClass,
  settingsSectionTitleClass,
  settingsSelectClass,
  settingsSelectOptionClass,
  templateCardClass,
  templateCardDescClass,
  templateCardIconClass,
  templateCardTitleClass,
  templateGridClass,
  warningPanelClass,
  warningPanelIconClass,
  warningPanelTextClass,
} from '../uiClasses';
import {
  IconProjects, IconInstalls, IconSettings, IconFolder, IconPlus, IconTrash, IconPlay,
  IconSun, IconMoon, IconMonitor, IconPackage, IconAlertTriangle, IconX, IconEmpty, IconSparkles,
  IconChevronLeft, IconCheck, IconInfo, IconLoader,
  AsterLogo,
} from '../icons';

// ─── Types ──────────────────────────────────────────────────────────────────

interface ProjectMeta {
  name: string;
  path: string;
  last_touched: string;
  toolchain_version: string;
}

interface InstallInfo {
  version: string;
  path: string;
  editor_available: boolean;
  runtime_available: boolean;
}

interface HubState {
  page: string;
  theme: string;
  locale: string;
  recent_projects: ProjectMeta[];
  installs: InstallInfo[];
  open_project: string | null;
}

interface Props {
  state: HubState;
  onOpenProject: (path: string) => Promise<void> | void;
  onNavigate: (page: string) => void;
  onSetTheme: (theme: string) => void;
  onSetLocale: (locale: string) => void;
  onRefresh: () => Promise<void>;
  onOpenQuests: () => void;
  onReturnToEditor: () => Promise<void> | void;
  returnProjectPath?: string | null;
}

// ─── Avatar helper ──────────────────────────────────────────────────────────

const AVATAR_COLORS = [
  'bg-linear-to-br from-[#3BC3DD] to-[#064E5A]',
  'bg-linear-to-br from-[#6AD7E5] to-[#087F8C]',
  'bg-linear-to-br from-[#78DFF0] to-[#075A64]',
  'bg-linear-to-br from-[#49D98B] to-[#0F766E]',
  'bg-linear-to-br from-[#168EA0] to-[#053F49]',
  'bg-linear-to-br from-[#64748B] to-[#1E293B]',
  'bg-linear-to-br from-[#38BDF8] to-[#1D4ED8]',
  'bg-linear-to-br from-[#F7B955] to-[#92400E]',
];

function getAvatarClass(name: string): string {
  const hash = name.split('').reduce((a, c) => a + c.charCodeAt(0), 0);
  return AVATAR_COLORS[hash % AVATAR_COLORS.length];
}

function getInitials(name: string): string {
  return name
    .split(/\s+/)
    .slice(0, 2)
    .map(w => w.charAt(0).toUpperCase())
    .join('')
    .slice(0, 2) || '?';
}

// last_touched may arrive as an epoch (seconds or millis) or an ISO string.
// Normalize to a readable local date; fall back to the raw value if unparseable.
function formatLastTouched(raw: string): string {
  if (!raw) return '';
  const numeric = Number(raw);
  let date: Date;
  if (Number.isFinite(numeric) && /^\d+$/.test(raw.trim())) {
    date = new Date(numeric < 1e12 ? numeric * 1000 : numeric);
  } else {
    date = new Date(raw);
  }
  if (Number.isNaN(date.getTime())) return raw.slice(0, 10);
  return date.toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' });
}

const hubClass = 'flex h-full min-h-0 w-full bg-[radial-gradient(circle_at_22%_-14%,rgba(106,215,229,0.13),transparent_32rem),radial-gradient(circle_at_88%_10%,rgba(var(--brand-rgb),0.18),transparent_34rem),var(--bg-base)]';
const hubMainClass = 'flex min-w-0 flex-1 flex-col overflow-hidden';
const hubPageHeaderClass = 'flex flex-shrink-0 items-center justify-between px-8 pt-7';
const hubPageHeaderTitleGroupClass = 'flex min-w-0 items-center gap-3';
const hubPageTitleClass = 'text-[24px] font-bold tracking-[-0.03em] text-[var(--text-primary)]';
const hubPageActionsClass = 'flex items-center gap-2';
const hubSearchBarClass = 'flex-shrink-0 px-8 pt-4 pb-3';
const hubSearchClass = 'w-full max-w-[520px] rounded-[14px] border border-white/[0.10] bg-[rgba(10,16,29,0.76)] bg-[url(data:image/svg+xml,%3Csvg%20xmlns=%27http://www.w3.org/2000/svg%27%20width=%2714%27%20height=%2714%27%20viewBox=%270%200%2024%2024%27%20fill=%27none%27%20stroke=%27%2375849D%27%20stroke-width=%272%27%20stroke-linecap=%27round%27%20stroke-linejoin=%27round%27%3E%3Ccircle%20cx=%2711%27%20cy=%2711%27%20r=%278%27/%3E%3Cline%20x1=%2721%27%20y1=%2721%27%20x2=%2716.65%27%20y2=%2716.65%27/%3E%3C/svg%3E)] bg-[position:14px_center] bg-no-repeat py-3 pr-4 pl-[40px] font-[var(--font-sans)] text-[14px] text-[var(--text-primary)] shadow-[var(--shadow-sm)] outline-none backdrop-blur-xl transition-[border-color,box-shadow] duration-[120ms] ease-in placeholder:text-[var(--text-muted)] focus:border-[var(--accent)] focus:shadow-[0_0_0_3px_var(--accent-dim)]';
const hubActionBarLabelClass = 'mr-1 text-[11px] text-[var(--text-muted)]';
const hubScrollClass = 'flex-1 overflow-y-auto px-8 pb-6 [scrollbar-color:var(--border)_transparent] [scrollbar-width:thin] [&::-webkit-scrollbar]:w-1.5 [&::-webkit-scrollbar-thumb]:rounded-[3px] [&::-webkit-scrollbar-thumb]:bg-[var(--border)] [&::-webkit-scrollbar-track]:bg-transparent';
const hubGridClass = 'grid grid-cols-[repeat(auto-fill,minmax(320px,1fr))] gap-3';
const installListClass = 'flex flex-col gap-2';
const hubEmptyActionsClass = 'mt-4 flex flex-wrap items-center justify-center gap-2';
const hubNoticeClass = 'mx-8 mb-3 rounded-[14px] border border-[rgba(var(--brand-rgb),0.22)] bg-[rgba(var(--brand-rgb),0.10)] px-4 py-3 text-[12px] leading-[1.55] text-[var(--text-secondary)] shadow-[var(--shadow-sm)]';

function hubActionBarClass(visible: boolean): string {
  return [
    'flex flex-shrink-0 items-center gap-1.5 overflow-hidden px-8 pb-2 transition-all duration-200 ease-in',
    visible ? 'min-h-8' : 'min-h-0',
  ].join(' ');
}

const sidebarClass = 'flex w-[240px] min-w-[240px] select-none flex-col border-r border-white/[0.08] bg-[linear-gradient(180deg,rgba(255,255,255,0.04),transparent_42%),rgba(7,12,22,0.90)] shadow-[inset_-1px_0_0_rgba(255,255,255,0.03)] backdrop-blur-2xl';
const logoClass = 'flex items-center gap-2.5 px-5 pt-7 pb-5 [&_svg]:shrink-0 [&_svg]:drop-shadow-[0_6px_14px_rgba(0,0,0,0.24)]';
const logoTitleClass = 'text-lg font-bold text-[var(--text-primary)]';
const logoTaglineClass = 'text-[11px] font-normal text-[var(--text-muted)]';
const navClass = 'flex flex-1 flex-col gap-0.5 px-2 py-2';
const navItemBaseClass = 'flex min-h-11 w-full cursor-pointer items-center gap-2.5 rounded-[12px] border border-transparent bg-transparent px-3 text-left font-[var(--font-sans)] text-[13px] font-semibold transition-[background,color,border-color] duration-[120ms] ease-in [&_svg]:h-[18px] [&_svg]:w-[18px] [&_svg]:shrink-0 [&_svg]:opacity-75 [&_svg]:transition-opacity [&_svg]:duration-[120ms] [&_svg]:ease-in hover:border-white/[0.10] hover:bg-white/[0.05] hover:text-[var(--text-primary)] hover:[&_svg]:opacity-100';
const sidebarFooterClass = 'flex items-center justify-between border-t border-white/[0.08] px-4 pt-3 pb-4';
const themeToggleLabelClass = 'text-xs text-[var(--text-secondary)]';
const themeToggleGroupClass = 'flex overflow-hidden rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--bg-base)] p-0.5';

function navItemClass(active = false): string {
  return [
    navItemBaseClass,
    active
      ? 'border-[rgba(var(--brand-rgb),0.34)] bg-[var(--brand-dim)] text-[var(--text-primary)] shadow-[inset_3px_0_0_var(--brand)] [&_svg]:text-[var(--accent-hover)] [&_svg]:opacity-100'
      : 'text-[var(--text-secondary)]',
  ].join(' ');
}

function themeToggleButtonClass(active: boolean): string {
  return [
    'cursor-pointer rounded-[4px] border-0 px-2 py-1 font-[var(--font-sans)] text-xs transition-all duration-[120ms] ease-in hover:text-[var(--text-primary)]',
    active ? 'bg-[var(--brand)] text-[var(--bg-base)] shadow-[var(--shadow-sm)] hover:bg-[var(--brand-hover)]' : 'bg-transparent text-[var(--text-muted)]',
  ].join(' ');
}

const settingsSectionClass = 'mb-7';
const settingsScrollClass = `${hubScrollClass} pt-4`;
const settingsContentClass = 'w-[min(780px,100%)]';
const settingsRowBaseClass = 'grid min-h-14 grid-cols-[minmax(180px,1fr)_minmax(240px,320px)] items-center gap-8 py-[11px] max-[780px]:grid-cols-1 max-[780px]:gap-2.5';
const settingsControlBaseClass = 'min-w-0 justify-self-end max-[780px]:w-full max-[780px]:justify-self-stretch';
const settingsControlClass = `${settingsControlBaseClass} w-full`;
const settingsControlCompactClass = `${settingsControlBaseClass} w-[200px]`;
const settingsActionsControlClass = `${settingsControlClass} flex justify-end`;
const settingsStickyFooterClass = 'sticky bottom-0 mt-8 flex items-center justify-between gap-3 border-t border-[var(--border)] bg-[linear-gradient(180deg,rgba(7,12,22,0),var(--bg-base)_24%,var(--bg-base))] pt-6 pb-2 max-[780px]:flex-col max-[780px]:items-stretch';
const settingsStickyHintClass = 'text-[12px] leading-[1.5] text-[var(--text-muted)]';
const settingsStickyActionsClass = 'flex flex-shrink-0 flex-wrap items-center justify-end gap-2 max-[780px]:justify-stretch max-[780px]:[&_button]:flex-1';
const themeSelectorClass = 'grid h-8 w-[200px] grid-cols-3 gap-0.5 overflow-hidden rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--bg-base)] p-0.5 max-[780px]:w-full';
const connectedTextClass = 'text-[var(--success)]';
const errorTextClass = 'text-[var(--error)]';
const endpointOptionalClass = 'opacity-60';
const settingsGatewayCardClass = 'mb-4 rounded-[18px] border border-[rgba(var(--brand-hover-rgb),0.24)] bg-[linear-gradient(135deg,rgba(var(--brand-hover-rgb),0.13),rgba(10,16,29,0.72)_46%,rgba(255,255,255,0.035))] p-4 shadow-[var(--shadow-sm)]';
const settingsGatewayHeaderClass = 'mb-2 flex items-center gap-2.5';
const settingsGatewayIconClass = 'flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-[10px] border border-[rgba(var(--brand-hover-rgb),0.25)] bg-[rgba(var(--brand-hover-rgb),0.12)] text-[var(--accent)]';
const settingsGatewayTitleClass = 'text-[14px] font-semibold tracking-[-0.01em] text-[var(--text-primary)]';
const settingsGatewayBadgeClass = 'rounded-full border border-[rgba(var(--brand-hover-rgb),0.24)] bg-[rgba(var(--brand-hover-rgb),0.10)] px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.08em] text-[var(--accent)]';
const settingsGatewayDescBlockClass = 'ml-[42px] text-[12px] leading-[1.55] text-[var(--text-secondary)] max-[780px]:ml-0';
const settingsGatewayChecklistClass = 'mt-3 grid grid-cols-3 gap-2 max-[780px]:grid-cols-1';
const settingsGatewayChecklistItemClass = 'rounded-[12px] border border-white/[0.08] bg-black/[0.13] px-3 py-2 text-[11px] leading-[1.45] text-[var(--text-muted)]';
const settingsFieldHintClass = 'mt-1.5 text-[11px] leading-[1.45] text-[var(--text-muted)]';
const settingsConnectionCardBaseClass = 'w-full rounded-[14px] border px-3 py-3 text-left shadow-[var(--shadow-sm)]';
const settingsConnectionHeaderClass = 'mb-1.5 flex items-center gap-2 text-[12px] font-semibold text-[var(--text-primary)]';
const settingsConnectionCodeClass = 'ml-auto rounded-full bg-black/[0.18] px-2 py-0.5 font-mono text-[10px] font-medium text-[var(--text-muted)]';
const settingsConnectionMessageClass = 'text-[12px] leading-[1.55] text-[var(--text-secondary)]';
const settingsConnectionMetaClass = 'mt-2 grid gap-1.5 text-[11px] text-[var(--text-muted)]';
const settingsConnectionModelsClass = 'mt-2 flex flex-wrap gap-1.5';
const settingsConnectionModelPillClass = 'max-w-full truncate rounded-full border border-white/[0.08] bg-black/[0.14] px-2 py-0.5 font-mono text-[10px] text-[var(--text-secondary)]';

function settingsConnectionCardClass(ok: boolean): string {
  return [
    settingsConnectionCardBaseClass,
    ok
      ? 'border-[rgba(73,217,139,0.28)] bg-[rgba(73,217,139,0.09)]'
      : 'border-[rgba(255,107,122,0.30)] bg-[rgba(255,107,122,0.08)]',
  ].join(' ');
}

function settingsRowClass(divided = false, extra = ''): string {
  return [
    settingsRowBaseClass,
    divided ? 'border-t border-[var(--border)]' : '',
    extra,
  ].filter(Boolean).join(' ');
}

function themeOptionButtonClass(active: boolean): string {
  return active
    ? 'flex h-[26px] min-w-0 cursor-pointer items-center justify-center whitespace-nowrap rounded-[4px] border-0 bg-[var(--brand)] px-2 font-[var(--font-sans)] text-xs leading-none text-[var(--bg-base)] shadow-[var(--shadow-sm)] transition-colors duration-[120ms] ease-in hover:bg-[var(--brand-hover)]'
    : 'flex h-[26px] min-w-0 cursor-pointer items-center justify-center whitespace-nowrap rounded-[4px] border-0 bg-transparent px-2 font-[var(--font-sans)] text-xs leading-none text-[var(--text-muted)] transition-colors duration-[120ms] ease-in hover:text-[var(--text-primary)]';
}

// ─── Sidebar ────────────────────────────────────────────────────────────────

function Sidebar({
  page,
  theme,
  hasOpenProject,
  onNavigate,
  onSetTheme,
  onOpenQuests,
  onReturnToEditor,
}: {
  page: string;
  theme: string;
  hasOpenProject: boolean;
  onNavigate: (p: string) => void;
  onSetTheme: (t: string) => void;
  onOpenQuests: () => void;
  onReturnToEditor: () => Promise<void> | void;
}) {
  const { t } = useTranslation();
  const navItems = [
    { id: 'projects', label: t('sidebar_projects'), icon: <IconProjects /> },
    { id: 'installs', label: t('sidebar_installs'), icon: <IconInstalls /> },
    { id: 'settings', label: t('sidebar_settings'), icon: <IconSettings /> },
  ];

  const themeOptions = [
    { id: 'dark', label: t('settings_theme_dark'), icon: <IconMoon /> },
    { id: 'light', label: t('settings_theme_light'), icon: <IconSun /> },
    { id: 'system', label: t('settings_theme_system'), icon: <IconMonitor /> },
  ];

  return (
    <aside className={sidebarClass}>
      {/* Logo */}
      <div className={logoClass}>
        <AsterLogo />
        <div>
          <h1 className={logoTitleClass}>Aster</h1>
          <span className={logoTaglineClass}>{t('app_tagline')}</span>
        </div>
      </div>

      {/* Navigation */}
      <nav className={navClass}>
        {page === 'settings' ? (
          <button
            className={navItemClass(false)}
            onClick={hasOpenProject ? onReturnToEditor : () => onNavigate('projects')}
            title={hasOpenProject ? t('settings_return_editor') : t('settings_back_projects')}
          >
            <IconChevronLeft />
            {hasOpenProject ? t('settings_return_editor') : t('settings_back_projects')}
          </button>
        ) : hasOpenProject && (
          <button
            className={navItemClass(false)}
            onClick={onReturnToEditor}
            title={t('settings_return_editor')}
          >
            <IconPlay />
            {t('settings_return_editor')}
          </button>
        )}
        {navItems.map(item => (
          <button
            key={item.id}
            className={navItemClass(page === item.id)}
            onClick={() => onNavigate(item.id)}
          >
            {item.icon}
            {item.label}
          </button>
        ))}
        <button className={navItemClass()} onClick={onOpenQuests} title={t('quest_intro_desc')}>
          <IconSparkles />
          {t('quest_title')}
        </button>
      </nav>

      {/* Theme Toggle */}
      <div className={sidebarFooterClass}>
        <span className={themeToggleLabelClass}>{t('sidebar_theme')}</span>
        <div className={themeToggleGroupClass}>
          {themeOptions.map(opt => (
            <button
              key={opt.id}
              className={themeToggleButtonClass(theme === opt.id)}
              onClick={() => onSetTheme(opt.id)}
              title={opt.label}
            >
              {opt.icon}
            </button>
          ))}
        </div>
      </div>
    </aside>
  );
}

// ─── New Project Dialog ─────────────────────────────────────────────────────

interface NewProjectDialogProps {
  installs: InstallInfo[];
  onClose: () => void;
  onCreate: (req: {
    name: string;
    location: string;
    template_id: string;
    toolchain_version: string;
  }) => Promise<void>;
}

const PROJECT_TEMPLATES = [
  { id: 'three_d', labelKey: 'template_3d', descKey: 'template_3d_desc' },
  { id: 'two_d', labelKey: 'template_2d', descKey: 'template_2d_desc' },
] as const;

function NewProjectDialog({ installs, onClose, onCreate }: NewProjectDialogProps) {
  const { t } = useTranslation();
  const [name, setName] = useState('');
  const [location, setLocation] = useState('');
  const [templateIdx, setTemplateIdx] = useState(0);
  const [versionIdx, setVersionIdx] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);

  const handleCreate = useCallback(async () => {
    if (!name.trim()) { setError(t('error_project_name_required')); return; }
    if (!location.trim()) { setError(t('error_project_location_required')); return; }
    setError(null);
    setCreating(true);
    try {
      await onCreate({
        name: name.trim(),
        location: location.trim(),
        template_id: PROJECT_TEMPLATES[templateIdx]?.id ?? 'three_d',
        toolchain_version: installs[versionIdx]?.version || '0.1.0',
      });
    } catch (e: unknown) {
      setError(typeof e === 'string' ? e : (e instanceof Error ? e.message : t('dialog_new_project')));
      setCreating(false);
    }
  }, [name, location, templateIdx, versionIdx, installs, onCreate, t]);

  const handleOverlayClick = useCallback((e: React.MouseEvent) => {
    if (e.target === e.currentTarget) onClose();
  }, [onClose]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Escape') onClose();
    if (e.key === 'Enter' && !creating) handleCreate();
  }, [onClose, handleCreate, creating]);

  return (
    <div className={modalOverlayClass} onClick={handleOverlayClick} onKeyDown={handleKeyDown}>
      <div className={modalClass()}>
        <div className={modalHeaderClass}>
          <h3 className={modalTitleClass}>{t('dialog_new_project')}</h3>
          <button className={modalCloseButtonClass} onClick={onClose}><IconX /></button>
        </div>
        <div className={modalBodyClass}>
          {/* Template */}
          <div className={formGroupClass}>
            <label className={formLabelClass}>{t('dialog_template')}</label>
            <div className={templateGridClass}>
              {PROJECT_TEMPLATES.map((tmpl, i) => (
                <div
                  key={tmpl.id}
                  className={templateCardClass(templateIdx === i)}
                  onClick={() => setTemplateIdx(i)}
                >
                  <span className={templateCardIconClass}>
                    {i === 0 ? (
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="24" height="24">
                        <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" />
                        <polyline points="3.27 6.96 12 12.01 20.73 6.96" />
                        <line x1="12" y1="22.08" x2="12" y2="12" />
                      </svg>
                    ) : (
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="24" height="24">
                        <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
                        <circle cx="8.5" cy="8.5" r="1.5" />
                        <polyline points="21 15 16 10 5 21" />
                      </svg>
                    )}
                  </span>
                  <div className={templateCardTitleClass}>{t(tmpl.labelKey)}</div>
                  <div className={templateCardDescClass}>{t(tmpl.descKey)}</div>
                </div>
              ))}
            </div>
          </div>

          {/* Project Name */}
          <div className={formGroupClass}>
            <label className={formLabelClass}>{t('dialog_project_name')}</label>
            <input
              className={formInputClass}
              type="text"
              placeholder={t('dialog_name_hint')}
              value={name}
              onChange={e => setName(e.target.value)}
              autoFocus
            />
          </div>

          {/* Location */}
          <div className={formGroupClass}>
            <label className={formLabelClass}>{t('dialog_location')}</label>
            <div className={locationInputRowClass}>
              <input
                className={`${formInputClass} flex-1`}
                type="text"
                placeholder={t('dialog_location_placeholder')}
                value={location}
                onChange={e => setLocation(e.target.value)}
              />
              <button
                className={buttonClass('secondary', 'sm', 'h-[34px] flex-shrink-0 whitespace-nowrap')}
                onClick={async () => {
                  setError(null);
                  try {
                    const selected = await selectProjectLocation();
                    if (selected) setLocation(selected);
                  } catch (err) {
                    setError(err instanceof Error ? err.message : String(err));
                  }
                }}
                type="button"
              >
                {t('dialog_browse')}
              </button>
            </div>
          </div>

          {/* Toolchain Version */}
          {installs.length > 0 && (
            <div className={formGroupClass}>
              <label className={formLabelClass}>{t('dialog_engine_version')}</label>
              <select
                className={formSelectClass}
                value={versionIdx}
                onChange={e => setVersionIdx(Number(e.target.value))}
              >
                {installs.map((inst, i) => (
                  <option key={i} value={i}>{inst.version}</option>
                ))}
              </select>
            </div>
          )}

          {/* Error */}
          {error && <p className={formErrorClass}>{error}</p>}
        </div>
        <div className={modalFooterClass}>
          <button className={buttonClass('secondary')} onClick={onClose}>{t('dialog_cancel')}</button>
          <button
            className={buttonClass('primary')}
            onClick={handleCreate}
            disabled={creating}
          >
            {creating ? t('dialog_creating') : t('dialog_create_project')}
          </button>
        </div>
      </div>
    </div>
  );
}

// ─── Confirm Delete Dialog ─────────────────────────────────────────────────

interface ConfirmDeleteProps {
  path: string;
  onClose: () => void;
  onRemoveRecent: () => void;
}

function ConfirmDeleteDialog({ path, onClose, onRemoveRecent }: ConfirmDeleteProps) {
  const { t, t_fmt } = useTranslation();
  const handleOverlayClick = useCallback((e: React.MouseEvent) => {
    if (e.target === e.currentTarget) onClose();
  }, [onClose]);

  return (
    <div className={modalOverlayClass} onClick={handleOverlayClick}>
      <div className={modalClass('w-[440px]')}>
        <div className={modalHeaderClass}>
          <h3 className={modalTitleClass}>{t('dialog_confirm_delete')}</h3>
          <button className={modalCloseButtonClass} onClick={onClose}><IconX /></button>
        </div>
        <div className={modalBodyClass}>
          <div className={warningPanelClass}>
            <IconAlertTriangle className={warningPanelIconClass} />
            <div className={warningPanelTextClass}>
              {t_fmt('dialog_confirm_message', { path })}
            </div>
          </div>
        </div>
        <div className={modalFooterClass}>
          <button className={buttonClass('secondary')} onClick={onClose}>{t('dialog_cancel')}</button>
          <button className={buttonClass('danger')} onClick={onRemoveRecent}>
            {t('dialog_remove_recents')}
          </button>
        </div>
      </div>
    </div>
  );
}

// ─── Projects Page ──────────────────────────────────────────────────────────

function ProjectsPage({
  projects,
  selectedPath,
  onSelect,
  onOpen,
  onDeleteRequest,
  onNewProject,
}: {
  projects: ProjectMeta[];
  selectedPath: string | null;
  onSelect: (path: string | null) => void;
  onOpen: (path: string) => Promise<void> | void;
  onDeleteRequest: (path: string) => void;
  onNewProject: () => void;
}) {
  const { t } = useTranslation();
  const [search, setSearch] = useState('');
  const [openError, setOpenError] = useState<string | null>(null);

  const searchText = search.trim().toLowerCase();
  const filtered = projects.filter(p => (
    p.name.toLowerCase().includes(searchText)
    || p.path.toLowerCase().includes(searchText)
  ));

  const handleCardClick = useCallback((path: string) => {
    if (selectedPath === path) {
      onSelect(null);
    } else {
      onSelect(path);
    }
  }, [selectedPath, onSelect]);

  const openProjectPath = useCallback(async (path: string) => {
    setOpenError(null);
    try {
      await Promise.resolve(onOpen(path));
    } catch (err) {
      setOpenError(err instanceof Error ? err.message : String(err));
    }
  }, [onOpen]);

  const handleCardDoubleClick = useCallback((path: string) => {
    openProjectPath(path);
  }, [openProjectPath]);

  const handleOpenExistingProject = useCallback(async () => {
    setOpenError(null);
    try {
      const selected = await selectProjectLocation();
      if (selected) await Promise.resolve(onOpen(selected));
    } catch (err) {
      setOpenError(err instanceof Error ? err.message : String(err));
    }
  }, [onOpen]);

  const handleOpenFolder = useCallback(async (e: React.MouseEvent, path: string) => {
    e.stopPropagation();
    try {
      await rpc('app/open_folder', { path });
    } catch {
      // folder open not supported on this platform
    }
  }, []);

  const selectedProject = projects.find(p => p.path === selectedPath);

  return (
    <>
      {/* Header */}
      <div className={hubPageHeaderClass}>
        <h2 className={hubPageTitleClass}>{t('hub_projects_title')}</h2>
        <div className={hubPageActionsClass}>
          <button className={buttonClass('secondary', 'sm')} onClick={handleOpenExistingProject}>
            <IconFolder /> {t('hub_open_project')}
          </button>
          <button className={buttonClass('primary', 'sm')} onClick={onNewProject}>
            <IconPlus /> {t('hub_new_project')}
          </button>
        </div>
      </div>

      {/* Search */}
      <div className={hubSearchBarClass}>
        <input
          className={hubSearchClass}
          type="text"
          placeholder={t('hub_search')}
          value={search}
          onChange={e => setSearch(e.target.value)}
        />
      </div>

      <div className={hubNoticeClass}>{t('hub_prototype_notice')}</div>
      {openError && <div className={`mx-8 mb-2 ${formErrorClass}`}>{openError}</div>}

      {/* Action bar (shown when a project is selected) */}
      <div className={hubActionBarClass(Boolean(selectedProject))}>
        {selectedProject && (
          <>
            <span className={hubActionBarLabelClass}>
              {selectedProject.name}
            </span>
            <button className={buttonClass('primary', 'sm')} onClick={() => openProjectPath(selectedProject.path)}>
              <IconPlay /> {t('hub_launch')}
            </button>
            <button className={buttonClass('danger', 'sm')} onClick={() => onDeleteRequest(selectedProject.path)}>
              <IconTrash /> {t('hub_delete')}
            </button>
          </>
        )}
      </div>

      {/* Project Cards */}
      <div className={hubScrollClass}>
        {filtered.length === 0 ? (
          <div className={hubEmptyClass}>
            <div className={hubEmptyIconClass}><IconEmpty /></div>
            {search ? (
              <>
                <h3 className={hubEmptyTitleClass}>{t('hub_search_no_matches')}</h3>
                <p className={hubEmptyTextClass}>{t('hub_search_no_matches_desc')}</p>
              </>
            ) : (
              <>
                <h3 className={hubEmptyTitleClass}>{t('hub_no_projects')}</h3>
                <p className={hubEmptyTextClass}>{t('hub_no_projects_desc')}</p>
                <div className={hubEmptyActionsClass}>
                  <button className={buttonClass('primary', 'sm')} onClick={onNewProject}>
                    <IconPlus /> {t('hub_new_project')}
                  </button>
                  <button className={buttonClass('secondary', 'sm')} onClick={handleOpenExistingProject}>
                    <IconFolder /> {t('hub_open_project')}
                  </button>
                </div>
              </>
            )}
          </div>
        ) : (
          <div className={hubGridClass}>
            {filtered.map(project => (
              <div
                key={project.path}
                className={projectCardClass(selectedPath === project.path)}
                onClick={() => handleCardClick(project.path)}
                onDoubleClick={() => handleCardDoubleClick(project.path)}
              >
                <div className={`${projectAvatarClass} ${getAvatarClass(project.name)}`}>
                  {getInitials(project.name)}
                </div>
                <div className={projectInfoClass}>
                  <div className={projectNameClass}>{project.name}</div>
                  <div className={projectPathClass}>{project.path}</div>
                  <div className={projectMetaClass}>
                    <span>{project.toolchain_version}</span>
                    <span className={projectMetaDotClass} />
                    <span>{formatLastTouched(project.last_touched)}</span>
                  </div>
                </div>
                <button
                  className={projectFolderButtonClass}
                  onClick={e => handleOpenFolder(e, project.path)}
                  title={t('hub_open_folder')}
                >
                  <IconFolder />
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
    </>
  );
}

// ─── Installs Page ──────────────────────────────────────────────────────────

function InstallsPage({ installs }: { installs: InstallInfo[] }) {
  const { t } = useTranslation();
  return (
    <>
      <div className={hubPageHeaderClass}>
        <h2 className={hubPageTitleClass}>{t('hub_installs_title')}</h2>
      </div>
      <div className={hubScrollClass}>
        {installs.length === 0 ? (
          <div className={hubEmptyClass}>
            <div className={hubEmptyIconClass}><IconPackage /></div>
            <h3 className={hubEmptyTitleClass}>{t('hub_installs_empty')}</h3>
            <p className={hubEmptyTextClass}>{t('hub_installs_empty_desc')}</p>
          </div>
        ) : (
          <div className={installListClass}>
            {installs.map((inst, i) => (
              <div key={i} className={installCardClass}>
                <div className={installIconClass}><IconPackage /></div>
                <div className={installInfoClass}>
                  <div className={installVersionClass}>{inst.version}</div>
                  <div className={installPathClass}>{inst.path}</div>
                </div>
                <div className={installBadgesClass}>
                  {inst.editor_available && <span className={badgeClass('green')}>{t('hub_installs_badge_editor')}</span>}
                  {!inst.editor_available && <span className={badgeClass('gray')}>{t('hub_installs_badge_no_editor')}</span>}
                  {inst.runtime_available && <span className={badgeClass('green')}>{t('hub_installs_badge_runtime')}</span>}
                  {!inst.runtime_available && <span className={badgeClass('gray')}>{t('hub_installs_badge_no_runtime')}</span>}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </>
  );
}

// ─── Copilot Settings ────────────────────────────────────────────────────────

interface ModelInfo {
  id: string;
  display_name: string;
  provider: string;
  context_window: number;
  default_max_tokens: number;
  capabilities: {
    can_reason: boolean;
    supports_vision: boolean;
    supports_tools: boolean;
  };
}

interface ProviderMeta {
  provider: string;
  display_name: string;
  requires_api_key: boolean;
  requires_endpoint: boolean;
  endpoint_configurable: boolean;
  default_endpoint: string | null;
  models: ModelInfo[];
}

interface MimoConfig {
  billing: 'subscription' | 'api';
  region: 'china' | 'singapore' | 'europe';
}

interface GlmConfig {
  billing: 'subscription' | 'api';
  region: 'bigmodel' | 'zai';
}

interface CopilotSettingsData {
  provider: 'stub' | 'anthropic' | 'openai' | 'codex_oauth' | 'gemini' | 'ollama' | 'custom' | 'mimo' | 'deepseek' | 'glm';
  model: string;
  api_endpoint: string | null;
  api_key: string | null;
  has_api_key?: boolean;
  max_tokens: number;
  mimo_config?: MimoConfig;
  glm_config?: GlmConfig;
}

interface ConnectionTestResult {
  ok: boolean;
  code: string;
  message: string;
  provider: string;
  provider_display: string;
  model: string;
  endpoint: string | null;
  has_api_key: boolean;
  model_count: number | null;
  model_found: boolean | null;
  models?: ModelInfo[];
}

function CopilotSettingsSection() {
  const { t } = useTranslation();
  const providerOptions: Array<{ value: CopilotSettingsData['provider']; label: string }> = [
    { value: 'anthropic', label: t('provider_anthropic') },
    { value: 'openai', label: t('provider_openai') },
    { value: 'codex_oauth', label: t('provider_codex_oauth') },
    { value: 'gemini', label: t('provider_gemini') },
    { value: 'deepseek', label: t('provider_deepseek') },
    { value: 'mimo', label: t('provider_mimo') },
    { value: 'glm', label: t('provider_glm') },
    { value: 'ollama', label: t('provider_ollama') },
    { value: 'custom', label: t('provider_custom') },
    { value: 'stub', label: t('provider_stub') },
  ];
  const [settings, setSettings] = useState<CopilotSettingsData>({
    provider: 'stub',
    model: '',
    api_endpoint: null,
    api_key: null,
    max_tokens: 4096,
  });
  const [saving, setSaving] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [providerMetas, setProviderMetas] = useState<ProviderMeta[]>([]);
  const [codexConnected, setCodexConnected] = useState(false);
  const [codexCode, setCodexCode] = useState<string | null>(null);
  const [codexAuthBusy, setCodexAuthBusy] = useState(false);
  const [codexAuthError, setCodexAuthError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [keyChanged, setKeyChanged] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [testBusy, setTestBusy] = useState(false);
  const [testResult, setTestResult] = useState<ConnectionTestResult | null>(null);

  useEffect(() => {
    Promise.all([
      rpc<CopilotSettingsData>('app/get_copilot_settings').catch(() => null),
      rpc<{ providers: ProviderMeta[] }>('app/get_model_registry').catch(() => ({ providers: [] })),
    ]).then(([s, reg]) => {
      if (s) {
        const providerMap: Record<string, CopilotSettingsData['provider']> = { open_a_i: 'openai' };
        const normalized = providerMap[s.provider] ?? s.provider;
        setSettings({ ...s, api_key: null, provider: normalized as CopilotSettingsData['provider'] });
      }
      setProviderMetas(reg.providers);
      setLoaded(true);
    });
    rpc<{ connected: boolean }>('app/codex_oauth_status')
      .then(status => setCodexConnected(status.connected))
      .catch(() => setCodexConnected(false));
  }, []);

  const currentMeta = useMemo(
    () => providerMetas.find(p => p.provider === settings.provider),
    [providerMetas, settings.provider]
  );

  const handleProviderChange = useCallback((provider: CopilotSettingsData['provider']) => {
    setSettings(s => ({ ...s, provider, api_endpoint: null }));
    setTestResult(null);
    setSaveError(null);
  }, []);

  const buildSettingsPayload = useCallback(() => {
    const payload = { ...settings };
    if (!keyChanged) delete (payload as any).api_key;
    return payload;
  }, [settings, keyChanged]);

  const handleSave = useCallback(async () => {
    setSaving(true);
    setSaved(false);
    setSaveError(null);
    try {
      await rpc('app/update_copilot_settings', buildSettingsPayload());
      setSaved(true);
      setKeyChanged(false);
      setTimeout(() => setSaved(false), 2000);
    } catch (err: any) {
      setSaveError(typeof err === 'string' ? err : err?.message ?? t('settings_save_failed'));
    } finally {
      setSaving(false);
    }
  }, [buildSettingsPayload, t]);

  const handleTestConnection = useCallback(async () => {
    setTestBusy(true);
    setTestResult(null);
    try {
      const result = await rpc<ConnectionTestResult>('app/test_copilot_connection', buildSettingsPayload());
      setTestResult(result);
    } catch (err: any) {
      setTestResult({
        ok: false,
        code: 'client_error',
        message: typeof err === 'string' ? err : err?.message ?? t('settings_connection_failed'),
        provider: settings.provider,
        provider_display: currentMeta?.display_name ?? settings.provider,
        model: settings.model,
        endpoint: settings.api_endpoint,
        has_api_key: Boolean(settings.api_key || settings.has_api_key),
        model_count: null,
        model_found: null,
        models: [],
      });
    } finally {
      setTestBusy(false);
    }
  }, [buildSettingsPayload, currentMeta, settings, t]);

  const handleCodexLogin = useCallback(async () => {
    setCodexAuthBusy(true);
    setCodexAuthError(null);
    try {
      const auth = await rpc<{ url: string; user_code: string; interval_seconds: number }>(
        'app/codex_oauth_start',
      );
      setCodexCode(auth.user_code);
      await rpc('app/open_folder', { path: auth.url });
      for (let attempt = 0; attempt < 100; attempt += 1) {
        await new Promise(resolve => setTimeout(resolve, (auth.interval_seconds + 3) * 1000));
        const result = await rpc<{ status: 'pending' | 'connected' }>('app/codex_oauth_poll');
        if (result.status === 'connected') {
          setCodexConnected(true);
          setCodexCode(null);
          setSettings(current => ({ ...current, provider: 'codex_oauth' }));
          return;
        }
      }
      throw new Error('Codex authorization timed out');
    } catch (err: any) {
      setCodexAuthError(typeof err === 'string' ? err : err.message || 'Authorization failed');
    } finally {
      setCodexAuthBusy(false);
    }
  }, []);

  const handleCodexLogout = useCallback(async () => {
    await rpc('app/codex_oauth_logout');
    setCodexConnected(false);
    setCodexCode(null);
  }, []);

  const showApiKey = currentMeta?.requires_api_key ?? (settings.provider !== 'ollama' && settings.provider !== 'stub');
  const showEndpoint = currentMeta?.endpoint_configurable
    ?? (settings.provider === 'ollama' || settings.provider === 'custom');
  const endpointRequired = settings.provider === 'custom';
  const modelOptions = currentMeta?.models ?? [];
  const testDisabled = testBusy
    || settings.provider === 'codex_oauth'
    || (settings.provider === 'custom' && !(settings.api_endpoint ?? '').trim());

  if (!loaded) return null;

  return (
    <div className={settingsSectionClass}>
      <div className={settingsSectionTitleClass}>{t('settings_ai_provider')}</div>

      <div className={settingsGatewayCardClass}>
        <div className={settingsGatewayHeaderClass}>
          <div className={settingsGatewayIconClass}><IconSparkles /></div>
          <div className="min-w-0 flex-1">
            <div className={settingsGatewayTitleClass}>{t('settings_openai_compatible_title')}</div>
          </div>
          <span className={settingsGatewayBadgeClass}>OpenAI</span>
        </div>
        <div className={settingsGatewayDescBlockClass}>
          {t('settings_openai_compatible_desc')}
          <div className={settingsGatewayChecklistClass}>
            <div className={settingsGatewayChecklistItemClass}>{t('settings_gateway_step_base_url')}</div>
            <div className={settingsGatewayChecklistItemClass}>{t('settings_gateway_step_key')}</div>
            <div className={settingsGatewayChecklistItemClass}>{t('settings_gateway_step_test')}</div>
          </div>
        </div>
      </div>

      {/* Provider */}
      <div className={settingsRowClass()}>
        <div>
          <div className={settingsLabelClass}>{t('settings_provider')}</div>
          <div className={settingsDescClass}>{t('settings_provider_desc')}</div>
        </div>
        <div className={settingsControlCompactClass}>
          <select
            className={settingsSelectClass}
            value={settings.provider}
            onChange={(e) => handleProviderChange(e.target.value as CopilotSettingsData['provider'])}
          >
            {providerOptions.map(opt => (
              <option key={opt.value} className={settingsSelectOptionClass} value={opt.value}>{opt.label}</option>
            ))}
          </select>
        </div>
      </div>

      {/* Endpoint */}
      {showEndpoint && (
        <div className={settingsRowClass(true)}>
          <div>
            <div className={settingsLabelClass}>
              {t(settings.provider === 'custom' ? 'settings_base_url' : 'settings_endpoint')} {endpointRequired ? '' : <small className={endpointOptionalClass}>{t('settings_endpoint_optional')}</small>}
            </div>
            <div className={settingsDescClass}>{t(settings.provider === 'custom' ? 'settings_base_url_desc' : 'settings_endpoint_desc')}</div>
          </div>
          <div className={settingsControlClass}>
            <input
              className={settingsInputClass}
              type="url"
              spellCheck={false}
              value={settings.api_endpoint ?? ''}
              placeholder={currentMeta?.default_endpoint ?? 'https://api.example.com/v1'}
              onChange={(e) => {
                setSettings(s => ({ ...s, api_endpoint: e.target.value || null }));
                setTestResult(null);
              }}
            />
            {settings.provider === 'custom' && (
              <div className={settingsFieldHintClass}>{t('settings_base_url_hint')}</div>
            )}
          </div>
        </div>
      )}

      {/* API Key */}
      {showApiKey && (
        <div className={settingsRowClass(true)}>
          <div>
            <div className={settingsLabelClass}>{t('settings_api_key')}</div>
            <div className={settingsDescClass}>{t('settings_api_key_desc')}</div>
          </div>
          <div className={settingsControlClass}>
            <input
              className={settingsInputClass}
              type="password"
              autoComplete="off"
              value={settings.api_key ?? ''}
              placeholder={settings.has_api_key ? t('settings_api_key_saved_placeholder') : 'sk-...'}
              onChange={(e) => {
                setSettings(s => ({ ...s, api_key: e.target.value || null }));
                setKeyChanged(true);
                setTestResult(null);
              }}
            />
            <div className={settingsFieldHintClass}>{t('settings_api_key_safe_hint')}</div>
          </div>
        </div>
      )}

      {/* Codex OAuth */}
      {settings.provider === 'codex_oauth' && (
        <div className={settingsRowClass(true)}>
          <div>
            <div className={settingsLabelClass}>{t('settings_chatgpt_account')}</div>
            <div className={settingsDescClass}>{t('settings_chatgpt_desc')}</div>
          </div>
          <div className={`${settingsControlClass} flex flex-col items-end gap-1`}>
            <button
              className={buttonClass('primary', 'sm')}
              onClick={codexConnected ? handleCodexLogout : handleCodexLogin}
              disabled={codexAuthBusy}
            >
              {codexAuthBusy ? t('settings_waiting_auth') : codexConnected ? t('settings_sign_out') : t('settings_sign_in_chatgpt')}
            </button>
            {codexConnected && <small className={connectedTextClass}>{t('settings_connected')}</small>}
            {codexCode && <small>{t('settings_enter_code').replace('{code}', codexCode ?? '')}</small>}
            {codexAuthError && <small className={errorTextClass}>{codexAuthError}</small>}
          </div>
        </div>
      )}

      {/* Model */}
      {settings.provider !== 'stub' && settings.provider !== 'codex_oauth' && (
        <div className={settingsRowClass(true)}>
          <div>
            <div className={settingsLabelClass}>{t('settings_model_id')}</div>
            <div className={settingsDescClass}>{t('settings_model_id_desc')}</div>
          </div>
          <div className={settingsControlClass}>
            <input
              className={settingsInputClass}
              type="text"
              list="copilot-model-options"
              spellCheck={false}
              value={settings.model ?? ''}
              placeholder={modelOptions[0]?.id ?? 'gpt-4.1-mini'}
              onChange={(e) => {
                setSettings(s => ({ ...s, model: e.target.value }));
                setTestResult(null);
              }}
            />
            <datalist id="copilot-model-options">
              {modelOptions.map(model => <option key={model.id} value={model.id}>{model.display_name}</option>)}
            </datalist>
            <div className={settingsFieldHintClass}>{t('settings_model_id_hint')}</div>
          </div>
        </div>
      )}

      {/* MiMo Configuration */}
      {settings.provider === 'mimo' && (
        <>
          <div className={settingsRowClass(true)}>
            <div>
              <div className={settingsLabelClass}>{t('settings_billing_mode')}</div>
              <div className={settingsDescClass}>{t('settings_mimo_billing_desc')}</div>
            </div>
            <div className={settingsControlCompactClass}>
              <select
                className={settingsSelectClass}
                value={settings.mimo_config?.billing ?? 'subscription'}
                onChange={(e) => setSettings(s => ({
                  ...s,
                  mimo_config: {
                    ...s.mimo_config,
                    billing: e.target.value as 'subscription' | 'api',
                    region: s.mimo_config?.region ?? 'china',
                  }
                }))}
              >
                <option className={settingsSelectOptionClass} value="subscription">{t('settings_token_plan')}</option>
                <option className={settingsSelectOptionClass} value="api">{t('settings_pay_as_you_go')}</option>
              </select>
            </div>
          </div>
          {(settings.mimo_config?.billing ?? 'subscription') === 'subscription' && (
            <div className={settingsRowClass(true)}>
              <div>
                <div className={settingsLabelClass}>{t('settings_region')}</div>
                <div className={settingsDescClass}>{t('settings_region_desc')}</div>
              </div>
              <div className={settingsControlCompactClass}>
                <select
                  className={settingsSelectClass}
                  value={settings.mimo_config?.region ?? 'china'}
                  onChange={(e) => setSettings(s => ({
                    ...s,
                    mimo_config: {
                      ...s.mimo_config,
                      billing: s.mimo_config?.billing ?? 'subscription',
                      region: e.target.value as 'china' | 'singapore' | 'europe',
                    }
                  }))}
                >
                  <option className={settingsSelectOptionClass} value="china">{t('settings_region_china')}</option>
                  <option className={settingsSelectOptionClass} value="singapore">{t('settings_region_singapore')}</option>
                  <option className={settingsSelectOptionClass} value="europe">{t('settings_region_europe')}</option>
                </select>
              </div>
            </div>
          )}
        </>
      )}

      {/* GLM Configuration */}
      {settings.provider === 'glm' && (
        <>
          <div className={settingsRowClass(true)}>
            <div>
              <div className={settingsLabelClass}>{t('settings_billing_mode')}</div>
              <div className={settingsDescClass}>{t('settings_glm_billing_desc')}</div>
            </div>
            <div className={settingsControlCompactClass}>
              <select
                className={settingsSelectClass}
                value={settings.glm_config?.billing ?? 'subscription'}
                onChange={(e) => setSettings(s => ({
                  ...s,
                  glm_config: {
                    ...s.glm_config,
                    billing: e.target.value as 'subscription' | 'api',
                    region: s.glm_config?.region ?? 'bigmodel',
                  }
                }))}
              >
                <option className={settingsSelectOptionClass} value="subscription">{t('settings_subscription')}</option>
                <option className={settingsSelectOptionClass} value="api">{t('settings_pay_as_you_go')}</option>
              </select>
            </div>
          </div>
          <div className={settingsRowClass(true)}>
            <div>
              <div className={settingsLabelClass}>{t('settings_region')}</div>
              <div className={settingsDescClass}>{t('settings_glm_region_desc')}</div>
            </div>
            <div className={settingsControlCompactClass}>
              <select
                className={settingsSelectClass}
                value={settings.glm_config?.region ?? 'bigmodel'}
                onChange={(e) => setSettings(s => ({
                  ...s,
                  glm_config: {
                    ...s.glm_config,
                    billing: s.glm_config?.billing ?? 'subscription',
                    region: e.target.value as 'bigmodel' | 'zai',
                  }
                }))}
              >
                <option className={settingsSelectOptionClass} value="bigmodel">{t('settings_bigmodel_china')}</option>
                <option className={settingsSelectOptionClass} value="zai">{t('settings_zai_international')}</option>
              </select>
            </div>
          </div>
        </>
      )}

      {/* Max Tokens */}
      {settings.provider !== 'stub' && (
        <div className={settingsRowClass(true)}>
          <div>
            <div className={settingsLabelClass}>{t('settings_max_tokens')}</div>
            <div className={settingsDescClass}>{t('settings_max_tokens_desc')}</div>
          </div>
          <div className={settingsControlCompactClass}>
            <input
              className={settingsInputClass}
              type="number"
              value={settings.max_tokens}
              min={256}
              max={128000}
              onChange={(e) => {
                setSettings(s => ({ ...s, max_tokens: parseInt(e.target.value) || 4096 }));
                setTestResult(null);
              }}
            />
          </div>
        </div>
      )}

      {/* Connection test */}
      {settings.provider !== 'codex_oauth' && (
        <div className={settingsRowClass(true, 'min-h-0 py-4')}>
          <div>
            <div className={settingsLabelClass}>{t('settings_connection_test_title')}</div>
            <div className={settingsDescClass}>{t('settings_connection_test_desc')}</div>
          </div>
          <div className={`${settingsControlClass} flex flex-col items-stretch gap-2`}>
            <button
              className={buttonClass('secondary', 'sm', 'justify-center')}
              onClick={handleTestConnection}
              disabled={testDisabled}
              title={settings.provider === 'custom' && !(settings.api_endpoint ?? '').trim() ? t('settings_connection_custom_requires_url') : undefined}
            >
              {testBusy ? <IconLoader className="animate-spin" /> : <IconInfo />}
              {testBusy ? t('settings_testing_connection') : t('settings_test_connection')}
            </button>
            {settings.provider === 'custom' && !(settings.api_endpoint ?? '').trim() && (
              <div className={settingsFieldHintClass}>{t('settings_connection_custom_requires_url')}</div>
            )}
            {testResult && (
              <div className={settingsConnectionCardClass(testResult.ok)} role="status" aria-live="polite">
                <div className={settingsConnectionHeaderClass}>
                  {testResult.ok ? <IconCheck /> : <IconAlertTriangle />}
                  <span>{testResult.ok ? t('settings_connection_success') : t('settings_connection_failed')}</span>
                  <span className={settingsConnectionCodeClass}>{testResult.code}</span>
                </div>
                <div className={settingsConnectionMessageClass}>{testResult.message}</div>
                <div className={settingsConnectionMetaClass}>
                  {testResult.endpoint && <div>{t('settings_connection_endpoint')}: {testResult.endpoint}</div>}
                  {typeof testResult.model_count === 'number' && <div>{t('settings_connection_model_count').replace('{count}', String(testResult.model_count))}</div>}
                  {testResult.model_found === true && <div>{t('settings_connection_model_found').replace('{model}', testResult.model || t('settings_model_empty'))}</div>}
                  {testResult.model_found === false && <div>{t('settings_connection_model_missing').replace('{model}', testResult.model || t('settings_model_empty'))}</div>}
                </div>
                {testResult.models && testResult.models.length > 0 && (
                  <div className={settingsConnectionModelsClass}>
                    {testResult.models.slice(0, 6).map(model => (
                      <span key={model.id} className={settingsConnectionModelPillClass}>{model.id}</span>
                    ))}
                  </div>
                )}
              </div>
            )}
          </div>
        </div>
      )}

      {/* Save button */}
      <div className={settingsRowClass(true, 'min-h-0 pt-3 max-[780px]:[&>*:first-child]:hidden')}>
        <div />
        <div className={`${settingsActionsControlClass} flex-col items-end gap-2`}>
          {saveError && <small className={errorTextClass}>{saveError}</small>}
          <button className={buttonClass('primary', 'sm')} onClick={handleSave} disabled={saving}>
            {saving ? t('settings_saving') : saved ? t('settings_saved') : t('settings_save_ai')}
          </button>
        </div>
      </div>
    </div>
  );
}

// ─── Settings Page ──────────────────────────────────────────────────────────

function SettingsPage({
  theme,
  locale,
  openProject,
  returnProjectPath,
  onSetTheme,
  onSetLocale,
  onReturnToEditor,
  onBackToProjects,
}: {
  theme: string;
  locale: string;
  openProject: string | null;
  returnProjectPath?: string | null;
  onSetTheme: (t: string) => void;
  onSetLocale: (l: string) => void;
  onReturnToEditor: () => Promise<void> | void;
  onBackToProjects: () => void;
}) {
  const { t, t_fmt } = useTranslation();
  const directReturnProject = openProject ?? returnProjectPath ?? null;
  const canReturnToEditor = Boolean(directReturnProject);
  const primaryBackLabel = canReturnToEditor ? t('settings_return_editor') : t('settings_back_projects');
  const handlePrimaryBack = () => {
    if (canReturnToEditor) {
      void onReturnToEditor();
      return;
    }
    onBackToProjects();
  };

  return (
    <>
      <div className={hubPageHeaderClass}>
        <div className={hubPageHeaderTitleGroupClass}>
          <button
            className={buttonClass('secondary', 'sm', 'min-h-9 px-3')}
            onClick={handlePrimaryBack}
            title={primaryBackLabel}
            aria-label={primaryBackLabel}
          >
            <IconChevronLeft /> {primaryBackLabel}
          </button>
          <h2 className={hubPageTitleClass}>{t('hub_settings_title')}</h2>
        </div>
        {canReturnToEditor && (
          <div className={hubPageActionsClass}>
            <button className={buttonClass('secondary', 'sm')} onClick={onBackToProjects}>
              <IconChevronLeft /> {t('settings_back_projects')}
            </button>
          </div>
        )}
      </div>
      <div className={settingsScrollClass}>
        <div className={settingsContentClass}>
          {/* Theme */}
          <div className={settingsSectionClass}>
            <div className={settingsSectionTitleClass}>{t('settings_appearance')}</div>
            <div className={settingsRowClass()}>
              <div>
                <div className={settingsLabelClass}>{t('settings_theme')}</div>
                <div className={settingsDescClass}>{t('settings_theme_desc')}</div>
              </div>
              <div className={settingsControlCompactClass}>
                <div className={themeSelectorClass}>
                  {[
                    { id: 'dark', label: t('settings_theme_dark') },
                    { id: 'light', label: t('settings_theme_light') },
                    { id: 'system', label: t('settings_theme_system') },
                  ].map(opt => (
                    <button
                      key={opt.id}
                      className={themeOptionButtonClass(theme === opt.id)}
                      onClick={() => onSetTheme(opt.id)}
                    >
                      {opt.label}
                    </button>
                  ))}
                </div>
              </div>
            </div>
          </div>

          {/* Language */}
          <div className={settingsSectionClass}>
            <div className={settingsSectionTitleClass}>{t('settings_language')}</div>
            <div className={settingsRowClass()}>
              <div>
                <div className={settingsLabelClass}>{t('settings_editor_language')}</div>
                <div className={settingsDescClass}>{t('settings_language_desc')}</div>
              </div>
              <div className={settingsControlCompactClass}>
                <select className={settingsSelectClass} value={locale} onChange={(e) => onSetLocale(e.target.value)}>
                  {[
                    { id: 'zh', label: t('settings_language_zh') },
                    { id: 'en', label: t('settings_language_en') },
                  ].map(opt => (
                    <option key={opt.id} className={settingsSelectOptionClass} value={opt.id}>{opt.label}</option>
                  ))}
                </select>
              </div>
            </div>
          </div>

          {/* AI Provider */}
          <CopilotSettingsSection />

          {/* About */}
          <div className={settingsSectionClass}>
            <div className={settingsSectionTitleClass}>{t('settings_about')}</div>
            <div className={settingsRowClass()}>
              <div>
                <div className={settingsLabelClass}>{t('settings_about_name')}</div>
                <div className={settingsDescClass}>{t_fmt('settings_about_version', { version: '0.1.0' })}</div>
              </div>
            </div>
          </div>

          <div className={settingsStickyFooterClass}>
            <div className={settingsStickyHintClass}>
              {canReturnToEditor ? t('settings_return_shortcut_hint') : t('settings_back_projects_hint')}
            </div>
            <div className={settingsStickyActionsClass}>
              {canReturnToEditor && (
                <button className={buttonClass('secondary', 'sm')} onClick={onBackToProjects}>
                  <IconProjects /> {t('settings_back_projects')}
                </button>
              )}
              <button className={buttonClass('primary', 'md', 'min-w-[150px] justify-center')} onClick={handlePrimaryBack}>
                <IconChevronLeft /> {primaryBackLabel}
              </button>
            </div>
          </div>
        </div>
      </div>
    </>
  );
}

// ─── HubPage (Root) ─────────────────────────────────────────────────────────

export default function HubPage({ state, onOpenProject, onNavigate, onSetTheme, onSetLocale, onRefresh, onOpenQuests, onReturnToEditor, returnProjectPath }: Props) {
  const [selectedProject, setSelectedProject] = useState<string | null>(null);
  const [showNewDialog, setShowNewDialog] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);
  const hasEditorReturnTarget = Boolean(state.open_project ?? returnProjectPath);

  // Reset selection when projects change
  useEffect(() => {
    setSelectedProject(prev => {
      if (!prev) return null;
      return state.recent_projects.some(p => p.path === prev) ? prev : null;
    });
  }, [state.recent_projects]);

  const handleNewProjectCreate = useCallback(async (req: {
    name: string;
    location: string;
    template_id: string;
    toolchain_version: string;
  }) => {
    await rpc('hub/create_project', {
      name: req.name,
      location: req.location,
      template_id: req.template_id,
      toolchain_version: req.toolchain_version,
    });
    setShowNewDialog(false);
    // Open the newly created project — use native path separator
    const sep = req.location.includes('\\') ? '\\' : '/';
    const createdPath = `${req.location}${sep}${req.name}`;
    await onOpenProject(createdPath);
  }, [onOpenProject]);

  const handleDeleteRecent = useCallback(async () => {
    if (!deleteTarget) return;
    try {
      await rpc('hub/delete_project', { path: deleteTarget, confirmed: true });
      await onRefresh();
    } catch {
      // Backend may refuse if project is open — silent
    }
    setDeleteTarget(null);
  }, [deleteTarget, onRefresh]);

  // Render the active page
  const renderPage = () => {
    switch (state.page) {
      case 'installs':
        return <InstallsPage installs={state.installs} />;
      case 'settings':
        return (
          <SettingsPage
            theme={state.theme}
            locale={state.locale}
            openProject={state.open_project}
            returnProjectPath={returnProjectPath}
            onSetTheme={onSetTheme}
            onSetLocale={onSetLocale}
            onReturnToEditor={onReturnToEditor}
            onBackToProjects={() => onNavigate('projects')}
          />
        );
      default:
        return (
          <ProjectsPage
            projects={state.recent_projects}
            selectedPath={selectedProject}
            onSelect={setSelectedProject}
            onOpen={onOpenProject}
            onDeleteRequest={setDeleteTarget}
            onNewProject={() => setShowNewDialog(true)}
          />
        );
    }
  };

  return (
    <div className={hubClass}>
      <Sidebar
        page={state.page}
        theme={state.theme}
        hasOpenProject={hasEditorReturnTarget}
        onNavigate={onNavigate}
        onSetTheme={onSetTheme}
        onOpenQuests={onOpenQuests}
        onReturnToEditor={onReturnToEditor}
      />

      <main className={hubMainClass}>
        {renderPage()}
      </main>

      {/* New Project Dialog */}
      {showNewDialog && (
        <NewProjectDialog
          installs={state.installs}
          onClose={() => setShowNewDialog(false)}
          onCreate={handleNewProjectCreate}
        />
      )}

      {/* Delete Confirmation */}
      {deleteTarget && (
        <ConfirmDeleteDialog
          path={deleteTarget}
          onClose={() => setDeleteTarget(null)}
          onRemoveRecent={handleDeleteRecent}
        />
      )}
    </div>
  );
}
