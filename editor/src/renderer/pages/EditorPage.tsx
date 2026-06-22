import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  fetchSceneGuides,
  openGameView,
  openNativeSceneView,
  rpc,
  viewportReadback,
} from '../api';
import { useTranslation } from '../i18n';
import {
  buttonClass,
  productEmptyClass,
  productEmptyIconClass,
  productEmptyTextClass,
  productEmptyTitleClass,
  taskOperationPermissionLabelClass,
  toolButtonClass,
} from '../uiClasses';
import AiPanel, { type AiWorkspaceState, type CopilotOperation } from './AiPanel';
import { CloseProjectDialog } from './Dialogs';
import { ViewportGrid, OrientationGizmo } from './ViewportOverlays';
import { type GuideEntity } from './SceneGuides';
import {
  type Vec3,
  createViewMatrix,
  createPerspectiveMatrix,
  createOrthographicMatrix,
  projectToScreen,
} from './gizmoMath';
import {
  IconAlertCircle,
  IconAudio,
  IconBot,
  IconCheck,
  IconChevronDown,
  IconChevronRight,
  IconCode,
  IconCopy,
  IconFile,
  IconLoader,
  IconModel,
  IconPackage,
  IconPlay,
  IconPlus,
  IconProjects,
  IconRedo,
  IconRefresh,
  IconSave,
  IconSettings,
  IconSparkles,
  IconSun,
  IconTrash,
  IconUndo,
  IconView,
  IconX,
} from '../icons';
import type { QuestEditorArtifact } from '../App';

// ─── Types ──────────────────────────────────────────────────────────────────

interface ShellState {
  has_project: boolean;
  project_name?: string;
  scene_dirty: boolean;
  can_undo: boolean;
  can_redo: boolean;
  scene_version?: number;
}

interface SceneObject {
  id: string;
  name: string;
  tag: string;
  position: [number, number, number];
  parent_id?: string | null;
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
    data: Record<string, unknown>;
  }>;
}

type ComponentFieldKind = 'Bool' | 'F32' | 'String' | 'Vec3' | 'AssetRef' | 'Object';

interface ComponentFieldSchema {
  name: string;
  kind: ComponentFieldKind;
  default_value: string;
}

interface ComponentSchema {
  type_id: string;
  display_name: string;
  version: number;
  fields: ComponentFieldSchema[];
}

interface EditorConsoleEntry {
  timestamp: number;
  level: string;
  subsystem: string;
  file?: string | null;
  line?: number | null;
  message: string;
}

interface ProjectAssetMeta {
  guid: string;
  source_path: string;
  kind: string;
  importer: string;
}

interface AssetReferenceRow {
  kind: string;
  label: string;
  detail: string;
}

type DiagnosticHealthStatus = 'ok' | 'warning' | 'error' | 'not_configured';

interface DiagnosticFixAction {
  id: string;
  label: string;
  description: string;
  safe: boolean;
}

interface DiagnosticHealthItem {
  id: string;
  label: string;
  status: DiagnosticHealthStatus;
  summary: string;
  detail: string;
  evidence: string[];
  fixes: DiagnosticFixAction[];
}

interface DiagnosticHealthGroup {
  id: string;
  label: string;
  description: string;
  status: DiagnosticHealthStatus;
  items: DiagnosticHealthItem[];
}

interface DiagnosticHealthSummary {
  status: DiagnosticHealthStatus;
  score: number;
  total: number;
  ok: number;
  warning: number;
  error: number;
  not_configured: number;
}

interface DiagnosticHealthReport {
  scanned_at: number;
  summary: DiagnosticHealthSummary;
  groups: DiagnosticHealthGroup[];
}

interface DiagnosticFixResult {
  applied: boolean;
  fix_id: string;
  message: string;
}

interface AsterScriptDiagnostic {
  code: string;
  severity: 'error' | 'warning';
  line?: number;
  column?: number;
  message: string;
  suggestion: string;
  source_line?: string;
}

type TextAssetDiagnostic = AsterScriptDiagnostic;

interface Props {
  onCloseProject: () => void;
  onOpenSettings?: () => void;
  onOpenQuest?: () => void;
  questArtifact?: QuestEditorArtifact | null;
  onDismissQuestArtifact?: () => void;
}

interface ArtifactSelection {
  kind: 'model' | 'code' | 'document';
  label: string;
  context: string;
  x: number;
  y: number;
}

type WorkspaceView = 'prd' | 'tasks' | 'game' | 'assets' | 'scripts' | 'build' | 'diagnostics';
type ProjectAssetCreateKind = 'script' | 'material' | 'prefab' | 'scene';
type BuildTarget = 'windows-x64' | 'linux-x64' | 'macos-universal' | 'android-arm64' | 'ios-universal' | 'embedded-linux';
type BuildFormat = 'folder' | 'exe' | 'msi' | 'nsis' | 'appimage' | 'deb' | 'rpm' | 'dmg' | 'apk' | 'aab' | 'ipa' | 'ipk';
type BuildChannel = 'debug' | 'release';
type DiagnosticLevelFilter = 'all' | 'error' | 'warn' | 'info' | 'debug';

interface BuildTargetOption {
  id: BuildTarget;
  label: string;
  formats: BuildFormat[];
  status: 'ready' | 'planned' | 'blocked';
  note: string;
}

interface BuildPreset {
  id: string;
  label: string;
  target: BuildTarget;
  format: BuildFormat;
  channel: BuildChannel;
}

interface BuildPackageResult {
  project: string;
  target: string;
  format: string;
  channel: string;
  path: string;
  binary: string;
  launcher: string;
}

const CURRENT_DESKTOP_BUILD_TARGET: BuildTarget = (() => {
  const platform = navigator.platform.toLowerCase();
  const userAgent = navigator.userAgent.toLowerCase();
  if (platform.includes('mac') || userAgent.includes('mac os')) return 'macos-universal';
  if (platform.includes('win') || userAgent.includes('windows')) return 'windows-x64';
  return 'linux-x64';
})();

const BUILD_TARGETS: BuildTargetOption[] = [
  {
    id: 'macos-universal',
    label: 'macOS 通用',
    formats: ['folder', 'dmg'],
    status: CURRENT_DESKTOP_BUILD_TARGET === 'macos-universal' ? 'ready' : 'planned',
    note: CURRENT_DESKTOP_BUILD_TARGET === 'macos-universal'
      ? '当前后端可以导出本机 macOS 文件夹包。DMG、签名与公证仍是后续能力。'
      : '需要 macOS 构建机与签名/公证流程，当前主机不能直接产出。',
  },
  {
    id: 'linux-x64',
    label: 'Linux x64',
    formats: ['folder', 'appimage', 'deb', 'rpm'],
    status: CURRENT_DESKTOP_BUILD_TARGET === 'linux-x64' ? 'ready' : 'planned',
    note: CURRENT_DESKTOP_BUILD_TARGET === 'linux-x64'
      ? '当前后端可以导出本机 Linux 文件夹包。AppImage / deb / rpm 是后续安装器能力。'
      : '需要 Linux 构建机或交叉构建工具链，当前主机不能直接产出。',
  },
  {
    id: 'windows-x64',
    label: 'Windows x64',
    formats: ['folder', 'exe', 'msi', 'nsis'],
    status: CURRENT_DESKTOP_BUILD_TARGET === 'windows-x64' ? 'ready' : 'planned',
    note: CURRENT_DESKTOP_BUILD_TARGET === 'windows-x64'
      ? '当前后端可以导出本机 Windows 文件夹包。exe / msi / nsis 是后续安装器能力。'
      : '需要 Windows 构建机或交叉构建工具链，当前主机不能直接产出。',
  },
  {
    id: 'android-arm64',
    label: 'Android ARM64',
    formats: ['apk', 'aab'],
    status: 'blocked',
    note: '需要 Android 运行时适配、SDK/NDK 检测、签名和移动端资源打包。',
  },
  {
    id: 'ios-universal',
    label: 'iOS 通用',
    formats: ['ipa'],
    status: 'blocked',
    note: '需要 Apple 工具链、证书、描述文件、签名和移动端运行时支持。',
  },
  {
    id: 'embedded-linux',
    label: '嵌入式 Linux',
    formats: ['ipk', 'folder'],
    status: 'blocked',
    note: '需要设备 profile、架构、libc、安装路径与控制元数据。',
  },
];

const BUILD_PRESETS: BuildPreset[] = [
  { id: 'native-debug-folder', label: '本机调试包', target: CURRENT_DESKTOP_BUILD_TARGET, format: 'folder', channel: 'debug' },
  { id: 'native-release-folder', label: '本机发布包', target: CURRENT_DESKTOP_BUILD_TARGET, format: 'folder', channel: 'release' },
  { id: 'macos-dmg-plan', label: 'macOS 安装包', target: 'macos-universal', format: 'dmg', channel: 'release' },
  { id: 'windows-installer-plan', label: 'Windows 安装器', target: 'windows-x64', format: 'nsis', channel: 'release' },
];

function cx(...classes: Array<string | false | null | undefined>): string {
  return classes.filter(Boolean).join(' ');
}

const shellClass = {
  loading: 'flex h-screen items-center justify-center bg-[var(--bg-base)] text-[var(--text-secondary)]',
  root: 'relative flex h-full w-full min-h-0 flex-col overflow-hidden bg-[radial-gradient(circle_at_22%_-18%,rgba(106,215,229,0.10),transparent_34rem),radial-gradient(circle_at_88%_-8%,rgba(var(--brand-rgb),0.13),transparent_36rem),linear-gradient(135deg,#05080d_0%,#070d14_42%,#05080d_100%)] before:pointer-events-none before:absolute before:inset-0 before:z-0 before:bg-[linear-gradient(rgba(255,255,255,0.018)_1px,transparent_1px),linear-gradient(90deg,rgba(255,255,255,0.014)_1px,transparent_1px)] before:bg-[size:44px_44px] before:[mask-image:radial-gradient(circle_at_50%_8%,black,transparent_72%)] [&>*]:relative [&>*]:z-[1]',
  toolbar: 'flex min-h-[58px] items-center gap-3 border-b border-white/[0.08] bg-[linear-gradient(180deg,rgba(10,16,27,0.94),rgba(5,8,14,0.84))] px-4 shadow-[0_12px_32px_rgba(0,0,0,0.34)] backdrop-blur-2xl',
  brand: 'flex min-w-[228px] items-center gap-3',
  brandMark: 'grid h-8 w-8 place-items-center rounded-[10px] border border-[rgba(var(--brand-hover-rgb),0.30)] bg-[linear-gradient(135deg,#13252d,#3bc3dd)] text-[15px] font-black text-[var(--text-on-brand)] shadow-[0_0_30px_rgba(var(--brand-rgb),0.30)]',
  toolbarProject: 'flex min-w-0 flex-col justify-center leading-[1.15]',
  toolbarProjectKicker: 'text-[13px] font-bold tracking-[-0.02em] text-[var(--text-primary)]',
  toolbarProjectName: 'mt-0.5 text-[12px] font-medium text-[var(--text-muted)]',
  modeTabs: 'mx-auto flex h-10 items-center gap-1 rounded-[14px] border border-white/[0.10] bg-black/25 p-1 shadow-[inset_0_1px_0_rgba(255,255,255,0.04)]',
  modeTab: 'flex h-8 min-w-[76px] cursor-pointer items-center justify-center rounded-[10px] border border-transparent px-4 text-[13px] font-semibold text-[var(--text-muted)] transition-all hover:bg-white/[0.05] hover:text-[var(--text-primary)]',
  modeTabActive: 'border-[rgba(var(--brand-hover-rgb),0.44)] bg-[linear-gradient(180deg,rgba(var(--brand-rgb),0.22),rgba(var(--brand-rgb),0.10))] text-[var(--text-primary)] shadow-[inset_0_-2px_0_var(--brand),0_0_22px_rgba(var(--brand-rgb),0.12)]',
  toolbarActions: 'ml-auto flex min-w-[310px] justify-end items-center gap-2',
  toolbarStatus: 'inline-flex h-8 items-center gap-2 rounded-full border border-[rgba(73,217,139,0.22)] bg-[rgba(73,217,139,0.08)] px-3 text-[12px] font-semibold text-[var(--success)]',
  toolbarDivider: 'mx-1 h-6 w-px bg-white/[0.10]',
  body: 'flex min-h-0 flex-1 overflow-hidden',
  statusbar: 'flex h-[30px] min-h-[30px] select-none items-center justify-between border-t border-white/[0.08] bg-[rgba(5,8,14,0.90)] px-4 text-[12px] backdrop-blur-xl',
  statusGroup: 'flex min-w-0 items-center gap-4',
  statusDivider: 'h-3 w-px flex-none bg-white/[0.10]',
  statusItem: 'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-[var(--text-secondary)]',
  statusSelection: 'text-[var(--accent-hover)]',
  statusSaved: 'text-[var(--success)]',
  statusDirty: 'flex items-center gap-[7px] text-[var(--warning)]',
  statusDot: 'size-1.5 rounded-full bg-[var(--warning)] shadow-[0_0_10px_var(--warning)]',
  version: 'text-[var(--accent-hover)]',
};

const appRailClass = {
  root: 'flex w-[76px] shrink-0 flex-col border-r border-white/[0.08] bg-[linear-gradient(180deg,rgba(8,14,24,0.92),rgba(5,9,17,0.90))] py-3 shadow-[inset_-1px_0_0_rgba(255,255,255,0.03)] backdrop-blur-2xl',
  nav: 'flex flex-1 flex-col items-center gap-2 px-2',
  item: 'group relative flex h-[62px] w-full cursor-pointer flex-col items-center justify-center gap-1 rounded-[14px] border border-transparent bg-transparent text-[11px] font-semibold text-[var(--text-muted)] transition-all hover:border-white/[0.10] hover:bg-white/[0.05] hover:text-[var(--text-primary)] [&_svg]:size-5 [&_svg]:opacity-80',
  itemActive: 'border-[rgba(var(--brand-rgb),0.36)] bg-[linear-gradient(180deg,rgba(var(--brand-rgb),0.24),rgba(106,215,229,0.08))] text-[var(--text-primary)] shadow-[inset_3px_0_0_var(--brand),0_14px_34px_rgba(0,0,0,0.26)] [&_svg]:text-[var(--accent-hover)] [&_svg]:opacity-100',
  badge: 'absolute top-1.5 right-1.5 grid min-h-5 min-w-5 place-items-center rounded-full bg-[var(--warning)] px-1.5 font-mono text-[10px] font-bold text-black shadow-[0_0_16px_rgba(247,185,85,0.36)]',
  bottom: 'mt-auto flex flex-col items-center gap-2 px-2 pt-3',
};

const aiShellClass = {
  header: 'flex min-h-[60px] items-center justify-between border-b border-white/[0.08] bg-[linear-gradient(180deg,rgba(10,16,27,0.92),rgba(5,8,14,0.72))] px-4 backdrop-blur-xl',
  titleWrap: 'min-w-0',
  title: 'flex items-center gap-2 text-[16px] font-bold tracking-[-0.02em] text-[var(--text-primary)]',
  status: 'mt-1 inline-flex items-center gap-1.5 rounded-full border border-[rgba(247,185,85,0.28)] bg-[var(--warning-dim)] px-2.5 py-0.5 text-[11px] font-semibold text-[var(--warning)]',
  statusReady: 'border-[rgba(73,217,139,0.28)] bg-[var(--success-dim)] text-[var(--success)]',
  statusBusy: 'border-[rgba(106,215,229,0.24)] bg-[rgba(106,215,229,0.08)] text-[var(--info)]',
  actions: 'flex items-center gap-2',
};

const workspaceClass = {
  root: 'flex min-w-0 flex-1 flex-col overflow-hidden bg-[rgba(7,10,16,0.62)]',
  tabs: 'hidden',
  tab: 'group relative flex cursor-pointer items-center gap-2 border-0 bg-transparent px-3 text-[12px] font-medium text-[var(--text-muted)] transition-colors duration-150 hover:text-[var(--text-primary)] [&_svg]:size-[14px] [&_svg]:opacity-70 hover:[&_svg]:opacity-100',
  tabActive: 'tab-active text-[var(--text-primary)] [&_svg]:text-[var(--brand)] [&_svg]:opacity-100 after:absolute after:inset-x-2 after:bottom-0 after:h-[2px] after:rounded-full after:bg-[var(--brand)]',
  tabBadge: 'min-w-[18px] rounded-full bg-[var(--bg-active)] px-1.5 text-center text-[10px] font-semibold leading-[17px] text-[var(--text-secondary)] group-[.tab-active]:bg-[var(--brand-dim)] group-[.tab-active]:text-[var(--brand)]',
  view: 'min-h-0 flex-1 overflow-auto p-3',
  viewGame: 'flex overflow-hidden p-0',
  aiPanel: 'flex min-w-[360px] flex-col overflow-hidden border-l border-[rgba(var(--brand-hover-rgb),0.16)] bg-[linear-gradient(180deg,rgba(9,15,26,0.96),rgba(6,10,18,0.96))] shadow-[-22px_0_54px_rgba(0,0,0,0.30),inset_1px_0_0_rgba(255,255,255,0.035)] backdrop-blur-2xl max-[900px]:min-w-[330px] [&_.ai-context-selected]:hidden',
  aiRail: 'flex w-14 shrink-0 flex-col items-center gap-2 border-l border-white/[0.08] bg-[rgba(8,13,24,0.94)] px-2 py-3 backdrop-blur-2xl',
  aiRailButton: 'grid size-10 cursor-pointer place-items-center rounded-[14px] border border-[rgba(var(--brand-rgb),0.25)] bg-[var(--brand-dim)] text-[var(--accent-hover)] hover:border-[var(--accent)] hover:bg-[rgba(var(--brand-rgb),0.26)] hover:text-[var(--text-primary)]',
  aiRailBadge: 'grid size-6 place-items-center rounded-full bg-[var(--warning)] font-mono text-[10px] font-bold text-black',
  resizeHandle: 'relative z-10 w-1 shrink-0 cursor-col-resize bg-transparent hover:bg-[var(--accent)] hover:opacity-50 active:bg-[var(--accent)] active:opacity-60 focus-visible:bg-[var(--accent-dim)] focus-visible:outline focus-visible:outline-1 focus-visible:-outline-offset-1 focus-visible:outline-[var(--accent)]',
};

const prdClass = {
  document: 'mx-auto mt-[34px] mb-16 w-[min(820px,calc(100%_-_64px))] text-[var(--text-secondary)]',
  header: 'border-b border-[var(--border)] pb-7',
  kicker: 'text-[10px] font-bold tracking-[0.1em] text-[var(--text-secondary)] uppercase',
  title: 'my-2 block text-[28px] tracking-[-0.03em] text-[var(--text-primary)]',
  description: 'm-0 text-xs text-[var(--text-muted)]',
  section: 'border-b border-[var(--border)] py-[25px]',
  sectionTitle: 'mb-[13px] text-sm text-[var(--text-primary)]',
  bodyText: 'text-xs leading-[1.75]',
  list: 'm-0 pl-[18px]',
  grid: 'grid grid-cols-2 gap-2.5 max-[900px]:grid-cols-1',
  gridCard: 'rounded-[7px] border border-[var(--border)] bg-[var(--bg-surface)] p-3.5',
  gridLabel: 'mb-[5px] block text-[10px] text-[var(--text-muted)]',
  gridValue: 'block text-xs text-[var(--text-primary)]',
};

const taskClass = {
  board: 'flex h-full min-h-0 flex-col bg-[radial-gradient(circle_at_18%_-18%,rgba(106,215,229,0.10),transparent_30rem),radial-gradient(circle_at_82%_4%,rgba(var(--brand-rgb),0.12),transparent_30rem),var(--bg-base)]',
  header: 'flex min-h-[86px] items-center justify-between gap-4 border-b border-white/[0.08] bg-[rgba(7,10,16,0.74)] px-5 backdrop-blur-2xl',
  headerText: 'min-w-0',
  kicker: 'block text-[10px] font-bold tracking-[0.08em] text-[var(--text-muted)] uppercase',
  title: 'mt-1 block text-[22px] font-bold tracking-[-0.03em] text-[var(--text-primary)]',
  meta: 'mt-1 block text-[12px] leading-[1.45] text-[var(--text-secondary)]',
  headerActions: 'flex shrink-0 flex-wrap justify-end gap-2',
  layout: 'grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_300px] gap-4 overflow-hidden p-4 max-[1120px]:grid-cols-1',
  main: 'min-h-0 overflow-hidden rounded-[18px] border border-white/[0.10] bg-[rgba(8,13,24,0.64)] shadow-[var(--shadow-lg)] backdrop-blur-xl',
  summary: 'grid grid-cols-4 gap-2.5 border-b border-white/[0.08] bg-white/[0.025] p-3 max-[980px]:grid-cols-2',
  summaryCard: 'rounded-[13px] border border-white/[0.09] bg-white/[0.035] px-3 py-2.5',
  summaryLabel: 'block text-[10px] font-bold tracking-[0.08em] text-[var(--text-muted)] uppercase',
  summaryValue: 'mt-1 block font-mono text-[18px] font-semibold text-[var(--text-primary)]',
  progress: 'border-b border-white/[0.08] p-4',
  progressTrack: 'h-2 overflow-hidden rounded-full bg-white/[0.06]',
  progressFill: 'h-full rounded-full bg-[linear-gradient(90deg,var(--brand),var(--accent))] shadow-[0_0_18px_rgba(106,215,229,0.22)]',
  progressText: 'mt-2 flex items-center justify-between text-[11px] text-[var(--text-muted)]',
  completedCard: 'm-4 mb-0 rounded-[16px] border border-[rgba(73,217,139,0.24)] bg-[rgba(73,217,139,0.07)] p-4 shadow-[var(--shadow-sm)]',
  completedHeader: 'flex items-start gap-3 text-[var(--success)]',
  completedTitle: 'block text-[14px] font-semibold text-[var(--text-primary)]',
  completedText: 'mt-1 block text-[12px] leading-[1.55] text-[var(--text-secondary)]',
  completedMetrics: 'mt-3 grid grid-cols-3 gap-2 max-[760px]:grid-cols-1',
  completedMetric: 'rounded-[10px] border border-white/[0.08] bg-black/10 px-3 py-2 text-[11px] text-[var(--text-muted)] [&_b]:block [&_b]:font-mono [&_b]:text-[15px] [&_b]:text-[var(--text-primary)]',
  artifactCard: 'm-4 mb-0 grid grid-cols-[minmax(0,1fr)_auto] gap-3.5 rounded-[14px] border border-[rgba(var(--brand-rgb),0.22)] bg-[rgba(var(--brand-rgb),0.10)] p-3.5 max-[880px]:grid-cols-1',
  artifactKicker: 'block text-[10px] font-bold tracking-[0.08em] text-[var(--text-muted)] uppercase',
  artifactTitle: 'mt-1 block text-[14px] font-semibold text-[var(--text-primary)]',
  artifactDescription: 'mt-2 mb-0 text-[12px] leading-[1.55] text-[var(--text-secondary)]',
  artifactPath: 'mt-2 block overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[10px] text-[var(--text-muted)]',
  artifactActions: 'flex items-start gap-2',
  artifactButton: 'inline-flex min-h-8 cursor-pointer items-center gap-1.5 rounded-[10px] border border-white/[0.10] bg-white/[0.045] px-3 text-[11px] font-semibold text-[var(--text-secondary)] hover:border-[var(--accent)] hover:bg-[var(--accent-dim)] hover:text-[var(--text-primary)]',
  operations: 'm-4 overflow-hidden rounded-[16px] border border-white/[0.09] bg-white/[0.035] shadow-[var(--shadow-sm)]',
  operationsTitle: 'flex min-h-[50px] items-center justify-between gap-3 border-b border-white/[0.08] px-4 text-[12px] font-semibold text-[var(--text-primary)]',
  operationsHint: 'text-[11px] font-normal text-[var(--text-muted)]',
  operationList: 'grid gap-2 p-3',
  operationRow: 'grid grid-cols-[86px_minmax(0,1fr)_minmax(150px,auto)] items-start gap-3 rounded-[13px] border border-white/[0.08] bg-black/10 px-3 py-3 text-[12px] text-[var(--text-secondary)] transition-colors hover:border-white/[0.14] hover:bg-white/[0.035] max-[920px]:grid-cols-1',
  operationRowApproved: 'border-[rgba(73,217,139,0.22)] bg-[rgba(73,217,139,0.055)]',
  operationRowDenied: 'border-[rgba(239,68,68,0.22)] bg-[rgba(239,68,68,0.045)] opacity-75',
  operationRowPending: 'border-[rgba(247,185,85,0.18)]',
  operationPermission: 'inline-flex min-h-7 w-max items-center rounded-[9px] border px-2.5 font-mono text-[10px] font-bold uppercase',
  operationMain: 'min-w-0',
  operationPreview: 'm-0 text-[12px] leading-[1.55] text-[var(--text-secondary)] [overflow-wrap:anywhere]',
  operationMeta: 'mt-1 block text-[10px] text-[var(--text-muted)]',
  operationDecision: 'flex flex-wrap justify-end gap-1.5 max-[920px]:justify-start',
  operationState: 'whitespace-nowrap rounded-[9px] border border-white/[0.08] bg-white/[0.035] px-2.5 py-1.5 text-[10px] font-semibold text-[var(--text-muted)]',
  operationButton: 'inline-flex min-h-7 cursor-pointer items-center rounded-[8px] border border-white/[0.10] bg-white/[0.04] px-2.5 text-[10px] font-semibold text-[var(--text-secondary)] hover:border-[var(--accent)] hover:bg-[var(--accent-dim)] hover:text-[var(--text-primary)]',
  operationDenyButton: 'hover:border-[var(--danger)] hover:bg-[var(--danger-dim)] hover:text-[var(--danger)]',
  sidebar: 'min-h-0 overflow-auto rounded-[18px] border border-white/[0.10] bg-[rgba(8,13,24,0.64)] p-4 shadow-[var(--shadow-lg)] backdrop-blur-xl',
  sidebarSection: 'mb-3 rounded-[14px] border border-white/[0.08] bg-white/[0.035] p-3 last:mb-0',
  sidebarTitle: 'text-[12px] font-semibold text-[var(--text-primary)]',
  sidebarText: 'mt-1 text-[11px] leading-[1.6] text-[var(--text-secondary)]',
  checklist: 'mt-3 grid gap-2',
  checklistItem: 'flex items-center gap-2 rounded-[10px] border border-white/[0.06] bg-black/10 px-2.5 py-2 text-[11px] text-[var(--text-secondary)]',
  footer: 'flex flex-wrap justify-end gap-2 border-t border-white/[0.08] bg-black/10 px-4 py-3',
};

const surfaceClass = {
  root: 'flex h-full min-h-0 flex-col bg-[radial-gradient(circle_at_24%_-18%,rgba(106,215,229,0.10),transparent_28rem),radial-gradient(circle_at_88%_8%,rgba(var(--brand-rgb),0.12),transparent_30rem),var(--bg-base)]',
  header: 'flex min-h-[72px] items-center justify-between gap-4 border-b border-white/[0.08] bg-[rgba(7,10,16,0.74)] px-5 backdrop-blur-2xl',
  buildHeader: 'flex min-h-[52px] items-center justify-between gap-3 border-b border-[var(--border)] bg-[var(--bg-overlay)] px-4 backdrop-blur-xl',
  headerKicker: 'block text-[10px] font-bold tracking-[0.08em] text-[var(--text-muted)] uppercase',
  buildKicker: 'block text-[10px] font-bold tracking-[0.08em] text-[var(--text-muted)] uppercase',
  headerTitle: 'mt-1 block text-[20px] font-bold tracking-[-0.03em] text-[var(--text-primary)]',
  headerDesc: 'mt-1 block max-w-[58ch] text-[12px] leading-[1.45] text-[var(--text-secondary)]',
  buildHeaderTitle: 'mt-[3px] block text-[13px] text-[var(--text-primary)]',
  toolbar: 'flex min-w-0 flex-wrap justify-end gap-2',
  button: 'inline-flex min-h-9 cursor-pointer items-center gap-1.5 rounded-[10px] border border-white/[0.10] bg-white/[0.045] px-3 text-[12px] font-semibold text-[var(--text-secondary)] shadow-[var(--shadow-sm)] hover:border-[var(--accent)] hover:bg-[var(--accent-dim)] hover:text-[var(--text-primary)] disabled:cursor-not-allowed disabled:opacity-50',
  primaryButton: 'inline-flex min-h-8 cursor-pointer items-center gap-[7px] rounded-[var(--radius-md)] border border-[var(--accent-hover)] bg-[var(--accent-dim)] px-3 text-[11px] text-[var(--accent)] shadow-[var(--shadow-sm)] hover:border-[var(--accent)] disabled:cursor-not-allowed disabled:opacity-60',
  list: 'min-h-0 flex-1 overflow-auto p-4',
  empty: 'grid min-h-[260px] place-items-center rounded-[18px] border border-white/[0.10] bg-white/[0.025] text-center text-xs text-[var(--text-muted)]',
};

