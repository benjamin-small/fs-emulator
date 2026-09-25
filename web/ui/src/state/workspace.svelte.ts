import { createContext } from "svelte";
import { parseRoute } from "../core/route";
import { stepFocusOffset } from "../core/stepFocus";
import { DEFAULT_FAMILY, FAMILY_IDS, ROUTE_ALIASES } from "../fs";
import type { FsFamilyId } from "../fs/adapter";
import { MOUNT, Vfs } from "../shell/vfs";
import { LayersStore } from "./layers.svelte";
import { ScenarioRunner } from "./scenarios.svelte";
import { SelectionStore } from "./selection.svelte";
import { VolumeStore } from "./volume.svelte";

/**
 * One family's tab: its own disk and timeline, selection, layers, running lesson, and the
 * shell's working directory in it. Components reach it through `getWorkspace()`, which the
 * nearest `WorkspaceView` or `WorkspaceScope` sets, never through a module singleton, so each
 * tab's components talk to their own stores.
 */
export class Workspace {
  readonly id: FsFamilyId;
  readonly vfs = new Vfs();
  readonly selection: SelectionStore;
  readonly volume: VolumeStore;
  readonly layers: LayersStore;
  readonly scenarios: ScenarioRunner;
  private readonly registry: WorkspaceRegistry;

  constructor(id: FsFamilyId, registry: WorkspaceRegistry) {
    this.id = id;
    this.registry = registry;
    // In dependency order. The selection's callback reads `this.volume` only when it runs,
    // after the constructor has assigned it.
    this.selection = new SelectionStore(() => this.volume.clearMessages());
    this.volume = new VolumeStore(id, this.selection, () => { this.vfs.cwd = MOUNT; });
    this.layers = new LayersStore(this.volume, this.selection);
    this.scenarios = new ScenarioRunner(this.volume, this.selection);
  }

  /** Whether this is the tab on screen. */
  get active(): boolean {
    return this.registry.activeId === this.id;
  }

  /**
   * Scrub to `step` and recenter the dump on what that step changed. Running an operation
   * never moves the dump — only explicit navigation does — so this is called from the
   * Timeline's controls and from App.svelte's `[`/`]` shortcuts. A step with no changes (or an
   * index off either end of the history) leaves the dump alone.
   */
  goToStep(step: number) {
    this.volume.seek(step);
    const offset = stepFocusOffset(this.volume.history[step]);
    if (offset !== null) this.selection.jumpTo(offset);
  }
}

/** The page's workspaces, one per family, each created the first time it is activated. */
export class WorkspaceRegistry {
  activeId = $state<FsFamilyId>(DEFAULT_FAMILY);
  /** Every workspace created so far, in creation order. None is ever disposed. */
  opened = $state.raw<readonly Workspace[]>([]);

  constructor(initial: FsFamilyId) {
    this.activate(initial);
  }

  get active(): Workspace {
    return this.opened.find((w) => w.id === this.activeId)!;
  }

  /** Show `id`'s workspace, creating it first if this is its first time. An event handler or
   *  the constructor calls this, never a getter or a `$derived`: creating a workspace writes
   *  state. */
  activate(id: FsFamilyId): Workspace {
    let ws = this.opened.find((w) => w.id === id);
    if (!ws) {
      ws = new Workspace(id, this);
      this.opened = [...this.opened, ws];
    }
    this.activeId = id;
    return ws;
  }
}

/** The page's registry: the tab the URL hash names, else the default family's. */
export const workspaces = new WorkspaceRegistry(parseRoute(location.hash, FAMILY_IDS, ROUTE_ALIASES) ?? DEFAULT_FAMILY);

/** The workspace of the tab a component belongs to; `setWorkspace` is called by
 *  `WorkspaceView` and `WorkspaceScope` during their initialisation. */
export const [getWorkspace, setWorkspace] = createContext<Workspace>();
