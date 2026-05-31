import React, { useCallback, useEffect, useRef, useState } from 'react';
import {
  openGameView, openScene, rpc, saveSceneAs, viewportReadback,
} from '../api';
import { useTranslation } from '../i18n';
import CopilotPanel from './CopilotPanel';
import ProjectPanel from './ProjectPanel';

// ─── Types ──────────────────────────────────────────────────────────────────

interface ShellState {
  has_project: boolean;
  project_name?: string;
  scene_dirty: boolean;
  can_undo: boolean;
  can_redo: boolean;
  scene_version?: number;
  desktop_integration?: {
    desktop_environment: string;
    prefers_native_chrome: boolean;
    window_background: string;
    window_backend?: string;
  };
}

interface SceneObject {
  id: string;
  name: string;
  tag: string;
  position: [number, number, number];
  parent_id?: string | null;
}

interface ConsoleEntry {
  timestamp: string;
  level: string;
  subsystem: string;
  message: string;
  file?: string;
  line?: number;
}

interface EntityDetails {
  id: string;
  name: string;
  tag: string;
  transform: {
    position: [number, number, number];
    rotation: [number, number, number, number];
    scale: [number, number, number];
  };
  components: Array<{
    type: string;
    data?: Record<string, unknown>;
  }>;
}

interface Props {
  onCloseProject: () => void;
}

// ─── Drag Handle Hook ────────────────────────────────────────────────────────

function useDragHandle(
  axis: 'horizontal' | 'vertical',
  onDelta: (delta: number) => void,
) {
  const dragging = useRef(false);
  const startPos = useRef(0);

  const onMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      dragging.current = true;
      startPos.current = axis === 'horizontal' ? e.clientX : e.clientY;

      const onMouseMove = (ev: MouseEvent) => {
        if (!dragging.current) return;
        const current = axis === 'horizontal' ? ev.clientX : ev.clientY;
        onDelta(current - startPos.current);
        startPos.current = current;
      };

      const onMouseUp = () => {
        dragging.current = false;
        window.removeEventListener('mousemove', onMouseMove);
        window.removeEventListener('mouseup', onMouseUp);
      };

      window.addEventListener('mousemove', onMouseMove);
      window.addEventListener('mouseup', onMouseUp);
    },
    [axis, onDelta],
  );

  return onMouseDown;
}

// ─── Resizable Panel ─────────────────────────────────────────────────────────

function ResizablePanel({
  side,
  width,
  minWidth,
  onResize,
  collapsed,
  onToggle,
  header,
  children,
}: {
  side: 'left' | 'right';
  width: number;
  minWidth: number;
  onResize: (w: number) => void;
  collapsed: boolean;
  onToggle: () => void;
  header: React.ReactNode;
  children: React.ReactNode;
}) {
  const handleMouseDown = useDragHandle('horizontal', (delta) => {
    const newW = side === 'left' ? width + delta : width - delta;
    onResize(Math.max(minWidth, Math.min(newW, 600)));
  });

  return (
    <>
      <aside
        className={`panel panel-${side}`}
        style={{ width: collapsed ? 0 : width, overflow: collapsed ? 'hidden' : undefined }}
      >
        <div className="panel-header">
          <span>{header}</span>
          <button className="panel-toggle" onClick={onToggle} title={`Toggle ${side} panel`}>
            {side === 'left' ? '\u00AB' : '\u00BB'}
          </button>
        </div>
        <div
          className="panel-content"
          style={{ display: collapsed ? 'none' : undefined }}
        >
          {children}
        </div>
      </aside>
      <div
        className={`resize-handle resize-handle-${side}`}
        onMouseDown={handleMouseDown}
      />
    </>
  );
}

// ─── Viewport ────────────────────────────────────────────────────────────────

