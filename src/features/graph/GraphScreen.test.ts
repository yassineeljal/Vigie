import { expect, it } from "vitest";
import type { GraphData } from "../../lib/ipc";
import { layout } from "./GraphScreen";

const data: GraphData = {
  nodes: [
    {
      id: "a1",
      kind: "account",
      label: "Instagram",
      criticality: "medium",
      dependents: 0,
      pivot: false,
      single_point_of_failure: false,
      level: 0,
    },
    {
      id: "a2",
      kind: "account",
      label: "Facebook",
      criticality: "medium",
      dependents: 0,
      pivot: false,
      single_point_of_failure: false,
      level: 0,
    },
    {
      id: "a3",
      kind: "account",
      label: "Gmail",
      criticality: "critical",
      dependents: 2,
      pivot: false,
      single_point_of_failure: true,
      level: 1,
    },
  ],
  edges: [
    { from: "a1", to: "a3", via: "recovery" },
    { from: "a2", to: "a3", via: "login" },
  ],
};

it("dispose les nœuds par niveau et signale les points de défaillance", () => {
  const { nodes, edges } = layout(data);
  expect(nodes.map((n) => n.position)).toEqual([
    { x: 0, y: 0 },
    { x: 0, y: 90 },
    { x: 300, y: 0 },
  ]);
  expect(nodes[2].ariaLabel).toContain("point de défaillance unique");
  expect(nodes[2].ariaLabel).toContain("criticité critique");
  expect(String(nodes[2].style?.border)).toContain("var(--danger)");
  expect(edges.map((e) => e.label)).toEqual(["récupération", "connexion"]);
});
