import React, { useCallback, useEffect, useState } from 'react';
import { rpc } from '../api';
import { useTranslation } from '../i18n';

// ─── Types ──────────────────────────────────────────────────────────────────

interface AssetEntry {
  guid: string;
  path: string;
  kind: string;
}

interface AssetMeta {
  guid: string;
  source_path: string;
  kind: string;
  importer: string;
}

interface ProjectAssets {
  entries: AssetEntry[];
  assets: AssetMeta[];
}

interface TreeNode {
  name: string;
  path: string;
  isDir: boolean;
  children: TreeNode[];
  meta?: AssetMeta;
}

// ─── Icons ──────────────────────────────────────────────────────────────────

const IconFolder = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="14" height="14">
    <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
  </svg>
);

const IconFile = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="14" height="14">
    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
    <polyline points="14 2 14 8 20 8" />
  </svg>
);

const IconImage = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="14" height="14">
    <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
    <circle cx="8.5" cy="8.5" r="1.5" />
    <polyline points="21 15 16 10 5 21" />
  </svg>
);

const IconCode = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="14" height="14">
    <polyline points="16 18 22 12 16 6" />
    <polyline points="8 6 2 12 8 18" />
  </svg>
);

const IconPlus = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="12" height="12">
    <line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" />
  </svg>
);

const IconRefresh = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="12" height="12">
    <polyline points="23 4 23 10 17 10" />
    <polyline points="1 20 1 14 7 14" />
    <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" />
  </svg>
);

// ─── Kind icon mapping ──────────────────────────────────────────────────────

function assetIcon(kind: string): React.ReactNode {
  const k = kind.toLowerCase();
  if (k.includes('texture') || k.includes('image') || k.includes('sprite')) return <IconImage />;
  if (k.includes('script') || k.includes('shader')) return <IconCode />;
  return <IconFile />;
}

// ─── Build tree from flat asset list ────────────────────────────────────────

function buildTree(assets: AssetMeta[]): { tree: TreeNode[]; folderCount: number } {
  const root: TreeNode[] = [];
  const map = new Map<string, TreeNode>();

  // Sort by path to ensure parent dirs come before children
  const sorted = [...assets].sort((a, b) => a.source_path.localeCompare(b.source_path));

  let folderCount = 0;

  for (const meta of sorted) {
    const parts = meta.source_path.split('/');
    let current = root;
    let currentPath = '';

    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      const isLast = i === parts.length - 1;
      const fullPath = currentPath ? `${currentPath}/${part}` : part;
      const key = isLast ? fullPath : `dir:${fullPath}`;

      let node = map.get(key);
      if (!node) {
        node = {
          name: part,
          path: fullPath,
          isDir: !isLast,
          children: [],
          meta: isLast ? meta : undefined,
        };
        map.set(key, node);
        current.push(node);
        if (!isLast) folderCount++;
      }
      current = node.children;
      currentPath = fullPath;
    }
  }

  return { tree: root, folderCount };
}

// ─── Tree Node Component ────────────────────────────────────────────────────

function TreeNodeItem({
  node,
  depth,
  search,
}: {
  node: TreeNode;
  depth: number;
  search: string;
}) {
  const [expanded, setExpanded] = useState(true);
  const hasChildren = node.children.length > 0;

  const matchesSearch = search === '' || node.name.toLowerCase().includes(search.toLowerCase());
  const childrenMatch = search !== '' && node.children.some(
    c => c.name.toLowerCase().includes(search.toLowerCase())
  );

  if (search && !matchesSearch && !childrenMatch && !node.isDir) return null;
  if (search && !matchesSearch && !childrenMatch && node.isDir && !hasChildren) return null;

  return (
    <>
      <div
        className={`project-tree-item ${node.isDir ? 'project-tree-dir' : 'project-tree-file'}`}
        style={{ paddingLeft: 8 + depth * 16 }}
        onClick={() => { if (hasChildren) setExpanded(!expanded); }}
        title={node.path}
      >
        {node.isDir ? (
          <span className="project-tree-caret">{expanded ? '▼' : '▶'}</span>
        ) : (
          <span className="project-tree-icon">
            {node.meta ? assetIcon(node.meta.kind) : <IconFile />}
          </span>
        )}
        <span className={`project-tree-name ${!matchesSearch && childrenMatch ? 'project-tree-dimmed' : ''}`}>
          {node.name}
        </span>
        {node.meta && (
          <span className="project-tree-kind">{node.meta.kind}</span>
        )}
      </div>
      {node.isDir && expanded && hasChildren && (
        <>
          {node.children.map((child, i) => (
            <TreeNodeItem key={child.path + i} node={child} depth={depth + 1} search={search} />
          ))}
          {search && node.children.length === 0 && (
            <div className="project-tree-empty" style={{ paddingLeft: 24 + depth * 16 }}>
              {node.name + '/'}
            </div>
          )}
        </>
      )}
    </>
  );
}

