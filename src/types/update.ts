export interface GitHubRelease {
  tag_name: string;
  name: string;
  body: string;
  html_url: string;
  published_at: string;
  prerelease: boolean;
  draft: boolean;
}

export interface UpdateInfo {
  version: string;
  description: string;
  updateDate: string;
  isForceUpdate: boolean;
  updateUrl: string;
  hasUpdate: boolean;
}
