import React, { useCallback, useEffect, useRef, useState } from 'react';
import {
  openGameView, openScene, rpc, saveSceneAs, viewportReadback,
} from '../api';
import { useTranslation } from '../i18n';

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
  components: Array<{ type: string }>;
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

  // Keep version ref in sync
  versionRef.current = sceneVersion;

  // Poll for frames via binary IPC with lazy rendering
  useEffect(() => {
    isActiveRef.current = true;

    const poll = async () => {
      if (!isActiveRef.current) return;
      const { width, height } = sizeRef.current;
      try {
        const buffer = await viewportReadback({
          width, height,
          lastVersion: lastRenderedVersionRef.current ?? undefined,
        });
        if (!isActiveRef.current || !canvasRef.current) return;

        // Parse header: [width: u32 LE][height: u32 LE][RGBA pixels...]
        const header = new Uint32Array(buffer, 0, 2);
        const w = header[0];
        const h = header[1];

        // w === 0 means "no change" (lazy rendering skip)
        if (w > 0 && h > 0) {
          lastRenderedVersionRef.current = versionRef.current;
          const ctx = canvasRef.current.getContext('2d');
          if (ctx) {
            const imageData = new ImageData(
              new Uint8ClampedArray(buffer, 8, w * h * 4),
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

  return (
    <div ref={containerRef} className="viewport-container">
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
}: {
  objects: SceneObject[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onCreateObject: (parentId?: string) => Promise<void>;
  onDeleteObject: (id: string) => Promise<void>;
  onDuplicateObject: (id: string) => Promise<void>;
}) {
  const [createMenuOpen, setCreateMenuOpen] = useState(false);
  const [contextMenu, setContextMenu] = useState<{
    id: string;
    x: number;
    y: number;
  } | null>(null);

  // Close context menu on outside click
  useEffect(() => {
    if (!contextMenu) return;
    const handler = () => setContextMenu(null);
    window.addEventListener('click', handler);
    return () => window.removeEventListener('click', handler);
  }, [contextMenu]);

  return (
    <>
      {/* Toolbar row */}
      <div className="hierarchy-toolbar">
        <span className="hierarchy-count">{objects.length}</span>
        <div style={{ position: 'relative' }}>
          <button
            className="hierarchy-add-btn"
            onClick={() => setCreateMenuOpen(!createMenuOpen)}
            title="Create GameObject"
          >+</button>
          {createMenuOpen && (
            <div className="context-menu" style={{ top: '100%', left: 0 }}>
              <button className="context-menu-item" onClick={async () => {
                setCreateMenuOpen(false);
                await onCreateObject(selectedId ?? undefined);
              }}>
                Empty GameObject
              </button>
              <button className="context-menu-item" onClick={async () => {
                setCreateMenuOpen(false);
                // Create with Camera component
                await rpc('shell/add_component', {
                  id: (await rpc<{ id: string }>('shell/create_object', {})).id,
                  component_type: 'Camera',
                });
                if (onCreateObject) onCreateObject();
              }}>
                Camera
              </button>
              <button className="context-menu-item" onClick={async () => {
                setCreateMenuOpen(false);
                // Create with Light component
                await rpc('shell/add_component', {
                  id: (await rpc<{ id: string }>('shell/create_object', {})).id,
                  component_type: 'Light',
                });
                if (onCreateObject) onCreateObject();
              }}>
                Light
              </button>
              <button className="context-menu-item" onClick={async () => {
                setCreateMenuOpen(false);
                await onCreateObject();
              }}>
                Create Empty
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Object list */}
      {objects.length === 0 && (
        <p className="panel-empty">No objects in scene</p>
      )}
      {objects.map((obj) => (
        <div
          key={obj.id}
          className={`hierarchy-item ${selectedId === obj.id ? 'selected' : ''}`}
          onClick={() => onSelect(obj.id)}
          onContextMenu={(e) => {
            e.preventDefault();
            setContextMenu({ id: obj.id, x: e.clientX, y: e.clientY });
          }}
          title={`Tag: ${obj.tag}\nPosition: ${obj.position.map((v) => v.toFixed(2)).join(', ')}`}
        >
          <span className={`entity-icon entity-icon-${obj.tag.toLowerCase()}`} />
          <span className="entity-name">{obj.name}</span>
          <span className="entity-tag">{obj.tag}</span>
        </div>
      ))}

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
            Duplicate
          </button>
          <div className="context-menu-sep" />
          <button
            className="context-menu-item danger"
            onClick={() => { onDeleteObject(contextMenu.id); setContextMenu(null); }}
          >
            Delete
          </button>
        </div>
      )}
    </>
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

  const loadDetails = useCallback(async () => {
    if (!selectedId) { setDetails(null); return; }
    try {
      const d = await rpc<EntityDetails>('shell/get_entity', { id: selectedId });
      setDetails(d);
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
        <div className="inspector-section-title">Transform</div>
        <div className="inspector-field">
          <label>Position</label>
          {vec3Inputs('position', details.transform.position)}
        </div>
        <div className="inspector-field">
          <label>Rotation</label>
          {vec3Inputs('rotation', details.transform.rotation, 2)}
        </div>
        <div className="inspector-field">
          <label>Scale</label>
          {vec3Inputs('scale', details.transform.scale)}
        </div>
      </div>

      {/* Components */}
      <div className="inspector-section">
        <div className="inspector-section-title">Components</div>
        {details.components.map((c, i) => (
          <div key={i} className="inspector-component">
            <span>{c.type}</span>
            <button
              className="inspector-remove-btn"
              onClick={() => removeComponent(c.type)}
              title={`Remove ${c.type}`}
            >
              ×
            </button>
          </div>
        ))}
        {details.components.length === 0 && (
          <p className="panel-empty">No components</p>
        )}
      </div>

      {/* Add Component */}
      <div className="inspector-add-component">
        <button
          className="tool-btn"
          onClick={() => setAddMenuOpen(!addMenuOpen)}
          disabled={addableTypes.length === 0}
        >
          + Add Component
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
              <span className="panel-empty">All components added</span>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

// ─── Console Panel ───────────────────────────────────────────────────────────

function ConsolePanel({ entries }: { entries: ConsoleEntry[] }) {
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
        <p className="panel-empty">No messages</p>
      )}
    </div>
  );
}

// ─── Editor Page ─────────────────────────────────────────────────────────────

export default function EditorPage({ onCloseProject }: Props) {
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
    }, 'Object created');
  }, [runEditorAction]);

  const handleDeleteObject = useCallback(async (id: string) => {
    await runEditorAction(async () => {
      await rpc('shell/delete_object', { id });
      if (selectedId === id) {
        setSelectedId(null);
      }
    }, 'Object deleted');
  }, [runEditorAction, selectedId]);

  const handleDuplicateObject = useCallback(async (id: string) => {
    await runEditorAction(async () => {
      await rpc('shell/duplicate_object', { id });
    }, 'Object duplicated');
  }, [runEditorAction]);

  const handleSaveScene = useCallback(async () => {
    await runEditorAction(async () => {
      await rpc('shell/save_scene');
    }, 'Scene saved');
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
    if (shellState?.scene_dirty && !window.confirm('Close without saving changes?')) {
      return;
    }
    onCloseProject();
  }, [onCloseProject, shellState?.scene_dirty]);

  // ── File handlers ──

  const handleOpenScene = useCallback(async () => {
    await runEditorAction(async () => {
      const result = await openScene();
      if (result) setStatusMessage(`Opened: ${result}`);
    });
  }, [runEditorAction]);

  const handleSaveSceneAs = useCallback(async () => {
    await runEditorAction(async () => {
      const result = await saveSceneAs();
      if (result) setStatusMessage(`Saved: ${result}`);
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

  // Bottom resize handle
  const bottomHandleDown = useDragHandle('vertical', (delta) => {
    setBottomHeight((h) => {
      const newH = h - delta; // dragging up = reduce
      return Math.max(40, Math.min(newH, 500));
    });
  });

  // ── Render ──

  if (!shellState) {
    return <div className="loading">Loading editor...</div>;
  }

  return (
    <div className="editor">
      <div className="editor-toolbar">
        <button
          className="tool-btn"
          onClick={handleUndo}
          disabled={!shellState.can_undo}
          title="Undo"
        >↩</button>
        <button
          className="tool-btn"
          onClick={handleRedo}
          disabled={!shellState.can_redo}
          title="Redo"
        >↪</button>
        <div className="toolbar-sep" />
        <button className="tool-btn" onClick={handleOpenScene} title="Open Scene (Ctrl+O)">
          Open
        </button>
        <button className="tool-btn" onClick={handleSaveSceneAs} title="Save Scene As (Ctrl+Shift+S)">
          Save As
        </button>
        <button className="tool-btn" onClick={handleSaveScene} disabled={!shellState.scene_dirty} title="Save Scene">
          Save
        </button>
        <div className="toolbar-sep" />
        <button className="tool-btn play-btn" onClick={openGameView} title="Play (opens Game View window)">
          ▶ Play
        </button>
        <div className="toolbar-sep" />
        <button className="tool-btn" onClick={handleClose} title="Close Project">
          Close
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
          header="Hierarchy"
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
          />
        </ResizablePanel>

        <main className="panel panel-center">
          <div className="panel-header">
            <span>Scene View</span>
          </div>
          <ViewportCanvas sceneVersion={sceneVersion} />
        </main>

        <ResizablePanel
          side="right"
          width={rightWidth}
          minWidth={180}
          onResize={setRightWidth}
          collapsed={rightCollapsed}
          onToggle={() => setRightCollapsed(!rightCollapsed)}
          header="Inspector"
        >
          <InspectorPanel selectedId={selectedId} onRefresh={refreshSceneTree} />
        </ResizablePanel>
      </div>

      {/* Bottom Console */}
      <div
        className={`panel panel-bottom${bottomCollapsed ? ' collapsed' : ''}`}
        style={{ height: bottomCollapsed ? 28 : bottomHeight }}
      >
        <div className="panel-header">
          <span>Console</span>
          <button
            className="panel-toggle"
            onClick={() => setBottomCollapsed(!bottomCollapsed)}
            title="Toggle console"
          >
            {bottomCollapsed ? '\u25B2' : '\u25BC'}
          </button>
        </div>
        {!bottomCollapsed && (
          <ConsolePanel entries={consoleEntries} />
        )}
      </div>

      {/* Resize handle for bottom panel */}
      <div className="resize-handle resize-handle-bottom" onMouseDown={bottomHandleDown} />

      {/* Status Bar */}
      <footer className="editor-statusbar">
        <span className="status-item">
          {shellState.project_name || 'No project'}
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
