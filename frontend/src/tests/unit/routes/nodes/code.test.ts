import { describe, expect, it } from 'vitest';
import { dockerCompose, dockerRun } from '$routes/nodes/[uuid]/code.svelte';

// Vite's `define` replaces `__version__` with the npm package version.
const image = `ghcr.io/profiidev/smaug/smaug-wings:${process.env.npm_package_version}`;

describe.each([
  ['dockerCompose', dockerCompose],
  ['dockerRun', dockerRun]
])('%s', (_name, gen) => {
  it('includes the token and the versioned wings image', () => {
    const out = gen('secret-token');
    expect(out).toContain('TOKEN=secret-token');
    expect(out).toContain(image);
  });

  it('exposes the wings port and the docker socket', () => {
    const out = gen('t');
    expect(out).toContain('8000:8000');
    expect(out).toContain('/var/run/docker.sock:/var/run/docker.sock');
  });
});
