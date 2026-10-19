/**
 * ============================================================================
 *  EGG FORMAT — TypeScript types for Pelican / Pterodactyl egg files
 * ============================================================================
 *
 * "Eggs" are the portable JSON (Pelican also: YAML) server-configuration
 * format used by:
 *   - Pterodactyl Panel  (github.com/pterodactyl/panel)
 *   - Pelican Panel      (github.com/pelican-dev/panel, a Pterodactyl fork)
 *
 * Format revisions, identified by `meta.version`:
 *   - "PTDL_v1" — Pterodactyl legacy: single `image` or `images[]` list.
 *   - "PTDL_v2" — Pterodactyl current: labelled `docker_images` map, single
 *                 `startup` string, pipe-joined `rules`, `field_type`.
 *   - "PLCN_v1" / "PLCN_v2" — early Pelican exports, same shape as PTDL_v2.
 *   - "PLCN_v3" — current Pelican: `uuid`, `icon`, `tags`,
 *                 `startup_commands` map, `rules` as an array, variable
 *                 `sort`, no `field_type`.
 * Pelican imports all five and upgrades them to PLCN_v3
 * (app/Services/Eggs/Sharing/EggImporterService.php); anything else is
 * rejected with "The file format is not recognized."
 * Pterodactyl only accepts PTDL_v1/PTDL_v2.
 *
 * This app's native format is PLCN_v3 (`PelicanEggV3`); it imports every
 * version (`AnyEgg`) and exports PLCN_v3 or PTDL_v2.
 *
 * ----------------------------------------------------------------------------
 * WHO CONSUMES EACH FIELD (short tags used below):
 *   [Panel]   admin UI / importer / exporter / end-user frontend.
 *   [Wings]   the Go daemon that builds/boots the container and rewrites
 *             config files inside it.
 *   [Install] the throwaway container `scripts.installation` runs in.
 *
 * SOURCE REFERENCES (pelican-dev/panel@main):
 *  - Export:  app/Services/Eggs/Sharing/EggExporterService.php
 *  - Import:  app/Services/Eggs/Sharing/EggImporterService.php
 *  - Models:  app/Models/Egg.php, app/Models/EggVariable.php
 *  - Wings:   pterodactyl/wings@develop parser/, environment/
 * ============================================================================
 */

// ============================================================================
// Shared bits
// ============================================================================

export const EGG_VERSIONS = [
  'PTDL_v1',
  'PTDL_v2',
  'PLCN_v1',
  'PLCN_v2',
  'PLCN_v3'
] as const;
export type EggVersion = (typeof EGG_VERSIONS)[number];

export const PELICAN_COMMENT =
  'DO NOT EDIT: FILE GENERATED AUTOMATICALLY BY PANEL';
export const PTERODACTYL_COMMENT =
  'DO NOT EDIT: FILE GENERATED AUTOMATICALLY BY PTERODACTYL PANEL - PTERODACTYL.IO';

export interface EggMeta<V extends EggVersion = EggVersion> {
  version: V;
  /**
   * URL the panel polls for a newer version of this egg (admin "Update"
   * button). Optional bookkeeping, `null` when untracked. [Panel] only.
   */
  update_url?: string | null;
}

/**
 * Feature flags the stock Pelican frontend reacts to. Each mounts a modal
 * that watches console output (e.g. `eula` offers to accept Mojang's EULA,
 * `java_version` reacts to UnsupportedClassVersionError, `steam_disk_space`
 * to SteamCMD's 0x202 error). Any other string is accepted but a no-op.
 */
export const FEATURES = [
  'eula',
  'java_version',
  'pid_limit',
  'gsl_token',
  'steam_disk_space'
] as const;
export type EggFeature = (typeof FEATURES)[number] | (string & {});

/**
 * Env names Pelican reserves (EggVariable::RESERVED_ENV_NAMES); a variable
 * using one is rejected. Matching is exact-case.
 */
