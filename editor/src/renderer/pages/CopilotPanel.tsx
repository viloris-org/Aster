import React, { useCallback, useEffect, useRef, useState } from 'react';
import { rpc } from '../api';
import { useTranslation } from '../i18n';

// ─── Types ──────────────────────────────────────────────────────────────────

interface CopilotOperation {
  index: number;
  preview: string;
  requires_write: boolean;
}

interface CopilotPlan {
  operations: CopilotOperation[];
  read_only: boolean;
  requires_write: boolean;
}

interface TraceEntry {
  tool: string;
  result: string;
  recovery_hint: string;
}

interface ConsoleEntry {
  level: string;
  message: string;
  subsystem: string;
}

interface ApplyResult {
  operations_performed: number;
  completed: boolean;
  summary: string | null;
  trace_entries: TraceEntry[];
  console_entries: ConsoleEntry[];
}

type CopilotStatus = 'idle' | 'planning' | 'ready' | 'executing' | 'complete' | 'error';

// ─── SVG Icons ──────────────────────────────────────────────────────────────

const IconSend = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="14" height="14">
    <line x1="22" y1="2" x2="11" y2="13" /><polygon points="22 2 15 22 11 13 2 9 22 2" />
  </svg>
);

const IconBot = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="14" height="14">
    <rect x="3" y="11" width="18" height="12" rx="2" ry="2" /><path d="M7 11V7a5 5 0 0 1 10 0v4" />
    <circle cx="9" cy="16" r="1" /><circle cx="15" cy="16" r="1" />
  </svg>
);

const IconCheck = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="12" height="12">
    <polyline points="20 6 9 17 4 12" />
  </svg>
);

const IconX = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="12" height="12">
    <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
  </svg>
);

const IconAlertCircle = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="14" height="14">
    <circle cx="12" cy="12" r="10" /><line x1="12" y1="8" x2="12" y2="12" /><line x1="12" y1="16" x2="12.01" y2="16" />
  </svg>
);

const IconChevronDown = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="12" height="12">
    <polyline points="6 9 12 15 18 9" />
  </svg>
);

const IconChevronRight = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="12" height="12">
    <polyline points="9 18 15 12 9 6" />
  </svg>
);

const IconInfo = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="12" height="12">
    <circle cx="12" cy="12" r="10" /><line x1="12" y1="16" x2="12" y2="12" /><line x1="12" y1="8" x2="12.01" y2="8" />
  </svg>
);

const IconLoader = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="14" height="14" className="spin-icon">
    <line x1="12" y1="2" x2="12" y2="6" /><line x1="12" y1="18" x2="12" y2="22" />
    <line x1="4.93" y1="4.93" x2="7.76" y2="7.76" /><line x1="16.24" y1="16.24" x2="19.07" y2="19.07" />
    <line x1="2" y1="12" x2="6" y2="12" /><line x1="18" y1="12" x2="22" y2="12" />
    <line x1="4.93" y1="19.07" x2="7.76" y2="16.24" /><line x1="16.24" y1="7.76" x2="19.07" y2="4.93" />
  </svg>
);

// ─── Copilot Message ─────────────────────────────────────────────────────────

function MessageBubble({ role, content }: { role: string; content: string }) {
  return (
    <div className={`copilot-message copilot-message-${role}`}>
      <div className="copilot-message-avatar">
        {role === 'assistant' ? <IconBot /> : <span>U</span>}
      </div>
      <div className="copilot-message-content">{content}</div>
    </div>
  );
}

// ─── Status Badge ────────────────────────────────────────────────────────────

function StatusBadge({ status }: { status: CopilotStatus }) {
  const { t } = useTranslation();
  const config: Record<CopilotStatus, { label: string; className: string }> = {
    idle: { label: '', className: '' },
    planning: { label: t('copilot_status_planning'), className: 'badge-copilot-planning' },
    ready: { label: t('copilot_status_ready'), className: 'badge-copilot-ready' },
    executing: { label: t('copilot_status_executing'), className: 'badge-copilot-executing' },
    complete: { label: t('copilot_status_complete'), className: 'badge-copilot-complete' },
    error: { label: t('copilot_status_error'), className: 'badge-copilot-error' },
  };
  const c = config[status];
  if (!c.label) return null;
  return <span className={`badge-copilot ${c.className}`}>{c.label}</span>;
}