function ViewportCanvas({ sceneVersion = 0 }: { sceneVersion?: number }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const sizeRef = useRef({ width: 640, height: 480 });
  const isActiveRef = useRef(true);
  const versionRef = useRef(sceneVersion);
  const lastRenderedVersionRef = useRef<number | null>(null);
  const cameraRef = useRef({ yaw: -0.5, pitch: 0.3, distance: 6, targetX: 0, targetY: 1, targetZ: 0 });
  const dragging = useRef<'orbit' | 'pan' | null>(null);
  const dragStart = useRef({ x: 0, y: 0, yaw: 0, pitch: 0, targetX: 0, targetY: 0, targetZ: 0 });

  // Keep version ref in sync
  versionRef.current = sceneVersion;

  // Poll for frames via binary IPC with lazy rendering
  useEffect(() => {
    isActiveRef.current = true;

    const poll = async () => {
      if (!isActiveRef.current) return;
      const { width, height } = sizeRef.current;
      const cam = cameraRef.current;
      try {
        const buffer = await viewportReadback({
          width, height,
          lastVersion: lastRenderedVersionRef.current ?? undefined,
          yaw: cam.yaw,
          pitch: cam.pitch,
          distance: cam.distance,
          targetX: cam.targetX,
          targetY: cam.targetY,
          targetZ: cam.targetZ,
        });
        if (!isActiveRef.current || !canvasRef.current) return;

        const uint8 = new Uint8Array(buffer);
        const header = new Uint32Array(uint8.buffer, uint8.byteOffset, 2);
        const w = header[0];
        const h = header[1];

        if (w > 0 && h > 0) {
          lastRenderedVersionRef.current = versionRef.current;
          const ctx = canvasRef.current.getContext('2d');
          if (ctx) {
            const imageData = new ImageData(
              new Uint8ClampedArray(uint8.buffer, uint8.byteOffset + 8, w * h * 4),
              w, h,
            );
            ctx.putImageData(imageData, 0, 0);
          }
        }
      } catch {
        // viewport not ready yet
      }
      setTimeout(poll, 100);
    };

    poll();
    return () => { isActiveRef.current = false; };
  }, []);

  // Resize observer
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect;
        if (width > 0 && height > 0) {
          const w = Math.round(width);
          const h = Math.round(height);
          sizeRef.current = { width: w, height: h };
          lastRenderedVersionRef.current = null;
          const canvas = canvasRef.current;
          if (canvas) { canvas.width = w; canvas.height = h; }
        }
      }
    });
    observer.observe(container);
    return () => observer.disconnect();
  }, []);

  // ── Mouse handlers ──

  const onMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button === 2) {
      // Right-click → orbit
      dragging.current = 'orbit';
      dragStart.current = { x: e.clientX, y: e.clientY, ...cameraRef.current };
      e.preventDefault();
    } else if (e.button === 1) {
      // Middle-click → pan
      dragging.current = 'pan';
      dragStart.current = { x: e.clientX, y: e.clientY, ...cameraRef.current };
      e.preventDefault();
    }
  }, []);

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!dragging.current) return;
      const dx = e.clientX - dragStart.current.x;
      const dy = e.clientY - dragStart.current.y;

      if (dragging.current === 'orbit') {
        cameraRef.current.yaw = dragStart.current.yaw - dx * 0.005;
        cameraRef.current.pitch = Math.max(-1.5, Math.min(1.5, dragStart.current.pitch + dy * 0.005));
      } else if (dragging.current === 'pan') {
        const d = cameraRef.current.distance * 0.002;
        const yaw = cameraRef.current.yaw;
        const sinY = Math.sin(yaw);
        const cosY = Math.cos(yaw);
        cameraRef.current.targetX = dragStart.current.targetX + (-dx * cosY - dy * sinY * 0.5) * d;
        cameraRef.current.targetY = dragStart.current.targetY + dy * d * 0.5;
        cameraRef.current.targetZ = dragStart.current.targetZ + (dx * sinY - dy * cosY * 0.5) * d;
      }

      // Force re-render by resetting the last version
      lastRenderedVersionRef.current = null;
    };

    const handleMouseUp = () => {
      dragging.current = null;
    };

    const handleWheel = (e: WheelEvent) => {
      if (containerRef.current && containerRef.current.contains(e.target as Node)) {
        const cam = cameraRef.current;
        cam.distance = Math.max(0.5, Math.min(100, cam.distance + e.deltaY * 0.01));
        lastRenderedVersionRef.current = null;
        e.preventDefault();
      }
    };

    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);
    window.addEventListener('wheel', handleWheel, { passive: false });

    return () => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
      window.removeEventListener('wheel', handleWheel);
    };
  }, []);

  return (
    <div
      ref={containerRef}
      className="viewport-container"
      onMouseDown={onMouseDown}
      onContextMenu={(e) => e.preventDefault()}
    >
      <canvas ref={canvasRef} className="viewport-canvas" />
    </div>
  );
}

// ─── Hierarchy Panel ─────────────────────────────────────────────────────────

