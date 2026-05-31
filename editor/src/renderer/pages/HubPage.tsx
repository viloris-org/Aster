import React, { useState, useCallback, useEffect, useRef } from 'react';
import { rpc, selectProjectLocation } from '../api';
import { useTranslation } from '../i18n';

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
  onOpenProject: (path: string) => void;
  onNavigate: (page: string) => void;
  onSetTheme: (theme: string) => void;
  onSetLocale: (locale: string) => void;
  onRefresh: () => Promise<void>;
}

// ─── Avatar helper ──────────────────────────────────────────────────────────

const AVATAR_COLORS = [
  'avatar-blue', 'avatar-green', 'avatar-purple', 'avatar-orange',
  'avatar-cyan', 'avatar-pink', 'avatar-red', 'avatar-teal',
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

// ─── SVG Icons ──────────────────────────────────────────────────────────────

const IconProjects = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <rect x="3" y="3" width="7" height="7" /><rect x="14" y="3" width="7" height="7" />
    <rect x="3" y="14" width="7" height="7" /><rect x="14" y="14" width="7" height="7" />
  </svg>
);

const IconInstalls = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" />
    <polyline points="3.27 6.96 12 12.01 20.73 6.96" /><line x1="12" y1="22.08" x2="12" y2="12" />
  </svg>
);

const IconSettings = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
  </svg>
);

const IconFolder = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="14" height="14">
    <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
  </svg>
);

const IconPlus = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="14" height="14">
    <line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" />
  </svg>
);

const IconTrash = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="14" height="14">
    <polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
  </svg>
);

const IconPlay = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="14" height="14">
    <polygon points="5 3 19 12 5 21 5 3" />
  </svg>
);

const IconSun = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="14" height="14">
    <circle cx="12" cy="12" r="5" /><line x1="12" y1="1" x2="12" y2="3" /><line x1="12" y1="21" x2="12" y2="23" />
    <line x1="4.22" y1="4.22" x2="5.64" y2="5.64" /><line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
    <line x1="1" y1="12" x2="3" y2="12" /><line x1="21" y1="12" x2="23" y2="12" />
    <line x1="4.22" y1="19.78" x2="5.64" y2="18.36" /><line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
  </svg>
);

const IconMoon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="14" height="14">
    <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
  </svg>
);

const IconMonitor = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="14" height="14">
    <rect x="2" y="3" width="20" height="14" rx="2" ry="2" /><line x1="8" y1="21" x2="16" y2="21" /><line x1="12" y1="17" x2="12" y2="21" />
  </svg>
);

const IconPackage = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="18" height="18">
    <line x1="16.5" y1="9.4" x2="7.5" y2="4.21" /><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" />
    <polyline points="3.27 6.96 12 12.01 20.73 6.96" /><line x1="12" y1="22.08" x2="12" y2="12" />
  </svg>
);

const IconAlertTriangle = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="18" height="18">
    <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
    <line x1="12" y1="9" x2="12" y2="13" /><line x1="12" y1="17" x2="12.01" y2="17" />
  </svg>
);

const IconX = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="16" height="16">
    <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
  </svg>
);

const IconEmpty = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" width="48" height="48">
    <rect x="2" y="3" width="20" height="14" rx="2" ry="2" /><line x1="8" y1="21" x2="16" y2="21" /><line x1="12" y1="17" x2="12" y2="21" />
  </svg>
);

// ─── Sidebar ────────────────────────────────────────────────────────────────

