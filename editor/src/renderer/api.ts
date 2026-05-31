// Tauri IPC wrapper — matches the old window.aster.rpc() signature.
// Swap to direct typed invoke() calls later if needed.

import { invoke } from '@tauri-apps/api/core';

/**
 * Call an editor backend method via JSON-RPC-style dispatch.
 *
 * @param method  Method name, e.g. "hub/get_state", "shell/get_scene_tree"
 * @param params  Optional parameters object
 */
export function rpc<T = unknown>(method: string, params?: unknown): Promise<T> {
  return invoke<T>('rpc', { method, params: params ?? {} });
}

/**
 * Viewport readback as raw RGBA via binary IPC, with lazy rendering support.
 *
 * When `lastVersion` matches the backend's scene version, the backend skips
 * rendering entirely and returns a 0-size buffer — no GPU work, no IPC transfer.
 *
 * Returns an ArrayBuffer with layout:
 *   [0..4)   width  (u32 LE) — 0 means "no change"
 *   [4..8)   height (u32 LE)
 *   [8..end) RGBA pixels (width × height × 4 bytes)
 */
export function viewportReadback(params: {
  width: number;
  height: number;
  lastVersion?: number;
  yaw?: number;
  pitch?: number;
  distance?: number;
  targetX?: number;
  targetY?: number;
  targetZ?: number;
  playMode?: boolean;
}): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>('viewport_readback_raw', {
    width: params.width,
    height: params.height,
    yaw: params.yaw ?? -0.5,
    pitch: params.pitch ?? 0.3,
    distance: params.distance ?? 6.0,
    target_x: params.targetX ?? 0,
    target_y: params.targetY ?? 0,
    target_z: params.targetZ ?? 0,
    last_version: params.lastVersion ?? null,
    play_mode: params.playMode ?? false,
  });
}

/**
 * Listen for push events from the Rust host.
 * Returns an unsubscribe function.
 */
import { listen, UnlistenFn } from '@tauri-apps/api/event';

export function onHostEvent(callback: (event: unknown) => void): Promise<UnlistenFn> {
  return listen<unknown>('host-event', (event) => {
    callback(event.payload);
  });
}

/**
 * Open the Game View in a separate Tauri window.
 */
export async function openGameView(): Promise<void> {
  await invoke('open_game_view');
}

export function selectProjectLocation(): Promise<string | null> {
  return invoke<string | null>('select_project_location');
}

/**
 * Show a native file-open dialog for scene JSON files,
 * then load the selected scene via RPC.
 * Returns the opened path, or null if cancelled.
 */
import { open, save } from '@tauri-apps/plugin-dialog';

export async function openScene(): Promise<string | null> {
  const selected = await open({
    title: 'Open Scene',
    filters: [{ name: 'Scene JSON', extensions: ['json', 'scene'] }],
    multiple: false,
  });
  if (!selected) return null;
  const result = await rpc<{ path: string }>('shell/open_scene', { path: selected });
  return result.path;
}

/**
 * Show a native Save-As dialog for scene JSON files,
 * then save the current scene to the selected path via RPC.
 * Returns the saved path, or null if cancelled.
 */
export async function saveSceneAs(): Promise<string | null> {
  const selected = await save({
    title: 'Save Scene As',
    filters: [{ name: 'Scene JSON', extensions: ['json', 'scene'] }],
  });
  if (!selected) return null;
  const result = await rpc<{ path: string }>('shell/save_scene_as', { path: selected });
  return result.path;
}

export function startPlayMode(): Promise<unknown> {
  return rpc('play/start');
}

export function stopPlayMode(): Promise<unknown> {
  return rpc('play/stop');
}