function HierarchyPanel({
  objects,
  selectedId,
  onSelect,
  onCreateObject,
  onDeleteObject,
  onDuplicateObject,
  onRename,
  onRefresh,
}: {
  objects: SceneObject[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onCreateObject: (parentId?: string) => Promise<void>;
  onDeleteObject: (id: string) => Promise<void>;
  onDuplicateObject: (id: string) => Promise<void>;
  onRename: (id: string, name: string) => Promise<void>;
  onRefresh: () => Promise<void>;
}) {
  const { t } = useTranslation();
  const [search, setSearch] = useState('');
  const [createMenuOpen, setCreateMenuOpen] = useState(false);
  const [contextMenu, setContextMenu] = useState<{
    id: string;
    x: number;
    y: number;
  } | null>(null);
  const [renameId, setRenameId] = useState<string | null>(null);
  const [renameText, setRenameText] = useState('');

  // Close context menu on outside click
  useEffect(() => {
    if (!contextMenu) return;
    const handler = () => setContextMenu(null);
    window.addEventListener('click', handler);
    return () => window.removeEventListener('click', handler);
  }, [contextMenu]);

  // Build tree: root objects first, then children grouped by parent_id
  const buildTree = useCallback(() => {
    const filtered = search
      ? objects.filter(obj => obj.name.toLowerCase().includes(search.toLowerCase()))
      : objects;

    const byParent = new Map<string | null, SceneObject[]>();
    for (const obj of filtered) {
      const parentKey = obj.parent_id ?? null;
      if (!byParent.has(parentKey)) byParent.set(parentKey, []);
      byParent.get(parentKey)!.push(obj);
    }

    // Recursively render
    const renderChildren = (parentKey: string | null, depth: number): React.ReactNode[] => {
      const children = byParent.get(parentKey) ?? [];
      return children.flatMap(obj => {
        const isRenaming = renameId === obj.id;
        const items: React.ReactNode[] = [
          <div
            key={obj.id}
            className={`hierarchy-item ${selectedId === obj.id ? 'selected' : ''}`}
            style={{ paddingLeft: 10 + depth * 16 }}
            onClick={() => onSelect(obj.id)}
            onDoubleClick={() => {
              setRenameId(obj.id);
              setRenameText(obj.name);
            }}
            onContextMenu={(e) => {
              e.preventDefault();
              setContextMenu({ id: obj.id, x: e.clientX, y: e.clientY });
            }}
            title={`Tag: ${obj.tag}\nPosition: ${obj.position.map((v) => v.toFixed(2)).join(', ')}`}
          >
            <span className={`entity-icon entity-icon-${obj.tag.toLowerCase()}`} />
            {isRenaming ? (
              <input
                className="hierarchy-rename-input"
                value={renameText}
                onChange={(e) => setRenameText(e.target.value)}
                onBlur={() => {
                  if (renameText.trim() && renameText !== obj.name) {
                    onRename(obj.id, renameText.trim());
                  }
                  setRenameId(null);
                }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') {
                    (e.target as HTMLInputElement).blur();
                  } else if (e.key === 'Escape') {
                    setRenameId(null);
                  }
                  e.stopPropagation();
                }}
                autoFocus
                onClick={(e) => e.stopPropagation()}
              />
            ) : (
              <>
                <span className="entity-name">{obj.name}</span>
                <span className="entity-tag">{obj.tag}</span>
              </>
            )}
          </div>,
          ...renderChildren(obj.id, depth + 1),
        ];
        return items;
      });
    };

    return renderChildren(null, 0);
  }, [objects, search, selectedId, renameId, renameText, onSelect, onRename]);

  return (
    <>
      {/* Search bar */}
      <div className="hierarchy-search-row">
        <input
          className="hierarchy-search"
          type="text"
          placeholder={t('hierarchy_search')}
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        {search && (
          <span className="hierarchy-search-count">
            {objects.filter(o => o.name.toLowerCase().includes(search.toLowerCase())).length}
          </span>
        )}
      </div>

      {/* Toolbar row */}
      <div className="hierarchy-toolbar">
        <span className="hierarchy-count">{objects.length}</span>
        <div style={{ position: 'relative' }}>
          <button
            className="hierarchy-add-btn"
            onClick={() => setCreateMenuOpen(!createMenuOpen)}
            title={t('hierarchy_create')}
          >+</button>
          {createMenuOpen && (
            <div className="context-menu" style={{ top: '100%', left: 0 }}>
              <button className="context-menu-item" onClick={async () => {
                setCreateMenuOpen(false);
                await onCreateObject(selectedId ?? undefined);
              }}>
                {t('hierarchy_create_empty')}
              </button>
              <button className="context-menu-item" onClick={async () => {
                setCreateMenuOpen(false);
                const { id } = await rpc<{ id: string }>('shell/create_object', {});
                await rpc('shell/add_component', { id, component_type: 'Camera' });
                await onRefresh();
              }}>
                {t('hierarchy_create_camera')}
              </button>
              <button className="context-menu-item" onClick={async () => {
                setCreateMenuOpen(false);
                const { id } = await rpc<{ id: string }>('shell/create_object', {});
                await rpc('shell/add_component', { id, component_type: 'Light' });
                await onRefresh();
              }}>
                {t('hierarchy_create_light')}
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Object tree */}
      <div className="hierarchy-scroll">
        {objects.length === 0 ? (
          <p className="panel-empty">{t('hierarchy_no_objects')}</p>
        ) : buildTree().length === 0 ? (
          <p className="panel-empty">{t('hierarchy_search_no_matches')}</p>
        ) : (
          buildTree()
        )}
      </div>

      {/* Context menu */}
      {contextMenu && (
        <div
          className="context-menu"
          style={{ position: 'fixed', left: contextMenu.x, top: contextMenu.y, zIndex: 1000 }}
          onClick={(e) => e.stopPropagation()}
        >
          <button
            className="context-menu-item"
            onClick={() => { onDuplicateObject(contextMenu.id); setContextMenu(null); }}
          >
            {t('hierarchy_duplicate')}
          </button>
          <div className="context-menu-sep" />
          <button
            className="context-menu-item danger"
            onClick={() => { onDeleteObject(contextMenu.id); setContextMenu(null); }}
          >
            {t('hierarchy_delete')}
          </button>
        </div>
      )}
    </>
  );
}

