export type ServerPathSource = "config" | "bundled" | "path";

export interface ServerPathResult {
  path: string;
  source: ServerPathSource;
}

export interface ResolveServerPathOptions {
  extensionPath: string;
  configuredPath: string | undefined;
  platform: string;
  arch: string;
  pathEnv: string | undefined;
  fileExists: (path: string) => boolean;
}

export function bundledBinaryFileName(platform: string): string {
  return platform === "win32" ? "yps-lsp.exe" : "yps-lsp";
}

function pathSeparator(platform: string): string {
  return platform === "win32" ? "\\" : "/";
}

function pathDelimiter(platform: string): string {
  return platform === "win32" ? ";" : ":";
}

function joinPath(platform: string, ...parts: string[]): string {
  const sep = pathSeparator(platform);
  const trimSep = new RegExp(`[\\\\/]+$`);
  const trimBoth = new RegExp(`^[\\\\/]+|[\\\\/]+$`, "g");
  return parts
    .filter((part) => part.length > 0)
    .map((part, index) => (index === 0 ? part.replace(trimSep, "") : part.replace(trimBoth, "")))
    .join(sep);
}

export function bundledBinaryRelativePath(platform: string, arch: string): string {
  return joinPath(platform, "bin", `${platform}-${arch}`, bundledBinaryFileName(platform));
}

function findOnPath(
  binaryName: string,
  platform: string,
  pathEnv: string | undefined,
  fileExists: (path: string) => boolean
): string | undefined {
  if (!pathEnv) {
    return undefined;
  }
  for (const dir of pathEnv.split(pathDelimiter(platform))) {
    if (!dir) {
      continue;
    }
    const candidate = joinPath(platform, dir, binaryName);
    if (fileExists(candidate)) {
      return candidate;
    }
  }
  return undefined;
}

export function resolveServerPath(
  options: ResolveServerPathOptions
): ServerPathResult | undefined {
  const configured = options.configuredPath?.trim();
  if (configured) {
    return { path: configured, source: "config" };
  }

  const bundledPath = joinPath(
    options.platform,
    options.extensionPath,
    bundledBinaryRelativePath(options.platform, options.arch)
  );
  if (options.fileExists(bundledPath)) {
    return { path: bundledPath, source: "bundled" };
  }

  const binaryName = bundledBinaryFileName(options.platform);
  const pathMatch = findOnPath(binaryName, options.platform, options.pathEnv, options.fileExists);
  if (pathMatch) {
    return { path: pathMatch, source: "path" };
  }

  return undefined;
}
