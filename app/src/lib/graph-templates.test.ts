import { grid } from '$lib/graph-templates/grid';
import { tree } from '$lib/graph-templates/tree';
import { describe, expect, it } from 'vitest';

describe('graph-templates', () => {
  describe('grid', () => {
    it('should create a grid graph with nodes and edges', () => {
      const result = grid(2, 3);
      expect(result.nodes.length).toBeGreaterThan(0);
      expect(result.edges.length).toBeGreaterThan(0);
    });

    it('should have output node at the end', () => {
      const result = grid(1, 1);
      const outputNode = result.nodes.find(n => n.type === 'max/plantarium/output');
      expect(outputNode).toBeDefined();
    });

    it('should create nodes based on grid dimensions', () => {
      const result = grid(2, 2);
      const mathNodes = result.nodes.filter(n => n.type === 'max/plantarium/math');
      expect(mathNodes.length).toBeGreaterThan(0);
      const outputNode = result.nodes.find(n => n.type === 'max/plantarium/output');
      expect(outputNode).toBeDefined();
    });

    it('should have output node at the end', () => {
      const result = grid(1, 1);
      const outputNode = result.nodes.find(n => n.type === 'max/plantarium/output');
      expect(outputNode).toBeDefined();
    });

    it('should create nodes based on grid dimensions', () => {
      const result = grid(2, 2);
      const mathNodes = result.nodes.filter(n => n.type === 'max/plantarium/math');
      expect(mathNodes.length).toBeGreaterThan(0);
      const outputNode = result.nodes.find(n => n.type === 'max/plantarium/output');
      expect(outputNode).toBeDefined();
    });

    it('should have valid node positions', () => {
      const result = grid(3, 2);

      result.nodes.forEach(node => {
        expect(node.position).toHaveLength(2);
        expect(typeof node.position[0]).toBe('number');
        expect(typeof node.position[1]).toBe('number');
      });
    });

    it('should generate valid graph structure', () => {
      const result = grid(2, 2);

      result.nodes.forEach(node => {
        expect(typeof node.id).toBe('number');
        expect(node.type).toBeTruthy();
      });

      result.edges.forEach(edge => {
        expect(edge).toHaveLength(4);
      });
    });
  });

  describe('tree', () => {
    it('should create a tree graph with specified depth', () => {
      const result = tree(0);

      expect(result.nodes.length).toBeGreaterThan(0);
      expect(result.edges.length).toBeGreaterThan(0);
    });

    it('should have root output node', () => {
      const result = tree(2);

      const outputNode = result.nodes.find(n => n.type === 'max/plantarium/output');
      expect(outputNode).toBeDefined();
      expect(outputNode?.id).toBe(0);
    });

    it('should increase node count with depth', () => {
      const tree0 = tree(0);
      const tree1 = tree(1);
      const tree2 = tree(2);

      expect(tree0.nodes.length).toBeLessThan(tree1.nodes.length);
      expect(tree1.nodes.length).toBeLessThan(tree2.nodes.length);
    });

    it('should create binary tree structure', () => {
      const result = tree(2);

      const mathNodes = result.nodes.filter(n => n.type === 'max/plantarium/math');
      expect(mathNodes.length).toBeGreaterThan(0);

      const edgeCount = result.edges.length;
      expect(edgeCount).toBe(result.nodes.length - 1);
    });

    it('should have valid node positions', () => {
      const result = tree(3);

      result.nodes.forEach(node => {
        expect(node.position).toHaveLength(2);
        expect(typeof node.position[0]).toBe('number');
        expect(typeof node.position[1]).toBe('number');
      });
    });
  });
});
