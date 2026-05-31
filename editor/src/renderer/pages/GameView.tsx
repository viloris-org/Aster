import React, { useEffect, useRef } from 'react';
import { startPlayMode, stopPlayMode, viewportReadback } from '../api';

/**
 * Game View — standalone fullscreen render target launched from the editor.
 * Renders via binary IPC (raw RGBA, no PNG/base64 overhead).
 */
export default function GameView() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const sizeRef = useRef({ width: 1280, height: 720 });
  const isActiveRef = useRef(true);

  // Poll for frames via binary IPC with lazy rendering at ~60fps
  useEffect(() => {
    isActiveRef.current = true;
    startPlayMode().catch(() => {});

    const poll = async () => {
      if (!isActiveRef.current) return;
      const { width, height } = sizeRef.current;
      try {
        const buffer = await viewportReadback({
          width, height,
          playMode: true,
        });
        if (!isActiveRef.current || !canvasRef.current) return;

        // Parse header: [width: u32 LE][height: u32 LE][RGBA pixels...]
        const uint8 = new Uint8Array(buffer);
        const header = new Uint32Array(uint8.buffer, uint8.byteOffset, 2);
        const w = header[0];
        const h = header[1];

        // w === 0 means "no change" (GPU render skipped on backend)
        if (w > 0 && h > 0) {
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
      setTimeout(poll, 16); // ~60fps target
    };

    poll();
    return () => {
      isActiveRef.current = false;
      stopPlayMode().catch(() => {});
    };
  }, []);

  // Fill the window
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

  // Close on Escape
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') window.close();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, []);

  return (
    <div ref={containerRef} className="gameview-container">
      <canvas ref={canvasRef} className="gameview-canvas" />
    </div>
  );
}
