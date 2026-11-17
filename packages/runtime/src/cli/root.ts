import { lstatSync, realpathSync, statSync } from 'node:fs';
import { basename, dirname, isAbsolute, join, relative, resolve, sep } from 'node:path';

/** A path given to `nodarium --root` that leads outside the root */
export class OutsideRootError extends Error {}

/**
 * Where the paths of the command line lead. Without a root a path is resolved against the
 * working directory as it always was. With one, against the root, and a path that leads
 * outside it, by `..`, as an absolute path or through a link, is refused.
 */
export class PathRule {
  /** the root with its links resolved, undefined without one */
  readonly root?: string;

  constructor(root?: string) {
    if (root === undefined) return;
    let real: string;
    try {
      real = realpathSync(resolve(root));
    } catch {
      throw new OutsideRootError(`--root ${root} does not exist`);
    }
    if (!statSync(real).isDirectory()) throw new OutsideRootError(`--root ${root} is no folder`);
    this.root = real;
  }

  /** The absolute path of `path`, throws an {@link OutsideRootError} when it leads outside */
  resolve(path: string): string {
    if (this.root === undefined) return resolve(path);
    const full = resolve(this.root, path);
    if (!this.contains(this.real(full, path))) {
      throw new OutsideRootError(`${path} is outside the root ${this.root}`);
    }
    return full;
  }

  /** Where `full` leads: the deepest part of it that exists with its links resolved, then the rest */
  private real(full: string, path: string): string {
    const rest: string[] = [];
    for (let at = full;; at = dirname(at)) {
      try {
        return join(realpathSync(at), ...rest);
      } catch (error) {
        const code = (error as NodeJS.ErrnoException).code;
        if (code !== 'ENOENT' && code !== 'ENOTDIR') throw error;
      }
      // a link to something that doesn't exist yet would be followed by the write
      if (lstatExists(at)) throw new OutsideRootError(`${path} is a link to nothing`);
      if (dirname(at) === at) return full;
      rest.unshift(basename(at));
    }
  }

  private contains(path: string) {
    const rel = relative(this.root!, path);
    return rel === '' || (rel !== '..' && !rel.startsWith(`..${sep}`) && !isAbsolute(rel));
  }
}

function lstatExists(path: string) {
  try {
    lstatSync(path);
    return true;
  } catch {
    return false;
  }
}
