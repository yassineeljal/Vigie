import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { Background, Controls, MarkerType, ReactFlow, type Edge, type Node } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { commands, type GraphData, type GraphNode } from "../../lib/ipc";
import { unwrap } from "../../lib/result";
import { CRITICALITY_LABELS } from "../../lib/labels";

const COL = 300;
const ROW = 90;

function borderFor(n: GraphNode): string {
  if (n.single_point_of_failure) return "#ef4444";
  if (n.pivot) return "#f59e0b";
  return "#3f3f46";
}

function describe(n: GraphNode): string {
  const parts = [n.label];
  if (n.kind === "unlinked_identity") parts.push("identité hors inventaire");
  if (n.criticality) parts.push(`criticité ${CRITICALITY_LABELS[n.criticality].toLowerCase()}`);
  if (n.dependents > 0) parts.push(`${n.dependents} compte(s) en dépendent`);
  if (n.pivot) parts.push("compte pivot");
  if (n.single_point_of_failure) parts.push("point de défaillance unique");
  return parts.join(", ");
}

export function layout(data: GraphData): { nodes: Node[]; edges: Edge[] } {
  const perLevel = new Map<number, number>();
  const nodes: Node[] = data.nodes.map((n) => {
    const row = perLevel.get(n.level) ?? 0;
    perLevel.set(n.level, row + 1);
    return {
      id: n.id,
      position: { x: n.level * COL, y: row * ROW },
      data: { label: `${n.label}${n.dependents ? ` (${n.dependents})` : ""}` },
      ariaLabel: describe(n),
      style: {
        background: n.kind === "unlinked_identity" ? "#27272a" : "#18181b",
        color: "#f4f4f5",
        border: `2px ${n.kind === "unlinked_identity" ? "dashed" : "solid"} ${borderFor(n)}`,
        borderRadius: 12,
        padding: 10,
        width: 220,
        fontSize: 13,
      },
    };
  });
  const edges: Edge[] = data.edges.map((e) => ({
    id: `${e.from}-${e.to}`,
    source: e.from,
    target: e.to,
    label: e.via === "recovery" ? "récupération" : "connexion",
    markerEnd: { type: MarkerType.ArrowClosed },
    style: { stroke: "#71717a" },
    labelStyle: { fill: "#a1a1aa", fontSize: 11 },
    labelBgStyle: { fill: "#09090b" },
  }));
  return { nodes, edges };
}

export function GraphScreen() {
  const { data } = useQuery({
    queryKey: ["graph"],
    queryFn: () => unwrap(commands.getRecoveryGraph()),
  });
  const flow = useMemo(() => (data ? layout(data) : null), [data]);
  if (!data || !flow) return null;

  const critical = data.nodes.filter((n) => n.pivot || n.single_point_of_failure);
  return (
    <section className="flex h-full flex-col gap-4">
      <div>
        <h2 className="text-xl font-semibold">Graphe de récupération</h2>
        <p className="text-sm text-zinc-400">
          Une flèche va d'un compte vers celui dont il dépend pour être récupéré (ex. ton courriel).
          Si le nœud de droite tombe, tout ce qui pointe vers lui tombe aussi.{" "}
          <span className="text-amber-400">Orange : compte pivot</span> ·{" "}
          <span className="text-red-400">Rouge : point de défaillance unique</span> · Pointillés :
          identité hors inventaire.
        </p>
      </div>
      {data.nodes.length === 0 ? (
        <p className="text-sm text-zinc-500">
          Aucune dépendance à afficher. Renseigne les identités de connexion/récupération de tes
          comptes et lie chaque courriel à son compte (onglet Identités).
        </p>
      ) : (
        <>
          <div
            className="min-h-0 flex-1 rounded-xl border border-zinc-800"
            aria-label="Graphe interactif"
          >
            <ReactFlow
              nodes={flow.nodes}
              edges={flow.edges}
              fitView
              colorMode="dark"
              nodesConnectable={false}
            >
              <Background />
              <Controls showInteractive={false} />
            </ReactFlow>
          </div>
          <div aria-label="Alternative textuelle">
            <h3 className="text-sm font-medium text-zinc-300">Points sensibles</h3>
            {critical.length === 0 ? (
              <p className="text-sm text-emerald-400">
                Aucun pivot ni point de défaillance unique détecté.
              </p>
            ) : (
              <ul className="list-inside list-disc text-sm text-zinc-300">
                {critical.map((n) => (
                  <li key={n.id}>{describe(n)}</li>
                ))}
              </ul>
            )}
          </div>
        </>
      )}
    </section>
  );
}
