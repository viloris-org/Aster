import React, { useCallback, useEffect, useRef, useState } from 'react';
import {
  rpc, openGameView,
} from '../api';

// ─── Types ──────────────────────────────────────────────────────────────────

interface ShellState {
  has_project: boolean;
  project_name?: string;
  scene_dirty: boolean;
  can_undo: boolean;
  can_redo: boolean;
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

interface ViewportFrame {
  width: number;
  height: number;
  png_base64: string;
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

function ViewportCanvas() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const sizeRef = useRef({ width: 640, height: 480 });

  // Poll for frames
  useEffect(() => {
    let active = true;
    let frameId = 0;

    const poll = async () => {
      if (!active) return;
      const { width, height } = sizeRef.current;
      try {
        const result = await rpc<ViewportFrame>('viewport/readback', {
          width, height,
          yaw: -0.5, pitch: 0.3, distance: 6.0,
        });

        if (!active || !canvasRef.current) return;

        const img = new Image();
        img.onload = () => {
          if (!active || !canvasRef.current) return;
          const ctx = canvasRef.current.getContext('2d');
          if (ctx) ctx.drawImage(img, 0, 0);
          URL.revokeObjectURL(img.src);
        };
        img.src = `data:image/png;base64,${result.png_base64}`;
      } catch {
        // viewport not ready
      }
      frameId = window.setTimeout(poll, 100);
    };

    poll();
    return () => { active = false; clearTimeout(frameId); };
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
}: {
  objects: SceneObject[];
  selectedId: string | null;
  onSelect: (id: string) => void;
}) {
  return (
    <>
      {objects.length === 0 && (
        <p className="panel-empty">No objects in scene</p>
      )}
      {objects.map((obj) => (
        <div
          key={obj.id}
          className={`hierarchy-item ${selectedId === obj.id ? 'selected' : ''}`}
          onClick={() => onSelect(obj.id)}
          title={`Tag: ${obj.tag}\nPosition: ${obj.position.map((v) => v.toFixed(2)).join(', ')}`}
        >
          <span className={`entity-icon entity-icon-${obj.tag.toLowerCase()}`} />
          <span className="entity-name">{obj.name}</span>
          <span className="entity-tag">{obj.tag}</span>
        </div>
      ))}
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
      // Debounced save: fire and forget
      rpc('shell/update_transform', { id: selectedId, [key]: arr });
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
    return <p className="panel-empty">Nothing selected</p>;
  }

  if (!details) {
    return <p className="panel-empty">Loading...</p>;
  }

  const labelAxis = (label: string) => ['X', 'Y', 'Z', 'W'].map((a) => `${label}${a}`);

  const vec3Inputs = (
    key: TransformKey,
    values: number[],
    decimals = 2,
  ) => (
    <div className="inspector-vec3">
      {values.map((v, i) => (
        <input
          key={i}
          type="text"
          value={v.toFixed(decimals)}
          onChange={(e) => updateTransform(key, i, e.target.value)}
          onBlur={() => loadDetails()}
          title={labelAxis(key)[i] || String(i)}
        />
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
            {c.type !== 'Camera' && ( // keep at least Camera
              <button
                className="inspector-remove-btn"
                onClick={() => removeComponent(c.type)}
                title={`Remove ${c.type}`}
              >
                ×
              </button>
            )}
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

  // Refresh data
  const refresh = useCallback(async () => {
    try {
      const [state, { objects }, { entries }] = await Promise.all([
        rpc<ShellState>('shell/get_state'),
        rpc<{ objects: SceneObject[] }>('shell/get_scene_tree'),
        rpc<{ entries: ConsoleEntry[] }>('console/get_entries'),
      ]);
      setShellState(state);
      setSceneTree(objects);
      setConsoleEntries(entries);
    } catch {
      // not ready
    }
  }, []);

  useEffect(() => {
    refresh();
    const interval = setInterval(refresh, 2000);
    return () => clearInterval(interval);
  }, [refresh]);

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
      {/* Custom Title Bar */}
      <header className="editor-titlebar">
        {/* Left: app icon + title */}
        <div className="titlebar-left">
          <svg className="titlebar-icon" width="16" height="16" viewBox="0 0 16 16">
            <polygon points="8,1 15,5 15,11 8,15 1,11 1,5" fill="var(--accent)" opacity="0.8" />
          </svg>
          <span className="titlebar-label">
            {shellState.project_name || 'Aster Editor'}
            {shellState.scene_dirty ? ' \u25CF' : ''}
          </span>
        </div>

        {/* Center: toolbar */}
        <div className="titlebar-center">
          <button
            className="tool-btn"
            onClick={() => rpc('shell/undo')}
            disabled={!shellState.can_undo}
            title="Undo"
          >↩</button>
          <button
            className="tool-btn"
            onClick={() => rpc('shell/redo')}
            disabled={!shellState.can_redo}
            title="Redo"
          >↪</button>
          <div className="titlebar-sep" />
          <button className="tool-btn play-btn" onClick={openGameView} title="Play (opens Game View window)">
            ▶ Play
          </button>
          <div className="titlebar-sep" />
          <button className="tool-btn" onClick={onCloseProject} title="Close Project">
            Close
          </button>
        </div>


      </header>

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
          />
        </ResizablePanel>

        <main className="panel panel-center">
          <div className="panel-header">
            <span>Scene View</span>
          </div>
          <ViewportCanvas />
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
          <InspectorPanel selectedId={selectedId} onRefresh={refresh} />
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
        <span className="status-item" style={{ color: 'var(--accent)' }}>
          v0.1.0
        </span>
      </footer>
    </div>
  );
}
