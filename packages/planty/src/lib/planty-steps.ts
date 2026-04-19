import type { DialogNode, StepCallback } from './types.js';

/**
 * Cross-module step hook registry.
 *
 * Create one shared instance and import it wherever you need to react to
 * Planty steps — no reference to the <Planty> component required.
 *
 * @example
 * // tutorial-steps.ts
 * export const steps = createPlantySteps();
 *
 * // graph-editor.ts
 * steps.before('highlight_graph', () => graphEditor.setHighlight(true));
 * steps.after ('highlight_graph', () => graphEditor.setHighlight(false));
 *
 * // +page.svelte
 * <Planty {config} {steps} />
 */
export class PlantySteps {
	private _before = new Map<string, StepCallback[]>();
	private _after  = new Map<string, StepCallback[]>();

	/** Register a handler to run before `nodeId` becomes active. Chainable. */
	before(nodeId: string, fn: StepCallback): this {
		const list = this._before.get(nodeId) ?? [];
		this._before.set(nodeId, [...list, fn]);
		return this;
	}

	/** Register a handler to run after the user leaves `nodeId`. Chainable. */
	after(nodeId: string, fn: StepCallback): this {
		const list = this._after.get(nodeId) ?? [];
		this._after.set(nodeId, [...list, fn]);
		return this;
	}

	/** Remove all handlers for a node (or all nodes if omitted). */
	clear(nodeId?: string) {
		if (nodeId) {
			this._before.delete(nodeId);
			this._after.delete(nodeId);
		} else {
			this._before.clear();
			this._after.clear();
		}
	}

	/** @internal — called by Planty */
	async runBefore(nodeId: string, node: DialogNode): Promise<void> {
		for (const fn of this._before.get(nodeId) ?? []) {
			await fn(nodeId, node);
		}
	}

	/** @internal — called by Planty */
	async runAfter(nodeId: string, node: DialogNode): Promise<void> {
		for (const fn of this._after.get(nodeId) ?? []) {
			await fn(nodeId, node);
		}
	}
}

export function createPlantySteps(): PlantySteps {
	return new PlantySteps();
}
