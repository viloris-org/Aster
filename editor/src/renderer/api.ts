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

/**
 * Window controls for the custom title bar.
 */
import { getCurrentWindow } from '@tauri-apps/api/window';

const win = getCurrentWindow();

export async function minimizeWindow(): Promise<void> {
  await win.minimize();
}

export async function toggleMaximizeWindow(): Promise<void> {
  await win.toggleMaximize();
}

export async function closeWindow(): Promise<void> {
  await win.close();
}

export function useMaximized(): boolean {
  // Note: In Tauri 2.0, we can listen for resize events
  // For now, we toggle without tracking state
  return false;
}