// ─── Project Panel ──────────────────────────────────────────────────────────

export default function ProjectPanel() {
  const { t } = useTranslation();
  const [assets, setAssets] = useState<AssetMeta[]>([]);
  const [tree, setTree] = useState<TreeNode[]>([]);
  const [search, setSearch] = useState('');
  const [loading, setLoading] = useState(true);
  const [createMenuOpen, setCreateMenuOpen] = useState(false);
  const [scriptName, setScriptName] = useState('');
  const [scriptBackend, setScriptBackend] = useState<'rhai' | 'python'>('rhai');

  const loadAssets = useCallback(async () => {
    setLoading(true);
    try {
      const data = await rpc<ProjectAssets>('project/list_assets');
      setAssets(data.assets);
      const { tree: t } = buildTree(data.assets);
      setTree(t);
    } catch {
      // no project open
    }
    setLoading(false);
  }, []);

  useEffect(() => { loadAssets(); }, [loadAssets]);

  const handleCreateScript = useCallback(async () => {
    if (!scriptName.trim()) return;
    try {
      await rpc('project/create_script', {
        name: scriptName.trim(),
        backend: scriptBackend,
      });
      setScriptName('');
      setCreateMenuOpen(false);
      await loadAssets();
    } catch (err) {
      console.error('Failed to create script:', err);
    }
  }, [scriptName, scriptBackend, loadAssets]);

  return (
    <div className="project-panel">
      {/* Toolbar */}
      <div className="project-toolbar">
        <div className="project-toolbar-left">
          <button
            className="project-btn-icon"
            onClick={loadAssets}
            title={t('project_refresh')}
          >
            <IconRefresh />
          </button>
          <div style={{ position: 'relative' }}>
            <button
              className="project-btn-icon"
              onClick={() => setCreateMenuOpen(!createMenuOpen)}
              title={t('project_create')}
            >
              <IconPlus />
            </button>
            {createMenuOpen && (
              <div className="context-menu" style={{ position: 'absolute', top: '100%', left: 0, zIndex: 100 }}>
                <div className="context-menu-item" style={{ display: 'flex', gap: 4, alignItems: 'center', padding: '4px 8px' }}>
                  <input
                    className="project-script-input"
                    type="text"
                    placeholder={t('project_script_name')}
                    value={scriptName}
                    onChange={(e) => setScriptName(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') handleCreateScript();
                      if (e.key === 'Escape') setCreateMenuOpen(false);
                      e.stopPropagation();
                    }}
                    autoFocus
                    onClick={(e) => e.stopPropagation()}
                  />
                  <select
                    className="project-script-select"
                    value={scriptBackend}
                    onChange={(e) => setScriptBackend(e.target.value as 'rhai' | 'python')}
                    onClick={(e) => e.stopPropagation()}
                  >
                    <option value="rhai">.rhai</option>
                    <option value="python">.py</option>
                  </select>
                </div>
              </div>
            )}
          </div>
        </div>
        {assets.length > 0 && (
          <span className="project-count">{assets.length}</span>
        )}
      </div>

      {/* Search */}
      <div className="project-search-row">
        <input
          className="project-search"
          type="text"
          placeholder={t('project_search')}
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      </div>

      {/* Tree */}
      <div className="project-scroll">
        {loading ? (
          <p className="panel-empty">{t('loading')}</p>
        ) : tree.length === 0 ? (
          <p className="panel-empty">{t('project_empty')}</p>
        ) : (
          tree.map((node, i) => (
            <TreeNodeItem key={node.path + i} node={node} depth={0} search={search} />
          ))
        )}
      </div>
    </div>
  );
}