// ─── Copilot Panel ───────────────────────────────────────────────────────────

export default function CopilotPanel() {
  const { t } = useTranslation();
  const [input, setInput] = useState('');
  const [messages, setMessages] = useState<{ role: string; content: string }[]>([]);
  const [status, setStatus] = useState<CopilotStatus>('idle');
  const [plan, setPlan] = useState<CopilotPlan | null>(null);
  const [approved, setApproved] = useState<Set<number>>(new Set());
  const [traceExpanded, setTraceExpanded] = useState(false);
  const [trace, setTrace] = useState<TraceEntry[]>([]);
  const [autoAccept, setAutoAccept] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Auto-scroll to bottom
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages, plan, trace]);

  // Focus input on mount
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // ── Submit prompt ──

  const submitPrompt = useCallback(async (prompt: string) => {
    if (!prompt.trim() || status === 'planning' || status === 'executing') return;

    setMessages(prev => [...prev, { role: 'user', content: prompt }]);
    setInput('');
    setStatus('planning');
    setPlan(null);
    setApproved(new Set());
    setTrace([]);
    setTraceExpanded(false);
    setErrorMsg(null);

    try {
      const result = await rpc<CopilotPlan>('copilot/plan', { prompt });
      setPlan(result);
      // Auto-approve read-only operations
      const autoApproved = new Set<number>();
      result.operations.forEach((op) => {
        if (!op.requires_write) autoApproved.add(op.index);
      });
      setApproved(autoApproved);
      setStatus('ready');

      setMessages(prev => [...prev, {
        role: 'assistant',
        content: result.operations.length > 0
          ? t('copilot_planned_ops').replace('{count}', String(result.operations.length))
          : t('copilot_no_ops'),
      }]);

      // Auto-execute if all ops are read-only and auto-accept is on
      if (autoAccept && result.operations.length > 0 && !result.requires_write) {
        await executeApproved(autoApproved);
      }
    } catch (err: any) {
      const msg = typeof err === 'string' ? err : err.message || 'Unknown error';
      setStatus('error');
      setErrorMsg(msg);
      setMessages(prev => [...prev, {
        role: 'assistant',
        content: `Error: ${msg}`,
      }]);
    }
  }, [status, autoAccept]);

  // ── Execute approved operations ──

  const executeApproved = useCallback(async (approvedSet?: Set<number>) => {
    const indices = Array.from(approvedSet ?? approved);
    if (indices.length === 0) return;

    setStatus('executing');
    setErrorMsg(null);

    try {
      const result = await rpc<ApplyResult>('copilot/apply', {
        approved_indices: indices,
      });

      setTrace(result.trace_entries);
      if (result.trace_entries.length > 0) {
        setTraceExpanded(true);
      }

      setStatus('complete');

      const summary = result.summary
        ? `✅ ${result.summary}`
        : `✅ ${t('copilot_applied_ops').replace('{count}', String(result.operations_performed))}`;
      setMessages(prev => [...prev, { role: 'assistant', content: summary }]);
      setPlan(null);
    } catch (err: any) {
      const msg = typeof err === 'string' ? err : err.message || 'Unknown error';
      setStatus('error');
      setErrorMsg(msg);
      setMessages(prev => [...prev, {
        role: 'assistant',
        content: `❌ ${msg}`,
      }]);
    }
  }, [approved]);

  // ── Toggle approval for an operation ──

  const toggleApproval = useCallback((index: number) => {
    setApproved(prev => {
      const next = new Set(prev);
      if (next.has(index)) next.delete(index);
      else next.add(index);
      return next;
    });
  }, []);

  // ── Keyboard handler ──

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      submitPrompt(input);
    }
  }, [input, submitPrompt]);

  // ── Render ──

  const approvedCount = approved.size;
  const hasPlan = plan && plan.operations.length > 0;

  return (
    <div className="copilot-panel">
      {/* Messages */}
      <div ref={scrollRef} className="copilot-messages">
        {messages.length === 0 && (
          <div className="copilot-empty">
            <IconBot />
            <p>{t('copilot_empty_hint')}</p>
          </div>
        )}
        {messages.map((msg, i) => (
          <MessageBubble key={i} role={msg.role} content={msg.content} />
        ))}

        {/* Plan Preview */}
        {hasPlan && status === 'ready' && (
          <div className="copilot-plan">
            <div className="copilot-plan-header">
              <IconInfo />
              <span>{t('copilot_plan_title')}</span>
              <StatusBadge status={status} />
            </div>
            {plan.operations.map((op) => (
              <label
                key={op.index}
                className={`copilot-plan-item ${!op.requires_write ? 'auto-approved' : ''}`}
              >
                {op.requires_write ? (
                  <input
                    type="checkbox"
                    checked={approved.has(op.index)}
                    onChange={() => toggleApproval(op.index)}
                  />
                ) : (
                  <span className="copilot-plan-badge-read"><IconCheck /></span>
                )}
                <span className="copilot-plan-preview">{op.preview}</span>
                {op.requires_write && (
                  <span className="copilot-plan-badge">{t('copilot_badge_write')}</span>
                )}
              </label>
            ))}
            <div className="copilot-plan-actions">
              <button
                className="btn btn-primary btn-sm"
                disabled={approvedCount === 0}
                onClick={() => executeApproved()}
              >
                {t('copilot_apply').replace('{count}', String(approvedCount))}
              </button>
              <button
                className="btn btn-ghost btn-sm"
                onClick={() => { setPlan(null); setStatus('idle'); }}
              >
                {t('copilot_reject')}
              </button>
            </div>
          </div>
        )}

        {/* Executing indicator */}
        {status === 'executing' && (
          <div className="copilot-executing">
            <IconLoader />
            <span>{t('copilot_executing')}</span>
          </div>
        )}

        {/* Error */}
        {errorMsg && (
          <div className="copilot-error">
            <IconAlertCircle />
            <span>{errorMsg}</span>
          </div>
        )}
      </div>

      {/* Trace (collapsible) */}
      {trace.length > 0 && (
        <div className="copilot-trace">
          <button
            className="copilot-trace-toggle"
            onClick={() => setTraceExpanded(!traceExpanded)}
          >
            {traceExpanded ? <IconChevronDown /> : <IconChevronRight />}
            <span>{t('copilot_trace')} ({trace.length})</span>
          </button>
          {traceExpanded && (
            <div className="copilot-trace-entries">
              {trace.map((entry, i) => (
                <div key={i} className={`copilot-trace-entry copilot-trace-${entry.result === 'applied' ? 'success' : entry.result.startsWith('failed') ? 'fail' : ''}`}>
                  <span className="copilot-trace-tool">{entry.tool}</span>
                  <span className="copilot-trace-result">{entry.result}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Auto-accept toggle */}
      <div className="copilot-options">
        <label className="copilot-auto-accept">
          <input
            type="checkbox"
            checked={autoAccept}
            onChange={(e) => setAutoAccept(e.target.checked)}
          />
          <span>{t('copilot_auto_accept')}</span>
        </label>
      </div>

      {/* Input */}
      <div className="copilot-input-row">
        <input
          ref={inputRef}
          className="copilot-input"
          type="text"
          placeholder={t('copilot_input_placeholder')}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={status === 'planning' || status === 'executing'}
        />
        <button
          className="copilot-send-btn"
          onClick={() => submitPrompt(input)}
          disabled={!input.trim() || status === 'planning' || status === 'executing'}
          title={t('copilot_send')}
        >
          <IconSend />
        </button>
      </div>
    </div>
  );
}