export const RESERVED_ENV_NAMES = [
  'P_SERVER_UUID',
  'P_SERVER_ALLOCATION_LIMIT',
  'SERVER_MEMORY',
  'SERVER_IP',
  'SERVER_PORT',
  'ENV',
  'HOME',
  'USER',
  'STARTUP',
  'MODIFIED_STARTUP',
  'SERVER_UUID',
  'UUID',
  'INTERNAL_IP',
  'HOSTNAME',
  'TERM',
  'LANG',
  'PWD',
  'TZ',
  'TIMEZONE'
] as const;
export type ReservedEnvName = (typeof RESERVED_ENV_NAMES)[number];

/** Wings config parsers. `yml` is an alias of `yaml`. */
export const PARSERS = [
  'file',
  'yaml',
  'yml',
  'properties',
  'ini',
  'json',
  'xml'
] as const;
export type EggConfigParser = (typeof PARSERS)[number];

// ============================================================================
// `config.*` (shared by every version)
// ============================================================================

/**
 * One `config.files` entry. [Wings] rewrites the file right before every
 * (re)start.
 */
export interface EggConfigFile {
  /**
   * Decoder for the file.
   *  - "file": line-based; each `find` key is a line PREFIX and the whole
   *    matching line is replaced by the value.
   *  - "yaml"/"yml", "json", "xml", "ini", "properties": structured; keys
   *    use dot notation, `*` is a single-level wildcard (`servers.*.port`).
   */
  parser: EggConfigParser;
  /** Key -> value to write. See `EggConfigFindValue`. */
  find: Record<string, EggConfigFindValue>;
}

export type EggConfigFindScalar = string | number | boolean | null;

/**
 * A find value is either written as-is (type preserved: `"25565"` and
 * `25565` differ in JSON/YAML targets) or, as an object, a conditional
 * replace map `{ ifCurrentValue: replacement }` applied to the existing
 * value. A map key prefixed with `regex:` is a regex match.
 *
 * Placeholders inside strings: `{{server.*}}` (e.g.
 * `{{server.allocations.default.port}}`, `{{server.build.memory_limit}}`),
 * `{{env.NAME}}` for egg variables and `{{config.*}}` for Wings' own config.
 */
export type EggConfigFindValue =
  | EggConfigFindScalar
  | Record<string, EggConfigFindScalar>;

/** Boot detection: flips a server from "starting" to "running". [Wings] */
export interface EggConfigStartup {
  /**
   * Console text that marks boot as done; an array means "any of these".
   * Entries starting with `regex:` are regular expressions, everything else
   * is a case-sensitive substring match. Matched verbatim (no trimming).
   */
  done: string | string[];
  /** Ignored by Wings; kept for compatibility. */
  userInteraction?: string[];
  /** Strip ANSI escape codes before matching `done`. */
  strip_ansi?: boolean;
}

/** Ignored by Wings entirely; only kept for compatibility. */
export interface EggConfigLogs {
  custom?: boolean;
  location?: string;
}

/** Decoded config. */
export interface EggConfig {
  files: Record<string, EggConfigFile> | null;
  startup: EggConfigStartup | null;
  logs: EggConfigLogs | null;
  /**
   * Graceful stop: written to the process' stdin, or, with a leading `^`,
   * sent as a signal instead (`^C` = SIGINT).
   * `null` falls back to Wings' default stop.
   */
  stop: string | null;
}

/**
 * Config as found in files: JSON exports encode `files`/`startup`/`logs` as
 * JSON strings, YAML exports (Pelican) use objects. Either may be null.
 */
export interface EggConfigRaw {
  files: string | Record<string, EggConfigFile> | null;
  startup: string | EggConfigStartup | null;
  logs: string | EggConfigLogs | null;
  stop: string | null;
}

/** [Install] script; runs on install / explicit reinstall only. */
export interface EggInstallScript {
  /** Shell script body, or null to run nothing. Egg variables are real env vars. */
  script: string | null;
  /** Installer image, e.g. "ghcr.io/pelican-eggs/installers:debian". */
  container: string;
  /** Shell used to run `script`, typically "bash" or "ash". */
  entrypoint: string;
}

// ============================================================================
// PLCN_v3 (canonical)
// ============================================================================

