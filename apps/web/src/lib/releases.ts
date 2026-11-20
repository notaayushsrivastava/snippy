export const RELEASES_PAGE_URL = 'https://github.com/notaayushsrivastava/snippy/releases/latest';
const RELEASES_API_URL = 'https://api.github.com/repos/notaayushsrivastava/snippy/releases/latest';

export interface WindowsReleaseInfo {
  version: string | null;
  windowsDownloadUrl: string;
  assetName: string | null;
}

interface GitHubAsset {
  name: string;
  browser_download_url: string;
}

interface GitHubReleaseResponse {
  tag_name?: string;
  assets?: GitHubAsset[];
}

export async function fetchLatestWindowsRelease(signal?: AbortSignal): Promise<WindowsReleaseInfo> {
  try {
    const res = await fetch(RELEASES_API_URL, {
      headers: { Accept: 'application/vnd.github+json' },
      signal,
    });
    if (!res.ok) {
      return { version: null, windowsDownloadUrl: RELEASES_PAGE_URL, assetName: null };
    }
    const data = (await res.json()) as GitHubReleaseResponse;
    const assets = data.assets ?? [];
    const exeAsset = assets.find((a) => /setup\.exe$|\.exe$/i.test(a.name) && !/\.sig$/i.test(a.name));
    const msiAsset = assets.find((a) => /\.msi$/i.test(a.name) && !/\.sig$/i.test(a.name));
    const chosen = exeAsset ?? msiAsset ?? null;

    return {
      version: data.tag_name ?? null,
      windowsDownloadUrl: chosen?.browser_download_url ?? RELEASES_PAGE_URL,
      assetName: chosen?.name ?? null,
    };
  } catch {
    return { version: null, windowsDownloadUrl: RELEASES_PAGE_URL, assetName: null };
  }
}
