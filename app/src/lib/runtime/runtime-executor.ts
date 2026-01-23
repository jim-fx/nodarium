import type { SettingsToStore } from '$lib/settings/app-settings.svelte';
import { RemoteNodeRegistry } from '@nodarium/registry';
import type {
  Graph,
  NodeDefinition,
  NodeInput,
  NodeRegistry,
  RuntimeExecutor,
  SyncCache
} from '@nodarium/types';
import {
  createLogger,
  createWasmWrapper,
  encodeFloat,
  type PerformanceStore
} from '@nodarium/utils';
import { DevSettingsType } from '../../routes/dev/settings.svelte';
import { logInt32ArrayChanges } from './helpers';
import type { RuntimeNode } from './types';

const log = createLogger('runtime-executor');
// log.mute(); // Keep logging enabled for debug info

const remoteRegistry = new RemoteNodeRegistry('');

type WasmExecute = (outputPos: number, args: number[]) => number;

function getValue(input: NodeInput, value?: unknown): number | number[] | Int32Array {
  if (value === undefined && 'value' in input) {
    value = input.value;
  }

  switch (input.type) {
    case 'float':
      return encodeFloat(value as number);

    case 'select':
      return (value as number) ?? 0;

    case 'vec3': {
      const arr = Array.isArray(value) ? value : [];
      return [0, arr.length + 1, ...arr.map(v => encodeFloat(v)), 1, 1];
    }
  }

  if (Array.isArray(value)) {
    if (input.type === 'vec3' || input.type === 'shape') {
      return [
        0,
        value.length + 1,
        ...value.map((v) => encodeFloat(v)),
        1,
        1
      ] as number[];
    }
    return [0, value.length + 1, ...value, 1, 1] as number[];
  }

  if (typeof value === 'boolean') return value ? 1 : 0;
  if (typeof value === 'number') return value;
  if (value instanceof Int32Array) return value;

  throw new Error(`Unsupported input type: ${input.type}`);
}

function compareInt32(a: Int32Array, b: Int32Array): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

