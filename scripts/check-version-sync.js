const fs = require('fs');
const path = require('path');

const repoRoot = path.resolve(__dirname, '..');
const cargoToml = fs.readFileSync(path.join(repoRoot, 'Cargo.toml'), 'utf8');
const packageSection = cargoToml.match(/\[package\]([\s\S]*?)(?:\n\[[^\]]+\]|$)/);

if (!packageSection) {
  console.error('Missing [package] section in Cargo.toml');
  process.exit(1);
}

const cargoVersionMatch = packageSection[1].match(/^version\s*=\s*"([^"]+)"\s*$/m);

if (!cargoVersionMatch) {
  console.error('Missing package.version in Cargo.toml');
  process.exit(1);
}

const cargoVersion = cargoVersionMatch[1];
const packageFiles = [
  path.join(repoRoot, 'package.json'),
  ...fs
    .readdirSync(path.join(repoRoot, 'npm'), { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => path.join(repoRoot, 'npm', entry.name, 'package.json')),
];

const mismatches = [];
const rootPackage = JSON.parse(fs.readFileSync(packageFiles[0], 'utf8'));

for (const packageFile of packageFiles) {
  const manifest = JSON.parse(fs.readFileSync(packageFile, 'utf8'));

  if (manifest.version !== cargoVersion) {
    mismatches.push(`${path.relative(repoRoot, packageFile)} version=${manifest.version} expected=${cargoVersion}`);
  }
}

for (const [name, version] of Object.entries(rootPackage.optionalDependencies || {})) {
  if (version !== cargoVersion) {
    mismatches.push(`package.json optionalDependencies.${name}=${version} expected=${cargoVersion}`);
  }
}

if (mismatches.length > 0) {
  console.error('Version sync check failed:');
  for (const mismatch of mismatches) {
    console.error(`- ${mismatch}`);
  }
  process.exit(1);
}

console.log(`Version sync OK: ${cargoVersion}`);