const assetsClass = {
  layout: 'grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_280px] gap-4 p-4 max-[1160px]:grid-cols-1',
  main: 'min-h-0 overflow-hidden rounded-[18px] border border-white/[0.10] bg-[rgba(8,13,24,0.64)] shadow-[var(--shadow-lg)] backdrop-blur-xl',
  summary: 'grid grid-cols-4 gap-2.5 border-b border-white/[0.08] bg-white/[0.025] p-3 max-[980px]:grid-cols-2',
  summaryCard: 'rounded-[13px] border border-white/[0.09] bg-white/[0.035] px-3 py-2.5',
  summaryLabel: 'block text-[10px] font-bold tracking-[0.08em] text-[var(--text-muted)] uppercase',
  summaryValue: 'mt-1 block font-mono text-[18px] font-semibold text-[var(--text-primary)]',
  filterBar: 'flex min-h-[48px] items-center gap-2 overflow-x-auto border-b border-white/[0.08] px-3 py-2',
  filterButton: 'inline-flex min-h-8 flex-none cursor-pointer items-center gap-1.5 rounded-[10px] border border-white/[0.08] bg-white/[0.035] px-3 text-[12px] font-semibold text-[var(--text-secondary)] hover:border-[var(--accent)] hover:bg-[var(--accent-dim)] hover:text-[var(--text-primary)]',
  filterButtonActive: 'border-[var(--accent)] bg-[var(--accent-dim)] text-[var(--text-primary)] shadow-[inset_0_-2px_0_var(--brand)]',
  tableHeader: 'grid grid-cols-[minmax(0,1.5fr)_100px_100px_auto] items-center gap-3 border-b border-white/[0.08] px-4 py-2 text-[10px] font-bold tracking-[0.08em] text-[var(--text-muted)] uppercase max-[900px]:hidden',
  row: 'grid grid-cols-[minmax(0,1.5fr)_100px_100px_auto] items-center gap-3 border-b border-white/[0.07] px-4 py-3 text-[var(--text-secondary)] transition-colors hover:bg-white/[0.035] max-[900px]:grid-cols-1 max-[900px]:items-start',
  rowMain: 'min-w-0',
  rowTitle: 'flex min-w-0 items-center gap-2 overflow-hidden text-ellipsis whitespace-nowrap text-[13px] font-semibold text-[var(--text-primary)] [&_svg]:size-4 [&_svg]:text-[var(--accent-hover)]',
  rowMeta: 'mt-1 block overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[10px] text-[var(--text-muted)]',
  pill: 'inline-flex min-h-7 items-center justify-start rounded-[8px] border border-white/[0.08] bg-white/[0.035] px-2.5 font-mono text-[10px] text-[var(--text-secondary)]',
  actions: 'flex justify-end gap-1.5 max-[900px]:justify-start',
  sidebar: 'min-h-0 overflow-auto rounded-[18px] border border-white/[0.10] bg-[rgba(8,13,24,0.64)] p-4 shadow-[var(--shadow-lg)] backdrop-blur-xl',
  sidebarSection: 'mb-3 rounded-[14px] border border-white/[0.08] bg-white/[0.035] p-3 last:mb-0',
  sidebarTitle: 'text-[12px] font-semibold text-[var(--text-primary)]',
  sidebarText: 'mt-1 text-[11px] leading-[1.55] text-[var(--text-secondary)]',
  sidebarList: 'mt-3 grid gap-2 text-[11px] text-[var(--text-secondary)]',
  sidebarItem: 'flex items-center justify-between gap-3 rounded-[10px] border border-white/[0.06] bg-black/10 px-2.5 py-2',
};

const buildClass = {
  layout: 'grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_320px] gap-4 overflow-hidden p-4 max-[1180px]:grid-cols-1',
  main: 'min-w-0 overflow-auto rounded-[18px] border border-white/[0.10] bg-[rgba(8,13,24,0.64)] shadow-[var(--shadow-lg)] backdrop-blur-xl',
  summary: 'grid grid-cols-4 gap-2.5 border-b border-white/[0.08] bg-white/[0.025] p-3 max-[1120px]:grid-cols-2',
  summaryCard: 'rounded-[13px] border border-white/[0.09] bg-white/[0.035] px-3 py-2.5',
  summaryLabel: 'block text-[10px] font-bold tracking-[0.08em] text-[var(--text-muted)] uppercase',
  summaryValue: 'mt-1 block overflow-hidden text-ellipsis whitespace-nowrap text-[13px] font-semibold text-[var(--text-primary)]',
  summaryMono: 'font-mono text-[12px]',
  section: 'border-b border-white/[0.08] p-4 last:border-b-0',
  presets: 'grid grid-cols-4 gap-2 max-[1180px]:grid-cols-2',
  presetButton: 'grid min-w-0 cursor-pointer grid-cols-[22px_minmax(0,1fr)] gap-x-2 gap-y-1 rounded-[14px] border border-white/[0.09] bg-white/[0.035] p-3 text-left text-[var(--text-secondary)] shadow-[var(--shadow-sm)] transition-[border-color,background-color,transform] duration-150 hover:-translate-y-px hover:border-[var(--accent)] hover:bg-[var(--accent-dim)] disabled:cursor-not-allowed disabled:opacity-45 disabled:hover:translate-y-0 disabled:hover:border-white/[0.09] disabled:hover:bg-white/[0.035] [&_svg]:row-span-2 [&_svg]:mt-0.5 [&_svg]:text-[var(--accent-hover)]',
  selectedButton: 'border-[var(--accent)] bg-[var(--accent-dim)] shadow-[inset_0_-2px_0_var(--brand)]',
  card: 'rounded-[16px] border border-white/[0.09] bg-white/[0.035] shadow-[var(--shadow-sm)]',
  sectionTitle: 'flex min-h-[50px] items-center justify-between gap-3 border-b border-white/[0.08] px-4',
  sectionValue: 'text-[11px] text-[var(--text-secondary)]',
  sectionHint: 'mt-1 text-[11px] leading-[1.45] text-[var(--text-muted)]',
  targetGrid: 'grid grid-cols-3 gap-2 p-3 max-[1180px]:grid-cols-2',
  targetButton: 'grid min-w-0 cursor-pointer gap-1.5 rounded-[14px] border border-white/[0.09] bg-white/[0.035] p-3 text-left text-[var(--text-secondary)] transition-[border-color,background-color] duration-150 hover:border-[var(--accent)] hover:bg-[var(--accent-dim)] disabled:cursor-not-allowed disabled:opacity-45 disabled:hover:border-white/[0.09] disabled:hover:bg-white/[0.035]',
  itemTitle: 'overflow-hidden text-ellipsis whitespace-nowrap text-[12px] font-bold text-[var(--text-primary)]',
  itemMeta: 'overflow-hidden text-ellipsis whitespace-nowrap text-[10px] text-[var(--text-muted)]',
  status: 'justify-self-start rounded-[8px] border px-2 py-1 font-[var(--font-sans)] text-[10px] font-bold',
  formGrid: 'grid grid-cols-2 gap-3 p-4',
  formLabel: 'grid min-w-0 gap-1.5',
  formLabelText: 'text-[11px] font-semibold text-[var(--text-muted)]',
  select: 'min-h-9 rounded-[10px] border border-white/[0.10] bg-[rgba(5,8,18,0.80)] px-3 text-[12px] text-[var(--text-primary)] outline-none focus:border-[var(--accent)] disabled:opacity-45',
  checkbox: 'grid min-h-10 grid-cols-[16px_minmax(0,1fr)] items-center gap-2 rounded-[10px] border border-white/[0.08] bg-white/[0.025] px-3',
  checkboxInput: 'size-3.5 accent-[var(--accent)]',
  output: 'rounded-[16px] border border-white/[0.09] bg-white/[0.035] p-4 shadow-[var(--shadow-sm)]',
  outputPath: 'mt-1.5 block [overflow-wrap:anywhere] font-mono text-[11px] text-[var(--text-primary)]',
  outputNote: 'mt-3 mb-0 text-[11px] leading-[1.6] text-[var(--text-secondary)]',
  outputPre: 'mt-3 max-h-48 overflow-auto whitespace-pre-wrap rounded-[12px] border border-white/[0.09] bg-[#050812] p-3 font-mono text-[10px] leading-relaxed text-[var(--text-secondary)]',
  sidebar: 'min-h-0 overflow-auto rounded-[18px] border border-white/[0.10] bg-[rgba(8,13,24,0.64)] p-4 shadow-[var(--shadow-lg)] backdrop-blur-xl',
  sidebarSection: 'mb-3 rounded-[14px] border border-white/[0.08] bg-white/[0.035] p-3 last:mb-0',
  sidebarList: 'mt-3 grid list-none gap-2 p-0',
  sidebarItem: 'flex items-center gap-2 rounded-[10px] border border-white/[0.06] bg-black/10 px-2.5 py-2 text-[11px] text-[var(--text-muted)]',
  sidebarItemDone: 'border-[rgba(74,222,128,0.18)] bg-[rgba(74,222,128,0.07)] text-[var(--success)]',
  sidebarItemActive: 'border-[rgba(106,215,229,0.20)] bg-[rgba(106,215,229,0.08)] text-[var(--accent)]',
  sidebarItemLocked: 'opacity-55',
  sidebarDl: 'mt-3 grid gap-2',
  sidebarDlRow: 'flex justify-between gap-3 rounded-[10px] border border-white/[0.06] bg-black/10 px-2.5 py-2',
  sidebarDt: 'text-[10px] text-[var(--text-muted)]',
  sidebarDd: 'm-0 overflow-hidden text-ellipsis whitespace-nowrap text-right text-[10px] text-[var(--text-secondary)]',
};

const gameClass = {
  surface: 'relative grid h-full min-h-0 w-full flex-1 gap-0 bg-[var(--bg-base)]',
  surfaceOpen: 'grid-cols-[minmax(196px,226px)_minmax(0,1fr)]',
  surfaceInspectorClosed: 'grid-cols-[minmax(196px,226px)_minmax(0,1fr)]',
  surfaceHierarchyClosed: 'grid-cols-[minmax(0,1fr)]',
  surfaceOnlyMain: 'grid-cols-[minmax(0,1fr)]',
  sidePanel: 'm-2.5 mr-0 flex min-h-0 min-w-0 flex-col overflow-hidden rounded-[14px] border border-white/[0.09] bg-[rgba(10,16,29,0.86)] shadow-[var(--shadow-md)] backdrop-blur-xl',
  inspectorPanel: 'absolute top-3 right-3 bottom-3 z-30 flex w-[min(330px,36vw)] min-h-0 min-w-[280px] flex-col overflow-hidden rounded-[16px] border border-white/[0.12] bg-[rgba(10,16,29,0.94)] shadow-[0_26px_80px_rgba(0,0,0,0.42),inset_0_1px_0_rgba(255,255,255,0.045)] backdrop-blur-2xl',
  panelHeader: 'flex min-h-[48px] items-center justify-between gap-2 border-b border-white/[0.08] bg-[rgba(255,255,255,0.025)] px-3.5 backdrop-blur-xl',
  panelHeaderText: 'text-[11px] font-bold tracking-[0.06em] text-[var(--text-muted)] uppercase',
  panelHeaderTitle: 'mt-1 block text-[13px] font-semibold text-[var(--text-primary)] normal-case',
  panelHeaderActions: 'flex items-center gap-1.5',
  iconButton: 'grid size-8 cursor-pointer place-items-center rounded-[10px] border border-white/[0.10] bg-white/[0.04] text-[var(--text-secondary)] shadow-[var(--shadow-sm)] hover:border-[var(--accent)] hover:bg-[var(--accent-dim)] hover:text-[var(--text-primary)]',
  hierarchyList: 'flex min-h-0 flex-1 flex-col overflow-auto py-2',
  hierarchyItem: 'group/row relative mx-2 flex min-h-[32px] cursor-pointer items-center gap-2 rounded-[10px] border border-transparent pr-2 text-left text-[13px] text-[var(--text-secondary)] transition-colors duration-100 hover:border-white/[0.08] hover:bg-white/[0.045] hover:text-[var(--text-primary)]',
  hierarchyItemSelected: 'border-[rgba(var(--brand-rgb),0.32)] bg-[rgba(var(--brand-rgb),0.20)] text-[var(--text-primary)] shadow-[inset_3px_0_0_var(--brand)]',
  hierarchyTwisty: 'grid size-5 flex-none place-items-center rounded-[6px] text-[var(--text-muted)] hover:bg-[var(--bg-active)] hover:text-[var(--text-primary)] [&_svg]:size-3.5',
  hierarchyTwistySpacer: 'size-5 flex-none',
  hierarchyIcon: 'flex-none text-[var(--text-muted)] group-hover/row:text-[var(--text-secondary)]',
  hierarchyIconSelected: 'flex-none text-[var(--accent-hover)]',
  hierarchyName: 'min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap',
  hierarchyTag: 'flex-none rounded-[6px] bg-white/[0.06] px-1.5 py-px text-[10px] font-medium text-[var(--text-muted)] opacity-0 group-hover/row:opacity-100',
  mainPanel: 'relative m-2.5 flex min-h-0 min-w-0 flex-col overflow-hidden rounded-[18px] border border-white/[0.10] bg-[rgba(7,10,16,0.68)] shadow-[var(--shadow-lg)]',
  previewBar: 'flex min-h-[50px] items-center justify-between border-b border-white/[0.08] bg-[rgba(7,10,16,0.78)] px-4 text-[12px] text-[var(--text-secondary)] backdrop-blur-xl',
  previewBarGroup: 'flex items-center gap-2.5 font-semibold',
  liveDot: 'size-2 rounded-full bg-[var(--success)] shadow-[0_0_12px_var(--success)]',
  modeSwitch: 'inline-flex gap-1 rounded-[12px] border border-white/[0.10] bg-black/20 p-1',
  modeButton: 'flex h-8 cursor-pointer items-center gap-1.5 rounded-[9px] bg-transparent px-3 text-[12px] font-semibold text-[var(--text-muted)] hover:bg-white/[0.06] hover:text-[var(--text-primary)] disabled:cursor-not-allowed disabled:opacity-50',
  modeButtonActive: 'bg-[var(--brand-dim)] text-[var(--text-primary)] shadow-[inset_0_-2px_0_var(--brand)]',
  createPresets: 'flex min-h-[44px] items-center gap-2 overflow-x-auto border-b border-white/[0.08] bg-[rgba(11,16,32,0.72)] px-3 py-1.5',
  createButton: 'inline-flex min-h-8 flex-none cursor-pointer items-center gap-1.5 rounded-[10px] border border-white/[0.10] bg-white/[0.04] px-3 text-[12px] font-semibold text-[var(--text-secondary)] hover:border-[var(--accent)] hover:bg-[var(--accent-dim)] hover:text-[var(--text-primary)]',
  previewCanvas: 'relative flex min-h-0 min-w-0 flex-1 overflow-hidden bg-[#050812]',
  viewportAtmosphere: 'pointer-events-none absolute inset-0 z-10 bg-[radial-gradient(circle_at_48%_42%,rgba(106,215,229,0.10),transparent_24rem),radial-gradient(circle_at_70%_70%,rgba(var(--brand-rgb),0.12),transparent_22rem),linear-gradient(180deg,rgba(255,255,255,0.03),transparent_44%)]',
  viewportHorizon: 'pointer-events-none absolute top-[52%] right-[7%] left-[7%] z-10 h-px bg-[linear-gradient(90deg,transparent,rgba(148,163,184,0.28),transparent)] shadow-[0_0_24px_rgba(106,215,229,0.14)]',
  viewportVignette: 'pointer-events-none absolute inset-0 z-10 shadow-[inset_0_0_90px_rgba(0,0,0,0.50),inset_0_-80px_120px_rgba(0,0,0,0.42)]',
  viewportHud: 'pointer-events-none absolute inset-x-4 top-4 z-30 flex items-start justify-between gap-3',
  viewportHudGroup: 'flex flex-wrap items-center gap-2',
  viewportPill: 'inline-flex min-h-8 items-center gap-2 rounded-[10px] border border-white/[0.10] bg-[rgba(8,13,24,0.72)] px-3 text-[11px] font-semibold text-[var(--text-secondary)] shadow-[0_14px_34px_rgba(0,0,0,0.28)] backdrop-blur-xl',
  viewportPillStrong: 'text-[var(--text-primary)]',
  viewportHint: 'rounded-[10px] border border-white/[0.09] bg-[rgba(8,13,24,0.58)] px-3 py-2 text-[11px] leading-[1.4] text-[var(--text-muted)] shadow-[0_14px_34px_rgba(0,0,0,0.24)] backdrop-blur-xl max-[1180px]:hidden',
  selectionCard: 'pointer-events-auto absolute top-[74px] right-4 z-30 w-[min(320px,calc(100%-32px))] overflow-hidden rounded-[16px] border border-white/[0.12] bg-[rgba(8,13,24,0.82)] shadow-[0_24px_70px_rgba(0,0,0,0.42),inset_0_1px_0_rgba(255,255,255,0.045)] backdrop-blur-2xl',
  selectionCardHeader: 'flex items-start justify-between gap-3 border-b border-white/[0.08] bg-white/[0.025] px-4 py-3',
  selectionCardKicker: 'text-[10px] font-bold tracking-[0.08em] text-[var(--text-muted)] uppercase',
  selectionCardTitle: 'mt-1 block overflow-hidden text-ellipsis whitespace-nowrap text-[14px] font-semibold text-[var(--text-primary)]',
  selectionCardTag: 'rounded-[8px] border border-white/[0.10] bg-white/[0.05] px-2 py-1 text-[10px] font-semibold text-[var(--text-secondary)]',
  selectionMetrics: 'grid grid-cols-[minmax(0,1.6fr)_minmax(54px,0.7fr)_minmax(68px,0.8fr)] gap-1.5 px-3 py-3',
  selectionMetric: 'rounded-[10px] border border-white/[0.08] bg-white/[0.035] px-2.5 py-2',
  selectionMetricLabel: 'block text-[10px] text-[var(--text-muted)]',
  selectionMetricValue: 'mt-1 block min-w-0 overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[11px] text-[var(--text-primary)]',
  selectionMetricPosition: 'text-[10px] tracking-[-0.03em]',
  selectionActions: 'flex flex-wrap gap-2 border-t border-white/[0.08] px-3 py-3',
  selectionActionButton: 'inline-flex min-h-8 flex-1 cursor-pointer items-center justify-center gap-1.5 rounded-[10px] border border-white/[0.10] bg-white/[0.04] px-3 text-[12px] font-semibold text-[var(--text-secondary)] hover:border-[var(--accent)] hover:bg-[var(--accent-dim)] hover:text-[var(--text-primary)] disabled:cursor-not-allowed disabled:opacity-45',
  emptyViewportState: 'pointer-events-auto absolute top-1/2 left-1/2 z-30 w-[min(430px,calc(100%-48px))] -translate-x-1/2 -translate-y-1/2 rounded-[18px] border border-white/[0.12] bg-[rgba(8,13,24,0.76)] p-5 text-center shadow-[0_26px_80px_rgba(0,0,0,0.42),inset_0_1px_0_rgba(255,255,255,0.05)] backdrop-blur-2xl',
  emptyViewportKicker: 'text-[10px] font-bold tracking-[0.10em] text-[var(--accent-hover)] uppercase',
  emptyViewportTitle: 'mt-2 block text-[18px] font-semibold tracking-[-0.02em] text-[var(--text-primary)]',
  emptyViewportText: 'mx-auto mt-2 max-w-[34ch] text-[12px] leading-[1.6] text-[var(--text-secondary)]',
  emptyViewportActions: 'mt-4 flex flex-wrap justify-center gap-2',
  emptyViewportButton: 'inline-flex min-h-9 cursor-pointer items-center gap-1.5 rounded-[10px] border border-white/[0.10] bg-white/[0.045] px-3.5 text-[12px] font-semibold text-[var(--text-secondary)] hover:border-[var(--accent)] hover:bg-[var(--accent-dim)] hover:text-[var(--text-primary)]',
  emptyViewportButtonPrimary: 'border-[var(--brand)] bg-[var(--brand)] text-[var(--text-on-brand)] hover:border-[var(--brand-hover)] hover:bg-[var(--brand-hover)]',
  empty: 'p-4 text-[12px] leading-relaxed text-[var(--text-muted)]',
};

const diagnosticsClass = {
  headerActions: 'flex flex-wrap justify-end gap-2',
  layout: 'grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_330px] gap-4 overflow-hidden p-4 max-[1180px]:grid-cols-1',
  main: 'grid min-h-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden rounded-[18px] border border-white/[0.10] bg-[rgba(8,13,24,0.64)] shadow-[var(--shadow-lg)] backdrop-blur-xl',
  hero: 'grid gap-3 border-b border-white/[0.08] bg-[radial-gradient(circle_at_8%_0%,rgba(var(--brand-rgb),0.12),transparent_22rem),rgba(255,255,255,0.025)] p-4',
  scoreRow: 'flex flex-wrap items-center gap-3',
  scoreDial: 'grid size-16 shrink-0 place-items-center rounded-[18px] border border-[rgba(var(--brand-rgb),0.28)] bg-[rgba(var(--brand-rgb),0.08)] font-mono text-[22px] font-bold text-[var(--text-primary)] shadow-[0_18px_42px_rgba(0,0,0,0.24),inset_0_1px_0_rgba(255,255,255,0.06)]',
  scoreCopy: 'min-w-0 flex-1',
  scoreKicker: 'block text-[10px] font-bold tracking-[0.12em] text-[var(--accent-hover)] uppercase',
  scoreTitle: 'mt-1 block text-[16px] font-semibold tracking-[-0.02em] text-[var(--text-primary)]',
  scoreDesc: 'mt-1 max-w-[70ch] text-[12px] leading-[1.55] text-[var(--text-secondary)]',
  statusMetrics: 'grid grid-cols-4 gap-2 max-[720px]:grid-cols-2',
  statusMetric: 'rounded-[12px] border border-white/[0.08] bg-black/15 px-3 py-2',
  statusMetricLabel: 'block text-[10px] text-[var(--text-muted)]',
  statusMetricValue: 'mt-1 block font-mono text-[15px] font-bold text-[var(--text-primary)]',
  capabilityList: 'min-h-0 overflow-auto p-3 [scrollbar-color:var(--border)_transparent] [scrollbar-width:thin]',
  group: 'mb-3 overflow-hidden rounded-[16px] border border-white/[0.09] bg-white/[0.03] last:mb-0',
  groupHeader: 'flex items-start justify-between gap-3 border-b border-white/[0.07] bg-black/10 px-3 py-3',
  groupTitle: 'text-[13px] font-semibold text-[var(--text-primary)]',
  groupDesc: 'mt-1 text-[11px] leading-[1.5] text-[var(--text-muted)]',
  capabilityGrid: 'grid gap-2 p-2',
  capabilityButton: 'grid w-full cursor-pointer grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3 rounded-[13px] border border-white/[0.07] bg-black/10 px-3 py-3 text-left transition hover:border-white/[0.14] hover:bg-white/[0.045] focus-visible:outline focus-visible:outline-1 focus-visible:outline-[var(--accent)]',
  capabilityButtonActive: 'border-[rgba(var(--brand-rgb),0.36)] bg-[rgba(var(--brand-rgb),0.09)] shadow-[inset_3px_0_0_var(--brand)]',
  statusDot: 'size-2.5 rounded-full bg-[var(--text-muted)] shadow-[0_0_0_3px_rgba(255,255,255,0.04)]',
  statusDotOk: 'bg-[var(--success)] shadow-[0_0_0_3px_rgba(73,217,139,0.12)]',
  statusDotWarning: 'bg-[var(--warning)] shadow-[0_0_0_3px_rgba(247,185,85,0.13)]',
  statusDotError: 'bg-[var(--danger)] shadow-[0_0_0_3px_rgba(255,107,122,0.14)]',
  statusDotMuted: 'bg-[var(--text-muted)]',
  capabilityTitle: 'block overflow-hidden text-ellipsis whitespace-nowrap text-[12px] font-semibold text-[var(--text-primary)]',
  capabilitySummary: 'mt-0.5 line-clamp-2 text-[11px] leading-[1.45] text-[var(--text-secondary)]',
  statusPill: 'rounded-[9px] border px-2 py-1 text-[10px] font-bold',
  statusOk: 'border-[rgba(73,217,139,0.24)] bg-[rgba(73,217,139,0.10)] text-[var(--success)]',
  statusWarning: 'border-[rgba(247,185,85,0.27)] bg-[rgba(247,185,85,0.10)] text-[var(--warning)]',
  statusError: 'border-[rgba(255,107,122,0.28)] bg-[rgba(255,107,122,0.10)] text-[var(--danger)]',
  statusMuted: 'border-white/[0.10] bg-white/[0.045] text-[var(--text-muted)]',
  filterBar: 'flex min-h-[52px] items-center gap-2 overflow-x-auto border-b border-white/[0.08] bg-white/[0.025] px-3 py-2',
  filterButton: 'inline-flex min-h-8 flex-none cursor-pointer items-center gap-1.5 rounded-[10px] border border-white/[0.08] bg-white/[0.035] px-3 text-[12px] font-semibold text-[var(--text-secondary)] hover:border-[var(--accent)] hover:bg-[var(--accent-dim)] hover:text-[var(--text-primary)]',
  filterButtonActive: 'border-[var(--accent)] bg-[var(--accent-dim)] text-[var(--text-primary)] shadow-[inset_0_-2px_0_var(--brand)]',
  list: 'min-h-0 max-h-full overflow-auto p-3',
  entry: 'grid grid-cols-[150px_minmax(0,1fr)_auto] gap-3 rounded-[14px] border border-white/[0.08] bg-white/[0.028] px-3 py-3 text-[11px] text-[var(--text-secondary)] shadow-[var(--shadow-sm)] transition-colors hover:border-white/[0.14] hover:bg-white/[0.04] max-[980px]:grid-cols-1',
  entryError: 'border-[rgba(239,68,68,0.22)] bg-[rgba(239,68,68,0.055)]',
  entryWarn: 'border-[rgba(247,185,85,0.22)] bg-[rgba(247,185,85,0.055)]',
  meta: 'min-w-0',
  level: 'mb-2 inline-flex rounded-[8px] border px-2 py-1 font-[var(--font-sans)] text-[10px] font-bold',
  subsystem: 'block overflow-hidden text-ellipsis whitespace-nowrap text-[11px] font-semibold text-[var(--text-primary)]',
  timestamp: 'mt-1 block font-mono text-[10px] text-[var(--text-muted)]',
  message: 'm-0 min-w-0 whitespace-pre-wrap text-[12px] leading-[1.55] text-[var(--text-secondary)] [overflow-wrap:anywhere]',
  source: 'mt-2 block font-mono text-[10px] text-[var(--text-muted)]',
  entryActions: 'flex items-start justify-end gap-1.5 max-[980px]:justify-start',
  sidebar: 'min-h-0 overflow-auto rounded-[18px] border border-white/[0.10] bg-[rgba(8,13,24,0.64)] p-4 shadow-[var(--shadow-lg)] backdrop-blur-xl [scrollbar-color:var(--border)_transparent] [scrollbar-width:thin]',
  sidebarSection: 'mb-3 rounded-[14px] border border-white/[0.08] bg-white/[0.035] p-3 last:mb-0',
  sidebarTitle: 'text-[12px] font-semibold text-[var(--text-primary)]',
  sidebarText: 'mt-1 text-[11px] leading-[1.55] text-[var(--text-secondary)]',
  detailTitle: 'block text-[15px] font-semibold tracking-[-0.02em] text-[var(--text-primary)]',
  detailSummary: 'mt-2 text-[12px] leading-[1.55] text-[var(--text-secondary)]',
  detailEvidence: 'mt-3 grid gap-1.5',
  evidenceRow: 'rounded-[9px] border border-white/[0.07] bg-black/18 px-2.5 py-2 font-mono text-[10px] leading-[1.45] text-[var(--text-muted)] [overflow-wrap:anywhere]',
  fixList: 'mt-3 grid gap-2',
  fixButton: 'inline-flex min-h-9 cursor-pointer items-center justify-center gap-2 rounded-[10px] border border-[rgba(var(--brand-rgb),0.30)] bg-[rgba(var(--brand-rgb),0.12)] px-3 text-[12px] font-bold text-[var(--accent-hover)] transition hover:border-[var(--accent)] hover:bg-[rgba(var(--brand-rgb),0.18)] disabled:cursor-not-allowed disabled:opacity-50',
  fixDesc: 'mt-1 block text-[10px] font-normal leading-[1.45] text-[var(--text-muted)]',
  logPanel: 'mt-3 overflow-hidden rounded-[14px] border border-white/[0.08] bg-black/15',
  metricGrid: 'mt-3 grid grid-cols-2 gap-2',
  metric: 'rounded-[10px] border border-white/[0.06] bg-black/10 px-2.5 py-2',
  metricLabel: 'block text-[10px] text-[var(--text-muted)]',
  metricValue: 'mt-1 block font-mono text-[15px] font-semibold text-[var(--text-primary)]',
};

const artifactPopoverClass = {
  root: 'fixed z-[90] translate-x-2.5 translate-y-2.5 drop-shadow-[0_10px_24px_rgba(0,0,0,0.42)]',
  button: 'flex h-[30px] cursor-pointer items-center gap-1.5 rounded-md border border-[var(--border-light)] bg-[var(--bg-elevated)] px-2.5 text-[10px] font-semibold text-[var(--text-secondary)] hover:border-[var(--accent)] hover:bg-[var(--bg-hover)]',
  panel: 'w-[310px] overflow-hidden rounded-lg border border-[var(--border-light)] bg-[var(--bg-elevated)]',
  header: 'flex h-8 items-center justify-between gap-2 border-b border-[var(--border)] px-[9px] font-mono text-[10px] text-[var(--text-secondary)]',
  label: 'overflow-hidden text-ellipsis whitespace-nowrap',
  closeButton: 'grid size-[22px] flex-none cursor-pointer place-items-center border-0 bg-transparent text-[var(--text-muted)]',
  form: 'flex gap-1.5 p-2',
  input: 'min-w-0 flex-1 rounded-[5px] border border-[var(--border-light)] bg-[var(--bg-base)] px-2 py-[7px] text-[10px] text-[var(--text-primary)] outline-none focus:border-[var(--accent)]',
  submit: 'cursor-pointer rounded-[5px] border-0 bg-[var(--brand)] px-2.5 text-[10px] font-semibold text-[var(--text-on-brand)] transition-[background] duration-[var(--transition-fast)] hover:not-disabled:bg-[var(--brand-hover)] disabled:cursor-default disabled:opacity-40',
};

const questBannerClass = {
  root: 'flex min-h-[46px] items-center gap-2 border-b border-[var(--border)] bg-[var(--accent-dim)] px-3.5 text-[var(--text-secondary)]',
  error: 'bg-[var(--danger-dim)]',
  icon: 'text-[var(--accent)]',
  errorIcon: 'text-[var(--danger)]',
  content: 'min-w-0 flex-1',
  kicker: 'block text-[10px] font-bold tracking-[0.08em] text-[var(--text-muted)] uppercase',
  title: 'block overflow-hidden text-ellipsis whitespace-nowrap text-xs text-[var(--text-primary)]',
  meta: 'block overflow-hidden text-ellipsis whitespace-nowrap text-[10px] text-[var(--text-muted)]',
  button: 'inline-flex min-h-7 cursor-pointer items-center gap-1.5 rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[10px] text-[var(--text-secondary)] shadow-[var(--shadow-sm)] hover:border-[var(--accent)] hover:text-[var(--text-primary)]',
  iconButton: 'grid size-7 cursor-pointer place-items-center rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-secondary)] shadow-[var(--shadow-sm)] hover:border-[var(--accent)] hover:text-[var(--text-primary)]',
};