export type Pointer = {
  start: number;
  end: number;
  _title?: string;
};

  private seed = Math.floor(Math.random() * 100000000);
  private debugData: Record<number, { type: string; data: Int32Array }> = {};

  perf?: PerformanceStore;

  constructor(
    private readonly registry: NodeRegistry,
    public cache?: SyncCache<Int32Array>
  ) {
    this.cache = undefined;
    this.refreshView();
    log.info('MemoryRuntimeExecutor initialized');
  }

  private refreshView(): void {
    this.memoryView = new Int32Array(this.memory.buffer);
    log.info(`Memory view refreshed, length: ${this.memoryView.length}`);
  }

  public getMemory(): Int32Array {
    return new Int32Array(this.memory.buffer);
  }

  private map = new Map<string, { definition: NodeDefinition; execute: WasmExecute }>();
  private async getNodeDefinitions(graph: Graph) {
    if (this.registry.status !== 'ready') {
      throw new Error('Node registry is not ready');
    }

    await this.registry.load(graph.nodes.map(n => n.type));
    log.info(`Loaded ${graph.nodes.length} node types from registry`);

    for (const { type } of graph.nodes) {
      if (this.map.has(type)) continue;

      const def = this.registry.getNode(type);
      if (!def) continue;

      log.info(`Fetching WASM for node type: ${type}`);
      const buffer = await remoteRegistry.fetchArrayBuffer(`nodes/${type}.wasm`);
      const wrapper = createWasmWrapper(buffer, this.memory);

      this.map.set(type, {
        definition: def,
        execute: wrapper.execute
      });
      log.info(`Node type ${type} loaded and wrapped`);
    }

    return this.map;
  }

  private async addMetaData(graph: Graph) {
    this.nodes = await this.getNodeDefinitions(graph);
    log.info(`Metadata added for ${this.nodes.size} nodes`);

    const graphNodes = graph.nodes.map(node => {
      const n = node as RuntimeNode;
      n.state = {
        depth: 0,
        children: [],
        parents: [],
        inputNodes: {}
      };
      return n;
    });

    const outputNode = graphNodes.find(n => n.type.endsWith('/output') || n.type.endsWith('/debug'))
      ?? graphNodes[0];

    const nodeMap = new Map(graphNodes.map(n => [n.id, n]));

    // loop through all edges and assign the parent and child nodes to each node
    for (const edge of graph.edges) {
      const [parentId, /*_parentOutput*/, childId, childInput] = edge;
      const parent = nodeMap.get(parentId);
      const child = nodeMap.get(childId);
      if (!parent || !child) continue;

      parent.state.children.push(child);
      child.state.parents.push(parent);
      child.state.inputNodes[childInput] = parent;
    }

    const nodes = new Map<number, RuntimeNode>();

    // loop through all the nodes and assign each nodes its depth
    const stack = [outputNode, ...graphNodes.filter(n => n.type.endsWith('/debug'))];
    while (stack.length) {
      const node = stack.pop()!;
      for (const parent of node.state.parents) {
        parent.state.depth = node.state.depth + 1;
        stack.push(parent);
      }
      nodes.set(node.id, node);
    }

    for (const node of graphNodes) {
      if (node.type.endsWith('/debug')) {
        node.state = node.state || {};
        const parent = node.state.parents[0];
        if (parent) {
          node.state.depth = parent.state.depth - 1;
          parent.state.debugNode = true;
        }
        nodes.set(node.id, node);
      }
    }

    const _nodes = [...nodes.values()];

    return [outputNode, _nodes] as const;
  }

  private writeToMemory(value: number | number[] | Int32Array, title?: string): Pointer {
    const start = this.offset;

    if (typeof value === 'number') {
      this.memoryView[this.offset++] = value;
    } else {
      this.memoryView.set(value, this.offset);
      this.offset += value.length;
    }

    let a = performance.now();
    this.debugData = {};

    // Then we add some metadata to the graph
    const [_outputNode, nodes] = await this.addMetaData(graph);

    /*
     * Here we sort the nodes into buckets, which we then execute one by one
     * +-b2-+-b1-+---b0---+
     * |    |    |        |
     * | n3 | n2 | Output |
     * | n6 | n4 | Level  |
     * |    | n5 |        |
     * |    |    |        |
     * +----+----+--------+
     */

    // we execute the nodes from the bottom up
    const sortedNodes = nodes.sort(
      (a, b) => (b.state?.depth || 0) - (a.state?.depth || 0)
    );

    console.log({ settings });

    this.printMemory();
    const seedPtr = this.writeToMemory(this.seed, 'seed');

    const settingPtrs = new Map<string, Pointer>(
      Object.entries(settings).map((
        [key, value]
      ) => [key as string, this.writeToMemory(value as number, `setting.${key}`)])
    );

    for (const node of sortedNodes) {
      const node_type = this.nodes.get(node.type)!;

      console.log('---------------');
      console.log('STARTING NODE EXECUTION', node_type.definition.id + '/' + node.id);
      this.printMemory();

      // console.log(node_type.definition.inputs);
      const inputs = Object.entries(node_type.definition.inputs || {}).map(
        ([key, input]) => {
          // We should probably initially write this to memory
          if (input.type === 'seed') {
            return seedPtr;
          }

          const title = `${node.id}.${key}`;

          // We should probably initially write this to memory
          // If the input is linked to a setting, we use that value
          // TODO: handle nodes which reference undefined settings
          if (input.setting) {
            return settingPtrs.get(input.setting)!;
          }

          // check if the input is connected to another node
          const inputNode = node.state.inputNodes[key];
          if (inputNode) {
            if (this.results[inputNode.id] === undefined) {
              throw new Error(
                `Node ${node.type}/${node.id} is missing input from node ${inputNode.type}/${inputNode.id}`
              );
            }
            return this.results[inputNode.id];
          }

          // If the value is stored in the node itself, we use that value
          if (node.props?.[key] !== undefined) {
            const value = getValue(input, node.props[key]);
            console.log(`Writing prop for ${node.id} -> ${key} to memory`, node.props[key], value);
            return this.writeToMemory(value, title);
          }

          return this.writeToMemory(getValue(input), title);
        }
      );

      this.printMemory();

      if (!node_type || !node.state || !node_type.execute) {
        log.warn(`Node ${node.id} has no definition`);
        continue;
      }

      this.inputPtrs[node.id] = inputs;
      const args = inputs.map(s => [s.start, s.end]).flat();
      console.log('ARGS', inputs);

      this.printMemory();
      try {
        a = performance.now();
        const encoded_inputs = concatEncodedArrays(inputs);
        b = performance.now();
        this.perf?.addPoint('encoded-inputs', b - a);

        a = performance.now();
        const inputHash = `node-${node.id}-${fastHashArrayBuffer(encoded_inputs)}`;
        b = performance.now();
        this.perf?.addPoint('hash-inputs', b - a);

        const cachedValue = this.cache?.get(inputHash);
        if (cachedValue !== undefined) {
          log.log(`Using cached value for ${node_type.id || node.id}`);
          this.perf?.addPoint('cache-hit', 1);
          results[node.id] = cachedValue as Int32Array;
          if (node.state.debugNode && node_type.outputs) {
            this.debugData[node.id] = {
              type: node_type.outputs[0],
              data: cachedValue
            };
          }
          continue;
        }
        this.perf?.addPoint('cache-hit', 0);

        log.group(`executing ${node_type.id}-${node.id}`);
        log.log(`Inputs:`, inputs);
        a = performance.now();
        results[node.id] = node_type.execute(encoded_inputs);
        if (node.state.debugNode && node_type.outputs) {
          this.debugData[node.id] = {
            type: node_type.outputs[0],
            data: results[node.id]
          };
        }
        log.log('Executed', node.type, node.id);
        b = performance.now();

        if (this.cache && node.id !== outputNode.id) {
          this.cache.set(inputHash, this.results[node.id]);
        }

        this.perf?.addPoint('node/' + node_type.id, b - a);
        log.log('Result:', results[node.id]);
        log.groupEnd();
      } catch (e) {
        console.error(`Failed to execute node ${node.type}/${node.id}`, e);
        this.isRunning = false;
      }
    }

    this.isRunning = true;
    log.info('Execution started');

    try {
      this.offset = 0;
      this.results = {};
      this.inputPtrs = {};
      this.allPtrs = [];
      this.seed += 2;

      this.refreshView();

      const [outputNode, nodes] = await this.addMetaData(graph);

      const sortedNodes = [...nodes].sort(
        (a, b) => (b.state.depth ?? 0) - (a.state.depth ?? 0)
      );

      const seedPtr = this.writeToMemory(this.seed, 'seed');

      const settingPtrs = new Map<string, Pointer>();
      for (const [key, value] of Object.entries(settings)) {
        const ptr = this.writeToMemory(value as number, `setting.${key}`);
        settingPtrs.set(key, ptr);
      }

      let lastNodePtr: Pointer | undefined = undefined;

      for (const node of sortedNodes) {
        const nodeType = this.nodes.get(node.type);
        if (!nodeType) continue;

        log.info(`Executing node: ${node.id} (type: ${node.type})`);

        const inputs = Object.entries(nodeType.definition.inputs || {}).map(
          ([key, input]) => {
            if (input.type === 'seed') return seedPtr;

            if (input.setting) {
              const ptr = settingPtrs.get(input.setting);
              if (!ptr) throw new Error(`Missing setting: ${input.setting}`);
              return ptr;
            }

            const src = node.state.inputNodes[key];
            if (src) {
              const res = this.results[src.id];
              if (!res) {
                throw new Error(`Missing input from ${src.type}/${src.id}`);
              }
              return res;
            }

            if (node.props?.[key] !== undefined) {
              return this.writeToMemory(
                getValue(input, node.props[key]),
                `${node.id}.${key}`
              );
            }

            return this.writeToMemory(getValue(input), `${node.id}.${key}`);
          }
        );

        this.inputPtrs[node.id] = inputs;
        const args = inputs.flatMap(p => [p.start * 4, p.end * 4]);

        log.info(`Executing node ${node.type}/${node.id}`);
        const memoryBefore = this.memoryView.slice(0, this.offset);
        const bytesWritten = nodeType.execute(this.offset * 4, args);
        this.refreshView();
        const memoryAfter = this.memoryView.slice(0, this.offset);
        logInt32ArrayChanges(memoryBefore, memoryAfter);
        this.refreshView();

        const outLen = bytesWritten >> 2;
        const outputStart = this.offset;

        if (
          args.length === 2
          && inputs[0].end - inputs[0].start === outLen
          && compareInt32(
            this.memoryView.slice(inputs[0].start, inputs[0].end),
            this.memoryView.slice(outputStart, outputStart + outLen)
          )
        ) {
          this.results[node.id] = inputs[0];
          this.allPtrs.push(this.results[node.id]);
          log.info(`Node ${node.id} result reused input memory`);
        } else {
          this.results[node.id] = {
            start: outputStart,
            end: outputStart + outLen,
            _title: `${node.id} ->`
          };
          this.allPtrs.push(this.results[node.id]);
          this.offset += outLen;
          lastNodePtr = this.results[node.id];
          log.info(
            `Node ${node.id} wrote result to memory: start=${outputStart}, end=${outputStart + outLen
            }`
          );
        }
      }

      const res = this.results[outputNode.id] ?? lastNodePtr;
      if (!res) throw new Error('Output node produced no result');

      log.info(`Execution finished, output pointer: start=${res.start}, end=${res.end}`);
      this.refreshView();
      return this.memoryView.slice(res.start, res.end);
    } catch (e) {
      log.info('Execution error:', e);
      console.error(e);
    } finally {
      this.isRunning = false;
      console.log('Final Memory', [...this.memoryView.slice(0, 20)]);
      this.perf?.endPoint('runtime');
      log.info('Executor state reset');
    }
  }

  getDebugData() {
    return this.debugData;
  }

  getPerformanceData() {
    return this.perf?.get();
  }
}
