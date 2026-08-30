import type { PanelSession } from './panelSession';

/**
 * PanelController — manages PanelSession lifecycle (66.md §33).
 * WorkspaceStore remains thin: layout, active side, clipboard.
 * Controller owns reload/navigate/close semantics with generation safety.
 */
export class PanelController {
  private session: PanelSession;
  constructor(session: PanelSession) { this.session = session; }

  get generation() { return this.session.generation; }

  /** Reload without blanking — preserve snapshot, state refreshing (66.md §6, §12, §36). */
  prepareReload(): { generation: number; signal: AbortSignal; preserve: boolean } {
    const { generation, signal } = this.session.newRequest();
    const preserve = this.session.entries.length > 0;
    this.session.status = preserve ? 'refreshing' : 'loading';
    this.session.error = null;
    return { generation, signal, preserve };
  }

  /** Navigate — abort previous, new generation (66.md §9). */
  prepareNavigate(): { generation: number; signal: AbortSignal } {
    const { generation, signal } = this.session.newRequest();
    // Keep previous entries visible until new data arrives (no blank)
    this.session.status = 'loading';
    this.session.error = null;
    return { generation, signal };
  }

  close(): void {
    // 1 mark closed 2 bump generation 3 abort 4 clear
    this.session.markClosed();
  }

  dispose(): void {
    this.session.dispose();
  }
}