const workspaceSelectionClass = {
  card: 'm-[0_8px_10px] flex flex-col gap-1.5 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--bg-elevated)] p-2.5 shadow-[var(--shadow-sm)]',
  title: 'flex items-start justify-between gap-2',
  titleText: 'flex min-w-0 flex-col gap-0.5',
  name: 'overflow-hidden text-ellipsis whitespace-nowrap text-xs',
  tag: 'text-[10px] text-[var(--text-muted)]',
  liveBadge: 'rounded-lg bg-[var(--success-dim)] px-[5px] py-0.5 font-mono text-[10px] font-bold text-[var(--success)] uppercase',
  label: 'text-[10px] font-bold tracking-[0.06em] text-[var(--text-muted)] uppercase',
  positionGrid: 'grid grid-cols-3 gap-1',
  positionInputWrap: 'grid grid-cols-[14px_1fr] items-center overflow-hidden rounded border border-[var(--border)] bg-[var(--bg-base)] focus-within:border-[var(--accent)]',
  positionAxis: 'text-center font-mono text-[10px] text-[var(--text-muted)]',
  positionInput: 'w-full min-w-0 border-0 bg-transparent px-[3px] py-[5px] font-mono text-[10px] text-[var(--text-secondary)] outline-none',
  button: 'cursor-pointer rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--bg-base)] px-2 py-1.5 text-[10px] font-medium text-[var(--text-secondary)] hover:border-[var(--accent)] hover:text-[var(--accent)]',
};

const viewportClass = {
  container: 'relative flex h-full w-full min-h-0 min-w-0 flex-1 overflow-hidden bg-[#0B0F16]',
  canvas: 'block h-full w-full object-fill',
  selectionOverlay: 'pointer-events-none absolute inset-0 z-20 h-full w-full',
};

const inspectorClass = {
  root: 'flex flex-col gap-2.5 p-2.5',
  section: 'rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--bg-elevated)] shadow-[var(--shadow-sm)]',
  sectionTitle: 'border-b border-[var(--border)] px-2.5 py-2 text-[10px] font-bold uppercase tracking-[0.08em] text-[var(--text-muted)]',
  field: 'grid gap-1.5 px-2.5 py-2 text-[11px] text-[var(--text-secondary)] [&>span]:text-[10px] [&>span]:font-semibold [&>span]:uppercase [&>span]:tracking-[0.06em] [&>span]:text-[var(--text-muted)]',
  fieldLabel: 'flex items-center justify-between gap-2',
  fieldDefaultBadge: 'rounded border border-[var(--border)] px-1 py-px font-normal tracking-normal text-[9px] normal-case text-[var(--text-muted)]',
  fieldHint: 'mt-1 text-[10px] leading-[1.45] text-[var(--text-muted)]',
  fieldRow: 'grid-cols-[minmax(0,1fr)_auto] items-center',
  input: 'w-full min-w-0 rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--bg-base)] px-2 py-1.5 font-[var(--font-sans)] text-[11px] text-[var(--text-primary)] outline-none focus:border-[var(--accent)]',
  select: 'w-full min-w-0 appearance-none rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--bg-base)] px-2 py-1.5 pr-[26px] font-[var(--font-sans)] text-[11px] text-[var(--text-primary)] outline-none focus:border-[var(--accent)]',
  assetSelectWrap: 'grid gap-1',
  assetCurrent: 'overflow-hidden text-ellipsis whitespace-nowrap rounded-[var(--radius-sm)] border border-white/[0.06] bg-white/[0.025] px-2 py-1 font-mono text-[10px] text-[var(--text-muted)]',
  assetEmptyHint: 'rounded-[var(--radius-sm)] border border-[var(--warning-dim)] bg-[var(--warning-dim)] px-2 py-1.5 text-[10px] leading-[1.45] text-[var(--warning)]',
  json: 'min-h-20 resize-y font-[var(--font-mono)]',
  colorField: 'grid gap-2 px-2.5 py-2 text-[11px] text-[var(--text-secondary)] [&>span]:text-[10px] [&>span]:font-semibold [&>span]:uppercase [&>span]:tracking-[0.06em] [&>span]:text-[var(--text-muted)]',
  colorCustom: 'flex items-center gap-2',
  colorPicker: 'relative grid size-7 cursor-pointer place-items-center overflow-hidden rounded border border-[var(--border)] [&_input]:absolute [&_input]:inset-0 [&_input]:cursor-pointer [&_input]:opacity-0 [&_span]:size-full',
  colorHex: 'min-w-0 flex-1 rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--bg-base)] px-2 py-1.5 font-[var(--font-mono)] text-[11px] text-[var(--text-primary)] outline-none focus:border-[var(--accent)]',
  colorPresets: 'grid grid-cols-10 gap-1',
  colorPreset: 'size-5 cursor-pointer rounded border border-[var(--border)] hover:border-[var(--accent)]',
  colorPresetActive: 'ring-1 ring-[var(--accent)]',
  colorChannels: 'grid grid-cols-3 gap-1.5',
  channelInput: 'grid grid-cols-[16px_1fr] items-center overflow-hidden rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--bg-base)] focus-within:border-[var(--accent)] [&_span]:text-center [&_span]:font-mono [&_span]:text-[10px] [&_input]:min-w-0 [&_input]:border-0 [&_input]:bg-transparent [&_input]:px-1 [&_input]:py-1.5 [&_input]:font-mono [&_input]:text-[10px] [&_input]:text-[var(--text-primary)] [&_input]:outline-none',
  vec3: 'grid grid-cols-3 gap-1.5',
  vec4: 'grid grid-cols-4 gap-1.5',
  vecInputWrap: 'grid grid-cols-[16px_1fr] items-center overflow-hidden rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--bg-base)] focus-within:border-[var(--accent)]',
  vecLabel: 'text-center font-mono text-[10px] text-[var(--text-muted)]',
  vecInput: 'min-w-0 border-0 bg-transparent px-1 py-1.5 font-mono text-[10px] text-[var(--text-primary)] outline-none',
  actionRow: 'mt-2 flex gap-1.5',
  actionButton: 'inline-flex min-h-7 cursor-pointer items-center gap-1.5 rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--bg-base)] px-2 text-[10px] text-[var(--text-secondary)] hover:border-[var(--accent)] hover:text-[var(--text-primary)] disabled:cursor-default disabled:opacity-40 disabled:hover:border-[var(--border)] disabled:hover:text-[var(--text-secondary)]',
  component: 'border-t border-[var(--border)] first:border-t-0',
  componentHeader: 'flex items-center justify-between gap-2 px-2.5 py-2',
  componentType: 'text-[11px] font-semibold text-[var(--text-primary)]',
  removeButton: 'grid size-6 cursor-pointer place-items-center rounded border border-[var(--border)] bg-transparent text-[var(--text-muted)] hover:border-[var(--danger)] hover:text-[var(--danger)]',
  componentFields: 'border-t border-[var(--border)]',
  emptyField: 'px-2.5 py-2 text-[10px] text-[var(--text-muted)]',
  addRow: 'mt-2 grid grid-cols-[minmax(0,1fr)_68px] gap-1.5',
  addButton: 'inline-flex min-h-8 cursor-pointer items-center justify-center gap-1.5 rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--bg-base)] px-2 text-[10px] text-[var(--text-secondary)] hover:border-[var(--accent)] hover:text-[var(--text-primary)] disabled:cursor-default disabled:opacity-40 disabled:hover:border-[var(--border)] disabled:hover:text-[var(--text-secondary)]',
};

const scriptSurfaceClass = {
  root: 'grid h-full grid-cols-[280px_minmax(0,1fr)] gap-0 bg-[var(--bg-base)] max-[980px]:grid-cols-[220px_minmax(0,1fr)]',
  sidebar: 'min-h-0 overflow-auto border-r border-white/[0.08] bg-[rgba(8,13,24,0.76)]',
  sidebarHeader: 'border-b border-white/[0.08] bg-[rgba(7,10,16,0.74)] px-4 py-4 backdrop-blur-xl',
  sidebarKicker: 'block text-[10px] font-bold tracking-[0.08em] text-[var(--text-muted)] uppercase',
  sidebarTitle: 'mt-1 block text-[16px] font-bold text-[var(--text-primary)]',
  sidebarMeta: 'mt-1 block text-[11px] text-[var(--text-muted)]',
  sidebarEmpty: 'm-3 rounded-[14px] border border-white/[0.10] bg-white/[0.035] p-4 text-[12px] leading-[1.55] text-[var(--text-muted)]',
  scriptButton: 'group grid w-full cursor-pointer grid-cols-[22px_minmax(0,1fr)] gap-x-2 gap-y-0.5 border-0 border-b border-white/[0.07] bg-transparent px-4 py-3 text-left text-[var(--text-muted)] transition-colors duration-150 hover:bg-white/[0.04] hover:text-[var(--text-primary)] [&_svg]:row-span-2 [&_svg]:mt-0.5 [&_svg]:text-[var(--accent-hover)]',
  scriptButtonActive: 'bg-[var(--accent-dim)] text-[var(--text-primary)] shadow-[inset_3px_0_0_var(--brand)]',
  scriptName: 'overflow-hidden text-ellipsis whitespace-nowrap text-[13px] font-semibold text-[var(--text-primary)]',
  scriptPath: 'overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[10px] text-[var(--text-muted)]',
  editor: 'flex min-w-0 flex-col bg-[linear-gradient(180deg,rgba(255,255,255,0.025),transparent_22%),#050812]',
  editorHeader: 'flex min-h-[72px] items-center justify-between gap-4 border-b border-white/[0.08] bg-[rgba(7,10,16,0.78)] px-5 backdrop-blur-2xl',
  editorTitle: 'min-w-0',
  editorKicker: 'block text-[10px] font-bold tracking-[0.08em] text-[var(--text-muted)] uppercase',
  editorFile: 'mt-1 block overflow-hidden text-ellipsis whitespace-nowrap text-[16px] font-semibold text-[var(--text-primary)]',
  editorMeta: 'mt-1 block overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[10px] text-[var(--text-muted)]',
  editorActions: 'flex shrink-0 items-center gap-2',
  editorHint: 'font-[var(--font-sans)] text-[10px] font-bold tracking-[0.08em] text-[var(--text-muted)]',
  editorButton: 'inline-flex min-h-9 cursor-pointer items-center gap-1.5 rounded-[10px] border border-white/[0.10] bg-white/[0.045] px-3 text-[12px] font-semibold text-[var(--text-secondary)] shadow-[var(--shadow-sm)] hover:not-disabled:border-[var(--accent)] hover:not-disabled:bg-[var(--accent-dim)] hover:not-disabled:text-[var(--text-primary)] disabled:cursor-not-allowed disabled:opacity-50',
  statusBadge: 'inline-flex min-h-8 items-center rounded-[10px] border border-white/[0.08] bg-white/[0.035] px-2.5 text-[11px] font-semibold text-[var(--text-secondary)]',
  statusBadgeDirty: 'border-[rgba(247,185,85,0.28)] bg-[var(--warning-dim)] text-[var(--warning)]',
  statusBadgeError: 'border-[rgba(239,68,68,0.28)] bg-[var(--danger-dim)] text-[var(--danger)]',
  editorPane: 'grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_300px] max-[1180px]:grid-cols-[minmax(0,1fr)_240px]',
  textarea: 'h-full min-h-0 w-full min-w-0 resize-none overflow-auto whitespace-pre border-0 border-r border-white/[0.08] bg-[#050812] px-6 py-5 font-mono text-[12px] leading-[1.72] text-[var(--text-primary)] outline-none [tab-size:2] focus:shadow-[inset_0_0_0_1px_var(--accent)] disabled:text-[var(--text-muted)]',
  gutter: 'm-0 flex-1 overflow-auto border-l border-white/[0.03] bg-[rgba(8,13,24,0.82)] px-4 py-5 font-mono text-[11px] leading-[1.72] text-[var(--text-secondary)] [tab-size:2] [&_code]:block [&_code]:min-w-max',
  gutterHeader: 'border-b border-white/[0.08] bg-[rgba(7,10,16,0.70)] px-4 py-3',
  gutterTitle: 'block text-[11px] font-semibold text-[var(--text-primary)]',
  gutterHint: 'mt-1 block text-[10px] leading-[1.45] text-[var(--text-muted)]',
  gutterButton: 'grid w-full cursor-text grid-cols-[42px_minmax(max-content,1fr)] border-0 bg-transparent p-0 text-left font-inherit text-inherit whitespace-pre hover:bg-[var(--accent-dim)]',
  gutterButtonSelected: 'bg-[var(--accent-dim)]',
  gutterLineNumber: 'select-none text-[#4b5563]',
  gutterLineText: 'pr-6 not-italic text-[var(--text-secondary)]',
  diagnostics: 'max-h-32 overflow-auto border-t border-[var(--border)] bg-[var(--bg-overlay)] px-3 py-2 font-mono text-[10px]',
  diagnostic: 'mb-1.5 grid gap-0.5 last:mb-0',
  diagnosticMessage: 'text-[var(--danger)]',
  diagnosticSuggestion: 'text-[var(--text-secondary)]',
};

function gameSurfaceClass(hierarchyOpen: boolean, inspectorOpen: boolean): string {
  return cx(
    gameClass.surface,
    hierarchyOpen && inspectorOpen && gameClass.surfaceOpen,
    hierarchyOpen && !inspectorOpen && gameClass.surfaceInspectorClosed,
    !hierarchyOpen && inspectorOpen && gameClass.surfaceHierarchyClosed,
    !hierarchyOpen && !inspectorOpen && gameClass.surfaceOnlyMain,
  );
}

function buildStatusClass(status: BuildTargetOption['status']): string {
  return cx(
    buildClass.status,
    status === 'ready' && 'bg-[var(--success-dim)] text-[var(--success)]',
    status === 'planned' && 'bg-[var(--accent-dim)] text-[var(--accent)]',
    status === 'blocked' && 'bg-[var(--warning-dim)] text-[var(--warning)]',
  );
}

function diagnosticLevelClass(level: string): string {
  if (level === 'error') return 'text-[var(--danger)]';
  if (level === 'warn' || level === 'warning') return 'text-[var(--warning)]';
  if (level === 'info') return 'text-[var(--accent)]';
  return '';
}

interface QuestArtifactContext {
  surface: WorkspaceView;
  title: string;
  description: string;
  focusPath?: string;
}

function formatInspectorValue(value: unknown): string {
  if (typeof value === 'string') return value;
  if (typeof value === 'number' || typeof value === 'boolean') return String(value);
  if (Array.isArray(value)) return value.map(item => formatInspectorValue(item)).join(', ');
  if (value === null || value === undefined) return '';
  return JSON.stringify(value, null, 2);
}

function parseInspectorValue(raw: string, current: unknown): unknown {
  if (typeof current === 'number') {
    const next = Number(raw);
    return Number.isFinite(next) ? next : current;
  }
  if (typeof current === 'boolean') {
    return raw === 'true';
  }
  if (Array.isArray(current)) {
    const parts = raw.split(',').map(part => part.trim());
    if (current.every(item => typeof item === 'number')) {
      const parsed = parts.map(Number);
      return parsed.length === current.length && parsed.every(Number.isFinite) ? parsed : current;
    }
    return parts;
  }
  if (current && typeof current === 'object') {
    try {
      return JSON.parse(raw);
    } catch {
      return current;
    }
  }
  return raw;
}

function parseSchemaDefaultValue(field: ComponentFieldSchema): unknown {
  const raw = field.default_value;
  switch (field.kind) {
    case 'Bool':
      return raw === 'true';
    case 'F32': {
      const parsed = Number(raw);
      return Number.isFinite(parsed) ? parsed : 0;
    }
    case 'Vec3': {
      const [x = 0, y = 0, z = 0] = raw.split(',').map(part => Number(part.trim()));
      return {
        x: Number.isFinite(x) ? x : 0,
        y: Number.isFinite(y) ? y : 0,
        z: Number.isFinite(z) ? z : 0,
      };
    }
    case 'Object':
      if (!raw) return {};
      try {
        return JSON.parse(raw);
      } catch {
        return raw;
      }
    case 'AssetRef':
    case 'String':
    default:
      return raw;
  }
}

function isVec3Record(value: unknown): value is { x: number; y: number; z: number } {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const record = value as Record<string, unknown>;
  return typeof record.x === 'number' && typeof record.y === 'number' && typeof record.z === 'number';
}

function componentFieldOptions(componentType: string, fieldName: string): string[] | null {
  if (componentType === 'Light' && fieldName === 'kind') return ['directional', 'point', 'spot'];
  if (componentType === 'Rigidbody' && fieldName === 'body_type') return ['dynamic', 'kinematic', 'static'];
  if (componentType === 'Collider' && fieldName === 'shape') return ['box', 'sphere', 'capsule'];
  if (componentType === 'AudioSource' && fieldName === 'spatial_mode') return ['direct', 'spatial'];
  if (componentType === 'AudioSource' && fieldName === 'shape') return ['point', 'cone'];
  if (componentType === 'AudioSource' && fieldName === 'attenuation') return ['none', 'linear', 'inverse'];
  if (componentType === 'AudioListener' && fieldName === 'output_mode') return ['stereo', 'surround'];
  if (componentType === 'AudioListener' && fieldName === 'hrtf_quality') return ['low', 'medium', 'high'];
  return null;
}

function normalizeAssetRefValue(value: unknown): string {
  if (value === null || value === undefined) return '';
  if (typeof value === 'string') return value;
  if (typeof value === 'number' && Number.isFinite(value)) return value.toString(16).padStart(32, '0');
  return '';
}

const BUILTIN_MATERIAL_OPTIONS = [
  { value: 'debug/default', label: '内置默认材质' },
];

function isMaterialRefField(componentType: string, fieldName: string, schema?: ComponentFieldSchema, value?: unknown): boolean {
  if (fieldName !== 'material') return false;
  if (componentType !== 'MeshRenderer' && componentType !== 'SkinnedMeshRenderer') return false;
  if (schema?.kind === 'Object') return true;
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const record = value as Record<string, unknown>;
  return Object.prototype.hasOwnProperty.call(record, 'asset') || Object.prototype.hasOwnProperty.call(record, 'builtin');
}

function normalizeMaterialRefSelection(value: unknown): string {
  if (value && typeof value === 'object' && !Array.isArray(value)) {
    const record = value as Record<string, unknown>;
    const asset = normalizeAssetRefValue(record.asset);
    if (asset) return `asset:${asset}`;
    const builtin = typeof record.builtin === 'string' ? record.builtin.trim() : '';
    if (builtin) return `builtin:${builtin}`;
  }
  if (typeof value === 'string' && value.trim()) return `builtin:${value.trim()}`;
  return 'none';
}

function materialRefFromSelection(selection: string): { asset: string | null; builtin: string | null } {
  if (selection.startsWith('asset:')) {
    return { asset: selection.slice('asset:'.length), builtin: null };
  }
  if (selection.startsWith('builtin:')) {
    return { asset: null, builtin: selection.slice('builtin:'.length) || null };
  }
  return { asset: null, builtin: null };
}

function isMaterialAsset(asset: ProjectAssetMeta): boolean {
  const descriptor = `${asset.kind} ${asset.source_path} ${asset.importer}`.toLowerCase();
  return /material|\\.mat\\b|\\.material\\.json$/.test(descriptor);
}

function assetMatchesInspectorField(asset: ProjectAssetMeta, componentType: string, fieldName: string): boolean {
  const descriptor = `${asset.kind} ${asset.source_path} ${asset.importer}`.toLowerCase();
  if ((componentType === 'MeshRenderer' || componentType === 'SkinnedMeshRenderer') && fieldName === 'mesh') {
    return /model|mesh|gltf|glb|obj|fbx|amdl|skinned/.test(descriptor);
  }
  if (
    (componentType === 'AudioSource' || componentType === 'AudioStreamPlayer2D' || componentType === 'AudioStreamPlayer3D')
    && fieldName === 'clip'
  ) {
    return /audio|sound|music|wav|mp3|ogg|flac/.test(descriptor);
  }
  if (componentType === 'AnimationPlayer' && fieldName === 'clip') {
    return /animation|anim|clip|gltf|glb/.test(descriptor);
  }
  if (componentType === 'Skybox' && fieldName === 'cubemap') {
    return /texture|image|cubemap|hdr|exr|png|jpg|jpeg|webp/.test(descriptor);
  }
  if ((componentType === 'Sprite2D' && fieldName === 'texture') || (componentType === 'TileMap' && fieldName === 'tileset')) {
    return /texture|image|sprite|tileset|tile|png|jpg|jpeg|webp/.test(descriptor);
  }
  return !asset.guid.startsWith('script:');
}

const COMPONENT_DISPLAY_LABELS: Record<string, string> = {
  Camera: '摄像机',
  Light: '灯光',
  MeshRenderer: '网格渲染器',
  Rigidbody: '刚体',
  Collider: '碰撞体',
  AudioSource: '音频源',
  AudioListener: '音频监听器',
  AcousticMaterial: '声学材质',
  AcousticGeometry: '声学几何',
  AcousticRoom: '声学房间',
  AcousticPortal: '声学门户',
  AudioZone: '音频区域',
  ParticleEmitter: '粒子发射器',
  Skybox: '天空盒',
  Sprite2D: '2D 精灵',
  TileMap: '瓦片地图',
  Camera2D: '2D 摄像机',
  Light2D: '2D 灯光',
  Occluder2D: '2D 遮挡体',
  AnimationPlayer: '动画播放器',
  SkinnedMeshRenderer: '蒙皮网格渲染器',
  AudioStreamPlayer2D: '2D 音频播放器',
  AudioStreamPlayer3D: '3D 音频播放器',
  Script: '脚本',
};

const COMPONENT_FIELD_LABELS: Record<string, string> = {
  vertical_fov_degrees: '垂直视角',
  near: '近裁剪',
  far: '远裁剪',
  primary: '主摄像机',
  clear_color: '背景色',
  mesh: '网格',
  builtin_mesh: '内置网格',
  material: '材质',
  casts_shadows: '投射阴影',
  receive_shadows: '接收阴影',
  kind: '类型',
  color: '颜色',
  intensity: '强度',
  range: '范围',
  spot_angle: '聚光角',
  body_type: '刚体类型',
  mass: '质量',
  use_gravity: '使用重力',
  linear_damping: '线性阻尼',
  angular_damping: '角阻尼',
  shape: '形状',
  size: '尺寸',
  is_trigger: '触发器',
  mask: '碰撞遮罩',
  physics_material: '物理材质',
  clip: '音频片段',
  volume: '音量',
  looping: '循环',
  play_on_start: '开始时播放',
  spatial_blend: '空间混合',
  spatial_mode: '空间模式',
  inner_angle_degrees: '内角',
  outer_angle_degrees: '外角',
  outer_gain: '外圈增益',
  sphere_radius: '球半径',
  attenuation: '衰减',
  min_distance: '最小距离',
  max_distance: '最大距离',
  doppler_scale: '多普勒',
  spread: '扩散',
  category: '类别',
  critical: '关键音效',
  use_hrtf: '启用 HRTF',
  output_mode: '输出模式',
  hrtf_quality: 'HRTF 质量',
  hrtf_enabled: '启用 HRTF',
  absorption: '吸收',
  transmission: '透射',
  scattering: '散射',
  blocks_direct_path: '阻挡直达声',
  reverb_send: '混响发送',
  openness: '开放度',
  direct_gain: '直达增益',
  max_particles: '最大粒子数',
  emission_rate: '发射率',
  lifetime: '生命周期',
  start_speed: '初速度',
  start_size: '起始大小',
  end_size: '结束大小',
  start_color: '起始颜色',
  end_color: '结束颜色',
  gravity: '重力',
  spread_degrees: '扩散角',
  backend: '脚本后端',
  script: '脚本文件',
  pending_recovery: '待恢复',
  cubemap: '天空盒贴图',
  zenith_color: '顶部颜色',
  horizon_color: '地平线颜色',
  rotation_degrees: '旋转角度',
  texture: '贴图',
  tileset: '瓦片集',
  tile_size: '瓦片尺寸',
  map_size: '地图尺寸',
  order_in_layer: '层内顺序',
  layer: '图层',
  centered: '居中',
  flip_h: '水平翻转',
  flip_v: '垂直翻转',
  auto_play: '自动播放',
  speed: '速度',
  skeleton_root: '骨骼根节点',
  bus: '音频总线',
};

const COMPONENT_OPTION_LABELS: Record<string, Record<string, string>> = {
  kind: {
    directional: '方向光',
    point: '点光源',
    spot: '聚光灯',
  },
  body_type: {
    dynamic: '动态',
    kinematic: '运动学',
    static: '静态',
  },
  shape: {
    box: '盒体',
    sphere: '球体',
    capsule: '胶囊体',
    point: '点',
    cone: '锥形',
  },
  spatial_mode: {
    direct: '直达声',
    spatial: '空间声',
  },
  attenuation: {
    none: '无',
    linear: '线性',
    inverse: '反比',
  },
  output_mode: {
    stereo: '立体声',
    surround: '环绕声',
  },
  hrtf_quality: {
    low: '低',
    medium: '中',
    high: '高',
  },
};

function componentDisplayLabel(typeId: string, schema?: ComponentSchema): string {
  return COMPONENT_DISPLAY_LABELS[typeId] ?? schema?.display_name ?? typeId;
}

function componentOptionLabel(fieldName: string, value: string): string {
  return COMPONENT_OPTION_LABELS[fieldName]?.[value] ?? value;
}

function formatFieldLabel(fieldName: string): string {
  return fieldName.replaceAll('_', ' ');
}

function numericWheelDelta(event: React.WheelEvent<HTMLInputElement>, baseStep = 0.1): number {
  const multiplier = event.shiftKey ? 10 : event.altKey ? 0.1 : 1;
  return (event.deltaY < 0 ? baseStep : -baseStep) * multiplier;
}

function nudgeNumericInput(
  input: HTMLInputElement,
  delta: number,
  options: { min?: number; max?: number; precision?: number } = {},
): number | null {
  const current = Number(input.value);
  if (!Number.isFinite(current)) return null;
  const precision = options.precision ?? 2;
  const factor = 10 ** precision;
  let next = Math.round((current + delta) * factor) / factor;
  if (options.min !== undefined) next = Math.max(options.min, next);
  if (options.max !== undefined) next = Math.min(options.max, next);
  input.value = next.toFixed(precision);
  return next;
}

function componentFieldLabel(componentType: string, fieldName: string): string {
  if (componentType === 'AnimationPlayer' && fieldName === 'clip') return '动画片段';
  if ((componentType === 'AudioSource' || componentType === 'AudioStreamPlayer2D' || componentType === 'AudioStreamPlayer3D') && fieldName === 'clip') return '音频片段';
  if (COMPONENT_FIELD_LABELS[fieldName]) return COMPONENT_FIELD_LABELS[fieldName];
  if (componentType === 'Camera' && fieldName === 'clear_color') return '背景色';
  if (componentType === 'Light' && fieldName === 'color') return '灯光颜色';
  return formatFieldLabel(fieldName);
}

function orderedComponentFields(
  component: EntityDetails['components'][number],
  schema?: ComponentSchema,
): Array<{ fieldName: string; value: unknown; schema?: ComponentFieldSchema; isDefaultOnly: boolean }> {
  const data = component.data ?? {};
  const rows: Array<{ fieldName: string; value: unknown; schema?: ComponentFieldSchema; isDefaultOnly: boolean }> = [];
  const seen = new Set<string>();

  for (const field of schema?.fields ?? []) {
    seen.add(field.name);
    const hasValue = Object.prototype.hasOwnProperty.call(data, field.name);
    rows.push({
      fieldName: field.name,
      value: hasValue ? data[field.name] : parseSchemaDefaultValue(field),
      schema: field,
      isDefaultOnly: !hasValue,
    });
  }

  for (const [fieldName, value] of Object.entries(data)) {
    if (seen.has(fieldName)) continue;
    rows.push({ fieldName, value, isDefaultOnly: false });
  }

  return rows;
}

function formatSceneNumber(value: number): string {
  if (!Number.isFinite(value)) return '0.00';
  return value.toFixed(2);
}

function formatScenePosition(position: [number, number, number]): string {
  return position.map(formatSceneNumber).join(' / ');
}

function usesColorPicker(componentType: string, fieldName: string): boolean {
  return fieldName.toLowerCase().includes('color') || (componentType === 'Light' && fieldName === 'color');
}

function vec3ToHex(value: { x: number; y: number; z: number }): string {
  return `#${[value.x, value.y, value.z]
    .map(channel => Math.round(Math.max(0, Math.min(1, channel)) * 255).toString(16).padStart(2, '0'))
    .join('')}`;
}

function hexToVec3(hex: string): { x: number; y: number; z: number } | null {
  const match = /^#?([0-9a-f]{6})$/i.exec(hex);
  if (!match) return null;
  const value = match[1];
  return {
    x: parseInt(value.slice(0, 2), 16) / 255,
    y: parseInt(value.slice(2, 4), 16) / 255,
    z: parseInt(value.slice(4, 6), 16) / 255,
  };
}

const COLOR_PRESETS = [
  '#ffffff',
  '#f8fafc',
  '#fef3c7',
  '#fed7aa',
  '#fecaca',
  '#fbcfe8',
  '#ddd6fe',
  '#d4d4d8',
  '#a7f3d0',
  '#111827',
];

const COMPONENT_PICK_ORDER = [
  'Camera',
  'Light',
  'MeshRenderer',
  'Rigidbody',
  'Collider',
  'Script',
  'AudioSource',
  'AudioListener',
  'Skybox',
  'Sprite2D',
  'TileMap',
  'Camera2D',
  'Light2D',
  'AnimationPlayer',
  'SkinnedMeshRenderer',
  'AudioStreamPlayer2D',
  'AudioStreamPlayer3D',
  'ParticleEmitter',
];

const TRANSFORM_FIELD_LABELS: Record<'position' | 'rotation' | 'scale', string> = {
  position: '位置',
  rotation: '旋转',
  scale: '缩放',
};