export interface PelicanVariable {
  /** Label shown to end users. 1–255 chars. */
  name: string;
  /** Help text under the input. */
  description: string;
  /**
   * Env var name injected into the container and usable as `{{NAME}}` in
   * startup commands and `{{env.NAME}}` in config files. alphaDash
   * (`[A-Za-z0-9_-]`), 1–255 chars, not in `RESERVED_ENV_NAMES`, unique
   * per egg.
   */
  env_variable: string;
  /** Initial value; must satisfy `rules`. */
  default_value: string;
  /** Visible to non-admins. Defaults to false. Not a security boundary. */
  user_viewable: boolean;
  /** Editable by non-admins (only meaningful if viewable). Defaults to false. */
  user_editable: boolean;
  /**
   * Laravel validation rules, e.g. ["required", "string", "max:20"].
   * `boolean` or `in:0,1` makes the frontend render a toggle.
   */
  rules: string[];
  /** Display order (ascending), null for unsorted. */
  sort: number | null;
}

/** Key order matches Pelican's EggExporterService. */
export interface PelicanEggV3 {
  _comment: string;
  meta: EggMeta<'PLCN_v3'>;
  /** ISO-8601 export timestamp, informational. */
  exported_at?: string;
  /** Max 255 chars. */
  name: string;
  /** Author email (validated as email). */
  author: string;
  /** 36-char UUID of the egg. */
  uuid: string;
  description: string | null;
  /** `data:image/(png|jpeg|webp);base64,...` or null. */
  icon?: string | null;
  /**
   * Older PLCN exports have `image: null` here instead of `icon`; a value
   * starting with `data:` is treated as the icon on import.
   */
  image?: string | null;
  tags: string[];
  features: EggFeature[] | null;
  /** Label -> image reference; at least one entry, order = display order. */
  docker_images: Record<string, string>;
  /** Glob patterns hidden from users' file manager. */
  file_denylist: string[];
  /**
   * Label -> startup command; at least one entry, values distinct, the
   * default label is "Default". `{{VAR}}` interpolates egg variables.
   */
  startup_commands: Record<string, string>;
  config: EggConfigRaw;
  scripts: { installation: EggInstallScript };
  variables: PelicanVariable[];
}

// ============================================================================
// Import-only legacy shapes
// ============================================================================

/** PTDL_v2 / PLCN_v1 / PLCN_v2 variable. */
export interface LegacyVariable {
  name: string;
  description: string;
  env_variable: string;
  default_value: string;
  user_viewable: boolean;
  user_editable: boolean;
  /** Pipe-joined rules, e.g. "required|string|max:20". */
  rules: string;
  /** Always "text" in practice; dropped on upgrade. */
  field_type?: string;
}

/** PTDL_v2, PLCN_v1 and PLCN_v2 share this shape. */
export interface LegacyEggV2<
  V extends 'PTDL_v2' | 'PLCN_v1' | 'PLCN_v2' = 'PTDL_v2'
> {
  _comment: string;
  meta: EggMeta<V>;
  exported_at?: string;
  name: string;
  author: string;
  description: string | null;
  features: EggFeature[] | null;
  docker_images: Record<string, string>;
  file_denylist: string[];
  /** Single startup command; becomes `startup_commands.Default`. */
  startup: string;
  config: EggConfigRaw;
  scripts: { installation: EggInstallScript };
  variables: LegacyVariable[];
}

export type PterodactylEggV2 = LegacyEggV2;

/** PTDL_v1: `image` or `images[]` instead of `docker_images`. */
export interface PterodactylEggV1 extends Omit<
  LegacyEggV2,
  'meta' | 'docker_images' | 'features' | 'file_denylist'
> {
  meta: EggMeta<'PTDL_v1'>;
  image?: string;
  images?: string[];
  features?: EggFeature[] | null;
  file_denylist?: string[];
}

export type AnyEgg =
  | PterodactylEggV1
  | PterodactylEggV2
  | LegacyEggV2<'PLCN_v1'>
  | LegacyEggV2<'PLCN_v2'>
  | PelicanEggV3;
