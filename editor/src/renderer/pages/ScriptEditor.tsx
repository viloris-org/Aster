import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from '../i18n';
import { toolButtonClass } from '../uiClasses';
import { detectLanguage, highlightCode } from './syntaxHighlight';

// ─── Types ──────────────────────────────────────────────────────────────────

interface ScriptEditorProps {
  filePath: string;
  initialContent: string;
  onSave: (path: string, content: string) => Promise<void>;
  onClose: () => void;
}

const editorClass = 'flex h-full flex-col bg-[var(--bg-base)]';
const headerClass = 'flex flex-shrink-0 items-center gap-2 border-b border-[var(--border)] bg-[var(--bg-surface)] px-2 py-1';
const titleClass = 'text-xs font-medium text-[var(--text-primary)]';
const languageClass = 'rounded-[var(--radius-sm)] bg-[var(--accent-dim)] px-1.5 py-px text-[10px] text-[var(--accent)]';
const dirtyClass = 'text-[10px] text-[var(--accent)]';
const actionsClass = 'ml-auto flex gap-1';
const findBarClass = 'flex items-center gap-1 border-b border-[var(--border)] bg-[var(--bg-surface)] px-2 py-1';
const findInputClass = 'flex-1 rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--bg-base)] px-1.5 py-0.5 font-[var(--font-mono)] text-xs text-[var(--text-primary)] outline-none focus:border-[var(--accent)]';
const bodyClass = 'flex min-h-0 flex-1 overflow-hidden';
const gutterClass = 'w-10 flex-shrink-0 select-none overflow-hidden border-r border-[var(--border)] bg-[var(--bg-surface)] pt-0.5';
const gutterLineClass = 'h-[1.6em] pr-2 text-right font-[var(--font-mono)] text-[11px] leading-[1.6] text-[var(--text-muted)]';
const codeAreaClass = 'relative flex-1 overflow-hidden';
const highlightClass = 'pointer-events-none absolute inset-0 m-0 overflow-auto whitespace-pre p-0.5 px-2 font-[var(--font-mono)] text-xs leading-[1.6] text-[var(--text-primary)] [tab-size:2] [&_code]:font-inherit [&_code]:text-inherit';
const textareaClass = 'absolute inset-0 h-full w-full resize-none overflow-auto whitespace-pre border-0 bg-transparent p-0.5 px-2 font-[var(--font-mono)] text-xs leading-[1.6] text-transparent caret-[var(--text-primary)] outline-none [tab-size:2] selection:bg-[var(--accent-dim)]';

// ─── Component ──────────────────────────────────────────────────────────────

export default function ScriptEditor({ filePath, initialContent, onSave, onClose }: ScriptEditorProps) {
  const { t } = useTranslation();
  const [content, setContent] = useState(initialContent);
  const [dirty, setDirty] = useState(false);
  const [findOpen, setFindOpen] = useState(false);
  const [findText, setFindText] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const highlightRef = useRef<HTMLPreElement>(null);
  const language = detectLanguage(filePath) ?? 'rhai';

  // Sync scroll between textarea and highlighted overlay
  const syncScroll = useCallback(() => {
    if (textareaRef.current && highlightRef.current) {
      highlightRef.current.scrollTop = textareaRef.current.scrollTop;
      highlightRef.current.scrollLeft = textareaRef.current.scrollLeft;
    }
  }, []);

  // Generate line numbers
  const lineCount = content.split('\n').length;
  const lineNumbers = Array.from({ length: Math.max(1, lineCount) }, (_, i) => i + 1);

  // Highlighted HTML
  const highlightedHtml = highlightCode(content, language);

  // Save handler
  const handleSave = useCallback(async () => {
    try {
      await onSave(filePath, content);
      setDirty(false);
    } catch (e) {
      console.error('[script-editor] save error:', e);
    }
  }, [filePath, content, onSave]);

  // Keyboard shortcuts
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // Ctrl+S → Save
      if ((e.ctrlKey || e.metaKey) && e.key === 's') {
        e.preventDefault();
        handleSave();
        return;
      }
      // Ctrl+F → Find
      if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
        e.preventDefault();
        setFindOpen(true);
        return;
      }
      // Escape → Close find
      if (e.key === 'Escape' && findOpen) {
        setFindOpen(false);
      }
    };

    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [handleSave, findOpen]);

  return (
    <div className={editorClass}>
      {/* Header */}
      <div className={headerClass}>
        <span className={titleClass}>{filePath}</span>
        <span className={languageClass}>{language}</span>
        {dirty && <span className={dirtyClass}>●</span>}
        <div className={actionsClass}>
          <button
            className={toolButtonClass()}
            onClick={handleSave}
            disabled={!dirty}
            title={t('script_save_title')}
          >
            {t('btn_save')}
          </button>
          <button
            className={toolButtonClass()}
            onClick={onClose}
            title={t('btn_close')}
          >
            × {t('btn_close')}
          </button>
        </div>
      </div>

      {/* Find bar */}
      {findOpen && (
        <div className={findBarClass}>
          <input
            className={findInputClass}
            type="text"
            placeholder={t('script_find_placeholder')}
            value={findText}
            onChange={(e) => setFindText(e.target.value)}
            autoFocus
          />
          <button className={toolButtonClass()} onClick={() => setFindOpen(false)}>×</button>
        </div>
      )}

      {/* Editor area */}
      <div className={bodyClass}>
        {/* Line numbers gutter */}
        <div className={gutterClass}>
          {lineNumbers.map(n => (
            <div key={n} className={gutterLineClass}>{n}</div>
          ))}
        </div>

        {/* Code area with highlighting overlay */}
        <div className={codeAreaClass}>
          <pre
            ref={highlightRef}
            className={highlightClass}
            aria-hidden="true"
          >
            <code dangerouslySetInnerHTML={{ __html: highlightedHtml + '\n' }} />
          </pre>
          <textarea
            ref={textareaRef}
            className={textareaClass}
            value={content}
            onChange={(e) => {
              setContent(e.target.value);
              setDirty(true);
            }}
            onScroll={syncScroll}
            onKeyDown={(e) => {
              if (e.key === 'Tab') {
                e.preventDefault();
                const ta = e.target as HTMLTextAreaElement;
                const start = ta.selectionStart;
                const end = ta.selectionEnd;
                const newContent = content.slice(0, start) + '  ' + content.slice(end);
                setContent(newContent);
                setDirty(true);
                // Restore cursor position after React re-render
                requestAnimationFrame(() => {
                  ta.selectionStart = ta.selectionEnd = start + 2;
                });
              }
            }}
            spellCheck={false}
          />
        </div>
      </div>
    </div>
  );
}