function ComponentFieldEditor({ componentType, fieldName, value, schema, isDefaultOnly, scriptOptions, assetOptions, onCommit }: {
  componentType: string;
  fieldName: string;
  value: unknown;
  schema?: ComponentFieldSchema;
  isDefaultOnly?: boolean;
  scriptOptions?: string[];
  assetOptions?: ProjectAssetMeta[];
  onCommit: (fieldName: string, value: unknown) => Promise<void>;
}) {
  const [draft, setDraft] = useState(() => formatInspectorValue(value));
  const [hexDraft, setHexDraft] = useState(() => isVec3Record(value) ? vec3ToHex(value) : '');
  const lastCommittedColorHexRef = useRef<string | null>(null);

  useEffect(() => {
    setDraft(formatInspectorValue(value));
    setHexDraft(isVec3Record(value) ? vec3ToHex(value) : '');
    lastCommittedColorHexRef.current = isVec3Record(value) ? vec3ToHex(value) : null;
  }, [value]);

  const commit = useCallback(async () => {
    const next = parseInspectorValue(draft, value);
    if (JSON.stringify(next) === JSON.stringify(value)) {
      setDraft(formatInspectorValue(value));
      return;
    }
    await onCommit(fieldName, next);
  }, [draft, fieldName, onCommit, value]);

  const label = componentFieldLabel(componentType, schema?.name ?? fieldName);
  const labelNode = (
    <span className={inspectorClass.fieldLabel}>
      <span>{label}</span>
      {isDefaultOnly && <small className={inspectorClass.fieldDefaultBadge}>默认值</small>}
    </span>
  );
  const options = (schema?.kind === 'String' || typeof value === 'string')
    ? componentFieldOptions(componentType, fieldName)
    : null;

  if (isMaterialRefField(componentType, fieldName, schema, value)) {
    const currentValue = normalizeMaterialRefSelection(value);
    const materialAssets = (assetOptions ?? [])
      .filter(asset => asset.guid && !asset.guid.startsWith('script:'))
      .filter(isMaterialAsset)
      .sort((left, right) => left.source_path.localeCompare(right.source_path));
    const currentAssetGuid = currentValue.startsWith('asset:') ? currentValue.slice('asset:'.length) : '';
    const currentAsset = materialAssets.find(asset => asset.guid === currentAssetGuid);
    const hasCurrentMissingAsset = Boolean(currentAssetGuid && !currentAsset);
    return (
      <label className={inspectorClass.field}>
        {labelNode}
        <div className={inspectorClass.assetSelectWrap}>
          <select
            className={inspectorClass.select}
            aria-label={label}
            value={currentValue}
            onChange={event => onCommit(fieldName, materialRefFromSelection(event.currentTarget.value))}
          >
            <option value="none">不绑定材质</option>
            {BUILTIN_MATERIAL_OPTIONS.map(option => (
              <option key={option.value} value={`builtin:${option.value}`}>{option.label}</option>
            ))}
            {hasCurrentMissingAsset && <option value={`asset:${currentAssetGuid}`}>{currentAssetGuid}（当前）</option>}
            {materialAssets.map(asset => (
              <option key={asset.guid} value={`asset:${asset.guid}`}>
                {asset.source_path.split('/').pop() || asset.source_path} · {assetKindLabel(asset.kind)}
              </option>
            ))}
          </select>
          {currentAsset && (
            <small className={inspectorClass.assetCurrent}>
              {currentAsset.source_path} · {currentAsset.guid}
            </small>
          )}
          {!currentAsset && currentAssetGuid && (
            <small className={inspectorClass.assetCurrent}>
              当前引用：{currentAssetGuid}
            </small>
          )}
          {currentValue.startsWith('builtin:') && (
            <small className={inspectorClass.assetCurrent}>
              当前使用：{BUILTIN_MATERIAL_OPTIONS.find(option => currentValue === `builtin:${option.value}`)?.label ?? currentValue.slice('builtin:'.length)}
            </small>
          )}
          {materialAssets.length === 0 && (
            <small className={inspectorClass.assetEmptyHint}>
              资源列表里还没有项目材质。可以继续使用内置默认材质，也可以先在资源面板创建材质。
            </small>
          )}
        </div>
      </label>
    );
  }

  if (schema?.kind === 'AssetRef') {
    const currentValue = normalizeAssetRefValue(value);
    const eligibleAssets = (assetOptions ?? [])
      .filter(asset => asset.guid && !asset.guid.startsWith('script:'))
      .filter(asset => assetMatchesInspectorField(asset, componentType, fieldName))
      .sort((left, right) => assetKindLabel(left.kind).localeCompare(assetKindLabel(right.kind)) || left.source_path.localeCompare(right.source_path));
    const currentAsset = eligibleAssets.find(asset => asset.guid === currentValue);
    const hasCurrentMissingAsset = Boolean(currentValue && !currentAsset);
    return (
      <label className={inspectorClass.field}>
        {labelNode}
        <div className={inspectorClass.assetSelectWrap}>
          <select
            className={inspectorClass.select}
            aria-label={label}
            value={currentValue}
            onChange={event => onCommit(fieldName, event.currentTarget.value || null)}
          >
            <option value="">不绑定资源</option>
            {hasCurrentMissingAsset && <option value={currentValue}>{currentValue}（当前）</option>}
            {eligibleAssets.map(asset => (
              <option key={asset.guid} value={asset.guid}>
                {asset.source_path.split('/').pop() || asset.source_path} · {assetKindLabel(asset.kind)}
              </option>
            ))}
          </select>
          {currentAsset && (
            <small className={inspectorClass.assetCurrent}>
              {currentAsset.source_path} · {currentAsset.guid}
            </small>
          )}
          {!currentAsset && currentValue && (
            <small className={inspectorClass.assetCurrent}>
              当前引用：{currentValue}
            </small>
          )}
          {eligibleAssets.length === 0 && (
            <small className={inspectorClass.assetEmptyHint}>
              资源列表里还没有匹配的{componentFieldLabel(componentType, fieldName)}资源。先在资源面板导入或创建资源，再回到这里选择。
            </small>
          )}
        </div>
      </label>
    );
  }

  if (componentType === 'Script' && fieldName === 'script' && scriptOptions && scriptOptions.length > 0) {
    const currentValue = typeof value === 'string' ? value : '';
    const hasCurrentValue = currentValue && !scriptOptions.includes(currentValue);
    return (
      <label className={inspectorClass.field}>
        {labelNode}
        <select
          className={inspectorClass.select}
          aria-label={label}
          value={hasCurrentValue ? currentValue : currentValue}
          onChange={event => onCommit(fieldName, event.currentTarget.value)}
        >
          <option value="">不绑定脚本</option>
          {hasCurrentValue && <option value={currentValue}>{currentValue}（当前）</option>}
          {scriptOptions.map(path => (
            <option key={path} value={path}>{path}</option>
          ))}
        </select>
        <small className={inspectorClass.fieldHint}>来自项目脚本列表，选择后会直接写入当前对象的 Script 组件。</small>
      </label>
    );
  }

  if (options) {
    return (
      <label className={inspectorClass.field}>
        {labelNode}
        <select
          className={inspectorClass.select}
          aria-label={label}
          value={typeof value === 'string' ? value : ''}
          onChange={event => onCommit(fieldName, event.currentTarget.value)}
        >
          {options.map(option => (
            <option key={option} value={option}>{componentOptionLabel(fieldName, option)}</option>
          ))}
        </select>
      </label>
    );
  }

  if (typeof value === 'boolean') {
    return (
      <label className={cx(inspectorClass.field, inspectorClass.fieldRow)}>
        {labelNode}
        <input
          type="checkbox"
          aria-label={label}
          checked={value}
          onChange={event => onCommit(fieldName, event.currentTarget.checked)}
        />
      </label>
    );
  }

  if (isVec3Record(value)) {
    const commitAxis = (axis: 'x' | 'y' | 'z', raw: string) => {
      const nextAxisValue = Number(raw);
      if (!Number.isFinite(nextAxisValue) || nextAxisValue === value[axis]) return;
      onCommit(fieldName, { ...value, [axis]: nextAxisValue });
    };
    const wheelAxis = (
      event: React.WheelEvent<HTMLInputElement>,
      axis: 'x' | 'y' | 'z',
      options?: { min?: number; max?: number; precision?: number },
    ) => {
      event.preventDefault();
      const nextAxisValue = nudgeNumericInput(
        event.currentTarget,
        numericWheelDelta(event, options?.precision === 2 && options?.min === 0 && options?.max === 1 ? 0.01 : 0.1),
        options,
      );
      if (nextAxisValue === null || nextAxisValue === value[axis]) return;
      onCommit(fieldName, { ...value, [axis]: nextAxisValue });
    };
    const isColor = usesColorPicker(componentType, fieldName);
    const hex = vec3ToHex(value);

    if (isColor) {
      const commitColor = (hexValue: string) => {
        const next = hexToVec3(hexValue);
        if (!next) {
          setHexDraft(hex);
          return;
        }
        const normalizedHex = vec3ToHex(next);
        setHexDraft(normalizedHex);
        if (normalizedHex === lastCommittedColorHexRef.current) return;
        lastCommittedColorHexRef.current = normalizedHex;
        onCommit(fieldName, next);
      };

      return (
        <div className={inspectorClass.colorField}>
          {labelNode}
          <div className={inspectorClass.colorCustom}>
            <label className={inspectorClass.colorPicker} title="打开颜色面板">
              <input type="color" value={hex} onChange={event => commitColor(event.currentTarget.value)} />
              <span style={{ backgroundColor: hex }} />
            </label>
            <input
              className={inspectorClass.colorHex}
              aria-label={`${label} Hex`}
              value={hexDraft}
              spellCheck={false}
              onChange={event => {
                const nextHex = event.currentTarget.value;
                setHexDraft(nextHex);
                if (/^#?[0-9a-f]{6}$/i.test(nextHex)) {
                  commitColor(nextHex);
                }
              }}
              onBlur={event => commitColor(event.currentTarget.value)}
              onKeyDown={event => {
                if (event.key === 'Enter') {
                  commitColor(event.currentTarget.value);
                  event.currentTarget.blur();
                }
                if (event.key === 'Escape') {
                  setHexDraft(hex);
                  event.currentTarget.blur();
                }
              }}
            />
          </div>
          <div className={inspectorClass.colorPresets} aria-label={`${label} presets`}>
            {COLOR_PRESETS.map(preset => (
              <button
                key={preset}
                type="button"
                className={cx(inspectorClass.colorPreset, preset === hex && inspectorClass.colorPresetActive)}
                style={{ backgroundColor: preset }}
                title={preset}
                onClick={() => commitColor(preset)}
              />
            ))}
          </div>
          <div className={inspectorClass.colorChannels}>
            {(['x', 'y', 'z'] as const).map((axis, index) => (
              <label className={inspectorClass.channelInput} key={`${fieldName}-${axis}`}>
                <span>{['R', 'G', 'B'][index]}</span>
                <input
                  aria-label={`${label} ${['R', 'G', 'B'][index]}`}
                  defaultValue={value[axis].toFixed(2)}
                  inputMode="decimal"
                  onBlur={event => commitAxis(axis, event.currentTarget.value)}
                  onWheel={event => wheelAxis(event, axis, { min: 0, max: 1, precision: 2 })}
                  onKeyDown={event => {
                    if (event.key === 'Enter') event.currentTarget.blur();
                    if (event.key === 'Escape') event.currentTarget.blur();
                  }}
                />
              </label>
            ))}
          </div>
        </div>
      );
    }

    return (
      <div className={inspectorClass.field}>
        {labelNode}
        <div className={inspectorClass.vec3}>
          {(['x', 'y', 'z'] as const).map(axis => (
            <label className={inspectorClass.vecInputWrap} key={`${fieldName}-${axis}`}>
              <span className={inspectorClass.vecLabel}>{axis.toUpperCase()}</span>
              <input
                className={inspectorClass.vecInput}
                aria-label={`${label} ${axis.toUpperCase()}`}
                defaultValue={value[axis].toFixed(2)}
                inputMode="decimal"
                onBlur={event => commitAxis(axis, event.currentTarget.value)}
                onWheel={event => wheelAxis(event, axis)}
                onKeyDown={event => {
                  if (event.key === 'Enter') event.currentTarget.blur();
                  if (event.key === 'Escape') event.currentTarget.blur();
                }}
              />
            </label>
          ))}
        </div>
      </div>
    );
  }

  const isObject = Boolean(value && typeof value === 'object' && !Array.isArray(value));
  const wheelNumberField = async (event: React.WheelEvent<HTMLInputElement>) => {
    if (typeof value !== 'number') return;
    event.preventDefault();
    const current = Number(draft);
    if (!Number.isFinite(current)) return;
    const next = Math.round((current + numericWheelDelta(event)) * 100) / 100;
    const formatted = next.toFixed(2);
    setDraft(formatted);
    await onCommit(fieldName, next);
  };

  return (
    <label className={inspectorClass.field}>
      {labelNode}
      {isObject ? (
        <textarea
          className={cx(inspectorClass.input, inspectorClass.json)}
          aria-label={label}
          value={draft}
          rows={4}
          onChange={event => setDraft(event.currentTarget.value)}
          onBlur={commit}
          onKeyDown={event => {
            if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') event.currentTarget.blur();
            if (event.key === 'Escape') {
              setDraft(formatInspectorValue(value));
              event.currentTarget.blur();
            }
          }}
        />
      ) : (
        <input
          className={inspectorClass.input}
          aria-label={label}
          value={draft}
          inputMode={typeof value === 'number' || (Array.isArray(value) && value.every(item => typeof item === 'number')) ? 'decimal' : undefined}
          onChange={event => setDraft(event.currentTarget.value)}
          onBlur={commit}
          onWheel={wheelNumberField}
          onKeyDown={event => {
            if (event.key === 'Enter') event.currentTarget.blur();
            if (event.key === 'Escape') {
              setDraft(formatInspectorValue(value));
              event.currentTarget.blur();
            }
          }}
        />
      )}
    </label>
  );
}

function isScriptPath(path?: string): boolean {
  return Boolean(path && /\.(aster|rhai|js|ts|tsx|lua|rs)$/i.test(path));
}

function assetKindLabel(kind?: string): string {
  const normalized = (kind || 'unknown').toLowerCase();
  if (/script|code|rhai|aster/.test(normalized)) return '脚本';
  if (/material/.test(normalized)) return '材质';
  if (/prefab/.test(normalized)) return '预制体';
  if (/scene/.test(normalized)) return '场景';
  if (/model|mesh/.test(normalized)) return '模型';
  if (/audio|sound/.test(normalized)) return '音频';
  if (/texture|image/.test(normalized)) return '贴图';
  return '其他';
}

function importerLabel(importer?: string): string {
  const normalized = (importer || 'default').toLowerCase();
  if (normalized === 'default') return '默认导入';
  if (/rhai/.test(normalized)) return 'Rhai 脚本';
  if (/script/.test(normalized)) return '脚本导入';
  if (/material/.test(normalized)) return '材质导入';
  if (/prefab/.test(normalized)) return '预制体导入';
  if (/scene/.test(normalized)) return '场景导入';
  return importer || '默认导入';
}

function artifactKindLabel(kind: ArtifactSelection['kind']): string {
  if (kind === 'code') return '代码';
  if (kind === 'model') return '场景对象';
  return '资源';
}

function buildStatusLabel(status: BuildTargetOption['status']): string {
  if (status === 'ready') return '可用';
  if (status === 'planned') return '规划中';
  return '待补齐';
}

function formatBuildFormat(format: BuildFormat): string {
  const labels: Record<BuildFormat, string> = {
    folder: '文件夹',
    exe: 'EXE',
    msi: 'MSI',
    nsis: 'NSIS',
    appimage: 'AppImage',
    deb: 'DEB',
    rpm: 'RPM',
    dmg: 'DMG',
    apk: 'APK',
    aab: 'AAB',
    ipa: 'IPA',
    ipk: 'IPK',
  };
  return labels[format] ?? format;
}

function formatBuildChannel(channel: BuildChannel): string {
  return channel === 'release' ? '发布' : '调试';
}

function canRunBuild(target: BuildTargetOption, format: BuildFormat): boolean {
  return target.status === 'ready' && format === 'folder';
}

function buildUnavailableReason(target: BuildTargetOption, format: BuildFormat): string | null {
  if (target.status !== 'ready') return `${target.label} 还没有连接到当前主机的构建工具链。`;
  if (format !== 'folder') return `${formatBuildFormat(format)} 安装器生成还没有后端实现，当前只能导出文件夹包。`;
  return null;
}

function normalizeDiagnosticLevel(level: string): DiagnosticLevelFilter {
  const normalized = level.toLowerCase();
  if (normalized === 'warning') return 'warn';
  if (normalized === 'error' || normalized === 'warn' || normalized === 'info' || normalized === 'debug') return normalized;
  return 'info';
}

function diagnosticLevelLabel(level: string): string {
  const normalized = normalizeDiagnosticLevel(level);
  if (normalized === 'error') return '错误';
  if (normalized === 'warn') return '警告';
  if (normalized === 'debug') return '调试';
  return '信息';
}

function healthStatusLabel(status: DiagnosticHealthStatus): string {
  if (status === 'ok') return '正常';
  if (status === 'error') return '异常';
  if (status === 'not_configured') return '未配置';
  return '注意';
}

function healthStatusClass(status: DiagnosticHealthStatus): string {
  if (status === 'ok') return diagnosticsClass.statusOk;
  if (status === 'error') return diagnosticsClass.statusError;
  if (status === 'not_configured') return diagnosticsClass.statusMuted;
  return diagnosticsClass.statusWarning;
}

function healthStatusDotClass(status: DiagnosticHealthStatus): string {
  if (status === 'ok') return diagnosticsClass.statusDotOk;
  if (status === 'error') return diagnosticsClass.statusDotError;
  if (status === 'not_configured') return diagnosticsClass.statusDotMuted;
  return diagnosticsClass.statusDotWarning;
}

function formatHealthScanTime(timestamp?: number): string {
  if (!timestamp || !Number.isFinite(timestamp)) return '尚未扫描';
  const date = new Date(timestamp < 1e12 ? timestamp * 1000 : timestamp);
  if (Number.isNaN(date.getTime())) return '尚未扫描';
  return date.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

function formatConsoleTime(timestamp: number): string {
  if (!Number.isFinite(timestamp)) return '--:--:--';
  const date = new Date(timestamp < 1e12 ? timestamp * 1000 : timestamp);
  if (Number.isNaN(date.getTime())) return '--:--:--';
  return date.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

// Infer a scene-tree icon from a node's name/tag, the way Godot shows a
// type glyph per node. Heuristic only — the backend exposes no component
// list on the lightweight scene-tree payload.
function sceneNodeIcon(object: SceneObject, selected: boolean): React.ReactNode {
  const hint = `${object.name} ${object.tag}`.toLowerCase();
  const cls = selected ? gameClass.hierarchyIconSelected : gameClass.hierarchyIcon;
  let glyph: React.ReactNode;
  if (/camera/.test(hint)) glyph = <IconView size={14} />;
  else if (/light|lamp|sun/.test(hint)) glyph = <IconSun size={14} />;
  else if (/audio|sound|music|speaker/.test(hint)) glyph = <IconAudio size={14} />;
  else if (/script|behavior|behaviour/.test(hint)) glyph = <IconCode size={14} />;
  else if (/mesh|model|cube|sphere|player|prop/.test(hint)) glyph = <IconModel size={14} />;
  else glyph = <span className="size-1.5 rounded-full bg-current" />;
  return <span className={cx(cls, 'grid size-3.5 place-items-center')}>{glyph}</span>;
}

function questArtifactContext(artifact: QuestEditorArtifact): QuestArtifactContext {
  const path = artifact.path;
  if (artifact.kind === 'spec' || artifact.kind === 'intent') {
    return {
      surface: 'prd',
      title: artifact.kind === 'spec' ? '任务规格说明' : '任务意图',
      description: '已作为规划文档打开，可审核或手动细化。',
      focusPath: path,
    };
  }
  if (artifact.kind === 'changed_file' && isScriptPath(path)) {
    return {
      surface: 'scripts',
      title: '任务脚本改动',
      description: '已在脚本工作区打开，可进行代码审核和本地修正。',
      focusPath: path,
    };
  }
  if (artifact.kind === 'changed_file' && /\.(scene|scn|prefab|level|ron|json)$/i.test(path ?? '')) {
    return {
      surface: 'game',
      title: '任务场景或资源改动',
      description: '已在可视化编辑区打开，可检查层级、视口和属性。',
      focusPath: path,
    };
  }
  if (artifact.kind === 'validation') {
    return {
      surface: 'tasks',
      title: '任务验证证据',
      description: '查看验证结果，并结合编辑器诊断或 AI 继续定位问题。',
      focusPath: path,
    };
  }
  if (artifact.kind === 'review_finding') {
    return {
      surface: 'tasks',
      title: '任务审核发现',
      description: '先查看未解决问题，再决定本地修复、继续任务或重新调整。',
      focusPath: path,
    };
  }
  if (artifact.kind === 'checkpoint') {
    return {
      surface: 'tasks',
      title: '任务检查点',
      description: '恢复或应用前，先检查可恢复检查点和工作记录。',
      focusPath: path,
    };
  }
  if (artifact.kind === 'trace') {
    return {
      surface: 'tasks',
      title: '任务时间线记录',
      description: '在任务上下文中查看执行历史、决策、验证和审核事件。',
      focusPath: path,
    };
  }
  return {
    surface: 'tasks',
    title: '任务产物',
    description: '已在任务工作区打开，可检查并继续交给 AI 跟进。',
    focusPath: path,
  };
}

function WorkspaceInspector({ object, onFocus, onPositionChange }: {
  object: SceneObject;
  onFocus: () => void;
  onPositionChange: (position: [number, number, number]) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [position, setPosition] = useState<[string, string, string]>(() => (
    object.position.map(value => value.toFixed(2)) as [string, string, string]
  ));

  useEffect(() => {
    setPosition(object.position.map(value => value.toFixed(2)) as [string, string, string]);
  }, [object.id, object.position]);

  const commitPosition = useCallback(async () => {
    const next = position.map(Number) as [number, number, number];
    if (next.some(value => !Number.isFinite(value))) {
      setPosition(object.position.map(value => value.toFixed(2)) as [string, string, string]);
      return;
    }
    await onPositionChange(next);
  }, [object.position, onPositionChange, position]);

  return (
    <div className={workspaceSelectionClass.card}>
      <div className={workspaceSelectionClass.title}>
        <div className={workspaceSelectionClass.titleText}>
          <strong className={workspaceSelectionClass.name}>{object.name}</strong>
          <span className={workspaceSelectionClass.tag}>{object.tag || t('entity_untagged')}</span>
        </div>
        <span className={workspaceSelectionClass.liveBadge}>{t('badge_live')}</span>
      </div>
      <label className={workspaceSelectionClass.label}>{t('prop_position')}</label>
      <div className={workspaceSelectionClass.positionGrid}>
        {position.map((value, index) => (
          <label className={workspaceSelectionClass.positionInputWrap} key={index}>
            <span
              className={cx(
                workspaceSelectionClass.positionAxis,
                index === 0 && 'text-[var(--axis-x)]',
                index === 1 && 'text-[var(--axis-y)]',
                index === 2 && 'text-[var(--axis-z)]',
              )}
            >
              {['X', 'Y', 'Z'][index]}
            </span>
            <input
              className={workspaceSelectionClass.positionInput}
              value={value}
              inputMode="decimal"
              aria-label={`${['X', 'Y', 'Z'][index]} position`}
              onChange={event => setPosition(current => {
                const next = [...current] as [string, string, string];
                next[index] = event.target.value;
                return next;
              })}
              onBlur={commitPosition}
              onKeyDown={event => {
                if (event.key === 'Enter') event.currentTarget.blur();
                if (event.key === 'Escape') {
                  setPosition(object.position.map(item => item.toFixed(2)) as [string, string, string]);
                  event.currentTarget.blur();
                }
              }}
            />
          </label>
        ))}
      </div>
      <button className={workspaceSelectionClass.button} onClick={onFocus}>{t('editor_focus_viewport')}</button>
    </div>
  );
}

function ViewportHud({
  projectName,
  sceneObjectCount,
  selectedObject,
  selectedEntityDetails,
  viewMode,
  dirty,
}: {
  projectName?: string;
  sceneObjectCount: number;
  selectedObject: SceneObject | null;
  selectedEntityDetails: EntityDetails | null;
  viewMode: '2d' | '3d';
  dirty: boolean;
}) {
  return (
    <div className={gameClass.viewportHud}>
      <div className={gameClass.viewportHudGroup}>
        <span className={gameClass.viewportPill}>
          场景 <strong className={gameClass.viewportPillStrong}>{projectName || '未命名项目'}</strong>
        </span>
        <span className={gameClass.viewportPill}>{sceneObjectCount} 个对象</span>
        <span className={gameClass.viewportPill}>{viewMode.toUpperCase()} 视图</span>
        {dirty && <span className={gameClass.viewportPill}>有未保存改动</span>}
        {selectedObject && (
          <span className={gameClass.viewportPill}>
            已选中 <strong className={gameClass.viewportPillStrong}>{selectedObject.name}</strong>
            {selectedEntityDetails ? ` · ${selectedEntityDetails.components.length} 组件` : null}
          </span>
        )}
      </div>
      <div className={gameClass.viewportHint}>拖动画布旋转/平移，滚轮缩放；点击场景对象可选中并交给 AI 分析。</div>
    </div>
  );
}

function ViewportSelectionCard({
  object,
  details,
  onFocus,
  onOpenInspector,
  onAskAi,
}: {
  object: SceneObject;
  details: EntityDetails | null;
  onFocus: () => void;
  onOpenInspector: () => void;
  onAskAi: () => void;
}) {
  const componentCount = details?.components.length ?? 0;
  return (
    <aside
      className={gameClass.selectionCard}
      aria-label="选中对象信息"
      onClick={event => event.stopPropagation()}
    >
      <header className={gameClass.selectionCardHeader}>
        <div className="min-w-0">
          <span className={gameClass.selectionCardKicker}>当前选择</span>
          <strong className={gameClass.selectionCardTitle}>{object.name}</strong>
        </div>
        <span className={gameClass.selectionCardTag}>{object.tag || '未标记'}</span>
      </header>
      <div className={gameClass.selectionMetrics}>
        <div className={gameClass.selectionMetric}>
          <span className={gameClass.selectionMetricLabel}>位置</span>
          <strong className={cx(gameClass.selectionMetricValue, gameClass.selectionMetricPosition)}>{formatScenePosition(object.position)}</strong>
        </div>
        <div className={gameClass.selectionMetric}>
          <span className={gameClass.selectionMetricLabel}>组件</span>
          <strong className={gameClass.selectionMetricValue}>{componentCount}</strong>
        </div>
        <div className={gameClass.selectionMetric}>
          <span className={gameClass.selectionMetricLabel}>ID</span>
          <strong className={gameClass.selectionMetricValue}>{object.id.slice(0, 8)}</strong>
        </div>
      </div>
      <div className={gameClass.selectionActions}>
        <button className={gameClass.selectionActionButton} onClick={onFocus}>聚焦</button>
        <button className={gameClass.selectionActionButton} onClick={onOpenInspector}>属性</button>
        <button className={gameClass.selectionActionButton} onClick={onAskAi}><IconSparkles /> 让 AI 分析</button>
      </div>
    </aside>
  );
}

function EmptyViewportState({
  onCreateObject,
  onCreateCamera,
  onOpenAi,
}: {
  onCreateObject: () => void;
  onCreateCamera: () => void;
  onOpenAi: () => void;
}) {
  return (
    <div
      className={gameClass.emptyViewportState}
      role="status"
      onClick={event => event.stopPropagation()}
    >
      <span className={gameClass.emptyViewportKicker}>空场景</span>
      <strong className={gameClass.emptyViewportTitle}>先建立一个可编辑对象</strong>
      <p className={gameClass.emptyViewportText}>这里会显示真实场景对象、选择状态和运行视图。你可以手动创建，也可以打开右侧 AI，让它按你的描述生成初始场景。</p>
      <div className={gameClass.emptyViewportActions}>
        <button className={cx(gameClass.emptyViewportButton, gameClass.emptyViewportButtonPrimary)} onClick={onCreateObject}>
          <IconPlus /> 空对象
        </button>
        <button className={gameClass.emptyViewportButton} onClick={onCreateCamera}>
          <IconPlus /> 摄像机
        </button>
        <button className={gameClass.emptyViewportButton} onClick={onOpenAi}>
          <IconSparkles /> 打开 AI
        </button>
      </div>
    </div>
  );
}

// ─── Resize Handle Hook ─────────────────────────────────────────────────────

function useDragHandle(
  axis: 'horizontal',
  onDelta: (delta: number) => void,
) {
  const dragging = useRef(false);
  const startPos = useRef(0);

  const onMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      dragging.current = true;
      startPos.current = e.clientX;

      const onMouseMove = (ev: MouseEvent) => {
        if (!dragging.current) return;
        const current = ev.clientX;
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
    [onDelta],
  );

  return onMouseDown;
}

// ─── Viewport ────────────────────────────────────────────────────────────────

function ViewportCanvas({ sceneVersion = 0, cameraRef, onCameraChange, onResize, viewMode, playMode, editorCamera }: {
  sceneVersion?: number;
  cameraRef?: React.MutableRefObject<{
    yaw: number; pitch: number; distance: number;
    targetX: number; targetY: number; targetZ: number;
  }>;
  onCameraChange?: () => void;
  onResize?: (size: { width: number; height: number }) => void;
  viewMode: '2d' | '3d';
  playMode?: boolean;
  editorCamera?: boolean;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const contextRef = useRef<CanvasRenderingContext2D | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const sizeRef = useRef({ width: 640, height: 480 });
  const isActiveRef = useRef(true);
  const versionRef = useRef(sceneVersion);
  const lastRenderedVersionRef = useRef<number | null>(null);
  const fastPreviewUntilRef = useRef(0);
  const onResizeRef = useRef(onResize);
  const internalCameraRef = useRef({
    yaw: -0.5, pitch: 0.3, distance: 6,
    targetX: 0, targetY: 1, targetZ: 0,
  });
  const camRef = cameraRef ?? internalCameraRef;
  const dragging = useRef<'orbit' | 'pan' | null>(null);
  const dragStart = useRef({
    x: 0, y: 0, yaw: 0, pitch: 0, targetX: 0, targetY: 0, targetZ: 0,
  });

  versionRef.current = sceneVersion;
  onResizeRef.current = onResize;

  // Poll for frames via binary IPC
  useEffect(() => {
    isActiveRef.current = true;
    lastRenderedVersionRef.current = null;
    const poll = async () => {
      if (!isActiveRef.current) return;
      const { width, height } = sizeRef.current;
      const cam = camRef.current;
      try {
        const buffer = await viewportReadback({
          width, height,
          lastVersion: lastRenderedVersionRef.current ?? undefined,
          yaw: cam.yaw, pitch: cam.pitch, distance: cam.distance,
          targetX: cam.targetX, targetY: cam.targetY, targetZ: cam.targetZ,
          viewMode,
          playMode,
          editorCamera,
        });
        if (!isActiveRef.current || !canvasRef.current) return;
        const uint8 = new Uint8Array(buffer);
        const header = new Uint32Array(uint8.buffer, uint8.byteOffset, 2);
        const w = header[0];
        const h = header[1];
        if (w > 0 && h > 0) {
          lastRenderedVersionRef.current = versionRef.current;
          const canvas = canvasRef.current;
          if (canvas.width !== w || canvas.height !== h) {
            canvas.width = w;
            canvas.height = h;
            contextRef.current = null;
          }
          const ctx = contextRef.current ?? canvas.getContext('2d');
          contextRef.current = ctx;
          if (ctx) {
            const pixelOffset = uint8.byteOffset + 8;
            const pixelBytes = w * h * 4;
            const imageData = new ImageData(
              new Uint8ClampedArray(uint8.buffer, pixelOffset, pixelBytes),
              w, h,
            );
            ctx.putImageData(imageData, 0, 0);
          }
        }
      } catch (e) {
        console.error('[viewport] readback error:', e);
      }
      // GPU readback is synchronous on the backend and copies the full RGBA
      // frame through IPC. Refresh quickly only while the camera is actively
      // moving; keep idle scene previews on a low-cost dirty/version poll.
      const previewIsActive = performance.now() < fastPreviewUntilRef.current;
      window.setTimeout(poll, playMode || previewIsActive ? 16 : 100);
    };
    poll();
    return () => { isActiveRef.current = false; };
  }, [viewMode, playMode, editorCamera]);

  // Resize observer
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const initRect = container.getBoundingClientRect();
    sizeRef.current = {
      width: Math.round(initRect.width) || 640,
      height: Math.round(initRect.height) || 480,
    };
    onResizeRef.current?.(sizeRef.current);
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect;
        if (width > 0 && height > 0) {
          const nextSize = { width: Math.round(width), height: Math.round(height) };
          sizeRef.current = nextSize;
          onResizeRef.current?.(nextSize);
          lastRenderedVersionRef.current = null;
          const canvas = canvasRef.current;
          if (canvas) {
            canvas.width = Math.round(width);
            canvas.height = Math.round(height);
            contextRef.current = null;
          }
        }
      }
    });
    observer.observe(container);
    return () => observer.disconnect();
  }, []);

  // Mouse handlers for orbit/pan
  const onMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button === 2) {
      dragging.current = viewMode === '2d' ? 'pan' : 'orbit';
      dragStart.current = { x: e.clientX, y: e.clientY, ...camRef.current };
      e.preventDefault();
    } else if (e.button === 1) {
      dragging.current = 'pan';
      dragStart.current = { x: e.clientX, y: e.clientY, ...camRef.current };
      e.preventDefault();
    }
  }, [viewMode]);

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!dragging.current) return;
      const dpiScale = window.devicePixelRatio || 1;
      const dx = (e.clientX - dragStart.current.x) / dpiScale;
      const dy = (e.clientY - dragStart.current.y) / dpiScale;
      if (dragging.current === 'orbit') {
        camRef.current.yaw = dragStart.current.yaw - dx * 0.005;
        camRef.current.pitch = Math.max(-1.5, Math.min(1.5, dragStart.current.pitch + dy * 0.005));
      } else if (dragging.current === 'pan') {
        const d = camRef.current.distance * 0.002;
        const yaw = camRef.current.yaw;
        camRef.current.targetX = dragStart.current.targetX + (-dx * Math.cos(yaw) - dy * Math.sin(yaw) * 0.5) * d;
        camRef.current.targetY = dragStart.current.targetY + dy * d * 0.5;
        camRef.current.targetZ = dragStart.current.targetZ + (dx * Math.sin(yaw) - dy * Math.cos(yaw) * 0.5) * d;
      }
      lastRenderedVersionRef.current = null;
      fastPreviewUntilRef.current = performance.now() + 160;
      onCameraChange?.();
    };
    const handleMouseUp = () => { dragging.current = null; };
    const handleWheel = (e: WheelEvent) => {
      if (containerRef.current && containerRef.current.contains(e.target as Node)) {
        camRef.current.distance = Math.max(0.5, Math.min(100, camRef.current.distance + e.deltaY * 0.01));
        lastRenderedVersionRef.current = null;
        fastPreviewUntilRef.current = performance.now() + 160;
        onCameraChange?.();
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
      className={viewportClass.container}
      onMouseDown={onMouseDown}
      onContextMenu={(e) => e.preventDefault()}
    >
      <canvas ref={canvasRef} className={viewportClass.canvas} />
      <ViewportGrid show={true} />
      {viewMode === '3d' && <OrientationGizmo camera={camRef.current} onSnapToAxis={(axis) => {
        switch (axis) {
          case 'top':    camRef.current.pitch = 1.5;  camRef.current.yaw = 0;     break;
          case 'bottom': camRef.current.pitch = -1.5; camRef.current.yaw = 0;     break;
          case 'left':   camRef.current.pitch = 0;    camRef.current.yaw = 1.5;   break;
          case 'right':  camRef.current.pitch = 0;    camRef.current.yaw = -1.5;  break;
          case 'front':  camRef.current.pitch = 0;    camRef.current.yaw = 0;     break;
          case 'back':   camRef.current.pitch = 0;    camRef.current.yaw = 3.14;  break;
        }
        lastRenderedVersionRef.current = null;
        fastPreviewUntilRef.current = performance.now() + 160;
        onCameraChange?.();
      }} />}
    </div>
  );
}
// ─── Click-to-Pick Utility ──────────────────────────────────────────────────