// ─── Component Field Editor ─────────────────────────────────────────────────

function ComponentFieldEditor({
  componentType,
  data,
  entityId,
  onRefresh,
}: {
  componentType: string;
  data: Record<string, unknown> | null;
  entityId: string;
  onRefresh: () => void;
}) {
  const { t } = useTranslation();
  const [collapsed, setCollapsed] = useState(true);

  // Debounced field update
  const updateField = useCallback((field: string, value: unknown) => {
    rpc('shell/update_component', {
      id: entityId,
      component_type: componentType,
      data: { [field]: value },
    }).catch(console.error);
  }, [entityId, componentType]);

  // Schema definition for known component types
  type FieldSchema = { key: string; label: string; type: 'f32' | 'bool' | 'string' | 'enum'; options?: string[] };

  const fieldSchemas: Record<string, FieldSchema[]> = {
    Camera: [
      { key: 'vertical_fov_degrees', label: 'FOV', type: 'f32' },
      { key: 'near', label: 'Near', type: 'f32' },
      { key: 'far', label: 'Far', type: 'f32' },
    ],
    Light: [
      { key: 'intensity', label: 'Intensity', type: 'f32' },
      { key: 'range', label: 'Range', type: 'f32' },
      { key: 'spot_angle', label: 'Spot Angle', type: 'f32' },
    ],
    Rigidbody: [
      { key: 'mass', label: 'Mass', type: 'f32' },
      { key: 'linear_damping', label: 'Linear Damping', type: 'f32' },
      { key: 'angular_damping', label: 'Angular Damping', type: 'f32' },
    ],
    Collider: [
      { key: 'is_trigger', label: 'Is Trigger', type: 'bool' },
    ],
    AudioSource: [
      { key: 'volume', label: 'Volume', type: 'f32' },
    ],
    MeshRenderer: [],  // No editable scalar fields currently
  };

  const schema = fieldSchemas[componentType];
  const hasFields = schema && schema.length > 0;

  // Color vector editor (3-component)
  const renderColorEditor = (key: string, val: unknown) => {
    if (!Array.isArray(val) || val.length < 3) return null;
    const numVal = val as number[];
    return (
      <div className="inspector-color-row">
        <div
          className="inspector-color-swatch"
          style={{
            background: `rgb(${Math.round(numVal[0] * 255)}, ${Math.round(numVal[1] * 255)}, ${Math.round(numVal[2] * 255)})`,
          }}
        />
        {['R', 'G', 'B'].map((axis, ai) => (
          <span key={ai} className="inspector-vec3-input-wrap" style={{ width: 'auto', flex: 1 }}>
            <span className="inspector-vec3-label">{axis}</span>
            <input
              type="text"
              defaultValue={numVal[ai].toFixed(2)}
              onBlur={(e) => {
                const newVal = [...numVal];
                newVal[ai] = parseFloat(e.target.value) || 0;
                updateField(key, newVal);
                onRefresh();
              }}
              onKeyDown={(e) => {
                if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
              }}
              style={{ width: '100%' }}
            />
          </span>
        ))}
      </div>
    );
  };

  return (
    <div className="inspector-component">
      <button
        className="inspector-component-header"
        onClick={() => setCollapsed(!collapsed)}
      >
        <span className="inspector-component-caret">{collapsed ? '▶' : '▼'}</span>
        <span className="inspector-component-type">{componentType}</span>
        <button
          className="inspector-remove-btn"
          onClick={(e) => {
            e.stopPropagation();
            rpc('shell/remove_component', { id: entityId, component_type: componentType })
              .then(() => onRefresh())
              .catch(console.error);
          }}
          title={t('inspector_remove_component')}
        >
          ×
        </button>
      </button>
      {!collapsed && (
        <div className="inspector-component-fields">
          {data && 'color' in data && renderColorEditor('color', data.color)}
          {hasFields && data ? schema.map((field) => {
            const val = data[field.key];
            if (val === undefined) return null;

            switch (field.type) {
              case 'f32': {
                const numVal = typeof val === 'number' ? val : parseFloat(String(val)) || 0;
                return (
                  <div key={field.key} className="inspector-field">
                    <label>{field.label}</label>
                    <input
                      type="text"
                      className="inspector-field-input"
                      defaultValue={numVal.toFixed(2)}
                      onBlur={(e) => {
                        updateField(field.key, parseFloat(e.target.value) || 0);
                        onRefresh();
                      }}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
                      }}
                    />
                  </div>
                );
              }
              case 'bool': {
                const boolVal = typeof val === 'boolean' ? val : val === 'true';
                return (
                  <div key={field.key} className="inspector-field inspector-field-row">
                    <label>{field.label}</label>
                    <input
                      type="checkbox"
                      checked={boolVal}
                      onChange={(e) => {
                        updateField(field.key, e.target.checked);
                        onRefresh();
                      }}
                    />
                  </div>
                );
              }
              default:
                return null;
            }
          }) : !hasFields && (
            <p className="inspector-field-empty">{t('common_no_editable_fields')}</p>
          )}
        </div>
      )}
    </div>
  );
}

