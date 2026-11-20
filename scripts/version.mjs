import fs from 'node:fs';
import path from 'node:path';
import { execSync } from 'node:child_process';

const root = process.cwd();
const releasefilePath = path.join(root, 'Releasefile');

function parseBool(val, fallback) {
  if (val == null) return fallback;
  const s = String(val).trim().toLowerCase();
  if (s === 'true' || s === 'yes' || s === '1') return true;
  if (s === 'false' || s === 'no' || s === '0') return false;
  return fallback;
}

function parseReleasefile() {
  if (!fs.existsSync(releasefilePath)) {
    return {
      declaredVersion: 'auto',
      buildWin: true,
      buildMac: false,
      notes: 'See the assets to download and install this version.',
    };
  }
  const raw = fs.readFileSync(releasefilePath, 'utf8').replace(/\r\n/g, '\n');
  const lines = raw.split('\n');
  const headers = {};
  let bodyStart = 0;

  while (bodyStart < lines.length) {
    const line = lines[bodyStart].trim();
    if (!line) {
      bodyStart += 1;
      break;
    }
    const m = /^([a-zA-Z0-9_]+)\s*:\s*(.*)$/.exec(line);
    if (!m) break;
    headers[m[1].toLowerCase()] = m[2].trim();
    bodyStart += 1;
  }

  while (bodyStart < lines.length && lines[bodyStart].trim() === '') {
    bodyStart += 1;
  }

  const notes = lines.slice(bodyStart).join('\n').trim() || 'See the assets to download and install this version.';
  return {
    declaredVersion: headers.version || 'auto',
    buildWin: parseBool(headers.build_win, true),
    buildMac: parseBool(headers.build_mac, false),
    notes,
  };
}

function parseSemver(v) {
  const m = /^v?(\d+)\.(\d+)\.(\d+)$/.exec(v.trim());
  if (!m) return null;
  return [Number(m[1]), Number(m[2]), Number(m[3])];
}

function cmpSemver(a, b) {
  for (let i = 0; i < 3; i += 1) {
    if (a[i] !== b[i]) return a[i] - b[i];
  }
  return 0;
}

function bumpSemver([maj, min, pat], kind) {
  if (kind === 'major') return `${maj + 1}.0.0`;
  if (kind === 'minor') return `${maj}.${min + 1}.0`;
  return `${maj}.${min}.${pat + 1}`;
}

function latestGitTagVersion() {
  try {
    const out = execSync('git tag --list "v*" --sort=-v:refname', { cwd: root, encoding: 'utf8' });
    for (const line of out.split(/\r?\n/)) {
      const parsed = parseSemver(line);
      if (parsed) return parsed;
    }
  } catch {
    // Ignore if git tags are unavailable
  }
  return null;
}

function currentCargoVersion() {
  const cargo = fs.readFileSync(path.join(root, 'Cargo.toml'), 'utf8');
  const m = /^version\s*=\s*"([^"]+)"/m.exec(cargo);
  return (m && parseSemver(m[1])) || [0, 1, 0];
}

function resolveVersion(declaredVersion) {
  const args = process.argv.slice(2);
  const flag = (name) => {
    const idx = args.indexOf(name);
    return idx >= 0 ? args[idx + 1] : undefined;
  };

  const explicit = flag('--version') || process.env.RELEASE_VERSION;
  if (explicit && parseSemver(explicit)) {
    return parseSemver(explicit).join('.');
  }

  const refName = process.env.GITHUB_REF_NAME || '';
  if (process.env.GITHUB_REF_TYPE === 'tag' && parseSemver(refName)) {
    return parseSemver(refName).join('.');
  }

  const bumpMode = (flag('--bump') || process.env.BUMP_MODE || 'auto').toLowerCase();
  const latest = latestGitTagVersion();
  const base = latest || currentCargoVersion();

  if (bumpMode === 'major' || bumpMode === 'minor' || bumpMode === 'patch') {
    return bumpSemver(base, bumpMode);
  }

  if (declaredVersion !== 'auto' && parseSemver(declaredVersion)) {
    const declared = parseSemver(declaredVersion);
    if (!latest || cmpSemver(declared, latest) > 0) {
      return declared.join('.');
    }
  }

  return latest ? bumpSemver(latest, 'patch') : base.join('.');
}

function syncVersion(version) {
  const cargoPath = path.join(root, 'Cargo.toml');
  const cargo = fs.readFileSync(cargoPath, 'utf8');
  fs.writeFileSync(
    cargoPath,
    cargo.replace(/^(version\s*=\s*")[^"]+(")/m, `$1${version}$2`),
  );

  const tauriConfPath = path.join(root, 'apps/desktop/src-tauri/tauri.conf.json');
  const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, 'utf8'));
  tauriConf.version = version;
  fs.writeFileSync(tauriConfPath, `${JSON.stringify(tauriConf, null, 2)}\n`);

  for (const pkgRel of ['package.json', 'apps/desktop/package.json', 'apps/web/package.json']) {
    const pkgPath = path.join(root, pkgRel);
    if (!fs.existsSync(pkgPath)) continue;
    const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));
    pkg.version = version;
    fs.writeFileSync(pkgPath, `${JSON.stringify(pkg, null, 2)}\n`);
  }
}

const { declaredVersion, buildWin, buildMac, notes } = parseReleasefile();
const version = resolveVersion(declaredVersion);
if (!process.argv.includes('--dry-run')) {
  syncVersion(version);
}

const matrix = [];
if (buildWin) matrix.push({ platform: 'windows-latest', args: '' });
if (buildMac) matrix.push({ platform: 'macos-latest', args: '--target universal-apple-darwin' });

if (process.env.GITHUB_OUTPUT) {
  const delim = `SNIPPY_EOF_${Date.now()}`;
  fs.appendFileSync(
    process.env.GITHUB_OUTPUT,
    [
      `version=${version}`,
      `build_win=${buildWin}`,
      `build_mac=${buildMac}`,
      `matrix=${JSON.stringify(matrix)}`,
      `notes<<${delim}`,
      notes,
      delim,
      '',
    ].join('\n'),
  );
}

console.log(`Snippy release version: ${version} (build_win=${buildWin}, build_mac=${buildMac})`);