const PICK_RADIUS_PX = 30;
const VIEWPORT_FOV_DEG = 60;

/**
 * Given a click position in the viewport, find the closest scene object.
 * Returns the entity ID or null if nothing is close enough.
 */
function pickEntityAtScreen(
  clickX: number,
  clickY: number,
  vpWidth: number,
  vpHeight: number,
  sceneTree: SceneObject[],
  camera: { yaw: number; pitch: number; distance: number; targetX: number; targetY: number; targetZ: number },
  viewMode: '2d' | '3d',
): string | null {
  if (sceneTree.length === 0 || vpWidth <= 0 || vpHeight <= 0) return null;

  const viewMatrix = createViewMatrix(
    viewMode === '2d' ? 0 : camera.yaw,
    viewMode === '2d' ? 0 : camera.pitch,
    camera.distance,
    camera.targetX, camera.targetY, camera.targetZ,
  );
  const fovRad = VIEWPORT_FOV_DEG * Math.PI / 180;
  const projMatrix = viewMode === '2d'
    ? createOrthographicMatrix(camera.distance * 2, vpWidth / vpHeight, 0.01, 1000)
    : createPerspectiveMatrix(fovRad, vpWidth / vpHeight, 0.1, 1000);

  let bestId: string | null = null;
  let bestDist = PICK_RADIUS_PX;
  let bestDepth = Infinity;

  for (const obj of sceneTree) {
    const screen = projectToScreen(obj.position, viewMatrix, projMatrix, vpWidth, vpHeight);
    if (!screen) continue;

    const dx = screen.x - clickX;
    const dy = screen.y - clickY;
    const dist = Math.sqrt(dx * dx + dy * dy);

    if (dist < bestDist || (dist === bestDist && screen.depth < bestDepth)) {
      bestDist = dist;
      bestDepth = screen.depth;
      bestId = obj.id;
    }
  }

  return bestId;
}

// ─── Selection Overlay ──────────────────────────────────────────────────────

function SelectionOverlay({ sceneTree, selectedId, camera, width, height, viewMode }: {
  sceneTree: SceneObject[];
  selectedId: string | null;
  camera: { yaw: number; pitch: number; distance: number; targetX: number; targetY: number; targetZ: number };
  width: number;
  height: number;
  viewMode: '2d' | '3d';
}) {
  const selected = selectedId ? sceneTree.find(o => o.id === selectedId) : null;

  const screenPos = useMemo(() => {
    if (!selected) return null;
    const viewMatrix = createViewMatrix(
      viewMode === '2d' ? 0 : camera.yaw,
      viewMode === '2d' ? 0 : camera.pitch,
      camera.distance,
      camera.targetX, camera.targetY, camera.targetZ,
    );
    const fovRad = VIEWPORT_FOV_DEG * Math.PI / 180;
    const aspect = width / Math.max(height, 1);
    const projMatrix = viewMode === '2d'
      ? createOrthographicMatrix(camera.distance * 2, aspect, 0.01, 1000)
      : createPerspectiveMatrix(fovRad, aspect, 0.01, 1000);
    return projectToScreen(selected.position, viewMatrix, projMatrix, width, height);
  }, [selected, camera.yaw, camera.pitch, camera.distance, camera.targetX, camera.targetY, camera.targetZ, width, height, viewMode]);

  if (!selected || !screenPos) return null;

  return (
    <svg
      className={viewportClass.selectionOverlay}
      viewBox={`0 0 ${width} ${height}`}
      preserveAspectRatio="none"
    >
      {/* Selection ring */}
      <circle
        cx={screenPos.x}
        cy={screenPos.y}
        r={18}
        fill="none"
        stroke="var(--accent, #A1A1AA)"
        strokeWidth={2}
        strokeDasharray="4 3"
        opacity={0.9}
      />
      {/* Entity name label */}
      <rect
        x={screenPos.x + 22}
        y={screenPos.y - 10}
        width={Math.max(60, selected.name.length * 7 + 16)}
        height={20}
        rx={4}
        fill="rgba(39, 39, 42, 0.9)"
      />
      <text
        x={screenPos.x + 30}
        y={screenPos.y + 4}
        fill="white"
        fontSize={11}
        fontFamily="var(--font-sans)"
        fontWeight={500}
      >
        {selected.name}
      </text>
    </svg>
  );
}

// ─── Editor Page ────────────────────────────────────────────────────────────

