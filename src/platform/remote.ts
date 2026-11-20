/**
 * Remote development is not part of GitRune. Everything here answers as a local window
 * so the settings, welcome and window plumbing keep one code path: no window is remote,
 * every location is a local path, and connecting is a no-op that reports success.
 *
 * The previous implementation drove `remote_*` IPC commands over SSH, WSL and dev
 * containers; that backend was removed with the rest of the remote support.
 */
import { create } from "zustand";

export type RemoteTarget = { kind: string; [key: string]: unknown };

export interface RemoteSession {
  target: RemoteTarget;
  root: string;
  [key: string]: unknown;
}

/** Always `null`: no window runs on a remote host. */
export const useRemote = create<{ session: RemoteSession | null }>(() => ({ session: null }));

/** Always `null`: nothing is connecting. */
export const useRemoteProgress = create<{
  location: string | null;
  stage: string;
  fraction: number;
  progress?: { stage: string; fraction: number | null } | null;
}>(() => ({ location: null, stage: "idle", fraction: 0, progress: null }));

export function isRemoteWindow(): boolean {
  return false;
}

export function hostOs(): string | null {
  return null;
}

export function targetName(..._args: unknown[]): string {
  return "";
}

export function sameTarget(...args: unknown[]): boolean {
  return args[0] === args[1];
}

/** No remote locations exist, so a path parses as a local one. */
export function remoteLocation(..._args: unknown[]): RemoteTarget | null {
  return null;
}

export function parseRemoteLocation(..._args: unknown[]): RemoteTarget | null {
  return null;
}

export async function resolveRemoteLocation(..._args: unknown[]): Promise<RemoteTarget | null> {
  return null;
}

export function projectLocation(root: string | null | undefined): string | undefined {
  return root ?? undefined;
}

export function remoteRouting(): unknown {
  return null;
}

export function encodeArgs(args: string[]): string[] {
  return args;
}

export function enableRemote(): void {}

export function disableRemote(): void {}

export async function restoreRemote(): Promise<boolean> {
  return false;
}

export async function connectRemote(..._args: unknown[]): Promise<boolean> {
  return false;
}

export async function disconnectRemote(..._args: unknown[]): Promise<void> {}

export interface RecentRemote {
  target: RemoteTarget;
  path: string;
  [key: string]: unknown;
}

export function recentRemotes(): RecentRemote[] {
  return [];
}

export function rememberRemote(_entry: RecentRemote): void {}

export function forgetRemote(..._args: unknown[]): void {}

export function initializeRemoteTrustMigration(): void {}

export function takeLegacyRemoteTrust(): null {
  return null;
}
