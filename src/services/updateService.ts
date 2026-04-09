import { invoke } from '@tauri-apps/api/core';
import { GitHubRelease, UpdateInfo } from '../types/update';

const GITHUB_RELEASES_API_URL = 'https://api.github.com/repos/wuqi-y/auto-cursor-releases/releases/latest';
const GITHUB_RELEASES_PAGE_URL = 'https://github.com/wuqi-y/auto-cursor-releases/releases/';

function normalizeVersion(version: string): string {
  return version.trim().replace(/^v/i, '');
}

/**
 * 检查是否需要更新
 */
export function needsUpdate(currentVersion: string, latestVersion: string): boolean {
  const currentParts = normalizeVersion(currentVersion).split('.');
  const latestParts = normalizeVersion(latestVersion).split('.');

  for (let i = 0; i < Math.max(currentParts.length, latestParts.length); i++) {
    const currentPart = parseInt(currentParts[i] || '0', 10);
    const latestPart = parseInt(latestParts[i] || '0', 10);

    if (latestPart > currentPart) {
      return true;
    }
    if (latestPart < currentPart) {
      return false;
    }
  }

  return false;
}

export async function getCurrentVersion(): Promise<string> {
  try {
    return await invoke<string>('get_app_version');
  } catch (error) {
    console.error('Failed to get app version:', error);
    return '0.0.0';
  }
}

export async function fetchLatestVersion(): Promise<GitHubRelease> {
  const response = await fetch(GITHUB_RELEASES_API_URL, {
    method: 'GET',
    headers: {
      Accept: 'application/vnd.github+json',
    },
  });

  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }

  return await response.json();
}

export async function checkForUpdates(): Promise<UpdateInfo> {
  try {
    const [currentVersion, release] = await Promise.all([
      getCurrentVersion(),
      fetchLatestVersion(),
    ]);

    const latestVersion = normalizeVersion(release.tag_name || release.name || '0.0.0');

    return {
      version: latestVersion,
      description: release.body || '暂无更新说明',
      updateDate: release.published_at,
      isForceUpdate: false,
      updateUrl: release.html_url || GITHUB_RELEASES_PAGE_URL,
      hasUpdate: needsUpdate(currentVersion, latestVersion),
    };
  } catch (error) {
    console.error('Failed to check for updates:', error);
    throw error;
  }
}

export async function openUpdateUrl(url: string): Promise<void> {
  try {
    await invoke<void>('open_update_url', { url });
  } catch (error) {
    console.error('Failed to open update URL:', error);
    window.open(url, '_blank');
  }
}