export default function EditorPage({
  onCloseProject,
  onOpenSettings,
  onOpenQuest,
  questArtifact,
  onDismissQuestArtifact,
}: Props) {
  const { t } = useTranslation();

  // State
  const [shellState, setShellState] = useState<ShellState | null>(null);
  const [sceneTree, setSceneTree] = useState<SceneObject[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [sceneVersion, setSceneVersion] = useState(0);
  const [showCloseDialog, setShowCloseDialog] = useState(false);
  const [aiPanelWidth, setAiPanelWidth] = useState(() => {
    const saved = Number(window.localStorage.getItem('aster.aiPanelWidth'));
    return Number.isFinite(saved) ? Math.max(360, Math.min(saved, 430)) : 390;
  });
  const [aiPanelOpen, setAiPanelOpen] = useState(() => (
    window.localStorage.getItem('aster.aiPanelOpen') !== 'false'
  ));
  const [inspectorOpen, setInspectorOpen] = useState(() => (
    window.localStorage.getItem('aster.inspectorOpen') === 'true'
  ));
  const [hierarchyOpen, setHierarchyOpen] = useState(() => (
    window.localStorage.getItem('aster.hierarchyOpen') !== 'false'
  ));
  const [workspaceView, setWorkspaceView] = useState<WorkspaceView>('game');
  const [aiWorkspace, setAiWorkspace] = useState<AiWorkspaceState | null>(null);
  const [scripts, setScripts] = useState<string[]>([]);
  const [assets, setAssets] = useState<ProjectAssetMeta[]>([]);
  const [assetsBusy, setAssetsBusy] = useState(false);
  const [assetKindFilter, setAssetKindFilter] = useState('全部');
  const [selectedScript, setSelectedScript] = useState<string | null>(null);
  const [openedQuestArtifact, setOpenedQuestArtifact] = useState<QuestArtifactContext | null>(null);
  const [selectedEntityDetails, setSelectedEntityDetails] = useState<EntityDetails | null>(null);
  const [selectedEntityNameDraft, setSelectedEntityNameDraft] = useState('');
  const [addComponentType, setAddComponentType] = useState('Camera');
  const [componentSchemas, setComponentSchemas] = useState<ComponentSchema[]>([]);
  const [scriptContent, setScriptContent] = useState('');
  const [scriptSavedContent, setScriptSavedContent] = useState('');
  const [scriptSaving, setScriptSaving] = useState(false);
  const [scriptLineRange, setScriptLineRange] = useState<[number, number] | null>(null);
  const [scriptDiagnostics, setScriptDiagnostics] = useState<AsterScriptDiagnostic[]>([]);
  const [consoleEntries, setConsoleEntries] = useState<EditorConsoleEntry[]>([]);
  const [consoleBusy, setConsoleBusy] = useState(false);
  const [buildTarget, setBuildTarget] = useState<BuildTarget>(CURRENT_DESKTOP_BUILD_TARGET);
  const [buildFormat, setBuildFormat] = useState<BuildFormat>('folder');
  const [buildChannel, setBuildChannel] = useState<BuildChannel>('debug');
  const [buildOptimizeAssets, setBuildOptimizeAssets] = useState(true);
  const [buildIncludeDebugSymbols, setBuildIncludeDebugSymbols] = useState(false);
  const [buildBusy, setBuildBusy] = useState(false);
  const [buildMessage, setBuildMessage] = useState<string | null>(null);
  const [diagnosticFilter, setDiagnosticFilter] = useState<DiagnosticLevelFilter>('all');
  const [healthReport, setHealthReport] = useState<DiagnosticHealthReport | null>(null);
  const [healthBusy, setHealthBusy] = useState(false);
  const [selectedHealthItemId, setSelectedHealthItemId] = useState<string | null>(null);
  const [healthFixBusy, setHealthFixBusy] = useState<string | null>(null);
  const [healthFixMessage, setHealthFixMessage] = useState<string | null>(null);
  const [artifactSelection, setArtifactSelection] = useState<ArtifactSelection | null>(null);
  const [artifactQuestionOpen, setArtifactQuestionOpen] = useState(false);
  const [artifactQuestion, setArtifactQuestion] = useState('');
  const [contextualRequest, setContextualRequest] = useState<{ id: number; prompt: string } | null>(null);
  const [guides, setGuides] = useState<GuideEntity[]>([]);
  const [viewMode, setViewMode] = useState<'2d' | '3d'>('3d');
  const [viewportSize, setViewportSize] = useState({ width: 640, height: 480 });
  const [, setCameraRevision] = useState(0);
  const cameraRevisionFrameRef = useRef<number | null>(null);
  const prevSceneVersionRef = useRef(0);
  const previousAiPlanRef = useRef(false);
  const previousAiCompletionRef = useRef(false);
  const cameraRef = useRef({
    yaw: -0.5, pitch: 0.3, distance: 6,
    targetX: 0, targetY: 1, targetZ: 0,
  });
  const [collapsedNodes, setCollapsedNodes] = useState<Set<string>>(() => new Set());
  const validParentOptions = useMemo(() => {
    if (!selectedId) return sceneTree;
    const descendants = new Set<string>();
    let changed = true;
    while (changed) {
      changed = false;
      for (const object of sceneTree) {
        if (
          object.parent_id
          && (object.parent_id === selectedId || descendants.has(object.parent_id))
          && !descendants.has(object.id)
        ) {
          descendants.add(object.id);
          changed = true;
        }
      }
    }
    return sceneTree.filter(object => object.id !== selectedId && !descendants.has(object.id));
  }, [sceneTree, selectedId]);

  const componentSchemaByType = useMemo(() => {
    const map = new Map<string, ComponentSchema>();
    for (const schema of componentSchemas) map.set(schema.type_id, schema);
    return map;
  }, [componentSchemas]);

  const addableComponentSchemas = useMemo(() => {
    const existing = new Set(selectedEntityDetails?.components.map(component => component.type) ?? []);
    return [...componentSchemas]
      .filter(schema => !existing.has(schema.type_id))
      .sort((left, right) => {
        const leftIndex = COMPONENT_PICK_ORDER.indexOf(left.type_id);
        const rightIndex = COMPONENT_PICK_ORDER.indexOf(right.type_id);
        const leftRank = leftIndex === -1 ? Number.MAX_SAFE_INTEGER : leftIndex;
        const rightRank = rightIndex === -1 ? Number.MAX_SAFE_INTEGER : rightIndex;
        return leftRank - rightRank || left.display_name.localeCompare(right.display_name);
      });
  }, [componentSchemas, selectedEntityDetails?.components]);

  // Flatten the scene tree into ordered rows with depth, parent-child grouping,
  // and collapse handling — the render layer just maps over this.
  const hierarchyRows = useMemo(() => {
    const childrenOf = new Map<string | null, SceneObject[]>();
    for (const object of sceneTree) {
      const key = object.parent_id ?? null;
      const list = childrenOf.get(key);
      if (list) list.push(object);
      else childrenOf.set(key, [object]);
    }
    const rows: Array<{ object: SceneObject; depth: number; hasChildren: boolean }> = [];
    const walk = (parentId: string | null, depth: number) => {
      for (const object of childrenOf.get(parentId) ?? []) {
        const hasChildren = childrenOf.has(object.id);
        rows.push({ object, depth, hasChildren });
        if (hasChildren && !collapsedNodes.has(object.id)) {
          walk(object.id, depth + 1);
        }
      }
    };
    walk(null, 0);
    return rows;
  }, [sceneTree, collapsedNodes]);

  const toggleNodeCollapsed = useCallback((id: string) => {
    setCollapsedNodes(current => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  // Gizmo state
  const [activeTool] = useState<'view' | 'move' | 'rotate' | 'scale'>('move');
  const [transformSpace] = useState<'global' | 'local'>('global');
  const [selectedPosition, setSelectedPosition] = useState<Vec3 | null>(null);
  const handleViewportResize = useCallback((size: { width: number; height: number }) => {
    setViewportSize(current => {
      if (current.width === size.width && current.height === size.height) return current;
      return size;
    });
  }, []);

  const handleCameraChange = useCallback(() => {
    if (cameraRevisionFrameRef.current !== null) return;
    cameraRevisionFrameRef.current = window.requestAnimationFrame(() => {
      cameraRevisionFrameRef.current = null;
      setCameraRevision(revision => revision + 1);
    });
  }, []);

  useEffect(() => () => {
    if (cameraRevisionFrameRef.current !== null) {
      window.cancelAnimationFrame(cameraRevisionFrameRef.current);
    }
  }, []);

  useEffect(() => {
    window.localStorage.setItem('aster.aiPanelWidth', String(aiPanelWidth));
  }, [aiPanelWidth]);

  useEffect(() => {
    window.localStorage.setItem('aster.aiPanelOpen', String(aiPanelOpen));
  }, [aiPanelOpen]);

  useEffect(() => {
    if (contextualRequest || aiWorkspace?.plan || aiWorkspace?.completedBundle || aiWorkspace?.status === 'thinking' || aiWorkspace?.status === 'executing' || aiWorkspace?.status === 'error') {
      setAiPanelOpen(true);
    }
  }, [aiWorkspace?.completedBundle, aiWorkspace?.plan, aiWorkspace?.status, contextualRequest]);

  useEffect(() => {
    window.localStorage.setItem('aster.inspectorOpen', String(inspectorOpen));
  }, [inspectorOpen]);

  useEffect(() => {
    window.localStorage.setItem('aster.hierarchyOpen', String(hierarchyOpen));
  }, [hierarchyOpen]);

  useEffect(() => {
    if (!shellState?.has_project) {
      setComponentSchemas([]);
      return;
    }
    let cancelled = false;
    rpc<{ components: ComponentSchema[] }>('shell/list_component_schemas')
      .then(result => {
        if (!cancelled) setComponentSchemas(result.components ?? []);
      })
      .catch(() => {
        if (!cancelled) setComponentSchemas([]);
      });
    return () => {
      cancelled = true;
    };
  }, [shellState?.has_project]);

  useEffect(() => {
    if (addableComponentSchemas.length === 0) return;
    if (!addableComponentSchemas.some(schema => schema.type_id === addComponentType)) {
      setAddComponentType(addableComponentSchemas[0].type_id);
    }
  }, [addComponentType, addableComponentSchemas]);

  // Periodic state poll
  useEffect(() => {
    const poll = async () => {
      try {
        const state = await rpc<ShellState>('shell/get_state');
        setShellState(state);
        const newVer = state.scene_version ?? 0;
        if (newVer !== prevSceneVersionRef.current) {
          prevSceneVersionRef.current = newVer;
          setSceneVersion(newVer);
          const { objects } = await rpc<{ objects: SceneObject[] }>('shell/get_scene_tree');
          setSceneTree(objects);
        }
      } catch { /* not ready */ }
    };
    poll();
    const interval = setInterval(poll, 2000);
    return () => clearInterval(interval);
  }, []);

  const refreshProjectAssets = useCallback(async () => {
    setAssetsBusy(true);
    try {
      const result = await rpc<{ entries: Array<{ path: string; kind: string }>; assets: ProjectAssetMeta[] }>('project/list_assets');
      setAssets(result.assets);
      const paths = result.entries
        .filter(entry => /script|model/i.test(entry.kind) || /\.(aster|rhai|amdl|js|ts|lua)$/i.test(entry.path))
        .map(entry => entry.path);
      setScripts(paths);
      setSelectedScript(current => current && paths.includes(current) ? current : paths[0] ?? null);
    } catch {
      setAssets([]);
      setScripts([]);
    } finally {
      setAssetsBusy(false);
    }
  }, []);

  useEffect(() => {
    if (!shellState?.has_project) return;
    refreshProjectAssets();
  }, [refreshProjectAssets, sceneVersion, shellState?.has_project]);

  useEffect(() => {
    if (!selectedScript) {
      setScriptContent('');
      return;
    }
    rpc<{ content: string }>('project/read_file', { path: selectedScript })
      .then(result => {
        setScriptContent(result.content);
        setScriptSavedContent(result.content);
      })
      .catch(() => {
        const fallback = '// Unable to load this script.';
        setScriptContent(fallback);
        setScriptSavedContent(fallback);
      });
  }, [selectedScript]);

  useEffect(() => {
    setScriptLineRange(null);
    setScriptDiagnostics([]);
    setArtifactSelection(null);
    setArtifactQuestionOpen(false);
  }, [selectedScript, workspaceView]);

  useEffect(() => {
    const lowerPath = selectedScript?.toLowerCase() ?? '';
    const checkMethod = lowerPath.endsWith('.aster')
      ? 'project/check_script'
      : lowerPath.endsWith('.amdl')
        ? 'project/check_amdl'
        : null;
    if (!checkMethod) {
      setScriptDiagnostics([]);
      return;
    }
    const timer = window.setTimeout(() => {
      rpc<{ valid: boolean; diagnostics: TextAssetDiagnostic[] }>(checkMethod, {
        path: selectedScript,
        source: scriptContent,
      })
        .then(result => setScriptDiagnostics(result.diagnostics))
        .catch(() => setScriptDiagnostics([]));
    }, 350);
    return () => window.clearTimeout(timer);
  }, [scriptContent, selectedScript]);

  useEffect(() => {
    if (!questArtifact) {
      setOpenedQuestArtifact(null);
      return;
    }
    const context = questArtifactContext(questArtifact);
    setOpenedQuestArtifact(context);
    setWorkspaceView(context.surface);
    if (context.surface === 'scripts' && context.focusPath) {
      setSelectedScript(context.focusPath);
    }
  }, [questArtifact]);

  // Track selected position
  useEffect(() => {
    if (selectedId) {
      const obj = sceneTree.find(o => o.id === selectedId);
      setSelectedPosition(obj ? ([...obj.position] as Vec3) : null);
    } else {
      setSelectedPosition(null);
    }
  }, [selectedId, sceneTree]);

  useEffect(() => {
    if (!selectedId) {
      setSelectedEntityDetails(null);
      setSelectedEntityNameDraft('');
      return;
    }
    rpc<EntityDetails>('shell/get_entity', { id: selectedId })
      .then(entity => {
        setSelectedEntityDetails(entity);
        setSelectedEntityNameDraft(entity.name);
      })
      .catch(() => {
        setSelectedEntityDetails(null);
        setSelectedEntityNameDraft('');
      });
  }, [selectedId, sceneVersion]);

  // Fetch scene guides
  useEffect(() => {
    if (!shellState?.has_project) return;
    fetchSceneGuides()
      .then(res => setGuides(res.guides ?? []))
      .catch(() => setGuides([]));
  }, [sceneVersion, shellState?.has_project]);

  // Scene tree refresh — returns the new scene objects list
  const refreshSceneTree = useCallback(async (): Promise<SceneObject[]> => {
    try {
      const state = await rpc<ShellState>('shell/get_state');
      setShellState(state);
      const newVer = state.scene_version ?? 0;
      prevSceneVersionRef.current = newVer;
      setSceneVersion(newVer);
      const { objects } = await rpc<{ objects: SceneObject[] }>('shell/get_scene_tree');
      setSceneTree(objects);
      return objects;
    } catch { /* ignore */ }
    return [];
  }, []);

  const refreshConsoleEntries = useCallback(async () => {
    setConsoleBusy(true);
    try {
      const result = await rpc<{ entries: EditorConsoleEntry[] }>('console/get_entries');
      setConsoleEntries(result.entries);
    } finally {
      setConsoleBusy(false);
    }
  }, []);

  const clearConsoleEntries = useCallback(async () => {
    setConsoleBusy(true);
    try {
      await rpc('console/clear');
      setConsoleEntries([]);
    } finally {
      setConsoleBusy(false);
    }
  }, []);

  const runHealthCheck = useCallback(async () => {
    setHealthBusy(true);
    setHealthFixMessage(null);
    try {
      const result = await rpc<DiagnosticHealthReport>('diagnostics/run_health_check');
      setHealthReport(result);
      const allItems = result.groups.flatMap(group => group.items);
      setSelectedHealthItemId(current => (
        current && allItems.some(item => item.id === current)
          ? current
          : allItems[0]?.id ?? null
      ));
    } finally {
      setHealthBusy(false);
    }
  }, []);

  const applyHealthFix = useCallback(async (fix: DiagnosticFixAction) => {
    setHealthFixBusy(fix.id);
    setHealthFixMessage(null);
    try {
      const result = await rpc<DiagnosticFixResult>('diagnostics/apply_fix', { fix_id: fix.id });
      setHealthFixMessage(result.message || '修复已执行。');
      if (fix.id === 'clear_console') setConsoleEntries([]);
      await runHealthCheck();
      if (fix.id !== 'clear_console') await refreshConsoleEntries();
    } catch (error) {
      setHealthFixMessage(`修复失败：${String(error)}`);
    } finally {
      setHealthFixBusy(null);
    }
  }, [refreshConsoleEntries, runHealthCheck]);

  useEffect(() => {
    if (workspaceView !== 'diagnostics' || !shellState?.has_project) return;
    runHealthCheck();
    refreshConsoleEntries();
  }, [refreshConsoleEntries, runHealthCheck, shellState?.has_project, workspaceView]);

  const selectedBuildTarget = useMemo(
    () => BUILD_TARGETS.find(target => target.id === buildTarget) ?? BUILD_TARGETS[0],
    [buildTarget],
  );

  useEffect(() => {
    if (!selectedBuildTarget.formats.includes(buildFormat)) {
      setBuildFormat(selectedBuildTarget.formats[0]);
    }
  }, [buildFormat, selectedBuildTarget]);

  const applyBuildPreset = useCallback((preset: BuildPreset) => {
    setBuildTarget(preset.target);
    setBuildFormat(preset.format);
    setBuildChannel(preset.channel);
  }, []);

  const requestBuildPackage = useCallback(async () => {
    const unavailableReason = buildUnavailableReason(selectedBuildTarget, buildFormat);
    if (unavailableReason) {
      setBuildMessage(`当前不能打包。\n${unavailableReason}`);
      await rpc('console/push_entry', {
        level: 'warn',
        subsystem: 'build',
        message: unavailableReason,
      }).catch(() => {});
      await refreshConsoleEntries().catch(() => {});
      return;
    }

    setBuildBusy(true);
    setBuildMessage(null);
    try {
      const result = await rpc<BuildPackageResult>('project/package', {
        target: buildTarget,
        format: buildFormat,
        channel: buildChannel,
        optimize_assets: buildOptimizeAssets,
        include_debug_symbols: buildIncludeDebugSymbols,
      });
      const message = [
        `已打包 ${result.project}`,
        `目标：${selectedBuildTarget.label}`,
        `格式：${formatBuildFormat(buildFormat)}`,
        `通道：${formatBuildChannel(buildChannel)}`,
        `输出：${result.path}`,
        `运行时：${result.binary}`,
        `启动器：${result.launcher}`,
      ].join('\n');
      setBuildMessage(message);
      await refreshConsoleEntries().catch(() => {});
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setBuildMessage(`打包失败。\n${message}`);
      await rpc('console/push_entry', {
        level: 'error',
        subsystem: 'build',
        message: `${selectedBuildTarget.label}/${formatBuildFormat(buildFormat)} 打包失败：${message}`,
      }).catch(() => {});
      await refreshConsoleEntries().catch(() => {});
    } finally {
      setBuildBusy(false);
    }
  }, [
    buildChannel,
    buildFormat,
    buildIncludeDebugSymbols,
    buildOptimizeAssets,
    buildTarget,
    refreshConsoleEntries,
    selectedBuildTarget,
  ]);

  const reloadAsset = useCallback(async (path: string) => {
    setAssetsBusy(true);
    try {
      await rpc('project/reimport_asset', { path });
      await refreshProjectAssets();
      await refreshConsoleEntries().catch(() => {});
    } finally {
      setAssetsBusy(false);
    }
  }, [refreshConsoleEntries, refreshProjectAssets]);

  const revealAssetReferences = useCallback(async (asset: ProjectAssetMeta, event: React.MouseEvent) => {
    setAssetsBusy(true);
    try {
      const result = await rpc<{ references: AssetReferenceRow[] }>('project/list_asset_references', {
        path: asset.source_path,
      });
      const references = result.references.length > 0
        ? result.references.map(ref => `${ref.kind}: ${ref.label} - ${ref.detail}`).join('\n')
        : 'No references found.';
      setArtifactSelection({
        kind: 'document',
        label: `${asset.source_path} references`,
        context: `资源：${asset.source_path}\n类型：${assetKindLabel(asset.kind)}\nGUID：${asset.guid}\n\n${references}`,
        x: event.clientX,
        y: event.clientY,
      });
      setArtifactQuestionOpen(false);
    } finally {
      setAssetsBusy(false);
    }
  }, []);

  const createProjectAsset = useCallback(async (kind: ProjectAssetCreateKind) => {
    const defaultName = `new_${kind}`;
    const rawName = window.prompt(`Create ${kind}`, defaultName);
    const name = rawName?.trim();
    if (!name) return;

    setAssetsBusy(true);
    try {
      const method = kind === 'script' ? 'project/create_script' : `project/create_${kind}`;
      const params = kind === 'script' ? { name, backend: 'rhai' } : { name };
      const result = await rpc<{ path: string }>(method, params);
      await refreshProjectAssets();
      await refreshConsoleEntries().catch(() => {});
      if (kind === 'script') {
        setSelectedScript(result.path);
        setWorkspaceView('scripts');
      }
    } finally {
      setAssetsBusy(false);
    }
  }, [refreshConsoleEntries, refreshProjectAssets]);

  useEffect(() => {
    if (!shellState?.has_project) return;
    refreshConsoleEntries().catch(() => {});
  }, [refreshConsoleEntries, sceneVersion, shellState?.has_project]);

  // Focus camera on a given position with smooth lerp
  const focusOnPosition = useCallback((target: [number, number, number]) => {
    const cam = cameraRef.current;
    const startX = cam.targetX;
    const startY = cam.targetY;
    const startZ = cam.targetZ;
    const [endX, endY, endZ] = target;
    let t = 0;
    const animate = () => {
      t += 0.08;
      if (t >= 1) {
        cam.targetX = endX;
        cam.targetY = endY;
        cam.targetZ = endZ;
        handleCameraChange();
        return;
      }
      const ease = 1 - Math.pow(1 - t, 3); // ease-out cubic
      cam.targetX = startX + (endX - startX) * ease;
      cam.targetY = startY + (endY - startY) * ease;
      cam.targetZ = startZ + (endZ - startZ) * ease;
      handleCameraChange();
      requestAnimationFrame(animate);
    };
    requestAnimationFrame(animate);
  }, [handleCameraChange]);

  // Handle scene changes from AI panel — detect new entities and focus
  const sceneTreeRef = useRef(sceneTree);
  sceneTreeRef.current = sceneTree;

  const handleAiSceneChanged = useCallback(async () => {
    const prevIds = new Set(sceneTreeRef.current.map(o => o.id));
    const newObjects = await refreshSceneTree();
    // Find newly created objects
    const created = newObjects.filter(o => !prevIds.has(o.id));
    if (created.length > 0) {
      // Focus on the first new object and select it
      const first = created[0];
      focusOnPosition(first.position);
      setSelectedId(first.id);
      rpc('shell/select_entity', { id: first.id });
    }
  }, [refreshSceneTree, focusOnPosition]);

  const selectSceneObject = useCallback((id: string | null) => {
    setSelectedId(id);
    rpc('shell/select_entity', id ? { id } : {}).catch(() => {});
    const object = id ? sceneTree.find(item => item.id === id) : null;
    if (object) focusOnPosition(object.position);
  }, [focusOnPosition, sceneTree]);

  const createSceneObject = useCallback(async (name = 'New Object') => {
    const created = await rpc<SceneObject>('shell/create_object', {
      name,
      parent_id: selectedId ?? undefined,
    });
    await refreshSceneTree();
    selectSceneObject(created.id);
  }, [refreshSceneTree, selectSceneObject, selectedId]);

  const renameSelectedObject = useCallback(async () => {
    if (!selectedId || !selectedEntityNameDraft.trim()) return;
    await rpc('shell/rename_object', { id: selectedId, name: selectedEntityNameDraft.trim() });
    await refreshSceneTree();
  }, [refreshSceneTree, selectedEntityNameDraft, selectedId]);

  const duplicateSelectedObject = useCallback(async () => {
    if (!selectedId) return;
    const duplicated = await rpc<SceneObject>('shell/duplicate_object', { id: selectedId });
    await refreshSceneTree();
    selectSceneObject(duplicated.id);
  }, [refreshSceneTree, selectSceneObject, selectedId]);

  const deleteSelectedObject = useCallback(async () => {
    if (!selectedId) return;
    await rpc('shell/delete_object', { id: selectedId });
    setSelectedId(null);
    setSelectedEntityDetails(null);
    await refreshSceneTree();
  }, [refreshSceneTree, selectedId]);

  const reparentSelectedObject = useCallback(async (parentId: string) => {
    if (!selectedId) return;
    await rpc('shell/reparent_object', {
      id: selectedId,
      parent_id: parentId || undefined,
    });
    await refreshSceneTree();
  }, [refreshSceneTree, selectedId]);

  const updateSelectedTransform = useCallback(async (
    field: 'position' | 'rotation' | 'scale',
    index: number,
    rawValue: string,
  ) => {
    if (!selectedId || !selectedEntityDetails) return;
    const value = Number(rawValue);
    if (!Number.isFinite(value)) return;
    const next = [...selectedEntityDetails.transform[field]];
    next[index] = value;
    await rpc('shell/update_transform', { id: selectedId, [field]: next });
    await refreshSceneTree();
  }, [refreshSceneTree, selectedEntityDetails, selectedId]);

  const nudgeSelectedTransform = useCallback(async (
    field: 'position' | 'rotation' | 'scale',
    index: number,
    event: React.WheelEvent<HTMLInputElement>,
  ) => {
    if (!selectedId || !selectedEntityDetails) return;
    event.preventDefault();
    const nextValue = nudgeNumericInput(event.currentTarget, numericWheelDelta(event));
    if (nextValue === null) return;
    const next = [...selectedEntityDetails.transform[field]];
    next[index] = nextValue;
    await rpc('shell/update_transform', { id: selectedId, [field]: next });
    await refreshSceneTree();
  }, [refreshSceneTree, selectedEntityDetails, selectedId]);

  const addSelectedComponent = useCallback(async () => {
    if (!selectedId || !addComponentType) return;
    await rpc('shell/add_component', { id: selectedId, component_type: addComponentType });
    await refreshSceneTree();
  }, [addComponentType, refreshSceneTree, selectedId]);

  const removeSelectedComponent = useCallback(async (componentType: string) => {
    if (!selectedId) return;
    await rpc('shell/remove_component', { id: selectedId, component_type: componentType });
    await refreshSceneTree();
  }, [refreshSceneTree, selectedId]);

  const updateSelectedComponentField = useCallback(async (
    componentType: string,
    fieldName: string,
    value: unknown,
  ) => {
    if (!selectedId) return;
    await rpc('shell/update_component', {
      id: selectedId,
      component_type: componentType,
      data: { [fieldName]: value },
    });
    await refreshSceneTree();
  }, [refreshSceneTree, selectedId]);

  // Quick actions from AI panel
  const handleQuickAction = useCallback(async (action: string) => {
    switch (action) {
      case 'save':
        await rpc('shell/save_scene').catch(() => {});
        await refreshSceneTree();
        break;
      case 'undo':
        await rpc('shell/undo').catch(() => {});
        await refreshSceneTree();
        break;
      case 'play':
        openGameView();
        break;
    }
  }, [refreshSceneTree]);

  // Close project handler
  const handleClose = useCallback(() => {
    if (shellState?.scene_dirty) {
      setShowCloseDialog(true);
    } else {
      onCloseProject();
    }
  }, [onCloseProject, shellState?.scene_dirty]);

  // Keyboard shortcuts (minimal set)
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLElement) {
        const tag = e.target.tagName;
        if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;
        if (e.target.isContentEditable) return;
      }
      if ((e.ctrlKey || e.metaKey) && !e.shiftKey && e.key === 's') {
        e.preventDefault();
        rpc('shell/save_scene').then(() => refreshSceneTree());
      }
      if ((e.ctrlKey || e.metaKey) && !e.shiftKey && e.key === 'z') {
        e.preventDefault();
        rpc('shell/undo').then(() => refreshSceneTree());
      }
      if ((e.ctrlKey || e.metaKey) && e.key === 'y') {
        e.preventDefault();
        rpc('shell/redo').then(() => refreshSceneTree());
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [refreshSceneTree]);

  // Resize handle for AI panel
  const handleResizeDown = useDragHandle('horizontal', (delta) => {
    setAiPanelWidth((w) => Math.max(360, Math.min(w - delta, 460)));
  });

  // Viewport click-to-pick
  const handleViewportClick = useCallback((e: React.MouseEvent) => {
    // Only left-click, not on buttons or gizmo elements
    if (e.button !== 0) return;
    const target = e.target as HTMLElement;
    if (target.closest('.transform-gizmo') || target.closest('.scene-guides') || target.closest('.orientation-gizmo')) return;

    const container = e.currentTarget as HTMLElement;
    const rect = container.getBoundingClientRect();
    const clickX = e.clientX - rect.left;
    const clickY = e.clientY - rect.top;
    const vpWidth = rect.width;
    const vpHeight = rect.height;

    const hitId = pickEntityAtScreen(
      clickX, clickY, vpWidth, vpHeight,
      sceneTree, cameraRef.current,
      viewMode,
    );

    if (hitId) {
      const object = sceneTree.find(item => item.id === hitId);
      selectSceneObject(hitId);
      if (object) {
        setArtifactSelection({
          kind: 'model',
          label: object.name,
          context: `Scene model: ${object.name}\nEntity ID: ${object.id}\nTag: ${object.tag || 'Untagged'}\nPosition: ${object.position.join(', ')}`,
          x: e.clientX,
          y: e.clientY,
        });
        setArtifactQuestionOpen(false);
      }
    } else {
      selectSceneObject(null);
      setArtifactSelection(null);
    }
  }, [sceneTree, selectSceneObject, viewMode]);

  const selectDocumentText = useCallback((event: React.MouseEvent<HTMLElement>) => {
    const selection = window.getSelection();
    const text = selection?.toString().trim();
    if (!text || text.length < 2) return;
    setArtifactSelection({
      kind: 'document',
      label: text.length > 48 ? `${text.slice(0, 48)}…` : text,
      context: `Spec excerpt:\n${text}`,
      x: event.clientX,
      y: event.clientY,
    });
    setArtifactQuestionOpen(false);
  }, []);

  const selectScriptLine = useCallback((line: number, extend: boolean, event: React.MouseEvent) => {
    const nextRange: [number, number] = extend && scriptLineRange
      ? [Math.min(scriptLineRange[0], line), Math.max(scriptLineRange[1], line)]
      : [line, line];
    setScriptLineRange(nextRange);
    const lines = scriptContent.split('\n').slice(nextRange[0] - 1, nextRange[1]);
    setArtifactSelection({
      kind: 'code',
      label: `${selectedScript || 'script'}:${nextRange[0]}${nextRange[1] === nextRange[0] ? '' : `-${nextRange[1]}`}`,
      context: `Script: ${selectedScript}\nLines ${nextRange[0]}-${nextRange[1]}:\n${lines.join('\n')}`,
      x: event.clientX,
      y: event.clientY,
    });
    setArtifactQuestionOpen(false);
  }, [scriptContent, scriptLineRange, selectedScript]);

  const submitArtifactQuestion = useCallback(() => {
    if (!artifactSelection || !artifactQuestion.trim()) return;
    setContextualRequest({
      id: Date.now(),
      prompt: `${artifactQuestion.trim()}\n\n[Selected context]\n${artifactSelection.context}`,
    });
    setArtifactQuestion('');
    setArtifactQuestionOpen(false);
  }, [artifactQuestion, artifactSelection]);

  const saveSelectedScript = useCallback(async () => {
    if (!selectedScript) return;
    setScriptSaving(true);
    try {
      const lowerPath = selectedScript.toLowerCase();
      const checkMethod = lowerPath.endsWith('.aster')
        ? 'project/check_script'
        : lowerPath.endsWith('.amdl')
          ? 'project/check_amdl'
          : null;
      if (checkMethod) {
        const validation = await rpc<{ valid: boolean; diagnostics: TextAssetDiagnostic[] }>(
          checkMethod,
          { path: selectedScript, source: scriptContent },
        );
        setScriptDiagnostics(validation.diagnostics);
        if (!validation.valid) return;
      }
      await rpc('project/write_file', { path: selectedScript, content: scriptContent });
      setScriptSavedContent(scriptContent);
      await refreshConsoleEntries().catch(() => {});
    } finally {
      setScriptSaving(false);
    }
  }, [refreshConsoleEntries, scriptContent, selectedScript]);

  const createPresetObject = useCallback(async (
    name: string,
    componentType?: string,
  ) => {
    const created = await rpc<SceneObject>('shell/create_object', {
      name,
      parent_id: selectedId ?? undefined,
    });
    if (componentType) {
      await rpc('shell/add_component', { id: created.id, component_type: componentType });
    }
    await refreshSceneTree();
    selectSceneObject(created.id);
  }, [refreshSceneTree, selectSceneObject, selectedId]);

  const createBehaviorObject = useCallback(async () => {
    const name = `behavior_${Date.now()}`;
    const result = await rpc<{ path: string }>('project/create_script', {
      name,
      backend: 'rhai',
    });
    const created = await rpc<SceneObject>('shell/create_object', {
      name: 'Behavior Object',
      parent_id: selectedId ?? undefined,
    });
    await rpc('shell/add_component', { id: created.id, component_type: 'Script' });
    await rpc('shell/update_component', {
      id: created.id,
      component_type: 'Script',
      data: { script: result.path },
    });
    await refreshProjectAssets();
    await refreshSceneTree();
    selectSceneObject(created.id);
    setSelectedScript(result.path);
    setWorkspaceView('scripts');
  }, [refreshProjectAssets, refreshSceneTree, selectSceneObject, selectedId]);

  // Derive selected entity name
  const selectedEntityName = selectedId
    ? sceneTree.find(o => o.id === selectedId)?.name ?? null
    : null;
  const selectedObject = selectedId
    ? sceneTree.find(o => o.id === selectedId) ?? null
    : null;
  const scriptDirty = scriptContent !== scriptSavedContent;
  const unifiedAssets = useMemo(() => {
    const seen = new Set<string>();
    const rows: ProjectAssetMeta[] = [];
    for (const asset of assets) {
      const key = asset.source_path || asset.guid;
      if (!key || seen.has(key)) continue;
      seen.add(key);
      rows.push(asset);
    }
    for (const path of scripts) {
      if (seen.has(path)) continue;
      seen.add(path);
      rows.push({
        guid: `script:${path}`,
        source_path: path,
        kind: 'script',
        importer: path.toLowerCase().endsWith('.rhai') ? 'rhai' : 'script',
      });
    }
    return rows;
  }, [assets, scripts]);
  const scriptBindingOptions = useMemo(() => scripts.filter(path => isScriptPath(path)), [scripts]);
  const assetKinds = useMemo(() => {
    const kinds = Array.from(new Set(unifiedAssets.map(asset => asset.kind || 'unknown'))).sort();
    return ['全部', ...kinds];
  }, [unifiedAssets]);
  const filteredAssets = assetKindFilter === '全部'
    ? unifiedAssets
    : unifiedAssets.filter(asset => (asset.kind || 'unknown') === assetKindFilter);
  const scriptAssetCount = unifiedAssets.filter(asset => isScriptPath(asset.source_path) || /script/i.test(asset.kind)).length;
  const materialAssetCount = unifiedAssets.filter(asset => /material/i.test(`${asset.kind} ${asset.source_path}`)).length;
  const sceneAssetCount = unifiedAssets.filter(asset => /scene/i.test(`${asset.kind} ${asset.source_path}`)).length;
  const selectedBuildCanRun = canRunBuild(selectedBuildTarget, buildFormat);
  const selectedBuildUnavailableReason = buildUnavailableReason(selectedBuildTarget, buildFormat);
  const diagnosticCounts = useMemo(() => consoleEntries.reduce(
    (counts, entry) => {
      const level = normalizeDiagnosticLevel(entry.level);
      counts.all += 1;
      counts[level] += 1;
      return counts;
    },
    { all: 0, error: 0, warn: 0, info: 0, debug: 0 } as Record<DiagnosticLevelFilter, number>,
  ), [consoleEntries]);
  const diagnosticFilters = useMemo(() => ([
    { id: 'all' as const, label: '全部', count: diagnosticCounts.all },
    { id: 'error' as const, label: '错误', count: diagnosticCounts.error },
    { id: 'warn' as const, label: '警告', count: diagnosticCounts.warn },
    { id: 'info' as const, label: '信息', count: diagnosticCounts.info },
    { id: 'debug' as const, label: '调试', count: diagnosticCounts.debug },
  ]), [diagnosticCounts]);
  const filteredConsoleEntries = diagnosticFilter === 'all'
    ? consoleEntries
    : consoleEntries.filter(entry => normalizeDiagnosticLevel(entry.level) === diagnosticFilter);
  const hasDiagnostics = consoleEntries.length > 0;
  const healthItems = healthReport?.groups.flatMap(group => group.items) ?? [];
  const selectedHealthItem = healthItems.find(item => item.id === selectedHealthItemId) ?? healthItems[0] ?? null;
  const visibleWorkspaceTabs = ([
    ['game', '场景', <IconView key="game" />] as const,
    ['assets', '资源', <IconFile key="assets" />] as const,
    ['scripts', '行为', <IconCode key="scripts" />] as const,
    ['build', '构建', <IconPackage key="build" />] as const,
    ['diagnostics', '诊断', <IconAlertCircle key="diagnostics" />] as const,
  ]);
  const pendingAiDecisionCount = aiWorkspace?.plan?.operations.filter(operation => (
    operation.permission_kind !== 'read'
    && !aiWorkspace?.approved.has(operation.index)
    && !aiWorkspace?.denied.has(operation.index)
  )).length ?? 0;
  const aiTaskOperations = aiWorkspace?.plan?.operations ?? [];
  const aiTaskReadCount = aiTaskOperations.filter(operation => operation.permission_kind === 'read').length;
  const aiTaskWriteCount = aiTaskOperations.filter(operation => operation.permission_kind === 'write').length;
  const aiTaskCommandCount = aiTaskOperations.filter(operation => operation.permission_kind === 'command').length;
  const aiTaskApprovedCount = aiTaskOperations.filter(operation => operation.permission_kind !== 'read' && aiWorkspace?.approved.has(operation.index)).length;
  const aiTaskDeniedCount = aiTaskOperations.filter(operation => aiWorkspace?.denied.has(operation.index)).length;
  const aiTaskAutoCount = aiTaskOperations.filter(operation => operation.permission_kind === 'read').length;
  const aiTaskDecidedCount = aiTaskApprovedCount + aiTaskDeniedCount + aiTaskAutoCount;
  const aiTaskProgressPercent = aiTaskOperations.length > 0
    ? Math.round((aiTaskDecidedCount / aiTaskOperations.length) * 100)
    : 0;
  const aiTaskPendingCount = aiTaskOperations.filter(operation => (
    operation.permission_kind !== 'read'
    && !aiWorkspace?.approved.has(operation.index)
    && !aiWorkspace?.denied.has(operation.index)
  )).length;
  const aiTaskCanApply = aiTaskApprovedCount > 0 && aiTaskPendingCount === 0 && aiWorkspace?.status === 'ready';
  const aiStatusText = aiWorkspace?.status === 'thinking'
    ? '正在规划'
    : aiWorkspace?.status === 'executing'
      ? '正在应用'
      : pendingAiDecisionCount > 0
        ? '等待确认'
        : aiWorkspace?.status === 'complete'
          ? '已完成'
          : '待命';
  const aiStatusClass = cx(
    aiShellClass.status,
    (aiWorkspace?.status === 'thinking' || aiWorkspace?.status === 'executing') && aiShellClass.statusBusy,
    (aiWorkspace?.status === 'complete' || pendingAiDecisionCount === 0) && aiShellClass.statusReady,
  );
  const railItems: Array<{
    key: string;
    label: string;
    icon: React.ReactNode;
    active: boolean;
    action: () => void;
    badge?: number;
  }> = [
    { key: 'workbench', label: '工作台', icon: <IconSparkles />, active: workspaceView === 'game' && aiPanelOpen, action: () => { setWorkspaceView('game'); setAiPanelOpen(true); } },
    { key: 'scene', label: '场景', icon: <IconView />, active: workspaceView === 'game', action: () => setWorkspaceView('game') },
    { key: 'assets', label: '资源', icon: <IconFile />, active: workspaceView === 'assets', action: () => setWorkspaceView('assets'), badge: assets.length || undefined },
    { key: 'behavior', label: '行为', icon: <IconCode />, active: workspaceView === 'scripts', action: () => setWorkspaceView('scripts'), badge: scripts.length || undefined },
    { key: 'tasks', label: '改动', icon: <IconCheck />, active: workspaceView === 'tasks', action: () => { setWorkspaceView('tasks'); setAiPanelOpen(true); }, badge: pendingAiDecisionCount || undefined },
    { key: 'build', label: '构建', icon: <IconPackage />, active: workspaceView === 'build', action: () => setWorkspaceView('build') },
    { key: 'diagnostics', label: '诊断', icon: <IconAlertCircle />, active: workspaceView === 'diagnostics', action: () => setWorkspaceView('diagnostics'), badge: consoleEntries.length || undefined },
    { key: 'advanced', label: '任务', icon: <IconProjects />, active: false, action: () => { onOpenQuest?.(); } },
  ];

  useEffect(() => {
    const hasAiPlan = Boolean(aiWorkspace?.plan);
    if (hasAiPlan && !previousAiPlanRef.current) {
      setAiPanelOpen(true);
    }
    previousAiPlanRef.current = hasAiPlan;
  }, [aiWorkspace?.plan]);

  useEffect(() => {
    const hasCompletion = Boolean(aiWorkspace?.completedBundle);
    if (hasCompletion && !previousAiCompletionRef.current) {
      setAiPanelOpen(true);
    }
    previousAiCompletionRef.current = hasCompletion;
  }, [aiWorkspace?.completedBundle]);

  useEffect(() => {
    const visible = new Set<WorkspaceView>([
      ...visibleWorkspaceTabs.map(([view]) => view),
      ...(aiWorkspace?.plan || aiWorkspace?.completedBundle ? ['tasks' as WorkspaceView] : []),
    ]);
    if (!visible.has(workspaceView)) {
      setWorkspaceView('game');
    }
  }, [aiWorkspace?.plan, hasDiagnostics, visibleWorkspaceTabs, workspaceView]);

  // ── Render ──

  if (!shellState) {
    return <div className={shellClass.loading}>{t('loading_editor')}</div>;
  }

  return (
    <div className={shellClass.root}>
      {/* Editor toolbar */}
      <div className={shellClass.toolbar}>
        <div className={shellClass.brand}>
          <div className={shellClass.brandMark}>A</div>
          <div className={shellClass.toolbarProject}>
            <span className={shellClass.toolbarProjectKicker}>Aster 游戏编辑器</span>
            <span className={shellClass.toolbarProjectName}>{shellState.project_name || t('editor_untitled')} / Level_01</span>
          </div>
        </div>

        <div className={shellClass.modeTabs} aria-label="编辑模式">
          <button className={cx(shellClass.modeTab, workspaceView === 'game' && shellClass.modeTabActive)} onClick={() => setWorkspaceView('game')}>编辑</button>
          <button className={shellClass.modeTab} onClick={openGameView}>预览</button>
          <button className={cx(shellClass.modeTab, workspaceView === 'build' && shellClass.modeTabActive)} onClick={() => setWorkspaceView('build')}>构建</button>
        </div>

        <div className={shellClass.toolbarActions}>
          <span className={shellClass.toolbarStatus}>{shellState.scene_dirty ? '未保存' : '已保存'}</span>
          <button
            className={toolButtonClass({ size: 'toolbar' })}
            onClick={() => rpc('shell/save_scene').then(() => refreshSceneTree())}
            disabled={!shellState.scene_dirty}
            title="保存场景"
          >
            <IconSave /> <span>保存</span>
          </button>
          <button className={toolButtonClass({ variant: 'play', size: 'toolbar' })} onClick={openGameView} title={t('editor_open_game_view')}><IconPlay /> <span>运行预览</span></button>
          <button className={toolButtonClass({ size: 'toolbar' })} onClick={() => setWorkspaceView('build')} title="导出与构建"><IconPackage /> <span>导出</span></button>
          <span className={shellClass.toolbarDivider} />
          <button
            className={toolButtonClass({ size: 'icon' })}
            onClick={() => rpc('shell/undo').then(() => refreshSceneTree())}
            disabled={!shellState.can_undo}
            title="撤销"
          >
            <IconUndo />
          </button>
          <button
            className={toolButtonClass({ size: 'icon' })}
            onClick={() => rpc('shell/redo').then(() => refreshSceneTree())}
            disabled={!shellState.can_redo}
            title="重做"
          >
            <IconRedo />
          </button>
          <button className={toolButtonClass({ size: 'icon' })} onClick={handleClose} title={t('editor_close')}><IconX /></button>
        </div>
      </div>

      {/* Main body: viewport + AI panel */}
      <div className={shellClass.body}>
        <aside className={appRailClass.root} aria-label="主导航">
          <nav className={appRailClass.nav}>
            {railItems.map(item => (
              <button
                key={item.key}
                className={cx(appRailClass.item, item.active && appRailClass.itemActive)}
                onClick={item.action}
                title={item.label}
                aria-current={item.active ? 'page' : undefined}
              >
                {item.icon}
                <span>{item.label}</span>
                {item.badge ? <b className={appRailClass.badge}>{item.badge}</b> : null}
              </button>
            ))}
          </nav>
          <div className={appRailClass.bottom}>
            <button className={appRailClass.item} onClick={onOpenSettings} title="设置">
              <IconSun />
              <span>设置</span>
            </button>
          </div>
        </aside>

        <main className={workspaceClass.root}>
          {questArtifact && (
            <div className={questBannerClass.root}>
              <IconProjects className={questBannerClass.icon} />
              <div className={questBannerClass.content}>
                <span className={questBannerClass.kicker}>来自任务模式</span>
                <strong className={questBannerClass.title}>{questArtifact.questTitle}</strong>
                <small className={questBannerClass.meta}>{questArtifact.kind.replaceAll('_', ' ')} · {questArtifact.label}</small>
              </div>
              <button className={questBannerClass.button} onClick={onOpenQuest}><IconProjects /> 返回</button>
              <button className={questBannerClass.iconButton} onClick={onDismissQuestArtifact} title="关闭"><IconX /></button>
            </div>
          )}
          <nav className={workspaceClass.tabs} role="tablist" aria-label="编辑器页面">
            {visibleWorkspaceTabs.map(([view, label, icon]) => (
              <button
                key={view}
                className={cx(workspaceClass.tab, workspaceView === view && workspaceClass.tabActive)}
                onClick={() => setWorkspaceView(view)}
                role="tab"
                aria-selected={workspaceView === view}
              >
                {icon}<span>{label}</span>
                {view === 'assets' && assets.length > 0 && <b className={workspaceClass.tabBadge}>{assets.length}</b>}
                {view === 'scripts' && scripts.length > 0 && <b className={workspaceClass.tabBadge}>{scripts.length}</b>}
              </button>
            ))}
            {aiWorkspace?.plan && (
              <button
                className={cx(workspaceClass.tab, workspaceView === 'tasks' && workspaceClass.tabActive)}
                onClick={() => {
                  setWorkspaceView('tasks');
                  setAiPanelOpen(true);
                }}
                role="tab"
                aria-selected={workspaceView === 'tasks'}
              >
                <IconCheck /><span>查看改动</span>
                <b className={workspaceClass.tabBadge}>{aiWorkspace.plan.operations.length}</b>
              </button>
            )}
            {hasDiagnostics && (
              <button
                className={cx(workspaceClass.tab, workspaceView === 'diagnostics' && workspaceClass.tabActive)}
                onClick={() => setWorkspaceView('diagnostics')}
                role="tab"
                aria-selected={workspaceView === 'diagnostics'}
              >
                <IconAlertCircle /><span>查看诊断</span>
                {consoleEntries.length > 0 && <b className={workspaceClass.tabBadge}>{consoleEntries.length}</b>}
              </button>
            )}
          </nav>

          <section className={cx(workspaceClass.view, workspaceView === 'game' && workspaceClass.viewGame)}>
            {workspaceView === 'prd' && openedQuestArtifact && <article className={prdClass.document} onMouseUp={selectDocumentText}>
              <header className={prdClass.header}><span className={prdClass.kicker}>{t('prd_header')}</span><strong className={prdClass.title}>{shellState.project_name || t('prd_untitled_game')}</strong><p className={prdClass.description}>{t('prd_brief_desc')}</p></header>
              <section className={prdClass.section}><h2 className={prdClass.sectionTitle}>{t('prd_vision')}</h2><p className={prdClass.bodyText}>{t('prd_vision_text').replace('{scene_count}', String(sceneTree.length)).replace('{script_count}', String(scripts.length))}</p></section>
              <section className={prdClass.section}><h2 className={prdClass.sectionTitle}>{t('prd_current_scope')}</h2><div className={prdClass.grid}><div className={prdClass.gridCard}><span className={prdClass.gridLabel}>{t('prd_scope_player_exp')}</span><strong className={prdClass.gridValue}>{t('prd_scope_playable')}</strong></div><div className={prdClass.gridCard}><span className={prdClass.gridLabel}>{t('prd_scope_world')}</span><strong className={prdClass.gridValue}>{sceneTree.length} {t('prd_scope_authored')}</strong></div><div className={prdClass.gridCard}><span className={prdClass.gridLabel}>{t('prd_scope_automation')}</span><strong className={prdClass.gridValue}>{t('prd_scope_review')}</strong></div><div className={prdClass.gridCard}><span className={prdClass.gridLabel}>{t('prd_scope_delivery')}</span><strong className={prdClass.gridValue}>{t('prd_scope_verification')}</strong></div></div></section>
              <section className={prdClass.section}><h2 className={prdClass.sectionTitle}>{t('prd_acceptance')}</h2><ul className={prdClass.list}><li className={prdClass.bodyText}>{t('prd_criteria_1')}</li><li className={prdClass.bodyText}>{t('prd_criteria_2')}</li><li className={prdClass.bodyText}>{t('prd_criteria_3')}</li><li className={prdClass.bodyText}>{t('prd_criteria_4')}</li></ul></section>
            </article>}

            {workspaceView === 'tasks' && <div className={taskClass.board} aria-label="AI 工作记录">
              <header className={taskClass.header}>
                <div className={taskClass.headerText}>
                  <span className={taskClass.kicker}>AI 工作记录</span>
                  <strong className={taskClass.title}>{aiWorkspace?.plan ? '改动需要确认' : aiWorkspace?.completedBundle ? 'AI 已完成应用' : openedQuestArtifact ? openedQuestArtifact.title : '等待 AI 计划'}</strong>
                  <small className={taskClass.meta}>项目 {shellState.project_name} · 场景内 {sceneTree.length} 个对象 · 右侧 AI 提案会在这里变成可确认的步骤。</small>
                </div>
                <div className={taskClass.headerActions}>
                  <button className={surfaceClass.button} onClick={() => { setWorkspaceView('game'); setAiPanelOpen(true); }}>
                    <IconSparkles /> 回到工作台
                  </button>
                  {aiWorkspace?.plan && (
                    <button className={surfaceClass.button} onClick={aiWorkspace.discardProposal}>
                      <IconX /> 放弃计划
                    </button>
                  )}
                </div>
              </header>

              <div className={taskClass.layout}>
                <section className={taskClass.main}>
                  {aiWorkspace?.plan ? (
                    <>
                      <div className={taskClass.summary}>
                        <div className={taskClass.summaryCard}><span className={taskClass.summaryLabel}>总步骤</span><b className={taskClass.summaryValue}>{aiTaskOperations.length}</b></div>
                        <div className={taskClass.summaryCard}><span className={taskClass.summaryLabel}>只读检查</span><b className={taskClass.summaryValue}>{aiTaskReadCount}</b></div>
                        <div className={taskClass.summaryCard}><span className={taskClass.summaryLabel}>文件改动</span><b className={taskClass.summaryValue}>{aiTaskWriteCount}</b></div>
                        <div className={taskClass.summaryCard}><span className={taskClass.summaryLabel}>命令</span><b className={taskClass.summaryValue}>{aiTaskCommandCount}</b></div>
                      </div>

                      <div className={taskClass.progress}>
                        <div className={taskClass.progressTrack}>
                          <div className={taskClass.progressFill} style={{ width: `${aiTaskProgressPercent}%` }} />
                        </div>
                        <div className={taskClass.progressText}>
                          <span>{`已确认 ${aiTaskDecidedCount} / ${aiTaskOperations.length} 步`}</span>
                          <span>{aiTaskPendingCount > 0 ? `${aiTaskPendingCount} 步等待你确认` : '没有待确认步骤'}</span>
                        </div>
                      </div>
                    </>
                  ) : !aiWorkspace?.completedBundle && (
                    <div className={taskClass.progress}>
                      <div className={taskClass.progressTrack}>
                        <div className={taskClass.progressFill} style={{ width: '0%' }} />
                      </div>
                      <div className={taskClass.progressText}>
                        <span>还没有 AI 提出的操作计划</span>
                        <span>先在右侧告诉 AI 想做什么</span>
                      </div>
                    </div>
                  )}

                  {aiWorkspace?.completedBundle && !aiWorkspace.plan && (
                    <section className={taskClass.completedCard}>
                      <div className={taskClass.completedHeader}>
                        <IconCheck />
                        <div>
                          <strong className={taskClass.completedTitle}>AI 已完成本次应用</strong>
                          <span className={taskClass.completedText}>{aiWorkspace.completedBundle.summary}</span>
                        </div>
                      </div>
                      <div className={taskClass.completedMetrics}>
                        <span className={taskClass.completedMetric}><b>{aiWorkspace.completedBundle.operationsPerformed}</b>已执行操作</span>
                        <span className={taskClass.completedMetric}><b>{aiWorkspace.completedBundle.traceEntries.length}</b>工具记录</span>
                        <span className={taskClass.completedMetric}><b>{aiWorkspace.completedBundle.consoleEntries.length}</b>控制台输出</span>
                      </div>
                    </section>
                  )}

                  {openedQuestArtifact && (
                    <section className={taskClass.artifactCard}>
                      <div>
                        <span className={taskClass.artifactKicker}>{questArtifact?.kind.replaceAll('_', ' ')}</span>
                        <strong className={taskClass.artifactTitle}>{openedQuestArtifact.title}</strong>
                        <p className={taskClass.artifactDescription}>{openedQuestArtifact.description}</p>
                        {openedQuestArtifact.focusPath && <small className={taskClass.artifactPath}>{openedQuestArtifact.focusPath}</small>}
                      </div>
                      <div className={taskClass.artifactActions}>
                        {openedQuestArtifact.surface !== 'tasks' && (
                          <button className={taskClass.artifactButton} onClick={() => setWorkspaceView(openedQuestArtifact.surface)}>
                            <IconFile /> 打开相关面板
                          </button>
                        )}
                        <button
                          className={taskClass.artifactButton}
                          onClick={() => {
                            setAiPanelOpen(true);
                            setContextualRequest({
                              id: Date.now(),
                              prompt: `请检查这个任务产物，并建议下一步本地编辑器检查。\n\n${openedQuestArtifact.title}\n${openedQuestArtifact.description}\n${openedQuestArtifact.focusPath ?? ''}`,
                            });
                          }}
                        >
                          <IconSparkles /> 问 AI
                        </button>
                        <button className={taskClass.artifactButton} onClick={onOpenQuest}><IconProjects /> 返回任务</button>
                      </div>
                    </section>
                  )}

                  <section className={taskClass.operations}>
                    <div className={taskClass.operationsTitle}>
                      <span>{t('task_proposed_ops')}</span>
                      <small className={taskClass.operationsHint}>只读会自动允许，写入和命令必须由你确认。</small>
                    </div>
                    {!aiWorkspace?.plan ? (
                      <div className={productEmptyClass}>
                        <IconProjects className={productEmptyIconClass} />
                        <strong className={productEmptyTitleClass}>{t('task_no_plan')}</strong>
                        <span className={productEmptyTextClass}>{t('task_no_plan_desc')}</span>
                      </div>
                    ) : (
                      <div className={taskClass.operationList}>
                        {aiWorkspace.plan.operations.map(operation => {
                          const approved = operation.permission_kind === 'read' || aiWorkspace.approved.has(operation.index);
                          const denied = aiWorkspace.denied.has(operation.index);
                          const pending = operation.permission_kind !== 'read' && !approved && !denied;
                          const stateLabel = operation.permission_kind === 'read'
                            ? t('op_auto_allowed')
                            : approved
                              ? t('op_allowed')
                              : denied
                                ? t('op_denied_once')
                                : t('op_awaiting');
                          const metaLabel = operation.permission_kind === 'read'
                            ? 'AI 可以读取项目状态，不会改文件'
                            : operation.permission_kind === 'write'
                              ? '会改动项目文件，应用前需要你确认'
                              : '会执行命令或工具调用，风险更高';
                          return (
                            <article
                              className={cx(
                                taskClass.operationRow,
                                approved && taskClass.operationRowApproved,
                                denied && taskClass.operationRowDenied,
                                pending && taskClass.operationRowPending,
                              )}
                              key={operation.index}
                            >
                              <span className={cx(taskClass.operationPermission, taskOperationPermissionLabelClass(operation.permission_kind))}>
                                {operation.permission_kind === 'read' ? '读取' : operation.permission_kind === 'write' ? '写入' : '命令'}
                              </span>
                              <div className={taskClass.operationMain}>
                                <p className={taskClass.operationPreview}>{operation.preview}</p>
                                <small className={taskClass.operationMeta}>{metaLabel}</small>
                              </div>
                              <div className={taskClass.operationDecision}>
                                {pending ? (
                                  <>
                                    <button
                                      className={taskClass.operationButton}
                                      onClick={() => { void aiWorkspace.decideOperation(operation as CopilotOperation, 'once'); }}
                                    >
                                      允许一次
                                    </button>
                                    {operation.permission_kind === 'write' && (
                                      <button
                                        className={taskClass.operationButton}
                                        onClick={() => { void aiWorkspace.decideOperation(operation as CopilotOperation, 'session'); }}
                                      >
                                        本次都允许
                                      </button>
                                    )}
                                    {operation.permission_kind === 'command' && (
                                      <button
                                        className={taskClass.operationButton}
                                        onClick={() => { void aiWorkspace.decideOperation(operation as CopilotOperation, 'always'); }}
                                      >
                                        总是允许
                                      </button>
                                    )}
                                    <button
                                      className={cx(taskClass.operationButton, taskClass.operationDenyButton)}
                                      onClick={() => { void aiWorkspace.decideOperation(operation as CopilotOperation, 'deny'); }}
                                    >
                                      拒绝
                                    </button>
                                  </>
                                ) : (
                                  <small className={taskClass.operationState}>{stateLabel}</small>
                                )}
                              </div>
                            </article>
                          );
                        })}
                      </div>
                    )}
                  </section>

                  {aiWorkspace?.plan && (
                    <footer className={taskClass.footer}>
                      <button className={buttonClass('ghost')} onClick={aiWorkspace.discardProposal}>放弃本次计划</button>
                      <button className={buttonClass('primary')} disabled={!aiTaskCanApply} onClick={aiWorkspace.applyApproved}>
                        {aiTaskApprovedCount > 0 ? `应用 ${aiTaskApprovedCount} 项已允许改动` : '等待你在右侧允许改动'}
                      </button>
                    </footer>
                  )}
                </section>

                <aside className={taskClass.sidebar}>
                  <section className={taskClass.sidebarSection}>
                    <strong className={taskClass.sidebarTitle}>确认要点</strong>
                    <p className={taskClass.sidebarText}>重点审核会改变项目的步骤：“写入”和“命令”必须经过确认后才会真正应用。</p>
                  </section>
                  <section className={taskClass.sidebarSection}>
                    <strong className={taskClass.sidebarTitle}>安全规则</strong>
                    <div className={taskClass.checklist}>
                      <div className={taskClass.checklistItem}><IconCheck /> 读取项目：自动允许</div>
                      <div className={taskClass.checklistItem}><IconAlertCircle /> 写入文件：必须确认</div>
                      <div className={taskClass.checklistItem}><IconPackage /> 命令工具：必须确认</div>
                      <div className={taskClass.checklistItem}><IconUndo /> 应用后尽量保留撤销入口</div>
                    </div>
                  </section>
                  <section className={taskClass.sidebarSection}>
                    <strong className={taskClass.sidebarTitle}>当前状态</strong>
                    <p className={taskClass.sidebarText}>已允许 {aiTaskApprovedCount} 项，已拒绝 {aiTaskDeniedCount} 项，等待确认 {aiTaskPendingCount} 项。</p>
                  </section>
                </aside>
              </div>
            </div>}

            {workspaceView === 'game' && <div className={gameSurfaceClass(hierarchyOpen, false)}>
              {hierarchyOpen && <aside className={gameClass.sidePanel}>
                <header className={gameClass.panelHeader}>
                  <div className={gameClass.panelHeaderText}><span>场景结构</span><strong className={gameClass.panelHeaderTitle}>{sceneTree.length} 个对象</strong></div>
                  <div className={gameClass.panelHeaderActions}>
                    <button className={gameClass.iconButton} onClick={() => createSceneObject()} title="创建对象"><IconPlus /></button>
                    <button className={gameClass.iconButton} onClick={() => setHierarchyOpen(false)} title="收起场景结构"><IconX /></button>
                  </div>
                </header>
                <div className={gameClass.hierarchyList}>
                  {hierarchyRows.length === 0 ? (
                    <p className={gameClass.empty}>当前场景还没有对象。</p>
                  ) : hierarchyRows.map(({ object, depth, hasChildren }) => {
                    const selected = selectedId === object.id;
                    const collapsed = collapsedNodes.has(object.id);
                    return (
                      <div
                        key={object.id}
                        className={cx(gameClass.hierarchyItem, selected && gameClass.hierarchyItemSelected)}
                        onClick={() => selectSceneObject(object.id)}
                        style={{ paddingLeft: depth * 14 + 6 }}
                      >
                        {hasChildren ? (
                          <span
                            className={gameClass.hierarchyTwisty}
                            onClick={event => { event.stopPropagation(); toggleNodeCollapsed(object.id); }}
                            title={collapsed ? 'Expand' : 'Collapse'}
                          >
                            {collapsed ? <IconChevronRight /> : <IconChevronDown />}
                          </span>
                        ) : (
                          <span className={gameClass.hierarchyTwistySpacer} />
                        )}
                        {sceneNodeIcon(object, selected)}
                        <span className={gameClass.hierarchyName}>{object.name}</span>
                        {object.tag && <span className={gameClass.hierarchyTag}>{object.tag}</span>}
                      </div>
                    );
                  })}
                </div>
              </aside>}

              <section className={gameClass.mainPanel}>
                <div className={gameClass.previewBar}>
                  <div className={gameClass.previewBarGroup}><span className={gameClass.liveDot} />场景 / 游戏视图</div>
                  <div className={gameClass.modeSwitch}>
                    {!hierarchyOpen && (
                      <button className={gameClass.modeButton} onClick={() => setHierarchyOpen(true)}>场景结构</button>
                    )}
                    <button className={cx(gameClass.modeButton, viewMode === '2d' && gameClass.modeButtonActive)} onClick={() => setViewMode('2d')}>2D</button>
                    <button className={cx(gameClass.modeButton, viewMode === '3d' && gameClass.modeButtonActive)} onClick={() => setViewMode('3d')}>3D</button>
                    <button className={gameClass.modeButton} onClick={() => rpc('shell/undo').then(() => refreshSceneTree())} disabled={!shellState.can_undo}>撤销</button>
                    <button className={gameClass.modeButton} onClick={() => rpc('shell/redo').then(() => refreshSceneTree())} disabled={!shellState.can_redo}>重做</button>
                    <button className={gameClass.modeButton} onClick={() => openNativeSceneView({
                      yaw: cameraRef.current.yaw,
                      pitch: cameraRef.current.pitch,
                      distance: cameraRef.current.distance,
                      targetX: cameraRef.current.targetX,
                      targetY: cameraRef.current.targetY,
                      targetZ: cameraRef.current.targetZ,
                    })}>原生视图</button>
                    <button className={gameClass.modeButton} onClick={openGameView}><IconPlay /> 运行</button>
                    {selectedId && !inspectorOpen && (
                      <button className={gameClass.modeButton} onClick={() => setInspectorOpen(true)}>属性</button>
                    )}
                  </div>
                </div>
                <div className={gameClass.createPresets} aria-label="创建场景对象">
                  <button className={gameClass.createButton} onClick={() => createSceneObject()}><IconPlus /> 空对象</button>
                  <button className={gameClass.createButton} onClick={() => createPresetObject('Camera', 'Camera')}><IconPlus /> 摄像机</button>
                  <button className={gameClass.createButton} onClick={() => createPresetObject('Light', 'Light')}><IconPlus /> 灯光</button>
                  <button className={gameClass.createButton} onClick={() => createPresetObject('Mesh Object', 'MeshRenderer')}><IconPlus /> 模型</button>
                  <button className={gameClass.createButton} onClick={() => createPresetObject('Audio Source', 'AudioSource')}><IconPlus /> 音频</button>
                  <button className={gameClass.createButton} onClick={() => createPresetObject('Rigid Body', 'Rigidbody')}><IconPlus /> 刚体</button>
                  <button className={gameClass.createButton} onClick={() => createPresetObject('Collider', 'Collider')}><IconPlus /> 碰撞体</button>
                  <button className={gameClass.createButton} onClick={createBehaviorObject}><IconCode /> 行为</button>
                </div>
                <div className={gameClass.previewCanvas} onClick={handleViewportClick}>
                  <ViewportCanvas
                    sceneVersion={sceneVersion}
                    cameraRef={cameraRef}
                    onCameraChange={handleCameraChange}
                    onResize={handleViewportResize}
                    viewMode={viewMode}
                    editorCamera
                  />
                  <div className={gameClass.viewportAtmosphere} />
                  <div className={gameClass.viewportHorizon} />
                  <div className={gameClass.viewportVignette} />
                  <ViewportHud
                    projectName={shellState.project_name}
                    sceneObjectCount={sceneTree.length}
                    selectedObject={selectedObject}
                    selectedEntityDetails={selectedEntityDetails}
                    viewMode={viewMode}
                    dirty={shellState.scene_dirty}
                  />
                  <SelectionOverlay sceneTree={sceneTree} selectedId={selectedId} camera={cameraRef.current} width={viewportSize.width} height={viewportSize.height} viewMode={viewMode} />
                  {sceneTree.length === 0 && (
                    <EmptyViewportState
                      onCreateObject={() => createSceneObject()}
                      onCreateCamera={() => createPresetObject('Camera', 'Camera')}
                      onOpenAi={() => setAiPanelOpen(true)}
                    />
                  )}
                  {selectedObject && !inspectorOpen && (
                    <ViewportSelectionCard
                      object={selectedObject}
                      details={selectedEntityDetails}
                      onFocus={() => focusOnPosition(selectedObject.position)}
                      onOpenInspector={() => setInspectorOpen(true)}
                      onAskAi={() => {
                        setAiPanelOpen(true);
                        setContextualRequest({
                          id: Date.now(),
                          prompt: `请分析这个场景对象，并给出适合新手理解的修改建议。不要直接应用改动，先给计划。\n\n对象：${selectedObject.name}\nID：${selectedObject.id}\n标签：${selectedObject.tag || '未标记'}\n位置：${formatScenePosition(selectedObject.position)}\n组件：${selectedEntityDetails?.components.map(component => component.type).join(', ') || '暂无组件'}`,
                        });
                      }}
                    />
                  )}
                </div>
              </section>

              {inspectorOpen && selectedEntityDetails && <aside className={gameClass.inspectorPanel} aria-label="对象属性面板">
                <header className={gameClass.panelHeader}>
                  <div className={gameClass.panelHeaderText}>
                    <span>属性</span>
                    <strong className={gameClass.panelHeaderTitle}>{selectedEntityDetails.name}</strong>
                  </div>
                  <button className={gameClass.iconButton} onClick={() => setInspectorOpen(false)} title="收起属性">
                    <IconX />
                  </button>
                </header>
                <div className={inspectorClass.root}>
                    <section className={inspectorClass.section}>
                      <div className={inspectorClass.sectionTitle}>对象</div>
                      <label className={inspectorClass.field}>
                        <span>名称</span>
                        <input
                          className={inspectorClass.input}
                          value={selectedEntityNameDraft}
                          onChange={event => setSelectedEntityNameDraft(event.target.value)}
                          onBlur={renameSelectedObject}
                          onKeyDown={event => {
                            if (event.key === 'Enter') event.currentTarget.blur();
                            if (event.key === 'Escape') setSelectedEntityNameDraft(selectedEntityDetails.name);
                          }}
                        />
                      </label>
                      <label className={inspectorClass.field}>
                        <span>父级</span>
                        <select
                          className={inspectorClass.select}
                          value={selectedObject?.parent_id ?? ''}
                          onChange={event => reparentSelectedObject(event.currentTarget.value)}
                        >
                          <option value="">场景根节点</option>
                          {validParentOptions.map(object => (
                            <option key={object.id} value={object.id}>{object.name}</option>
                          ))}
                        </select>
                      </label>
                      <div className={inspectorClass.actionRow}>
                        <button className={inspectorClass.actionButton} onClick={duplicateSelectedObject}><IconCopy /> 复制</button>
                        <button className={inspectorClass.actionButton} onClick={deleteSelectedObject}><IconTrash /> 删除</button>
                      </div>
                    </section>
                    {(['position', 'rotation', 'scale'] as const).map(field => (
                      <section className={inspectorClass.section} key={field}>
                        <div className={inspectorClass.sectionTitle}>{TRANSFORM_FIELD_LABELS[field]}</div>
                        <div className={field === 'rotation' ? inspectorClass.vec4 : inspectorClass.vec3}>
                          {selectedEntityDetails.transform[field].map((value, index) => (
                            <label className={inspectorClass.vecInputWrap} key={`${field}-${index}`}>
                              <span className={inspectorClass.vecLabel}>{['X', 'Y', 'Z', 'W'][index]}</span>
                              <input
                                className={inspectorClass.vecInput}
                                defaultValue={value.toFixed(2)}
                                inputMode="decimal"
                                onBlur={event => updateSelectedTransform(field, index, event.currentTarget.value)}
                                onWheel={event => nudgeSelectedTransform(field, index, event)}
                                onKeyDown={event => {
                                  if (event.key === 'Enter') event.currentTarget.blur();
                                  if (event.key === 'Escape') event.currentTarget.blur();
                                }}
                              />
                            </label>
                          ))}
                        </div>
                      </section>
                    ))}
                    <section className={inspectorClass.section}>
                      <div className={inspectorClass.sectionTitle}>组件</div>
                      {selectedEntityDetails.components.map(component => {
                        const schema = componentSchemaByType.get(component.type);
                        const fields = orderedComponentFields(component, schema);
                        return (
                          <div className={inspectorClass.component} key={component.type}>
                            <div className={inspectorClass.componentHeader}>
                              <span className={inspectorClass.componentType}>{componentDisplayLabel(component.type, schema)}</span>
                              <button className={inspectorClass.removeButton} onClick={() => removeSelectedComponent(component.type)} title="移除组件">×</button>
                            </div>
                            <div className={inspectorClass.componentFields}>
                              {fields.length === 0 ? (
                                <div className={inspectorClass.emptyField}>没有可编辑字段</div>
                              ) : fields.map(field => (
                                <ComponentFieldEditor
                                  key={`${component.type}-${field.fieldName}`}
                                  componentType={component.type}
                                  fieldName={field.fieldName}
                                  value={field.value}
                                  schema={field.schema}
                                  isDefaultOnly={field.isDefaultOnly}
                                  scriptOptions={scriptBindingOptions}
                                  assetOptions={unifiedAssets}
                                  onCommit={(name, nextValue) => updateSelectedComponentField(component.type, name, nextValue)}
                                />
                              ))}
                            </div>
                          </div>
                        );
                      })}
                      <div className={inspectorClass.addRow}>
                        <select
                          className={inspectorClass.select}
                          value={addComponentType}
                          onChange={event => setAddComponentType(event.target.value)}
                          disabled={addableComponentSchemas.length === 0}
                        >
                          {addableComponentSchemas.map(schema => (
                            <option key={schema.type_id} value={schema.type_id}>{componentDisplayLabel(schema.type_id, schema)}</option>
                          ))}
                        </select>
                        <button className={inspectorClass.addButton} onClick={addSelectedComponent} disabled={addableComponentSchemas.length === 0}>
                          <IconPlus /> 添加
                        </button>
                      </div>
                    </section>
                </div>
              </aside>}
            </div>}

            {workspaceView === 'assets' && <div className={surfaceClass.root} aria-label="资源工作区">
              <header className={surfaceClass.header}>
                <div>
                  <span className={surfaceClass.headerKicker}>项目资源</span>
                  <strong className={surfaceClass.headerTitle}>{unifiedAssets.length} 个可管理资源</strong>
                  <small className={surfaceClass.headerDesc}>统一管理脚本、场景、材质和预制体。这里的操作直接连接项目文件与资源导入后端。</small>
                </div>
                <div className={surfaceClass.toolbar}>
                  <button className={surfaceClass.button} onClick={() => createProjectAsset('script')} disabled={assetsBusy}>脚本</button>
                  <button className={surfaceClass.button} onClick={() => createProjectAsset('material')} disabled={assetsBusy}>材质</button>
                  <button className={surfaceClass.button} onClick={() => createProjectAsset('prefab')} disabled={assetsBusy}>预制体</button>
                  <button className={surfaceClass.button} onClick={() => createProjectAsset('scene')} disabled={assetsBusy}>场景</button>
                  <button className={surfaceClass.button} onClick={refreshProjectAssets} disabled={assetsBusy}>
                    {assetsBusy ? '刷新中' : '刷新'}
                  </button>
                </div>
              </header>
              <div className={assetsClass.layout}>
                <section className={assetsClass.main}>
                  <div className={assetsClass.summary}>
                    <div className={assetsClass.summaryCard}><span className={assetsClass.summaryLabel}>全部资源</span><strong className={assetsClass.summaryValue}>{unifiedAssets.length}</strong></div>
                    <div className={assetsClass.summaryCard}><span className={assetsClass.summaryLabel}>行为脚本</span><strong className={assetsClass.summaryValue}>{scriptAssetCount}</strong></div>
                    <div className={assetsClass.summaryCard}><span className={assetsClass.summaryLabel}>场景文件</span><strong className={assetsClass.summaryValue}>{sceneAssetCount}</strong></div>
                    <div className={assetsClass.summaryCard}><span className={assetsClass.summaryLabel}>材质资源</span><strong className={assetsClass.summaryValue}>{materialAssetCount}</strong></div>
                  </div>
                  <div className={assetsClass.filterBar} aria-label="资源类型筛选">
                    {assetKinds.map(kind => (
                      <button
                        key={kind}
                        className={cx(assetsClass.filterButton, assetKindFilter === kind && assetsClass.filterButtonActive)}
                        onClick={() => setAssetKindFilter(kind)}
                      >
                        {kind === '全部' ? kind : assetKindLabel(kind)}
                      </button>
                    ))}
                  </div>
                  <div className={assetsClass.tableHeader}>
                    <span>资源</span>
                    <span>类型</span>
                    <span>导入器</span>
                    <span className="justify-self-end">动作</span>
                  </div>
                  <div className={surfaceClass.list}>
                    {filteredAssets.length === 0 ? (
                      <div className={surfaceClass.empty}>
                        <div>
                          <strong className="block text-[14px] text-[var(--text-primary)]">这里还没有匹配资源</strong>
                          <span className="mt-2 block text-[12px] text-[var(--text-muted)]">可以创建脚本、材质、预制体或场景，也可以刷新项目索引。</span>
                        </div>
                      </div>
                    ) : filteredAssets.map(asset => {
                      const canOpenScript = isScriptPath(asset.source_path);
                      return (
                        <article className={assetsClass.row} key={asset.guid || asset.source_path}>
                          <div className={assetsClass.rowMain}>
                            <strong className={assetsClass.rowTitle}>
                              {canOpenScript ? <IconCode /> : <IconFile />}
                              {asset.source_path.split('/').pop() || asset.source_path}
                            </strong>
                            <span className={assetsClass.rowMeta}>{asset.source_path}</span>
                          </div>
                          <small className={assetsClass.pill}>{assetKindLabel(asset.kind)}</small>
                          <small className={assetsClass.pill}>{importerLabel(asset.importer)}</small>
                          <div className={assetsClass.actions}>
                            {canOpenScript ? (
                              <button className={surfaceClass.button} onClick={() => {
                                setSelectedScript(asset.source_path);
                                setWorkspaceView('scripts');
                              }}>打开</button>
                            ) : (
                              <button className={surfaceClass.button} onClick={event => {
                                setArtifactSelection({
                                  kind: 'document',
                                  label: asset.source_path,
                                  context: `资源：${asset.source_path}\n类型：${assetKindLabel(asset.kind)}\n导入器：${importerLabel(asset.importer)}\nGUID：${asset.guid}`,
                                  x: event.clientX,
                                  y: event.clientY,
                                });
                                setArtifactQuestionOpen(false);
                              }}>检查</button>
                            )}
                            <button className={surfaceClass.button} onClick={event => revealAssetReferences(asset, event)} disabled={assetsBusy}>引用</button>
                            <button className={surfaceClass.button} onClick={() => reloadAsset(asset.source_path)} disabled={assetsBusy}>重载</button>
                          </div>
                        </article>
                      );
                    })}
                  </div>
                </section>
                <aside className={assetsClass.sidebar}>
                  <section className={assetsClass.sidebarSection}>
                    <strong className={assetsClass.sidebarTitle}>给新手的说明</strong>
                    <p className={assetsClass.sidebarText}>资源不是独立摆设：脚本可打开到行为编辑器，引用会查询后端依赖关系，重载会走项目重新导入流程。</p>
                  </section>
                  <section className={assetsClass.sidebarSection}>
                    <strong className={assetsClass.sidebarTitle}>当前索引</strong>
                    <div className={assetsClass.sidebarList}>
                      <div className={assetsClass.sidebarItem}><span>筛选</span><b>{assetKindFilter === '全部' ? assetKindFilter : assetKindLabel(assetKindFilter)}</b></div>
                      <div className={assetsClass.sidebarItem}><span>显示</span><b>{filteredAssets.length}</b></div>
                      <div className={assetsClass.sidebarItem}><span>状态</span><b>{assetsBusy ? '同步中' : '已同步'}</b></div>
                    </div>
                  </section>
                  <section className={assetsClass.sidebarSection}>
                    <strong className={assetsClass.sidebarTitle}>建议流程</strong>
                    <p className={assetsClass.sidebarText}>先创建脚本或场景，再把具体修改交给右侧 AI。AI 提出计划后再确认应用，不直接破坏项目文件。</p>
                  </section>
                </aside>
              </div>
            </div>}

            {workspaceView === 'scripts' && <div className={scriptSurfaceClass.root} aria-label="行为脚本工作区">
              <aside className={scriptSurfaceClass.sidebar}>
                <header className={scriptSurfaceClass.sidebarHeader}>
                  <span className={scriptSurfaceClass.sidebarKicker}>行为脚本</span>
                  <strong className={scriptSurfaceClass.sidebarTitle}>{scripts.length} 个脚本</strong>
                  <small className={scriptSurfaceClass.sidebarMeta}>选择脚本后可编辑、保存、检查，并把某一行交给 AI。</small>
                </header>
                {scripts.length === 0 ? <p className={scriptSurfaceClass.sidebarEmpty}>{t('scripts_empty')}</p> : scripts.map(path => (
                  <button key={path} className={cx(scriptSurfaceClass.scriptButton, selectedScript === path && scriptSurfaceClass.scriptButtonActive)} onClick={() => setSelectedScript(path)}>
                    <IconCode /><span className={scriptSurfaceClass.scriptName}>{path.split('/').pop()}</span><small className={scriptSurfaceClass.scriptPath}>{path}</small>
                  </button>
                ))}
              </aside>
              <article className={scriptSurfaceClass.editor}>
                <header className={scriptSurfaceClass.editorHeader}>
                  <div className={scriptSurfaceClass.editorTitle}>
                    <span className={scriptSurfaceClass.editorKicker}>脚本编辑器</span>
                    <strong className={scriptSurfaceClass.editorFile}>{selectedScript ? selectedScript.split('/').pop() : t('scripts_select')}{scriptDirty ? ' *' : ''}</strong>
                    <small className={scriptSurfaceClass.editorMeta}>{selectedScript || '选择左侧脚本开始编辑'}</small>
                  </div>
                  <div className={scriptSurfaceClass.editorActions}>
                    <span className={cx(scriptSurfaceClass.statusBadge, scriptDirty && scriptSurfaceClass.statusBadgeDirty)}>{scriptDirty ? '未保存' : '已同步'}</span>
                    {scriptDiagnostics.length > 0 && <b className={cx(scriptSurfaceClass.statusBadge, scriptSurfaceClass.statusBadgeError)}>{scriptDiagnostics.length} 个诊断</b>}
                    <b className={scriptSurfaceClass.editorHint}>{t('scripts_line_select_hint')}</b>
                    <button className={scriptSurfaceClass.editorButton} onClick={saveSelectedScript} disabled={!selectedScript || !scriptDirty || scriptSaving}>
                      {scriptSaving ? '保存中' : '保存'}
                    </button>
                  </div>
                </header>
                <div className={scriptSurfaceClass.editorPane}>
                  <textarea
                    className={scriptSurfaceClass.textarea}
                    value={scriptContent}
                    spellCheck={false}
                    disabled={!selectedScript}
                    onChange={event => setScriptContent(event.currentTarget.value)}
                    onSelect={event => {
                      const target = event.currentTarget;
                      const before = target.value.slice(0, target.selectionStart);
                      const selectedText = target.value.slice(target.selectionStart, target.selectionEnd);
                      if (!selectedText.trim()) return;
                      const startLine = before.split('\n').length;
                      const endLine = startLine + selectedText.split('\n').length - 1;
                      setScriptLineRange([startLine, endLine]);
                    }}
                    onKeyDown={event => {
                      if ((event.ctrlKey || event.metaKey) && event.key === 's') {
                        event.preventDefault();
                        saveSelectedScript();
                      }
                    }}
                  />
                  <aside className="flex min-h-0 flex-col">
                    <div className={scriptSurfaceClass.gutterHeader}>
                      <strong className={scriptSurfaceClass.gutterTitle}>行引用</strong>
                      <small className={scriptSurfaceClass.gutterHint}>点击任意行可以把上下文交给右侧 AI。Shift 点击可扩展选择范围。</small>
                    </div>
                    <pre className={scriptSurfaceClass.gutter}><code>{(scriptContent || '// Aster-generated scripts will appear here.').split('\n').map((line, index) => { const lineNumber = index + 1; const selected = scriptLineRange && lineNumber >= scriptLineRange[0] && lineNumber <= scriptLineRange[1]; return <button key={lineNumber} className={cx(scriptSurfaceClass.gutterButton, selected && scriptSurfaceClass.gutterButtonSelected)} onClick={event => selectScriptLine(lineNumber, event.shiftKey, event)}><span className={scriptSurfaceClass.gutterLineNumber}>{lineNumber}</span><i className={scriptSurfaceClass.gutterLineText}>{line || ' '}</i></button>; })}</code></pre>
                  </aside>
                </div>
                {scriptDiagnostics.length > 0 && <div className={scriptSurfaceClass.diagnostics}>
                  {scriptDiagnostics.map((diagnostic, index) => <div className={scriptSurfaceClass.diagnostic} key={`${diagnostic.code}-${diagnostic.line ?? 0}-${index}`}>
                    <strong className={scriptSurfaceClass.diagnosticMessage}>
                      {diagnostic.code} {diagnostic.line ? `第 ${diagnostic.line} 行${diagnostic.column ? `:${diagnostic.column}` : ''}` : ''}：{diagnostic.message}
                    </strong>
                    <span className={scriptSurfaceClass.diagnosticSuggestion}>建议：{diagnostic.suggestion}</span>
                  </div>)}
                </div>}
              </article>
            </div>}

            {workspaceView === 'build' && <div className={surfaceClass.root} aria-label="构建与打包工作区">
              <header className={surfaceClass.header}>
                <div>
                  <span className={surfaceClass.headerKicker}>构建与打包</span>
                  <strong className={surfaceClass.headerTitle}>{shellState.project_name || '未命名项目'}</strong>
                  <small className={surfaceClass.headerDesc}>当前只开放后端已支持的本机文件夹包；跨平台和安装器会明确显示为待补齐，避免误点假功能。</small>
                </div>
                <button
                  className={surfaceClass.primaryButton}
                  onClick={requestBuildPackage}
                  disabled={buildBusy || !selectedBuildCanRun}
                  title={selectedBuildUnavailableReason ?? '打包当前项目'}
                >
                  {buildBusy ? <IconLoader className="animate-spin" /> : <IconPackage />}
                  {buildBusy ? '打包中' : selectedBuildCanRun ? '开始打包' : '暂不可打包'}
                </button>
              </header>

              <div className={buildClass.layout}>
                <section className={buildClass.main}>
                  <div className={buildClass.summary}>
                    <div className={buildClass.summaryCard}>
                      <span className={buildClass.summaryLabel}>当前目标</span>
                      <strong className={buildClass.summaryValue}>{selectedBuildTarget.label}</strong>
                    </div>
                    <div className={buildClass.summaryCard}>
                      <span className={buildClass.summaryLabel}>输出格式</span>
                      <strong className={cx(buildClass.summaryValue, buildClass.summaryMono)}>{formatBuildFormat(buildFormat)}</strong>
                    </div>
                    <div className={buildClass.summaryCard}>
                      <span className={buildClass.summaryLabel}>构建通道</span>
                      <strong className={buildClass.summaryValue}>{formatBuildChannel(buildChannel)}</strong>
                    </div>
                    <div className={buildClass.summaryCard}>
                      <span className={buildClass.summaryLabel}>后端状态</span>
                      <strong className={buildStatusClass(selectedBuildTarget.status)}>{buildStatusLabel(selectedBuildTarget.status)}</strong>
                    </div>
                  </div>

                  <section className={buildClass.section}>
                    <div className={buildClass.sectionTitle}>
                      <div>
                        <span className={surfaceClass.buildKicker}>构建预设</span>
                        <p className={buildClass.sectionHint}>预设只负责快速切换目标，不会绕过后端能力限制。</p>
                      </div>
                    </div>
                    <div className={buildClass.presets} aria-label="构建预设">
                      {BUILD_PRESETS.map(preset => {
                        const target = BUILD_TARGETS.find(item => item.id === preset.target) ?? BUILD_TARGETS[0];
                        const presetRunnable = canRunBuild(target, preset.format);
                        return (
                          <button
                            key={preset.id}
                            className={cx(
                              buildClass.presetButton,
                              buildTarget === preset.target && buildFormat === preset.format && buildChannel === preset.channel && buildClass.selectedButton,
                            )}
                            onClick={() => applyBuildPreset(preset)}
                            title={presetRunnable ? '选择这个构建预设' : buildUnavailableReason(target, preset.format) ?? undefined}
                          >
                            <IconPackage />
                            <span className={buildClass.itemTitle}>{preset.label}</span>
                            <small className={buildClass.itemMeta}>{target.label} · {formatBuildFormat(preset.format)} · {formatBuildChannel(preset.channel)}</small>
                          </button>
                        );
                      })}
                    </div>
                  </section>

                  <section className={buildClass.section}>
                    <div className={buildClass.card}>
                      <div className={buildClass.sectionTitle}>
                        <div>
                          <span className={surfaceClass.buildKicker}>目标平台</span>
                          <p className={buildClass.sectionHint}>只有“可用 + 文件夹”会真正调用 project/package。</p>
                        </div>
                        <strong className={buildStatusClass(selectedBuildTarget.status)}>{buildStatusLabel(selectedBuildTarget.status)}</strong>
                      </div>
                      <div className={buildClass.targetGrid}>
                        {BUILD_TARGETS.map(target => (
                          <button
                            key={target.id}
                            className={cx(buildClass.targetButton, buildTarget === target.id && buildClass.selectedButton)}
                            onClick={() => {
                              setBuildTarget(target.id);
                              setBuildFormat(target.status === 'ready' ? 'folder' : target.formats[0]);
                            }}
                            title={target.note}
                          >
                            <span className={buildClass.itemTitle}>{target.label}</span>
                            <small className={buildClass.itemMeta}>{target.formats.map(formatBuildFormat).join(' / ')}</small>
                            <b className={buildStatusClass(target.status)}>{buildStatusLabel(target.status)}</b>
                          </button>
                        ))}
                      </div>
                    </div>
                  </section>

                  <section className={buildClass.section}>
                    <div className={buildClass.card}>
                      <div className={buildClass.sectionTitle}>
                        <div>
                          <span className={surfaceClass.buildKicker}>打包设置</span>
                          <p className={buildClass.sectionHint}>{selectedBuildUnavailableReason ?? '当前配置会调用真实打包后端。'}</p>
                        </div>
                        <strong className={buildClass.sectionValue}>{selectedBuildTarget.label}</strong>
                      </div>
                      <div className={buildClass.formGrid}>
                        <label className={buildClass.formLabel}>
                          <span className={buildClass.formLabelText}>格式</span>
                          <select className={buildClass.select} value={buildFormat} onChange={event => setBuildFormat(event.currentTarget.value as BuildFormat)}>
                            {selectedBuildTarget.formats.map(format => (
                              <option key={format} value={format}>{formatBuildFormat(format)}</option>
                            ))}
                          </select>
                        </label>
                        <label className={buildClass.formLabel}>
                          <span className={buildClass.formLabelText}>通道</span>
                          <select className={buildClass.select} value={buildChannel} onChange={event => setBuildChannel(event.currentTarget.value as BuildChannel)}>
                            <option value="release">发布</option>
                            <option value="debug">调试</option>
                          </select>
                        </label>
                        <label className={cx(buildClass.formLabel, buildClass.checkbox)}>
                          <input
                            className={buildClass.checkboxInput}
                            type="checkbox"
                            checked={buildOptimizeAssets}
                            onChange={event => setBuildOptimizeAssets(event.currentTarget.checked)}
                          />
                          <span className={buildClass.formLabelText}>优化资源</span>
                        </label>
                        <label className={cx(buildClass.formLabel, buildClass.checkbox)}>
                          <input
                            className={buildClass.checkboxInput}
                            type="checkbox"
                            checked={buildIncludeDebugSymbols}
                            onChange={event => setBuildIncludeDebugSymbols(event.currentTarget.checked)}
                          />
                          <span className={buildClass.formLabelText}>包含调试符号</span>
                        </label>
                      </div>
                    </div>
                  </section>

                  <section className={buildClass.section}>
                    <div className={buildClass.output}>
                      <div>
                        <span className={surfaceClass.buildKicker}>输出位置</span>
                        <strong className={buildClass.outputPath}>exports/{shellState.project_name || 'project'}/{buildTarget}/{buildChannel}</strong>
                      </div>
                      <p className={buildClass.outputNote}>{selectedBuildTarget.note}</p>
                      {buildMessage && <pre className={buildClass.outputPre}>{buildMessage}</pre>}
                    </div>
                  </section>
                </section>

                <aside className={buildClass.sidebar}>
                  <section className={buildClass.sidebarSection}>
                    <span className={surfaceClass.buildKicker}>真实流程</span>
                    <ol className={buildClass.sidebarList}>
                      <li className={cx(buildClass.sidebarItem, buildClass.sidebarItemDone)}><IconCheck /> 保存当前场景</li>
                      <li className={cx(buildClass.sidebarItem, buildClass.sidebarItemActive)}><IconPackage /> 编译最小运行时</li>
                      <li className={cx(buildClass.sidebarItem, buildClass.sidebarItemActive)}><IconPackage /> 复制项目资源</li>
                      <li className={cx(buildClass.sidebarItem, selectedBuildCanRun ? buildClass.sidebarItemActive : buildClass.sidebarItemLocked)}><IconPackage /> 写入启动器与清单</li>
                      <li className={cx(buildClass.sidebarItem, buildClass.sidebarItemLocked)}><IconCheck /> 安装器 / 签名 / 公证</li>
                    </ol>
                  </section>
                  <section className={buildClass.sidebarSection}>
                    <span className={surfaceClass.buildKicker}>当前请求</span>
                    <dl className={buildClass.sidebarDl}>
                      <div className={buildClass.sidebarDlRow}><dt className={buildClass.sidebarDt}>项目</dt><dd className={buildClass.sidebarDd}>{shellState.project_name || 'project'}</dd></div>
                      <div className={buildClass.sidebarDlRow}><dt className={buildClass.sidebarDt}>目标</dt><dd className={buildClass.sidebarDd}>{selectedBuildTarget.label}</dd></div>
                      <div className={buildClass.sidebarDlRow}><dt className={buildClass.sidebarDt}>格式</dt><dd className={buildClass.sidebarDd}>{formatBuildFormat(buildFormat)}</dd></div>
                      <div className={buildClass.sidebarDlRow}><dt className={buildClass.sidebarDt}>通道</dt><dd className={buildClass.sidebarDd}>{formatBuildChannel(buildChannel)}</dd></div>
                      <div className={buildClass.sidebarDlRow}><dt className={buildClass.sidebarDt}>资源</dt><dd className={buildClass.sidebarDd}>{buildOptimizeAssets ? '优化' : '原始'}</dd></div>
                      <div className={buildClass.sidebarDlRow}><dt className={buildClass.sidebarDt}>符号</dt><dd className={buildClass.sidebarDd}>{buildIncludeDebugSymbols ? '包含' : '剥离'}</dd></div>
                    </dl>
                  </section>
                  <section className={buildClass.sidebarSection}>
                    <span className={surfaceClass.buildKicker}>交给 AI</span>
                    <p className={assetsClass.sidebarText}>如果打包失败，先去诊断面板查看 build 日志，再让右侧 AI 根据错误和项目状态给出下一步。</p>
                    <button
                      className={surfaceClass.button}
                      onClick={() => {
                        setAiPanelOpen(true);
                        setContextualRequest({
                          id: Date.now(),
                          prompt: `请检查当前 Aster 构建配置并给出下一步。项目：${shellState.project_name || 'project'}；目标：${selectedBuildTarget.label}；格式：${formatBuildFormat(buildFormat)}；通道：${formatBuildChannel(buildChannel)}；状态：${buildStatusLabel(selectedBuildTarget.status)}；说明：${selectedBuildTarget.note}`,
                        });
                      }}
                    >
                      <IconSparkles /> 让 AI 检查构建
                    </button>
                  </section>
                </aside>
              </div>
            </div>}

            {workspaceView === 'diagnostics' && <div className={surfaceClass.root} aria-label="功能健康检查中心">
              <header className={surfaceClass.header}>
                <div>
                  <span className={surfaceClass.headerKicker}>功能健康检查中心</span>
                  <strong className={surfaceClass.headerTitle}>
                    {healthReport ? `${healthReport.summary.score} 分 · ${healthReport.summary.total} 项能力` : '正在连接真实后端能力'}
                  </strong>
                  <small className={surfaceClass.headerDesc}>不是只看日志：这里会实际检查项目、场景、资源、脚本、AI 网关、任务实验室和构建出口，并只提供安全的一键修复。</small>
                </div>
                <div className={diagnosticsClass.headerActions}>
                  <button className={surfaceClass.button} onClick={runHealthCheck} disabled={healthBusy}>
                    {healthBusy ? <IconLoader className="size-3 animate-spin" /> : <IconCheck />} {healthBusy ? '检查中' : '全面检查'}
                  </button>
                  <button className={surfaceClass.button} onClick={() => refreshConsoleEntries()} disabled={consoleBusy}>
                    {consoleBusy ? <IconLoader className="size-3 animate-spin" /> : <IconRefresh />} 刷新日志
                  </button>
                  <button className={surfaceClass.button} onClick={clearConsoleEntries} disabled={consoleBusy || consoleEntries.length === 0}>清空旧日志</button>
                </div>
              </header>

              <div className={diagnosticsClass.layout}>
                <section className={diagnosticsClass.main}>
                  <div className={diagnosticsClass.hero}>
                    <div className={diagnosticsClass.scoreRow}>
                      <div className={diagnosticsClass.scoreDial}>{healthReport?.summary.score ?? '--'}</div>
                      <div className={diagnosticsClass.scoreCopy}>
                        <span className={diagnosticsClass.scoreKicker}>Health scan · {formatHealthScanTime(healthReport?.scanned_at)}</span>
                        <strong className={diagnosticsClass.scoreTitle}>{healthReport ? healthStatusLabel(healthReport.summary.status) : '等待首次检查'}</strong>
                        <p className={diagnosticsClass.scoreDesc}>点击左侧能力项，右侧会显示问题、证据和可执行修复。危险或不可逆操作不会放进“一键修复”，只会交给 AI 生成计划并等待确认。</p>
                      </div>
                    </div>
                    <div className={diagnosticsClass.statusMetrics}>
                      <div className={diagnosticsClass.statusMetric}><span className={diagnosticsClass.statusMetricLabel}>正常</span><b className={diagnosticsClass.statusMetricValue}>{healthReport?.summary.ok ?? 0}</b></div>
                      <div className={diagnosticsClass.statusMetric}><span className={diagnosticsClass.statusMetricLabel}>注意</span><b className={diagnosticsClass.statusMetricValue}>{healthReport?.summary.warning ?? 0}</b></div>
                      <div className={diagnosticsClass.statusMetric}><span className={diagnosticsClass.statusMetricLabel}>异常</span><b className={diagnosticsClass.statusMetricValue}>{healthReport?.summary.error ?? 0}</b></div>
                      <div className={diagnosticsClass.statusMetric}><span className={diagnosticsClass.statusMetricLabel}>未配置</span><b className={diagnosticsClass.statusMetricValue}>{healthReport?.summary.not_configured ?? 0}</b></div>
                    </div>
                  </div>

                  <div className={diagnosticsClass.capabilityList}>
                    {!healthReport ? (
                      <div className={surfaceClass.empty}>
                        <div>
                          {healthBusy ? <IconLoader className="mx-auto mb-3 size-7 animate-spin text-[var(--accent)]" /> : <IconAlertCircle className="mx-auto mb-3 size-7 text-[var(--text-muted)]" />}
                          <strong className="block text-[13px] text-[var(--text-primary)]">还没有健康检查结果</strong>
                          <span className="mt-1 block text-[11px] text-[var(--text-muted)]">点击“全面检查”，系统会调用 diagnostics/run_health_check 生成真实能力清单。</span>
                        </div>
                      </div>
                    ) : healthReport.groups.map(group => (
                      <section className={diagnosticsClass.group} key={group.id}>
                        <header className={diagnosticsClass.groupHeader}>
                          <div>
                            <strong className={diagnosticsClass.groupTitle}>{group.label}</strong>
                            <p className={diagnosticsClass.groupDesc}>{group.description}</p>
                          </div>
                          <span className={cx(diagnosticsClass.statusPill, healthStatusClass(group.status))}>{healthStatusLabel(group.status)}</span>
                        </header>
                        <div className={diagnosticsClass.capabilityGrid}>
                          {group.items.map(item => (
                            <button
                              key={item.id}
                              className={cx(diagnosticsClass.capabilityButton, selectedHealthItem?.id === item.id && diagnosticsClass.capabilityButtonActive)}
                              onClick={() => setSelectedHealthItemId(item.id)}
                            >
                              <span className={cx(diagnosticsClass.statusDot, healthStatusDotClass(item.status))} />
                              <span className="min-w-0">
                                <span className={diagnosticsClass.capabilityTitle}>{item.label}</span>
                                <span className={diagnosticsClass.capabilitySummary}>{item.summary}</span>
                              </span>
                              <span className={cx(diagnosticsClass.statusPill, healthStatusClass(item.status))}>{healthStatusLabel(item.status)}</span>
                            </button>
                          ))}
                        </div>
                      </section>
                    ))}
                  </div>
                </section>

                <aside className={diagnosticsClass.sidebar}>
                  <section className={diagnosticsClass.sidebarSection}>
                    <strong className={diagnosticsClass.sidebarTitle}>选中能力详情</strong>
                    {selectedHealthItem ? (
                      <>
                        <div className="mt-3 flex items-start justify-between gap-2">
                          <strong className={diagnosticsClass.detailTitle}>{selectedHealthItem.label}</strong>
                          <span className={cx(diagnosticsClass.statusPill, healthStatusClass(selectedHealthItem.status))}>{healthStatusLabel(selectedHealthItem.status)}</span>
                        </div>
                        <p className={diagnosticsClass.detailSummary}>{selectedHealthItem.detail || selectedHealthItem.summary}</p>
                        <div className={diagnosticsClass.detailEvidence}>
                          {(selectedHealthItem.evidence.length > 0 ? selectedHealthItem.evidence : ['暂无额外证据。']).map((line, index) => (
                            <div className={diagnosticsClass.evidenceRow} key={`${selectedHealthItem.id}-evidence-${index}`}>{line}</div>
                          ))}
                        </div>
                        {selectedHealthItem.fixes.length > 0 ? (
                          <div className={diagnosticsClass.fixList}>
                            {selectedHealthItem.fixes.map(fix => (
                              <button
                                key={fix.id}
                                className={diagnosticsClass.fixButton}
                                onClick={() => applyHealthFix(fix)}
                                disabled={healthFixBusy !== null}
                              >
                                {healthFixBusy === fix.id ? <IconLoader className="size-3 animate-spin" /> : <IconCheck />}
                                <span>
                                  {fix.label}
                                  <small className={diagnosticsClass.fixDesc}>{fix.description}</small>
                                </span>
                              </button>
                            ))}
                          </div>
                        ) : (
                          <p className={diagnosticsClass.sidebarText}>这项没有安全的一键修复。需要改项目结构或写入文件时，应让 AI 先生成计划并等待你确认。</p>
                        )}
                        {healthFixMessage && <p className={diagnosticsClass.sidebarText}>{healthFixMessage}</p>}
                      </>
                    ) : (
                      <p className={diagnosticsClass.sidebarText}>先运行一次全面检查。</p>
                    )}
                  </section>

                  <section className={diagnosticsClass.sidebarSection}>
                    <strong className={diagnosticsClass.sidebarTitle}>交给 AI 处理</strong>
                    <p className={diagnosticsClass.sidebarText}>如果某项是异常或未配置，可以把证据发给右侧 AI，让它生成可审查的修复计划，而不是直接乱改。</p>
                    <button
                      className={surfaceClass.button}
                      disabled={!selectedHealthItem}
                      onClick={() => {
                        if (!selectedHealthItem) return;
                        setAiPanelOpen(true);
                        setContextualRequest({
                          id: Date.now(),
                          prompt: `请根据 Aster 健康检查结果给出可执行修复计划。能力：${selectedHealthItem.label}；状态：${healthStatusLabel(selectedHealthItem.status)}；摘要：${selectedHealthItem.summary}；详情：${selectedHealthItem.detail}；证据：${selectedHealthItem.evidence.join(' | ')}`,
                        });
                      }}
                    >
                      <IconSparkles /> 让 AI 分析此项
                    </button>
                  </section>

                  <section className={diagnosticsClass.sidebarSection}>
                    <strong className={diagnosticsClass.sidebarTitle}>最近后端日志</strong>
                    <div className={diagnosticsClass.metricGrid}>
                      <div className={diagnosticsClass.metric}><span className={diagnosticsClass.metricLabel}>错误</span><b className={diagnosticsClass.metricValue}>{diagnosticCounts.error}</b></div>
                      <div className={diagnosticsClass.metric}><span className={diagnosticsClass.metricLabel}>警告</span><b className={diagnosticsClass.metricValue}>{diagnosticCounts.warn}</b></div>
                    </div>
                    <div className={diagnosticsClass.logPanel}>
                      <div className={diagnosticsClass.filterBar} aria-label="诊断级别筛选">
                        {diagnosticFilters.map(filter => (
                          <button
                            key={filter.id}
                            className={cx(diagnosticsClass.filterButton, diagnosticFilter === filter.id && diagnosticsClass.filterButtonActive)}
                            onClick={() => setDiagnosticFilter(filter.id)}
                          >
                            {filter.label}
                            <b className={workspaceClass.tabBadge}>{filter.count}</b>
                          </button>
                        ))}
                      </div>
                      <div className="max-h-64 overflow-auto p-2 [scrollbar-color:var(--border)_transparent] [scrollbar-width:thin]">
                        {consoleEntries.length === 0 ? (
                          <div className="px-2 py-5 text-center text-[11px] text-[var(--text-muted)]">暂无日志。运行构建、资源导入或脚本检查后会显示在这里。</div>
                        ) : filteredConsoleEntries.length === 0 ? (
                          <div className="px-2 py-5 text-center text-[11px] text-[var(--text-muted)]">当前筛选条件下没有记录。</div>
                        ) : filteredConsoleEntries.slice(0, 8).map((entry, index) => {
                          const level = normalizeDiagnosticLevel(entry.level);
                          const source = `${entry.file ?? '无文件'}${entry.line ? `:${entry.line}` : ''}`;
                          return (
                            <article
                              className={cx(
                                'mb-2 grid gap-2 rounded-[12px] border border-white/[0.08] bg-white/[0.028] px-2.5 py-2.5 text-[11px] text-[var(--text-secondary)] last:mb-0',
                                level === 'error' && diagnosticsClass.entryError,
                                level === 'warn' && diagnosticsClass.entryWarn,
                              )}
                              key={`${entry.timestamp}-${index}`}
                            >
                              <div className="flex items-center gap-2">
                                <span className={cx(diagnosticsClass.level, diagnosticLevelClass(entry.level))}>{diagnosticLevelLabel(entry.level)}</span>
                                <strong className={diagnosticsClass.subsystem}>{entry.subsystem || 'editor'}</strong>
                                <small className="ml-auto font-mono text-[10px] text-[var(--text-muted)]">{formatConsoleTime(entry.timestamp)}</small>
                              </div>
                              <p className={diagnosticsClass.message}>{entry.message}</p>
                              {(entry.file || entry.line) && <small className={diagnosticsClass.source}>{source}</small>}
                              <div className={diagnosticsClass.entryActions}>
                                <button className={surfaceClass.button} onClick={() => { void navigator.clipboard?.writeText(`[${diagnosticLevelLabel(entry.level)}] ${entry.subsystem || 'editor'} ${source}\n${entry.message}`); }}><IconCopy /> 复制</button>
                              </div>
                            </article>
                          );
                        })}
                      </div>
                    </div>
                  </section>
                </aside>
              </div>
            </div>}
          </section>

          {artifactSelection && <div className={artifactPopoverClass.root} style={{ left: artifactSelection.x, top: artifactSelection.y }}>
            {!artifactQuestionOpen ? <button className={artifactPopoverClass.button} onClick={() => setArtifactQuestionOpen(true)}><IconSparkles /> 询问 Aster 关于{artifactKindLabel(artifactSelection.kind)}</button> : <div className={artifactPopoverClass.panel}><header className={artifactPopoverClass.header}><span className={artifactPopoverClass.label}>{artifactSelection.label}</span><button className={artifactPopoverClass.closeButton} onClick={() => setArtifactSelection(null)}><IconX /></button></header><div className={artifactPopoverClass.form}><input className={artifactPopoverClass.input} autoFocus value={artifactQuestion} onChange={event => setArtifactQuestion(event.target.value)} onKeyDown={event => { if (event.key === 'Enter') submitArtifactQuestion(); if (event.key === 'Escape') setArtifactQuestionOpen(false); }} placeholder={t('artifact_ask_placeholder')} /><button className={artifactPopoverClass.submit} onClick={submitArtifactQuestion} disabled={!artifactQuestion.trim()}>{t('btn_ask')}</button></div></div>}
          </div>}
        </main>

        {aiPanelOpen ? (
          <>
            <div
              className={workspaceClass.resizeHandle}
              onMouseDown={handleResizeDown}
              role="separator"
              aria-label="调整 AI 工作区宽度"
              aria-orientation="vertical"
              aria-valuemin={360}
              aria-valuemax={460}
              aria-valuenow={aiPanelWidth}
              tabIndex={0}
              onKeyDown={event => {
                if (event.key === 'ArrowLeft') setAiPanelWidth(width => Math.min(460, width + 16));
                if (event.key === 'ArrowRight') setAiPanelWidth(width => Math.max(360, width - 16));
              }}
            />

            <aside className={workspaceClass.aiPanel} style={{ width: aiPanelWidth }} aria-label="AI 工作区">
              <div className={aiShellClass.header}>
                <div className={aiShellClass.titleWrap}>
                  <strong className={aiShellClass.title}><IconBot /> AI 工作流</strong>
                  <span className={aiStatusClass}>
                    {aiWorkspace?.status === 'thinking' || aiWorkspace?.status === 'executing' ? <IconLoader className="size-3 animate-spin" /> : <IconCheck className="size-3" />}
                    {aiStatusText}
                  </span>
                </div>
                <div className={aiShellClass.actions}>
                  <button className={toolButtonClass({ size: 'icon' })} onClick={onOpenSettings} title="AI 设置">
                    <IconSettings />
                  </button>
                  <button className={toolButtonClass({ size: 'icon' })} onClick={() => setAiPanelOpen(false)} title="收起 AI 工作区">
                    <IconChevronRight />
                  </button>
                </div>
              </div>
              <AiPanel
                projectName={shellState.project_name}
                selectedEntity={selectedId}
                selectedEntityName={selectedEntityName}
                sceneObjectCount={sceneTree.length}
                sceneObjects={sceneTree}
                onQuickAction={handleQuickAction}
                onSceneChanged={handleAiSceneChanged}
                onFocusPosition={focusOnPosition}
                chatOnly
                onWorkspaceStateChange={setAiWorkspace}
                contextualRequest={contextualRequest}
                onContextualRequestConsumed={id => setContextualRequest(current => current?.id === id ? null : current)}
                onOpenSettings={onOpenSettings}
              />
            </aside>
          </>
        ) : (
          <aside className={workspaceClass.aiRail} aria-label="AI 工作区">
            <button className={workspaceClass.aiRailButton} onClick={() => setAiPanelOpen(true)} title="打开 AI 工作区">
              <IconBot />
            </button>
            {pendingAiDecisionCount > 0 && <span className={workspaceClass.aiRailBadge}>{pendingAiDecisionCount}</span>}
            {aiWorkspace?.status === 'thinking' || aiWorkspace?.status === 'executing' ? (
              <IconLoader className="mt-1 size-4 animate-spin text-[var(--text-muted)]" />
            ) : null}
          </aside>
        )}
      </div>

      {/* Status Bar */}
      <footer className={shellClass.statusbar}>
        <div className={shellClass.statusGroup}>
          <span className={shellClass.statusItem}>{shellState.project_name || t('status_no_project')}</span>
          <span className={shellClass.statusDivider} />
          <span className={shellClass.statusItem}>{sceneTree.length} {t('label_objects')}</span>
          {selectedEntityName && <><span className={shellClass.statusDivider} /><span className={cx(shellClass.statusItem, shellClass.statusSelection)}>{t('status_selected')} {selectedEntityName}</span></>}
        </div>
        <div className={shellClass.statusGroup}>
          {shellState.scene_dirty ? (
            <span className={cx(shellClass.statusItem, shellClass.statusDirty)}><span className={shellClass.statusDot} />{t('status_unsaved')}</span>
          ) : (
            <span className={cx(shellClass.statusItem, shellClass.statusSaved)}>{t('status_saved')}</span>
          )}
          <span className={shellClass.statusDivider} />
          <span className={cx(shellClass.statusItem, shellClass.version)}>v0.1.0</span>
        </div>
      </footer>

      {/* Close Project Dialog */}
      {showCloseDialog && shellState && (
        <CloseProjectDialog
          projectName={shellState.project_name || 'project'}
          onSave={async () => {
            setShowCloseDialog(false);
            await rpc('shell/save_scene').catch(() => {});
            onCloseProject();
          }}
          onDiscard={() => { setShowCloseDialog(false); onCloseProject(); }}
          onCancel={() => setShowCloseDialog(false)}
        />
      )}
    </div>
  );
}