// ─── Inspector Panel ─────────────────────────────────────────────────────────

type TransformKey = 'position' | 'rotation' | 'scale';

function InspectorPanel({
  selectedId,
  onRefresh,
}: {
  selectedId: string | null;
  onRefresh: () => void;
}) {
  const { t } = useTranslation();
  const [details, setDetails] = useState<EntityDetails | null>(null);
  const [addMenuOpen, setAddMenuOpen] = useState(false);
  const [refreshVersion, setRefreshVersion] = useState(0);

  const loadDetails = useCallback(async () => {
    if (!selectedId) { setDetails(null); return; }
    try {
      const d = await rpc<EntityDetails>('shell/get_entity', { id: selectedId });
      setDetails(d);
      setRefreshVersion((v) => v + 1);
    } catch { setDetails(null); }
  }, [selectedId]);

  useEffect(() => { loadDetails(); }, [loadDetails]);

  // ── Transform update helper ──

  const updateTransform = useCallback(
    (key: TransformKey, axis: number, value: string) => {
      if (!selectedId || !details) return;
      const num = parseFloat(value);
      if (isNaN(num)) return;
      const arr = [...details.transform[key]] as number[];
      arr[axis] = num;
      const newTransform = { ...details.transform, [key]: arr };
      setDetails({ ...details, transform: newTransform });
      // Debounced save
      rpc('shell/update_transform', { id: selectedId, [key]: arr }).catch(console.error);
    },
    [selectedId, details],
  );

  // ── Component add / remove ──

  const addComponent = useCallback(
    async (compType: string) => {
      if (!selectedId) return;
      await rpc('shell/add_component', { id: selectedId, component_type: compType });
      setAddMenuOpen(false);
      await loadDetails();
      onRefresh();
    },
    [selectedId, loadDetails, onRefresh],
  );

  const removeComponent = useCallback(
    async (compType: string) => {
      if (!selectedId) return;
      await rpc('shell/remove_component', { id: selectedId, component_type: compType });
      await loadDetails();
      onRefresh();
    },
    [selectedId, loadDetails, onRefresh],
  );

  // ── Render ──

  if (!selectedId) {
    return <p className="panel-empty">{t('inspector_select_hint')}</p>;
  }

  if (!details) {
    return <p className="panel-empty">{t('loading')}</p>;
  }

  const labelAxis = (label: string) => ['X', 'Y', 'Z', 'W'].map((a) => `${label}${a}`);

  const vec3Inputs = (
    key: TransformKey,
    values: number[],
    decimals = 2,
  ) => (
    <div className="inspector-vec3">
      {values.map((v, i) => (
        <span key={i} className="inspector-vec3-input-wrap">
          <span className="inspector-vec3-label">{['X', 'Y', 'Z', 'W'][i]}</span>
          <input
            type="text"
            value={v.toFixed(decimals)}
            onChange={(e) => updateTransform(key, i, e.target.value)}
            onBlur={() => loadDetails()}
            title={labelAxis(key)[i] || String(i)}
          />
        </span>
      ))}
    </div>
  );

  // Available component types to add (excluding already-added)
  const allComponentTypes = [
    'Camera', 'Light', 'MeshRenderer', 'Rigidbody',
    'Collider', 'AudioSource', 'Script',
  ];
  const existingTypes = details.components.map((c) => c.type);
  const addableTypes = allComponentTypes.filter((t) => !existingTypes.includes(t));

  return (
    <div className="inspector">
      {/* Entity header */}
      <div className="inspector-header">
        <h3>{details.name}</h3>
        <span className="inspector-tag">{details.tag}</span>
      </div>

      {/* Transform */}
      <div className="inspector-section">
        <div className="inspector-section-title">{t('inspector_transform')}</div>
        <div className="inspector-field">
          <label>{t('inspector_position')}</label>
          {vec3Inputs('position', details.transform.position)}
        </div>
        <div className="inspector-field">
          <label>{t('inspector_rotation')}</label>
          {vec3Inputs('rotation', details.transform.rotation, 2)}
        </div>
        <div className="inspector-field">
          <label>{t('inspector_scale')}</label>
          {vec3Inputs('scale', details.transform.scale)}
        </div>
      </div>

      {/* Components */}
      <div className="inspector-section">
        <div className="inspector-section-title">{t('inspector_components')}</div>
        {details.components.map((c) => (
          <ComponentFieldEditor
            key={`${selectedId}-${c.type}-${refreshVersion}`}
            componentType={c.type}
            data={c.data ?? null}
            entityId={selectedId}
            onRefresh={loadDetails}
          />
        ))}
        {details.components.length === 0 && (
          <p className="panel-empty">{t('common_none')}</p>
        )}
      </div>

      {/* Add Component */}
      <div className="inspector-add-component">
        <button
          className="tool-btn"
          onClick={() => setAddMenuOpen(!addMenuOpen)}
          disabled={addableTypes.length === 0}
        >
          {t('inspector_add_component')}
        </button>
        {addMenuOpen && (
          <div className="inspector-add-menu">
            {addableTypes.map((t) => (
              <button
                key={t}
                className="inspector-add-option"
                onClick={() => addComponent(t)}
              >
                {t}
              </button>
            ))}
            {addableTypes.length === 0 && (
              <span className="panel-empty">{t('inspector_all_components_added')}</span>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

// ─── Console Panel ───────────────────────────────────────────────────────────

function ConsolePanel({ entries }: { entries: ConsoleEntry[] }) {
  const { t } = useTranslation();
  const scrollRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [entries]);

  return (
    <div ref={scrollRef} className="console">
      {entries.map((entry, i) => (
        <div key={i} className={`console-entry console-${entry.level}`}>
          <span className="console-level">{entry.level.toUpperCase()}</span>
          <span className="console-subsystem">[{entry.subsystem}]</span>
          <span className="console-message">{entry.message}</span>
        </div>
      ))}
      {entries.length === 0 && (
        <p className="panel-empty">{t('console_no_messages')}</p>
      )}
    </div>
  );
}

// ─── Editor Page ─────────────────────────────────────────────────────────────

export default function EditorPage({ onCloseProject }: Props) {
  const { t, t_fmt } = useTranslation();
  // Panel sizes
  const [leftWidth, setLeftWidth] = useState(220);
  const [rightWidth, setRightWidth] = useState(280);
  const [bottomHeight, setBottomHeight] = useState(140);
  const [leftCollapsed, setLeftCollapsed] = useState(false);
  const [rightCollapsed, setRightCollapsed] = useState(false);
  const [bottomCollapsed, setBottomCollapsed] = useState(false);

  // Data
  const [shellState, setShellState] = useState<ShellState | null>(null);
  const [sceneTree, setSceneTree] = useState<SceneObject[]>([]);
  const [consoleEntries, setConsoleEntries] = useState<ConsoleEntry[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [sceneVersion, setSceneVersion] = useState(0);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const prevSceneVersionRef = useRef(0);

  // Periodic lightweight state poll (every 2s)
  useEffect(() => {
    const poll = async () => {
      try {
        const state = await rpc<ShellState>('shell/get_state');
        setShellState(state);

        const newVer = state.scene_version ?? 0;
        if (newVer !== prevSceneVersionRef.current) {
          prevSceneVersionRef.current = newVer;
          setSceneVersion(newVer);

          // Scene changed — fetch updated scene tree
          const { objects } = await rpc<{ objects: SceneObject[] }>('shell/get_scene_tree');
          setSceneTree(objects);
        }
      } catch {
        // not ready
      }
    };

    poll();
    const interval = setInterval(poll, 2000);
    return () => clearInterval(interval);
  }, []);

  // Fetch console entries only when the panel is visible
  const consoleVisible = !bottomCollapsed;
  useEffect(() => {
    if (!consoleVisible) return;
    let active = true;
    const poll = async () => {
      if (!active) return;
      try {
        const { entries } = await rpc<{ entries: ConsoleEntry[] }>('console/get_entries');
        if (active) setConsoleEntries(entries);
      } catch { /* ignore */ }
      setTimeout(poll, 2000);
    };
    poll();
    return () => { active = false; };
  }, [consoleVisible]);

  // Immediate scene tree refresh (called after add/remove component)
  const refreshSceneTree = useCallback(async () => {
    try {
      // Fetch fresh state to get the new scene_version
      const state = await rpc<ShellState>('shell/get_state');
      setShellState(state);
      const newVer = state.scene_version ?? 0;
      prevSceneVersionRef.current = newVer;
      setSceneVersion(newVer);
      // Fetch the updated tree
      const { objects } = await rpc<{ objects: SceneObject[] }>('shell/get_scene_tree');
      setSceneTree(objects);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  }, []);

  const runEditorAction = useCallback(async (
    action: () => Promise<void>,
    success?: string,
  ) => {
    setErrorMessage(null);
    try {
      await action();
      await refreshSceneTree();
      if (success) setStatusMessage(success);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  }, [refreshSceneTree]);

  // ── Scene CRUD handlers ──

  const handleCreateObject = useCallback(async (parentId?: string) => {
    await runEditorAction(async () => {
      const params: Record<string, unknown> = {};
      if (parentId) params.parent_id = parentId;
      await rpc('shell/create_object', params);
    }, t('editor_status_object_created'));
  }, [runEditorAction]);

  const handleDeleteObject = useCallback(async (id: string) => {
    await runEditorAction(async () => {
      await rpc('shell/delete_object', { id });
      if (selectedId === id) {
        setSelectedId(null);
      }
    }, t('editor_status_object_deleted'));
  }, [runEditorAction, selectedId]);

  const handleDuplicateObject = useCallback(async (id: string) => {
    await runEditorAction(async () => {
      await rpc('shell/duplicate_object', { id });
    }, t('editor_status_object_duplicated'));
  }, [runEditorAction]);

  const handleSaveScene = useCallback(async () => {
    await runEditorAction(async () => {
      await rpc('shell/save_scene');
    }, t('editor_status_scene_saved'));
  }, [runEditorAction]);

  const handleUndo = useCallback(async () => {
    await runEditorAction(async () => {
      await rpc('shell/undo');
    });
  }, [runEditorAction]);

  const handleRedo = useCallback(async () => {
    await runEditorAction(async () => {
      await rpc('shell/redo');
    });
  }, [runEditorAction]);

  const handleClose = useCallback(() => {
    if (shellState?.scene_dirty && !window.confirm(t('dialog_close_unsaved'))) {
      return;
    }
    onCloseProject();
  }, [onCloseProject, shellState?.scene_dirty]);

  // ── File handlers ──

  const handleOpenScene = useCallback(async () => {
    await runEditorAction(async () => {
      const result = await openScene();
      if (result) setStatusMessage(t_fmt('editor_status_opened', { path: result }));
    });
  }, [runEditorAction]);

  const handleSaveSceneAs = useCallback(async () => {
    await runEditorAction(async () => {
      const result = await saveSceneAs();
      if (result) setStatusMessage(t_fmt('editor_status_saved', { path: result }));
    });
  }, [runEditorAction]);

  // Keyboard shortcuts
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // Ignore if typing in an input/textarea
      if (e.target instanceof HTMLElement && (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA')) {
        return;
      }

      // Ctrl+O → Open Scene
      if ((e.ctrlKey || e.metaKey) && e.key === 'o') {
        e.preventDefault();
        handleOpenScene();
        return;
      }

      // Ctrl+Shift+S → Save As
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 's') {
        e.preventDefault();
        handleSaveSceneAs();
        return;
      }

      // Delete/Backspace → remove selected object
      if ((e.key === 'Delete' || e.key === 'Backspace') && selectedId) {
        handleDeleteObject(selectedId);
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [selectedId, handleDeleteObject, handleOpenScene, handleSaveSceneAs]);

  const [rightTab, setRightTab] = useState<'inspector' | 'copilot'>('inspector');
  const [bottomTab, setBottomTab] = useState<'console' | 'project'>('console');

  // Bottom resize handle
  const bottomHandleDown = useDragHandle('vertical', (delta) => {
    setBottomHeight((h) => {
      const newH = h - delta; // dragging up = reduce
      return Math.max(40, Math.min(newH, 500));
    });
  });

  // ── Render ──

  if (!shellState) {
    return <div className="loading">{t('loading_editor')}</div>;
  }

  return (
    <div className="editor">
      <div className="editor-toolbar">
        <button
          className="tool-btn"
          onClick={handleUndo}
          disabled={!shellState.can_undo}
          title={t('command_undo')}
        >↩</button>
        <button
          className="tool-btn"
          onClick={handleRedo}
          disabled={!shellState.can_redo}
          title={t('command_redo')}
        >↪</button>
        <div className="toolbar-sep" />
        <button className="tool-btn" onClick={handleOpenScene} title={t('command_open_scene')}>
          {t('command_open_scene')}
        </button>
        <button className="tool-btn" onClick={handleSaveSceneAs} title={t('command_save_as')}>
          {t('command_save_as')}
        </button>
        <button className="tool-btn" onClick={handleSaveScene} disabled={!shellState.scene_dirty} title={t('command_save')}>
          {t('command_save')}
        </button>
        <div className="toolbar-sep" />
        <button className="tool-btn play-btn" onClick={openGameView} title={t('command_play')}>
          ▶ {t('command_play')}
        </button>
        <div className="toolbar-sep" />
        <button className="tool-btn" onClick={handleClose} title={t('command_close_project')}>
          {t('command_close_project')}
        </button>
      </div>

      {/* Main Editor Body (hierarchy | scene view | inspector) */}
      <div className="editor-body">
        <ResizablePanel
          side="left"
          width={leftWidth}
          minWidth={160}
          onResize={setLeftWidth}
          collapsed={leftCollapsed}
          onToggle={() => setLeftCollapsed(!leftCollapsed)}
          header={t('panel_hierarchy')}
        >
          <HierarchyPanel
            objects={sceneTree}
            selectedId={selectedId}
            onSelect={(id) => {
              setSelectedId(id);
              rpc('shell/select_entity', { id });
            }}
            onCreateObject={handleCreateObject}
            onDeleteObject={handleDeleteObject}
            onDuplicateObject={handleDuplicateObject}
            onRename={async (id, name) => {
              await rpc('shell/rename_object', { id, name });
              await refreshSceneTree();
            }}
            onRefresh={refreshSceneTree}
          />
        </ResizablePanel>

        <main className="panel panel-center">
          <div className="panel-header">
            <span>{t('panel_scene_view')}</span>
          </div>
          <ViewportCanvas sceneVersion={sceneVersion} />
        </main>

        <ResizablePanel
          side="right"
          width={rightWidth}
          minWidth={220}
          onResize={setRightWidth}
          collapsed={rightCollapsed}
          onToggle={() => setRightCollapsed(!rightCollapsed)}
          header={
            <div className="panel-tabs">
              <button
                className={`panel-tab ${rightTab === 'inspector' ? 'active' : ''}`}
                onClick={() => setRightTab('inspector')}
              >
                {t('panel_inspector')}
              </button>
              <button
                className={`panel-tab ${rightTab === 'copilot' ? 'active' : ''}`}
                onClick={() => setRightTab('copilot')}
              >
                {t('panel_copilot')}
              </button>
            </div>
          }
        >
          {rightTab === 'inspector' ? (
            <InspectorPanel selectedId={selectedId} onRefresh={refreshSceneTree} />
          ) : (
            <CopilotPanel />
          )}
        </ResizablePanel>
      </div>

      {/* Bottom Panel (Console / Project) */}
      <div
        className={`panel panel-bottom${bottomCollapsed ? ' collapsed' : ''}`}
        style={{ height: bottomCollapsed ? 28 : bottomHeight }}
      >
        <div className="panel-header">
          <div className="panel-tabs">
            <button
              className={`panel-tab ${bottomTab === 'console' ? 'active' : ''}`}
              onClick={() => setBottomTab('console')}
            >
              {t('panel_console')}
            </button>
            <button
              className={`panel-tab ${bottomTab === 'project' ? 'active' : ''}`}
              onClick={() => setBottomTab('project')}
            >
              {t('panel_project')}
            </button>
          </div>
          <button
            className="panel-toggle"
            onClick={() => setBottomCollapsed(!bottomCollapsed)}
            title={t('panel_console')}
          >
            {bottomCollapsed ? '\u25B2' : '\u25BC'}
          </button>
        </div>
        {!bottomCollapsed && (
          bottomTab === 'console' ? (
            <ConsolePanel entries={consoleEntries} />
          ) : (
            <ProjectPanel />
          )
        )}
      </div>

      {/* Resize handle for bottom panel */}
      <div className="resize-handle resize-handle-bottom" onMouseDown={bottomHandleDown} />

      {/* Status Bar */}
      <footer className="editor-statusbar">
        <span className="status-item">
          {shellState.project_name || t('status_no_project')}
        </span>
        {errorMessage && (
          <span className="status-item status-error">
            {errorMessage}
          </span>
        )}
        {!errorMessage && statusMessage && (
          <span className="status-item">
            {statusMessage}
          </span>
        )}
        <span className="status-item" style={{ color: 'var(--accent)' }}>
          v0.1.0
        </span>
      </footer>
    </div>
  );
}