function Sidebar({
  page,
  theme,
  onNavigate,
  onSetTheme,
}: {
  page: string;
  theme: string;
  onNavigate: (p: string) => void;
  onSetTheme: (t: string) => void;
}) {
  const { t } = useTranslation();
  const navItems = [
    { id: 'projects', label: t('sidebar_projects'), icon: <IconProjects /> },
    { id: 'installs', label: t('sidebar_installs'), icon: <IconInstalls /> },
    { id: 'settings', label: t('sidebar_settings'), icon: <IconSettings /> },
  ];

  const themeOptions = [
    { id: 'dark', icon: <IconMoon /> },
    { id: 'light', icon: <IconSun /> },
    { id: 'system', icon: <IconMonitor /> },
  ];

  return (
    <aside className="hub-sidebar">
      {/* Logo */}
      <div className="hub-logo">
        <svg width="24" height="24" viewBox="0 0 16 16">
          <polygon points="8,1 15,5 15,11 8,15 1,11 1,5" fill="#22C55E" opacity="0.9" />
        </svg>
        <div>
          <h1>Aster</h1>
          <span>Game Engine</span>
        </div>
      </div>

      {/* Navigation */}
      <nav className="hub-nav">
        {navItems.map(item => (
          <button
            key={item.id}
            className={`hub-nav-item ${page === item.id ? 'active' : ''}`}
            onClick={() => onNavigate(item.id)}
          >
            {item.icon}
            {item.label}
          </button>
        ))}
      </nav>

      {/* Theme Toggle */}
      <div className="hub-sidebar-footer">
        <span className="theme-toggle-label">{t('sidebar_theme')}</span>
        <div className="theme-toggle-group">
          {themeOptions.map(opt => (
            <button
              key={opt.id}
              className={`theme-toggle-btn ${theme === opt.id ? 'active' : ''}`}
              onClick={() => onSetTheme(opt.id)}
              title={opt.id.charAt(0).toUpperCase() + opt.id.slice(1)}
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

function NewProjectDialog({ installs, onClose, onCreate }: NewProjectDialogProps) {
  const { t } = useTranslation();
  const [name, setName] = useState('');
  const [location, setLocation] = useState('');
  const [templateIdx, setTemplateIdx] = useState(0);
  const [versionIdx, setVersionIdx] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);

  const templates = [
    { id: 'three_d', title: '3D', desc: 'Full 3D scene with camera, light, and a default cube' },
    { id: 'two_d', title: '2D', desc: 'Orthographic 2D scene with sprite renderer set up' },
  ];

  const handleCreate = useCallback(async () => {
    if (!name.trim()) { setError(t('error_project_name_required')); return; }
    if (!location.trim()) { setError(t('error_project_location_required')); return; }
    setError(null);
    setCreating(true);
    try {
      await onCreate({
        name: name.trim(),
        location: location.trim(),
        template_id: templates[templateIdx].id,
        toolchain_version: installs[versionIdx]?.version || '0.1.0',
      });
    } catch (e: any) {
      setError(typeof e === 'string' ? e : e.message || t('dialog_new_project'));
      setCreating(false);
    }
  }, [name, location, templateIdx, versionIdx, installs, onCreate]);

  const handleOverlayClick = useCallback((e: React.MouseEvent) => {
    if (e.target === e.currentTarget) onClose();
  }, [onClose]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Escape') onClose();
    if (e.key === 'Enter' && !creating) handleCreate();
  }, [onClose, handleCreate, creating]);

  return (
    <div className="modal-overlay" onClick={handleOverlayClick} onKeyDown={handleKeyDown}>
      <div className="modal">
        <div className="modal-header">
          <h3>{t('dialog_new_project')}</h3>
          <button className="modal-close-btn" onClick={onClose}><IconX /></button>
        </div>
        <div className="modal-body">
          {/* Template */}
          <div className="form-group">
            <label className="form-label">{t('dialog_template')}</label>
            <div className="template-grid">
              {templates.map((tmpl, i) => (
                <div
                  key={tmpl.id}
                  className={`template-card ${templateIdx === i ? 'selected' : ''}`}
                  onClick={() => setTemplateIdx(i)}
                >
                  <span className="template-card-icon">
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
                  <div className="template-card-title">{t('template_' + tmpl.id)}</div>
                  <div className="template-card-desc">{t('template_' + tmpl.id + '_desc')}</div>
                </div>
              ))}
            </div>
          </div>

          {/* Project Name */}
          <div className="form-group">
            <label className="form-label">{t('dialog_project_name')}</label>
            <input
              className="form-input"
              type="text"
              placeholder={t('dialog_name_hint')}
              value={name}
              onChange={e => setName(e.target.value)}
              autoFocus
            />
          </div>

          {/* Location */}
          <div className="form-group">
            <label className="form-label">{t('dialog_location')}</label>
            <div className="location-input-row">
              <input
                className="form-input"
                type="text"
                placeholder={t('dialog_location_placeholder')}
                value={location}
                onChange={e => setLocation(e.target.value)}
              />
              <button
                className="btn btn-secondary btn-sm btn-browse"
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
            <div className="form-group">
              <label className="form-label">{t('dialog_engine_version')}</label>
              <select
                className="form-select"
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
          {error && <p className="form-error">{error}</p>}
        </div>
        <div className="modal-footer">
          <button className="btn btn-secondary" onClick={onClose}>{t('dialog_cancel')}</button>
          <button
            className="btn btn-primary"
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
    <div className="modal-overlay" onClick={handleOverlayClick}>
      <div className="modal" style={{ width: 440 }}>
        <div className="modal-header">
          <h3>{t('dialog_confirm_delete')}</h3>
          <button className="modal-close-btn" onClick={onClose}><IconX /></button>
        </div>
        <div className="modal-body">
          <div className="delete-warning">
            <IconAlertTriangle />
            <div className="delete-warning-text">
              {t_fmt('dialog_confirm_message', { path })}
            </div>
          </div>
        </div>
        <div className="modal-footer">
          <button className="btn btn-secondary" onClick={onClose}>{t('dialog_cancel')}</button>
          <button className="btn btn-danger" onClick={onRemoveRecent}>
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
  onOpen: (path: string) => void;
  onDeleteRequest: (path: string) => void;
  onNewProject: () => void;
}) {
  const { t } = useTranslation();
  const [search, setSearch] = useState('');

  const filtered = projects.filter(p =>
    p.name.toLowerCase().includes(search.toLowerCase())
  );

  const handleCardClick = useCallback((path: string) => {
    if (selectedPath === path) {
      onSelect(null);
    } else {
      onSelect(path);
    }
  }, [selectedPath, onSelect]);

  const handleCardDoubleClick = useCallback((path: string) => {
    onOpen(path);
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
      <div className="hub-page-header">
        <h2>{t('hub_projects_title')}</h2>
        <div className="hub-page-actions">
          <button className="btn btn-primary btn-sm" onClick={onNewProject}>
            <IconPlus /> {t('hub_new_project')}
          </button>
        </div>
      </div>

      {/* Search */}
      <div className="hub-search-bar">
        <input
          className="hub-search"
          type="text"
          placeholder={t('hub_search')}
          value={search}
          onChange={e => setSearch(e.target.value)}
        />
      </div>

      {/* Action bar (shown when a project is selected) */}
      <div className={`hub-action-bar ${selectedProject ? 'visible' : ''}`}>
        {selectedProject && (
          <>
            <span className="hub-action-bar-label">
              {selectedProject.name}
            </span>
            <button className="btn btn-sm btn-primary" onClick={() => onOpen(selectedProject.path)}>
              <IconPlay /> {t('hub_launch')}
            </button>
            <button className="btn btn-sm btn-danger" onClick={() => onDeleteRequest(selectedProject.path)}>
              <IconTrash /> {t('hub_delete')}
            </button>
          </>
        )}
      </div>

      {/* Project Cards */}
      <div className="hub-scroll">
        {filtered.length === 0 ? (
          <div className="hub-empty">
            <div className="hub-empty-icon"><IconEmpty /></div>
            {search ? (
              <>
                <h3>{t('hub_search_no_matches')}</h3>
                <p>{t('hub_search_no_matches_desc')}</p>
              </>
            ) : (
              <>
                <h3>{t('hub_no_projects')}</h3>
                <p>{t('hub_no_projects_desc')}</p>
              </>
            )}
          </div>
        ) : (
          <div className="hub-grid">
            {filtered.map(project => (
              <div
                key={project.path}
                className={`project-card ${selectedPath === project.path ? 'selected' : ''}`}
                onClick={() => handleCardClick(project.path)}
                onDoubleClick={() => handleCardDoubleClick(project.path)}
              >
                <div className={`project-avatar ${getAvatarClass(project.name)}`}>
                  {getInitials(project.name)}
                </div>
                <div className="project-info">
                  <div className="project-name">{project.name}</div>
                  <div className="project-path">{project.path}</div>
                  <div className="project-meta">
                    <span>{project.toolchain_version}</span>
                    <span className="project-meta-dot" />
                    <span>{project.last_touched.slice(0, 10)}</span>
                  </div>
                </div>
                <button
                  className="project-folder-btn"
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
      <div className="hub-page-header">
        <h2>{t('hub_installs_title')}</h2>
      </div>
      <div className="hub-scroll">
        {installs.length === 0 ? (
          <div className="hub-empty">
            <div className="hub-empty-icon"><IconPackage /></div>
            <h3>{t('hub_installs_empty')}</h3>
            <p>{t('hub_installs_empty_desc')}</p>
          </div>
        ) : (
          <div className="install-list">
            {installs.map((inst, i) => (
              <div key={i} className="install-card">
                <div className="install-icon"><IconPackage /></div>
                <div className="install-info">
                  <div className="install-version">{inst.version}</div>
                  <div className="install-path">{inst.path}</div>
                </div>
                <div className="install-badges">
                  {inst.editor_available && <span className="badge badge-green">{t('hub_installs_badge_editor')}</span>}
                  {!inst.editor_available && <span className="badge badge-gray">{t('hub_installs_badge_no_editor')}</span>}
                  {inst.runtime_available && <span className="badge badge-green">{t('hub_installs_badge_runtime')}</span>}
                  {!inst.runtime_available && <span className="badge badge-gray">{t('hub_installs_badge_no_runtime')}</span>}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </>
  );
}

// ─── Settings Page ──────────────────────────────────────────────────────────

function SettingsPage({
  theme,
  locale,
  onSetTheme,
  onSetLocale,
}: {
  theme: string;
  locale: string;
  onSetTheme: (t: string) => void;
  onSetLocale: (l: string) => void;
}) {
  const { t, t_fmt } = useTranslation();
  return (
    <>
      <div className="hub-page-header">
        <h2>{t('hub_settings_title')}</h2>
      </div>
      <div className="hub-scroll" style={{ maxWidth: 520 }}>
        {/* Theme */}
        <div className="settings-section">
          <div className="settings-section-title">{t('settings_appearance')}</div>
          <div className="settings-row">
            <div>
              <div className="settings-label">{t('settings_theme')}</div>
              <div className="settings-desc">{t('settings_theme_desc')}</div>
            </div>
            <div className="settings-control">
              <div className="theme-selector">
                {[
                  { id: 'dark', label: t('settings_theme_dark') },
                  { id: 'light', label: t('settings_theme_light') },
                  { id: 'system', label: t('settings_theme_system') },
                ].map(opt => (
                  <button
                    key={opt.id}
                    className={`theme-option ${theme === opt.id ? 'active' : ''}`}
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
        <div className="settings-section">
          <div className="settings-section-title">{t('settings_language')}</div>
          <div className="settings-row">
            <div>
              <div className="settings-label">{t('settings_editor_language')}</div>
              <div className="settings-desc">{t('settings_language_desc')}</div>
            </div>
            <div className="settings-control" style={{ display: 'flex', gap: 4 }}>
              {[
                { id: 'en', label: 'English' },
                { id: 'zh', label: '中文' },
              ].map(opt => (
                <button
                  key={opt.id}
                  className={`lang-btn ${locale === opt.id ? 'active' : ''}`}
                  onClick={() => onSetLocale(opt.id)}
                >
                  {opt.label}
                </button>
              ))}
            </div>
          </div>
        </div>

        {/* About */}
        <div className="settings-section">
          <div className="settings-section-title">{t('settings_about')}</div>
          <div className="settings-row">
            <div>
              <div className="settings-label">{t('settings_about_name')}</div>
              <div className="settings-desc">{t_fmt('settings_about_version', { version: '0.1.0' })}</div>
            </div>
          </div>
        </div>
      </div>
    </>
  );
}

// ─── HubPage (Root) ─────────────────────────────────────────────────────────

export default function HubPage({ state, onOpenProject, onNavigate, onSetTheme, onSetLocale, onRefresh }: Props) {
  const [selectedProject, setSelectedProject] = useState<string | null>(null);
  const [showNewDialog, setShowNewDialog] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);

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
    // Open the newly created project
    const createdPath = `${req.location}/${req.name}`;
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
            onSetTheme={onSetTheme}
            onSetLocale={onSetLocale}
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
    <div className="hub">
      <Sidebar
        page={state.page}
        theme={state.theme}
        onNavigate={onNavigate}
        onSetTheme={onSetTheme}
      />

      <main className="hub-main">
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
