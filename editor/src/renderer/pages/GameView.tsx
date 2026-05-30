import React, { useEffect, useRef, useState } from 'react';
import { rpc } from '../api';

interface ViewportFrame {
  width: number;
  height: number;
  png_base64: string;
}

/**
 * Game View — standalone fullscreen render target launched from the editor.
 * Runs its own viewport polling loop independent of the editor Scene View.
 */
export default function GameView() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const sizeRef = useRef({ width: 1280, height: 720 });

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
      frameId = window.setTimeout(poll, 16); // ~60fps target
    };

    poll();
    return () => { active = false; clearTimeout(frameId); };
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
